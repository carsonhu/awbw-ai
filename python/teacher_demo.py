"""Behaviour-cloning data, generated on the fly.

There is no dataset on disk. An observation is ~19k floats, so a million
samples would be seventy-odd gigabytes, while the engine regenerates them at
tens of thousands a second. The teacher just plays, continuously, and hands
back what it did.

    py -3.12 python/teacher_demo.py
"""

import sys
import time
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402

import awbw  # noqa: E402

KIND_NAMES = ["wait", "attack", "capture", "supply", "join", "load", "unload", "build"]


def main():
    env = awbw.TeacherEnv(num_envs=64, teacher="greedy", seed=7)
    print(env)
    print(f"map {env.map_name}, board {env.board_shape}, heads {env.action_sizes}")

    obs = np.zeros((env.num_envs, env.observation_size), dtype=np.float32)
    steps = int(sys.argv[1]) if len(sys.argv) > 1 else 300

    kinds = Counter()
    start = time.perf_counter()
    for _ in range(steps):
        env.observe_into(obs)
        targets = env.act()
        # (obs, targets) is the training pair: this position, this order.
        assert targets.shape == (env.num_envs, 4)
        kinds.update(targets[:, 2].tolist())
    elapsed = time.perf_counter() - start

    samples = steps * env.num_envs
    finished, seat0_rate = env.stats
    print(f"\n{samples:,} labelled samples in {elapsed:.2f}s")
    print(f"  {samples / elapsed:,.0f} samples/sec, no disk involved")
    print(f"  {finished} games completed, seat 0 won {seat0_rate:.0%}")

    print("\nwhat the teacher actually does:")
    for kind, n in sorted(kinds.items(), key=lambda kv: -kv[1]):
        print(f"  {KIND_NAMES[kind]:<8} {n / samples:>6.1%}")

    # A cloned policy can only be as good as its labels, so it is worth seeing
    # that the teacher does more than shuffle and end turns.
    share = kinds[0] / samples
    print(f"\nplain moves are {share:.0%} of orders; the rest is real play")


if __name__ == "__main__":
    main()
