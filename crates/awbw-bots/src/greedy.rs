//! A one-ply heuristic opponent.
//!
//! Scores every legal order by its immediate value and takes the best. There is
//! no search and no lookahead beyond the counterattack the engine already
//! predicts, which is the point: it plays the obvious move, so an agent that
//! cannot beat it has not learned anything an obvious move would not have got.
//!
//! Funds are the common currency. A unit is worth its cost scaled by health, so
//! damage is worth the health it removes, and every other consideration is
//! priced against that.

use awbw_engine::actions::{Action, Engine};
use awbw_engine::map::Pos;
use awbw_engine::state::{PlayerId, Unit};
use awbw_engine::types::{TerrainKind, UnitType};

use crate::Bot;

/// What a property is worth taking, in funds-equivalent.
const CAPTURE_STEP: f32 = 4_000.0;
const CAPTURE_COMPLETE: f32 = 12_000.0;
const HQ_BONUS: f32 = 100_000.0;
/// Per tile of progress toward whatever a unit should be heading for.
const APPROACH: f32 = 120.0;
/// Ending the turn scores zero, so anything worth doing must beat this.
const IDLE: f32 = 0.0;

pub struct GreedyBot {
    buffer: Vec<Action>,
    /// Ties are broken at random rather than by enumeration order.
    ///
    /// Order-based tie-breaking is a geographic bias in disguise: the action
    /// list is built row-major, so equal-scoring options resolve towards one
    /// corner of the board. On a symmetric map with identical bots that turned
    /// a mirror match into 15/85, and it would have been baked into any policy
    /// cloned from this teacher.
    rng: awbw_engine::rng::Rng,
    ties: Vec<Action>,
    name: String,
    /// When false the bot never attacks, only expands. A deliberately weaker
    /// rung: it plays the economy correctly and the fight not at all, so a
    /// learner that beats it has worked out combat and nothing else yet.
    fights: bool,
}

impl Default for GreedyBot {
    fn default() -> Self {
        Self::new()
    }
}

impl GreedyBot {
    pub fn new() -> Self {
        GreedyBot {
            buffer: Vec::new(),
            rng: awbw_engine::rng::Rng::new(0x9E37_79B9),
            ties: Vec::new(),
            name: "greedy".to_string(),
            fights: true,
        }
    }

    /// Expands and defends its ground but never initiates combat.
    pub fn capture_only() -> Self {
        GreedyBot {
            buffer: Vec::new(),
            rng: awbw_engine::rng::Rng::new(0x2545_F491),
            ties: Vec::new(),
            name: "capturer".to_string(),
            fights: false,
        }
    }
}

fn unit_value(unit: &Unit) -> f32 {
    unit.typ.stats().cost as f32 * unit.hp100 as f32 / 100.0
}

fn is_capturer(typ: UnitType) -> bool {
    matches!(typ, UnitType::Infantry | UnitType::Mech)
}

/// The nearest thing this unit ought to be walking toward.
///
/// Foot soldiers chase property, everything else chases the enemy. Without this
/// the bot builds an army and leaves it standing on its own doorstep.
fn objective(engine: &Engine, unit: &Unit) -> Option<Pos> {
    let state = &engine.state;
    let me = unit.owner;
    if is_capturer(unit.typ) {
        return state
            .buildings()
            .iter()
            .filter(|b| b.owner != Some(me))
            .map(|b| b.pos)
            .min_by_key(|p| p.distance(unit.pos));
    }
    state
        .units()
        .filter(|u| state.are_enemies(me, u.owner) && u.carried_by.is_none())
        .map(|u| u.pos)
        .min_by_key(|p| p.distance(unit.pos))
}

/// What to build with the funds available.
///
/// Infantry until there are enough of them to take ground, then the most
/// expensive thing affordable, which is a crude but serviceable proxy for the
/// strongest thing affordable.
///
/// Scores sit in their own band below `CAPTURE_STEP`. Production and unit
/// orders come from different tiles, so the bot does both in a turn regardless
/// of their order, but keeping the bands apart stops "build another infantry"
/// from outranking taking the property the infantry is standing on.
fn build_score(engine: &Engine, typ: UnitType, player: PlayerId) -> f32 {
    // The bot has no transport plan, so it declines to fund one.
    if matches!(
        typ,
        UnitType::Apc | UnitType::TCopter | UnitType::Lander | UnitType::BlackBoat
    ) {
        return f32::MIN;
    }

    let state = &engine.state;
    let cost = engine.unit_cost(player, typ) as f32;
    let mut score = 500.0 + 2_500.0 * (cost / 28_000.0).min(1.0);

    let infantry = state
        .units_of(player)
        .filter(|u| is_capturer(u.typ))
        .count();
    let properties = state.property_count(player).max(1) as usize;
    if is_capturer(typ) && infantry < properties {
        // Bodies to take ground come first; without them nothing else matters.
        score += 1_500.0;
    }
    score
}

impl GreedyBot {
    fn score(&self, engine: &Engine, action: Action) -> f32 {
        let state = &engine.state;
        let me = state.current;
        match action {
            Action::EndTurn => IDLE,

            Action::Build { at, typ } => {
                let _ = at;
                build_score(engine, typ, me)
            }

            Action::Attack { unit, dest, target } => {
                if !self.fights {
                    return f32::MIN;
                }
                let (Some(attacker), Some(defender)) = (state.unit(unit), state.unit_at(target))
                else {
                    return f32::MIN;
                };
                let Some(spread) = engine.preview_damage(unit, target) else {
                    return f32::MIN;
                };
                // Value the health actually removed, not the health rolled for.
                let dealt = (spread.expected as f32).min(defender.hp100 as f32);
                let mut score = dealt / 100.0 * defender.typ.stats().cost as f32;
                if defender.hp100 as f32 <= spread.min as f32 {
                    // A guaranteed kill is worth the whole unit, not a fraction.
                    score += unit_value(defender) * 0.5;
                }
                // Subtract what the counterattack is expected to cost, from the
                // tile we will be standing on and against the health the
                // defender has left after our strike. Indirect fire draws none,
                // which is most of why it is good.
                let Some(defender_id) = state.unit_id_at(target) else {
                    return f32::MIN;
                };
                let surviving = (defender.hp100 as f32 - dealt).max(0.0) as i32;
                if let Some(counter) =
                    engine.preview_counter(defender_id, surviving, unit, dest)
                {
                    let taken = (counter.expected as f32).min(attacker.hp100 as f32);
                    score -= taken / 100.0 * attacker.typ.stats().cost as f32;
                }
                score
            }

            Action::Capture { unit, dest } => {
                let Some(actor) = state.unit(unit) else {
                    return f32::MIN;
                };
                let Some(building) = state.building_at(dest) else {
                    return f32::MIN;
                };
                let rate = state.co_of(me).capture_multiplier_pct;
                let progress = actor.display_hp() as u32 * rate / 100;
                let mut score = CAPTURE_STEP;
                if progress >= building.capture_remaining as u32 {
                    score = CAPTURE_COMPLETE;
                    if building.kind == TerrainKind::Hq {
                        score += HQ_BONUS;
                    }
                }
                score
            }

            Action::Move { unit, dest } => {
                let Some(actor) = state.unit(unit) else {
                    return f32::MIN;
                };
                match objective(engine, actor) {
                    Some(goal) => {
                        let before = actor.pos.distance(goal) as f32;
                        let after = dest.distance(goal) as f32;
                        (before - after) * APPROACH
                    }
                    // Nothing to do: sitting on cover beats sitting in the open.
                    None => state.map.terrain_at(dest).defense() as f32,
                }
            }

            Action::Join { unit, dest } => {
                // Merging is only worth it to rescue a nearly-dead unit.
                let (Some(a), Some(b)) = (state.unit(unit), state.unit_at(dest)) else {
                    return f32::MIN;
                };
                if a.hp100 <= 30 && b.hp100 <= 70 {
                    2_000.0
                } else {
                    f32::MIN
                }
            }

            // No transport plan, so no boarding.
            Action::Load { .. } | Action::Unload { .. } => f32::MIN,

            Action::Supply { unit, dest } => {
                let _ = (unit, dest);
                200.0
            }
        }
    }
}

impl Bot for GreedyBot {
    fn name(&self) -> &str {
        &self.name
    }

    fn reset(&mut self, seed: u64) {
        self.rng = awbw_engine::rng::Rng::new(seed ^ 0x5DEE_CE66);
    }

    fn choose(&mut self, engine: &mut Engine) -> Action {
        let mut orders = std::mem::take(&mut self.buffer);
        let mut ties = std::mem::take(&mut self.ties);
        engine.legal_actions_into(&mut orders);

        // Scores are funds-valued, so anything inside a few funds of the best
        // is the same decision as far as the heuristic is concerned.
        const EPSILON: f32 = 1.0;
        let mut best_score = IDLE;
        ties.clear();
        ties.push(Action::EndTurn);
        for &action in orders.iter() {
            let score = self.score(engine, action);
            if score > best_score + EPSILON {
                best_score = score;
                ties.clear();
                ties.push(action);
            } else if (score - best_score).abs() <= EPSILON && score > IDLE {
                ties.push(action);
            }
        }
        let pick = ties[self.rng.roll_inclusive(ties.len() as u32 - 1) as usize];

        self.buffer = orders;
        self.ties = ties;
        pick
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::symmetric_map;
    use crate::RandomBot;
    use awbw_engine::state::Outcome;

    fn engine() -> Engine {
        let (map, owners) = symmetric_map(13, 13);
        let players = vec![
            awbw_engine::state::Player::new(10_000, 1),
            awbw_engine::state::Player::new(10_000, 2),
        ];
        let state = awbw_engine::state::GameState::new(
            map,
            awbw_engine::state::GameSettings::default(),
            players,
            &owners,
        );
        Engine::new(state, 1)
    }

    #[test]
    fn every_chosen_order_is_legal() {
        let mut e = engine();
        let mut bot = GreedyBot::new();
        for _ in 0..2000 {
            if e.state.outcome() != Outcome::InProgress || e.state.day > 20 {
                break;
            }
            let action = bot.choose(&mut e);
            e.apply(action).expect("greedy must choose legal orders");
        }
    }

    #[test]
    fn it_captures_a_property_it_is_standing_on() {
        let mut e = engine();
        let mut bot = GreedyBot::new();
        // Find a neutral property and put an infantry on it.
        let target = e
            .state
            .buildings()
            .iter()
            .find(|b| b.owner.is_none())
            .map(|b| b.pos)
            .expect("map has neutral property");
        e.state.spawn(awbw_engine::types::UnitType::Infantry, 0, target);
        e.refresh_vision();
        let action = bot.choose(&mut e);
        assert!(
            matches!(action, Action::Capture { dest, .. } if dest == target),
            "expected a capture, got {action:?}"
        );
    }

    #[test]
    fn the_counterattack_penalty_actually_applies() {
        // Regression: the penalty used to look the defender up on the tile the
        // attacker was moving to, which is empty at scoring time, so it was
        // silently skipped for every move-then-attack -- which is nearly all of
        // them. A move-in trade must score below the same trade taken for free.
        let mut e = engine();
        let mine = e.state.spawn(UnitType::Tank, 0, Pos::new(6, 6));
        e.state.spawn(UnitType::Tank, 1, Pos::new(8, 6));
        e.refresh_vision();
        let bot = GreedyBot::new();

        let closing = Action::Attack {
            unit: mine,
            dest: Pos::new(7, 6),
            target: Pos::new(8, 6),
        };
        assert!(e.check(closing).is_ok());
        let with_counter = bot.score(&e, closing);

        // Score the same exchange with the counter suppressed, by asking the
        // engine directly, and confirm the bot really subtracted something.
        let defender_id = e.state.unit_id_at(Pos::new(8, 6)).unwrap();
        let counter = e
            .preview_counter(defender_id, 100, mine, Pos::new(7, 6))
            .expect("two adjacent tanks counter each other");
        assert!(counter.expected > 0.0, "counter should do real damage");
        let gross = e.preview_damage(mine, Pos::new(8, 6)).unwrap();
        let gross_value = gross.expected as f32 / 100.0 * UnitType::Tank.stats().cost as f32;
        assert!(
            with_counter < gross_value,
            "score {with_counter} should be below the gross {gross_value}"
        );
    }

    #[test]
    fn it_beats_random_convincingly() {
        let mut greedy = GreedyBot::new();
        let mut random = RandomBot::new(9);
        let result = crate::arena::play_match(&mut greedy, &mut random, 7, 30);
        assert!(
            result.wins_a >= 6,
            "greedy should dominate random, got {result:?}"
        );
    }
}
