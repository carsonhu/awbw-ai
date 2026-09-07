//! Auxiliary value targets for behaviour cloning, dumped once per corpus.
//!
//! KataGo's third global-bundle answer is a value head that also predicts
//! things it was not asked for -- final score, final ownership -- which forces
//! the trunk to carry board-wide state the policy alone never demands. The AWBW
//! analogs are the ones `network.md` names: how the property race *ends*, and
//! where it stands a few days out.
//!
//! These are futures, and a teacher streaming a game in order cannot see them.
//! It can, though, say exactly which (game, turn) each row came from -- so the
//! futures are computed here, offline, and joined on that key at train time.
//! The alternative, replaying every game to its end inside the teacher, pays
//! the cost once per epoch instead of once ever.
//!
//! Turn indices are indices into `replay.turns`, which is what
//! `Cursor::turn_index` reports, so the join needs no translation.
//!
//! Usage: aux-targets [prepared dir] [--map NAME] [--limit N] [--out FILE]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use awbw_replay::schema::Replay;
use awbw_replay::Verifier;

/// What one game contributes. `props` and `final_owner` are parallel, and both
/// are read off the last turn that loads: buildings never move, so the tile
/// list is the map's and only the ownership is a future.
#[derive(serde::Serialize)]
struct GameAux {
    seats: usize,
    funds_per_property: u32,
    /// The board these tile indices are on. A corpus spans maps, and a trainer
    /// serves one: anything else has to be dropped rather than mis-indexed.
    height: u32,
    width: u32,
    /// Income-producing tiles, as `y * width + x` -- the index the net's
    /// per-tile heads already use.
    props: Vec<u32>,
    /// Who holds each of them at the end: a seat, or -1 for neutral.
    final_owner: Vec<i8>,
    final_counts: Vec<u16>,
    /// `[day, income per seat...]` per turn, `null` where the snapshot does not
    /// load. Indexed by turn, so a row joins by position.
    turns: Vec<Option<Vec<i64>>>,
}

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

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = match args.first() {
        Some(first) if !first.starts_with('-') => first.clone(),
        _ => "data/prepared".to_string(),
    };
    let limit = flag(&args, "--limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0usize);
    let want_map = flag(&args, "--map");
    let out = flag(&args, "--out").unwrap_or_else(|| "data/aux-targets.json".to_string());

    let mut games: BTreeMap<String, GameAux> = BTreeMap::new();
    let (mut seen, mut turns_total, mut turns_dropped) = (0u64, 0u64, 0u64);

    for file in collect(Path::new(&dir), limit) {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        if let Some(want) = &want_map {
            if !text.contains(want.as_str()) {
                continue;
            }
        }
        let Ok(replay) = serde_json::from_str::<Replay>(&text) else { continue };
        if let Some(want) = &want_map {
            if replay.map_name.as_deref() != Some(want.as_str()) {
                continue;
            }
        }
        let Ok(verifier) = Verifier::new(std::sync::Arc::new(replay)) else { continue };
        seen += 1;
        // A handle on the same replay, so iterating its turns does not borrow
        // the verifier that has to load each one.
        let replay = verifier.replay().clone();
        let game_id = replay.game_id;
        let seats = replay.players.len();
        let funds_per_property = replay.funds_per_property;
        let (height, width) = (replay.height, replay.width);

        let mut rows: Vec<Option<Vec<i64>>> = Vec::with_capacity(replay.turns.len());
        let mut last: Option<(Vec<u32>, Vec<i8>, Vec<u16>)> = None;
        for turn in &replay.turns {
            turns_total += 1;
            let Ok(loaded) = verifier.load_turn_public(turn) else {
                rows.push(None);
                turns_dropped += 1;
                continue;
            };
            let state = loaded.state();
            let mut row = Vec::with_capacity(seats + 1);
            row.push(turn.day as i64);
            for seat in 0..seats {
                row.push(state.income(seat as u8) as i64);
            }
            rows.push(Some(row));

            let mut props = Vec::new();
            let mut owner = Vec::new();
            for building in state.buildings() {
                if !building.kind.produces_income() {
                    continue;
                }
                props.push(state.map.index(building.pos) as u32);
                owner.push(building.owner.map_or(-1, |p| p as i8));
            }
            let counts = (0..seats)
                .map(|seat| state.property_count(seat as u8))
                .collect();
            last = Some((props, owner, counts));
        }

        let Some((props, final_owner, final_counts)) = last else { continue };
        games.insert(
            game_id.to_string(),
            GameAux {
                seats,
                funds_per_property,
                height,
                width,
                props,
                final_owner,
                final_counts,
                turns: rows,
            },
        );
    }

    let path = Path::new(&out);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let text = serde_json::to_string(&games).expect("serialize");
    std::fs::write(path, &text).expect("write");

    println!("{seen} games read, {} written to {out}", games.len());
    println!(
        "  turns {turns_total}, snapshots that would not load {turns_dropped} ({:.2}%)",
        100.0 * turns_dropped as f64 / turns_total.max(1) as f64
    );
    println!("  {:.1} MB", text.len() as f64 / 1e6);
}
