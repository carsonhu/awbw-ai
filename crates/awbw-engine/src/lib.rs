//! A rules engine for Advance Wars by Web (AWBW), designed for RL self-play:
//! compact copyable state, allocation-free hot paths, seeded determinism.
//!
//! Data provenance: unit stats and terrain/movement charts are generated from
//! AWBW's own chart pages, the damage table from the site's `js/damage_inc.json`,
//! and the terrain-id mapping from the AWBW DB dump (via RizeBot's generated
//! table). See `tools/` at the workspace root.

pub mod actions;
pub mod co_data;
pub mod combat;
pub mod data;
pub mod map;
pub mod movement;
pub mod rng;
pub mod state;
pub mod types;

pub use actions::{Action, ActionError, ActionReport, Engine};
pub use co_data::{co_by_awbw_id, co_by_name, CoData};
pub use map::{Map, Pos};
pub use movement::Reach;
pub use rng::Rng;
pub use state::{GameSettings, GameState, Outcome, Player, Unit, UnitId};
pub use types::{MoveType, TerrainKind, UnitType, Weather};

#[cfg(test)]
mod data_tests {
    use crate::data;
    use crate::types::{MoveType, TerrainKind, UnitType, Weather};

    #[test]
    fn unit_ids_round_trip() {
        for ut in UnitType::ALL {
            assert_eq!(data::unit_type_by_awbw_id(ut.stats().awbw_id), Some(ut));
        }
        assert_eq!(data::unit_type_by_awbw_id(999), None);
    }

    #[test]
    fn spot_check_unit_stats() {
        let inf = UnitType::Infantry.stats();
        assert_eq!((inf.cost, inf.move_points, inf.max_ammo), (1000, 3, 0));
        let mega = UnitType::MegaTank.stats();
        assert_eq!((mega.awbw_id, mega.cost, mega.max_ammo), (1141438, 28000, 3));
        let carrier = UnitType::Carrier.stats();
        assert_eq!((carrier.range_min, carrier.range_max), (3, 8));
        assert!(UnitType::Battleship.is_indirect());
        assert!(!UnitType::Tank.is_indirect());
    }

    #[test]
    fn spot_check_move_costs() {
        use MoveType::*;
        use Weather::*;
        // Tires pay double on plains, treads don't.
        assert_eq!(TerrainKind::Plain.move_cost(Clear, Tires), Some(2));
        assert_eq!(TerrainKind::Plain.move_cost(Clear, Tread), Some(1));
        // Rain bogs vehicles down in woods.
        assert_eq!(TerrainKind::Wood.move_cost(Rain, Tread), Some(3));
        // Snow slows air.
        assert_eq!(TerrainKind::Sea.move_cost(Snow, Air), Some(2));
        // Mountains: foot only (and air).
        assert_eq!(TerrainKind::Mountain.move_cost(Clear, Tread), None);
        assert_eq!(TerrainKind::Mountain.move_cost(Clear, Foot), Some(2));
        assert_eq!(TerrainKind::Mountain.move_cost(Snow, Foot), Some(4));
        // Pipes: piperunners only.
        assert_eq!(TerrainKind::Pipe.move_cost(Clear, Air), None);
        assert_eq!(TerrainKind::Pipe.move_cost(Clear, Pipe), Some(1));
        // Bases are pipe-network entry points.
        assert_eq!(TerrainKind::Base.move_cost(Clear, Pipe), Some(1));
        // Landers exist on shoals and seas, not land.
        assert_eq!(TerrainKind::Shoal.move_cost(Clear, MoveType::Lander), Some(1));
        assert_eq!(TerrainKind::Road.move_cost(Clear, MoveType::Lander), None);
    }

    #[test]
    fn spot_check_terrain_ids() {
        let plain = data::terrain_by_awbw_id(1).unwrap();
        assert_eq!(plain.kind, TerrainKind::Plain);
        let os_hq = data::terrain_by_awbw_id(42).unwrap();
        assert_eq!(os_hq.kind, TerrainKind::Hq);
        assert_eq!(os_hq.country, Some("os"));
        let neutral_city = data::terrain_by_awbw_id(34).unwrap();
        assert_eq!(neutral_city.kind, TerrainKind::City);
        assert_eq!(neutral_city.country, None);
        assert!(data::terrain_by_awbw_id(9999).is_none());
    }

    #[test]
    fn damage_table_covers_known_pairings() {
        use data::{BASE_DAMAGE_PRIMARY, BASE_DAMAGE_SECONDARY};
        let a = |u: UnitType| u as usize;
        // Mech bazooka vs tank: 55.
        assert_eq!(BASE_DAMAGE_PRIMARY[a(UnitType::Mech)][a(UnitType::Tank)], 55);
        // Rockets vs infantry: 95.
        assert_eq!(BASE_DAMAGE_PRIMARY[a(UnitType::Rocket)][a(UnitType::Infantry)], 95);
        // Anti-air shreds copters: 120.
        assert_eq!(BASE_DAMAGE_PRIMARY[a(UnitType::AntiAir)][a(UnitType::BCopter)], 120);
        // Infantry MG vs mega tank: 1.
        assert_eq!(BASE_DAMAGE_SECONDARY[a(UnitType::Infantry)][a(UnitType::MegaTank)], 1);
        // Landers are unarmed.
        for d in 0..data::NUM_UNIT_TYPES {
            assert_eq!(BASE_DAMAGE_PRIMARY[a(UnitType::Lander)][d], -1);
            assert_eq!(BASE_DAMAGE_SECONDARY[a(UnitType::Lander)][d], -1);
        }
    }
}
