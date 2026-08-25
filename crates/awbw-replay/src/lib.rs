//! Differential verification of the engine against recorded AWBW games.
//!
//! Each recorded turn is an independent test case: load the snapshot taken at
//! the start of the turn, replay that turn's orders through the engine, and
//! diff the result against the snapshot taken at the start of the next turn.
//! Treating turns independently means one wrong rule shows up as a divergence
//! on the turns that exercise it, rather than poisoning the whole game.
//!
//! Combat luck (AWBW rolls 0-9% per attack) cannot be reproduced, so damage is
//! checked as a *range*: the recorded outcome must be achievable under the
//! engine's own min/max spread. HP differences within one displayed point of
//! the record are attributed to luck and reported separately from real bugs.

pub mod schema;

use std::collections::HashMap;
use std::sync::Arc;

use awbw_engine::actions::{Action, Engine};
use awbw_engine::combat;
use awbw_engine::map::{Map, Pos};
use awbw_engine::movement::Reach;
use awbw_engine::state::{
    GameSettings, GameState, Player, PlayerId, UnitId, CAPTURE_FULL,
};
use awbw_engine::types::{UnitType, Weather};
use awbw_engine::data;

use schema::{unwrap_vision, BuildingRec, Replay, Turn, UnitRec};

/// Reads a number that AWBW may have encoded as an integer, a float, or a
/// string. Its JSON is inconsistent about this even within one payload, and a
/// strict `as_i64` silently drops the string cases.
fn as_num(value: Option<&serde_json::Value>) -> Option<i64> {
    let value = value?;
    if let Some(n) = value.as_i64() {
        return Some(n);
    }
    if let Some(f) = value.as_f64() {
        return Some(f.round() as i64);
    }
    value.as_str()?.trim().parse::<f64>().ok().map(|f| f.round() as i64)
}

/// One thing the engine got wrong, or could not check.
#[derive(Debug, Clone)]
pub struct Divergence {
    pub turn_index: usize,
    pub day: u16,
    pub kind: &'static str,
    pub detail: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub game_id: i64,
    pub turns_checked: usize,
    pub actions_applied: usize,
    pub actions_unsupported: HashMap<String, usize>,
    pub checks: usize,
    pub divergences: Vec<Divergence>,
    /// HP differences of at most one displayed point, i.e. attributable to luck.
    pub luck_slack: usize,
    pub skipped: Option<String>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.divergences.is_empty()
    }

    pub fn counts_by_kind(&self) -> HashMap<&'static str, usize> {
        let mut out = HashMap::new();
        for d in &self.divergences {
            *out.entry(d.kind).or_insert(0) += 1;
        }
        out
    }
}

/// Translates AWBW unit names into engine unit types.
fn unit_type_by_name(name: &str) -> Option<UnitType> {
    UnitType::ALL
        .into_iter()
        .find(|t| t.stats().name.eq_ignore_ascii_case(name))
        .or_else(|| match name {
            // AWBW writes a few names differently in different places.
            "Pipe Runner" | "PipeRunner" => Some(UnitType::Piperunner),
            "MegaTank" | "Mega tank" => Some(UnitType::MegaTank),
            "Black bomb" => Some(UnitType::BlackBomb),
            "Anti Air" | "AntiAir" => Some(UnitType::AntiAir),
            _ => None,
        })
}

/// COs with no day-to-day ability, so the engine's vanilla 100/100 combat
/// numbers are correct for them. Every other CO changes attack, defence, cost
/// or movement in ways the engine does not model yet.
pub fn has_only_plain_cos(replay: &Replay) -> bool {
    replay
        .players
        .iter()
        .all(|p| matches!(p.co_name.as_str(), "Andy" | "No CO"))
}

/// Whether any CO power was activated, which changes stats mid-turn.
pub fn uses_powers(replay: &Replay) -> bool {
    replay.turns.iter().any(|t| {
        t.actions
            .iter()
            .any(|a| a.get("action").and_then(|v| v.as_str()) == Some("Power"))
    })
}

fn weather_from_code(code: &str) -> Weather {
    match code {
        "R" => Weather::Rain,
        "S" => Weather::Snow,
        _ => Weather::Clear,
    }
}

/// Maps AWBW's player ids onto dense engine player indices, ordered by seat.
pub struct PlayerMap {
    order: Vec<i64>,
}

impl PlayerMap {
    fn new(replay: &Replay) -> Self {
        let mut players: Vec<&schema::PlayerInfo> = replay.players.iter().collect();
        players.sort_by_key(|p| p.order);
        PlayerMap {
            order: players.iter().map(|p| p.id).collect(),
        }
    }

    fn index(&self, awbw_id: i64) -> Option<PlayerId> {
        self.order.iter().position(|&id| id == awbw_id).map(|i| i as PlayerId)
    }
}

/// The engine state for one recorded turn, plus the id translation needed to
/// follow the recorded orders.
struct Loaded {
    engine: Engine,
    /// AWBW unit id -> engine unit id.
    ids: HashMap<i64, UnitId>,
    /// Engine unit id -> AWBW unit id.
    ///
    /// The engine recycles the slot of a destroyed unit, so a build later in
    /// the same turn can land on the id a casualty used to hold. Without this
    /// reverse map the casualty looks alive, because its stale forward mapping
    /// still resolves to an occupied slot.
    awbw_of: HashMap<UnitId, i64>,
}

impl Loaded {
    fn bind(&mut self, awbw_id: i64, unit: UnitId) {
        self.ids.insert(awbw_id, unit);
        self.awbw_of.insert(unit, awbw_id);
    }

    /// The engine unit currently holding this AWBW id, if the slot has not
    /// since been handed to someone else.
    fn live(&self, awbw_id: i64) -> Option<UnitId> {
        let unit = *self.ids.get(&awbw_id)?;
        (self.awbw_of.get(&unit) == Some(&awbw_id)).then_some(unit)
    }
}

pub struct Verifier<'a> {
    replay: &'a Replay,
    players: PlayerMap,
    map: Arc<Map>,
    settings: GameSettings,
    /// country code -> player index, for reading ownership off terrain ids.
    country_owner: HashMap<String, PlayerId>,
}

impl<'a> Verifier<'a> {
    pub fn new(replay: &'a Replay) -> Result<Self, String> {
        let flat: Vec<u16> = replay.terrain.iter().flatten().copied().collect();
        let map = Map::from_awbw_ids(replay.width, replay.height, &flat)
            .map_err(|e| format!("map: {e}"))?;

        let players = PlayerMap::new(replay);
        let mut country_owner = HashMap::new();
        for p in &replay.players {
            if let Some(index) = players.index(p.id) {
                country_owner.insert(p.country.clone(), index);
            }
        }

        let settings = GameSettings {
            funds_per_property: replay.funds_per_property,
            capture_limit: replay.capture_limit,
            fog: replay.fog,
            ..GameSettings::default()
        };

        Ok(Verifier {
            replay,
            players,
            map: Arc::new(map),
            settings,
            country_owner,
        })
    }

    /// Rebuilds the engine state from a recorded snapshot.
    fn load_turn(&self, turn: &Turn) -> Result<Loaded, String> {
        let players: Vec<Player> = self
            .replay
            .players
            .iter()
            .map(|p| {
                let mut player = Player::new(
                    turn.funds.get(&p.id.to_string()).copied().unwrap_or(0).max(0) as u32,
                    p.team.bytes().fold(0u8, |a, b| a.wrapping_add(b)),
                );
                player.eliminated = turn
                    .eliminated
                    .get(&p.id.to_string())
                    .copied()
                    .unwrap_or(false);
                if let Some(co) = awbw_engine::co_data::co_by_name(&p.co_name) {
                    player.co = co;
                }
                player
            })
            .collect();

        let mut state = GameState::new(self.map.clone(), self.settings, players, &[]);
        state.day = turn.day;
        state.weather = weather_from_code(&self.replay.weather);
        state.current = self
            .players
            .index(turn.active)
            .ok_or_else(|| format!("unknown active player {}", turn.active))?;

        self.apply_buildings(&mut state, &turn.buildings);

        let mut ids = HashMap::new();
        // Units on the board first, so transports exist before their cargo.
        for rec in turn.units.iter().filter(|u| !u.carried) {
            let typ = unit_type_by_name(&rec.typ)
                .ok_or_else(|| format!("unknown unit type {:?}", rec.typ))?;
            let owner = self
                .players
                .index(rec.player)
                .ok_or_else(|| format!("unknown unit owner {}", rec.player))?;
            let id = state.spawn(typ, owner, Pos::new(rec.x, rec.y));
            apply_unit_record(&mut state, id, rec);
            ids.insert(rec.id, id);
        }
        for rec in turn.units.iter().filter(|u| u.carried) {
            let typ = unit_type_by_name(&rec.typ)
                .ok_or_else(|| format!("unknown unit type {:?}", rec.typ))?;
            let owner = self
                .players
                .index(rec.player)
                .ok_or_else(|| format!("unknown unit owner {}", rec.player))?;
            // Find the transport that lists this unit as cargo.
            let transport = turn
                .units
                .iter()
                .find(|t| t.cargo.contains(&rec.id))
                .and_then(|t| ids.get(&t.id).copied());
            let id = match transport {
                Some(t) => match state.spawn_into(typ, owner, t) {
                    Some(id) => id,
                    None => state.spawn(typ, owner, Pos::new(rec.x, rec.y)),
                },
                None => state.spawn(typ, owner, Pos::new(rec.x, rec.y)),
            };
            apply_unit_record(&mut state, id, rec);
            ids.insert(rec.id, id);
        }

        let awbw_of = ids.iter().map(|(&awbw, &unit)| (unit, awbw)).collect();
        Ok(Loaded {
            engine: Engine::new(state, 0x5EED),
            ids,
            awbw_of,
        })
    }

    fn apply_buildings(&self, state: &mut GameState, records: &[BuildingRec]) {
        for rec in records {
            let owner = data::terrain_by_awbw_id(rec.terrain_id)
                .and_then(|info| info.country)
                .and_then(|c| self.country_owner.get(c).copied());
            let pos = Pos::new(rec.x, rec.y);
            if let Some(b) = state.building_at_mut(pos) {
                b.owner = owner;
                b.capture_remaining = rec.capture;
            }
        }
    }

    pub fn verify(&self) -> Report {
        let mut report = Report {
            game_id: self.replay.game_id,
            ..Report::default()
        };

        for i in 0..self.replay.turns.len().saturating_sub(1) {
            let turn = &self.replay.turns[i];
            let next = &self.replay.turns[i + 1];

            let mut loaded = match self.load_turn(turn) {
                Ok(l) => l,
                Err(e) => {
                    report.divergences.push(Divergence {
                        turn_index: i,
                        day: turn.day,
                        kind: "load",
                        detail: e,
                    });
                    continue;
                }
            };
            report.turns_checked += 1;

            let mut ended = false;
            for action in &turn.actions {
                match self.apply_action(&mut loaded, action, i, turn.day, &mut report) {
                    ActionOutcome::Applied => report.actions_applied += 1,
                    ActionOutcome::Ended => {
                        ended = true;
                        report.actions_applied += 1;
                    }
                    ActionOutcome::Unsupported(kind) => {
                        *report.actions_unsupported.entry(kind).or_insert(0) += 1;
                    }
                    ActionOutcome::Terminal => {
                        ended = true;
                        break;
                    }
                }
                if ended {
                    break;
                }
            }

            if ended {
                self.diff_states(&loaded, next, i, turn.day, &mut report);
            }
        }

        report
    }

    fn apply_action(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) -> ActionOutcome {
        let kind = action.get("action").and_then(|a| a.as_str()).unwrap_or("?");
        match kind {
            "Build" => self.do_build(loaded, action, turn_index, day, report),
            "Move" => self.do_move(loaded, action, turn_index, day, report),
            "Capt" => self.do_capture(loaded, action, turn_index, day, report),
            "Fire" => self.do_fire(loaded, action, turn_index, day, report),
            "Join" => self.do_join(loaded, action, turn_index, day, report),
            "Load" => self.do_load(loaded, action, turn_index, day, report),
            "Unload" => self.do_unload(loaded, action, turn_index, day, report),
            "Supply" | "Repair" => self.do_supply(loaded, action, turn_index, day, report),
            "Hide" | "Unhide" => self.do_hide(loaded, action, kind == "Hide"),
            "Delete" | "Explode" => self.do_delete(loaded, action),
            // Ending the turn runs the engine's own income, repair and fuel
            // bookkeeping, which is exactly what the next snapshot reflects.
            "End" => {
                let _ = loaded.engine.apply(Action::EndTurn);
                ActionOutcome::Ended
            }
            // A game that stops mid-turn has no next snapshot to compare against.
            "Resign" | "GameOver" | "Eliminated" => ActionOutcome::Terminal,
            other => ActionOutcome::Unsupported(other.to_string()),
        }
    }

    // --- individual action handlers ---------------------------------------

    fn do_build(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) -> ActionOutcome {
        let Some(rec) = action.get("newUnit").and_then(unwrap_vision) else {
            return ActionOutcome::Unsupported("Build/newUnit".into());
        };
        let name = rec.get("units_name").and_then(|v| v.as_str()).unwrap_or("");
        let Some(typ) = unit_type_by_name(name) else {
            return ActionOutcome::Unsupported(format!("Build/{name}"));
        };
        let x = as_num(rec.get("units_x")).unwrap_or(-1) as u8;
        let y = as_num(rec.get("units_y")).unwrap_or(-1) as u8;
        let at = Pos::new(x, y);
        let awbw_id = as_num(rec.get("units_id")).unwrap_or(0);

        report.checks += 1;
        let build = Action::Build { at, typ };
        match loaded.engine.apply(build) {
            Ok(out) => {
                if let Some(id) = out.unit_built {
                    loaded.bind(awbw_id, id);
                }
            }
            Err(e) => {
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "build-illegal",
                    detail: format!("{name} at ({x},{y}) rejected: {e}"),
                });
                // Force it so the rest of the turn still lines up.
                let owner = loaded.engine.state.current;
                let id = loaded.engine.state.spawn(typ, owner, at);
                if let Some(u) = loaded.engine.state.unit_mut(id) {
                    u.moved = true;
                }
                let cost = typ.stats().cost;
                let funds = &mut loaded.engine.state.players[owner as usize].funds;
                *funds = funds.saturating_sub(cost);
                loaded.bind(awbw_id, id);
            }
        }
        ActionOutcome::Applied
    }

    /// Reads the `Move` sub-payload shared by Move, Capt, Fire, Join and Load,
    /// returning the acting unit and the whole recorded route.
    fn move_parts(
        &self,
        loaded: &Loaded,
        move_action: &serde_json::Value,
    ) -> Option<(UnitId, Vec<Pos>)> {
        let unit = move_action.get("unit").and_then(unwrap_vision)?;
        let awbw_id = as_num(unit.get("units_id"))?;
        let id = loaded.live(awbw_id)?;
        let path = move_action.get("paths").and_then(unwrap_vision)?.as_array()?;
        let route: Vec<Pos> = path
            .iter()
            .filter_map(|step| {
                Some(Pos::new(
                    step.get("x")?.as_i64()? as u8,
                    step.get("y")?.as_i64()? as u8,
                ))
            })
            .collect();
        (!route.is_empty()).then_some((id, route))
    }

    /// Checks a recorded move against the engine's movement rules.
    ///
    /// The route AWBW records is the one the player actually clicked, which is
    /// not always the cheapest, so fuel is charged along *that* route rather
    /// than along our shortest path — otherwise every scenic detour reads as a
    /// fuel bug.
    fn check_move_legality(
        &self,
        loaded: &Loaded,
        unit: UnitId,
        route: &[Pos],
        recorded_fuel: Option<i64>,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) {
        report.checks += 1;
        let state = &loaded.engine.state;
        let Some(actor) = state.unit(unit) else {
            return;
        };
        let dest = *route.last().unwrap();

        if route[0] != actor.pos {
            report.divergences.push(Divergence {
                turn_index,
                day,
                kind: "move-origin",
                detail: format!("unit {unit} stands at {:?}, route starts {:?}", actor.pos, route[0]),
            });
        }

        // Walk the recorded route, charging terrain as we go.
        let move_type = actor.move_type();
        let mut cost: u32 = 0;
        let mut broken = None;
        for pair in route.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if from.distance(to) != 1 {
                broken = Some(format!("{from:?} -> {to:?} is not a single step"));
                break;
            }
            match state.map.terrain_at(to).move_cost(state.weather, move_type) {
                Some(step) => cost += step as u32,
                None => {
                    broken = Some(format!("{to:?} is impassable for this unit"));
                    break;
                }
            }
        }
        if let Some(detail) = broken {
            report.divergences.push(Divergence {
                turn_index,
                day,
                kind: "move-path",
                detail: format!("unit {unit}: {detail}"),
            });
            return;
        }

        let budget = actor.typ.stats().move_points.min(actor.fuel) as u32;
        if cost > budget {
            report.divergences.push(Divergence {
                turn_index,
                day,
                kind: "move-over-budget",
                detail: format!(
                    "unit {unit} ({}) to {dest:?}: route costs {cost}, engine allows {budget}",
                    actor.typ.stats().name
                ),
            });
        }

        // The destination must also be reachable at all, which catches units
        // blocked by an enemy the engine places differently.
        let mut reach = Reach::new();
        reach.compute(state, unit);
        if !reach.can_reach(state, dest) {
            report.divergences.push(Divergence {
                turn_index,
                day,
                kind: "move-unreachable",
                detail: format!(
                    "unit {unit} ({}): {:?} -> {dest:?} unreachable",
                    actor.typ.stats().name,
                    actor.pos
                ),
            });
        }

        if let Some(recorded) = recorded_fuel {
            let expected = actor.fuel as i64 - cost as i64;
            if expected != recorded {
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "move-fuel",
                    detail: format!(
                        "unit {unit} ({}) to {dest:?}: engine {expected} fuel, AWBW {recorded}",
                        actor.typ.stats().name
                    ),
                });
            }
        }
    }

    fn do_move(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) -> ActionOutcome {
        let Some((unit, route)) = self.move_parts(loaded, action) else {
            return ActionOutcome::Unsupported("Move/unit".into());
        };
        let recorded_fuel = action
            .get("unit")
            .and_then(unwrap_vision)
            .and_then(|u| u.get("units_fuel"))
            .and_then(|v| v.as_i64());
        self.check_move_legality(loaded, unit, &route, recorded_fuel, turn_index, day, report);
        force_move(&mut loaded.engine, unit, &route);
        ActionOutcome::Applied
    }

    fn do_capture(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) -> ActionOutcome {
        if let Some(mv) = action.get("Move") {
            if let Some((unit, route)) = self.move_parts(loaded, mv) {
                let fuel = mv
                    .get("unit")
                    .and_then(unwrap_vision)
                    .and_then(|u| u.get("units_fuel"))
                    .and_then(|v| v.as_i64());
                self.check_move_legality(loaded, unit, &route, fuel, turn_index, day, report);
                force_move(&mut loaded.engine, unit, &route);
            }
        }
        let Some(info) = action.get("Capt").and_then(|c| c.get("buildingInfo")) else {
            return ActionOutcome::Unsupported("Capt/buildingInfo".into());
        };
        let (Some(x), Some(y)) = (
            info.get("buildings_x").and_then(|v| v.as_i64()),
            info.get("buildings_y").and_then(|v| v.as_i64()),
        ) else {
            return ActionOutcome::Unsupported("Capt/coords".into());
        };
        let pos = Pos::new(x as u8, y as u8);
        let recorded = info.get("buildings_capture").and_then(|v| v.as_i64());

        let Some(id) = loaded.engine.state.unit_id_at(pos) else {
            report.divergences.push(Divergence {
                turn_index,
                day,
                kind: "capture-no-unit",
                detail: format!("nothing standing on {pos:?} to capture it"),
            });
            return ActionOutcome::Applied;
        };

        report.checks += 1;
        let before = loaded
            .engine
            .state
            .building_at(pos)
            .map(|b| b.capture_remaining)
            .unwrap_or(CAPTURE_FULL);
        let hp = loaded.engine.state.unit(id).map(|u| u.display_hp()).unwrap_or(0);
        let expected = before.saturating_sub(hp);
        if let Some(recorded) = recorded {
            // Once a property flips, AWBW reports the counter reset to 20.
            let agrees = expected as i64 == recorded
                || (expected == 0 && recorded == CAPTURE_FULL as i64);
            if !agrees {
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "capture-progress",
                    detail: format!(
                        "{pos:?}: {before} - {hp} HP = engine {expected} left, AWBW {recorded}"
                    ),
                });
            }
        }

        let captor = loaded.engine.state.unit(id).map(|u| u.owner);
        if let Some(b) = loaded.engine.state.building_at_mut(pos) {
            if expected == 0 {
                b.owner = captor;
                b.capture_remaining = CAPTURE_FULL;
            } else {
                b.capture_remaining = expected;
            }
        }
        if let Some(u) = loaded.engine.state.unit_mut(id) {
            u.moved = true;
        }
        ActionOutcome::Applied
    }

    fn do_fire(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) -> ActionOutcome {
        if let Some(mv) = action.get("Move") {
            if let Some((unit, route)) = self.move_parts(loaded, mv) {
                let fuel = mv
                    .get("unit")
                    .and_then(unwrap_vision)
                    .and_then(|u| u.get("units_fuel"))
                    .and_then(|v| v.as_i64());
                self.check_move_legality(loaded, unit, &route, fuel, turn_index, day, report);
                force_move(&mut loaded.engine, unit, &route);
            }
        }

        let Some(info) = action
            .get("Fire")
            .and_then(|f| f.get("combatInfoVision"))
            .and_then(unwrap_vision)
            .and_then(|v| v.get("combatInfo"))
        else {
            return ActionOutcome::Unsupported("Fire/combatInfo".into());
        };

        let read = |key: &str| -> Option<(UnitId, i64, i64)> {
            let side = info.get(key)?;
            let awbw_id = as_num(side.get("units_id"))?;
            let id = loaded.live(awbw_id)?;
            let hp = as_num(side.get("units_hit_points"))?;
            let ammo = as_num(side.get("units_ammo")).unwrap_or(-1);
            Some((id, hp, ammo))
        };

        let attacker = read("attacker");
        let defender = read("defender");

        // The recorded damage must be reachable under the engine's own spread.
        if let (Some((att_id, _, _)), Some((def_id, def_hp, _))) = (attacker, defender) {
            let def_pos = loaded.engine.state.unit(def_id).map(|u| u.pos);
            if let Some(def_pos) = def_pos {
                report.checks += 1;
                let before = loaded.engine.state.unit(def_id).map(|u| u.hp100).unwrap_or(0) as i32;
                if let Some(spread) = loaded.engine.preview_damage(att_id, def_pos) {
                    // AWBW reports displayed HP here, so compare display bands.
                    let hi = combat::display_hp((before - spread.min).max(0));
                    let lo = combat::display_hp((before - spread.max).max(0));
                    if def_hp as i32 > hi || (def_hp as i32) < lo {
                        let att = loaded.engine.state.unit(att_id);
                        let def = loaded.engine.state.unit(def_id);
                        let terrain = loaded.engine.state.map.terrain_at(def_pos);
                        report.divergences.push(Divergence {
                            turn_index,
                            day,
                            kind: "damage-range",
                            detail: format!(
                                "{} (hp {}, ammo {}) -> {} on {terrain:?}: AWBW left {def_hp} HP, \
                                 engine allows {lo}..{hi} (damage {}..{} from {before})",
                                att.map(|u| u.typ.stats().name).unwrap_or("?"),
                                att.map(|u| u.hp100).unwrap_or(0),
                                att.map(|u| u.ammo).unwrap_or(0),
                                def.map(|u| u.typ.stats().name).unwrap_or("?"),
                                spread.min,
                                spread.max
                            ),
                        });
                    }
                } else {
                    report.divergences.push(Divergence {
                        turn_index,
                        day,
                        kind: "damage-impossible",
                        detail: format!("engine says unit {att_id} cannot attack {def_id}"),
                    });
                }
            }
        }

        // Snap to the record: luck is unreproducible, so carrying our own roll
        // forward would make every later check in this turn meaningless.
        for side in [attacker, defender].into_iter().flatten() {
            let (id, hp, ammo) = side;
            if hp <= 0 {
                loaded.engine.state.destroy(id);
                continue;
            }
            if let Some(u) = loaded.engine.state.unit_mut(id) {
                u.hp100 = (hp as u8).saturating_mul(10).min(100);
                if ammo >= 0 {
                    u.ammo = ammo as u8;
                }
                u.moved = true;
            }
        }
        // A defender absent from combatInfo was destroyed outright.
        if defender.is_none() {
            if let Some((att_id, _, _)) = attacker {
                let _ = att_id;
            }
        }
        ActionOutcome::Applied
    }

    fn do_join(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) -> ActionOutcome {
        let Some(mv) = action.get("Move") else {
            return ActionOutcome::Unsupported("Join/Move".into());
        };
        let Some((unit, route)) = self.move_parts(loaded, mv) else {
            return ActionOutcome::Unsupported("Join/unit".into());
        };
        let dest = *route.last().unwrap();
        self.check_move_legality(loaded, unit, &route, None, turn_index, day, report);
        if let Some(other) = loaded.engine.state.unit_id_at(dest) {
            report.checks += 1;
            if loaded.engine.apply(Action::Join { unit, dest }).is_err() {
                // Force the merge so the snapshot diff stays meaningful.
                let (hp, fuel, ammo) = loaded
                    .engine
                    .state
                    .unit(unit)
                    .map(|u| (u.hp100, u.fuel, u.ammo))
                    .unwrap_or((0, 0, 0));
                if let Some(target) = loaded.engine.state.unit_mut(other) {
                    target.hp100 = (target.hp100 + hp).min(100);
                    target.fuel = target.fuel.saturating_add(fuel);
                    target.ammo = target.ammo.saturating_add(ammo);
                    target.moved = true;
                }
                loaded.engine.state.destroy(unit);
            }
        } else {
            force_move(&mut loaded.engine, unit, &route);
        }
        ActionOutcome::Applied
    }

    fn do_load(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) -> ActionOutcome {
        let Some(mv) = action.get("Move") else {
            return ActionOutcome::Unsupported("Load/Move".into());
        };
        let Some((unit, route)) = self.move_parts(loaded, mv) else {
            return ActionOutcome::Unsupported("Load/unit".into());
        };
        let dest = *route.last().unwrap();
        self.check_move_legality(loaded, unit, &route, None, turn_index, day, report);
        report.checks += 1;
        if let Some(transport) = loaded.engine.state.unit_id_at(dest) {
            if !loaded.engine.state.load_into(unit, transport) {
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "load-rejected",
                    detail: format!("unit {unit} could not board {transport} at {dest:?}"),
                });
            } else if let Some(u) = loaded.engine.state.unit_mut(unit) {
                u.moved = true;
            }
        }
        ActionOutcome::Applied
    }

    fn do_unload(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        _turn_index: usize,
        _day: u16,
        report: &mut Report,
    ) -> ActionOutcome {
        let Some(rec) = action.get("unit").and_then(unwrap_vision) else {
            return ActionOutcome::Unsupported("Unload/unit".into());
        };
        let awbw_id = as_num(rec.get("units_id")).unwrap_or(0);
        let Some(cargo) = loaded.live(awbw_id) else {
            return ActionOutcome::Unsupported("Unload/unknown-unit".into());
        };
        let x = as_num(rec.get("units_x")).unwrap_or(0) as u8;
        let y = as_num(rec.get("units_y")).unwrap_or(0) as u8;
        let transport_awbw = as_num(action.get("transportID")).unwrap_or(0);
        report.checks += 1;
        if let Some(transport) = loaded.live(transport_awbw) {
            loaded.engine.state.unload_to(transport, cargo, Pos::new(x, y));
        }
        ActionOutcome::Applied
    }

    /// Finds the acting unit id in payloads that name one directly.
    fn acting_unit(&self, loaded: &Loaded, action: &serde_json::Value) -> Option<UnitId> {
        for key in ["unit", "Unit"] {
            if let Some(rec) = action.get(key).and_then(unwrap_vision) {
                if let Some(awbw_id) = as_num(rec.get("units_id")) {
                    return loaded.live(awbw_id);
                }
            }
        }
        as_num(action.get("unitId").or_else(|| action.get("unitID")))
            .and_then(|id| loaded.live(id))
    }

    /// APC resupply and Black Boat repair: both top up neighbours in place.
    fn do_supply(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) -> ActionOutcome {
        if let Some(mv) = action.get("Move") {
            if let Some((unit, route)) = self.move_parts(loaded, mv) {
                self.check_move_legality(loaded, unit, &route, None, turn_index, day, report);
                force_move(&mut loaded.engine, unit, &route);
            }
        }
        // AWBW records the refreshed units, but the engine only needs to know
        // the supplier acted; the snapshot diff catches any fuel or ammo drift.
        if let Some(unit) = self.acting_unit(loaded, action) {
            if let Some(u) = loaded.engine.state.unit_mut(unit) {
                u.moved = true;
            }
        }
        ActionOutcome::Applied
    }

    fn do_hide(
        &self,
        loaded: &mut Loaded,
        action: &serde_json::Value,
        hidden: bool,
    ) -> ActionOutcome {
        if let Some(mv) = action.get("Move") {
            if let Some((unit, route)) = self.move_parts(loaded, mv) {
                force_move(&mut loaded.engine, unit, &route);
            }
        }
        if let Some(unit) = self.acting_unit(loaded, action) {
            if let Some(u) = loaded.engine.state.unit_mut(unit) {
                u.hidden = hidden;
                u.moved = true;
            }
        }
        ActionOutcome::Applied
    }

    fn do_delete(&self, loaded: &mut Loaded, action: &serde_json::Value) -> ActionOutcome {
        if let Some(unit) = self.acting_unit(loaded, action) {
            loaded.engine.state.destroy(unit);
        }
        ActionOutcome::Applied
    }

    // --- snapshot comparison ----------------------------------------------

    fn diff_states(
        &self,
        loaded: &Loaded,
        next: &Turn,
        turn_index: usize,
        day: u16,
        report: &mut Report,
    ) {
        let state = &loaded.engine.state;

        // Funds, after the turn transition the engine ran on EndTurn.
        for p in &self.replay.players {
            let Some(index) = self.players.index(p.id) else {
                continue;
            };
            let Some(&recorded) = next.funds.get(&p.id.to_string()) else {
                continue;
            };
            report.checks += 1;
            let ours = state.players[index as usize].funds as i64;
            if ours != recorded {
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "funds",
                    detail: format!("player {}: engine {ours}, AWBW {recorded}", p.id),
                });
            }
        }

        // Units: presence, position, and HP.
        let mut recorded_by_id: HashMap<i64, &UnitRec> = HashMap::new();
        for rec in &next.units {
            recorded_by_id.insert(rec.id, rec);
        }

        for (awbw_id, rec) in &recorded_by_id {
            let Some(id) = loaded.live(*awbw_id) else {
                // Units built this turn are registered; anything else is new to
                // us, which is itself worth reporting only if it is not a build.
                continue;
            };
            report.checks += 1;
            let Some(unit) = state.unit(id) else {
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "unit-missing",
                    detail: format!("unit {awbw_id} ({}) survived in AWBW but not here", rec.typ),
                });
                continue;
            };
            if !rec.carried && (unit.pos.x != rec.x || unit.pos.y != rec.y) {
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "unit-position",
                    detail: format!(
                        "unit {awbw_id} ({}): engine {:?}, AWBW ({},{})",
                        rec.typ, unit.pos, rec.x, rec.y
                    ),
                });
            }
            let hp_delta = (unit.hp100 as i32 - rec.hp100).abs();
            if hp_delta > 0 {
                if hp_delta <= 10 {
                    report.luck_slack += 1;
                } else {
                    report.divergences.push(Divergence {
                        turn_index,
                        day,
                        kind: "unit-hp",
                        detail: format!(
                            "unit {awbw_id} ({}): engine {} HP, AWBW {}",
                            rec.typ, unit.hp100, rec.hp100
                        ),
                    });
                }
            }
        }

        // Units we think survived but AWBW says are gone. Driven by the reverse
        // map so a recycled slot is credited to whoever holds it now.
        for (&id, awbw_id) in &loaded.awbw_of {
            if recorded_by_id.contains_key(awbw_id) {
                continue;
            }
            if state.unit(id).is_some() {
                report.checks += 1;
                let unit = state.unit(id).unwrap();
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "unit-extra",
                    detail: format!(
                        "unit {awbw_id} ({} at {:?}, {} HP) survived here but is gone in AWBW",
                        unit.typ.stats().name,
                        unit.pos,
                        unit.hp100
                    ),
                });
            }
        }

        // Property ownership and capture counters.
        for rec in &next.buildings {
            let pos = Pos::new(rec.x, rec.y);
            let expected_owner = data::terrain_by_awbw_id(rec.terrain_id)
                .and_then(|info| info.country)
                .and_then(|c| self.country_owner.get(c).copied());
            let Some(building) = state.building_at(pos) else {
                continue;
            };
            report.checks += 1;
            if building.owner != expected_owner {
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "building-owner",
                    detail: format!(
                        "{pos:?}: engine {:?}, AWBW {:?}",
                        building.owner, expected_owner
                    ),
                });
            }
            if building.capture_remaining != rec.capture {
                report.divergences.push(Divergence {
                    turn_index,
                    day,
                    kind: "building-capture",
                    detail: format!(
                        "{pos:?}: engine {} left, AWBW {}",
                        building.capture_remaining, rec.capture
                    ),
                });
            }
        }
    }
}

enum ActionOutcome {
    Applied,
    Ended,
    Terminal,
    Unsupported(String),
}

/// Moves a unit regardless of legality, so a rejected order does not derail the
/// rest of the turn. The legality check has already been recorded by then.
fn force_move(engine: &mut Engine, unit: UnitId, route: &[Pos]) {
    let Some(&dest) = route.last() else { return };
    let cost = route_cost(&engine.state, unit, route);
    engine.state.relocate(unit, dest);
    if let Some(u) = engine.state.unit_mut(unit) {
        u.fuel = u.fuel.saturating_sub(cost);
        u.moved = true;
    }
}

/// Movement points the recorded route costs this unit.
fn route_cost(state: &GameState, unit: UnitId, route: &[Pos]) -> u8 {
    let Some(actor) = state.unit(unit) else { return 0 };
    let move_type = actor.move_type();
    let mut cost: u32 = 0;
    for pair in route.windows(2) {
        match state.map.terrain_at(pair[1]).move_cost(state.weather, move_type) {
            Some(step) => cost += step as u32,
            None => return cost.min(255) as u8,
        }
    }
    cost.min(255) as u8
}

fn apply_unit_record(state: &mut GameState, id: UnitId, rec: &UnitRec) {
    if let Some(unit) = state.unit_mut(id) {
        unit.hp100 = rec.hp100.clamp(0, 100) as u8;
        unit.fuel = rec.fuel.clamp(0, 255) as u8;
        unit.ammo = rec.ammo.clamp(0, 255) as u8;
        unit.moved = rec.moved;
        unit.hidden = rec.sub_dive;
    }
}
