//! Core enums. Discriminants are table indices, so the order here must match
//! the generated tables in `data.rs` (which tools/gen_tables.py also fixes).

use crate::data;

/// The 25 AWBW unit types, ordered by ascending AWBW generic id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum UnitType {
    Infantry,
    Mech,
    MdTank,
    Tank,
    Recon,
    Apc,
    Artillery,
    Rocket,
    AntiAir,
    Missile,
    Fighter,
    Bomber,
    BCopter,
    TCopter,
    Battleship,
    Cruiser,
    Lander,
    Sub,
    BlackBoat,
    Carrier,
    Stealth,
    Neotank,
    Piperunner,
    BlackBomb,
    MegaTank,
}

impl UnitType {
    pub const ALL: [UnitType; data::NUM_UNIT_TYPES] = [
        UnitType::Infantry,
        UnitType::Mech,
        UnitType::MdTank,
        UnitType::Tank,
        UnitType::Recon,
        UnitType::Apc,
        UnitType::Artillery,
        UnitType::Rocket,
        UnitType::AntiAir,
        UnitType::Missile,
        UnitType::Fighter,
        UnitType::Bomber,
        UnitType::BCopter,
        UnitType::TCopter,
        UnitType::Battleship,
        UnitType::Cruiser,
        UnitType::Lander,
        UnitType::Sub,
        UnitType::BlackBoat,
        UnitType::Carrier,
        UnitType::Stealth,
        UnitType::Neotank,
        UnitType::Piperunner,
        UnitType::BlackBomb,
        UnitType::MegaTank,
    ];

    #[inline]
    pub fn stats(self) -> &'static data::UnitStats {
        &data::UNIT_STATS[self as usize]
    }

    /// Indirect units (min range > 1) fire from where they stand and never
    /// counterattack.
    #[inline]
    pub fn is_indirect(self) -> bool {
        self.stats().range_min > 1
    }

    #[inline]
    pub fn from_awbw_id(id: u32) -> Option<UnitType> {
        data::unit_type_by_awbw_id(id)
    }
}

/// AWBW movement classes, in the site's F/B/T/W/A/S/L/P column order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MoveType {
    /// "F": infantry.
    Foot,
    /// "B": mech.
    Boot,
    Tread,
    Tires,
    Air,
    Sea,
    Lander,
    Pipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Weather {
    Clear,
    Rain,
    Snow,
}

/// Terrain classes with distinct rules. Individual AWBW terrain ids (all 196
/// of them: orientations, country variants) map onto these via
/// `data::terrain_by_awbw_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TerrainKind {
    Plain,
    Mountain,
    Wood,
    River,
    Road,
    Bridge,
    Sea,
    Shoal,
    Reef,
    City,
    Base,
    Airport,
    Port,
    Hq,
    Pipe,
    Silo,
    SiloEmpty,
    ComTower,
    Lab,
    Teleporter,
    PipeSeam,
    PipeRubble,
}

impl TerrainKind {
    /// Movement cost for a unit class in the given weather; None = impassable.
    #[inline]
    pub fn move_cost(self, weather: Weather, mt: MoveType) -> Option<u8> {
        match data::MOVE_COST[self as usize][weather as usize][mt as usize] {
            0 => None,
            c => Some(c),
        }
    }

    /// Defence stars. Note AWBW gives air units and units on pipe seams zero
    /// stars regardless; `combat` handles that.
    #[inline]
    pub fn defense(self) -> u8 {
        data::TERRAIN_DEFENSE[self as usize]
    }

    /// Terrain that conceals a ground or sea unit from anything not adjacent.
    ///
    /// "Woods and Reefs will always hide ground and sea units (but not air
    /// units) from vision, unless an allied unit is directly adjacent to them."
    /// — the AWBW wiki, which describes the live game.
    #[inline]
    pub fn provides_cover(self) -> bool {
        matches!(self, TerrainKind::Wood | TerrainKind::Reef)
    }

    /// Extra sight range a surface unit gains from standing here.
    #[inline]
    pub fn vision_boost(self) -> u8 {
        match self {
            TerrainKind::Mountain => 3,
            _ => 0,
        }
    }

    #[inline]
    pub fn is_capturable(self) -> bool {
        matches!(
            self,
            TerrainKind::City
                | TerrainKind::Base
                | TerrainKind::Airport
                | TerrainKind::Port
                | TerrainKind::Hq
                | TerrainKind::ComTower
                | TerrainKind::Lab
        )
    }

    /// Properties that pay 1000 funds each per day. Com Towers and Labs are
    /// owned but generate nothing (AWBW funcs/new_turn.php).
    #[inline]
    pub fn produces_income(self) -> bool {
        matches!(
            self,
            TerrainKind::City
                | TerrainKind::Base
                | TerrainKind::Airport
                | TerrainKind::Port
                | TerrainKind::Hq
        )
    }
}
