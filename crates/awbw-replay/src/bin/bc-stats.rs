//! How much usable imitation data do the replays actually yield?
//!
//! Before training on human games it is worth knowing how many of their orders
//! survive translation, how many the engine agrees are legal in the position
//! they were played in, and how many have to be dropped because a CO power was
//! running. A label the engine cannot even reproduce is not a label.
//!
//! Usage: bc-stats <prepared dir> [--limit N] [--map NAME]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use awbw_engine::encoding::OrderKind;
use awbw_replay::imitate::Cursor;
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
    let want_map = args
        .iter()
        .position(|a| a == "--map")
        .and_then(|i| args.get(i + 1))
        .cloned();

    let (mut games, mut samples, mut legal, mut powered) = (0u64, 0u64, 0u64, 0u64);
    // Legal orders whose action code does not decode back to the order itself.
    // Those are labels a masked policy could never emit, so they are worse than
    // useless: the loss pulls toward an output the sampler cannot produce.
    let mut unemittable = 0u64;
    // Illegality by position within the turn. If a rejected order corrupts the
    // state, later orders in the same turn should fail far more often.
    let mut by_position: Vec<(u64, u64)> = vec![(0, 0); 6];
    let mut kinds: BTreeMap<&str, u64> = BTreeMap::new();
    let mut illegal_kinds: BTreeMap<&str, u64> = BTreeMap::new();

    for file in collect(Path::new(&dir), limit) {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let Ok(replay) = serde_json::from_str::<Replay>(&text) else { continue };
        if replay.fog {
            continue;
        }
        if let Some(want) = &want_map {
            if replay.map_name.as_deref() != Some(want.as_str()) {
                continue;
            }
        }
        let Ok(verifier) = Verifier::new(std::sync::Arc::new(replay)) else { continue };
        games += 1;

        let mut cursor = Cursor::new(verifier);
        while !cursor.finished() {
            let Some(sample) = cursor.sample() else { break };
            samples += 1;
            let bucket = (cursor.order_index() / 5).min(by_position.len() - 1);
            by_position[bucket].0 += 1;
            if !sample.legal {
                by_position[bucket].1 += 1;
            }
            let name = OrderKind::from_index(sample.code.kind as usize)
                .map(|k| match k {
                    OrderKind::Wait => "move",
                    OrderKind::Attack => "attack",
                    OrderKind::Capture => "capture",
                    OrderKind::Supply => "supply",
                    OrderKind::Join => "join",
                    OrderKind::Load => "load",
                    OrderKind::Unload => "unload",
                    OrderKind::Build => "build",
                })
                .unwrap_or("?");
            *kinds.entry(name).or_insert(0) += 1;
            if sample.legal {
                legal += 1;
                if !sample.emittable {
                    unemittable += 1;
                }
            } else {
                *illegal_kinds.entry(name).or_insert(0) += 1;
            }
            if sample.power_active {
                powered += 1;
            }
            cursor.advance();
        }
    }

    let pct = |n: u64| 100.0 * n as f64 / samples.max(1) as f64;
    println!("{games} games -> {samples} labelled orders");
    println!("  the engine agrees are legal: {legal} ({:.2}%)", pct(legal));
    println!(
        "  legal but not emittable:    {unemittable} ({:.3}%)",
        pct(unemittable)
    );
    println!("  played under a CO power:     {powered} ({:.1}%)", pct(powered));
    let usable = samples.saturating_sub(powered);
    println!("  usable for a loss:           {usable} ({:.1}%)", pct(usable));
    if games > 0 {
        println!("  per game:                    {}", samples / games);
    }

    println!("\nillegality by position within the turn:");
    for (i, (total, bad)) in by_position.iter().enumerate() {
        if *total == 0 {
            continue;
        }
        let label = if i + 1 == by_position.len() {
            format!("{}+", i * 5)
        } else {
            format!("{}-{}", i * 5, i * 5 + 4)
        };
        println!(
            "  orders {label:<7} {:>5.1}% rejected  ({total} orders)",
            100.0 * *bad as f64 / *total as f64
        );
    }

    println!("\nwhat humans actually do:");
    let mut rows: Vec<_> = kinds.iter().collect();
    rows.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    for (name, n) in rows {
        let bad = illegal_kinds.get(*name).copied().unwrap_or(0);
        let note = if bad > 0 {
            format!("   ({bad} the engine rejects)")
        } else {
            String::new()
        };
        println!("  {name:<8} {:>6.1}%{note}", pct(*n));
    }
}
