//! JakeMan, ported from DefendPeace.
//!
//! Picked by playing DefendPeace's own AIs against each other on this map
//! rather than by reading them — the largest and most elaborate of the five
//! came fourth. See `docs/log/2026-08-25-defendpeace-ai-ranking.md`.
//!
//! What it adds over [`crate::greedy`] is three things, and they are the reason
//! it wins:
//!
//! *A threat map.* Every tile carries, per enemy unit type, the weight of the
//! units that could strike it. A move onto a tile nothing can punish is worth
//! more than the same move onto one three tanks cover, and `greedy` cannot see
//! the difference because it only ever looks one exchange deep.
//!
//! *Counter-building.* Production is scored against what the enemy actually
//! fields, not against a fixed preference order. `greedy` builds the same army
//! whatever it is facing, which is exactly the uniformity a learned policy
//! discovers and exploits.
//!
//! *A safety test before committing.* A unit only takes a property or walks
//! into range if the friendly units nearby can answer whatever threatens the
//! tile — the counter-power accounting in [`Ai::dude_free`].
//!
//! The control flow is not ported. DefendPeace runs a queue of modules that
//! each claim units; this scores every legal order the way those modules would
//! and takes the best, which is the shape the engine here already offers and
//! reaches the same decisions.

use std::collections::HashMap;

use awbw_engine::actions::{Action, Engine};
use awbw_engine::combat;
use awbw_engine::map::Pos;
use awbw_engine::movement::Reach;
use awbw_engine::rng::Rng;
use awbw_engine::state::{ActivePower, GameState, PlayerId, Unit, UnitId};
use awbw_engine::types::{TerrainKind, UnitType};

use crate::Bot;

/// Damage a weapon must do before it counts as threatening. A unit that fires
/// back can shrug off a light hit; one that cannot is in trouble from anything.
const DIRECT_THREAT: i32 = 30;
const INDIRECT_THREAT: i32 = 7;

/// Hitting something that threatens you first is worth double: the exchange you
/// avoid is as valuable as the damage you deal.
const FIRST_STRIKE: f32 = 2.0;
/// Charged against an attack that kills the attacker. Trading a unit away has
/// to clear a real bar, not merely come out even on funds.
const STAY_ALIVE_BIAS: f32 = 2_000.0;
/// Below this health a unit is nearly spent, so finishing it is worth less than
/// opening up something fresh.
const BIG_THREAT_HP: u8 = 80;
/// A unit standing still counts less as its own defender than one that is
/// actively covering the tile.
const PEACEFUL_SELF_RATIO: f32 = 1.0;

// Scores share the bands `greedy` uses so the two remain comparable.
const IDLE: f32 = -1.0;
const CAPTURE_COMPLETE: f32 = 100_000.0;
const CAPTURE_STEP: f32 = 40_000.0;
const HQ_BONUS: f32 = 500_000.0;
const BUILD_FLOOR: f32 = 20_000.0;
/// Lifts a worthwhile attack clear of every walk.
///
/// DefendPeace does not weigh the two against each other: `GetFreeDudes`
/// returns `findBestAttack`'s pick outright, and `Travel` is the module that
/// runs afterwards for whatever did not act — "if no attack/capture actions are
/// available now, just move around". Flattening that into one number put an
/// attack worth a few hundred against a walk worth `APPROACH` a tile, and the
/// walk won 24 times in 412 on the training map. A floor restores the
/// precedence while leaving attacks ranked among themselves by value. Below
/// `BUILD_FLOOR` deliberately: attacks spend no money and builds consume no
/// unit, so within a turn both happen either way and the order does not matter.
const ATTACK_FLOOR: f32 = 10_000.0;
const APPROACH: f32 = 100.0;
/// Charged against ending a move somewhere the threat accounting says is lost.
const UNSAFE: f32 = 6_000.0;

/// Per unit type, the threat resting on each tile, indexed by tile.
type ThreatMap = HashMap<UnitType, Vec<f32>>;

/// Whether `threat` hits `target` hard enough to matter.
///
/// The threshold depends on the *target*: something that shoots back only
/// counts a real blow as a threat, where an indirect unit — which never
/// counterattacks — is threatened by far less.
fn threatened_by(target: UnitType, threat: UnitType) -> bool {
    let bar = if target.is_indirect() {
        INDIRECT_THREAT
    } else {
        DIRECT_THREAT
    };
    combat::base_percentage(threat, target, threat.stats().max_ammo)
        .is_some_and(|(pct, _)| pct >= bar)
}

/// Threatened by it, and unable to threaten it back. The matchups worth
/// building for.
fn weak_to(target: UnitType, threat: UnitType) -> bool {
    threatened_by(target, threat) && !threatened_by(threat, target)
}

fn is_capturer(typ: UnitType) -> bool {
    matches!(typ, UnitType::Infantry | UnitType::Mech)
}

fn unit_value(unit: &Unit) -> f32 {
    unit.typ.stats().cost as f32 * unit.hp100 as f32 / 100.0
}

pub struct JakeManBot {
    name: String,
    rng: Rng,
    reach: Reach,
    buffer: Vec<Action>,
    /// Rebuilt when the turn changes; every score in a turn reads the same map.
    enemy: ThreatMap,
    friendly: ThreatMap,
    built_for: Option<(u16, PlayerId)>,
    /// Enemy army composition, as funds on the board per unit type.
    composition: HashMap<UnitType, f32>,
}

impl Default for JakeManBot {
    fn default() -> Self {
        JakeManBot::new(0)
    }
}

impl JakeManBot {
    pub fn new(seed: u64) -> Self {
        JakeManBot {
            name: "jakeman".to_string(),
            rng: Rng::new(seed),
            reach: Reach::new(),
            buffer: Vec::new(),
            enemy: HashMap::new(),
            friendly: HashMap::new(),
            built_for: None,
            composition: HashMap::new(),
        }
    }

    /// Tiles this unit could put a shot on this turn.
    ///
    /// An indirect unit fires from where it stands and may not move first, so
    /// its reach is a ring around its own tile; everything else threatens the
    /// neighbours of every tile it can walk to.
    fn threat_tiles(&mut self, state: &GameState, unit: &Unit, out: &mut Vec<usize>) {
        out.clear();
        let map = &state.map;
        if unit.typ.is_indirect() {
            let stats = unit.typ.stats();
            let (lo, hi) = (stats.range_min as i32, stats.range_max as i32);
            for index in 0..map.tile_count() {
                let pos = map.pos_of(index);
                let d = pos.distance(unit.pos) as i32;
                if d >= lo && d <= hi {
                    out.push(index);
                }
            }
            return;
        }
        self.reach.compute(state, unit.id);
        for from in self.reach.reachable(state) {
            for step in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (x, y) = (from.x as i32 + step.0, from.y as i32 + step.1);
                if x < 0 || y < 0 || x >= map.width as i32 || y >= map.height as i32 {
                    continue;
                }
                out.push(map.index(Pos::new(x as u8, y as u8)));
            }
        }
        out.sort_unstable();
        out.dedup();
    }

    /// Rebuilds the threat maps and the enemy's composition for a new turn.
    fn survey(&mut self, engine: &Engine) {
        let state = &engine.state;
        let key = (state.day, state.current);
        if self.built_for == Some(key) {
            return;
        }
        self.built_for = Some(key);
        self.enemy.clear();
        self.friendly.clear();
        self.composition.clear();

        let tiles = state.map.tile_count();
        let me = state.current;
        let units: Vec<(UnitType, bool, f32, UnitId)> = state
            .units()
            .filter(|u| u.carried_by.is_none())
            .map(|u| {
                (
                    u.typ,
                    state.are_enemies(me, u.owner),
                    // Squared, so a nearly-dead unit stops counting as a whole
                    // one — it can still shoot, but not survive the answer.
                    {
                        let h = u.hp100 as f32 / 100.0;
                        h * h
                    },
                    u.id,
                )
            })
            .collect();

        let mut scratch = Vec::new();
        for (typ, hostile, weight, id) in units {
            let Some(unit) = state.unit(id) else { continue };
            let unit = unit.clone();
            self.threat_tiles(state, &unit, &mut scratch);
            let map = if hostile {
                &mut self.enemy
            } else {
                &mut self.friendly
            };
            let entry = map.entry(typ).or_insert_with(|| vec![0.0; tiles]);
            for &t in &scratch {
                entry[t] += weight;
            }
            if hostile {
                *self.composition.entry(typ).or_insert(0.0) += unit_value(&unit);
            }
        }
    }

    /// Whether a unit can stand on `pos` without being picked off.
    ///
    /// Ported from `isDudeFree`. Each enemy type that threatens the tile is set
    /// against the friendly power covering the tiles next to it: a threat that
    /// our neighbours can answer is written off, and only a surplus counts. A
    /// unit that has not moved counts less as its own cover, since sitting
    /// still is not the same as guarding a square.
    fn dude_free(&self, state: &GameState, unit: &Unit, pos: Pos, attacking: bool) -> bool {
        let index = state.map.index(pos);
        let mut threats: Vec<(UnitType, f32)> = self
            .enemy
            .iter()
            .filter(|(&typ, _)| threatened_by(unit.typ, typ))
            .filter_map(|(&typ, tiles)| {
                let v = tiles[index];
                (v > 0.0).then_some((typ, v))
            })
            .collect();
        if threats.is_empty() {
            return true;
        }

        let neighbours: Vec<usize> = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .filter_map(|(dx, dy)| {
                let (x, y) = (pos.x as i32 + dx, pos.y as i32 + dy);
                (x >= 0 && y >= 0 && x < state.map.width as i32 && y < state.map.height as i32)
                    .then(|| state.map.index(Pos::new(x as u8, y as u8)))
            })
            .collect();

        for (threat, surplus) in threats.iter_mut() {
            for (&counter, tiles) in &self.friendly {
                if !threatened_by(*threat, counter) {
                    continue;
                }
                let mine_and_idle = !attacking && counter == unit.typ;
                let mut total = 0.0;
                for &n in &neighbours {
                    let mut power = tiles[n];
                    if mine_and_idle {
                        power -= PEACEFUL_SELF_RATIO * unit.display_hp() as f32 / 10.0;
                    }
                    total += power.max(0.0);
                }
                let average = total / neighbours.len().max(1) as f32;
                if average >= *surplus {
                    *surplus = 0.0;
                    break;
                }
                *surplus -= average;
            }
        }
        threats.retain(|(_, surplus)| *surplus > 0.0);
        if threats.is_empty() {
            return true;
        }

        // Cover can still make it worth standing: three defence stars, one
        // leftover threat, and that threat the same type as the unit itself.
        let defense = state.map.terrain_at(pos).defense();
        if defense < 3 || unit.typ.stats().move_type == awbw_engine::types::MoveType::Air {
            return false;
        }
        threats.len() == 1 && threats[0].0 == unit.typ && threats[0].1 < 1.3
    }

    /// `findBestAttack`'s scoring: what the exchange is worth in funds, with a
    /// thumb on the scale for hitting what would otherwise hit you.
    fn attack_score(&self, engine: &Engine, unit_id: UnitId, dest: Pos, target: Pos) -> f32 {
        let state = &engine.state;
        let (Some(attacker), Some(defender)) = (state.unit(unit_id), state.unit_at(target)) else {
            return f32::MIN;
        };
        let Some(spread) = engine.preview_damage(unit_id, target) else {
            return f32::MIN;
        };
        let dealt = (spread.expected as f32).min(defender.hp100 as f32);
        let mut damage = dealt / 100.0 * defender.typ.stats().cost as f32;
        if threatened_by(attacker.typ, defender.typ) {
            damage *= FIRST_STRIKE;
        }
        if defender.hp100 < BIG_THREAT_HP {
            damage /= 1.5;
        }

        let mut loss = 0.0;
        let surviving = (defender.hp100 as f32 - dealt).max(0.0) as i32;
        if let Some(defender_id) = state.unit_id_at(target) {
            if let Some(counter) = engine.preview_counter(defender_id, surviving, unit_id, dest) {
                let taken = (counter.expected as f32).min(attacker.hp100 as f32);
                if taken >= attacker.hp100 as f32 {
                    loss += STAY_ALIVE_BIAS;
                }
                loss += taken / 100.0 * attacker.typ.stats().cost as f32;
            }
        }
        damage - loss
    }

    /// What to build, weighed against what the enemy actually has.
    fn build_score(&self, engine: &Engine, typ: UnitType, player: PlayerId) -> f32 {
        let state = &engine.state;
        // No transport plan, so no transports.
        if matches!(
            typ,
            UnitType::Apc | UnitType::TCopter | UnitType::Lander | UnitType::BlackBoat
        ) {
            return f32::MIN;
        }
        let cost = engine.unit_cost(player, typ) as f32;
        let mut score = BUILD_FLOOR;

        // Counter-building: how much of what they field does this beat, and how
        // much of it beats this back. Normalised by the enemy's total value, so
        // the shape of their army decides rather than its size.
        let total: f32 = self.composition.values().sum::<f32>().max(1.0);
        let mut counter = 0.0;
        for (&theirs, &value) in &self.composition {
            let share = value / total;
            if weak_to(theirs, typ) {
                counter += share;
            }
            if weak_to(typ, theirs) {
                counter -= share;
            }
        }
        score += counter * 9_000.0;

        // Properties still win the game, so the capture engine keeps running.
        let cappers = state.units_of(player).filter(|u| is_capturer(u.typ)).count();
        let takeable = state
            .buildings()
            .iter()
            .filter(|b| b.owner != Some(player))
            .count();
        let wanted = takeable.min(state.property_count(player) as usize + 4);
        let funds = state.player(player).funds as f32;
        let poor = (1.0 - funds / 20_000.0).clamp(0.0, 1.0);
        if cappers < wanted && typ == UnitType::Infantry {
            score += poor * 4_000.0;
        }
        // With money to spare, a base-turn is worth more than the money.
        score += (1.0 - poor) * 2_000.0 * (cost / 28_000.0).min(1.0);
        score
    }

    /// The nearest thing worth walking toward.
    fn objective(&self, state: &GameState, unit: &Unit) -> Option<Pos> {
        let me = unit.owner;
        if is_capturer(unit.typ) {
            return state
                .buildings()
                .iter()
                .filter(|b| b.owner != Some(me))
                .map(|b| b.pos)
                .min_by_key(|p| p.distance(unit.pos));
        }
        state
            .units()
            .filter(|u| state.are_enemies(me, u.owner) && u.carried_by.is_none())
            // Chase what this unit actually beats.
            .min_by_key(|u| {
                let bonus = if weak_to(u.typ, unit.typ) { 0 } else { 4 };
                u.pos.distance(unit.pos) as u32 + bonus
            })
            .map(|u| u.pos)
    }

    fn score(&self, engine: &Engine, action: Action) -> f32 {
        let state = &engine.state;
        let me = state.current;
        match action {
            Action::EndTurn => IDLE,
            // Fire the biggest charged power immediately — RizeBot's
            // milestone-1 policy. Only ever offered when legal.
            Action::Activate { power } => match power {
                ActivePower::Scop => f32::MAX,
                _ => f32::MAX / 2.0,
            },
            Action::Build { typ, .. } => self.build_score(engine, typ, me),

            Action::Attack { unit, dest, target } => {
                // A losing exchange keeps its own negative score, so it still
                // ranks below walking away, exactly as a null from
                // `findBestAttack` leaves the unit to `Travel`.
                let value = self.attack_score(engine, unit, dest, target);
                if value > 0.0 {
                    ATTACK_FLOOR + value
                } else {
                    value
                }
            }

            Action::Capture { unit, dest } => {
                let (Some(actor), Some(building)) = (state.unit(unit), state.building_at(dest))
                else {
                    return f32::MIN;
                };
                let rate = state.co_of(me).capture_multiplier_pct;
                let progress = actor.display_hp() as u32 * rate / 100;
                let mut score = if progress >= building.capture_remaining as u32 {
                    CAPTURE_COMPLETE
                        + if building.kind == TerrainKind::Hq {
                            HQ_BONUS
                        } else {
                            0.0
                        }
                } else {
                    CAPTURE_STEP
                };
                // A capture that gets the capturer killed hands the property
                // straight back, so it is worth less than it looks.
                if !self.dude_free(state, actor, dest, false) {
                    score -= UNSAFE;
                }
                score
            }

            Action::Move { unit, dest } => {
                let Some(actor) = state.unit(unit) else {
                    return f32::MIN;
                };
                let mut score = match self.objective(state, actor) {
                    Some(goal) => {
                        let before = actor.pos.distance(goal) as f32;
                        let after = dest.distance(goal) as f32;
                        (before - after) * APPROACH
                    }
                    None => state.map.terrain_at(dest).defense() as f32,
                };
                if !self.dude_free(state, actor, dest, false) {
                    score -= UNSAFE;
                }
                score
            }

            Action::Join { unit, dest } => {
                let (Some(a), Some(b)) = (state.unit(unit), state.unit_at(dest)) else {
                    return f32::MIN;
                };
                if a.hp100 <= 30 && b.hp100 <= 70 {
                    2_000.0
                } else {
                    f32::MIN
                }
            }

            Action::Load { .. } | Action::Unload { .. } => f32::MIN,
            Action::Supply { .. } => 200.0,
        }
    }
}

impl Bot for JakeManBot {
    fn name(&self) -> &str {
        &self.name
    }

    fn reset(&mut self, seed: u64) {
        self.rng = Rng::new(seed);
        self.built_for = None;
    }

    fn choose(&mut self, engine: &mut Engine) -> Action {
        self.survey(engine);
        engine.legal_actions_into(&mut self.buffer);
        if self.buffer.is_empty() {
            return Action::EndTurn;
        }
        // Ties broken at random: the action list is built row-major, so
        // resolving them by enumeration order is a geographic bias in disguise.
        let mut best = Action::EndTurn;
        let mut best_score = f32::MIN;
        let mut seen = 0u32;
        let actions = std::mem::take(&mut self.buffer);
        for &action in &actions {
            let score = self.score(engine, action);
            if score > best_score {
                best_score = score;
                best = action;
                seen = 1;
            } else if score == best_score {
                seen += 1;
                if self.rng.roll_inclusive(seen - 1) == 0 {
                    best = action;
                }
            }
        }
        self.buffer = actions;
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::symmetric_map;
    use awbw_engine::state::Outcome;

    fn engine() -> Engine {
        let (map, owners) = symmetric_map(13, 13);
        let players = vec![
            awbw_engine::state::Player::new(10_000, 1),
            awbw_engine::state::Player::new(10_000, 2),
        ];
        let state = awbw_engine::state::GameState::new(
            map,
            awbw_engine::state::GameSettings::default(),
            players,
            &owners,
        );
        Engine::new(state, 1)
    }

    #[test]
    fn every_chosen_order_is_legal() {
        let mut engine = engine();
        let mut bot = JakeManBot::new(1);
        for _ in 0..2_000 {
            if engine.state.outcome() != Outcome::InProgress {
                break;
            }
            let action = bot.choose(&mut engine);
            assert!(engine.check(action).is_ok(), "illegal order {action:?}");
            engine.apply(action).expect("apply");
        }
    }

    #[test]
    fn recon_beside_an_infantry_shoots_it() {
        // DefendPeace takes `findBestAttack`'s answer outright; here an attack
        // competes on score with everything else the unit could do, so the
        // bands have to leave a plain attack on top of a plain walk.
        let mut engine = engine();
        let recon = engine.state.spawn(UnitType::Recon, 0, Pos::new(6, 6));
        engine.state.spawn(UnitType::Infantry, 1, Pos::new(7, 6));
        engine.state.current = 0;

        let mut bot = JakeManBot::new(1);
        bot.survey(&engine);

        let mut actions = Vec::new();
        engine.legal_actions_into(&mut actions);
        let mut best = None;
        let mut best_score = f32::MIN;
        for &a in &actions {
            let owner = match a {
                Action::Attack { unit, .. }
                | Action::Move { unit, .. }
                | Action::Capture { unit, .. }
                | Action::Join { unit, .. } => Some(unit),
                _ => None,
            };
            if owner != Some(recon) {
                continue;
            }
            let s = bot.score(&engine, a);
            if matches!(a, Action::Attack { .. } | Action::Move { .. }) {
                println!("{s:>8.1}  {a:?}");
            }
            if s > best_score {
                best_score = s;
                best = Some(a);
            }
        }
        println!("recon's own best: {best:?} at {best_score}");
        assert!(
            matches!(best, Some(Action::Attack { .. })),
            "recon beside an infantry chose {best:?} at {best_score}"
        );
    }

    #[test]
    #[ignore = "measurement, not a check; cargo test -- --ignored --nocapture"]
    fn probe_declined_attacks() {
        // Over a whole game, how often does a unit with a legal attack in front
        // of it get ordered to walk instead? The bot picks one action at a time,
        // so a build outscoring an attack is fine -- the attack comes later. A
        // *move* by the same unit is not: moving ends that unit's turn, and the
        // attack is gone.
        let mut engine = engine();
        let mut bot = JakeManBot::new(1);
        let mut declined = 0;
        let mut walked = 0;
        let mut worst: f32 = 0.0;
        let mut actions = Vec::new();
        for _ in 0..4_000 {
            if engine.state.outcome() != Outcome::InProgress {
                break;
            }
            let chosen = bot.choose(&mut engine);
            if let Action::Move { unit, .. } = chosen {
                walked += 1;
                engine.legal_actions_into(&mut actions);
                let best_attack = actions
                    .iter()
                    .filter(|a| matches!(a, Action::Attack { unit: u, .. } if *u == unit))
                    .map(|a| bot.score(&engine, *a))
                    .fold(f32::MIN, f32::max);
                let move_score = bot.score(&engine, chosen);
                if best_attack > f32::MIN && best_attack > 0.0 {
                    declined += 1;
                    worst = worst.max(best_attack - move_score);
                }
            }
            engine.apply(chosen).expect("apply");
        }
        println!(
            "moves {walked}, of which {declined} had a positive attack available; \
             largest attack-minus-move gap {worst:.1}"
        );
    }

    /// The board and opening the policy actually trains against, so a probe
    /// here sees what a recorded game shows rather than a 13x13 with a bank.
    fn river_supreme() -> Option<Engine> {
        let map = crate::awbw_map::AwbwMap::load(
            std::path::Path::new("../../").join(crate::awbw_map::RIVER_SUPREME),
        )
        .ok()?;
        let state = map.new_game(awbw_engine::state::GameSettings::default(), 0);
        Some(Engine::new(state, 1))
    }

    #[test]
    #[ignore = "measurement, not a check; cargo test -- --ignored --nocapture"]
    fn probe_build_mix_river() {
        let Some(mut engine) = river_supreme() else {
            println!("map not cached; skipped");
            return;
        };
        let mut bots: Vec<Box<dyn Bot>> =
            vec![Box::new(JakeManBot::new(1)), Box::new(JakeManBot::new(2))];
        let mut builds: [HashMap<UnitType, u32>; 2] = Default::default();
        let mut early: [HashMap<UnitType, u32>; 2] = Default::default();
        for _ in 0..20_000 {
            if engine.state.outcome() != Outcome::InProgress || engine.state.day > 30 {
                break;
            }
            let seat = engine.state.current as usize;
            let chosen = bots[seat].choose(&mut engine);
            if let Action::Build { typ, .. } = chosen {
                *builds[seat].entry(typ).or_default() += 1;
                if engine.state.day <= 5 {
                    *early[seat].entry(typ).or_default() += 1;
                }
            }
            engine.apply(chosen).expect("apply");
        }
        for seat in 0..2 {
            let mut all: Vec<_> = builds[seat].iter().collect();
            all.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
            let mut e: Vec<_> = early[seat].iter().collect();
            e.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
            println!("P{} all {:?}", seat + 1, all);
            println!("P{} d1-5 {:?}", seat + 1, e);
        }
        println!(
            "day {} props P1 {} P2 {}",
            engine.state.day,
            engine.state.property_count(0),
            engine.state.property_count(1)
        );
    }

    #[test]
    #[ignore = "measurement, not a check; cargo test -- --ignored --nocapture"]
    fn probe_declined_attacks_river() {
        // DefendPeace takes `findBestAttack`'s answer outright and only travels
        // with units that did not act; here the two compete on score, and a
        // move worth APPROACH-per-tile can outbid an attack worth a few
        // hundred. This counts how often that actually happens on the board the
        // policy trains on.
        let Some(mut engine) = river_supreme() else {
            println!("map not cached; skipped");
            return;
        };
        // Concrete instances, not `dyn Bot`: the scores have to come from the
        // same object that made the decision, or they are read off a threat map
        // built at a different point in the turn -- the map is cached per turn,
        // so a second instance surveying mid-turn sees a different board.
        let mut bots = [JakeManBot::new(1), JakeManBot::new(2)];
        let (mut walked, mut declined) = (0u32, 0u32);
        let mut worst: f32 = 0.0;
        let mut actions = Vec::new();
        for _ in 0..40_000 {
            if engine.state.outcome() != Outcome::InProgress || engine.state.day > 40 {
                break;
            }
            let seat = engine.state.current as usize;
            let chosen = bots[seat].choose(&mut engine);
            if let Action::Move { unit, .. } = chosen {
                walked += 1;
                engine.legal_actions_into(&mut actions);
                let best_attack = actions
                    .iter()
                    .filter(|a| matches!(a, Action::Attack { unit: u, .. } if *u == unit))
                    .map(|a| bots[seat].score(&engine, *a))
                    .fold(f32::MIN, f32::max);
                if best_attack > 0.0 {
                    declined += 1;
                    let m = bots[seat].score(&engine, chosen);
                    worst = worst.max(best_attack - m);
                }
            }
            engine.apply(chosen).expect("apply");
        }
        println!(
            "river: {walked} moves, {declined} with a positive attack going \
             unused, largest attack-minus-move {worst:.1}"
        );
    }

    #[test]
    #[ignore = "measurement, not a check; cargo test -- --ignored --nocapture"]
    fn probe_seat_balance() {
        // A mirror: the same bot on both seats, differing only in seed. Whatever
        // it scores is the board's, not the player's.
        let mut wins = [0u32; 2];
        let mut draws = 0;
        let games = 40;
        for game in 0..games {
            let Some(mut engine) = river_supreme() else {
                println!("map not cached; skipped");
                return;
            };
            let mut bots: Vec<Box<dyn Bot>> = vec![
                Box::new(JakeManBot::new(game * 2 + 1)),
                Box::new(JakeManBot::new(game * 2 + 2)),
            ];
            for _ in 0..40_000 {
                if engine.state.outcome() != Outcome::InProgress || engine.state.day > 60 {
                    break;
                }
                let seat = engine.state.current as usize;
                let chosen = bots[seat].choose(&mut engine);
                engine.apply(chosen).expect("apply");
            }
            match engine.state.outcome() {
                Outcome::Winner(p) => wins[p as usize] += 1,
                _ => draws += 1,
            }
        }
        println!(
            "jakeman mirror over {games}: P1 {}, P2 {}, draws {draws}",
            wins[0], wins[1]
        );
    }

    #[test]
    #[ignore = "measurement, not a check; cargo test -- --ignored --nocapture"]
    fn probe_build_mix() {
        // What it actually spends money on, and when. Counter-building is worth
        // up to 9,000 and the capture-economy bonus at most 4,000, so a unit
        // that beats what the enemy fields outbids the infantry that takes the
        // properties -- which is the opening the economy is decided in.
        let mut engine = engine();
        let mut bot = JakeManBot::new(1);
        let mut builds: HashMap<UnitType, u32> = HashMap::new();
        let mut early: HashMap<UnitType, u32> = HashMap::new();
        for _ in 0..4_000 {
            if engine.state.outcome() != Outcome::InProgress {
                break;
            }
            let chosen = bot.choose(&mut engine);
            if let Action::Build { typ, .. } = chosen {
                *builds.entry(typ).or_default() += 1;
                if engine.state.day <= 5 {
                    *early.entry(typ).or_default() += 1;
                }
            }
            engine.apply(chosen).expect("apply");
        }
        let mut all: Vec<_> = builds.iter().collect();
        all.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        println!("whole game: {all:?}");
        let mut e: Vec<_> = early.iter().collect();
        e.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
        println!("days 1-5:   {e:?}");
    }

    #[test]
    fn a_threatened_tile_reads_as_threatened() {
        // Infantry is threatened by a tank; a tank is not much threatened back.
        assert!(threatened_by(UnitType::Infantry, UnitType::Tank));
        assert!(weak_to(UnitType::Infantry, UnitType::Tank));
        assert!(!weak_to(UnitType::Tank, UnitType::Infantry));
        // Artillery never counterattacks, so far less counts as a threat to it.
        assert!(threatened_by(UnitType::Artillery, UnitType::Infantry));
    }
}
