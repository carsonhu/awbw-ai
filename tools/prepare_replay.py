"""Normalize AWBW replay zips into engine-friendly JSON for the Rust verifier.

Python owns the messy part (zip + gzip + PHP serialization + the AWBW map API);
the Rust side only ever sees a flat, documented schema.

Usage:
  python tools/prepare_replay.py <replay.zip> [-o out.json]
  python tools/prepare_replay.py --glob '<pattern>' --out-dir data/prepared [--limit N]

Map terrain is fetched from AWBW's map API and cached under data/maps/.
"""

import argparse
import glob as globlib
import gzip
import io
import json
import sys
import time
import urllib.request
import zipfile
from pathlib import Path

sys.path.insert(0, r"F:\awbw\awbw-replay-analyzer")
from parse_replays import PHP, parse_action_line  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
MAP_CACHE = ROOT / "data" / "maps"
MAP_API = "https://awbw.amarriner.com/api/map/map_info.php?maps_id={}"
CO_TABLE = json.loads((ROOT / "data" / "cos.json").read_text())

# countries_id -> AWBW country code. From the replay analyzer, which derived it
# from terrain ids observed in real games.
COUNTRY_CODE = {
    1: "os", 2: "bm", 3: "ge", 4: "yc", 5: "bh", 6: "rf", 7: "gs", 8: "bd",
    9: "ab", 10: "js", 16: "ci", 17: "pc", 19: "tg", 20: "pl", 21: "ar",
    22: "wn", 23: "aa", 24: "ne", 25: "sc", 26: "uw",
}


def decompress(raw):
    try:
        with gzip.GzipFile(fileobj=io.BytesIO(raw)) as gz:
            return gz.read().decode("latin-1")
    except Exception:
        import zlib
        return zlib.decompress(raw, 47).decode("latin-1")


def fetch_map(maps_id):
    MAP_CACHE.mkdir(parents=True, exist_ok=True)
    cached = MAP_CACHE / f"{maps_id}.json"
    if cached.exists():
        return json.loads(cached.read_text())
    with urllib.request.urlopen(MAP_API.format(maps_id), timeout=30) as r:
        data = json.loads(r.read().decode("utf-8"))
    cached.write_text(json.dumps(data))
    time.sleep(0.3)  # be polite to the site
    return data


def load_replay(path):
    with zipfile.ZipFile(path) as z:
        names = sorted(z.namelist(), key=len)
        states = decompress(z.read(names[0])).strip().split("\n")
        actions = decompress(z.read(names[1])).strip().split("\n")
    return states, actions


def unit_record(u):
    return {
        "id": u["id"],
        "type": u["name"],
        "player": u["players_id"],
        "x": u["x"],
        "y": u["y"],
        # AWBW stores HP as a 0-10 float with .1 granularity; we use 0-100.
        "hp100": int(round(float(u["hit_points"]) * 10)),
        "fuel": u["fuel"],
        "ammo": u["ammo"],
        "moved": bool(u["moved"]),
        "capture": u.get("capture", 0),
        "carried": u.get("carried") == "Y",
        "sub_dive": u.get("sub_dive") == "Y",
        "cargo": [c for c in (u.get("cargo1_units_id", 0), u.get("cargo2_units_id", 0)) if c],
    }


def normalize(path):
    states, actions = load_replay(path)
    head = PHP(states[0]).parse()
    maps_id = head["maps_id"]
    map_data = fetch_map(maps_id)

    width = map_data["Size X"]
    height = map_data["Size Y"]
    # The API returns [x][y]; transpose to row-major [y][x].
    tm = map_data["Terrain Map"]
    terrain = [[tm[x][y] for x in range(width)] for y in range(height)]

    players = []
    for p in head.get("players", {}).values():
        co_id = p.get("co_id")
        players.append({
            "id": p["id"],
            "order": p.get("order", 1),
            "country": COUNTRY_CODE.get(p.get("countries_id"), "os"),
            "co": co_id,
            "co_name": CO_TABLE.get(str(co_id), {}).get("name", f"CO{co_id}"),
            "team": str(p.get("team")),
        })
    players.sort(key=lambda p: p["order"])

    # Pair actions to snapshots by (player, day) rather than by line index: the
    # two files are usually parallel, but some replays are truncated or carry an
    # extra line, and a silent off-by-one would attribute one player's orders to
    # the other.
    by_turn = {}
    for line in actions:
        try:
            a = parse_action_line(line)
        except Exception:
            continue
        by_turn.setdefault((a["player_id"], a["day"]), a["actions"])

    turns = []
    unmatched = 0
    for i, line in enumerate(states):
        g = PHP(line).parse()
        key = (g["turn"], g["day"])
        act = by_turn.get(key)
        if act is None:
            act = []
            unmatched += 1
        turns.append({
            "day": g["day"],
            "active": g["turn"],
            "funds": {str(p["id"]): p["funds"] for p in g.get("players", {}).values()},
            "eliminated": {
                str(p["id"]): p.get("eliminated") == "Y"
                for p in g.get("players", {}).values()
            },
            "co_power_on": {
                str(p["id"]): p.get("co_power_on", "N")
                for p in g.get("players", {}).values()
            },
            "units": [unit_record(u) for u in g.get("units", {}).values()],
            "buildings": [
                {"x": b["x"], "y": b["y"], "terrain_id": b["terrain_id"],
                 "capture": b.get("capture", 20)}
                for b in g.get("buildings", {}).values()
            ],
            "actions": act,
        })

    return {
        "game_id": head["id"],
        "name": head.get("name"),
        "map_id": maps_id,
        "map_name": map_data.get("Name"),
        "width": width,
        "height": height,
        "terrain": terrain,
        "predeployed": map_data.get("Predeployed Units", []),
        "fog": head.get("fog") == "Y",
        "funds_per_property": head.get("funds", 1000),
        "starting_funds": head.get("starting_funds", 0),
        "capture_limit": head.get("capture_win"),
        "weather": head.get("weather_code", "C"),
        "use_powers": head.get("use_powers") == "Y",
        "players": players,
        "turns": turns,
        "unmatched_turns": unmatched,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("replay", nargs="?")
    ap.add_argument("--glob")
    ap.add_argument("-o", "--out")
    ap.add_argument("--out-dir")
    ap.add_argument("--limit", type=int, default=0)
    args = ap.parse_args()

    if args.glob:
        out_dir = Path(args.out_dir or ROOT / "data" / "prepared")
        out_dir.mkdir(parents=True, exist_ok=True)
        paths = sorted(globlib.glob(args.glob, recursive=True))
        if args.limit:
            paths = paths[: args.limit]
        ok = failed = 0
        for path in paths:
            try:
                data = normalize(path)
            except Exception as exc:
                failed += 1
                print(f"SKIP {Path(path).name}: {type(exc).__name__}: {exc}")
                continue
            dest = out_dir / f"{data['game_id']}.json"
            dest.write_text(json.dumps(data))
            ok += 1
        print(f"prepared {ok}, skipped {failed} -> {out_dir}")
        return

    data = normalize(args.replay)
    out = Path(args.out) if args.out else ROOT / "data" / "prepared" / f"{data['game_id']}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(data))
    print(f"{out}  ({data['width']}x{data['height']}, {len(data['turns'])} turns, "
          f"fog={data['fog']}, map={data['map_name']!r})")


if __name__ == "__main__":
    main()
