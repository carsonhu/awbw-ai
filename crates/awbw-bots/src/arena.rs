//! Running matches and turning the results into a rating.
//!
//! Seats are swapped every other game, so a bot that only knows how to play
//! first cannot pick up a rating from the draw.

use awbw_engine::actions::Engine;
use awbw_engine::state::{GameSettings, GameState, Outcome, Player, PlayerId};

use crate::awbw_map::AwbwMap;
use crate::map::symmetric_map;
use crate::Bot;

/// Where matches are played.
///
/// A real league map is the honest test — it is the board the replay corpus was
/// recorded on and the one an agent would face — but the synthetic board needs
/// no data file, so it stays available for tests.
#[derive(Debug, Clone)]
pub enum Board {
    Synthetic { width: u8, height: u8 },
    Awbw(Box<AwbwMap>),
}

impl Board {
    pub fn name(&self) -> &str {
        match self {
            Board::Synthetic { .. } => "synthetic",
            Board::Awbw(m) => &m.name,
        }
    }

    /// The opening position, including whatever units the map deploys.
    pub fn new_state(&self, fog: bool) -> GameState {
        let settings = GameSettings { fog, ..GameSettings::default() };
        match self {
            Board::Synthetic { width, height } => {
                let (map, owners) = symmetric_map(*width, *height);
                let players = vec![Player::new(10_000, 1), Player::new(10_000, 2)];
                GameState::new(map, settings, players, &owners)
            }
            Board::Awbw(m) => m.new_game(settings, 10_000),
        }
    }
}

impl Default for Board {
    fn default() -> Self {
        Board::Synthetic { width: 13, height: 13 }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MatchResult {
    pub wins_a: u32,
    pub wins_b: u32,
    pub draws: u32,
}

impl MatchResult {
    pub fn games(&self) -> u32 {
        self.wins_a + self.wins_b + self.draws
    }

    /// A's score, counting a draw as half a win.
    pub fn score_a(&self) -> f64 {
        if self.games() == 0 {
            return 0.5;
        }
        (self.wins_a as f64 + 0.5 * self.draws as f64) / self.games() as f64
    }
}

/// Rating difference implied by a score, in Elo points.
///
/// Clamped, because a clean sweep implies an infinite gap and the honest
/// reading of one is "at least this much".
pub fn elo_difference(score: f64) -> f64 {
    let clamped = score.clamp(0.005, 0.995);
    -400.0 * (1.0 / clamped - 1.0).log10()
}

fn new_game(board: &Board, seed: u64, fog: bool) -> Engine {
    Engine::new(board.new_state(fog), seed)
}

/// Plays one game and returns the winner, or `None` for a draw on the day cap.
///
/// The cap matters: two bad players can shuffle indefinitely, and a match
/// harness that waits for a decisive result would simply hang.
pub fn play_game(
    first: &mut dyn Bot,
    second: &mut dyn Bot,
    seed: u64,
    max_day: u16,
    fog: bool,
) -> Option<PlayerId> {
    play_game_on(&Board::default(), first, second, seed, max_day, fog)
}

/// As [`play_game`], on a chosen board.
pub fn play_game_on(
    board: &Board,
    first: &mut dyn Bot,
    second: &mut dyn Bot,
    seed: u64,
    max_day: u16,
    fog: bool,
) -> Option<PlayerId> {
    let mut engine = new_game(board, seed, fog);
    first.reset(seed);
    second.reset(seed ^ 0x9E37_79B9);

    while engine.state.day <= max_day {
        match engine.state.outcome() {
            Outcome::Winner(p) => return Some(p),
            Outcome::Draw => return None,
            Outcome::InProgress => {}
        }
        let action = if engine.state.current == 0 {
            first.choose(&mut engine)
        } else {
            second.choose(&mut engine)
        };
        // A bot that returns something illegal forfeits its turn rather than
        // taking the whole harness down with it.
        if engine.apply(action).is_err() {
            let _ = engine.apply(awbw_engine::actions::Action::EndTurn);
        }
    }

    // Out of time: award it to whoever is further ahead, and call a genuine
    // stalemate a draw.
    let value = |p: PlayerId| {
        engine
            .state
            .units_of(p)
            .map(|u| u.typ.stats().cost as u64 * u.hp100 as u64 / 100)
            .sum::<u64>()
            + engine.state.property_count(p) as u64 * 5_000
    };
    let (a, b) = (value(0), value(1));
    match a.cmp(&b) {
        std::cmp::Ordering::Greater => Some(0),
        std::cmp::Ordering::Less => Some(1),
        std::cmp::Ordering::Equal => None,
    }
}

/// Plays `games` games, alternating who moves first.
pub fn play_match(a: &mut dyn Bot, b: &mut dyn Bot, games: u32, max_day: u16) -> MatchResult {
    play_match_on(&Board::default(), a, b, games, max_day)
}

/// As [`play_match`], on a chosen board.
///
/// Swapping seats matters more on a real map than on the synthetic one: league
/// maps often hand the player moving second an extra starting unit, so the
/// seats are not interchangeable.
pub fn play_match_on(
    board: &Board,
    a: &mut dyn Bot,
    b: &mut dyn Bot,
    games: u32,
    max_day: u16,
) -> MatchResult {
    let mut result = MatchResult::default();
    for game in 0..games {
        let seed = 0xA11CE ^ game as u64;
        let a_is_first = game % 2 == 0;
        let winner = if a_is_first {
            play_game_on(board, a, b, seed, max_day, false)
        } else {
            play_game_on(board, b, a, seed, max_day, false)
        };
        let a_seat: PlayerId = if a_is_first { 0 } else { 1 };
        match winner {
            Some(p) if p == a_seat => result.wins_a += 1,
            Some(_) => result.wins_b += 1,
            None => result.draws += 1,
        }
    }
    result
}
