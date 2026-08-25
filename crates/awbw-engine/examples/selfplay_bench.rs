//! Random-policy self-play throughput, the number that decides whether RL
//! training at this scale is practical.
//!
//! Run with: cargo run --release --example selfplay_bench

use std::sync::Arc;
use std::time::Instant;

use awbw_engine::actions::Engine;
use awbw_engine::map::{Map, Pos};
use awbw_engine::rng::Rng;
use awbw_engine::state::{GameSettings, GameState, Outcome, Player};
use awbw_engine::types::{TerrainKind, UnitType};

/// A small symmetric two-player map: HQ + base + airport per side, a contested
/// city row in the middle, and some woods and mountains for terrain variety.
fn build_map(width: u8, height: u8) -> (Arc<Map>, Vec<Option<u8>>) {
    let mut kinds = vec![TerrainKind::Plain; width as usize * height as usize];
    let at = |x: u8, y: u8| y as usize * width as usize + x as usize;

    for y in 0..height {
        for x in 0..width {
            if (x + y) % 7 == 3 {
                kinds[at(x, y)] = TerrainKind::Wood;
            } else if (x * 3 + y) % 11 == 5 {
                kinds[at(x, y)] = TerrainKind::Mountain;
            }
        }
    }

    kinds[at(0, 0)] = TerrainKind::Hq;
    kinds[at(1, 0)] = TerrainKind::Base;
    kinds[at(0, 1)] = TerrainKind::Base;
    kinds[at(2, 0)] = TerrainKind::Airport;

    kinds[at(width - 1, height - 1)] = TerrainKind::Hq;
    kinds[at(width - 2, height - 1)] = TerrainKind::Base;
    kinds[at(width - 1, height - 2)] = TerrainKind::Base;
    kinds[at(width - 3, height - 1)] = TerrainKind::Airport;

    let mid = height / 2;
    for x in 0..width {
        if x % 2 == 0 {
            kinds[at(x, mid)] = TerrainKind::City;
        }
    }

    let map = Map::from_kinds(width, height, kinds).unwrap();
    // Properties come back in row-major order; the two corners get owners.
    let owners = map
        .properties()
        .iter()
        .map(|p| {
            if p.pos.y < mid {
                if p.pos.y <= 1 && p.pos.x <= 2 {
                    Some(0u8)
                } else {
                    None
                }
            } else if p.pos.y >= height - 2 && p.pos.x >= width - 3 {
                Some(1u8)
            } else {
                None
            }
        })
        .collect();
    (Arc::new(map), owners)
}

fn new_engine(seed: u64) -> Engine {
    let (map, owners) = build_map(15, 15);
    let players = vec![
        Player { funds: 10_000, team: 1, eliminated: false },
        Player { funds: 10_000, team: 2, eliminated: false },
    ];
    let state = GameState::new(map, GameSettings::default(), players, &owners);
    let mut engine = Engine::new(state, seed);
    engine.state.spawn(UnitType::Infantry, 0, Pos::new(3, 1));
    engine.state.spawn(UnitType::Infantry, 1, Pos::new(11, 13));
    engine
}

struct Stats {
    steps: u64,
    branching: f64,
    days: f64,
    elapsed: f64,
}

/// Enumerate every legal order each step, then pick one. This is what a flat
/// policy or a search needs.
fn run_flat(games: u32, max_day: u16) -> Stats {
    let mut steps = 0u64;
    let mut actions_seen = 0u64;
    let mut days = 0u64;
    let mut buffer = Vec::new();

    let start = Instant::now();
    for game in 0..games {
        let mut engine = new_engine(game as u64);
        let mut rng = Rng::new(0xBEEF + game as u64);
        while engine.state.outcome() == Outcome::InProgress && engine.state.day <= max_day {
            engine.legal_actions_into(&mut buffer);
            actions_seen += buffer.len() as u64;
            let pick = buffer[rng.roll_inclusive(buffer.len() as u32 - 1) as usize];
            engine.apply(pick).expect("enumerated actions must apply");
            steps += 1;
        }
        days += engine.state.day as u64;
    }
    let elapsed = start.elapsed().as_secs_f64();
    Stats {
        steps,
        branching: actions_seen as f64 / steps as f64,
        days: days as f64 / games as f64,
        elapsed,
    }
}

/// Pick a unit first, then enumerate only that unit's orders. This is the
/// factorized action space an RL policy would use, and it costs one
/// reachability search per step instead of one per unit.
fn run_factorized(games: u32, max_day: u16) -> Stats {
    let mut steps = 0u64;
    let mut actions_seen = 0u64;
    let mut days = 0u64;
    let mut buffer = Vec::new();
    let mut movable = Vec::new();

    let start = Instant::now();
    for game in 0..games {
        let mut engine = new_engine(game as u64);
        let mut rng = Rng::new(0xBEEF + game as u64);
        while engine.state.outcome() == Outcome::InProgress && engine.state.day <= max_day {
            movable.clear();
            movable.extend(engine.movable_units());

            // Roll among the movable units plus "build or end the turn".
            let choice = rng.roll_inclusive(movable.len() as u32);
            if choice as usize >= movable.len() {
                engine.legal_actions_into(&mut buffer);
                let builds: Vec<_> = buffer
                    .iter()
                    .copied()
                    .filter(|a| matches!(a, awbw_engine::actions::Action::Build { .. }))
                    .collect();
                let pick = if builds.is_empty() || rng.roll_inclusive(3) == 0 {
                    awbw_engine::actions::Action::EndTurn
                } else {
                    builds[rng.roll_inclusive(builds.len() as u32 - 1) as usize]
                };
                engine.apply(pick).expect("action must apply");
            } else {
                let unit = movable[choice as usize];
                engine.legal_actions_for(unit, &mut buffer);
                actions_seen += buffer.len() as u64;
                if buffer.is_empty() {
                    engine.apply(awbw_engine::actions::Action::EndTurn).unwrap();
                } else {
                    let pick = buffer[rng.roll_inclusive(buffer.len() as u32 - 1) as usize];
                    engine.apply(pick).expect("action must apply");
                }
            }
            steps += 1;
        }
        days += engine.state.day as u64;
    }
    let elapsed = start.elapsed().as_secs_f64();
    Stats {
        steps,
        branching: actions_seen as f64 / steps as f64,
        days: days as f64 / games as f64,
        elapsed,
    }
}

fn report(label: &str, s: &Stats) {
    println!("{label}");
    println!("  micro-steps:    {}", s.steps);
    println!("  mean days/game: {:.1}", s.days);
    println!("  mean branching: {:.1}", s.branching);
    println!("  elapsed:        {:.2}s", s.elapsed);
    println!(
        "  throughput:     {:.0} micro-steps/sec/core",
        s.steps as f64 / s.elapsed
    );
}

fn main() {
    // Real AWBW 1v1s run roughly 20-30 days, so cap there: a random policy left
    // alone builds until the unit cap and reports throughput for positions no
    // trained agent would ever reach.
    const GAMES: u32 = 300;
    const MAX_DAY: u16 = 30;

    report("flat enumeration (all units, every step)", &run_flat(GAMES, MAX_DAY));
    println!();
    report("factorized (unit first, then order)", &run_factorized(GAMES, MAX_DAY));
}
