//! Fog of war: what one army can see.
//!
//! A tile is lit if some friendly unit or property is within sight of it.
//! Sight is Manhattan distance, extended by the terrain the watcher stands on
//! (mountains add three). Two grades of sight matter:
//!
//! - **Piercing** — the tile is adjacent to a watcher (distance 0 or 1). Cover
//!   and concealment do not help there; you see whatever stands on it.
//! - **Plain** — the tile is lit but further off. A surface unit sitting in
//!   woods or a reef stays hidden, as does a dived sub or a hidden stealth.
//!
//! Aircraft are an exception in the other direction: they fly above cover, so a
//! plane over woods is visible at any range.
//!
//! Modelled after DefendPeace's `MapPerspective::revealFog`, whose fog rules
//! follow AWBW's.

use crate::map::Pos;
use crate::state::{GameState, PlayerId, UnitId, Unit};
use crate::types::MoveType;

/// One army's view of the board. Reusable across turns: call
/// [`Vision::compute`] again to refill it.
#[derive(Debug, Clone, Default)]
pub struct Vision {
    lit: Vec<bool>,
    piercing: Vec<bool>,
    player: PlayerId,
}

impl Vision {
    pub fn new() -> Self {
        Vision::default()
    }

    /// The army this view belongs to.
    #[inline]
    pub fn player(&self) -> PlayerId {
        self.player
    }

    /// Fills this view with everything `player` and their allies can see.
    pub fn compute(&mut self, state: &GameState, player: PlayerId) {
        let map = &state.map;
        let tiles = map.tile_count();
        self.lit.clear();
        self.lit.resize(tiles, false);
        self.piercing.clear();
        self.piercing.resize(tiles, false);
        self.player = player;

        // Without fog the whole board is in plain sight.
        if !state.settings.fog {
            self.lit.iter_mut().for_each(|v| *v = true);
            self.piercing.iter_mut().for_each(|v| *v = true);
            return;
        }

        for unit in state.units() {
            if unit.carried_by.is_some() || !state.are_allied(player, unit.owner) {
                continue;
            }
            let boost = if unit.move_type() == MoveType::Air {
                0
            } else {
                map.terrain_at(unit.pos).vision_boost()
            };
            let range = unit.typ.stats().vision as u32 + boost as u32;
            self.reveal_around(state, unit.pos, range);
        }

        // Your properties watch their own tile.
        for building in state.buildings() {
            if building.owner.is_some_and(|o| state.are_allied(player, o)) {
                let index = map.index(building.pos);
                self.lit[index] = true;
                self.piercing[index] = true;
            }
        }
    }

    fn reveal_around(&mut self, state: &GameState, from: Pos, range: u32) {
        let map = &state.map;
        let range = range as i32;
        for dy in -range..=range {
            let span = range - dy.abs();
            for dx in -span..=span {
                let (x, y) = (from.x as i32 + dx, from.y as i32 + dy);
                if !map.contains(x, y) {
                    continue;
                }
                let index = map.index(Pos::new(x as u8, y as u8));
                self.lit[index] = true;
                if dx.abs() + dy.abs() <= 1 {
                    self.piercing[index] = true;
                }
            }
        }
    }

    /// Whether the tile itself is lit. Terrain is always revealed when lit,
    /// even if a unit standing there is not.
    #[inline]
    pub fn sees_tile(&self, state: &GameState, pos: Pos) -> bool {
        self.lit[state.map.index(pos)]
    }

    /// Whether the tile is close enough that cover and concealment fail.
    #[inline]
    pub fn pierces_tile(&self, state: &GameState, pos: Pos) -> bool {
        self.piercing[state.map.index(pos)]
    }

    /// Whether this army can see a particular unit.
    pub fn sees_unit(&self, state: &GameState, unit: &Unit) -> bool {
        // You always know where your own army is, transports included.
        if state.are_allied(self.player, unit.owner) {
            return true;
        }
        if unit.carried_by.is_some() {
            return false;
        }
        let index = state.map.index(unit.pos);
        if !self.lit[index] {
            return false;
        }
        if self.piercing[index] {
            return true;
        }
        // Beyond arm's reach, concealment holds.
        if unit.hidden {
            return false;
        }
        let terrain = state.map.terrain_at(unit.pos);
        // Aircraft fly over cover rather than into it.
        !terrain.provides_cover() || unit.move_type() == MoveType::Air
    }

    /// The unit standing on a tile, if this army can see it.
    pub fn unit_at(&self, state: &GameState, pos: Pos) -> Option<UnitId> {
        let id = state.unit_id_at(pos)?;
        let unit = state.unit(id)?;
        self.sees_unit(state, unit).then_some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;
    use crate::state::{GameSettings, GameState, Player};
    use crate::types::{TerrainKind, UnitType};
    use std::sync::Arc;

    fn fog_state(kinds: Vec<TerrainKind>, w: u8, h: u8) -> GameState {
        let map = Arc::new(Map::from_kinds(w, h, kinds).unwrap());
        let players = vec![Player::new(0, 1), Player::new(0, 2)];
        let settings = GameSettings { fog: true, ..GameSettings::default() };
        GameState::new(map, settings, players, &[])
    }

    fn plains(w: u8, h: u8) -> GameState {
        fog_state(vec![TerrainKind::Plain; w as usize * h as usize], w, h)
    }

    #[test]
    fn without_fog_everything_is_visible() {
        let map = Arc::new(Map::from_kinds(5, 5, vec![TerrainKind::Wood; 25]).unwrap());
        let players = vec![Player::new(0, 1), Player::new(0, 2)];
        let mut state = GameState::new(map, GameSettings::default(), players, &[]);
        let enemy = state.spawn(UnitType::Infantry, 1, Pos::new(4, 4));
        let mut vision = Vision::new();
        vision.compute(&state, 0);
        assert!(vision.sees_tile(&state, Pos::new(4, 4)));
        assert!(vision.sees_unit(&state, state.unit(enemy).unwrap()));
    }

    #[test]
    fn sight_is_manhattan_and_bounded_by_the_vision_stat() {
        let mut state = plains(9, 9);
        // Infantry sees 2 tiles.
        state.spawn(UnitType::Infantry, 0, Pos::new(4, 4));
        let mut vision = Vision::new();
        vision.compute(&state, 0);
        assert!(vision.sees_tile(&state, Pos::new(4, 6)));
        assert!(vision.sees_tile(&state, Pos::new(5, 5)));
        assert!(!vision.sees_tile(&state, Pos::new(4, 7)));
        assert!(!vision.sees_tile(&state, Pos::new(6, 6)));
    }

    #[test]
    fn mountains_extend_sight_for_ground_units() {
        let mut kinds = vec![TerrainKind::Plain; 81];
        kinds[4 * 9 + 4] = TerrainKind::Mountain;
        let mut state = fog_state(kinds, 9, 9);
        state.spawn(UnitType::Infantry, 0, Pos::new(4, 4));
        let mut vision = Vision::new();
        vision.compute(&state, 0);
        // 2 base + 3 from the mountain.
        assert!(vision.sees_tile(&state, Pos::new(4, 8)));
        // Range 5 covers the whole of its own column, but not the far corner.
        assert!(!vision.sees_tile(&state, Pos::new(0, 0)));
    }

    #[test]
    fn aircraft_gain_nothing_from_the_ground_below() {
        let mut kinds = vec![TerrainKind::Plain; 81];
        kinds[4 * 9 + 4] = TerrainKind::Mountain;
        let mut state = fog_state(kinds, 9, 9);
        // B-Copter sees 3, and must not pick up the mountain's boost.
        state.spawn(UnitType::BCopter, 0, Pos::new(4, 4));
        let mut vision = Vision::new();
        vision.compute(&state, 0);
        assert!(vision.sees_tile(&state, Pos::new(4, 7)));
        assert!(!vision.sees_tile(&state, Pos::new(4, 8)));
    }

    #[test]
    fn woods_hide_ground_units_until_you_stand_beside_them() {
        let mut kinds = vec![TerrainKind::Plain; 25];
        kinds[2 * 5 + 3] = TerrainKind::Wood;
        let mut state = fog_state(kinds, 5, 5);
        let scout = state.spawn(UnitType::Infantry, 0, Pos::new(1, 2));
        let hider = state.spawn(UnitType::Infantry, 1, Pos::new(3, 2));

        let mut vision = Vision::new();
        vision.compute(&state, 0);
        // The tile is lit, but the unit in the woods is not.
        assert!(vision.sees_tile(&state, Pos::new(3, 2)));
        assert!(!vision.sees_unit(&state, state.unit(hider).unwrap()));
        assert_eq!(vision.unit_at(&state, Pos::new(3, 2)), None);

        // Step adjacent and cover stops helping.
        state.relocate(scout, Pos::new(2, 2));
        vision.compute(&state, 0);
        assert!(vision.sees_unit(&state, state.unit(hider).unwrap()));
        assert_eq!(vision.unit_at(&state, Pos::new(3, 2)), Some(hider));
    }

    #[test]
    fn aircraft_over_woods_stay_visible() {
        let mut kinds = vec![TerrainKind::Plain; 25];
        kinds[2 * 5 + 3] = TerrainKind::Wood;
        let mut state = fog_state(kinds, 5, 5);
        state.spawn(UnitType::Infantry, 0, Pos::new(1, 2));
        let plane = state.spawn(UnitType::BCopter, 1, Pos::new(3, 2));
        let mut vision = Vision::new();
        vision.compute(&state, 0);
        assert!(vision.sees_unit(&state, state.unit(plane).unwrap()));
    }

    #[test]
    fn dived_subs_are_invisible_until_adjacent() {
        let mut state = fog_state(vec![TerrainKind::Sea; 25], 5, 5);
        let scout = state.spawn(UnitType::Cruiser, 0, Pos::new(1, 2));
        let sub = state.spawn(UnitType::Sub, 1, Pos::new(3, 2));
        state.unit_mut(sub).unwrap().hidden = true;

        let mut vision = Vision::new();
        vision.compute(&state, 0);
        assert!(vision.sees_tile(&state, Pos::new(3, 2)));
        assert!(!vision.sees_unit(&state, state.unit(sub).unwrap()));

        state.relocate(scout, Pos::new(2, 2));
        vision.compute(&state, 0);
        assert!(vision.sees_unit(&state, state.unit(sub).unwrap()));
    }

    #[test]
    fn you_always_see_your_own_army_and_never_cargo() {
        let mut state = plains(9, 9);
        let far = state.spawn(UnitType::Infantry, 0, Pos::new(8, 8));
        let apc = state.spawn(UnitType::Apc, 1, Pos::new(0, 0));
        let rider = state.spawn(UnitType::Infantry, 1, Pos::new(1, 0));
        state.load_into(rider, apc);

        let mut vision = Vision::new();
        vision.compute(&state, 0);
        assert!(vision.sees_unit(&state, state.unit(far).unwrap()));
        // Even standing on the transport, you cannot see who is inside.
        state.relocate(state.unit(far).unwrap().id, Pos::new(1, 0));
        vision.compute(&state, 0);
        assert!(vision.sees_unit(&state, state.unit(apc).unwrap()));
        assert!(!vision.sees_unit(&state, state.unit(rider).unwrap()));
    }

    #[test]
    fn owned_properties_watch_their_own_tile() {
        let mut kinds = vec![TerrainKind::Plain; 25];
        kinds[4 * 5 + 4] = TerrainKind::City;
        let map = Arc::new(Map::from_kinds(5, 5, kinds).unwrap());
        let players = vec![Player::new(0, 1), Player::new(0, 2)];
        let settings = GameSettings { fog: true, ..GameSettings::default() };
        let mut state = GameState::new(map, settings, players, &[Some(0)]);

        let mut vision = Vision::new();
        vision.compute(&state, 0);
        assert!(vision.sees_tile(&state, Pos::new(4, 4)));
        assert!(!vision.sees_tile(&state, Pos::new(0, 0)));

        // An enemy standing on it is in plain view.
        let enemy = state.spawn(UnitType::Infantry, 1, Pos::new(4, 4));
        vision.compute(&state, 0);
        assert!(vision.sees_unit(&state, state.unit(enemy).unwrap()));
    }
}
