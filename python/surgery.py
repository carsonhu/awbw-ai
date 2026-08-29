"""Carries a checkpoint across an observation change, instead of orphaning it.

Every plane-count change so far has invalidated every checkpoint at once --
three or four times now, the most expensive event class in the project. OpenAI
Five faced the same problem and built weight transfer instead of restarting
("surgery"), and for *added* planes the surgery is the easiest case there is:
widen the stem convolution and zero-initialise the slice that reads the new
channels. Zero weights make the new planes invisible, so the widened network
computes exactly what the old one did until training moves it -- verified here
by playing both against the same observations.

Limits, stated plainly: this covers additions. A change that *redefines*
existing planes (threat v1 -> v2 replaced four planes with six) is a different
observation, and no loader can transfer what was learned from channels that no
longer mean the same thing.

New planes are assumed appended after the old ones, before the globals --
which is where every plane addition so far has gone (threat planes after
`plane::COUNT`).

    py -3.12 python/surgery.py --checkpoint checkpoints/old.pt \
        --planes 70 --out checkpoints/old-widened.pt
"""

import argparse
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "python"))

import torch  # noqa: E402


def widen_planes(saved, new_planes):
    """A checkpoint dict widened to read `new_planes`, function unchanged.

    The stem convolution's input is [planes | globals], so the new channels
    are spliced in at the end of the plane block and the globals' weights
    shift up unchanged. The new slice is zeros: the widened stem's output on
    any observation equals the old stem's output on the same observation
    minus the new planes, whatever those planes contain.
    """
    config = dict(saved["config"])
    old_planes = config["planes"]
    if new_planes < old_planes:
        raise SystemExit(
            f"cannot narrow {old_planes} planes to {new_planes}; surgery "
            "covers additions only")
    if new_planes == old_planes:
        return saved

    state = dict(saved["policy"])
    weight = state["stem.0.weight"]  # (channels, old_planes + globals, 3, 3)
    added = new_planes - old_planes
    patch = torch.zeros(
        (weight.shape[0], added, *weight.shape[2:]),
        dtype=weight.dtype,
    )
    state["stem.0.weight"] = torch.cat(
        [weight[:, :old_planes], patch, weight[:, old_planes:]], dim=1)

    config["planes"] = new_planes
    out = dict(saved)
    out["policy"] = state
    out["config"] = config
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", required=True)
    parser.add_argument("--planes", type=int, required=True,
                        help="plane count of the new observation layout")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    saved = torch.load(ROOT / args.checkpoint, map_location="cpu",
                       weights_only=True)
    old = saved["config"]["planes"]
    widened = widen_planes(saved, args.planes)
    torch.save(widened, ROOT / args.out)
    print(f"{args.checkpoint}: {old} -> {args.planes} planes, "
          f"function unchanged, saved {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
