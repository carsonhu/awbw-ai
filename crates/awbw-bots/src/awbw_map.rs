//! Loading real AWBW maps.
//!
//! The synthetic board in `map.rs` is fair and cheap, but it is not a board
//! anyone plays on. Training and evaluating on a real league map removes the
//! mismatch between the environment and the replay corpus: the same board,
//! the same rules, the same opening position humans face.
//!
//! Maps come from AWBW's `api/map/map_info.php`, cached under `data/maps/` by
//! `tools/prepare_replay.py`. One is committed — see [`RIVER_SUPREME`].

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use awbw_engine::data;
use awbw_engine::map::{Map, Pos};
use awbw_engine::state::{GameSettings, GameState, Player, PlayerId};
use awbw_engine::types::UnitType;

use serde::Deserialize;

/// The default training map: "A River Supreme", AWBW map 119544.
///
/// Chosen over the other popular league maps because it has the largest pool of
/// recorded standard games (~1,875), the smallest board of the clean
/// candidates, perfect 180-degree rotational symmetry, and — uniquely among the
/// popular maps — no terrain the engine leaves unimplemented.
pub const RIVER_SUPREME: &str = "data/maps/119544.json";

/// AWBW's country codes, in the site's own order. A seat is assigned per
/// country actually present, so seat numbers stay stable across loads.
const COUNTRY_ORDER: [&str; 20] = [
    "os", "bm", "ge", "yc", "bh", "rf", "gs", "bd", "ab", "js", "ci", "pc", "tg", "pl", "ar",
    "wn", "aa", "ne", "sc", "uw",
];

#[derive(Debug, Deserialize)]
struct RawMap {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Size X")]
    width: u32,
    #[serde(rename = "Size Y")]
    height: u32,
    /// Indexed `[x][y]`, unlike everything else here.
    #[serde(rename = "Terrain Map")]
    terrain: Vec<Vec<u16>>,
    #[serde(rename = "Predeployed Units", default)]
    predeployed: Vec<RawUnit>,
}

#[derive(Debug, Deserialize)]
struct RawUnit {
    #[serde(rename = "Unit ID")]
    id: u32,
    #[serde(rename = "Unit X")]
    x: u8,
    #[serde(rename = "Unit Y")]
    y: u8,
    #[serde(rename = "Unit HP")]
    hp: u8,
    #[serde(rename = "Country Code")]
    country: String,
}

/// A unit the map starts with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deployment {
    pub typ: UnitType,
    pub owner: PlayerId,
    pub pos: Pos,
    /// Displayed HP, 1..=10.
    pub hp: u8,
}

#[derive(Debug, Clone)]
pub struct AwbwMap {
    pub name: String,
    pub map: Arc<Map>,
    /// Property owners, in the same order as `map.properties()`.
    pub owners: Vec<Option<PlayerId>>,
    pub deployments: Vec<Deployment>,
    pub players: usize,
}

impl AwbwMap {
    /// Reads a cached AWBW map export.
    pub fn load(path: impl AsRef<Path>) -> Result<AwbwMap, String> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        AwbwMap::from_json(&text)
    }

    pub fn from_json(text: &str) -> Result<AwbwMap, String> {
        let raw: RawMap = serde_json::from_str(text).map_err(|e| format!("map json: {e}"))?;
        let (w, h) = (raw.width as usize, raw.height as usize);
        if raw.terrain.len() != w || raw.terrain.iter().any(|col| col.len() != h) {
            return Err(format!(
                "terrain is {}x{}, header says {w}x{h}",
                raw.terrain.len(),
                raw.terrain.first().map_or(0, |c| c.len())
            ));
        }

        // The export is column-major; the engine wants row-major.
        let mut flat = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                flat.push(raw.terrain[x][y]);
            }
        }
        let map = Map::from_awbw_ids(raw.width, raw.height, &flat)
            .map_err(|e| format!("map: {e}"))?;

        // Every country that owns something, or has a unit, gets a seat.
        let mut present: BTreeMap<usize, &str> = BTreeMap::new();
        let mut note = |code: &str| {
            if let Some(rank) = COUNTRY_ORDER.iter().position(|&c| c == code) {
                present.entry(rank).or_insert(COUNTRY_ORDER[rank]);
            }
        };
        for seed in map.properties() {
            if let Some(country) = seed.country {
                note(country);
            }
        }
        for unit in &raw.predeployed {
            note(&unit.country);
        }
        let seats: BTreeMap<&str, PlayerId> = present
            .values()
            .enumerate()
            .map(|(seat, &code)| (code, seat as PlayerId))
            .collect();

        let owners = map
            .properties()
            .iter()
            .map(|seed| seed.country.and_then(|c| seats.get(c).copied()))
            .collect();

        let mut deployments = Vec::with_capacity(raw.predeployed.len());
        for unit in &raw.predeployed {
            let typ = data::unit_type_by_awbw_id(unit.id)
                .ok_or_else(|| format!("unknown predeployed unit id {}", unit.id))?;
            let owner = *seats
                .get(unit.country.as_str())
                .ok_or_else(|| format!("unit for unknown country {:?}", unit.country))?;
            deployments.push(Deployment {
                typ,
                owner,
                pos: Pos::new(unit.x, unit.y),
                hp: unit.hp.clamp(1, 10),
            });
        }

        Ok(AwbwMap {
            name: raw.name,
            map: Arc::new(map),
            owners,
            deployments,
            players: seats.len().max(2),
        })
    }

    /// Builds a starting position: property ownership and the map's own units.
    pub fn new_game(&self, settings: GameSettings, starting_funds: u32) -> GameState {
        let players = (0..self.players)
            .map(|seat| Player::new(starting_funds, seat as u8 + 1))
            .collect();
        let mut state = GameState::new(self.map.clone(), settings, players, &self.owners);
        for deployment in &self.deployments {
            let id = state.spawn(deployment.typ, deployment.owner, deployment.pos);
            if let Some(unit) = state.unit_mut(id) {
                unit.hp100 = deployment.hp.min(10) * 10;
            }
        }
        state
    }

    /// Whether the board maps onto itself under a 180-degree rotation.
    ///
    /// Only the terrain is checked. A map can be terrain-symmetric and still
    /// hand one seat an extra starting unit, which several league maps do to
    /// compensate the player who moves second — see `deployments_are_symmetric`.
    pub fn terrain_is_symmetric(&self) -> bool {
        let (w, h) = (self.map.width, self.map.height);
        (0..h).all(|y| {
            (0..w).all(|x| {
                self.map.terrain_at(Pos::new(x, y))
                    == self.map.terrain_at(Pos::new(w - 1 - x, h - 1 - y))
            })
        })
    }

    /// Whether the starting units mirror as well as the terrain does.
    pub fn deployments_are_symmetric(&self) -> bool {
        let (w, h) = (self.map.width, self.map.height);
        self.deployments.iter().all(|d| {
            let mirror = Pos::new(w - 1 - d.pos.x, h - 1 - d.pos.y);
            self.deployments
                .iter()
                .any(|other| other.pos == mirror && other.typ == d.typ && other.owner != d.owner)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use awbw_engine::types::TerrainKind;

    fn river_supreme() -> AwbwMap {
        // Tests run from the crate directory, the binaries from the root.
        AwbwMap::load(RIVER_SUPREME)
            .or_else(|_| AwbwMap::load(Path::new("../..").join(RIVER_SUPREME)))
            .expect("the canonical map is committed")
    }

    #[test]
    fn loads_the_canonical_map() {
        let m = river_supreme();
        assert_eq!(m.name, "A River Supreme");
        assert_eq!((m.map.width, m.map.height), (17, 18));
        assert_eq!(m.players, 2);
    }

    #[test]
    fn the_terrain_is_rotationally_symmetric() {
        assert!(river_supreme().terrain_is_symmetric());
    }

    #[test]
    fn the_starting_units_are_not() {
        // Blue Moon gets an infantry Orange Star has no counterpart for, which
        // is a real feature of this map rather than a loading bug: seats are not
        // interchangeable, so evaluation has to swap them.
        let m = river_supreme();
        assert!(!m.deployments_are_symmetric());
        assert_eq!(m.deployments.len(), 3);
    }

    #[test]
    fn each_side_gets_an_hq_and_two_bases() {
        let m = river_supreme();
        let mut hqs = [0; 2];
        let mut bases = [0; 2];
        for (seed, owner) in m.map.properties().iter().zip(m.owners.iter()) {
            let Some(seat) = owner else { continue };
            match seed.kind {
                TerrainKind::Hq => hqs[*seat as usize] += 1,
                TerrainKind::Base => bases[*seat as usize] += 1,
                _ => {}
            }
        }
        assert_eq!(hqs, [1, 1]);
        assert_eq!(bases, [2, 2]);
    }

    #[test]
    fn every_rule_this_map_needs_is_implemented() {
        // The reason this map was picked: nothing on it is faked.
        let m = river_supreme();
        for y in 0..m.map.height {
            for x in 0..m.map.width {
                let kind = m.map.terrain_at(Pos::new(x, y));
                assert!(
                    !matches!(
                        kind,
                        TerrainKind::Silo
                            | TerrainKind::SiloEmpty
                            | TerrainKind::Pipe
                            | TerrainKind::PipeSeam
                            | TerrainKind::PipeRubble
                            | TerrainKind::Teleporter
                            | TerrainKind::Lab
                    ),
                    "unimplemented terrain {kind:?} at ({x},{y})"
                );
            }
        }
    }

    #[test]
    fn a_game_starts_with_the_maps_own_units() {
        let m = river_supreme();
        let state = m.new_game(GameSettings::default(), 10_000);
        assert_eq!(state.units().count(), 3);
        for d in &m.deployments {
            let unit = state.unit_at(d.pos).expect("deployment is on the board");
            assert_eq!(unit.typ, d.typ);
            assert_eq!(unit.owner, d.owner);
        }
    }
}
