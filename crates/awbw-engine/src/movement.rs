//! Movement reachability and pathfinding.
//!
//! A unit may spend the lesser of its movement points and its remaining fuel.
//! Enemy units block movement entirely — you can neither pass through nor stop
//! on them. Allied units can be passed through but not stopped on, except when
//! loading into a transport or joining a damaged unit of the same type.
//!
//! Costs are tiny (1..=4) and budgets small (<= 9), so this uses a bucket queue
//! rather than a binary heap, and reuses its buffers across calls to keep the
//! self-play hot loop allocation-free.

use crate::map::Pos;
use crate::state::{GameState, UnitId, NO_UNIT};
use crate::vision::Vision;

pub const UNREACHABLE: u8 = u8::MAX;
const NO_PREV: u16 = u16::MAX;

/// The set of tiles a unit can reach this turn, with predecessors for path
/// reconstruction. Reusable: call [`Reach::compute`] again to refill it.
#[derive(Debug, Clone, Default)]
pub struct Reach {
    /// Movement points spent to reach each tile; `UNREACHABLE` if it cannot.
    cost: Vec<u8>,
    prev: Vec<u16>,
    /// Bucket queue, indexed by cost.
    buckets: Vec<Vec<u16>>,
    budget: u8,
    origin: Pos,
}

impl Reach {
    pub fn new() -> Self {
        Reach::default()
    }

    #[inline]
    pub fn budget(&self) -> u8 {
        self.budget
    }

    #[inline]
    pub fn origin(&self) -> Pos {
        self.origin
    }

    #[inline]
    pub fn cost_at(&self, index: usize) -> u8 {
        self.cost[index]
    }

    /// Fills this `Reach` with everywhere `unit` can move to this turn,
    /// assuming the whole board is known.
    pub fn compute(&mut self, state: &GameState, unit_id: UnitId) {
        self.compute_inner(state, unit_id, None)
    }

    /// As [`Reach::compute`], but from behind fog.
    ///
    /// An enemy the mover cannot see does not block the route it plans, because
    /// the mover does not know it is there. Walking into one is what springs an
    /// ambush, and that is resolved when the move is actually made.
    pub fn compute_with_vision(&mut self, state: &GameState, unit_id: UnitId, vision: &Vision) {
        self.compute_inner(state, unit_id, Some(vision))
    }

    fn compute_inner(&mut self, state: &GameState, unit_id: UnitId, vision: Option<&Vision>) {
        let map = &state.map;
        let tiles = map.tile_count();
        self.cost.clear();
        self.cost.resize(tiles, UNREACHABLE);
        self.prev.clear();
        self.prev.resize(tiles, NO_PREV);

        let Some(unit) = state.unit(unit_id) else {
            self.budget = 0;
            return;
        };
        // A unit riding in a transport has no movement of its own.
        if unit.carried_by.is_some() {
            self.budget = 0;
            self.origin = unit.pos;
            return;
        }

        let stats = unit.typ.stats();
        // Some COs extend movement for particular units (Sami's transports).
        let move_points = (stats.move_points as i32
            + state.co_of(unit.owner).move_delta[unit.typ as usize] as i32)
            .clamp(0, 255) as u8;
        let budget = move_points.min(unit.fuel);
        let move_type = stats.move_type;
        let weather = state.weather;
        let owner = unit.owner;

        self.budget = budget;
        self.origin = unit.pos;

        // Grow but never shrink: clearing the outer vector would free every
        // bucket's buffer and force a fresh allocation on the next call, which
        // is the whole cost of this routine in a self-play loop.
        if self.buckets.len() < budget as usize + 1 {
            self.buckets.resize(budget as usize + 1, Vec::new());
        }
        for bucket in self.buckets.iter_mut() {
            bucket.clear();
        }

        let start = map.index(unit.pos);
        self.cost[start] = 0;
        self.buckets[0].push(start as u16);

        for spent in 0..=budget as usize {
            // `buckets` is borrowed while we walk it, so drain into a scratch
            // list first; the vector is reused, not reallocated.
            let mut frontier = std::mem::take(&mut self.buckets[spent]);
            let mut i = 0;
            while i < frontier.len() {
                let node = frontier[i] as usize;
                i += 1;
                if self.cost[node] as usize != spent {
                    continue; // stale entry, already improved
                }
                let pos = map.pos_of(node);
                for next in map.neighbors(pos) {
                    let next_index = map.index(next);
                    let Some(step) = map.terrain_at(next).move_cost(weather, move_type) else {
                        continue;
                    };
                    // Enemies block the tile outright -- but only the ones
                    // the mover knows about.
                    let occupant = state.occupancy_at(next_index);
                    if occupant != NO_UNIT {
                        match state.unit(occupant) {
                            Some(other) if state.are_enemies(owner, other.owner) => {
                                let known =
                                    vision.map_or(true, |v| v.sees_unit(state, other));
                                if known {
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    let total = spent + step as usize;
                    if total > budget as usize || total >= self.cost[next_index] as usize {
                        continue;
                    }
                    self.cost[next_index] = total as u8;
                    self.prev[next_index] = node as u16;
                    self.buckets[total].push(next_index as u16);
                }
            }
            frontier.clear();
            self.buckets[spent] = frontier;
        }
    }

    /// Movement points spent reaching `pos`, or `None` if out of range.
    #[inline]
    pub fn cost_to(&self, state: &GameState, pos: Pos) -> Option<u8> {
        match self.cost[state.map.index(pos)] {
            UNREACHABLE => None,
            c => Some(c),
        }
    }

    #[inline]
    pub fn can_reach(&self, state: &GameState, pos: Pos) -> bool {
        self.cost_to(state, pos).is_some()
    }

    /// Every tile within range, including the unit's own tile. Borrows rather
    /// than collecting, so enumerating a turn's moves allocates nothing.
    pub fn reachable<'a>(&'a self, state: &'a GameState) -> impl Iterator<Item = Pos> + 'a {
        self.cost
            .iter()
            .enumerate()
            .filter_map(move |(i, &c)| (c != UNREACHABLE).then(|| state.map.pos_of(i)))
    }

    /// The cheapest path from the origin to `pos`, origin first. Empty when
    /// unreachable.
    pub fn path_to(&self, state: &GameState, pos: Pos) -> Vec<Pos> {
        let mut index = state.map.index(pos);
        if self.cost[index] == UNREACHABLE {
            return Vec::new();
        }
        let mut path = vec![pos];
        while self.prev[index] != NO_PREV {
            index = self.prev[index] as usize;
            path.push(state.map.pos_of(index));
        }
        path.reverse();
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;
    use crate::state::{GameSettings, GameState, Player};
    use crate::types::{TerrainKind, UnitType, Weather};
    use std::sync::Arc;

    fn plain_state(w: u8, h: u8) -> GameState {
        let map = Arc::new(Map::from_kinds(w, h, vec![TerrainKind::Plain; (w as usize) * (h as usize)]).unwrap());
        let players = vec![
            Player::new(0, 1),
            Player::new(0, 2),
        ];
        GameState::new(map, GameSettings::default(), players, &[])
    }

    #[test]
    fn infantry_reaches_three_tiles_on_plains() {
        let mut state = plain_state(9, 1);
        let id = state.spawn(UnitType::Infantry, 0, Pos::new(4, 0));
        let mut reach = Reach::new();
        reach.compute(&state, id);
        assert_eq!(reach.cost_to(&state, Pos::new(7, 0)), Some(3));
        assert_eq!(reach.cost_to(&state, Pos::new(1, 0)), Some(3));
        assert_eq!(reach.cost_to(&state, Pos::new(8, 0)), None);
    }

    #[test]
    fn tires_pay_double_on_plains() {
        let mut state = plain_state(9, 1);
        // Recon has 8 MP but tires cost 2 per plain tile: 4 tiles.
        let id = state.spawn(UnitType::Recon, 0, Pos::new(4, 0));
        let mut reach = Reach::new();
        reach.compute(&state, id);
        assert_eq!(reach.cost_to(&state, Pos::new(8, 0)), Some(8));
        assert_eq!(reach.cost_to(&state, Pos::new(0, 0)), Some(8));
    }

    #[test]
    fn mountains_stop_vehicles_but_not_infantry() {
        let mut kinds = vec![TerrainKind::Plain; 5];
        kinds[2] = TerrainKind::Mountain;
        let map = Arc::new(Map::from_kinds(5, 1, kinds).unwrap());
        let players = vec![
            Player::new(0, 1),
            Player::new(0, 2),
        ];
        let mut state = GameState::new(map, GameSettings::default(), players, &[]);

        let tank = state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        let mut reach = Reach::new();
        reach.compute(&state, tank);
        assert_eq!(reach.cost_to(&state, Pos::new(1, 0)), Some(1));
        assert_eq!(reach.cost_to(&state, Pos::new(3, 0)), None);

        state.destroy(tank);
        let mech = state.spawn(UnitType::Mech, 0, Pos::new(0, 0));
        reach.compute(&state, mech);
        // Mech: 2 MP, mountain costs 1 for Boot.
        assert_eq!(reach.cost_to(&state, Pos::new(2, 0)), Some(2));
    }

    #[test]
    fn enemies_block_and_allies_do_not() {
        let mut state = plain_state(7, 1);
        let mover = state.spawn(UnitType::Infantry, 0, Pos::new(0, 0));
        state.spawn(UnitType::Infantry, 1, Pos::new(2, 0));
        let mut reach = Reach::new();
        reach.compute(&state, mover);
        assert_eq!(reach.cost_to(&state, Pos::new(1, 0)), Some(1));
        assert_eq!(reach.cost_to(&state, Pos::new(2, 0)), None);
        assert_eq!(reach.cost_to(&state, Pos::new(3, 0)), None);

        // Replace the blocker with a friendly unit: passable.
        let blocker = state.unit_id_at(Pos::new(2, 0)).unwrap();
        state.destroy(blocker);
        state.spawn(UnitType::Infantry, 0, Pos::new(2, 0));
        reach.compute(&state, mover);
        assert_eq!(reach.cost_to(&state, Pos::new(3, 0)), Some(3));
    }

    #[test]
    fn fuel_caps_movement() {
        let mut state = plain_state(9, 1);
        let id = state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        state.unit_mut(id).unwrap().fuel = 2;
        let mut reach = Reach::new();
        reach.compute(&state, id);
        assert_eq!(reach.budget(), 2);
        assert_eq!(reach.cost_to(&state, Pos::new(3, 0)), None);
    }

    #[test]
    fn weather_slows_movement() {
        let mut state = plain_state(9, 1);
        let id = state.spawn(UnitType::Infantry, 0, Pos::new(4, 0));
        let mut reach = Reach::new();
        state.weather = Weather::Snow;
        reach.compute(&state, id);
        // Snow makes plains cost 2 for foot: 3 MP reaches one tile.
        assert_eq!(reach.cost_to(&state, Pos::new(5, 0)), Some(2));
        assert_eq!(reach.cost_to(&state, Pos::new(6, 0)), None);
    }

    #[test]
    fn path_follows_the_cheapest_route() {
        let mut state = plain_state(5, 5);
        let id = state.spawn(UnitType::Tank, 0, Pos::new(0, 0));
        let mut reach = Reach::new();
        reach.compute(&state, id);
        let path = reach.path_to(&state, Pos::new(2, 2));
        assert_eq!(path.first(), Some(&Pos::new(0, 0)));
        assert_eq!(path.last(), Some(&Pos::new(2, 2)));
        assert_eq!(path.len(), 5); // four steps
        assert!(reach.path_to(&state, Pos::new(4, 4)).is_empty());
    }

    #[test]
    fn carried_units_cannot_move() {
        let mut state = plain_state(5, 1);
        let apc = state.spawn(UnitType::Apc, 0, Pos::new(0, 0));
        let inf = state.spawn(UnitType::Infantry, 0, Pos::new(1, 0));
        assert!(state.load_into(inf, apc));
        let mut reach = Reach::new();
        reach.compute(&state, inf);
        assert_eq!(reach.budget(), 0);
        assert_eq!(reach.cost_to(&state, Pos::new(1, 0)), None);
    }
}
