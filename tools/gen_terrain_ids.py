"""Convert RizeBot's terrain-table.ts (generated from AWBW's DB dump) into
data/terrain_ids.json: awbw terrain_id -> {name, kind, defense, country, flags}.
"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = Path(r"F:\awbw\UnofficialAWBWRizeBot\src\awbw\terrain-table.ts")

ENTRY = re.compile(
    r'(\d+):\s*\{\s*id:\s*(\d+),\s*name:\s*"([^"]*)",\s*kind:\s*"([A-Z_]+)",'
    r'\s*defense:\s*(\d+),\s*country:\s*(null|"[a-z]+"),\s*isProperty:\s*(true|false),'
    r'\s*capturable:\s*(true|false),\s*producesIncome:\s*(true|false),\s*active:\s*(true|false)'
)


def main():
    text = SRC.read_text(encoding="utf-8")
    out = {}
    for m in ENTRY.finditer(text):
        tid = int(m.group(1))
        country = None if m.group(6) == "null" else m.group(6).strip('"')
        out[tid] = {
            "name": m.group(3),
            "kind": m.group(4),
            "defense": int(m.group(5)),
            "country": country,
            "is_property": m.group(7) == "true",
            "capturable": m.group(8) == "true",
            "produces_income": m.group(9) == "true",
            "active": m.group(10) == "true",
        }
    (ROOT / "data" / "terrain_ids.json").write_text(
        json.dumps(out, indent=1), encoding="utf-8")
    kinds = sorted({v["kind"] for v in out.values()})
    print(f"{len(out)} terrain ids, kinds: {kinds}")


if __name__ == "__main__":
    main()
