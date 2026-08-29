"""Downloads replay zips from awbw.mooo.com, slowly.

Reads a metadata file written by fetch_mooo.py, downloads each game's zip
that is not already prepared or already downloaded, and verifies every file
opens as a zip before counting it. One request every few seconds with an
identifying User-Agent: this is somebody's community archive, and 364 games
do not need to arrive quickly.

    py -3.12 tools/fetch_mooo_replays.py --meta data/mooo-t4.json \
        --out data/replays-mooo --delay 4
"""

import argparse
import json
import time
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
URL = "http://awbw.mooo.com/replay/{}.zip"
AGENT = "awbw-ai research corpus builder (contact: carson8164@gmail.com)"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--meta", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--delay", type=float, default=4.0)
    parser.add_argument("--prepared", default="data/prepared")
    args = parser.parse_args()

    meta = json.loads((ROOT / args.meta).read_text())
    out = ROOT / args.out
    out.mkdir(parents=True, exist_ok=True)
    prepared = {p.stem for p in (ROOT / args.prepared).glob("*.json")}

    todo = [g for g in sorted(meta)
            if g not in prepared and not (out / f"{g}.zip").exists()]
    print(f"{len(meta)} in metadata, {len(todo)} to download")

    good = bad = 0
    for i, game in enumerate(todo, 1):
        path = out / f"{game}.zip"
        request = urllib.request.Request(URL.format(game),
                                         headers={"User-Agent": AGENT})
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                path.write_bytes(response.read())
            if zipfile.is_zipfile(path):
                good += 1
            else:
                path.unlink()
                bad += 1
                print(f"  {game}: not a zip, dropped")
        except Exception as error:  # noqa: BLE001 - log and continue
            bad += 1
            print(f"  {game}: {error}")
        if i % 25 == 0:
            print(f"  {i}/{len(todo)} ({good} ok, {bad} failed)")
        time.sleep(args.delay)

    print(f"done: {good} downloaded, {bad} failed, into {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
