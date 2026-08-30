//! AWBW's combat formula, matching the site's server engine exactly.
//!
//! The formula (per luck roll), as implemented in AWBW's engine
//! (helper/fire.rs, mirrored by funcs/calculate_percentage.php and by
//! RizeBot's port, all of which agree):
//!
//! ```text
//! attackPower  = coAttack + comTowerBonus + (coAttackPower - 100)
//! defensePower = coDefense + (coDefensePower - 100)
//! d1 = ceil(attHP)/10 * (percentage * attackPower/100 + goodLuck - badLuck)
//! d2 = d1 * (200 - (defensePower + terrainDefense * ceil(defHP))) / 100
//! d3 = round(d2 * 10) / 10          // one decimal place
//! d4 = trunc(clamp(d3, 0, 100))     // damage out of 100 (= 10.0 HP)
//! ```
//!
//! HP here is on AWBW's internal 0..=100 scale ("hp100"); the displayed 1-10
//! value is `ceil(hp100 / 10)`. Terrain defence is zero for air units and on
//! pipe seams. Default luck is bad 0, good 0..=9 inclusive.

use crate::co_data::{AttackCondition, CoData};
use crate::data;
use crate::state::ActivePower;
use crate::types::{MoveType, TerrainKind, UnitType};

/// CO attack/defence hooks. Vanilla (no CO abilities) is all-100s; real COs
/// plug in here later without touching the formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoModifiers {
    /// CO base attack %, before the Com Tower bonus. Vanilla is 100.
    pub attack: i32,
    /// CO base defence %. Vanilla is 100.
    pub defense: i32,
    /// Attack multiplier while a power is active; 100 when off.
    pub attack_power: i32,
    /// Defence multiplier while a power is active; 100 when off.
    pub defense_power: i32,
}

pub const VANILLA_CO: CoModifiers = CoModifiers {
    attack: 100,
    defense: 100,
    attack_power: 100,
    defense_power: 100,
};

/// Vanilla luck: no bad luck, 0..=9 good luck (inclusive).
pub const VANILLA_GOOD_LUCK_MAX: i32 = 9;

/// Turns a CO's day-to-day ability into the modifiers the formula wants.
///
/// `terrain` is the tile the *attacker* stands on, which is what the
/// terrain-conditional COs key off: Kindle on properties, Koal on roads, Jake
/// on plains, and Lash scaling with the tile's own defence stars. It has no
/// bearing on the defence half, so passing the defender's tile is harmless.
///
/// `power` is whichever of this CO's powers is running: any power grants +10
/// attack and +10 defence on top of its listed effect. (CO-specific combat
/// boosts beyond the universal +10 are not modelled yet.)
pub fn co_modifiers(
    co: &CoData,
    unit: UnitType,
    terrain: TerrainKind,
    power: ActivePower,
) -> CoModifiers {
    let index = unit as usize;
    // Powers escalate the conditional bonus (Jake's plains, Koal's roads),
    // under the same terrain gate as the day-to-day part. PerTerrainStar has
    // no modelled escalation, and the data holds zero there.
    let cond_value = co.conditional_attack[index] as i32
        + match power {
            ActivePower::None => 0,
            ActivePower::Cop => co.cop_conditional_bonus as i32,
            ActivePower::Scop => co.scop_conditional_bonus as i32,
        };
    let conditional = match co.condition {
        AttackCondition::Always => 0,
        AttackCondition::UrbanOnly => {
            if terrain.is_capturable() {
                cond_value
            } else {
                0
            }
        }
        AttackCondition::RoadOnly => {
            if matches!(terrain, TerrainKind::Road | TerrainKind::Bridge) {
                cond_value
            } else {
                0
            }
        }
        AttackCondition::PlainOnly => {
            if terrain == TerrainKind::Plain {
                cond_value
            } else {
                0
            }
        }
        AttackCondition::PerTerrainStar => {
            co.conditional_attack[index] as i32 * terrain.defense() as i32
        }
    };

    // The universal +10 during any power, plus the CO's own power attack
    // (Grimm's +20/+50, Jess's vehicles) — the formula reads this term as
    // percentage points over 100, so both add straight in.
    let boost = match power {
        ActivePower::None => 100,
        ActivePower::Cop => 110 + co.cop_attack[index] as i32,
        ActivePower::Scop => 110 + co.scop_attack[index] as i32,
    };
    let defense_boost = if power == ActivePower::None { 100 } else { 110 };
    CoModifiers {
        attack: 100 + co.attack[index] as i32 + conditional,
        defense: 100 + co.defense[index] as i32,
        attack_power: boost,
        defense_power: defense_boost,
    }
}

/// A unit's firing range after its CO's day-to-day modifier (Grit reaches one
/// tile further, Max's indirects one tile less) and whatever a running power
/// adds (Jake's land indirects reach one further under either power).
pub fn effective_range(co: &CoData, unit: UnitType, power: ActivePower) -> (u32, u32) {
    let stats = unit.stats();
    let power_delta = match power {
        ActivePower::None => 0,
        ActivePower::Cop => co.cop_range_delta[unit as usize] as i32,
        ActivePower::Scop => co.scop_range_delta[unit as usize] as i32,
    };
    let min = stats.range_min.max(1) as i32;
    let max = stats.range_max.max(1) as i32
        + co.range_delta[unit as usize] as i32
        + power_delta;
    (min.max(1) as u32, max.max(min) as u32)
}

/// Displayed HP (1..=10) from internal hp100.
#[inline]
pub fn display_hp(hp100: i32) -> i32 {
    debug_assert!((0..=100).contains(&hp100));
    (hp100 + 9) / 10
}

/// Which weapon a shot uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weapon {
    Primary,
    Secondary,
}

/// Base damage % and the weapon that deals it. The primary is used when it has
/// an entry and the attacker has ammo; otherwise the secondary. None = this
/// pairing cannot attack at all.
#[inline]
pub fn base_percentage(attacker: UnitType, defender: UnitType, attacker_ammo: u8) -> Option<(i32, Weapon)> {
    let a = attacker as usize;
    let d = defender as usize;
    let primary = data::BASE_DAMAGE_PRIMARY[a][d];
    if primary > 0 && attacker_ammo > 0 {
        return Some((primary as i32, Weapon::Primary));
    }
    let secondary = data::BASE_DAMAGE_SECONDARY[a][d];
    if secondary > 0 {
        return Some((secondary as i32, Weapon::Secondary));
    }
    None
}

/// Dived subs are only attackable by cruisers and subs; hidden stealths only
/// by fighters and stealths.
#[inline]
pub fn can_target_hidden(attacker: UnitType, defender: UnitType) -> bool {
    match defender {
        UnitType::Sub => matches!(attacker, UnitType::Cruiser | UnitType::Sub),
        UnitType::Stealth => matches!(attacker, UnitType::Fighter | UnitType::Stealth),
        _ => true,
    }
}

/// Terrain stars the defender actually benefits from: air units and pipe
/// seams get none.
#[inline]
pub fn effective_terrain_defense(defender_move_type: MoveType, terrain: TerrainKind) -> i32 {
    if defender_move_type == MoveType::Air {
        return 0;
    }
    if matches!(terrain, TerrainKind::PipeSeam | TerrainKind::PipeRubble) {
        return 0;
    }
    terrain.defense() as i32
}

/// One luck roll of the damage formula. Returns damage out of 100.
///
/// `terrain_defense` must already be the *effective* stars (see
/// [`effective_terrain_defense`]). `tower_bonus` is +10 per Com Tower the
/// attacker's army owns.
pub fn damage_roll(
    percentage: i32,
    attacker_hp100: i32,
    defender_hp100: i32,
    terrain_defense: i32,
    attacker_co: CoModifiers,
    defender_co: CoModifiers,
    tower_bonus: i32,
    good_luck: i32,
    bad_luck: i32,
) -> i32 {
    let attack_power = attacker_co.attack + tower_bonus + (attacker_co.attack_power - 100);
    let defense_power = defender_co.defense + (defender_co.defense_power - 100);

    let d1 = (display_hp(attacker_hp100) as f64 / 10.0)
        * (percentage as f64 * (attack_power as f64 / 100.0) + good_luck as f64 - bad_luck as f64);
    let d2 = d1 * (200.0 - (defense_power as f64 + terrain_defense as f64 * display_hp(defender_hp100) as f64))
        / 100.0;
    // Round to one decimal, then truncate: only x.95 and up gains a point.
    let d3 = (d2 * 10.0).round() / 10.0;
    d3.clamp(0.0, 100.0).trunc() as i32
}

/// Min / max / expected damage across a luck range (both bounds inclusive),
/// and the share of rolls that kill outright — the number a player actually
/// weighs before committing to an attack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageSpread {
    pub min: i32,
    pub max: i32,
    pub expected: f64,
    /// P(damage >= the defender's remaining hp100) over the same rolls.
    pub kill_chance: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn damage_spread(
    percentage: i32,
    attacker_hp100: i32,
    defender_hp100: i32,
    terrain_defense: i32,
    attacker_co: CoModifiers,
    defender_co: CoModifiers,
    tower_bonus: i32,
    good_luck_max: i32,
    bad_luck_max: i32,
) -> DamageSpread {
    let mut min = i32::MAX;
    let mut max = i32::MIN;
    let mut total = 0i64;
    let mut count = 0i64;
    let mut kills = 0i64;
    for bad in 0..=bad_luck_max {
        for good in 0..=good_luck_max {
            let d = damage_roll(
                percentage,
                attacker_hp100,
                defender_hp100,
                terrain_defense,
                attacker_co,
                defender_co,
                tower_bonus,
                good,
                bad,
            );
            min = min.min(d);
            max = max.max(d);
            total += d as i64;
            count += 1;
            kills += (d >= defender_hp100) as i64;
        }
    }
    DamageSpread {
        min,
        max,
        expected: total as f64 / count as f64,
        kill_chance: kills as f64 / count as f64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_active_power_grants_ten_and_ten() {
        let off = co_modifiers(
            &CoData::VANILLA,
            UnitType::Infantry,
            TerrainKind::Plain,
            ActivePower::None,
        );
        let on = co_modifiers(
            &CoData::VANILLA,
            UnitType::Infantry,
            TerrainKind::Plain,
            ActivePower::Cop,
        );
        assert_eq!((off.attack_power, off.defense_power), (100, 100));
        assert_eq!((on.attack_power, on.defense_power), (110, 110));
    }

    /// Firepower totals per co.php: the formula reads
    /// `attack + (attack_power - 100)` as the attacker's percentage.
    fn firepower(co: &str, unit: UnitType, terrain: TerrainKind, power: ActivePower) -> i32 {
        let m = co_modifiers(
            crate::co_data::co_by_name(co).unwrap(),
            unit,
            terrain,
            power,
        );
        m.attack + (m.attack_power - 100)
    }

    #[test]
    fn grimm_powers_stack_attack_on_his_day_to_day() {
        // D2D +30; Knuckleduster +50 total, Haymaker +80, plus the universal
        // +10 while any power runs. Defence stays at his -20 throughout.
        let t = TerrainKind::Road;
        assert_eq!(firepower("Grimm", UnitType::Tank, t, ActivePower::None), 130);
        assert_eq!(firepower("Grimm", UnitType::Tank, t, ActivePower::Cop), 160);
        assert_eq!(firepower("Grimm", UnitType::Tank, t, ActivePower::Scop), 190);
        let m = co_modifiers(
            crate::co_data::co_by_name("Grimm").unwrap(),
            UnitType::Tank,
            t,
            ActivePower::Scop,
        );
        assert_eq!(m.defense, 80);
        assert_eq!(m.defense_power, 110);
    }

    #[test]
    fn jake_escalates_plains_only_and_extends_land_indirects() {
        // Plains 10 -> 20 (COP) -> 40 (SCOP); off plains only the universal +10.
        let p = TerrainKind::Plain;
        assert_eq!(firepower("Jake", UnitType::Tank, p, ActivePower::None), 110);
        assert_eq!(firepower("Jake", UnitType::Tank, p, ActivePower::Cop), 130);
        assert_eq!(firepower("Jake", UnitType::Tank, p, ActivePower::Scop), 150);
        assert_eq!(firepower("Jake", UnitType::Tank, TerrainKind::Road, ActivePower::Scop), 110);

        let jake = crate::co_data::co_by_name("Jake").unwrap();
        assert_eq!(effective_range(jake, UnitType::Artillery, ActivePower::None), (2, 3));
        assert_eq!(effective_range(jake, UnitType::Artillery, ActivePower::Cop), (2, 4));
        assert_eq!(effective_range(jake, UnitType::Rocket, ActivePower::Scop), (3, 6));
        // Naval indirects are not land indirects.
        assert_eq!(effective_range(jake, UnitType::Battleship, ActivePower::Scop), (2, 6));
    }

    #[test]
    fn koal_escalates_roads_and_jess_boosts_vehicles() {
        // Koal: the road bonus runs 10 -> 20 -> 30 under the powers, plus
        // the universal ten while one is active.
        let r = TerrainKind::Road;
        assert_eq!(firepower("Koal", UnitType::Tank, r, ActivePower::None), 110);
        assert_eq!(firepower("Koal", UnitType::Tank, r, ActivePower::Cop), 130);
        assert_eq!(firepower("Koal", UnitType::Tank, r, ActivePower::Scop), 140);

        // Jess: vehicles 10 -> 20 -> 40; her footsoldiers stay at -10 and
        // gain only the universal ten.
        let p = TerrainKind::Plain;
        assert_eq!(firepower("Jess", UnitType::Tank, p, ActivePower::None), 110);
        assert_eq!(firepower("Jess", UnitType::Tank, p, ActivePower::Cop), 130);
        assert_eq!(firepower("Jess", UnitType::Tank, p, ActivePower::Scop), 150);
        assert_eq!(firepower("Jess", UnitType::Infantry, p, ActivePower::Scop), 100);
    }

    #[test]
    fn infantry_vs_infantry_on_plain() {
        // Base 55%, defender on 1 star at 10 HP: 55 * 0.9 = 49.5 -> 49.
        let (pct, weapon) = base_percentage(UnitType::Infantry, UnitType::Infantry, 0).unwrap();
        assert_eq!(pct, 55);
        assert_eq!(weapon, Weapon::Secondary); // infantry MG is a secondary
        let d = damage_roll(pct, 100, 100, 1, VANILLA_CO, VANILLA_CO, 0, 0, 0);
        assert_eq!(d, 49);
        // Best luck: (55 + 9) * 0.9 = 57.6 -> 57.
        let d = damage_roll(pct, 100, 100, 1, VANILLA_CO, VANILLA_CO, 0, 9, 0);
        assert_eq!(d, 57);
    }

    #[test]
    fn tank_vs_recon_on_road() {
        // Primary cannon, base 85%, zero stars: exactly base + luck.
        let (pct, weapon) = base_percentage(UnitType::Tank, UnitType::Recon, 9).unwrap();
        assert_eq!(pct, 85);
        assert_eq!(weapon, Weapon::Primary);
        assert_eq!(damage_roll(pct, 100, 100, 0, VANILLA_CO, VANILLA_CO, 0, 0, 0), 85);
        assert_eq!(damage_roll(pct, 100, 100, 0, VANILLA_CO, VANILLA_CO, 0, 9, 0), 94);
    }

    #[test]
    fn out_of_ammo_falls_back_to_secondary() {
        // Tank with no ammo uses MG vs recon: 40%... (secondary table value)
        let (pct, weapon) = base_percentage(UnitType::Tank, UnitType::Recon, 0).unwrap();
        assert_eq!(weapon, Weapon::Secondary);
        assert!(pct > 0);
        // Tank with no ammo cannot touch a Md.Tank if MG can't hurt it -> still
        // has an entry (1%) per the AWBW table.
        assert!(base_percentage(UnitType::Tank, UnitType::MdTank, 0).is_some());
    }

    #[test]
    fn indirect_pairings_without_entries_cannot_attack() {
        // Lander has no weapons at all.
        for def in UnitType::ALL {
            assert!(base_percentage(UnitType::Lander, def, 9).is_none());
        }
        // Anti-Air cannot shoot a sub.
        assert!(base_percentage(UnitType::AntiAir, UnitType::Sub, 9).is_none());
    }

    #[test]
    fn damaged_attacker_scales_by_displayed_hp() {
        // 5.5 HP displays as 6: 0.6 * 55 * 0.9 = 29.7 -> 29.
        let d = damage_roll(55, 55, 100, 1, VANILLA_CO, VANILLA_CO, 0, 0, 0);
        assert_eq!(d, 29);
    }

    #[test]
    fn defender_hp_scales_terrain_stars() {
        // Defender at 1 HP on 3 stars only gets 3 effective: 55*(200-103)/100.
        let d = damage_roll(55, 100, 10, 3, VANILLA_CO, VANILLA_CO, 0, 0, 0);
        assert_eq!(d, (55.0f64 * 0.97).trunc() as i32);
    }

    #[test]
    fn spread_bounds() {
        let s = damage_spread(55, 100, 100, 1, VANILLA_CO, VANILLA_CO, 0, VANILLA_GOOD_LUCK_MAX, 0);
        assert_eq!(s.min, 49);
        assert_eq!(s.max, 57);
        assert!(s.expected > 49.0 && s.expected < 57.0);
    }

    #[test]
    fn hidden_targeting() {
        assert!(can_target_hidden(UnitType::Cruiser, UnitType::Sub));
        assert!(!can_target_hidden(UnitType::Battleship, UnitType::Sub));
        assert!(can_target_hidden(UnitType::Fighter, UnitType::Stealth));
        assert!(!can_target_hidden(UnitType::Missile, UnitType::Stealth));
    }
}
