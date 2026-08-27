"""Generate crates/awbw-engine/src/co_data.rs from the AWBW-Replay-Player's
COs.json, which encodes each CO's day-to-day ability in machine-readable form.

Day-to-day abilities are fully generated. Of powers, the *meter* is generated
for every CO (COP/SCOP star counts; -1 when the CO lacks that power), while
power *effects* are modelled only for the COs in POWER_MOVE below — the
engine's universal +10/+10 during any power needs no data. The replay harness
still excludes power-affected turns for unmodelled COs.
"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = Path(r"F:\awbw\AWBW-Replay-Player\AWBWApp.Resources\Json\COs.json")
OUT = ROOT / "crates" / "awbw-engine" / "src" / "co_data.rs"

# Same order as UnitType in types.rs / gen_tables.py.
UNIT_ORDER = [
    "Infantry", "Mech", "Md.Tank", "Tank", "Recon", "APC", "Artillery", "Rocket",
    "Anti-Air", "Missile", "Fighter", "Bomber", "B-Copter", "T-Copter",
    "Battleship", "Cruiser", "Lander", "Sub", "Black Boat", "Carrier", "Stealth",
    "Neotank", "Piperunner", "Black Bomb", "Mega Tank",
]
INDEX = {name: i for i, name in enumerate(UNIT_ORDER)}
N = len(UNIT_ORDER)

# Abilities COs.json leaves empty. The wording below is AWBW's own, from
# https://awbw.amarriner.com/co.php (mirrored at data/awbw-site/co.php.html),
# which is the authoritative prose description of every CO.
MANUAL = {
    # Lash: +10% attack per terrain star the attacker stands on.
    "Lash": {"condition": "PerTerrainStar", "conditional_attack_all": 10},
    # Sami: footsoldiers capture at 1.5x. COs.json records her attack bonus but
    # not this, and it is the sole cause of capture divergence in her games.
    # Sami: "Footsoldiers gain ... a 50% capture point bonus (rounded down) ...
    # Transports gain +1 movement."
    "Sami": {"capture_pct": 150, "move_delta_transports": 1},
    # Rachel: "Units repair +1 additional HP (note: liable for costs)."
    "Rachel": {"repair_bonus_hp100": 10},
    # Adder, Rachel, Javier and Andy have no day-to-day combat effect we model.
    # (Javier's +20% defence against indirects is conditional on the *attacker*
    # being indirect, which the current per-unit table cannot express.)
}

# All-unit movement gained during (COP, SCOP), for the COs whose power effects
# the engine models. COs.json records only star counts; effects live in its
# comments, so they are transcribed here from co.php. Powers whose movement is
# restricted to some units (Jess, Jake's SCOP) need a finer shape than this.
POWER_MOVE = {
    "Adder": (1, 2),  # Sideslip / Sidewinder
}


def group_indices(affected):
    if "all" in affected:
        return list(range(N))
    out = []
    for name in affected:
        if name in INDEX:
            out.append(INDEX[name])
    return out


def build(co):
    d2d = co.get("DayToDayPower", {}) or {}
    attack = [0] * N
    defense = [0] * N
    range_delta = [0] * N
    conditional = [0] * N
    condition = "Always"

    for group in d2d.get("PowerIncreases", []):
        idxs = group_indices(group.get("AffectedUnits", []))
        atk = int(round(group.get("PowerIncrease", 0) * 100))
        dfn = int(round(group.get("DefenseIncrease", 0) * 100))
        rng = int(group.get("RangeIncrease", 0))
        terrain = group.get("ConditionalTerrain")
        for i in idxs:
            if terrain:
                conditional[i] += atk
            else:
                attack[i] += atk
            defense[i] += dfn
            range_delta[i] += rng
        if terrain:
            condition = {
                "Building": "UrbanOnly",
                "Road": "RoadOnly",
                "Plain": "PlainOnly",
            }.get(terrain, "Always")

    # Units that carry cargo, for COs that speed up transports.
    TRANSPORTS = ["APC", "T-Copter", "Lander", "Black Boat", "Cruiser", "Carrier"]
    move_delta = [0] * N

    manual = MANUAL.get(co["Name"])
    if manual:
        if manual.get("move_delta_transports"):
            for name in TRANSPORTS:
                move_delta[INDEX[name]] = manual["move_delta_transports"]
        condition = manual.get("condition", condition)
        bonus = manual.get("conditional_attack_all")
        if bonus is not None:
            conditional = [bonus] * N

    def stars(section):
        if not section:
            return -1
        return int(section.get("PowerStars", -1))

    cop_move, scop_move = POWER_MOVE.get(co["Name"], (0, 0))

    luck = d2d.get("LuckRange")
    luck_bad, luck_good = 0, 9
    if isinstance(luck, dict):
        # Stored as {"x": worst roll, "y": best roll}; x is negative for COs
        # that can roll badly (Flak, Jugger).
        luck_bad = abs(int(luck.get("x", 0)))
        luck_good = int(luck.get("y", 9))

    return {
        "name": co["Name"],
        "awbw_id": co["AWBWID"],
        "price": int(round(d2d.get("UnitPriceMultiplier", 1.0) * 100)),
        "attack": attack,
        "defense": defense,
        "range": range_delta,
        "conditional": conditional,
        "condition": condition,
        "luck_bad": luck_bad,
        "luck_good": luck_good,
        "fund_bonus": int(d2d.get("PropertyFundIncrease", 0)),
        "air_fuel": int(d2d.get("AirFuelUsageDecrease", 0)),
        "capture_pct": (manual or {}).get("capture_pct", 100),
        "move_delta": move_delta,
        "repair_bonus": (manual or {}).get("repair_bonus_hp100", 0),
        "cop_stars": stars(co.get("NormalPower")),
        "scop_stars": stars(co.get("SuperPower")),
        "cop_move": cop_move,
        "scop_move": scop_move,
        "modelled": co["Name"] in POWER_MOVE,
    }


def arr(values, typ):
    return "[" + ", ".join(str(v) for v in values) + f"]  as [{typ}; {N}]" if False else \
        "[" + ", ".join(str(v) for v in values) + "]"


def main():
    raw = SRC.read_text(encoding="utf-8")
    data = json.loads(re.sub(r"//[^\n]*", "", raw))
    cos = [build(v) for v in data.values()]
    cos.sort(key=lambda c: c["awbw_id"])

    lines = []
    w = lines.append
    w("// AUTO-GENERATED by tools/gen_cos.py -- do not edit by hand.")
    w("// Source: AWBW-Replay-Player's COs.json (day-to-day abilities only).")
    w("#![cfg_attr(rustfmt, rustfmt_skip)]")
    w("")
    w("use crate::data::NUM_UNIT_TYPES;")
    w("")
    w("/// When a CO's conditional attack bonus applies.")
    w("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
    w("pub enum AttackCondition {")
    w("    /// No conditional bonus.")
    w("    Always,")
    w("    /// Kindle: only while the attacker stands on a property.")
    w("    UrbanOnly,")
    w("    /// Koal: only on roads and bridges.")
    w("    RoadOnly,")
    w("    /// Jake: only on plains.")
    w("    PlainOnly,")
    w("    /// Lash: the bonus is per terrain star under the attacker.")
    w("    PerTerrainStar,")
    w("}")
    w("")
    w("/// A CO's day-to-day ability. Percentages are deltas on the vanilla 100.")
    w("#[derive(Debug, Clone, Copy)]")
    w("pub struct CoData {")
    w("    pub name: &'static str,")
    w("    pub awbw_id: u32,")
    w("    /// Build cost multiplier, as a percentage (100 = normal).")
    w("    pub price_multiplier_pct: u32,")
    w("    /// Attack bonus per unit type, in percentage points.")
    w("    pub attack: [i16; NUM_UNIT_TYPES],")
    w("    pub defense: [i16; NUM_UNIT_TYPES],")
    w("    pub range_delta: [i8; NUM_UNIT_TYPES],")
    w("    /// Extra attack that applies only when `condition` holds.")
    w("    pub conditional_attack: [i16; NUM_UNIT_TYPES],")
    w("    pub condition: AttackCondition,")
    w("    pub luck_bad_max: i32,")
    w("    pub luck_good_max: i32,")
    w("    /// Extra funds per property per day (Sasha).")
    w("    pub property_fund_bonus: u32,")
    w("    /// Fuel per turn air units save (Eagle).")
    w("    pub air_fuel_decrease: u8,")
    w("    /// Capture speed as a percentage of displayed HP (Sami captures at 150).")
    w("    pub capture_multiplier_pct: u32,")
    w("    /// Movement points added per unit type (Sami's transports gain one).")
    w("    pub move_delta: [i8; NUM_UNIT_TYPES],")
    w("    /// Extra HP repaired per turn, on the 0..=100 scale (Rachel).")
    w("    pub repair_bonus_hp100: u8,")
    w("    /// Stars a COP costs; -1 when the CO has no COP (Von Bolt).")
    w("    pub cop_stars: i8,")
    w("    /// Stars a SCOP costs; -1 when the CO has no SCOP.")
    w("    pub scop_stars: i8,")
    w("    /// All-unit movement gained during the COP / SCOP, for the COs")
    w("    /// whose power effects the engine models (Adder). Zero elsewhere;")
    w("    /// the universal +10/+10 during any power needs no data.")
    w("    pub cop_move_bonus: i8,")
    w("    pub scop_move_bonus: i8,")
    w("    /// Whether this CO's power *effects* are modelled. The meter and")
    w("    /// the universal +10/+10 work for every CO regardless.")
    w("    pub power_effects_modelled: bool,")
    w("}")
    w("")
    w("impl CoData {")
    w("    /// A CO with no day-to-day effect at all: the engine's default.")
    w("    pub const VANILLA: CoData = CoData {")
    w('        name: "Andy",')
    w("        awbw_id: 1,")
    w("        price_multiplier_pct: 100,")
    w(f"        attack: [0; NUM_UNIT_TYPES],")
    w(f"        defense: [0; NUM_UNIT_TYPES],")
    w(f"        range_delta: [0; NUM_UNIT_TYPES],")
    w(f"        conditional_attack: [0; NUM_UNIT_TYPES],")
    w("        condition: AttackCondition::Always,")
    w("        luck_bad_max: 0,")
    w("        luck_good_max: 9,")
    w("        property_fund_bonus: 0,")
    w("        air_fuel_decrease: 0,")
    w("        capture_multiplier_pct: 100,")
    w("        move_delta: [0; NUM_UNIT_TYPES],")
    w("        repair_bonus_hp100: 0,")
    w("        cop_stars: -1,")
    w("        scop_stars: -1,")
    w("        cop_move_bonus: 0,")
    w("        scop_move_bonus: 0,")
    w("        power_effects_modelled: false,")
    w("    };")
    w("}")
    w("")
    w(f"pub static COS: [CoData; {len(cos)}] = [")
    for c in cos:
        w("    CoData {")
        w(f'        name: "{c["name"]}", awbw_id: {c["awbw_id"]}, '
          f'price_multiplier_pct: {c["price"]},')
        w(f'        attack: {arr(c["attack"], "i16")},')
        w(f'        defense: {arr(c["defense"], "i16")},')
        w(f'        range_delta: {arr(c["range"], "i8")},')
        w(f'        conditional_attack: {arr(c["conditional"], "i16")},')
        w(f'        condition: AttackCondition::{c["condition"]},')
        w(f'        luck_bad_max: {c["luck_bad"]}, luck_good_max: {c["luck_good"]},')
        w(f'        property_fund_bonus: {c["fund_bonus"]}, air_fuel_decrease: {c["air_fuel"]},')
        w(f'        capture_multiplier_pct: {c["capture_pct"]},')
        w(f'        move_delta: {arr(c["move_delta"], "i8")},')
        w(f'        repair_bonus_hp100: {c["repair_bonus"]},')
        w(f'        cop_stars: {c["cop_stars"]}, scop_stars: {c["scop_stars"]}, '
          f'cop_move_bonus: {c["cop_move"]}, scop_move_bonus: {c["scop_move"]},')
        w(f'        power_effects_modelled: {"true" if c["modelled"] else "false"},')
        w("    },")
    w("];")
    w("")
    w("pub fn co_by_awbw_id(id: u32) -> Option<&'static CoData> {")
    w("    COS.iter().find(|c| c.awbw_id == id)")
    w("}")
    w("")
    w("pub fn co_by_name(name: &str) -> Option<&'static CoData> {")
    w("    COS.iter().find(|c| c.name.eq_ignore_ascii_case(name))")
    w("}")

    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {OUT} ({len(cos)} COs)")
    for c in cos:
        effects = []
        if c["price"] != 100:
            effects.append(f"cost {c['price']}%")
        if any(c["attack"]):
            effects.append(f"atk {min(c['attack'])}..{max(c['attack'])}")
        if any(c["defense"]):
            effects.append(f"def {min(c['defense'])}..{max(c['defense'])}")
        if any(c["conditional"]):
            effects.append(f"cond {c['condition']} +{max(c['conditional'])}")
        if (c["luck_bad"], c["luck_good"]) != (0, 9):
            effects.append(f"luck -{c['luck_bad']}..+{c['luck_good']}")
        if c["fund_bonus"]:
            effects.append(f"funds +{c['fund_bonus']}/property")
        if c["air_fuel"]:
            effects.append(f"air fuel -{c['air_fuel']}")
        if c["capture_pct"] != 100:
            effects.append(f"capture {c['capture_pct']}%")
        if any(c["move_delta"]):
            effects.append("transports +1 move")
        if c["repair_bonus"]:
            effects.append(f"repair +{c['repair_bonus']//10} HP")
        if c["cop_move"] or c["scop_move"]:
            effects.append(f"power move +{c['cop_move']}/+{c['scop_move']}")
        if not effects:
            effects.append("none modelled")
        stars = f"{c['cop_stars']}/{c['scop_stars']}*"
        print(f"  {c['name']:12} {stars:7} {', '.join(effects)}")


if __name__ == "__main__":
    main()
