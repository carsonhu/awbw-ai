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
        let Ok(verifier) = Verifier::new(&replay) else { continue };
        games += 1;

        let mut cursor = Cursor::new(&verifier, &replay);
        while !cursor.finished() {
            let Some(sample) = cursor.sample() else { break };
            samples += 1;
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
    println!("  played under a CO power:     {powered} ({:.1}%)", pct(powered));
    let usable = samples.saturating_sub(powered);
    println!("  usable for a loss:           {usable} ({:.1}%)", pct(usable));
    if games > 0 {
        println!("  per game:                    {}", samples / games);
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
