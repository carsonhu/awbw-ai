"""Turns a recorded engine game into a real AWBW replay file.

This is `prepare_replay.py` run backwards, and it keeps the same split: the
engine emits a flat log of what happened (`VecEnv(record=True)`), and everything
AWBW-specific lives here -- PHP serialization, the per-viewer action payloads,
gzip, zip. The output opens in AWBW's own replay viewers, which is the point:
watching a checkpoint play is far more informative than a win rate, and nobody
should have to write a viewer to do it.

AWBW's format is two gzipped members in a zip. `<id>` holds one PHP-serialized
`awbwGame` per turn, snapshotted at the turn's start; `a<id>` holds one line per
turn, `p:<player>;d:<day>;a:<php>`, whose payloads are JSON strings. Those
payloads are written once per viewer plus a `global` copy, for fog; these games
are no-fog, so every copy is the same object.

    python tools/write_replay.py game.json -o replays/
    python python/record_games.py --checkpoint checkpoints/ppo.pt   # the usual way
"""

import argparse
import gzip
import json
import re
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UNITS = json.loads((ROOT / "data" / "units.json").read_text())
TERRAIN = json.loads((ROOT / "data" / "terrain_ids.json").read_text())

# AWBW numbers players by a global id, not a seat. Any distinct pair works for a
# replay nobody will upload; these are picked to be obviously synthetic.
BASE_PLAYER_ID = 900_000

# `users_id` is the exception: it has to be a real account. A replay stores no
# username at all -- AWBW Replay Player scrapes `profile.php?users_id=N` for
# one -- so an invented id resolves to nothing and the viewer reports that it
# could not download usernames. These are the project owner's own accounts.
DEFAULT_USERS = [117993, 117993]  # DarthNoob7
BASE_UNIT_ID = 800_000
BASE_BUILDING_ID = 700_000

# Seat -> country. Orange Star and Blue Moon are what A River Supreme's own
# terrain ids say the two sides are.
COUNTRIES = ["os", "bm"]
COUNTRY_ID = {"os": 1, "bm": 2}
STAMP = "2026-01-01 00:00:00"

# Capture points a property starts with, counting down as it is taken. AWBW and
# the engine agree on twenty.
CAPTURE_FULL = 20

# `symbol` means two different things in the two files, which is worth stating
# plainly because writing one where the other belongs looks like nothing.
#
# In a game state it is the unit's *domain*: G on the ground, S at sea, M in the
# air, derived from the movement type. In an action payload it is a per-unit-type
# letter instead. Real replays are unambiguous on both.
DOMAINS = {"A": "M", "L": "S", "S": "S"}

# The action payload's letter follows no rule anyone can see -- near-alphabetical
# by unit id, but Fighter onwards is off by one and Neotank is nowhere near.
# These are the ones real replays actually show; anything else falls back to a
# first letter, which is wrong but cosmetic.
SYMBOLS = {
    "Infantry": "A", "Mech": "B", "Md.Tank": "C", "Tank": "D", "Recon": "E",
    "APC": "F", "Artillery": "G", "Rocket": "H", "Anti-Air": "I",
    "Fighter": "L", "Bomber": "M", "B-Copter": "N", "Neotank": "T",
}

# The engine's variant names against AWBW's. Only the irregular ones are listed;
# everything else already agrees, and no one rule covers APC, Md.Tank and
# B-Copter at once.
UNIT_NAMES = {
    "Apc": "APC",
    "MdTank": "Md.Tank",
    "AntiAir": "Anti-Air",
    "BCopter": "B-Copter",
    "TCopter": "T-Copter",
    "BlackBoat": "Black Boat",
    "BlackBomb": "Black Bomb",
    "MegaTank": "Mega Tank",
}


class PhpObject:
    """A record that serializes as a PHP object rather than a plain array.

    AWBW stores its rows as `awbwGame`, `awbwPlayer`, `awbwUnit` and
    `awbwBuilding` objects. The difference from an associative array is only the
    class name, but readers do check it.
    """

    def __init__(self, cls, fields):
        self.cls = cls
        self.fields = fields


def php(value):
    """PHP's serialize(), which is what AWBW stores."""
    if isinstance(value, PhpObject):
        body = "".join(php(k) + php(v) for k, v in value.fields.items())
        return f'O:{len(value.cls)}:"{value.cls}":{len(value.fields)}:{{{body}}}'
    if value is None:
        return "N;"
    if isinstance(value, bool):
        return f"b:{int(value)};"
    if isinstance(value, int):
        return f"i:{value};"
    if isinstance(value, float):
        return f"d:{value};"
    if isinstance(value, str):
        # Lengths are in bytes, and AWBW's files are latin-1.
        return f's:{len(value.encode("latin-1", "replace"))}:"{value}";'
    if isinstance(value, dict):
        body = "".join(php(k) + php(v) for k, v in value.items())
        return f"a:{len(value)}:{{{body}}}"
    if isinstance(value, list):
        body = "".join(php(i) + php(v) for i, v in enumerate(value))
        return f"a:{len(value)}:{{{body}}}"
    raise TypeError(f"cannot serialize {type(value).__name__}")


# (kind, country) -> AWBW terrain id. The engine names kinds in CamelCase and
# the table in SCREAMING_SNAKE, so `ComTower` has to become `COM_TOWER` and not
# `COMTOWER`. Built once: this is looked up per building per turn.
TERRAIN_BY_KIND = {
    (info["kind"], info["country"]): int(tid) for tid, info in TERRAIN.items()
}


def terrain_id(kind, owner):
    """The AWBW terrain id for a property of this kind and owner."""
    snake = re.sub(r"(?<=[a-z])(?=[A-Z])", "_", kind).upper()
    country = COUNTRIES[owner] if owner is not None else None
    try:
        return TERRAIN_BY_KIND[(snake, country)]
    except KeyError:
        raise KeyError(f"no terrain id for {kind} ({snake}) owned by {country}")


def player_id(seat):
    return BASE_PLAYER_ID + seat


class Writer:
    """Builds one replay's two files from a recorded game."""

    def __init__(self, log, game_id, map_id, name, users=None):
        self.log = log
        self.users = list(users or DEFAULT_USERS)
        self.game_id = game_id
        self.map_id = map_id
        self.name = name
        self.seats = sorted({t["active"] for t in log["turns"]} | {0, 1})
        # What each property's capture stood at before the most recent capture
        # *event*, which is not the same as the previous snapshot: snapshots
        # alternate players, so comparing against one makes the marker flicker
        # off on every other turn. This persists until the tile is captured
        # again, which is what keeps the marker up across the opponent's turn.
        self.previous = {}
        # A property's id has to be stable across turns, or a viewer sees every
        # building replaced each time the board is snapshotted.
        first = log["turns"][0]
        self.building_ids = {
            (b["x"], b["y"]): BASE_BUILDING_ID + i
            for i, b in enumerate(first["buildings"])
        }

    def user(self, seat):
        """The AWBW account a seat is attributed to."""
        return self.users[seat % len(self.users)]

    # ── records ──────────────────────────────────────────────────────────────

    @staticmethod
    def capturing(u, properties):
        """Whether this unit is part-way through taking the tile it stands on.

        AWBW carries this on the *unit*, and as a flag rather than a count --
        the progress itself lives on the building -- and it is what a viewer
        draws the capture marker from. The engine tracks only the building, so
        it is derived: a property under someone else's flag, part captured.
        """
        prop = properties.get((u["x"], u["y"]))
        if prop is None or prop["capture"] >= CAPTURE_FULL:
            return 0
        return int(prop["owner"] != u["player"])

    def unit(self, u, capturing=0):
        """One `awbwUnit`, in AWBW's own field names."""
        name = UNIT_NAMES.get(u["type"], u["type"])
        stats = UNITS[name]
        return {
            "id": BASE_UNIT_ID + u["id"],
            "games_id": self.game_id,
            "players_id": player_id(u["player"]),
            "name": name,
            "movement_points": stats["move_points"],
            "vision": stats["vision"],
            "fuel": u["fuel"],
            "fuel_per_turn": stats["fuel_per_turn"],
            "sub_dive": "N",
            "ammo": u["ammo"],
            "short_range": stats["range_min"],
            "long_range": stats["range_max"],
            "second_weapon": "N",
            "symbol": DOMAINS.get(stats["move_type"], "G"),
            "cost": stats["cost"],
            "movement_type": stats["move_type"],
            "x": u["x"],
            "y": u["y"],
            "moved": int(bool(u["moved"])),
            "capture": capturing,
            "fired": 0,
            # A snapshot keeps HP to a tenth -- AWBW's 0-10 scale is the engine's
            # 0-100 divided by ten, exactly, and rounding here to the *displayed*
            # whole number loses the sub-point HP that decides whether the next
            # attack kills. Action payloads do round; see `unit_payload`.
            "hit_points": u["hp100"] / 10.0,
            "cargo1_units_id": BASE_UNIT_ID + u["cargo"][0] if u["cargo"] else 0,
            "cargo2_units_id": BASE_UNIT_ID + u["cargo"][1] if len(u["cargo"]) > 1 else 0,
            "carried": "Y" if u["carried"] else "N",
        }

    def unit_payload(self, u, capturing=0):
        """The same unit as an *action* carries it.

        Action payloads prefix every column with `units_`, lead with the id
        under the key `"0"`, and add the country, which the row itself does not
        hold. Same data, a different table join.
        """
        name = UNIT_NAMES.get(u["type"], u["type"])
        record = {"0": BASE_UNIT_ID + u["id"]}
        record.update({f"units_{k}": v for k, v in self.unit(u, capturing).items()})
        # Unlike a snapshot, an action reports the HP a player sees, rounded up:
        # a unit on 6.7 shows as 7 and is destroyed at 0. And its `symbol` is
        # the per-type letter rather than the domain; see DOMAINS.
        record["units_hit_points"] = -(-u["hp100"] // 10)
        record["units_symbol"] = SYMBOLS.get(name, name[0])
        record["countries_code"] = COUNTRIES[u["player"]]
        return record

    def per_viewer(self, value):
        """AWBW writes one copy per player plus a global one, for fog."""
        out = {"global": value}
        for seat in self.seats:
            out[str(player_id(seat))] = value
        return out

    def discovered(self):
        return {str(player_id(s)): None for s in self.seats}

    def capture_info(self, order):
        """What a capture did to the property, as AWBW reports it.

        A capture that *finishes* is a different record, not a smaller number:
        it carries the new terrain and owner, and a viewer flips the tile the
        moment it reads them. Without that it has to wait for the next snapshot,
        so the property changes hands at the end of the turn rather than under
        the unit that took it.
        """
        tile = (order["x"], order["y"])
        bid = self.building_ids.get(tile, BASE_BUILDING_ID)
        if not order.get("captured"):
            return {
                "buildings_capture": order["remaining"],
                "buildings_id": bid,
                "buildings_x": order["x"],
                "buildings_y": order["y"],
                "buildings_team": None,
            }
        tid = terrain_id(order["terrain"], order["owner"])
        owner = player_id(order["owner"])
        return {
            "0": bid,
            "buildings_id": bid,
            "buildings_x": order["x"],
            "buildings_y": order["y"],
            "buildings_capture": 0,
            "terrain_id": tid,
            "terrain_name": TERRAIN[str(tid)]["name"],
            "terrain_defense": TERRAIN[str(tid)]["defense"],
            "buildings_players_id": owner,
            "buildings_team": str(owner),
        }

    # ── action payloads ──────────────────────────────────────────────────────

    def move(self, order, capturing=0):
        """The Move half of an order, or `[]` when the unit never left its tile.

        AWBW writes an empty array rather than omitting the key, which is the
        same quirk that cost this project half of every capture on the way in.
        """
        path = order.get("path") or []
        if len(path) <= 1:
            return []
        return {
            "action": "Move",
            "unit": self.per_viewer(self.unit_payload(order["unit"], capturing)),
            "paths": {"global": [
                {"unit_visible": True, "x": p["x"], "y": p["y"]} for p in path
            ]},
            "dist": len(path) - 1,
            "trapped": False,
            "discovered": self.discovered(),
        }

    def payload(self, order, turn):
        kind = order["kind"]
        if kind == "Move":
            moved = self.move(order)
            # A unit told to stay put still has to be reported as having acted.
            return moved if moved != [] else {
                "action": "Move",
                "unit": self.per_viewer(self.unit_payload(order["unit"])),
                "paths": {"global": [{
                    "unit_visible": True,
                    "x": order["unit"]["x"],
                    "y": order["unit"]["y"],
                }]},
                "dist": 0,
                "trapped": False,
                "discovered": self.discovered(),
            }
        if kind == "Capt":
            return {
                "action": "Capt",
                # The mover is capturing by definition, and a viewer takes the
                # marker from the unit rather than from the order.
                "Move": self.move(order, capturing=1),
                "Capt": {
                    "action": "Capt",
                    "buildingInfo": self.capture_info(order),
                    "vision": {"global": {
                        "onCapture": {"x": order["x"], "y": order["y"]}}},
                    "income": None,
                },
            }
        if kind == "Fire":
            attacker = self.unit_payload(order["unit"])
            defender = (self.unit_payload(order["defender"])
                        if order.get("defender") else None)
            return {
                "action": "Fire",
                "Move": self.move(order),
                "Fire": {
                    "action": "Fire",
                    "combatInfoVision": {"global": {
                        "hasVision": True,
                        "combatInfo": {"attacker": attacker, "defender": defender},
                    }},
                    "copValues": {
                        "attacker": {"playerId": player_id(order["unit"]["player"]),
                                     "copValue": 0, "tagValue": None},
                        "defender": {"playerId": player_id(
                            order["defender"]["player"]) if order.get("defender")
                            else player_id(turn["active"]),
                            "copValue": 0, "tagValue": None},
                    },
                },
            }
        if kind == "Build":
            return {
                "action": "Build",
                "newUnit": {"global": self.unit_payload(order["unit"])},
                "discovered": self.discovered(),
            }
        if kind == "Load":
            return {
                "action": "Load",
                "Move": self.move(order),
                "Load": {
                    "action": "Load",
                    "loaded": {"global": BASE_UNIT_ID + order["unit"]["id"]},
                    "transport": {"global": BASE_UNIT_ID + (order["transport"] or 0)},
                },
            }
        if kind == "Unload":
            return {
                "action": "Unload",
                "unit": self.per_viewer(self.unit_payload(order["unit"])),
                "transportID": BASE_UNIT_ID + order["transport"],
                "discovered": self.discovered(),
            }
        if kind == "Join":
            return {
                "action": "Join",
                "Move": self.move(order),
                "Join": {
                    "action": "Join",
                    "newFunds": {"global": turn["funds"][turn["active"]]},
                    "playerId": player_id(turn["active"]),
                    # The unit that survives the merge, and the id of the one
                    # that walked in and stopped existing.
                    "unit": {"global": self.unit_payload(order["into"])},
                    "joinID": {"global": BASE_UNIT_ID + order["unit"]["id"]},
                },
            }
        if kind == "Supply":
            return {
                "action": "Supply",
                "Move": self.move(order),
                "Supply": {
                    "action": "Supply",
                    "unit": {"global": BASE_UNIT_ID + order["unit"]["id"]},
                    "rows": [],
                    "supplied": {str(player_id(s)): [] for s in self.seats},
                },
            }
        if kind == "End":
            return {
                "action": "End",
                # These keys, in this order, and nothing else: a viewer reads
                # `nextWeather` as a *code*, so the word "Clear" is rejected
                # where "C" is fine. The round trip through our own parser
                # cannot catch that -- it ignores the field entirely.
                "updatedInfo": {
                    "event": "NextTurn",
                    "nextPId": player_id(order["next"]),
                    "nextFunds": {"global": order["funds"][order["next"]]},
                    "nextTimer": 0,
                    "nextWeather": "C",
                    "supplied": self.per_viewer([]),
                    "repaired": self.per_viewer([]),
                    "day": order["day"],
                },
            }
        raise ValueError(f"unknown order kind {kind!r}")

    # ── files ────────────────────────────────────────────────────────────────

    def player(self, seat, turn):
        return {
            "id": player_id(seat),
            "users_id": self.user(seat),
            "games_id": self.game_id,
            "countries_id": COUNTRY_ID[COUNTRIES[seat]],
            "co_id": 1,
            "funds": turn["funds"][seat],
            "turn": None,
            "email": None,
            "uniq_id": None,
            "eliminated": "N",
            "last_read": STAMP,
            "last_read_broadcasts": None,
            "emailpress": None,
            "signature": None,
            "co_power": 0,
            "co_power_on": "N",
            "order": seat + 1,
            "accept_draw": "N",
            "co_max_power": 90000,
            "co_max_spower": 180000,
            "co_image": "andy.png",
            "team": str(player_id(seat)),
            "aet_count": 0,
            "turn_start": STAMP,
            "turn_clock": 0,
            "tags_co_id": None,
            "tags_co_power": None,
            "tags_co_max_power": None,
            "tags_co_max_spower": None,
            "interface": "N",
        }

    def state_line(self, turn):
        properties = {(b["x"], b["y"]): b for b in turn["buildings"]}
        game = {
            "id": self.game_id,
            "name": self.name,
            "password": None,
            "creator": self.user(0),
            "start_date": STAMP,
            "end_date": None,
            "activity_date": STAMP,
            "maps_id": self.map_id,
            "weather_type": "Clear",
            "weather_start": None,
            "weather_code": "C",
            "win_condition": None,
            "turn": player_id(turn["active"]),
            "day": turn["day"],
            "active": "Y",
            "funds": 1000,
            "capture_win": 1000,
            "fog": "N",
            "comment": None,
            "type": "N",
            "boot_interval": -1,
            "starting_funds": 0,
            "official": "N",
            "min_rating": 0,
            "max_rating": None,
            "league": None,
            "team": "N",
            "aet_interval": -1,
            "aet_date": None,
            "use_powers": "N",
            "players": [
                PhpObject("awbwPlayer", self.player(s, turn)) for s in self.seats
            ],
            "buildings": [
                PhpObject("awbwBuilding", {
                    "id": self.building_ids.get((b["x"], b["y"]), BASE_BUILDING_ID),
                    "games_id": self.game_id,
                    "terrain_id": terrain_id(b["kind"], b["owner"]),
                    "x": b["x"],
                    "y": b["y"],
                    "capture": b["capture"],
                    # What it was on the previous snapshot, not a copy of the
                    # current value. A viewer draws the capture marker from
                    # `capture != last_capture` -- equal values mean nothing is
                    # in progress -- so writing the same number twice hides
                    # every capture in the game.
                    "last_capture": self.previous.get(
                        (b["x"], b["y"]), CAPTURE_FULL),
                    "last_updated": STAMP,
                })
                for b in turn["buildings"]
            ],
            # Carried units stay in the list, flagged, rather than being left
            # out: the transport points at them by id, and a reader that cannot
            # resolve those ids loses the passengers the moment one unloads.
            "units": [
                PhpObject("awbwUnit", self.unit(u, self.capturing(u, properties)))
                for u in turn["units"]
            ],
            "timers_initial": 0,
            "timers_increment": 0,
            "timers_max_turn": 0,
        }
        return PhpObject("awbwGame", game)

    def action_line(self, turn, number):
        payloads = [json.dumps(self.payload(o, turn), separators=(",", ":"))
                    for o in turn["orders"]]
        pid = player_id(turn["active"])
        return f"p:{pid};d:{turn['day']};a:" + php({0: pid, 1: number, 2: payloads})

    def advance_captures(self, turn):
        """Folds a turn's capture orders into what `last_capture` will report."""
        running = {(b["x"], b["y"]): b["capture"] for b in turn["buildings"]}
        for order in turn["orders"]:
            if order["kind"] != "Capt":
                continue
            tile = (order["x"], order["y"])
            self.previous[tile] = running.get(tile, CAPTURE_FULL)
            running[tile] = order["remaining"]

    def write(self, out_dir):
        states, actions = [], []
        for number, turn in enumerate(self.log["turns"], start=1):
            states.append(php(self.state_line(turn)))
            actions.append(self.action_line(turn, number))
            self.advance_captures(turn)

        path = Path(out_dir) / f"{self.game_id}.zip"
        path.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
            z.writestr(str(self.game_id), gz("\n".join(states) + "\n"))
            z.writestr(f"a{self.game_id}", gz("\n".join(actions) + "\n"))
        return path


def gz(text):
    return gzip.compress(text.encode("latin-1", "replace"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", help="JSON from VecEnv.take_replay")
    parser.add_argument("-o", "--out", default="replays")
    parser.add_argument("--game-id", type=int, default=999001)
    parser.add_argument("--map-id", type=int, default=119544)
    parser.add_argument("--name", default="awbw-ai")
    parser.add_argument("--users", default=None,
                        help="comma-separated AWBW users_id, one per seat")
    args = parser.parse_args()

    log = json.loads(Path(args.log).read_text())
    users = [int(u) for u in args.users.split(',')] if args.users else None
    path = Writer(log, args.game_id, args.map_id, args.name, users).write(args.out)
    print(f"wrote {path} ({len(log['turns'])} turns)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
