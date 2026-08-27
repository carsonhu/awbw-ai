"""Rates a checkpoint against a fixed panel, always the same one.

A self-play run's own score is against a moving opponent, and a ladder's
internal head-to-head only says a policy beats what it just played: generation
four beat generation one 93.5% while scoring 22.8% against `greedy`, where the
checkpoint it grew from scores 65.1%. Nothing inside the loop could see that,
because the loop only ever plays its own recent past.

So a claim that a checkpoint is better is a claim about this panel, not about
whatever it trained on. Keep the members fixed; add to them rarely, and never
drop one because a checkpoint does badly on it — that is the measurement
working.

    py -3.12 python/panel.py --checkpoint checkpoints/ppo-adder3.pt --co Adder
"""
import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Two scripted bots, the imitation clone as an off-distribution human-ish
# style, and the strongest checkpoint that predates the self-play lineage.
# `bc-powers-scaled2` earns its place: it rates 2% against `greedy` and still
# takes a third of the games off the best self-play policy.
PANEL = [
    ("greedy", "bot"),
    ("jakeman", "bot"),
    ("checkpoints/bc-powers-scaled2.pt", "net"),
    ("checkpoints/ppo-adder3.pt", "net"),
]


def rate(checkpoint, member, kind, games, co, decide_cap, temperature):
    cmd = [sys.executable, str(ROOT / "python" / "evaluate.py"),
           "--checkpoint", checkpoint, "--games", str(games),
           "--temperature", str(temperature)]
    cmd += ["--opponent", member] if kind == "bot" else ["--versus", member]
    if co:
        cmd += ["--co", co]
    if decide_cap:
        cmd += ["--decide-cap"]
    out = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT).stdout
    for line in out.splitlines():
        if line.strip().startswith("score "):
            return line.strip()[len("score "):]
    return "no score (see evaluate.py output)"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", required=True)
    parser.add_argument("--games", type=int, default=200,
                        help="per member; 200 is the floor a rating needs")
    parser.add_argument("--co", default="Adder")
    parser.add_argument("--temperature", type=float, default=1.0)
    parser.add_argument("--no-decide-cap", action="store_true",
                        help="draw day-capped games instead of settling them")
    args = parser.parse_args()

    name = Path(args.checkpoint).stem
    print(f"{name} on the panel, {args.games} games each, "
          f"co {args.co or 'vanilla'}\n")
    for member, kind in PANEL:
        if kind == "net" and Path(member).stem == name:
            print(f"  {'(itself)':<24} skipped")
            continue
        if kind == "net" and not (ROOT / member).exists():
            print(f"  {Path(member).stem:<24} missing")
            continue
        label = member if kind == "bot" else Path(member).stem
        score = rate(args.checkpoint, member, kind, args.games, args.co,
                     not args.no_decide_cap, args.temperature)
        print(f"  {label:<24} {score}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
