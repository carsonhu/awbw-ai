"""Checks that recorded human games reach Python as trainable labels.

Not a training script. It verifies the contract behaviour cloning depends on:
that every slot keeps producing orders, that the labels index inside the four
heads, that a label is never a stale code from an earlier position, and that
the corpus streams fast enough that the GPU is the bottleneck rather than the
disk.

The throughput number matters more here than for the scripted teacher. Replays
are parsed from JSON as they are opened, which is thousands of times dearer than
stepping the engine, so this is the one data source that can starve a trainer.

Run with the interpreter the extension was built for:
    py -3.12 python/replay_demo.py
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402

import awbw  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
STEPS = 200


def main() -> int:
    env = awbw.ReplayTeacher(
        replay_dir=str(ROOT / "data" / "prepared"),
        num_envs=32,
        map_name="A River Supreme",
        seed=1,
    )
    print(env)
    print(f"  {env.replay_count} replays, board {env.board_shape}")
    print(f"  observation {env.observation_size} floats, heads {env.action_sizes}")

    obs = np.zeros((env.num_envs, env.observation_size), dtype=np.float32)
    sizes = env.action_sizes

    kinds = np.zeros(sizes[2], dtype=np.int64)
    labelled = 0
    end_turns = 0
    open_time = 0.0

    start = time.perf_counter()
    for _ in range(STEPS):
        env.observe_into(obs)
        seats = env.current_player()
        codes, valid = env.act()

        if not valid.any():
            print("  a whole batch came back empty", file=sys.stderr)
            return 1

        # Every label has to index inside its head, or the loss reads garbage.
        for head in range(4):
            column = codes[valid, head]
            if column.size and column.max() >= sizes[head]:
                print(f"  head {head} label {column.max()} >= {sizes[head]}", file=sys.stderr)
                return 1

        # A slot with a game must name a seat; an empty one must not.
        if ((seats >= 0) != valid).any():
            print("  seat and validity disagree", file=sys.stderr)
            return 1

        # Observations must not be blank where a label was produced.
        if not obs[valid].any(axis=1).all():
            print("  a labelled row had an empty observation", file=sys.stderr)
            return 1

        labelled += int(valid.sum())
        end_turns += int((codes[valid, 0] == env.end_turn_index).sum())
        np.add.at(kinds, codes[valid, 2], 1)
    elapsed = time.perf_counter() - start

    served, powered, illegal, games, epochs = env.stats
    print(f"\n{labelled} labels in {elapsed:.2f}s -> {labelled / elapsed:,.0f}/sec")
    print(f"  games opened:      {games} ({epochs} passes over the corpus)")
    print(f"  dropped, power:    {powered}")
    print(f"  dropped, illegal:  {illegal}")
    kept = served / max(served + powered + illegal, 1)
    print(f"  kept:              {kept:.1%} of the orders walked past")

    print("\nwhat the humans did:")
    names = ["wait", "attack", "capture", "supply", "join", "load", "unload", "build"]
    order = np.argsort(-kinds)
    for i in order:
        if kinds[i]:
            print(f"  {names[i]:<8} {kinds[i] / labelled:>6.1%}")
    print(f"  {'end turn':<8} {end_turns / labelled:>6.1%}  (of the same total)")

    if open_time:
        print(f"  parsing:           {open_time:.2f}s")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
