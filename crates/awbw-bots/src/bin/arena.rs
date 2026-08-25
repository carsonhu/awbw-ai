//! Round-robin between the baseline bots.
//!
//! Usage: arena [--games N] [--max-day D]

use awbw_bots::arena::{elo_difference, play_match_on, Board};
use awbw_bots::awbw_map::{AwbwMap, RIVER_SUPREME};
use awbw_bots::greedy::GreedyBot;
use awbw_bots::{Bot, RandomBot};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str, default: u32| -> u32 {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    // The real league map by default; the synthetic board needs no data file.
    let board = if args.iter().any(|a| a == "--synthetic") {
        Board::default()
    } else {
        let path = args
            .iter()
            .position(|a| a == "--map")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| RIVER_SUPREME.to_string());
        match AwbwMap::load(&path) {
            Ok(m) => Board::Awbw(Box::new(m)),
            Err(e) => {
                eprintln!("could not load {path}: {e}\nfalling back to the synthetic board");
                Board::default()
            }
        }
    };

    if args.iter().any(|a| a == "--show-map") {
        if let Board::Awbw(m) = &board {
            println!("{} ({}x{})", m.name, m.map.width, m.map.height);
            print!("{}", awbw_bots::map::render(&m.map, &m.owners));
            println!("{}", awbw_bots::map::RENDER_LEGEND);
            return;
        }
        let (map, owners) = awbw_bots::map::symmetric_map(13, 13);
        print!("{}", awbw_bots::map::render(&map, &owners));
        println!("{}", awbw_bots::map::RENDER_LEGEND);
        println!(". plain  w wood  ^ mountain  - road  H hq  B base  c city");
        return;
    }
    let games = flag("--games", 20);
    // 60, not 30: two competent players need about 40 days to resolve a game,
    // so a 30-day cap decides every mirror match on the material tiebreak and
    // never on an actual win. See the `decisiveness` example.
    let max_day = flag("--max-day", 60) as u16;

    let mut bots: Vec<Box<dyn Bot>> = vec![
        Box::new(GreedyBot::new()),
        Box::new(GreedyBot::capture_only()),
        Box::new(RandomBot::new(1)),
    ];

    println!(
        "{games} games per pairing on {}, {max_day}-day cap, seats swapped each game\n",
        board.name()
    );

    let names: Vec<String> = bots.iter().map(|b| b.name().to_string()).collect();
    let mut scores = vec![0.0f64; bots.len()];
    let mut played = vec![0u32; bots.len()];

    for i in 0..bots.len() {
        for j in (i + 1)..bots.len() {
            // Split the borrow so two bots from the same vector can play.
            let (left, right) = bots.split_at_mut(j);
            let result =
                play_match_on(&board, left[i].as_mut(), right[0].as_mut(), games, max_day);
            let score = result.score_a();
            println!(
                "{:>8} vs {:<8}  {}-{}-{}  ({:.0}% for {}, {:+.0} Elo)",
                names[i],
                names[j],
                result.wins_a,
                result.wins_b,
                result.draws,
                score * 100.0,
                names[i],
                elo_difference(score),
            );
            scores[i] += score * result.games() as f64;
            scores[j] += (1.0 - score) * result.games() as f64;
            played[i] += result.games();
            played[j] += result.games();
        }
    }

    // Rated against the field rather than against each other, so the gap
    // between two rows is the gap between those bots. A clean sweep implies an
    // infinite rating, so the number is a floor, not a measurement.
    println!("\noverall (Elo vs the field average):");
    let mut table: Vec<(usize, f64)> = (0..bots.len())
        .map(|i| (i, scores[i] / played[i].max(1) as f64))
        .collect();
    table.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    for (i, score) in table {
        println!(
            "  {:<9} {:>3.0}%  {:+.0}",
            names[i],
            score * 100.0,
            elo_difference(score),
        );
    }
}
