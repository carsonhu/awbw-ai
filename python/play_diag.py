"""What a checkpoint builds, and whether the fights it picks are worth taking.

A win rate says a policy got better, and `order_diag.py` says how much of its
imitation error is bookkeeping. Neither says whether the play is *sound*. Two
questions this answers, both read from recorded games rather than asked of the
network:

*Composition* -- what it spends money on. A policy that never builds an
indirect has never had to learn indirect play, which decides whether a CO whose
powers change indirect range (Jake, Grit) is an increment on a skill it has or
a skill it lacks.

*Engagements* -- what each attack actually traded. Damage dealt is priced at
the defender's cost and the counterattack at the attacker's, both as fractions
of displayed HP, so every attack has a net worth in funds. Indirects take no
counterattack and so cannot lose one; direct fire can, and the share of direct
attacks that came out behind is the number that says whether the policy picks
fights that make sense.

Pop position comes along for the ride, because the log that found the agent
firing its power as the turn's last order read it from exactly these records.

    py -3.12 python/play_diag.py --checkpoint checkpoints/ppo-t2v1.pt
        --co Adder --baseline
"""

import argparse
import collections
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "python"))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import awbw  # noqa: E402
import evaluate  # noqa: E402

UNITS = json.loads((ROOT / "data" / "units.json").read_text())
COST = {name: u["cost"] for name, u in UNITS.items()}
# `range_min > 1` is the engine's own definition of an indirect (types.rs).
INDIRECT = {name for name, u in UNITS.items() if u["range_min"] > 1}
# The recorder writes Rust's `UnitType` variant name (`Apc`, `MdTank`);
# units.json is keyed by AWBW's (`APC`, `Md.Tank`). They differ only in case
# and punctuation, so they are matched on letters alone rather than through a
# hand-written table -- a table silently prices a name it forgot at zero, which
# is exactly what a missing `Apc` did on the first run of this script.
def _fold(name):
    return "".join(c for c in name if c.isalnum()).lower()


FOLDED = {_fold(name): name for name in COST}


def named(kind):
    resolved = FOLDED.get(_fold(kind))
    if resolved is None:
        raise SystemExit(f"unit type {kind!r} is not in data/units.json")
    return resolved


class Tally:
    def __init__(self):
        self.builds = collections.Counter()
        self.attacks = collections.Counter()
        self.dealt = collections.Counter()
        self.lost = collections.Counter()
        self.losing = collections.Counter()
        self.pop_index = []
        self.turn_len = []
        self.games = 0

    def add_game(self, game, seat):
        """One recorded game, counting only what our seat did."""
        self.games += 1
        hp = {}
        for turn in game["turns"]:
            # The snapshot is the truth at turn start for *both* sides, so it
            # is applied whoever is moving: the opponent's units take damage on
            # our turns too, and their health has to keep up or the next trade
            # is priced against a stale number.
            for unit in turn.get("units") or []:
                if unit:
                    hp[unit["id"]] = unit["hp100"]
            orders = turn.get("orders") or []
            ours = turn.get("active") == seat
            if ours:
                self.turn_len.append(len(orders))
            for i, order in enumerate(orders):
                kind = order.get("kind")
                if kind == "Fire":
                    self.fire(order, hp, ours)
                elif kind == "Build" and ours:
                    self.builds[named(order["unit"]["type"])] += 1
                elif kind == "Power" and ours:
                    self.pop_index.append(i)
                for key in ("unit", "defender"):
                    rec = order.get(key)
                    if isinstance(rec, dict) and "id" in rec:
                        hp[rec["id"]] = rec["hp100"]

    def fire(self, order, hp, ours):
        attacker, defender = order.get("unit"), order.get("defender")
        if not isinstance(attacker, dict) or not isinstance(defender, dict):
            return
        a_type, d_type = named(attacker["type"]), named(defender["type"])
        # Pre-fight health is whatever the log last said. A unit first seen
        # here is taken at the health it ended with, which scores it no damage
        # rather than inventing some.
        a_before = hp.get(attacker["id"], attacker["hp100"])
        d_before = hp.get(defender["id"], defender["hp100"])
        if not ours:
            return
        hurt = max(0, d_before - defender["hp100"])
        dealt = hurt / 100 * COST.get(d_type, 0)
        lost = max(0, a_before - attacker["hp100"]) / 100 * COST.get(a_type, 0)
        bucket = "indirect" if a_type in INDIRECT else "direct"
        self.attacks[bucket] += 1
        self.dealt[bucket] += dealt
        self.lost[bucket] += lost
        if dealt < lost:
            self.losing[bucket] += 1

    def report(self, label, against, baseline=None, base_games=0):
        print(f"\n{label} vs {against}, {self.games} games\n")
        total = sum(self.builds.values())
        spend = sum(COST.get(n, 0) * c for n, c in self.builds.items())
        ind_n = sum(c for n, c in self.builds.items() if n in INDIRECT)
        ind_spend = sum(COST.get(n, 0) * c
                        for n, c in self.builds.items() if n in INDIRECT)
        print(f"  built {total} units, {total / max(self.games, 1):.1f}/game, "
              f"{spend:,} funds")
        for name, count in self.builds.most_common():
            mark = "  indirect" if name in INDIRECT else ""
            print(f"    {name:<12} {count:>4}  {count / max(total, 1):>5.1%}"
                  f"{mark}")
        print(f"  indirects: {ind_n / max(total, 1):.1%} of units, "
              f"{ind_spend / max(spend, 1):.1%} of spending")

        if baseline:
            base_total = sum(baseline.values())
            base_ind = sum(c for n, c in baseline.items()
                           if named(n) in INDIRECT)
            print()
            print(f"  against {base_games} recorded human games "
                  f"({base_total / max(base_games, 1):.1f} builds/game):")
            names = {n for n, _ in self.builds.most_common(8)}
            names |= {named(n) for n, _ in baseline.most_common(8)}
            rows = sorted(names, key=lambda n: -baseline.get(n, 0))
            for name in rows:
                theirs = baseline.get(name, 0) / max(base_total, 1)
                ours = self.builds.get(name, 0) / max(total, 1)
                print(f"    {name:<12} human {theirs:>5.1%}   "
                      f"policy {ours:>5.1%}   {ours - theirs:>+6.1%}")
            print(f"    {'indirects':<12} human "
                  f"{base_ind / max(base_total, 1):>5.1%}   "
                  f"policy {ind_n / max(total, 1):>5.1%}")

        print()
        for bucket in ("direct", "indirect"):
            n = self.attacks[bucket]
            if not n:
                print(f"  {bucket:<9} no attacks")
                continue
            net = self.dealt[bucket] - self.lost[bucket]
            print(f"  {bucket:<9} {n:>5} attacks  "
                  f"dealt {self.dealt[bucket] / n:>7,.0f}  "
                  f"lost {self.lost[bucket] / n:>7,.0f}  "
                  f"net {net / n:>+8,.0f}/attack  "
                  f"losing {self.losing[bucket] / n:.1%}")

        if self.pop_index:
            mean_len = sum(self.turn_len) / max(len(self.turn_len), 1)
            print(f"\n  {len(self.pop_index)} pops, "
                  f"{len(self.pop_index) / max(self.games, 1):.2f}/game, "
                  f"at order {sum(self.pop_index) / len(self.pop_index):.1f} "
                  f"of a {mean_len:.1f}-order turn")
        else:
            print("\n  no powers fired")


def corpus_mix(prepared):
    """The same composition, from the recorded human games.

    A build mix means nothing on its own -- 35% Anti-Air is only obviously
    wrong once you know humans build 6%. The corpus is every prepared game on
    the board, all COs, so it is the mix of the population the agent would have
    to be competent against rather than of any one opponent.
    """
    builds = collections.Counter()
    games = 0
    for path in sorted(Path(prepared).glob("*.json")):
        try:
            game = json.loads(path.read_text())
        except (ValueError, OSError):
            continue
        games += 1
        for turn in game.get("turns") or []:
            for action in turn.get("actions") or []:
                if not isinstance(action, dict):
                    continue
                if action.get("action") != "Build":
                    continue
                unit = action.get("newUnit")
                unit = unit.get("global") if isinstance(unit, dict) else None
                if isinstance(unit, dict) and unit.get("units_name"):
                    builds[unit["units_name"]] += 1
    return builds, games


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", default="checkpoints/ppo.pt")
    parser.add_argument("--opponent", default="greedy",
                        choices=["greedy", "jakeman", "capturer", "random"])
    parser.add_argument("--versus", default=None,
                        help="a checkpoint for the other seat")
    parser.add_argument("--games", type=int, default=30)
    parser.add_argument("--envs", type=int, default=16)
    parser.add_argument("--max-day", type=int, default=60)
    parser.add_argument("--temperature", type=float, default=1.0)
    parser.add_argument("--co", default=None)
    parser.add_argument("--seed", type=int, default=17)
    parser.add_argument("--baseline", action="store_true",
                        help="also print the human corpus build mix, which is "
                             "what makes the policy's own mix readable")
    parser.add_argument("--prepared", default="data/prepared")
    args = parser.parse_args()

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    rng = np.random.default_rng(args.seed)
    torch.manual_seed(args.seed)

    # Same layout dance as record_games.py: the env emits whatever the newest
    # checkpoint expects, and an older policy reads through a PlaneSlice.
    def peek(path):
        saved = torch.load(ROOT / path, map_location="cpu", weights_only=True)
        return saved["config"]["planes"]

    probe = awbw.VecEnv(num_envs=1)
    tiles = probe.board_shape[0] * probe.board_shape[1]
    want = peek(args.checkpoint)
    if args.versus:
        want = max(want, peek(args.versus))
    threat = want > probe.observation_size // tiles

    env = awbw.VecEnv(
        num_envs=args.envs, seed=args.seed, max_day=args.max_day,
        opponent=None if args.versus else args.opponent, record=True,
        co=args.co, threat=threat,
    )
    emitted = env.observation_size // tiles
    base_planes = probe.observation_size // tiles

    def fit(policy):
        if policy.planes == emitted:
            return policy
        if policy.planes == base_planes:
            return evaluate.PlaneSlice(policy, tiles)
        raise SystemExit(
            f"checkpoint has {policy.planes} planes but the env emits "
            f"{emitted}; threat-plane versions differ and cannot be mixed")

    agent = evaluate.Net(
        fit(evaluate.load(ROOT / args.checkpoint, env, device)), device,
        args.temperature)
    if args.versus:
        other = evaluate.Net(
            fit(evaluate.load(ROOT / args.versus, env, device)), device,
            args.temperature)
        agent = evaluate.Duel(agent, other, env, device)

    obs = torch.empty((args.envs, env.observation_size), dtype=torch.float32,
                      pin_memory=device.type == "cuda")
    view = obs.numpy()
    tally = Tally()
    while tally.games < args.games:
        env.observe_into(view)
        s, d, k, p = agent.choose(env, obs.to(device, non_blocking=True), rng)
        env.step(s, d, k, p)
        for slot in range(args.envs):
            log = env.take_replay(slot)
            if log is None or tally.games >= args.games:
                continue
            tally.add_game(json.loads(log), int(env.agent_seat()[slot]))

    baseline, base_games = (corpus_mix(ROOT / args.prepared)
                            if args.baseline else (None, 0))
    against = Path(args.versus).stem if args.versus else args.opponent
    tally.report(Path(args.checkpoint).stem, against, baseline, base_games)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
