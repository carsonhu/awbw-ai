//! Checks the engine's fog-of-war rules against recorded fog games.
//!
//! Fog replays turn out to store the *full* board in their per-turn snapshots,
//! so those cannot test vision. The move records can: AWBW writes one path per
//! player, and the opposing player's copy flags each step with whether they
//! could see the unit standing there. That is a direct, per-tile statement of
//! what the defender's fog allowed, and the engine has to agree with it.
//!
//! An opponent's vision is fixed while it is not their turn, so it is computed
//! once per turn from the snapshot and then queried per step.
//!
//! Usage: check-fog <prepared dir> [--limit N] [--verbose]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use awbw_engine::map::Pos;
use awbw_engine::state::PlayerId;
use awbw_engine::types::{MoveType, UnitType};
use awbw_engine::vision::Vision;
use awbw_replay::schema::Replay;
use awbw_replay::Verifier;

fn collect(dir: &Path, limit: usize) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    if limit > 0 {
        files.truncate(limit);
    }
    files
}

/// Pulls the `Move` sub-payload out of whichever action wraps it.
fn move_payload(action: &serde_json::Value) -> Option<&serde_json::Value> {
    if action.get("action").and_then(|v| v.as_str()) == Some("Move") {
        return Some(action);
    }
    action.get("Move").filter(|m| m.is_object())
}

/// Removes units this action killed, returning whether anything died.
///
/// Only the defender's losses matter for the check, but removing both sides
/// keeps the board honest for later actions in the same turn.
fn apply_casualties(loaded: &mut awbw_replay::Loaded, action: &serde_json::Value) -> bool {
    let Some(info) = action
        .get("Fire")
        .and_then(|f| f.get("combatInfoVision"))
        .and_then(|v| awbw_replay::schema::unwrap_vision(v))
        .and_then(|v| v.get("combatInfo"))
    else {
        return false;
    };
    let mut died = false;
    for side in ["attacker", "defender"] {
        let Some(rec) = info.get(side) else { continue };
        let hp = rec
            .get("units_hit_points")
            .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()));
        if hp != Some(0) {
            continue;
        }
        let Some(awbw_id) = rec.get("units_id").and_then(|v| v.as_i64()) else {
            continue;
        };
        if let Some(id) = loaded.unit_for(awbw_id) {
            loaded.state_mut().destroy(id);
            died = true;
        }
    }
    died
}

fn unit_type_by_name(name: &str) -> Option<UnitType> {
    UnitType::ALL
        .into_iter()
        .find(|t| t.stats().name.eq_ignore_ascii_case(name))
}

#[derive(Default)]
struct Tally {
    steps: usize,
    agree: usize,
    /// AWBW says visible, engine says hidden: our sight is too short.
    too_blind: usize,
    /// AWBW says hidden, engine says visible: our sight is too generous.
    too_sharp: usize,
    samples: Vec<String>,
    blind_samples: Vec<String>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "--verbose");
    let dir = args
        .iter()
        .find(|a| !a.starts_with('-') && a.parse::<usize>().is_err())
        .cloned()
        .unwrap_or_else(|| "data/prepared".to_string());
    let limit = args
        .iter()
        .position(|a| a == "--limit")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0usize);

    let mut tally = Tally::default();
    let mut fog_games = 0usize;
    let mut skipped_types = 0usize;

    for file in collect(Path::new(&dir), limit) {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let Ok(replay) = serde_json::from_str::<Replay>(&text) else {
            continue;
        };
        if !replay.fog {
            continue;
        }
        let replay = std::sync::Arc::new(replay);
        let Ok(verifier) = Verifier::new(replay.clone()) else {
            continue;
        };
        fog_games += 1;

        // AWBW player id -> engine seat.
        let seats: HashMap<i64, PlayerId> = replay
            .players
            .iter()
            .filter_map(|p| verifier.player_index(p.id).map(|i| (p.id, i)))
            .collect();

        for turn in &replay.turns {
            let Ok(mut loaded) = verifier.load_turn_public(turn) else {
                continue;
            };
            let active = loaded.state().current;

            // A defender's sight shrinks as its units fall, so the view is
            // rebuilt whenever this turn claims another casualty rather than
            // being computed once from the opening snapshot.
            let mut views: HashMap<i64, Vision> = HashMap::new();
            let mut stale = true;

            for action in &turn.actions {
                if stale {
                    views.clear();
                    for (&awbw_id, &seat) in &seats {
                        if loaded.state().are_allied(active, seat) {
                            continue;
                        }
                        let mut v = Vision::new();
                        v.compute(loaded.state(), seat);
                        views.insert(awbw_id, v);
                    }
                    stale = false;
                }
                let state = loaded.state();
                let Some(mv) = move_payload(action) else {
                    stale |= apply_casualties(&mut loaded, action);
                    continue;
                };
                let Some(paths) = mv.get("paths").and_then(|p| p.as_object()) else {
                    continue;
                };
                // The mover's own record carries its type and dive state.
                let Some(rec) = mv
                    .get("unit")
                    .and_then(|u| u.as_object())
                    .and_then(|o| o.values().find(|v| v.is_object()))
                else {
                    continue;
                };
                let Some(typ) = rec
                    .get("units_name")
                    .and_then(|v| v.as_str())
                    .and_then(unit_type_by_name)
                else {
                    skipped_types += 1;
                    continue;
                };
                let hidden = rec.get("units_sub_dive").and_then(|v| v.as_str()) == Some("Y");
                let is_air = typ.stats().move_type == MoveType::Air;

                for (pid, steps) in paths {
                    let Ok(awbw_id) = pid.parse::<i64>() else {
                        continue;
                    };
                    let Some(view) = views.get(&awbw_id) else {
                        continue; // the mover's own view tells us nothing
                    };
                    let Some(steps) = steps.as_array() else {
                        continue;
                    };
                    for (index, step) in steps.iter().enumerate() {
                        let (Some(x), Some(y)) = (
                            step.get("x").and_then(|v| v.as_i64()),
                            step.get("y").and_then(|v| v.as_i64()),
                        ) else {
                            continue;
                        };
                        let recorded = step
                            .get("unit_visible")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        let pos = Pos::new(x as u8, y as u8);

                        // Concealment is tested per step, not just where the
                        // unit stops: exempting the tiles it passes through was
                        // tried and cost four points of agreement, so AWBW
                        // really does re-hide a mover behind every cover tile
                        // it crosses.
                        let _ = index;
                        let terrain = state.map.terrain_at(pos);
                        let predicted = if !view.sees_tile(state, pos) {
                            false
                        } else if view.pierces_tile(state, pos) {
                            true
                        } else if hidden {
                            false
                        } else {
                            !terrain.provides_cover() || is_air
                        };

                        tally.steps += 1;
                        if predicted == recorded {
                            tally.agree += 1;
                        } else if recorded {
                            tally.too_blind += 1;
                            if tally.blind_samples.len() < 8 {
                                let seat = seats[&awbw_id];
                                let closest = state
                                    .units()
                                    .filter(|u| {
                                        state.are_allied(seat, u.owner) && u.carried_by.is_none()
                                    })
                                    .map(|u| (u.pos.distance(pos), u.typ.stats().name))
                                    .min_by_key(|t| t.0);
                                let lit = view.sees_tile(state, pos);
                                let pierced = view.pierces_tile(state, pos);
                                tally.blind_samples.push(format!(
                                    "game {} day {}: {} at {pos:?} on {terrain:?} (dived={hidden}) \
                                     seen by AWBW; engine lit={lit} pierced={pierced}, nearest \
                                     watcher {closest:?}",
                                    replay.game_id,
                                    turn.day,
                                    typ.stats().name
                                ));
                            }
                        } else {
                            tally.too_sharp += 1;
                            if tally.samples.len() < 10 {
                                // Name the closest watcher, so an over-generous
                                // rule can be attributed rather than guessed at.
                                let seat = seats[&awbw_id];
                                let closest = state
                                    .units()
                                    .filter(|u| {
                                        state.are_allied(seat, u.owner) && u.carried_by.is_none()
                                    })
                                    .map(|u| {
                                        (
                                            u.pos.distance(pos),
                                            u.typ.stats().name,
                                            u.typ.stats().vision,
                                            state.map.terrain_at(u.pos),
                                        )
                                    })
                                    .min_by_key(|t| t.0);
                                let prop = state
                                    .buildings()
                                    .iter()
                                    .filter(|b| b.owner.is_some_and(|o| state.are_allied(seat, o)))
                                    .map(|b| b.pos.distance(pos))
                                    .min();
                                tally.samples.push(format!(
                                    "game {} day {}: {} at {pos:?} on {terrain:?}; nearest watcher \
                                     {closest:?}, nearest own property {prop:?}",
                                    replay.game_id,
                                    turn.day,
                                    typ.stats().name
                                ));
                            }
                        }
                    }
                }
                stale |= apply_casualties(&mut loaded, action);
            }
        }
    }

    let pct = if tally.steps > 0 {
        100.0 * tally.agree as f64 / tally.steps as f64
    } else {
        0.0
    };
    println!("fog games:        {fog_games}");
    println!("path steps judged: {}", tally.steps);
    println!("agreement:        {pct:.2}%");
    println!("  engine too blind (AWBW saw it):  {}", tally.too_blind);
    println!("  engine too sharp (AWBW did not): {}", tally.too_sharp);
    if skipped_types > 0 {
        println!("  skipped (unknown unit type):     {skipped_types}");
    }
    if verbose {
        if !tally.blind_samples.is_empty() {
            println!("
cases the engine failed to see:");
            for s in &tally.blind_samples {
                println!("  {s}");
            }
        }
        if !tally.samples.is_empty() {
            println!("
cases the engine saw but AWBW did not:");
            for s in &tally.samples {
                println!("  {s}");
            }
        }
    }
}
