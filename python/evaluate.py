"""Rates a policy by making it play, not by asking how well it predicts.

Imitation accuracy answers "does it guess what a human did"; it does not answer
"can it play". A policy can be right about the eighty percent of orders that are
obvious and wrong about every one that decides the game. So this puts a
checkpoint on the board against a fixed scripted opponent and counts wins.

Seats alternate across the batch, so a policy is rated on both sides rather than
on whichever side of the map is better. Every choice is sampled under the
engine's own masks, so an illegal order is not possible — what is being measured
is judgement, not legality.

    py -3.12 python/evaluate.py --checkpoint checkpoints/bc-human.pt
    py -3.12 python/evaluate.py --policy random          # the floor
    py -3.12 python/evaluate.py --policy greedy          # the mirror, ~50%
"""

import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import awbw  # noqa: E402
import net as netmod  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


def pick(logits, mask, rng: np.random.Generator, temperature: float):
    """Chooses one index per row, only where the mask allows.

    Sampling rather than taking the best: a deterministic policy against a
    deterministic opponent plays the same game every time, and forty identical
    games measure one game.
    """
    logits = logits.float().masked_fill(~mask, float("-inf"))
    if temperature <= 0:
        return logits.argmax(dim=1)
    probabilities = torch.softmax(logits / temperature, dim=1)
    # A row whose mask is empty cannot happen -- end-turn is always legal -- but
    # a NaN here would be silent, so fall back rather than trust it.
    bad = ~torch.isfinite(probabilities).all(dim=1)
    if bad.any():
        probabilities[bad] = mask[bad].float()
    return torch.multinomial(probabilities, 1).squeeze(1)


def as_mask(array, device):
    return torch.from_numpy(array).to(device)


def as_orders(chosen):
    """One head's choices, in the width the engine decodes."""
    return chosen.to(torch.int32).cpu().numpy().astype(np.uint32)


class Net:
    """Plays by sampling the four heads under the engine's masks."""

    def __init__(self, policy, device, temperature: float):
        self.policy = policy
        self.device = device
        self.temperature = temperature

    @torch.no_grad()
    def choose(self, env, obs, rng):
        features, flat, pooled = self.policy.trunk(obs)

        source = pick(
            self.policy.source_logits(features, pooled),
            as_mask(env.source_mask(), self.device), rng, self.temperature,
        )
        s = source.to(torch.int32).cpu().numpy().astype(np.uint32)

        dest = pick(
            self.policy.dest_logits(features, flat, pooled, source),
            as_mask(env.dest_mask(s), self.device), rng, self.temperature,
        )
        d = dest.to(torch.int32).cpu().numpy().astype(np.uint32)

        context = self.policy.context_of(flat, pooled, source, dest)
        kind = pick(
            self.policy.kind_logits(context),
            as_mask(env.kind_mask(s, d), self.device), rng, self.temperature,
        )
        k = kind.to(torch.int32).cpu().numpy().astype(np.uint32)

        param = pick(
            self.policy.param_logits(features, context, kind),
            as_mask(env.param_mask(s, d, k), self.device), rng, self.temperature,
        )
        p = param.to(torch.int32).cpu().numpy().astype(np.uint32)
        return s, d, k, p


class Duel:
    """Two checkpoints sharing a board, one to a seat.

    The observation is written from the *moving* player's point of view, so both
    networks read a position the same way and neither has to be told which side
    it is on. All that is decided per row is whose choice gets submitted.

    Both networks are run on the whole batch and the rows are then selected
    between, rather than splitting the batch: each head's mask depends on the
    choice the previous head actually made, so the two sides have to advance in
    step. It costs a second forward pass to keep that simple.
    """

    def __init__(self, mine, theirs, env, device):
        self.mine = mine
        self.theirs = theirs
        self.device = device
        self.seat = torch.from_numpy(env.agent_seat()).to(device)

    @torch.no_grad()
    def choose(self, env, obs, rng):
        ours = torch.from_numpy(env.current_player()).to(self.device) == self.seat
        a = self.mine.policy.trunk(obs)
        b = self.theirs.policy.trunk(obs)

        def blend(ours_logits, theirs_logits, mask):
            return torch.where(
                ours,
                pick(ours_logits, mask, rng, self.mine.temperature),
                pick(theirs_logits, mask, rng, self.theirs.temperature),
            )

        source = blend(
            self.mine.policy.source_logits(a[0], a[2]),
            self.theirs.policy.source_logits(b[0], b[2]),
            as_mask(env.source_mask(), self.device),
        )
        s = as_orders(source)
        dest = blend(
            self.mine.policy.dest_logits(a[0], a[1], a[2], source),
            self.theirs.policy.dest_logits(b[0], b[1], b[2], source),
            as_mask(env.dest_mask(s), self.device),
        )
        d = as_orders(dest)

        ours_context = self.mine.policy.context_of(a[1], a[2], source, dest)
        their_context = self.theirs.policy.context_of(b[1], b[2], source, dest)
        kind = blend(
            self.mine.policy.kind_logits(ours_context),
            self.theirs.policy.kind_logits(their_context),
            as_mask(env.kind_mask(s, d), self.device),
        )
        k = as_orders(kind)
        param = blend(
            self.mine.policy.param_logits(a[0], ours_context, kind),
            self.theirs.policy.param_logits(b[0], their_context, kind),
            as_mask(env.param_mask(s, d, k), self.device),
        )
        return s, d, k, as_orders(param)


class Uniform:
    """The floor: uniform over whatever the masks allow."""

    @staticmethod
    def _draw(mask, rng):
        # Gumbel-max: the argmax of iid Gumbel noise over the allowed entries is
        # a uniform draw among them, and it vectorises over the whole batch
        # where `rng.choice` per row does not.
        noise = rng.gumbel(size=mask.shape)
        return np.where(mask, noise, -np.inf).argmax(axis=1).astype(np.uint32)

    def choose(self, env, obs, rng):
        s = self._draw(env.source_mask(), rng)
        d = self._draw(env.dest_mask(s), rng)
        k = self._draw(env.kind_mask(s, d), rng)
        p = self._draw(env.param_mask(s, d, k), rng)
        return s, d, k, p


def load(path, env, device):
    saved = torch.load(path, map_location=device, weights_only=True)
    config = saved["config"]
    policy = netmod.Policy(
        planes=config["planes"],
        globals_=config["globals"],
        height=config["height"],
        width=config["width"],
        head_sizes=config["head_sizes"],
        channels=config["channels"],
        blocks=config["blocks"],
    ).to(device)
    policy.load_state_dict(saved["policy"])
    policy.eval()
    if list(config["head_sizes"]) != list(env.action_sizes):
        raise SystemExit(
            f"checkpoint expects heads {config['head_sizes']}, "
            f"environment has {list(env.action_sizes)}"
        )
    return policy


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", default="checkpoints/bc-human.pt")
    parser.add_argument("--policy", default="net",
                        choices=["net", "random", "greedy", "capturer"],
                        help="what plays the agent's seat")
    parser.add_argument("--opponent", default="greedy",
                        choices=["greedy", "jakeman", "capturer", "random"])
    # The scripted ladder tops out -- `greedy` is beaten 96% -- so rating a
    # checkpoint against another checkpoint is the only way to keep measuring
    # once self-play starts. Seats still alternate, so this is a fair pairing
    # and two copies of one file should come out at half.
    parser.add_argument("--versus", default=None,
                        help="checkpoint to play the other seat, instead of a bot")
    parser.add_argument("--versus-temperature", type=float, default=None,
                        help="defaults to --temperature")
    parser.add_argument("--games", type=int, default=200)
    parser.add_argument("--envs", type=int, default=50)
    parser.add_argument("--max-day", type=int, default=60)
    # Must match the training run being rated. A capped game is half a point
    # undecided and a whole one either way decided, so the two settings put a
    # stalling policy several points apart.
    parser.add_argument("--decide-cap", action="store_true",
                        help="settle a day-capped game on income, then property count")
    parser.add_argument("--temperature", type=float, default=1.0,
                        help="0 for the best order every time")
    parser.add_argument("--seed", type=int, default=3)
    args = parser.parse_args()

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    rng = np.random.default_rng(args.seed)
    torch.manual_seed(args.seed)

    if args.policy in ("greedy", "capturer"):
        # A scripted mirror: the same bot on both seats, as a sanity check that
        # the board and the seating are fair before reading anything into a
        # policy's number.
        env = awbw.TeacherEnv(num_envs=args.envs, teacher=args.policy,
                              seed=args.seed, max_day=args.max_day)
        start = time.perf_counter()
        while env.stats[0] < args.games:
            env.act()
        played, rate = env.stats
        print(f"{args.policy} mirror: {played} games, seat 0 won {rate:.1%} "
              f"({time.perf_counter() - start:.0f}s)")
        return 0

    # With a checkpoint on the other seat there is no scripted opponent at all:
    # the caller moves both sides, and `agent_seat` says which rows are ours.
    env = awbw.VecEnv(num_envs=args.envs, seed=args.seed, max_day=args.max_day,
                      decide_cap=args.decide_cap,
                      opponent=None if args.versus else args.opponent)
    if args.policy == "net":
        agent = Net(load(ROOT / args.checkpoint, env, device), device,
                    args.temperature)
    else:
        agent = Uniform()
    if args.versus:
        if args.policy != "net":
            raise SystemExit("--versus needs --policy net on this seat")
        other = Net(load(ROOT / args.versus, env, device), device,
                    args.versus_temperature
                    if args.versus_temperature is not None else args.temperature)
        agent = Duel(agent, other, env, device)

    obs = torch.empty((args.envs, env.observation_size), dtype=torch.float32,
                      pin_memory=device.type == "cuda")
    view = obs.numpy()

    versus = Path(args.versus).stem if args.versus else args.opponent
    print(f"{args.policy} vs {versus} on {env.map_name}, "
          f"{args.games} games, day cap {args.max_day}")
    start = time.perf_counter()
    steps = 0
    while env.results[0] < args.games:
        env.observe_into(view)
        s, d, k, p = agent.choose(env, obs.to(device, non_blocking=True), rng)
        env.step(s, d, k, p)
        steps += args.envs
        if steps % (args.envs * 2000) == 0:
            played, won, drawn = env.results
            print(f"  {played} games, {won} won, {drawn} drawn")

    played, won, drawn = env.results
    lost = played - won - drawn
    elapsed = time.perf_counter() - start
    print(f"\n{played} games in {elapsed:.0f}s ({steps / elapsed:,.0f} orders/sec)")
    print(f"  won   {won:>4}  ({won / played:.1%})")
    print(f"  drawn {drawn:>4}  ({drawn / played:.1%})   -- almost all the day cap")
    print(f"  lost  {lost:>4}  ({lost / played:.1%})")

    # Draws count half, which is the usual convention and stops a policy that
    # only stalls from looking like one that only loses.
    score = (won + 0.5 * drawn) / played
    # Games are independent, so the score is a mean of independent draws and its
    # error falls as 1/sqrt(n). Printed because the differences worth chasing
    # here are a few points wide, and a hundred games cannot resolve those --
    # without this the next run's noise reads as progress.
    error = (score * (1 - score) / played) ** 0.5
    print(f"  score {score:.1%} +- {error:.1%}  ({played} games)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
