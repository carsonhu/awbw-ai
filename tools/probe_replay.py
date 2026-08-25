"""Probe an AWBW replay's structure: how many state lines, what keys they carry,
and whether map terrain is present."""

import gzip
import io
import sys
import zipfile
from pathlib import Path

sys.path.insert(0, r"F:\awbw\awbw-replay-analyzer")
from parse_replays import PHP  # noqa: E402


def decompress(raw):
    try:
        with gzip.GzipFile(fileobj=io.BytesIO(raw)) as gz:
            return gz.read().decode("latin-1")
    except Exception:
        import zlib
        return zlib.decompress(raw, 47).decode("latin-1")


def main():
    path = Path(sys.argv[1])
    with zipfile.ZipFile(path) as z:
        names = sorted(z.namelist(), key=len)
        state_raw = decompress(z.read(names[0]))
        action_raw = decompress(z.read(names[1]))

    state_lines = state_raw.strip().split("\n")
    action_lines = action_raw.strip().split("\n")
    print(f"state lines:  {len(state_lines)}")
    print(f"action lines: {len(action_lines)}")

    first = PHP(state_lines[0]).parse()
    print(f"\ntop-level keys: {sorted(first.keys())}")
    print(f"\nmaps_id={first.get('maps_id')} day={first.get('day')} "
          f"turn={first.get('turn')} fog={first.get('fog')} "
          f"funds={first.get('funds')} capture_win={first.get('capture_win')} "
          f"weather={first.get('weather_code')} starting_funds={first.get('starting_funds')}")

    units = first.get("units", {})
    print(f"units in first state: {len(units)}")
    for u in list(units.values())[:3]:
        print("  ", {k: u[k] for k in ("id", "name", "x", "y", "hit_points", "fuel",
                                       "moved", "players_id") if k in u})

    bldgs = first.get("buildings", {})
    print(f"buildings in first state: {len(bldgs)}")
    for b in list(bldgs.values())[:3]:
        print("  ", b)

    # Does any state line carry raw terrain?
    print(f"\n'terrain' appears in state blob: {'terrain' in state_raw}")

    # Day progression across state lines.
    days = []
    for line in state_lines:
        try:
            g = PHP(line).parse()
            days.append((g.get("day"), g.get("turn"), len(g.get("units", {}))))
        except Exception as exc:
            days.append(("ERR", str(exc)[:40], 0))
    print(f"\n(day, active_player, unit_count) per state line:")
    for d in days:
        print("  ", d)


if __name__ == "__main__":
    main()
