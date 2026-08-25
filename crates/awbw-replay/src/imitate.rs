//! Turning recorded human games into imitation labels.
//!
//! A behaviour-cloning sample is a position and the order a human gave in it.
//! The engine already reconstructs the position; this turns the recorded order
//! into the engine's own `Action` vocabulary, and from there into the four head
//! indices a policy predicts.
//!
//! Nothing is written to disk. An observation is tens of thousands of floats
//! and the engine regenerates one in microseconds, so the cursor walks a replay
//! and hands out positions on demand.
//!
//! **The labels check themselves.** A translated order is put through
//! `Engine::check` before it is offered, so an order this module gets wrong is
//! counted rather than quietly taught. Orders played while a CO power is active
//! are flagged: the engine does not model powers, so those positions do not
//! explain the human's choice and belong out of the loss.

use awbw_engine::actions::{Action, Engine};
use awbw_engine::encoding::{encode, ActionCode};
use awbw_engine::map::Pos;
use awbw_engine::state::{GameState, UnitId};
use awbw_engine::types::UnitType;
use awbw_engine::vision::Vision;

use crate::schema::{unwrap_vision, Replay, Turn};
use crate::{Loaded, Verifier};

/// One imitation sample: the order, and whether it can be trusted as a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub code: ActionCode,
    /// The engine agrees this order is legal in the position it was taken from.
    pub legal: bool,
    /// A CO power was active. The engine does not model powers, so the position
    /// does not explain the choice; drop these from the loss.
    pub power_active: bool,
}

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

fn unit_type_by_name(name: &str) -> Option<UnitType> {
    UnitType::ALL
        .into_iter()
        .find(|t| t.stats().name.eq_ignore_ascii_case(name))
}

/// The acting unit and where it ended up, from a `Move` sub-payload.
fn move_parts(loaded: &Loaded, mv: &serde_json::Value) -> Option<(UnitId, Pos)> {
    let rec = mv.get("unit").and_then(unwrap_vision)?;
    let unit = loaded.unit_for(as_num(rec.get("units_id"))?)?;
    let path = mv.get("paths").and_then(unwrap_vision)?.as_array()?;
    let last = path.last()?;
    Some((
        unit,
        Pos::new(
            as_num(last.get("x"))? as u8,
            as_num(last.get("y"))? as u8,
        ),
    ))
}

/// Translates one recorded order into the engine's vocabulary.
///
/// Returns `None` for anything that is not a decision a policy makes — powers,
/// resignations, the dive and surface toggles — rather than inventing a label.
pub fn translate(loaded: &Loaded, action: &serde_json::Value) -> Option<Action> {
    let kind = action.get("action")?.as_str()?;
    match kind {
        "End" => Some(Action::EndTurn),

        "Build" => {
            let rec = action.get("newUnit").and_then(unwrap_vision)?;
            let typ = unit_type_by_name(rec.get("units_name")?.as_str()?)?;
            Some(Action::Build {
                at: Pos::new(
                    as_num(rec.get("units_x"))? as u8,
                    as_num(rec.get("units_y"))? as u8,
                ),
                typ,
            })
        }

        "Move" => {
            let (unit, dest) = move_parts(loaded, action)?;
            Some(Action::Move { unit, dest })
        }

        "Capt" => {
            let (unit, dest) = move_parts(loaded, action.get("Move")?)?;
            Some(Action::Capture { unit, dest })
        }

        "Fire" => {
            let (unit, dest) = move_parts(loaded, action.get("Move")?)?;
            let info = action
                .get("Fire")?
                .get("combatInfoVision")
                .and_then(unwrap_vision)?
                .get("combatInfo")?
                .get("defender")?;
            Some(Action::Attack {
                unit,
                dest,
                target: Pos::new(
                    as_num(info.get("units_x"))? as u8,
                    as_num(info.get("units_y"))? as u8,
                ),
            })
        }

        "Join" => {
            let (unit, dest) = move_parts(loaded, action.get("Move")?)?;
            Some(Action::Join { unit, dest })
        }

        "Load" => {
            let (unit, dest) = move_parts(loaded, action.get("Move")?)?;
            Some(Action::Load { unit, dest })
        }

        "Unload" => {
            let rec = action.get("unit").and_then(unwrap_vision)?;
            let cargo = loaded.unit_for(as_num(rec.get("units_id"))?)?;
            let transport = loaded.unit_for(as_num(action.get("transportID"))?)?;
            let dest = loaded.state().unit(transport)?.pos;
            Some(Action::Unload {
                transport,
                dest,
                cargo,
                drop_at: Pos::new(
                    as_num(rec.get("units_x"))? as u8,
                    as_num(rec.get("units_y"))? as u8,
                ),
            })
        }

        "Supply" | "Repair" => {
            let (unit, dest) = match action.get("Move") {
                Some(mv) => move_parts(loaded, mv)?,
                None => {
                    let rec = action.get("unit").and_then(unwrap_vision)?;
                    let unit = loaded.unit_for(as_num(rec.get("units_id"))?)?;
                    (unit, loaded.state().unit(unit)?.pos)
                }
            };
            Some(Action::Supply { unit, dest })
        }

        // Not policy decisions, or not modelled.
        _ => None,
    }
}

/// Walks one replay, handing out positions and the orders played in them.
pub struct Cursor<'a> {
    verifier: &'a Verifier<'a>,
    replay: &'a Replay,
    turn: usize,
    order: usize,
    loaded: Option<Loaded>,
    /// A power fired earlier in this turn, so the rest of it is off-model.
    power_active: bool,
    vision: Vision,
}

impl<'a> Cursor<'a> {
    pub fn new(verifier: &'a Verifier<'a>, replay: &'a Replay) -> Cursor<'a> {
        let mut cursor = Cursor {
            verifier,
            replay,
            turn: 0,
            order: 0,
            loaded: None,
            power_active: false,
            vision: Vision::new(),
        };
        cursor.open_turn();
        cursor
    }

    fn open_turn(&mut self) {
        while self.turn < self.replay.turns.len() {
            let turn = &self.replay.turns[self.turn];
            // A snapshot with no orders is one whose action record was lost.
            if !turn.actions.is_empty() {
                if let Ok(loaded) = self.verifier.load_turn_public(turn) {
                    self.loaded = Some(loaded);
                    self.order = 0;
                    self.power_active = false;
                    return;
                }
            }
            self.turn += 1;
        }
        self.loaded = None;
    }

    fn turn_ref(&self) -> Option<&'a Turn> {
        self.replay.turns.get(self.turn)
    }

    pub fn finished(&self) -> bool {
        self.loaded.is_none()
    }

    /// The position the next order was played in.
    pub fn state(&self) -> Option<&GameState> {
        self.loaded.as_ref().map(|l| l.state())
    }

    /// What the acting player could see. Full board without fog.
    pub fn vision(&mut self) -> Option<&Vision> {
        let loaded = self.loaded.as_ref()?;
        let player = loaded.state().current;
        self.vision.compute(loaded.state(), player);
        Some(&self.vision)
    }

    /// The order played here, and whether it is fit to learn from. Advances
    /// past orders that carry no label at all.
    pub fn sample(&mut self) -> Option<Sample> {
        loop {
            let turn = self.turn_ref()?;
            let raw = turn.actions.get(self.order)?;
            let loaded = self.loaded.as_mut()?;

            let kind = raw.get("action").and_then(|v| v.as_str()).unwrap_or("");
            if kind == "Power" {
                self.power_active = true;
            }

            match translate(loaded, raw) {
                Some(action) => {
                    let legal = loaded.engine_mut().check(action).is_ok();
                    let code = encode(loaded.state(), action)?;
                    return Some(Sample {
                        code,
                        legal,
                        power_active: self.power_active,
                    });
                }
                // No label here; play it out and look at the next one.
                None => self.advance_raw(),
            }
        }
    }

    /// Plays the current order and moves to the next.
    pub fn advance(&mut self) {
        self.advance_raw();
    }

    fn advance_raw(&mut self) {
        let Some(turn) = self.turn_ref() else { return };
        if let (Some(raw), Some(loaded)) = (turn.actions.get(self.order), self.loaded.as_mut()) {
            if let Some(action) = translate(loaded, raw) {
                // Force it through: a rejected order still happened, and the
                // next position has to reflect it.
                if loaded.engine_mut().apply(action).is_err() {
                    let _ = loaded.engine_mut().apply(Action::EndTurn);
                }
            }
        }
        self.order += 1;
        let done = self
            .turn_ref()
            .map_or(true, |t| self.order >= t.actions.len());
        if done {
            self.turn += 1;
            self.open_turn();
        }
    }
}

impl Loaded {
    /// The engine, mutably, for callers replaying a turn themselves.
    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }
}
