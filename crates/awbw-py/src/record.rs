//! Recording a played game in a shape `tools/write_replay.py` can turn into a
//! real AWBW replay file.
//!
//! This is the replay pipeline run backwards, and it keeps the same split of
//! labour: everything AWBW-specific and messy -- zip, gzip, PHP serialization,
//! the enormous per-viewer action payloads -- stays in Python, and Rust emits a
//! flat log of what happened. The engine is the only thing that knows the path
//! a unit actually walked or the HP left after a fight, so those are recorded
//! here rather than guessed at afterwards.
//!
//! A snapshot is taken at the start of each turn, matching AWBW, whose replay
//! files pair one game state with one turn's worth of orders.

use awbw_engine::actions::{Action, Engine};
use awbw_engine::movement::Reach;
use awbw_engine::state::{GameState, Outcome, PlayerId, UnitId};
use awbw_engine::map::Pos;

use std::collections::HashMap;

use serde_json::{json, Value};

/// One game's log, built as it is played.
#[derive(Default)]
pub struct Recorder {
    turns: Vec<Value>,
    /// Orders played in the turn currently open.
    orders: Vec<Value>,
    /// The `(day, player)` the open snapshot was taken for.
    open: Option<(u16, PlayerId)>,
    snapshot: Option<Value>,
    /// Set once the game ends, and taken by `finished`.
    done: Option<Value>,
    /// Engine unit slot -> an id unique to the unit that currently holds it.
    ///
    /// `UnitId` is a *slot* and the engine reuses it once its occupant dies, so
    /// a unit built later can inherit a dead one's id. AWBW ids are unique for
    /// the life of a game, and a reader handed the same id twice sees one unit
    /// jump across the map -- which is what a replay of a long game looked
    /// like, worsening as the losses piled up.
    live: HashMap<UnitId, u64>,
    next_id: u64,
}

impl Recorder {
    /// Reconciles slots with the board: retires the dead, numbers the new.
    fn sync(&mut self, state: &GameState) {
        self.live.retain(|&slot, _| state.unit(slot).is_some());
        for unit in state.units() {
            if !self.live.contains_key(&unit.id) {
                self.next_id += 1;
                self.live.insert(unit.id, self.next_id);
            }
        }
    }

    fn unit_json(&self, state: &GameState, id: UnitId) -> Value {
        unit_json(&self.live, state, id)
    }

    /// A slot's stable replay id — the number every *other* record calls it by.
    ///
    /// `UnitId` is a slot and the engine reuses it; `live` is what keeps one
    /// number meaning one unit for a whole game. Anything written into a
    /// payload has to go through here, or it names whichever unit later
    /// inherited the slot. A transport id written raw pointed at the wrong
    /// unit, and a reader that cannot resolve it drops the unload entirely,
    /// leaving the passenger standing on the boat.
    fn stable(&self, id: UnitId) -> u64 {
        self.live.get(&id).copied().unwrap_or(0)
    }
}

fn unit_json(live: &HashMap<UnitId, u64>, state: &GameState, id: UnitId) -> Value {
    let Some(unit) = state.unit(id) else {
        return Value::Null;
    };
    let cargo: Vec<u64> = unit
        .cargo
        .iter()
        .filter(|&&c| state.unit(c).is_some())
        .filter_map(|c| live.get(c).copied())
        .collect();
    json!({
        "id": live.get(&id).copied().unwrap_or(0),
        "type": format!("{:?}", unit.typ),
        "player": unit.owner,
        "x": unit.pos.x,
        "y": unit.pos.y,
        "hp100": unit.hp100,
        "fuel": unit.fuel,
        "ammo": unit.ammo,
        "moved": unit.moved,
        "carried": unit.carried_by.is_some(),
        "cargo": cargo,
    })
}

impl Recorder {
    /// The turn-start snapshot, if the turn has moved on since the last one.
    fn open_turn(&mut self, state: &GameState) {
        let key = (state.day, state.current);
        if self.open == Some(key) {
            return;
        }
        self.close_turn();
        self.open = Some(key);

        let units: Vec<Value> = state.units().map(|u| self.unit_json(state, u.id)).collect();
        let buildings: Vec<Value> = state
            .buildings()
            .iter()
            .map(|b| {
                json!({
                    "x": b.pos.x,
                    "y": b.pos.y,
                    "kind": format!("{:?}", b.kind),
                    "owner": b.owner,
                    "capture": b.capture_remaining,
                })
            })
            .collect();
        let funds: Vec<u32> = state.players.iter().map(|p| p.funds).collect();
        // The power meter, per player, as AWBW's own player row carries it: the
        // raw charge and the flag for a power running right now. Without these
        // a written replay has to say the bar is empty on every turn, which is
        // a disagreement on every turn for anything that re-reads it.
        let charge: Vec<u32> = state.players.iter().map(|p| p.charge).collect();
        // The thresholds as they stand *now*: each activation raises a star's
        // price, so a fixed pair taken from the CO's star counts is right only
        // until the first power is fired, and anything reading the bar back
        // reconstructs the wrong number of uses from it afterwards.
        let cop_cost: Vec<u32> = state
            .players
            .iter()
            .map(|p| p.power_cost(awbw_engine::state::ActivePower::Cop).unwrap_or(0))
            .collect();
        let scop_cost: Vec<u32> = state
            .players
            .iter()
            .map(|p| p.power_cost(awbw_engine::state::ActivePower::Scop).unwrap_or(0))
            .collect();
        let power: Vec<&str> = state
            .players
            .iter()
            .map(|p| match p.active_power {
                awbw_engine::state::ActivePower::None => "N",
                awbw_engine::state::ActivePower::Cop => "Y",
                awbw_engine::state::ActivePower::Scop => "S",
            })
            .collect();
        self.snapshot = Some(json!({
            "day": state.day,
            "active": state.current,
            "funds": funds,
            "charge": charge,
            "power": power,
            "cop_cost": cop_cost,
            "scop_cost": scop_cost,
            "units": units,
            "buildings": buildings,
        }));
    }

    fn close_turn(&mut self) {
        if let Some(mut snapshot) = self.snapshot.take() {
            snapshot["orders"] = Value::Array(std::mem::take(&mut self.orders));
            self.turns.push(snapshot);
        }
        self.orders.clear();
    }

    /// Reads what stops existing once the action lands, and opens the turn.
    ///
    /// Recording is split around `apply` because neither half is available on
    /// its own: the path a unit walks and the tile it starts from are gone
    /// afterwards, while the HP left after a fight and the id of a unit just
    /// built do not exist until then.
    pub fn begin(&mut self, state: &GameState, action: Action) -> Value {
        self.sync(state);
        self.open_turn(state);
        self.before(state, action)
    }

    /// Completes the order `begin` opened, now that it has been applied.
    pub fn end(&mut self, engine: &Engine, action: Action, before: Value) {
        // Before reading the result: a unit built by this order needs an id,
        // and one destroyed by it has already been replaced in `before`.
        self.sync(&engine.state);
        if let Some(order) = self.after(&engine.state, action, before) {
            self.orders.push(order);
        }
    }

    /// A combatant's record after the fight, or the one from before it at zero
    /// HP if it did not survive.
    ///
    /// AWBW reports a destroyed unit rather than omitting it, and the defender's
    /// record is the only place its tile appears -- so a null there loses what
    /// was attacked, and a reader silently drops the whole order.
    fn survivor(&self, state: &GameState, id: Option<UnitId>, before: Option<&Value>) -> Value {
        if let Some(id) = id {
            if state.unit(id).is_some() {
                return self.unit_json(state, id);
            }
        }
        let mut dead = before.cloned().unwrap_or(Value::Null);
        if let Some(fields) = dead.as_object_mut() {
            fields.insert("hp100".into(), json!(0));
        }
        dead
    }

    /// A mover's record when it did not survive the move it made.
    ///
    /// `before` was read before the unit set off, so it holds the tile it left
    /// and the fuel it left with — and the unit really did arrive, pay for the
    /// trip, and die there. Reading that back afterwards is impossible, the
    /// unit is gone, so the arrival is written on by hand. Left uncorrected an
    /// attacker killed by the counterattack is recorded as never having moved,
    /// which is a whole move's fuel adrift and the wrong tile besides.
    fn dead_mover(before: Option<&Value>, dest: Pos, spent: u8) -> Value {
        let mut dead = before.cloned().unwrap_or(Value::Null);
        if let Some(fields) = dead.as_object_mut() {
            let fuel = fields
                .get("fuel")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_sub(spent as u64);
            fields.insert("hp100".into(), json!(0));
            fields.insert("x".into(), json!(dest.x));
            fields.insert("y".into(), json!(dest.y));
            fields.insert("fuel".into(), json!(fuel));
            fields.insert("moved".into(), json!(true));
        }
        dead
    }

    /// The path a unit walks to `dest`, origin first, as AWBW records it, and
    /// what the walk costs it — the two come from one reachability search.
    fn path_and_cost(state: &GameState, unit: UnitId, dest: Pos) -> (Value, u8) {
        let mut reach = Reach::new();
        reach.compute(state, unit);
        let steps: Vec<Value> = reach
            .path_to(state, dest)
            .into_iter()
            .map(|p| json!({"x": p.x, "y": p.y}))
            .collect();
        (Value::Array(steps), reach.cost_to(state, dest).unwrap_or(0))
    }

    fn path(state: &GameState, unit: UnitId, dest: Pos) -> Value {
        Self::path_and_cost(state, unit, dest).0
    }

    /// What has to be read before the action lands.
    fn before(&self, state: &GameState, action: Action) -> Value {
        match action {
            Action::Move { unit, dest }
            | Action::Capture { unit, dest }
            | Action::Load { unit, dest }
            | Action::Supply { unit, dest } => json!({"path": Self::path(state, unit, dest)}),
            Action::Join { unit, dest } => {
                let (path, spent) = Self::path_and_cost(state, unit, dest);
                json!({
                    "path": path,
                    "spent": spent,
                    // The mover is consumed by the merge, so its record has to
                    // be kept from before it: AWBW reports the join as the
                    // *mover* arriving at the tile, and names the survivor
                    // separately.
                    "mover": self.unit_json(state, unit),
                })
            }
            Action::Attack { unit, dest, target } => {
                let (path, spent) = Self::path_and_cost(state, unit, dest);
                json!({
                    "path": path,
                    // What the walk costs, kept because a unit killed by the
                    // counterattack cannot be asked for it afterwards.
                    "spent": spent,
                    // Both records are kept from before the fight, because a
                    // unit that dies in it leaves nothing to read afterwards
                    // and AWBW still reports it -- at zero HP, not as a null.
                    // Dropping the defender would take the target tile with
                    // it, and a reader then cannot tell what was attacked.
                    "attacker": self.unit_json(state, unit),
                    "defender": state.unit_id_at(target).map(|d| self.unit_json(state, d)),
                })
            }
            Action::Build { .. } => json!({}),
            Action::Unload { .. } => json!({}),
            Action::Activate { .. } => json!({}),
            Action::EndTurn => json!({"day": state.day, "player": state.current}),
        }
    }

    /// The order record, completed with whatever only exists after the fact.
    fn after(&self, state: &GameState, action: Action, before: Value) -> Option<Value> {
        let path = before.get("path").cloned().unwrap_or(Value::Null);
        Some(match action {
            Action::Move { unit, .. } => json!({
                "kind": "Move", "path": path, "unit": self.unit_json(state, unit),
            }),
            Action::Capture { unit, dest } => {
                let building = state.building_at(dest);
                let taker = state.unit(unit).map(|u| u.owner);
                // A finished capture is not a smaller `remaining`: the engine
                // puts the counter back to full and moves the flag instead, so
                // the only way to tell it from an untouched property is that
                // the tile now belongs to whoever was standing on it. AWBW
                // reports the two cases in quite different shapes.
                let taken = taker.is_some() && building.and_then(|b| b.owner) == taker;
                json!({
                    "kind": "Capt",
                    "path": path,
                    "unit": self.unit_json(state, unit),
                    "x": dest.x,
                    "y": dest.y,
                    "remaining": building.map(|b| b.capture_remaining).unwrap_or(0),
                    "captured": taken,
                    "owner": building.and_then(|b| b.owner),
                    "terrain": building.map(|b| format!("{:?}", b.kind)),
                })
            }
            Action::Attack { unit, dest, target } => {
                // The attacker moved before it fought, so if it did not survive
                // its record has to be carried forward to where it died; the
                // defender never moved, and its own tile is already right.
                let spent = before.get("spent").and_then(Value::as_u64).unwrap_or(0);
                let attacker = if state.unit(unit).is_some() {
                    self.unit_json(state, unit)
                } else {
                    Self::dead_mover(before.get("attacker"), dest, spent as u8)
                };
                json!({
                    "kind": "Fire",
                    "path": path,
                    "unit": attacker,
                    "defender": self.survivor(
                        state, state.unit_id_at(target), before.get("defender")),
                    "target_x": target.x,
                    "target_y": target.y,
                })
            }
            Action::Build { at, .. } => {
                let built = state.unit_id_at(at)?;
                json!({"kind": "Build", "unit": self.unit_json(state, built)})
            }
            Action::Load { unit, dest } => json!({
                "kind": "Load",
                "path": path,
                "unit": self.unit_json(state, unit),
                "transport": state.unit_id_at(dest).map(|t| self.stable(t)),
            }),
            Action::Unload { transport, cargo, drop_at } => json!({
                "kind": "Unload",
                "transport": self.stable(transport),
                "unit": self.unit_json(state, cargo),
                "x": drop_at.x,
                "y": drop_at.y,
            }),
            Action::Join { dest, .. } => {
                // The mover, reported where it ended up rather than where it
                // set off from, which is how AWBW writes a move -- and having
                // paid for the trip, which it also did before being merged
                // away. Its HP is not zeroed: a join is not a death.
                let spent = before.get("spent").and_then(Value::as_u64).unwrap_or(0);
                let mut mover = before.get("mover").cloned().unwrap_or(Value::Null);
                if let Some(fields) = mover.as_object_mut() {
                    let fuel = fields
                        .get("fuel")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .saturating_sub(spent);
                    fields.insert("x".into(), json!(dest.x));
                    fields.insert("y".into(), json!(dest.y));
                    fields.insert("fuel".into(), json!(fuel));
                }
                json!({
                    "kind": "Join",
                    "path": path,
                    "unit": mover,
                    "into": state.unit_id_at(dest).map(|j| self.unit_json(state, j)),
                })
            }
            Action::Supply { unit, .. } => json!({
                "kind": "Supply", "path": path, "unit": self.unit_json(state, unit),
            }),
            Action::Activate { power } => {
                // Shaped like AWBW's own Power record: the flag, and the
                // meter after the cost came off.
                let player = state.current;
                json!({
                    "kind": "Power",
                    "playerID": player,
                    "coName": state.co_of(player).name,
                    "coPower": if power == awbw_engine::state::ActivePower::Scop {
                        "S"
                    } else {
                        "Y"
                    },
                    "playersCOP": state.players[player as usize].charge,
                })
            }
            Action::EndTurn => json!({
                "kind": "End",
                "day": state.day,
                "next": state.current,
                "funds": state.players.iter().map(|p| p.funds).collect::<Vec<_>>(),
            }),
        })
    }

    /// Seals the log once the game is decided or out of time.
    pub fn finish(&mut self, state: &GameState) {
        if self.done.is_some() {
            return;
        }
        self.close_turn();
        let outcome = match state.outcome() {
            Outcome::Winner(p) => json!({"winner": p}),
            _ => json!({"winner": Value::Null}),
        };
        self.done = Some(json!({
            "days": state.day,
            "outcome": outcome,
            "turns": Value::Array(std::mem::take(&mut self.turns)),
        }));
    }

    /// Takes a finished game's log, leaving the recorder ready for the next.
    pub fn take(&mut self) -> Option<Value> {
        self.done.take()
    }

    /// Drops a part-played game, for a slot that is being restarted.
    pub fn clear(&mut self) {
        self.turns.clear();
        self.orders.clear();
        self.snapshot = None;
        self.open = None;
    }
}
