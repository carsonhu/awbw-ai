"""Downloads the AWBW sprites the local play client renders with.

The naming rules come from AWBW's own move planner (mirrored in
`data/awbw-site/moveplanner.js`): units are
`terrain/ani/{gs_ if moved}{country}{name}.gif`, buildings
`{country}{type}.gif`, HP badges `{1..9}.gif`. Terrain names come from
`data/terrain_ids.json` for exactly the ids the committed map uses, so the
board renders from the same table the engine was generated from.

Where a filename has plausible variants (buildings), each candidate is tried
and whichever answers 200 is recorded in `manifest.json` -- the client asks
the manifest, never guesses. Sprites are AWBW's art: fetched for local play,
kept out of git.

    py -3.12 tools/fetch_sprites.py
"""

import json
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "data" / "sprites"
BASE = "https://awbw.amarriner.com/terrain/ani/"
AGENT = "awbw-ai local play client (contact: carson8164@gmail.com)"

COUNTRIES = {"os": "os", "bm": "bm", "neutral": "neutral"}
BUILDING_TYPES = ["city", "base", "airport", "port", "comtower", "lab", "hq"]


def fetch(name):
    """Downloads one gif if not already present; True on success."""
    path = OUT / name
    if path.exists():
        return True
    request = urllib.request.Request(BASE + name,
                                     headers={"User-Agent": AGENT})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            data = response.read()
    except Exception:
        return False
    if not data.startswith(b"GIF"):
        return False
    path.write_bytes(data)
    time.sleep(0.25)
    return True


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    manifest = {"terrain": {}, "building": {}, "unit": {}, "misc": {}}

    # Terrain, exactly the ids the map uses.
    ids = json.loads((ROOT / "data" / "terrain_ids.json").read_text())
    game_map = json.loads((ROOT / "data" / "maps" / "119544.json").read_text())
    used = sorted({t for row in game_map["Terrain Map"] for t in row})
    for tid in used:
        name = ids[str(tid)]["name"].lower().replace(" ", "")
        if fetch(name + ".gif"):
            manifest["terrain"][str(tid)] = name + ".gif"
        else:
            print(f"  MISSING terrain {tid} ({name})")

    # Buildings in every ownership, for tiles that change hands in play.
    # AWBW has used both short and long country prefixes over the years;
    # try both and record the one that exists.
    for owner, code in [("neutral", ["neutral"]),
                        ("os", ["os", "orangestar"]),
                        ("bm", ["bm", "bluemoon"])]:
        for kind in BUILDING_TYPES:
            hit = next((p + kind + ".gif" for p in code
                        if fetch(p + kind + ".gif")), None)
            if hit:
                manifest["building"][f"{owner}:{kind}"] = hit
            elif not (owner == "neutral" and kind == "hq"):  # no neutral HQ
                print(f"  MISSING building {owner}:{kind}")

    # Units, both countries, moved (gs_) and not.
    units = json.loads((ROOT / "data" / "units.json").read_text())
    for unit in units:
        stem = unit.lower().replace(" ", "")
        for code in ("os", "bm"):
            for prefix in ("", "gs_"):
                name = prefix + code + stem + ".gif"
                if fetch(name):
                    manifest["unit"][f"{prefix}{code}:{unit}"] = name
                else:
                    print(f"  MISSING unit {name}")

    # HP digits and the capture icon.
    for n in range(1, 10):
        if fetch(f"{n}.gif"):
            manifest["misc"][str(n)] = f"{n}.gif"
    # capture.gif lives one directory up from ani/.
    request = urllib.request.Request(
        "https://awbw.amarriner.com/terrain/capture.gif",
        headers={"User-Agent": AGENT})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            (OUT / "capture.gif").write_bytes(response.read())
        manifest["misc"]["capture"] = "capture.gif"
    except Exception:
        print("  MISSING capture.gif")

    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=1))
    total = sum(len(v) for v in manifest.values())
    print(f"{total} sprites resolved into {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
