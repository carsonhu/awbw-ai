"""Correlate each replay state snapshot with the actions recorded for the same
turn, to pin down whether a snapshot precedes or follows its turn's actions."""

import gzip
import io
import json
import sys
import zipfile
from collections import Counter
from pathlib import Path

sys.path.insert(0, r"F:\awbw\awbw-replay-analyzer")
from parse_replays import PHP, parse_action_line  # noqa: E402


def decompress(raw):
    try:
        with gzip.GzipFile(fileobj=io.BytesIO(raw)) as gz:
            return gz.read().decode("latin-1")
    except Exception:
        import zlib
        return zlib.decompress(raw, 47).decode("latin-1")


def load(path):
    with zipfile.ZipFile(path) as z:
        names = sorted(z.namelist(), key=len)
        states = decompress(z.read(names[0])).strip().split("\n")
        actions = decompress(z.read(names[1])).strip().split("\n")
    return states, actions


def main():
    states, actions = load(Path(sys.argv[1]))
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else 6

    for i in range(min(limit, len(states))):
        g = PHP(states[i]).parse()
        units = g.get("units", {})
        by_player = Counter(u["players_id"] for u in units.values())
        funds = {p["id"]: p["funds"] for p in g.get("players", {}).values()}
        orders = {p["id"]: p.get("order") for p in g.get("players", {}).values()}

        print(f"--- state[{i}] day={g['day']} turn={g['turn']} "
              f"units={dict(by_player)} funds={funds} orders={orders}")

        try:
            a = parse_action_line(actions[i])
        except Exception as exc:
            print(f"    action parse failed: {exc}")
            continue
        kinds = Counter(x.get("action") for x in a["actions"])
        print(f"    action[{i}] player={a['player_id']} day={a['day']} "
              f"n={len(a['actions'])} kinds={dict(kinds)}")
        for x in a["actions"][:4]:
            kind = x.get("action")
            if kind == "Build":
                nu = x.get("newUnit", {})
                for pu in nu.values():
                    if isinstance(pu, dict):
                        for v in pu.values():
                            if isinstance(v, dict) and "units_name" in v:
                                print(f"      Build {v['units_name']} at "
                                      f"({v['units_x']},{v['units_y']}) "
                                      f"for player {v['units_players_id']}")
                                break
                        break
            elif kind == "End":
                info = x.get("updatedInfo", {})
                print(f"      End -> nextPId={info.get('nextPId')} "
                      f"nextFunds={info.get('nextFunds')} day={info.get('day')}")
            else:
                print(f"      {kind}: keys={sorted(x.keys())}")


if __name__ == "__main__":
    main()
