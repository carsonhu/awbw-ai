//! Turning games into tensors, and tensors back into orders.
//!
//! **Observations** are written from the moving player's point of view, so a
//! policy never learns "seat 0" and "seat 1" separately: ownership channels are
//! relative (mine / theirs), not absolute. Under fog only what that player can
//! see is written, which means the observation is exactly the information the
//! agent is entitled to act on.
//!
//! **Actions** are factorized into four spatial choices rather than one flat
//! index, because the flat product of (unit, destination, order, target) is
//! enormous and mostly illegal:
//!
//! ```text
//!   source  -> which tile acts (a unit, or a production property), or end turn
//!   dest    -> where it ends up
//!   kind    -> what it does there
//!   param   -> whom it shoots / what it builds / which passenger it drops
//! ```
//!
//! Three of the four are maps over the board, which is what a convolutional
//! policy naturally emits. Each is masked by the one before it, so every path
//! through the four heads lands on a legal order.

use crate::actions::{Action, Engine};
use crate::map::Pos;
use crate::state::{GameState, PlayerId, MAX_CARGO};
use crate::types::{TerrainKind, UnitType, Weather};
use crate::vision::Vision;

// --- observation ----------------------------------------------------------

/// Terrain kinds, unit types, and the handful of scalar channels.
pub const TERRAIN_PLANES: usize = 22;
pub const UNIT_TYPE_PLANES: usize = 25;

/// Channel layout, in order. Anything spatial lives here.
pub mod plane {
    use super::{TERRAIN_PLANES, UNIT_TYPE_PLANES};

    pub const TERRAIN: usize = 0;
    pub const TERRAIN_DEFENSE: usize = TERRAIN + TERRAIN_PLANES;
    pub const IS_PROPERTY: usize = TERRAIN_DEFENSE + 1;
    pub const PROPERTY_MINE: usize = IS_PROPERTY + 1;
    pub const PROPERTY_THEIRS: usize = PROPERTY_MINE + 1;
    pub const PROPERTY_NEUTRAL: usize = PROPERTY_THEIRS + 1;
    pub const CAPTURE_LEFT: usize = PROPERTY_NEUTRAL + 1;
    pub const UNIT_MINE: usize = CAPTURE_LEFT + 1;
    pub const UNIT_THEIRS: usize = UNIT_MINE + 1;
    /// Signed: +1 on my unit of that type, -1 on theirs.
    pub const UNIT_TYPE: usize = UNIT_THEIRS + 1;
    pub const UNIT_HP: usize = UNIT_TYPE + UNIT_TYPE_PLANES;
    pub const UNIT_FUEL: usize = UNIT_HP + 1;
    pub const UNIT_AMMO: usize = UNIT_FUEL + 1;
    pub const UNIT_MOVED: usize = UNIT_AMMO + 1;
    pub const UNIT_HIDDEN: usize = UNIT_MOVED + 1;
    pub const UNIT_CARGO: usize = UNIT_HIDDEN + 1;
    /// Whether this tile is visible at all. Always 1 without fog.
    pub const LIT: usize = UNIT_CARGO + 1;
    pub const COUNT: usize = LIT + 1;
}

/// Non-spatial features: funds, day, weather, and army sizes.
pub const GLOBAL_FEATURES: usize = 11;

/// Floats one observation needs for a given board.
pub fn observation_len(state: &GameState) -> usize {
    plane::COUNT * state.map.tile_count() + GLOBAL_FEATURES
}

/// Writes the moving player's view of `state` into `out`, which must be
/// [`observation_len`] long. Planes come first, channel-major, then globals.
pub fn encode_observation(state: &GameState, vision: &Vision, out: &mut [f32]) {
    let tiles = state.map.tile_count();
    assert_eq!(out.len(), observation_len(state), "observation buffer size");
    out.fill(0.0);

    let me = state.current;
    let (planes, globals) = out.split_at_mut(plane::COUNT * tiles);
    let mut at = |channel: usize, index: usize, value: f32| {
        planes[channel * tiles + index] = value;
    };

    for index in 0..tiles {
        let pos = state.map.pos_of(index);
        let terrain = state.map.terrain_at(pos);
        at(plane::TERRAIN + terrain as usize, index, 1.0);
        at(plane::TERRAIN_DEFENSE, index, terrain.defense() as f32 / 4.0);

        let lit = vision.sees_tile(state, pos);
        at(plane::LIT, index, if lit { 1.0 } else { 0.0 });

        // Terrain and property ownership are public knowledge; only units hide.
        if let Some(building) = state.building_at(pos) {
            at(plane::IS_PROPERTY, index, 1.0);
            match building.owner {
                Some(owner) if state.are_allied(me, owner) => {
                    at(plane::PROPERTY_MINE, index, 1.0)
                }
                Some(_) => at(plane::PROPERTY_THEIRS, index, 1.0),
                None => at(plane::PROPERTY_NEUTRAL, index, 1.0),
            }
            at(
                plane::CAPTURE_LEFT,
                index,
                building.capture_remaining as f32 / 20.0,
            );
        }

        let Some(unit) = state.unit_at(pos) else {
            continue;
        };
        if !vision.sees_unit(state, unit) {
            continue;
        }
        let mine = state.are_allied(me, unit.owner);
        let stats = unit.typ.stats();
        at(
            if mine { plane::UNIT_MINE } else { plane::UNIT_THEIRS },
            index,
            1.0,
        );
        at(
            plane::UNIT_TYPE + unit.typ as usize,
            index,
            if mine { 1.0 } else { -1.0 },
        );
        at(plane::UNIT_HP, index, unit.hp100 as f32 / 100.0);
        at(
            plane::UNIT_FUEL,
            index,
            unit.fuel as f32 / stats.max_fuel.max(1) as f32,
        );
        at(
            plane::UNIT_AMMO,
            index,
            unit.ammo as f32 / stats.max_ammo.max(1) as f32,
        );
        at(plane::UNIT_MOVED, index, if unit.moved { 1.0 } else { 0.0 });
        at(plane::UNIT_HIDDEN, index, if unit.hidden { 1.0 } else { 0.0 });
        at(
            plane::UNIT_CARGO,
            index,
            unit.cargo_len() as f32 / MAX_CARGO as f32,
        );
    }

    let them: PlayerId = (0..state.players.len() as PlayerId)
        .find(|&p| state.are_enemies(me, p))
        .unwrap_or(me);
    globals[0] = state.players[me as usize].funds as f32 / 10_000.0;
    // Enemy funds are hidden information; the harness may still want the slot.
    globals[1] = state.players[them as usize].funds as f32 / 10_000.0;
    globals[2] = state.day as f32 / 30.0;
    globals[3 + state.weather as usize] = 1.0;
    globals[6] = state.property_count(me) as f32 / 20.0;
    globals[7] = state.property_count(them) as f32 / 20.0;
    globals[8] = state.unit_count(me) as f32 / 50.0;
    globals[9] = state.unit_count(them) as f32 / 50.0;
    globals[10] = if state.settings.fog { 1.0 } else { 0.0 };
}

// --- actions --------------------------------------------------------------

/// What an order does once the unit has moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OrderKind {
    /// Move there and stop.
    Wait = 0,
    Attack = 1,
    Capture = 2,
    Supply = 3,
    Join = 4,
    Load = 5,
    Unload = 6,
    Build = 7,
}

pub const ORDER_KINDS: usize = 8;

impl OrderKind {
    pub fn from_index(i: usize) -> Option<OrderKind> {
        Some(match i {
            0 => OrderKind::Wait,
            1 => OrderKind::Attack,
            2 => OrderKind::Capture,
            3 => OrderKind::Supply,
            4 => OrderKind::Join,
            5 => OrderKind::Load,
            6 => OrderKind::Unload,
            7 => OrderKind::Build,
            _ => return None,
        })
    }
}

/// One order, as four head selections.
///
/// `source` indexes a tile, except for the single extra index equal to the tile
/// count, which means "end the turn".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionCode {
    pub source: u32,
    pub dest: u32,
    pub kind: u8,
    /// Attack: the target tile. Build: the unit type. Unload: cargo slot times
    /// four plus the drop direction. Otherwise zero.
    pub param: u32,
}

/// Directions in the same order as `Map::neighbors`.
const DIRECTIONS: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];

fn direction_of(from: Pos, to: Pos) -> Option<u32> {
    let (dx, dy) = (to.x as i32 - from.x as i32, to.y as i32 - from.y as i32);
    DIRECTIONS
        .iter()
        .position(|&d| d == (dx, dy))
        .map(|i| i as u32)
}

fn offset(state: &GameState, from: Pos, direction: u32) -> Option<Pos> {
    let (dx, dy) = *DIRECTIONS.get(direction as usize)?;
    let (x, y) = (from.x as i32 + dx, from.y as i32 + dy);
    state
        .map
        .contains(x, y)
        .then(|| Pos::new(x as u8, y as u8))
}

/// The `source` value meaning "end the turn".
pub fn end_turn_source(state: &GameState) -> u32 {
    state.map.tile_count() as u32
}

/// Number of logits each head needs.
pub fn head_sizes(state: &GameState) -> [usize; 4] {
    let tiles = state.map.tile_count();
    [
        tiles + 1,
        tiles,
        ORDER_KINDS,
        tiles.max(UNIT_TYPE_PLANES).max(MAX_CARGO * 4),
    ]
}

/// Encodes an order. Returns `None` for an action whose pieces are not on the
/// board any more.
pub fn encode(state: &GameState, action: Action) -> Option<ActionCode> {
    let tile = |pos: Pos| state.map.index(pos) as u32;
    let of = |unit| state.unit(unit).map(|u| tile(u.pos));

    Some(match action {
        Action::EndTurn => ActionCode {
            source: end_turn_source(state),
            dest: 0,
            kind: OrderKind::Wait as u8,
            param: 0,
        },
        Action::Build { at, typ } => ActionCode {
            source: tile(at),
            dest: tile(at),
            kind: OrderKind::Build as u8,
            param: typ as u32,
        },
        Action::Move { unit, dest } => ActionCode {
            source: of(unit)?,
            dest: tile(dest),
            kind: OrderKind::Wait as u8,
            param: 0,
        },
        Action::Attack { unit, dest, target } => ActionCode {
            source: of(unit)?,
            dest: tile(dest),
            kind: OrderKind::Attack as u8,
            param: tile(target),
        },
        Action::Capture { unit, dest } => ActionCode {
            source: of(unit)?,
            dest: tile(dest),
            kind: OrderKind::Capture as u8,
            param: 0,
        },
        Action::Supply { unit, dest } => ActionCode {
            source: of(unit)?,
            dest: tile(dest),
            kind: OrderKind::Supply as u8,
            param: 0,
        },
        Action::Join { unit, dest } => ActionCode {
            source: of(unit)?,
            dest: tile(dest),
            kind: OrderKind::Join as u8,
            param: 0,
        },
        Action::Load { unit, dest } => ActionCode {
            source: of(unit)?,
            dest: tile(dest),
            kind: OrderKind::Load as u8,
            param: 0,
        },
        Action::Unload {
            transport,
            dest,
            cargo,
            drop_at,
        } => {
            let slot = state
                .unit(transport)?
                .cargo
                .iter()
                .position(|&c| c == cargo)? as u32;
            let direction = direction_of(dest, drop_at)?;
            ActionCode {
                source: of(transport)?,
                dest: tile(dest),
                kind: OrderKind::Unload as u8,
                param: slot * 4 + direction,
            }
        }
    })
}

/// Turns head selections back into an order. Returns `None` when the code does
/// not describe anything the board can do; it does not check legality, which is
/// [`Engine::check`]'s job.
pub fn decode(state: &GameState, code: ActionCode) -> Option<Action> {
    if code.source == end_turn_source(state) {
        return Some(Action::EndTurn);
    }
    let tiles = state.map.tile_count() as u32;
    if code.source >= tiles || code.dest >= tiles {
        return None;
    }
    let source = state.map.pos_of(code.source as usize);
    let dest = state.map.pos_of(code.dest as usize);
    let kind = OrderKind::from_index(code.kind as usize)?;

    if kind == OrderKind::Build {
        let typ = *UnitType::ALL.get(code.param as usize)?;
        return Some(Action::Build { at: source, typ });
    }

    let unit = state.unit_id_at(source)?;
    Some(match kind {
        OrderKind::Wait => Action::Move { unit, dest },
        OrderKind::Attack => {
            if code.param >= tiles {
                return None;
            }
            Action::Attack {
                unit,
                dest,
                target: state.map.pos_of(code.param as usize),
            }
        }
        OrderKind::Capture => Action::Capture { unit, dest },
        OrderKind::Supply => Action::Supply { unit, dest },
        OrderKind::Join => Action::Join { unit, dest },
        OrderKind::Load => Action::Load { unit, dest },
        OrderKind::Unload => {
            let slot = (code.param / 4) as usize;
            let cargo = *state.unit(unit)?.cargo.get(slot)?;
            let drop_at = offset(state, dest, code.param % 4)?;
            Action::Unload {
                transport: unit,
                dest,
                cargo,
                drop_at,
            }
        }
        OrderKind::Build => unreachable!("handled above"),
    })
}

/// Legality masks for the four heads.
///
/// Staged rather than eager. The source mask needs only to know which tiles can
/// act, which is a scan of the moving player's units and properties; the deeper
/// masks need one tile's orders. Enumerating the whole action set to build them
/// would throw away the factorized action space's entire advantage.
///
/// Masks are still derived by encoding real orders, so they agree with the
/// engine by construction rather than by a second implementation of the rules.
#[derive(Debug, Default)]
pub struct ActionMasks {
    orders: Vec<Action>,
    codes: Vec<ActionCode>,
    cached: Option<u32>,
}

impl ActionMasks {
    pub fn new() -> Self {
        ActionMasks::default()
    }

    /// Which tiles can act, plus the end-turn index.
    pub fn source_mask(&mut self, engine: &mut Engine, out: &mut Vec<bool>) {
        self.cached = None;
        let tiles = engine.state.map.tile_count();
        out.clear();
        out.resize(tiles + 1, false);

        for unit in engine.movable_units() {
            if let Some(u) = engine.state.unit(unit) {
                out[engine.state.map.index(u.pos)] = true;
            }
        }
        let sites: Vec<Pos> = engine
            .state
            .buildings_of(engine.state.current)
            .filter(|b| {
                matches!(
                    b.kind,
                    TerrainKind::Base | TerrainKind::Airport | TerrainKind::Port
                )
            })
            .map(|b| b.pos)
            .collect();
        for at in sites {
            if engine.can_build_anything(at) {
                out[engine.state.map.index(at)] = true;
            }
        }
        // Ending the turn is always available, so no mask is ever empty.
        out[tiles] = true;
    }

    /// Caches the orders available from one tile. Later masks filter this.
    pub fn select_source(&mut self, engine: &mut Engine, source: u32) -> &[ActionCode] {
        if self.cached != Some(source) {
            self.codes.clear();
            if source == end_turn_source(&engine.state) {
                if let Some(code) = encode(&engine.state, Action::EndTurn) {
                    self.codes.push(code);
                }
            } else if (source as usize) < engine.state.map.tile_count() {
                let at = engine.state.map.pos_of(source as usize);
                let mut orders = std::mem::take(&mut self.orders);
                engine.legal_actions_at(at, &mut orders);
                let state = &engine.state;
                self.codes
                    .extend(orders.iter().filter_map(|&a| encode(state, a)));
                self.orders = orders;
            }
            self.cached = Some(source);
        }
        &self.codes
    }

    /// The orders cached by the last [`ActionMasks::select_source`].
    pub fn codes(&self) -> &[ActionCode] {
        &self.codes
    }

    fn fill(out: &mut Vec<bool>, len: usize, values: impl Iterator<Item = usize>) {
        out.clear();
        out.resize(len, false);
        for v in values {
            if v < len {
                out[v] = true;
            }
        }
    }

    pub fn dest_mask(&mut self, engine: &mut Engine, source: u32, out: &mut Vec<bool>) {
        let len = engine.state.map.tile_count();
        self.select_source(engine, source);
        Self::fill(out, len, self.codes.iter().map(|c| c.dest as usize));
    }

    pub fn kind_mask(&mut self, engine: &mut Engine, source: u32, dest: u32, out: &mut Vec<bool>) {
        self.select_source(engine, source);
        Self::fill(
            out,
            ORDER_KINDS,
            self.codes
                .iter()
                .filter(|c| c.dest == dest)
                .map(|c| c.kind as usize),
        );
    }

    pub fn param_mask(
        &mut self,
        engine: &mut Engine,
        source: u32,
        dest: u32,
        kind: u8,
        out: &mut Vec<bool>,
    ) {
        let len = head_sizes(&engine.state)[3];
        self.select_source(engine, source);
        Self::fill(
            out,
            len,
            self.codes
                .iter()
                .filter(|c| c.dest == dest && c.kind == kind)
                .map(|c| c.param as usize),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Map;
    use crate::rng::Rng;
    use crate::state::{GameSettings, GameState, Outcome, Player};
    use std::sync::Arc;

    fn board(fog: bool) -> Engine {
        let mut kinds = vec![TerrainKind::Plain; 49];
        kinds[0] = TerrainKind::Base;
        kinds[24] = TerrainKind::City;
        kinds[48] = TerrainKind::Base;
        kinds[10] = TerrainKind::Wood;
        kinds[30] = TerrainKind::Mountain;
        let map = Arc::new(Map::from_kinds(7, 7, kinds).unwrap());
        let players = vec![Player::new(20_000, 1), Player::new(20_000, 2)];
        let settings = GameSettings { fog, ..GameSettings::default() };
        let state = GameState::new(map, settings, players, &[Some(0), None, Some(1)]);
        let mut e = Engine::new(state, 3);
        e.state.spawn(UnitType::Infantry, 0, Pos::new(2, 2));
        e.state.spawn(UnitType::Artillery, 0, Pos::new(1, 1));
        let apc = e.state.spawn(UnitType::Apc, 0, Pos::new(3, 1));
        let rider = e.state.spawn(UnitType::Mech, 0, Pos::new(4, 1));
        e.state.load_into(rider, apc);
        e.state.spawn(UnitType::Tank, 1, Pos::new(3, 3));
        e.refresh_vision();
        e
    }

    #[test]
    fn every_legal_order_round_trips() {
        let mut e = board(false);
        for action in e.legal_actions() {
            let code = encode(&e.state, action).expect("encodable");
            let back = decode(&e.state, code).expect("decodable");
            assert_eq!(back, action, "round trip failed for {action:?} via {code:?}");
        }
    }

    #[test]
    fn round_trips_hold_across_a_whole_random_game() {
        let mut e = board(false);
        let mut rng = Rng::new(5);
        for _ in 0..600 {
            if e.state.outcome() != Outcome::InProgress {
                break;
            }
            let actions = e.legal_actions();
            for &action in &actions {
                let code = encode(&e.state, action).expect("encodable");
                assert_eq!(decode(&e.state, code), Some(action));
            }
            let pick = actions[rng.roll_inclusive(actions.len() as u32 - 1) as usize];
            e.apply(pick).unwrap();
        }
    }

    #[test]
    fn masks_lead_only_to_legal_orders() {
        let mut e = board(false);
        let mut masks = ActionMasks::new();

        let mut sources = Vec::new();
        let mut dests = Vec::new();
        let mut kinds = Vec::new();
        let mut params = Vec::new();
        masks.source_mask(&mut e, &mut sources);
        assert!(sources.iter().any(|&b| b));
        assert!(sources[end_turn_source(&e.state) as usize], "ending the turn is always legal");

        let mut checked = 0;
        for (source, &ok) in sources.iter().enumerate() {
            if !ok || source as u32 == end_turn_source(&e.state) {
                continue;
            }
            let source = source as u32;
            masks.dest_mask(&mut e, source, &mut dests);
            for (dest, &ok) in dests.iter().enumerate() {
                if !ok {
                    continue;
                }
                let dest = dest as u32;
                masks.kind_mask(&mut e, source, dest, &mut kinds);
                for (kind, &ok) in kinds.iter().enumerate() {
                    if !ok {
                        continue;
                    }
                    masks.param_mask(&mut e, source, dest, kind as u8, &mut params);
                    for (param, &ok) in params.iter().enumerate() {
                        if !ok {
                            continue;
                        }
                        let code = ActionCode { source, dest, kind: kind as u8, param: param as u32 };
                        let action = decode(&e.state, code).expect("mask implies decodable");
                        assert!(e.check(action).is_ok(), "mask allowed illegal {action:?}");
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 20, "expected a decent spread of orders, got {checked}");
    }

    #[test]
    fn staged_masks_cover_exactly_the_legal_orders() {
        // Walking the heads must reach every order the flat enumeration finds,
        // and no others -- the staged path is an optimisation, not a different
        // rule set.
        let mut e = board(false);
        let mut flat: Vec<ActionCode> = {
            let legal = e.legal_actions();
            let state = &e.state;
            legal.iter().filter_map(|&a| encode(state, a)).collect()
        };

        let mut staged = Vec::new();
        let mut masks = ActionMasks::new();
        let mut sources = Vec::new();
        masks.source_mask(&mut e, &mut sources);
        for (source, &ok) in sources.iter().enumerate() {
            if ok {
                staged.extend_from_slice(masks.select_source(&mut e, source as u32));
            }
        }

        let key = |c: &ActionCode| (c.source, c.dest, c.kind, c.param);
        flat.sort_by_key(key);
        staged.sort_by_key(key);
        assert_eq!(staged, flat);
    }

    #[test]
    fn observation_is_written_from_the_moving_players_side() {
        let mut e = board(false);
        let mut obs = vec![0.0; observation_len(&e.state)];
        let tiles = e.state.map.tile_count();
        encode_observation(&e.state, e.vision(), &mut obs);

        let mine = e.state.map.index(Pos::new(2, 2));
        let theirs = e.state.map.index(Pos::new(3, 3));
        assert_eq!(obs[plane::UNIT_MINE * tiles + mine], 1.0);
        assert_eq!(obs[plane::UNIT_THEIRS * tiles + theirs], 1.0);
        // The unit-type channel is signed, so ownership survives the encoding.
        let infantry = plane::UNIT_TYPE + UnitType::Infantry as usize;
        let tank = plane::UNIT_TYPE + UnitType::Tank as usize;
        assert_eq!(obs[infantry * tiles + mine], 1.0);
        assert_eq!(obs[tank * tiles + theirs], -1.0);

        // Hand the turn over and the same units swap sides.
        e.apply(Action::EndTurn).unwrap();
        encode_observation(&e.state, e.vision(), &mut obs);
        assert_eq!(obs[plane::UNIT_THEIRS * tiles + mine], 1.0);
        assert_eq!(obs[plane::UNIT_MINE * tiles + theirs], 1.0);
        assert_eq!(obs[infantry * tiles + mine], -1.0);
    }

    #[test]
    fn fog_keeps_hidden_units_out_of_the_observation() {
        let mut e = board(true);
        // Move the enemy well away and behind cover.
        let tank = e.state.unit_id_at(Pos::new(3, 3)).unwrap();
        e.state.relocate(tank, Pos::new(6, 5));
        e.refresh_vision();

        let tiles = e.state.map.tile_count();
        let mut obs = vec![0.0; observation_len(&e.state)];
        encode_observation(&e.state, e.vision(), &mut obs);
        let far = e.state.map.index(Pos::new(6, 5));
        assert_eq!(obs[plane::UNIT_THEIRS * tiles + far], 0.0);
        assert_eq!(obs[plane::LIT * tiles + far], 0.0);

        // Terrain and property ownership stay public even in the dark.
        let hq = e.state.map.index(Pos::new(6, 6));
        assert_eq!(obs[plane::IS_PROPERTY * tiles + hq], 1.0);
    }

    #[test]
    fn observation_length_matches_the_spec() {
        let e = board(false);
        let tiles = e.state.map.tile_count();
        assert_eq!(observation_len(&e.state), plane::COUNT * tiles + GLOBAL_FEATURES);
        assert_eq!(plane::COUNT, 62);
        assert_eq!(head_sizes(&e.state), [tiles + 1, tiles, ORDER_KINDS, tiles]);
    }
}
