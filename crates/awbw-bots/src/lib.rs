//! Baseline opponents.
//!
//! These exist to be a *yardstick*, not to be good. Self-play Elo is
//! self-referential — an agent can climb its own ladder while playing badly —
//! so a fixed opponent that never changes is the only thing that says whether
//! a learned policy is actually getting anywhere.

pub mod arena;
pub mod awbw_map;
pub mod greedy;
pub mod map;

use awbw_engine::actions::{Action, Engine};

/// Something that can pick one order at a time.
///
/// One call is one micro-step, the same granularity the RL environment uses, so
/// a bot and a policy are interchangeable in the arena.
pub trait Bot {
    fn name(&self) -> &str;

    /// Chooses an order. Must return something `Engine::check` accepts;
    /// `Action::EndTurn` is always legal, so there is no failure case.
    fn choose(&mut self, engine: &mut Engine) -> Action;

    /// Called once at the start of each game.
    fn reset(&mut self, _seed: u64) {}
}

/// Uniform over the legal orders. The floor: anything that cannot beat this is
/// not playing the game.
pub struct RandomBot {
    rng: awbw_engine::rng::Rng,
    buffer: Vec<Action>,
}

impl RandomBot {
    pub fn new(seed: u64) -> Self {
        RandomBot {
            rng: awbw_engine::rng::Rng::new(seed),
            buffer: Vec::new(),
        }
    }
}

impl Bot for RandomBot {
    fn name(&self) -> &str {
        "random"
    }

    fn reset(&mut self, seed: u64) {
        self.rng = awbw_engine::rng::Rng::new(seed);
    }

    fn choose(&mut self, engine: &mut Engine) -> Action {
        engine.legal_actions_into(&mut self.buffer);
        if self.buffer.is_empty() {
            return Action::EndTurn;
        }
        let i = self.rng.roll_inclusive(self.buffer.len() as u32 - 1) as usize;
        self.buffer[i]
    }
}
