"""Does the value head know who is winning? Scored against real outcomes.

The first instrument in this project whose ground truth is human games rather
than bots or the policy's own ancestors. Positions come from held-out corpus
games via `ReplayTeacher`, the label is the recorded winner
(`data/game-meta-119544.json`), and the value head's tanh-scale output is read
as a win probability. Reported by game phase — a critic that only knows who
won *after* the armies are traded is a different instrument from one that
reads an opening — and by the players' Elo where the archive has it.

Two numbers per bucket. *Accuracy*: sign agreement between the value and the
outcome. *Brier*: mean squared error of the implied probability, which
punishes confident wrongness the way accuracy cannot.

    py -3.12 python/value_diag.py --checkpoint checkpoints/bc-net2.pt
"""

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "python"))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import awbw  # noqa: E402
import net as netmod  # noqa: E402

PHASES = ((0.0, 0.25), (0.25, 0.5), (0.5, 0.75), (0.75, 1.01))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", required=True)
    parser.add_argument("--meta", default="data/game-meta-119544.json")
    parser.add_argument("--batch", type=int, default=64)
    parser.add_argument("--steps", type=int, default=1500)
    parser.add_argument("--holdout", type=float, default=0.12)
    parser.add_argument("--map-name", default="A River Supreme")
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--elo-split", type=int, default=900,
                        help="boundary between the low and high Elo buckets")
    args = parser.parse_args()

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    saved = torch.load(ROOT / args.checkpoint, map_location=device,
                       weights_only=True)
    policy = netmod.from_config(saved["config"]).to(device)
    policy.load_state_dict(saved["policy"])
    policy.eval()

    meta = json.loads((ROOT / args.meta).read_text(encoding="utf-8"))
    winners = {int(g): e["winner_index"] for g, e in meta.items()
               if e.get("winner_index") is not None}
    elos = {int(g): e["elo"] for g, e in meta.items() if e.get("elo")}

    # The same held-out split bc.py trains against, seed included, so no
    # position scored here was ever a training label. The env must emit the
    # plane count the checkpoint expects.
    threat = saved["config"]["planes"] > 64
    env = awbw.ReplayTeacher(
        replay_dir=str(ROOT / "data" / "prepared"), num_envs=args.batch,
        map_name=args.map_name, seed=args.seed + 1, holdout=args.holdout,
        validation=True, threat=threat,
    )
    if env.observation_size != policy.planes * policy.tiles + policy.globals_:
        raise SystemExit("checkpoint and environment disagree on layout")

    obs = torch.empty((args.batch, env.observation_size), dtype=torch.float32,
                      pin_memory=device.type == "cuda")
    view = obs.numpy()

    # Progress through a game: count the orders served from each game and
    # normalise by that game's own total afterwards. Robust to the day cap
    # and to games of very different lengths, and needs nothing decoded out
    # of the observation.
    rows = []  # (game, value, won, day_fraction)
    per_game_orders = {}
    with torch.no_grad():
        for _ in range(args.steps):
            env.observe_into(view)
            ids = env.game_ids()
            movers = env.current_player()
            values = policy.value_of(obs.to(device)).cpu().numpy()
            env.act()
            for gid, mover, value in zip(ids, movers, values):
                winner = winners.get(int(gid))
                if winner is None or mover < 0:
                    continue
                order_index = per_game_orders.get(int(gid), 0)
                per_game_orders[int(gid)] = order_index + 1
                rows.append((int(gid), float(value),
                             1.0 if winner == mover else 0.0, order_index))

    # Second pass: order index -> fraction of that game's served span.
    samples = []
    for gid, value, won, order_index in rows:
        total = per_game_orders[gid]
        prob = 0.5 * (np.tanh(value) + 1.0)
        samples.append((gid, prob, won, order_index / max(total - 1, 1)))

    def report(name, chosen):
        if not chosen:
            print(f"  {name:<18} no samples")
            return
        prob = np.array([s[1] for s in chosen])
        won = np.array([s[2] for s in chosen])
        accuracy = float((((prob > 0.5) == (won > 0.5))).mean())
        brier = float(((prob - won) ** 2).mean())
        print(f"  {name:<18} n={len(chosen):>6}  accuracy {accuracy:.3f}  "
              f"brier {brier:.3f}")

    games = {s[0] for s in samples}
    print(f"\n{Path(args.checkpoint).stem}: {len(samples)} positions from "
          f"{len(games)} held-out games ({len(winners)} labeled in meta)")
    print("\nby game phase (fraction of the game's orders):")
    for lo, hi in PHASES:
        report(f"  {lo:.0%}-{min(hi, 1.0):.0%}",
               [s for s in samples if lo <= s[3] < hi])
    print("\nby the game's mean Elo (seat-aligned games only):")
    low = [s for s in samples if s[0] in elos
           and sum(elos[s[0]]) / 2 < args.elo_split]
    high = [s for s in samples if s[0] in elos
            and sum(elos[s[0]]) / 2 >= args.elo_split]
    report(f"  under {args.elo_split}", low)
    report(f"  {args.elo_split} and up", high)
    print("\nbaseline: predicting 0.5 everywhere scores brier 0.250, "
          "accuracy 0.500")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
