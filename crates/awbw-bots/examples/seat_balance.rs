//! Is the board fair?
//!
//! A mirror match between identical bots should sit near even. A large skew
//! means either the map genuinely favours a seat, or something about the way it
//! is loaded does — and those need telling apart before anything is trained on
//! it.

use awbw_bots::arena::{play_game_on, Board};
use awbw_bots::awbw_map::{AwbwMap, RIVER_SUPREME};
use awbw_bots::greedy::GreedyBot;
use awbw_bots::{Bot, RandomBot};

/// Which bot both seats play, so a skew can be blamed on the board or the bot.
enum Mirror {
    Greedy,
    Capturer,
    Random,
}

fn make(kind: &Mirror, seed: u64) -> Box<dyn Bot> {
    match kind {
        Mirror::Greedy => Box::new(GreedyBot::new()),
        Mirror::Capturer => Box::new(GreedyBot::capture_only()),
        Mirror::Random => Box::new(RandomBot::new(seed)),
    }
}

fn mirror_with(label: &str, kind: Mirror, board: &Board, games: u32, max_day: u16) {
    let (mut seat0, mut seat1, mut timeout) = (0, 0, 0);
    for g in 0..games {
        let mut a = make(&kind, 1000 + g as u64);
        let mut b = make(&kind, 5000 + g as u64);
        match play_game_on(board, a.as_mut(), b.as_mut(), 0xBA1A_1CE ^ g as u64, max_day, false) {
            Some(0) => seat0 += 1,
            Some(_) => seat1 += 1,
            None => timeout += 1,
        }
    }
    let decisive = seat0 + seat1;
    let share = if decisive > 0 {
        format!("{:.0}%", 100.0 * seat0 as f64 / decisive as f64)
    } else {
        "-".into()
    };
    println!("  {label:<34} seat0 {seat0:>3}  seat1 {seat1:>3}  level {timeout:>3}  -> seat0 {share}");
}

fn mirror(label: &str, board: &Board, games: u32, max_day: u16) {
    mirror_with(label, Mirror::Greedy, board, games, max_day);
}

fn main() {
    let path = std::path::Path::new("data/maps/119544.json");
    let path = if path.exists() { path } else { std::path::Path::new(RIVER_SUPREME) };
    let full = AwbwMap::load(path).expect("canonical map");

    // The same map with the map's own starting units removed, to separate the
    // terrain's fairness from the deployment's.
    let mut bare = full.clone();
    bare.deployments.clear();

    // ...and with only the mirrored pair kept, dropping the odd one out.
    let mut mirrored = full.clone();
    let (w, h) = (full.map.width, full.map.height);
    mirrored.deployments.retain(|d| {
        let m = awbw_engine::map::Pos::new(w - 1 - d.pos.x, h - 1 - d.pos.y);
        full.deployments
            .iter()
            .any(|o| o.pos == m && o.typ == d.typ && o.owner != d.owner)
    });

    let real = Board::Awbw(Box::new(full.clone()));
    println!("greedy vs greedy, 60-day cap:");
    mirror("synthetic board", &Board::default(), 60, 60);
    mirror("A River Supreme, as published", &real, 60, 60);
    mirror("  ...with no starting units", &Board::Awbw(Box::new(bare)), 60, 60);
    mirror("  ...with only the mirrored pair", &Board::Awbw(Box::new(mirrored)), 60, 60);

    // A skew that survives swapping the bot is the board's; one that does not
    // belongs to the bot.
    println!("
same board, different mirror:");
    mirror_with("  capturer vs capturer", Mirror::Capturer, &real, 60, 60);
    mirror_with("  random vs random", Mirror::Random, &real, 60, 120);
    mirror_with("  random vs random (synthetic)", Mirror::Random, &Board::default(), 60, 120);
}
