"""Parse AWBW's terrain.php and units.php chart pages into clean JSON.

Inputs  (downloaded 2026-08-25): data/awbw-site/terrain.php.html, units.php.html
Outputs: data/units.json, data/terrain_chart.json

The damage table is NOT parsed from HTML; data/awbw-site/damage_inc.json is the
site's own data file and is used as-is.
"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "data" / "awbw-site"

# AWBW generic unit ids, as used by damage_inc.json and the Build action.
# Sources: RizeBot tests/helpers/fake-page.ts (from awbw db dump) plus the
# well-known AWBW ids for the four units that fixture omits.
GENERIC_IDS = {
    "Infantry": 1, "Mech": 2, "Md.Tank": 3, "Tank": 4, "Recon": 5,
    "APC": 6, "Artillery": 7, "Rocket": 8, "Anti-Air": 9, "Missile": 10,
    "Fighter": 11, "Bomber": 12, "B-Copter": 13, "T-Copter": 14,
    "Battleship": 15, "Cruiser": 16, "Lander": 17, "Sub": 18,
    "Black Boat": 28, "Carrier": 29, "Stealth": 30, "Neotank": 46,
    "Piperunner": 960900, "Black Bomb": 968731, "Mega Tank": 1141438,
}

CELL = re.compile(r"<td[^>]*>(?:<span[^>]*>)?\s*(.*?)\s*(?:</span>)?</td>", re.S)
ROW = re.compile(r"<tr>(.*?)</tr>", re.S)
IMG = re.compile(r"<img src=terrain/aw1/([a-z0-9\-_]+)\.gif>")


def cells_of(row_html):
    out = []
    for m in CELL.finditer(row_html):
        text = re.sub(r"<[^>]+>", "", m.group(1))
        out.append(text.replace("&nbsp;", " ").strip())
    return out


def parse_units():
    html = (SITE / "units.php.html").read_text(encoding="utf-8", errors="replace")
    units = {}
    for row in ROW.finditer(html):
        body = row.group(1)
        if "terrain/aw1/" not in body or "border-left" not in body:
            continue
        c = cells_of(body)
        # img, name, MP, ammo, fuel, fuel/turn, vision, range, move type, cost
        if len(c) < 10:
            continue
        name = c[1]
        lo, hi = c[7].split("-")
        # Sub/Stealth show "1 / 5*" style: normal cost / cost while dived-hidden.
        fuel_parts = [int(p.strip().rstrip("*")) for p in c[5].split("/")]
        units[name] = {
            "id": GENERIC_IDS[name],
            "move_points": int(c[2]),
            "max_ammo": int(c[3]),
            "max_fuel": int(c[4]),
            "fuel_per_turn": fuel_parts[0],
            "fuel_per_turn_hidden": fuel_parts[-1],
            "vision": int(c[6]),
            "range_min": int(lo),
            "range_max": int(hi),
            "move_type": c[8],
            "cost": int(c[9]),
        }
    return units


MOVE_TYPES = ["F", "B", "T", "W", "A", "S", "L", "P"]
WEATHERS = ["clear", "rain", "snow"]


def parse_terrain_clean():
    html = (SITE / "terrain.php.html").read_text(encoding="utf-8", errors="replace")
    out = []
    for row in ROW.finditer(html):
        body = row.group(1)
        img = IMG.search(body)
        if not img or "border-left" not in body:
            continue
        c = cells_of(body)
        # Expect: [img-cell(empty), defense, 24 x cost] => 26 cells, first is "".
        if len(c) != 26:
            continue
        defense = int(c[1])
        costs = {}
        for wi, weather in enumerate(WEATHERS):
            per = {}
            for mi, mt in enumerate(MOVE_TYPES):
                raw = c[2 + wi * 8 + mi]
                per[mt] = None if raw == "-" else int(raw)
            costs[weather] = per
        out.append({"gif": img.group(1), "defense": defense, "costs": costs})
    return out


def main():
    units = parse_units()
    terrain = parse_terrain_clean()
    (ROOT / "data" / "units.json").write_text(
        json.dumps(units, indent=2), encoding="utf-8")
    (ROOT / "data" / "terrain_chart.json").write_text(
        json.dumps(terrain, indent=2), encoding="utf-8")
    print(f"units: {len(units)}")
    for name in GENERIC_IDS:
        if name not in units:
            print(f"  MISSING unit: {name}")
    print(f"terrain rows: {len(terrain)}")
    for t in terrain:
        print(f"  {t['gif']}: def={t['defense']} clear={t['costs']['clear']}")


if __name__ == "__main__":
    main()
