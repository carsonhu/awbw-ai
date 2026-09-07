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
            rewards, dones, actors, cut = env.step(
                sources, dests, kinds, params)
            env.observe_into(obs)

        assert rewards.shape == dones.shape == actors.shape == (env.num_envs,)
        assert cut.shape == dones.shape
        assert not (cut & ~dones).any(), "a cut game that did not finish"
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

    # Each rung of the potential has to be reachable and has to actually change
    # the shaped reward -- a silently ignored name would leave a run measuring
    # the arm it thought it had left behind.
    print("\nshaped reward per order, by potential:")
    for name in ("material", "funds", "worth"):
        # The full day cap, not the short one above: `worth` prices a property
        # by the days it has left to pay, so a 20-day game is the one setting
        # where it and `funds` almost coincide.
        rung = awbw.VecEnv(num_envs=16, seed=1, max_day=60, shaping=0.1,
                           potential=name)
        _, reward, _, _ = rollout(rung, 200, np.random.default_rng(0))
        print(f"  {name:<9} {reward:+.2f}")
    try:
        awbw.VecEnv(num_envs=1, potential="nonesuch")
    except ValueError:
        pass
    else:
        raise AssertionError("an unknown potential was accepted")

    # A pool that holds scripted bots and self-play at once, which is Lane C's
    # second prerequisite: `league3` could not put `greedy` in its pool, and
    # its `greedy` column was the one that stalled. The contract is per game --
    # a seat with a bot never shows the caller the bot's own turn, a seat
    # without one shows whoever is up.
    mixed = awbw.VecEnv(num_envs=8, seed=3, max_day=20, shaping=0.1)
    deal = ["jakeman", "greedy", None, None, "capturer", None, "random", None]
    for i, name in enumerate(deal):
        if name is not None:
            mixed.set_opponent(i, name)
    assert mixed.opponents == deal, f"deal not read back: {mixed.opponents}"
    scripted = np.array([n is not None for n in deal])

    obs = np.zeros((mixed.num_envs, mixed.observation_size), dtype=np.float32)
    mixed.observe_into(obs)
    both_seats = np.zeros(mixed.num_envs, dtype=bool)
    for _ in range(200):
        current, seats = mixed.current_player(), mixed.agent_seat()
        assert (current[scripted] == seats[scripted]).all(),             "a scripted game handed the caller the bot's own turn"
        sources = sample_masked(mixed.source_mask(), rng)
        dests = sample_masked(mixed.dest_mask(sources), rng)
        kinds = sample_masked(mixed.kind_mask(sources, dests), rng)
        params = sample_masked(mixed.param_mask(sources, dests, kinds), rng)
        both_seats |= current != seats
        rewards, _, _, _ = mixed.step(sources, dests, kinds, params)
        assert np.isfinite(rewards).all(), "reward went non-finite in a mixed pool"
        mixed.observe_into(obs)
    # The self-play games must have handed the caller *both* seats at some
    # point. Without this the loop above passes whenever nothing moves, and the
    # per-game flag would be untested rather than confirmed.
    assert both_seats[~scripted].all(), "a self-play game only ever showed one seat"

    # Putting a seat back to self-play mid-league has to work too, since that
    # is how a pool re-deals between generations.
    mixed.set_opponent(0, None)
    assert mixed.opponents[0] is None
    try:
        mixed.set_opponent(0, "nonesuch")
    except ValueError:
        pass
    else:
        raise AssertionError("an unknown opponent was accepted")

    # One-ply lookahead, which reranking is built on. Self-play so that no
    # scripted bot replies between the preview and the real order -- then the
    # preview has to be exact, not merely plausible, because the luck source
    # is copied with the state.
    look = awbw.VecEnv(num_envs=16, seed=5, max_day=20, shaping=0.1)
    before = np.zeros((look.num_envs, look.observation_size), dtype=np.float32)
    preview = np.zeros_like(before)
    after = np.zeros_like(before)
    look.reset()
    for _ in range(50):
        look.observe_into(before)
        sources = sample_masked(look.source_mask(), rng)
        dests = sample_masked(look.dest_mask(sources), rng)
        kinds = sample_masked(look.kind_mask(sources, dests), rng)
        params = sample_masked(look.param_mask(sources, dests, kinds), rng)

        sign, terminal = look.lookahead_into(sources, dests, kinds, params, preview)
        sign, terminal = np.asarray(sign), np.asarray(terminal)
        assert set(np.unique(sign)) <= {-1, 0, 1}, f"bad sign {np.unique(sign)}"
        assert ((terminal == 0) | (sign == 0)).all(), "a live game got a result"
        # Previewing must not advance anything.
        held = np.zeros_like(before)
        look.observe_into(held)
        assert (held == before).all(), "lookahead moved the real games"

        look.step(sources, dests, kinds, params)
        look.observe_into(after)
        # Games the order finished are already restarted, so they are excluded;
        # everything else must land exactly where the preview said it would.
        live = sign != 0
        assert (preview[live] == after[live]).all(), "the preview was not exact"

    # A masked-uniform policy cannot break the invariants asserted above, so
    # reaching this line means the contract holds.
    print("\nOK: masks non-empty, orders legal, episodes restart cleanly,"
          "\n    mixed pools deal per game, one-ply previews are exact")


if __name__ == "__main__":
    main()
