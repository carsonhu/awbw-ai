//! How often does a game end in an actual win, rather than running out of days?
//!
//! This decides how much learning signal a win/loss reward can carry. A
//! terminal reward is worth nothing if nobody ever reaches a terminal state,
//! and the arena's tiebreak at the day cap hides that: it reports a winner
//! either way. Here the two are counted apart.

use awbw_bots::greedy::GreedyBot;
use awbw_bots::map::symmetric_map;
use awbw_bots::{Bot, RandomBot};
use awbw_engine::actions::{Action, Engine};
use awbw_engine::state::{GameSettings, GameState, Outcome, Player};

fn play(first: &mut dyn Bot, second: &mut dyn Bot, seed: u64, max_day: u16) -> (bool, u16) {
    let (map, owners) = symmetric_map(13, 13);
    let players = vec![Player::new(10_000, 1), Player::new(10_000, 2)];
    let state = GameState::new(map, GameSettings::default(), players, &owners);
    let mut engine = Engine::new(state, seed);
    first.reset(seed);
    second.reset(seed ^ 0x9E37_79B9);

    while engine.state.day <= max_day {
        if engine.state.outcome() != Outcome::InProgress {
            return (true, engine.state.day);
        }
        let action = if engine.state.current == 0 {
            first.choose(&mut engine)
        } else {
            second.choose(&mut engine)
        };
        if engine.apply(action).is_err() {
            let _ = engine.apply(Action::EndTurn);
        }
    }
    (false, engine.state.day)
}

fn measure(label: &str, a: &mut dyn Bot, b: &mut dyn Bot, games: u32, max_day: u16) {
    let mut real_wins = 0;
    let mut total_days = 0u32;
    for g in 0..games {
        let (won, days) = play(a, b, 0x00C0_FFEE ^ g as u64, max_day);
        if won {
            real_wins += 1;
            total_days += days as u32;
        }
    }
    let mean = if real_wins > 0 {
        format!("{:.0}", total_days as f64 / real_wins as f64)
    } else {
        "-".to_string()
    };
    println!(
        "  {label:<20} {real_wins:>3}/{games} ended in a real win (mean day {mean}), \
         {} timed out",
        games - real_wins
    );
}

fn main() {
    for max_day in [30u16, 60, 120] {
        println!("{max_day}-day cap:");
        measure("random vs random", &mut RandomBot::new(1), &mut RandomBot::new(2), 40, max_day);
        measure("greedy vs greedy", &mut GreedyBot::new(), &mut GreedyBot::new(), 40, max_day);
        measure("greedy vs random", &mut GreedyBot::new(), &mut RandomBot::new(3), 40, max_day);
        println!();
    }
}
