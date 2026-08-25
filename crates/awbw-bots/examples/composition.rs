//! What does the greedy bot actually build, and does it ever leave a base idle?
//!
//! Received wisdom in Advance Wars is that a base should almost never sit empty
//! and that infantry are the engine of the whole game, because properties are.
//! A teacher that under-builds infantry teaches a policy to under-build them.

use std::collections::BTreeMap;

use awbw_bots::arena::Board;
use awbw_bots::awbw_map::{AwbwMap, RIVER_SUPREME};
use awbw_bots::greedy::GreedyBot;
use awbw_bots::Bot;
use awbw_engine::actions::{Action, Engine};
use awbw_engine::state::Outcome;
use awbw_engine::types::{TerrainKind, UnitType};

fn main() {
    let path = std::path::Path::new("data/maps/119544.json");
    let path = if path.exists() { path } else { std::path::Path::new(RIVER_SUPREME) };
    let board = Board::Awbw(Box::new(AwbwMap::load(path).expect("canonical map")));

    let mut built: BTreeMap<&str, u32> = BTreeMap::new();
    let mut idle_base_turns = 0u32;
    let mut affordable_idle = 0u32;
    let mut turns = 0u32;
    let mut funds_samples: Vec<u32> = Vec::new();
    let mut infantry_vs_property: Vec<(u32, u32)> = Vec::new();

    for game in 0..12u64 {
        let mut engine = Engine::new(board.new_state(false), 0xB0A7 ^ game);
        let mut bots = [GreedyBot::new(), GreedyBot::new()];
        let mut last_day = 0;

        let mut seat = engine.state.current;
        while engine.state.day <= 40 && engine.state.outcome() == Outcome::InProgress {
            // Sample at the moment the turn changes hands: the previous player
            // has finished, so an empty affordable base is a base-turn thrown
            // away, and funds still in hand are funds that bought nothing.
            if engine.state.current != seat || engine.state.day != last_day {
                seat = engine.state.current;
                last_day = engine.state.day;
                turns += 1;
                let me = engine.state.current;
                funds_samples.push(engine.state.player(me).funds);

                let infantry = engine
                    .state
                    .units_of(me)
                    .filter(|u| matches!(u.typ, UnitType::Infantry | UnitType::Mech))
                    .count() as u32;
                infantry_vs_property.push((infantry, engine.state.property_count(me) as u32));

                let sites: Vec<_> = engine
                    .state
                    .buildings_of(me)
                    .filter(|b| {
                        matches!(b.kind, TerrainKind::Base | TerrainKind::Airport | TerrainKind::Port)
                    })
                    .map(|b| b.pos)
                    .collect();
                for at in sites {
                    if engine.state.unit_id_at(at).is_none() {
                        idle_base_turns += 1;
                        if engine.can_build_anything(at) {
                            affordable_idle += 1;
                        }
                    }
                }
            }

            let seat = engine.state.current as usize;
            let action = bots[seat].choose(&mut engine);
            if let Action::Build { typ, .. } = action {
                *built.entry(typ.stats().name).or_insert(0) += 1;
            }
            if engine.apply(action).is_err() {
                let _ = engine.apply(Action::EndTurn);
            }
        }
    }

    let total: u32 = built.values().sum();
    println!("units built across 12 games ({total} total):");
    let mut rows: Vec<_> = built.into_iter().collect();
    rows.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (name, n) in rows {
        println!("  {name:<12} {n:>4}  {:>5.1}%", 100.0 * n as f64 / total as f64);
    }

    let mean_funds = funds_samples.iter().sum::<u32>() as f64 / funds_samples.len() as f64;
    let hoarding = funds_samples.iter().filter(|&&f| f >= 8_000).count();
    println!("\nat the start of a turn:");
    println!("  mean funds in hand: {mean_funds:.0}");
    println!(
        "  turns holding 8000+ unspent: {hoarding}/{} ({:.0}%)",
        funds_samples.len(),
        100.0 * hoarding as f64 / funds_samples.len() as f64
    );
    println!(
        "  empty production sites seen: {idle_base_turns}, of which {affordable_idle} could \
         have built something"
    );

    let (inf, prop): (u32, u32) = infantry_vs_property
        .iter()
        .fold((0, 0), |acc, x| (acc.0 + x.0, acc.1 + x.1));
    println!(
        "  mean footsoldiers {:.1} against {:.1} properties held",
        inf as f64 / turns as f64,
        prop as f64 / turns as f64
    );
}
