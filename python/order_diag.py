"""How much of the source head's error is picking a different order, not a wrong one?

A turn is fourteen orders on average and their sequence is largely arbitrary: if
eight units still need moving and any of them may go next, a policy that spreads
its mass correctly over all eight is still marked wrong on seven of them. Top-1
accuracy cannot tell that apart from not knowing which units matter.

So score the same predictions two ways. *Exact* is the usual thing — did the
policy name the unit the human moved next. *In-turn* asks whether it named a
unit the human moved somewhere in this same turn. The gap between them is the
part of the error that is bookkeeping rather than judgement.

    py -3.12 python/order_diag.py --checkpoint checkpoints/bc-scaled.pt
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import awbw  # noqa: E402
import evaluate as ev  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", default="checkpoints/bc-scaled.pt")
    parser.add_argument("--envs", type=int, default=32)
    parser.add_argument("--steps", type=int, default=1500)
    parser.add_argument("--map-name", default="A River Supreme")
    parser.add_argument("--seed", type=int, default=5)
    args = parser.parse_args()

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    env = awbw.ReplayTeacher(
        replay_dir=str(ROOT / "data" / "prepared"), num_envs=args.envs,
        map_name=args.map_name, seed=args.seed, holdout=0.12, validation=True,
    )
    policy = ev.load(ROOT / args.checkpoint, env, device)
    end_turn = env.end_turn_index

    obs = torch.empty((args.envs, env.observation_size), dtype=torch.float32,
                      pin_memory=device.type == "cuda")
    view = obs.numpy()

    # Per slot, the current turn's predictions and the sources the human used.
    # Scored only once the turn is complete, since "somewhere in this turn"
    # includes orders that have not been read yet.
    pending = [[] for _ in range(args.envs)]
    used = [set() for _ in range(args.envs)]
    where = [-1] * args.envs
    exact = in_turn = counted = 0
    chance = 0
    end_turn_rows = 0
    rng = np.random.default_rng(args.seed)

    def flush(slot):
        # Scored against a uniform pick under the same mask. A player issues
        # about fourteen orders with about thirteen units, so nearly every
        # legal source is used at some point in a turn -- without this baseline
        # a high in-turn rate would say nothing at all.
        nonlocal exact, in_turn, counted, chance
        for predicted, actual, random_pick in pending[slot]:
            counted += 1
            exact += predicted == actual
            in_turn += predicted in used[slot]
            chance += random_pick in used[slot]
        pending[slot].clear()
        used[slot].clear()

    for _ in range(args.steps):
        env.observe_into(view)
        turns = env.turn_index()
        raw_mask = env.source_mask()
        with torch.no_grad():
            features, _, pooled = policy.trunk(obs.to(device))
            logits = policy.source_logits(features, pooled)
            # Under the engine's own mask, as at play time.
            mask = torch.from_numpy(raw_mask).to(device)
            predicted = logits.masked_fill(~mask, float("-inf")).argmax(1)
        predicted = predicted.cpu().numpy()
        # Gumbel-max: a uniform draw over the allowed entries, vectorised.
        noise = rng.gumbel(size=raw_mask.shape)
        random_picks = np.where(raw_mask, noise, -np.inf).argmax(axis=1)

        codes, valid = env.act()
        for slot in range(args.envs):
            if turns[slot] != where[slot]:
                flush(slot)
                where[slot] = turns[slot]
            if not valid[slot]:
                continue
            actual = int(codes[slot, 0])
            if actual == end_turn:
                end_turn_rows += 1
                continue
            pending[slot].append((int(predicted[slot]), actual,
                                  int(random_picks[slot])))
            used[slot].add(actual)
    for slot in range(args.envs):
        flush(slot)

    n = max(counted, 1)
    print(f"{counted} orders scored ({end_turn_rows} end-turns excluded, "
          f"they name no unit)")
    print(f"  exact    {exact / n:.1%}   the unit the human moved *next*")
    print(f"  in-turn  {in_turn / n:.1%}   a unit the human moved this turn")
    print(f"  chance   {chance / n:.1%}   a uniform pick under the same mask")
    print()
    print(f"  gap over exact   {(in_turn - exact) / n:+.1%}  "
          f"right unit, different order")
    print(f"  edge over chance {(in_turn - chance) / n:+.1%}  "
          f"how much the in-turn number is worth")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
