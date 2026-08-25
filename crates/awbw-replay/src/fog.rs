//! Checking the engine's fog rules against recorded fog games.
//!
//! A fog replay's per-turn snapshot lists only the units the recording player
//! could see, which makes it a direct test of vision: every enemy unit that
//! appears in the snapshot must be one the engine agrees is visible.
//!
//! The test is deliberately one-sided. Units the recorder could *not* see are
//! simply absent from the file, so the corpus can prove the engine's sight is
//! too short but never that it is too long. Vision itself is computed only from
//! the recorder's own units and properties, and those are always listed in
//! full, so the input is complete even though the expected output is not.

use awbw_engine::map::Pos;
use awbw_engine::state::{GameState, PlayerId};
use awbw_engine::vision::Vision;

use crate::schema::{Replay, Turn};

#[derive(Debug, Default, Clone)]
pub struct FogReport {
    pub game_id: i64,
    pub turns: usize,
    /// Enemy units listed in a snapshot, i.e. ones the recorder could see.
    pub visible_enemies: usize,
    /// Of those, how many the engine would wrongly have kept hidden.
    pub missed: usize,
    pub samples: Vec<String>,
}

/// Which seat a fog snapshot was recorded from.
///
/// AWBW writes one file per game rather than per player, so the perspective has
/// to be inferred; whichever seat explains more of the enemy units the file
/// lists is the one the file was written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Perspective {
    ActivePlayer,
    Fixed(PlayerId),
}

/// Counts the enemy units in `turn` that `viewer` should be able to see, and
/// how many the engine would miss.
pub fn check_turn(
    state: &GameState,
    turn: &Turn,
    viewer: PlayerId,
    vision: &mut Vision,
    report: &mut FogReport,
) {
    vision.compute(state, viewer);
    for unit in state.units() {
        if state.are_allied(viewer, unit.owner) || unit.carried_by.is_some() {
            continue;
        }
        report.visible_enemies += 1;
        if !vision.sees_unit(state, unit) {
            report.missed += 1;
            if report.samples.len() < 4 {
                let terrain = state.map.terrain_at(unit.pos);
                report.samples.push(format!(
                    "day {}: {} at {:?} on {terrain:?} (hidden={}) listed but engine says unseen",
                    turn.day,
                    unit.typ.stats().name,
                    unit.pos,
                    unit.hidden
                ));
            }
        }
    }
}

/// The distance from `pos` to the nearest unit or property belonging to
/// `viewer`, for diagnosing how far short the engine's sight fell.
pub fn nearest_watcher(state: &GameState, viewer: PlayerId, pos: Pos) -> Option<u32> {
    let from_units = state
        .units()
        .filter(|u| state.are_allied(viewer, u.owner) && u.carried_by.is_none())
        .map(|u| u.pos.distance(pos));
    let from_props = state
        .buildings()
        .iter()
        .filter(|b| b.owner.is_some_and(|o| state.are_allied(viewer, o)))
        .map(|b| b.pos.distance(pos));
    from_units.chain(from_props).min()
}

/// True when this replay is a fog game worth checking.
pub fn is_fog(replay: &Replay) -> bool {
    replay.fog
}
