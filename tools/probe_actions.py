"""Dump one full example of each distinct action type across a set of replays,
so the engine's action translator can be written against real payloads."""

import glob
import gzip
import io
import json
import sys
import zipfile
from collections import Counter

sys.path.insert(0, r"F:\awbw\awbw-replay-analyzer")
from parse_replays import parse_action_line  # noqa: E402


def decompress(raw):
    try:
        with gzip.GzipFile(fileobj=io.BytesIO(raw)) as gz:
            return gz.read().decode("latin-1")
    except Exception:
        import zlib
        return zlib.decompress(raw, 47).decode("latin-1")


def main():
    pattern = sys.argv[1]
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else 40

    seen = {}
    counts = Counter()
    files = glob.glob(pattern, recursive=True)[:limit]
    for path in files:
        try:
            with zipfile.ZipFile(path) as z:
                names = sorted(z.namelist(), key=len)
                lines = decompress(z.read(names[1])).strip().split("\n")
        except Exception:
            continue
        for line in lines:
            try:
                turn = parse_action_line(line)
            except Exception:
                continue
            for a in turn["actions"]:
                kind = a.get("action", "?")
                counts[kind] += 1
                if kind not in seen:
                    seen[kind] = a

    print("action frequencies:")
    for kind, n in counts.most_common():
        print(f"  {kind:12} {n}")
    print()
    for kind, example in sorted(seen.items()):
        text = json.dumps(example, indent=1)
        if len(text) > 1800:
            text = text[:1800] + "\n ...(truncated)"
        print(f"===== {kind} =====")
        print(text)
        print()


if __name__ == "__main__":
    main()
