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
        self.snapshot = Some(json!({
            "day": state.day,
            "active": state.current,
            "funds": funds,
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

    /// The path a unit walks to `dest`, origin first, as AWBW records it.
    fn path(state: &GameState, unit: UnitId, dest: Pos) -> Value {
        let mut reach = Reach::new();
        reach.compute(state, unit);
        let steps: Vec<Value> = reach
            .path_to(state, dest)
            .into_iter()
            .map(|p| json!({"x": p.x, "y": p.y}))
            .collect();
        Value::Array(steps)
    }

    /// What has to be read before the action lands.
    fn before(&self, state: &GameState, action: Action) -> Value {
        match action {
            Action::Move { unit, dest }
            | Action::Capture { unit, dest }
            | Action::Load { unit, dest }
            | Action::Supply { unit, dest } => json!({"path": Self::path(state, unit, dest)}),
            Action::Join { unit, dest } => json!({
                "path": Self::path(state, unit, dest),
                // The mover is consumed by the merge, so its record has to be
                // kept from before it: AWBW reports the join as the *mover*
                // arriving at the tile, and names the survivor separately.
                "mover": self.unit_json(state, unit),
            }),
            Action::Attack { unit, dest, target } => json!({
                "path": Self::path(state, unit, dest),
                // Both records are kept from before the fight, because a unit
                // that dies in it leaves nothing to read afterwards and AWBW
                // still reports it -- at zero HP, not as a null. Dropping the
                // defender would take the target tile with it, and a reader
                // then cannot tell what was attacked.
                "attacker": self.unit_json(state, unit),
                "defender": state.unit_id_at(target).map(|d| self.unit_json(state, d)),
            }),
            Action::Build { .. } => json!({}),
            Action::Unload { .. } => json!({}),
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
                json!({
                    "kind": "Capt",
                    "path": path,
                    "unit": self.unit_json(state, unit),
                    "x": dest.x,
                    "y": dest.y,
                    // AWBW reports what is left to capture, and zero means the
                    // property changed hands on this order.
                    "remaining": building.map(|b| b.capture_remaining).unwrap_or(0),
                })
            }
            Action::Attack { unit, target, .. } => json!({
                "kind": "Fire",
                "path": path,
                "unit": self.survivor(state, Some(unit), before.get("attacker")),
                "defender": self.survivor(
                    state, state.unit_id_at(target), before.get("defender")),
                "target_x": target.x,
                "target_y": target.y,
            }),
            Action::Build { at, .. } => {
                let built = state.unit_id_at(at)?;
                json!({"kind": "Build", "unit": self.unit_json(state, built)})
            }
            Action::Load { unit, dest } => json!({
                "kind": "Load",
                "path": path,
                "unit": self.unit_json(state, unit),
                "transport": state.unit_id_at(dest),
            }),
            Action::Unload { transport, cargo, drop_at } => json!({
                "kind": "Unload",
                "transport": transport,
                "unit": self.unit_json(state, cargo),
                "x": drop_at.x,
                "y": drop_at.y,
            }),
            Action::Join { dest, .. } => {
                // The mover, reported where it ended up rather than where it
                // set off from, which is how AWBW writes a move.
                let mut mover = before.get("mover").cloned().unwrap_or(Value::Null);
                if let Some(fields) = mover.as_object_mut() {
                    fields.insert("x".into(), json!(dest.x));
                    fields.insert("y".into(), json!(dest.y));
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
