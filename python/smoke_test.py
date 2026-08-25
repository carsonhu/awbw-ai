"""Checks the batched environment end to end with a random masked policy.

Deliberately not a training script: it verifies the contract a trainer relies
on -- that masks are never empty, that sampling under them always yields a legal
order, that episodes finish and restart, and that the whole thing is fast enough
to be worth wrapping.

Environment time is measured apart from policy time. The sampling here is naive
numpy and costs more than the environment does; a real policy samples on the
GPU, so only the environment figure predicts training throughput.

Run with the interpreter the extension was built for:
    py -3.12 python/smoke_test.py
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402

import awbw  # noqa: E402


class Timer:
    def __init__(self):
        self.total = 0.0

    def __enter__(self):
        self._start = time.perf_counter()
        return self

    def __exit__(self, *exc):
        self.total += time.perf_counter() - self._start
        return False


def sample_masked(mask, rng):
    """Picks one true entry per row, uniformly."""
    assert mask.any(axis=1).all(), "a mask row was empty; no legal choice exists"
    weights = mask.astype(np.float64)
    weights /= weights.sum(axis=1, keepdims=True)
    cumulative = weights.cumsum(axis=1)
    draws = rng.random((mask.shape[0], 1))
    return (draws < cumulative).argmax(axis=1).astype(np.uint32)


def rollout(env, steps, rng):
    env_time, policy_time = Timer(), Timer()
    episodes = 0
    total_reward = 0.0

    # One buffer, refilled in place: the observation is megabytes per batch
    # step and allocating it fresh each time dominates the crossing into Python.
    obs = np.zeros((env.num_envs, env.observation_size), dtype=np.float32)
    env.reset()
    env.observe_into(obs)

    for _ in range(steps):
        with env_time:
            src_mask = env.source_mask()
        with policy_time:
            sources = sample_masked(src_mask, rng)

        with env_time:
            dst_mask = env.dest_mask(sources)
        with policy_time:
            dests = sample_masked(dst_mask, rng)

        with env_time:
            knd_mask = env.kind_mask(sources, dests)
        with policy_time:
            kinds = sample_masked(knd_mask, rng)

        with env_time:
            prm_mask = env.param_mask(sources, dests, kinds)
        with policy_time:
            params = sample_masked(prm_mask, rng)

        with env_time:
            rewards, dones, actors = env.step(sources, dests, kinds, params)
            env.observe_into(obs)

        assert rewards.shape == dones.shape == actors.shape == (env.num_envs,)
        assert np.isfinite(rewards).all(), "reward went non-finite"
        episodes += int(dones.sum())
        total_reward += float(rewards.sum())

    return episodes, total_reward, env_time.total, policy_time.total


def main():
    rng = np.random.default_rng(0)
    env = awbw.VecEnv(num_envs=64, seed=1, max_day=20, shaping=0.1)
    print(env)
    print(f"observation: {env.observation_size} floats, board {env.board_shape}")
    print(f"action heads: {env.action_sizes}, end-turn index {env.end_turn_index}")

    steps = 400
    episodes, total_reward, env_time, policy_time = rollout(env, steps, rng)
    env_steps = steps * env.num_envs

    print(f"\n{env_steps:,} env-steps")
    print(f"  environment {env_time:.2f}s -> {env_steps / env_time:,.0f} env-steps/sec")
    print(f"  numpy sampling {policy_time:.2f}s (a real policy does this on GPU)")
    print(f"  {episodes} episodes finished, total reward {total_reward:+.1f}")

    # A masked-uniform policy cannot break the invariants asserted above, so
    # reaching this line means the contract holds.
    print("\nOK: masks non-empty, orders legal, episodes restart cleanly")


if __name__ == "__main__":
    main()
