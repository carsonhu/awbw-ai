"""Peek at the structure of an AWBW replay zip."""

import gzip
import sys
import zipfile
from pathlib import Path


def load(path):
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        out = {}
        for name in names:
            with z.open(name) as f:
                out[name] = gzip.decompress(f.read())
    return out


def main():
    path = Path(sys.argv[1])
    members = load(path)
    for name, blob in members.items():
        print(f"=== {name}: {len(blob)} bytes ===")
        text = blob.decode("utf-8", errors="replace")
        print(text[:1500])
        print("...")
        print(text[-600:])
        print()


if __name__ == "__main__":
    main()
