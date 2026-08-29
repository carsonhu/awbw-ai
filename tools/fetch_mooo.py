"""Game metadata from awbw.mooo.com: the victor, and each player's Elo.

The prepared corpus has no winner label -- replays record eliminations, and a
resignation eliminates nobody -- and no player ratings at all. mooo's search
carries both for every archived game on a map: the loser's cells wear class
`l` (line-through and a greyscale portrait under "show victor"), and the Elo
columns hold each player's rating when the game was played. Ratings make the
corpus stratifiable ("does the clone play like a 1400 or like a 700"), and
the victor unlocks value-head calibration against real outcomes.

Nine GET requests for the whole map archive, parsed with regexes against
markup that is stable enough to pin (the page is server-rendered, one <tr>
per game). Output is one JSON keyed by AWBW game id, so joining against
`data/prepared/<id>.json` is the filename.

    py -3.12 tools/fetch_mooo.py --query "a river supreme" \
        --out data/mooo-119544.json
"""

import argparse
import json
import re
import time
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASE = "http://awbw.mooo.com/search"

ROW = re.compile(r"<tr><td class=\"dC\">.*?</tr>", re.S)
GAME = re.compile(r"replay/(\d+)\.zip")
DATE = re.compile(r'class="dtC"[^>]*>(\d{4}-\d{2}-\d{2})')
DAYS = re.compile(r'class="daC">(\d+)')
# Sides in document order: CO cell then player cell then elo cell, twice.
# The loser's cells carry an extra `l` class; elo may be empty.
CO = re.compile(r'class="coC( l)?" data-sort="([a-z-]+)"')
PLAYER = re.compile(r'class="pC( l)?"><a[^>]*username=([^"&]+)')
ELO = re.compile(r'class="eC">(\d*)')


def parse_rows(page):
    out = {}
    for row in ROW.findall(page):
        game = GAME.search(row)
        if not game:
            continue
        cos = CO.findall(row)
        players = PLAYER.findall(row)
        elos = ELO.findall(row)
        if len(cos) != 2 or len(players) != 2 or len(elos) != 2:
            continue  # team games render differently; this corpus is 1v1
        date = DATE.search(row)
        days = DAYS.search(row)
        sides = []
        for i in range(2):
            lost = bool(cos[i][0]) or bool(players[i][0])
            sides.append({
                "name": urllib.parse.unquote(players[i][1]),
                "co": cos[i][1],
                "elo": int(elos[i]) if elos[i] else None,
                "lost": lost,
            })
        # Exactly one loser is a decided game; anything else is a draw or an
        # unfinished archive row, recorded as winner None rather than guessed.
        losses = sum(s["lost"] for s in sides)
        out[game.group(1)] = {
            "players": sides,
            "winner": (1 - [s["lost"] for s in sides].index(True)
                       if losses == 1 else None),
            "days": int(days.group(1)) if days else None,
            "date": date.group(1) if date else None,
        }
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--query", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--page-size", type=int, default=500)
    parser.add_argument("--max-pages", type=int, default=40)
    args = parser.parse_args()

    games = {}
    for page_index in range(args.max_pages):
        offset = page_index * args.page_size + (1 if page_index else 0)
        url = (f"{BASE}?q={urllib.parse.quote_plus(args.query)}"
               + (f"&offset={offset}" if page_index else ""))
        with urllib.request.urlopen(url, timeout=60) as response:
            page = response.read().decode("utf-8", errors="replace")
        found = parse_rows(page)
        new = {k: v for k, v in found.items() if k not in games}
        games.update(new)
        print(f"  offset {offset}: {len(found)} rows, {len(new)} new")
        if not new:
            break
        time.sleep(1)  # a small community site; be gentle

    path = ROOT / args.out
    path.write_text(json.dumps(games, indent=1), encoding="utf-8")
    rated = sum(1 for g in games.values()
                if all(s["elo"] is not None for s in g["players"]))
    decided = sum(1 for g in games.values() if g["winner"] is not None)
    print(f"{len(games)} games -> {args.out}: "
          f"{decided} with a victor, {rated} with both Elos")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
