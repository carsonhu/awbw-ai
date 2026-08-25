//! Static board geometry and terrain. A map never changes during a game (only
//! property *ownership* does, which lives in `GameState`), so it is shared
//! behind an `Arc` and cloning a state never copies it.

use crate::data;
use crate::types::TerrainKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Pos {
    pub x: u8,
    pub y: u8,
}

impl Pos {
    #[inline]
    pub const fn new(x: u8, y: u8) -> Self {
        Pos { x, y }
    }

    /// Manhattan distance, which is the metric AWBW uses for weapon ranges.
    #[inline]
    pub fn distance(self, other: Pos) -> u32 {
        (self.x as i32 - other.x as i32).unsigned_abs() + (self.y as i32 - other.y as i32).unsigned_abs()
    }
}

/// A property tile as it appears on a freshly loaded map, before a game
/// assigns countries to seats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertySeed {
    pub pos: Pos,
    pub kind: TerrainKind,
    /// AWBW country code ("os", "bm", ...) for pre-owned properties.
    pub country: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Map {
    pub width: u8,
    pub height: u8,
    /// Row-major, `height * width` entries.
    terrain: Vec<TerrainKind>,
    properties: Vec<PropertySeed>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError {
    /// The tile count did not match `width * height`.
    WrongTileCount { expected: usize, got: usize },
    /// An AWBW terrain id that is not in the terrain table.
    UnknownTerrainId { index: usize, id: u16 },
    TooLarge { width: u32, height: u32 },
}

impl std::fmt::Display for MapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapError::WrongTileCount { expected, got } => {
                write!(f, "expected {expected} tiles, got {got}")
            }
            MapError::UnknownTerrainId { index, id } => {
                write!(f, "unknown AWBW terrain id {id} at tile {index}")
            }
            MapError::TooLarge { width, height } => {
                write!(f, "map {width}x{height} exceeds the 255x255 limit")
            }
        }
    }
}

impl std::error::Error for MapError {}

impl Map {
    /// Builds a map from AWBW terrain ids in row-major order, as they appear in
    /// map exports and replay files.
    pub fn from_awbw_ids(width: u32, height: u32, ids: &[u16]) -> Result<Self, MapError> {
        if width > u8::MAX as u32 || height > u8::MAX as u32 {
            return Err(MapError::TooLarge { width, height });
        }
        let expected = width as usize * height as usize;
        if ids.len() != expected {
            return Err(MapError::WrongTileCount {
                expected,
                got: ids.len(),
            });
        }

        let mut terrain = Vec::with_capacity(expected);
        let mut properties = Vec::new();
        for (index, &id) in ids.iter().enumerate() {
            let info =
                data::terrain_by_awbw_id(id).ok_or(MapError::UnknownTerrainId { index, id })?;
            terrain.push(info.kind);
            if info.kind.is_capturable() {
                properties.push(PropertySeed {
                    pos: Pos::new((index % width as usize) as u8, (index / width as usize) as u8),
                    kind: info.kind,
                    country: info.country,
                });
            }
        }

        Ok(Map {
            width: width as u8,
            height: height as u8,
            terrain,
            properties,
        })
    }

    /// Builds a map directly from terrain kinds, for tests and generated maps.
    pub fn from_kinds(width: u8, height: u8, kinds: Vec<TerrainKind>) -> Result<Self, MapError> {
        let expected = width as usize * height as usize;
        if kinds.len() != expected {
            return Err(MapError::WrongTileCount {
                expected,
                got: kinds.len(),
            });
        }
        let properties = kinds
            .iter()
            .enumerate()
            .filter(|(_, k)| k.is_capturable())
            .map(|(index, &kind)| PropertySeed {
                pos: Pos::new((index % width as usize) as u8, (index / width as usize) as u8),
                kind,
                country: None,
            })
            .collect();
        Ok(Map {
            width,
            height,
            terrain: kinds,
            properties,
        })
    }

    #[inline]
    pub fn tile_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    #[inline]
    pub fn index(&self, pos: Pos) -> usize {
        pos.y as usize * self.width as usize + pos.x as usize
    }

    #[inline]
    pub fn pos_of(&self, index: usize) -> Pos {
        Pos::new(
            (index % self.width as usize) as u8,
            (index / self.width as usize) as u8,
        )
    }

    #[inline]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32
    }

    #[inline]
    pub fn terrain_at(&self, pos: Pos) -> TerrainKind {
        self.terrain[self.index(pos)]
    }

    #[inline]
    pub fn terrain_at_index(&self, index: usize) -> TerrainKind {
        self.terrain[index]
    }

    pub fn properties(&self) -> &[PropertySeed] {
        &self.properties
    }

    /// The four orthogonal neighbours that lie on the board.
    pub fn neighbors(&self, pos: Pos) -> impl Iterator<Item = Pos> + '_ {
        const STEPS: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];
        STEPS.iter().filter_map(move |&(dx, dy)| {
            let (nx, ny) = (pos.x as i32 + dx, pos.y as i32 + dy);
            self.contains(nx, ny).then(|| Pos::new(nx as u8, ny as u8))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_from_awbw_ids_and_finds_properties() {
        // 1 = Plain, 42 = Orange Star HQ, 34 = Neutral City, 2 = Mountain.
        let map = Map::from_awbw_ids(2, 2, &[1, 42, 34, 2]).unwrap();
        assert_eq!(map.terrain_at(Pos::new(0, 0)), TerrainKind::Plain);
        assert_eq!(map.terrain_at(Pos::new(1, 0)), TerrainKind::Hq);
        assert_eq!(map.terrain_at(Pos::new(0, 1)), TerrainKind::City);
        assert_eq!(map.terrain_at(Pos::new(1, 1)), TerrainKind::Mountain);

        let props = map.properties();
        assert_eq!(props.len(), 2);
        assert_eq!(props[0].country, Some("os"));
        assert_eq!(props[1].country, None);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(
            Map::from_awbw_ids(2, 2, &[1, 1, 1]),
            Err(MapError::WrongTileCount { .. })
        ));
        assert!(matches!(
            Map::from_awbw_ids(1, 1, &[9999]),
            Err(MapError::UnknownTerrainId { .. })
        ));
    }

    #[test]
    fn neighbors_stay_on_the_board() {
        let map = Map::from_kinds(3, 3, vec![TerrainKind::Plain; 9]).unwrap();
        assert_eq!(map.neighbors(Pos::new(0, 0)).count(), 2);
        assert_eq!(map.neighbors(Pos::new(1, 1)).count(), 4);
        assert_eq!(map.neighbors(Pos::new(2, 2)).count(), 2);
    }

    #[test]
    fn distance_is_manhattan() {
        assert_eq!(Pos::new(0, 0).distance(Pos::new(3, 4)), 7);
        assert_eq!(Pos::new(5, 5).distance(Pos::new(5, 5)), 0);
    }
}
