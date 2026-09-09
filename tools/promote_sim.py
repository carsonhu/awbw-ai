"""What the league's promotion bar fires on, without renting anything.

`ppo.py` promotes when every pool member is being beaten at `--refresh-at`,
each on at least `--refresh-games` games since the last promotion. Whether that
is a bar worth setting is a statistics question, not a training question: the
check runs every iteration against an estimate that only accumulates, so the
quantity to know is how often it fires on a learner that has improved nothing.

Members' true rates default to the measured roster of the first league
(`docs/log/2026-09-08-the-pool-is-six-opponents.md`): `mixjm-s43` scored 90.0,
73.0, 70.5, 54.0 and 50.0 against the five frozen members it was dealt.

    python tools/promote_sim.py            # the two sweeps behind that entry
    python tools/promote_sim.py 0.9 0.5    # a roster of your own
"""

import random
import sys

ROSTER = [0.900, 0.730, 0.705, 0.540, 0.500]


def run(rates, bar, need, z=0.0, iters=200, per=6, hold=5, seed=0):
    """Iterations until the rule fires, or None. One member holds the seat."""
    rng = random.Random(seed)
    book = [[0, 0.0] for _ in rates]
    seat, held = 0, hold
    for it in range(iters):
        if held == 0:
            seat, held = rng.randrange(len(rates)), hold
        held -= 1
        book[seat][0] += per
        book[seat][1] += sum(1 for _ in range(per) if rng.random() < rates[seat])
        if all(b[0] >= need for b in book):
            if all((s / g) - z * ((s / g) * (1 - s / g) / g) ** 0.5 >= bar
                   for g, s in book):
                return it
    return None


def fires(rates, bar, need, z=0.0, n=600, **kw):
    return sum(run(rates, bar, need, z, seed=s, **kw) is not None
               for s in range(n)) / n


def better(rates, points):
    return [min(r + points, 0.99) for r in rates]


def main():
    roster = [float(a) for a in sys.argv[1:]] or ROSTER
    print(f"roster {roster}, 200 iterations, 6 games an iteration\n")
    print("bar 0.60, 30 games a member -- what a confidence margin costs")
    print(f"{'margin':>7} | {'no gain':>8} | {'+10 pts':>8} | {'+15 pts':>8}")
    for z in (0.0, 0.25, 0.5, 1.0):
        print(f"{z:>7.2f} | {fires(roster, 0.60, 30, z):>8.1%} | "
              f"{fires(better(roster, 0.10), 0.60, 30, z):>8.1%} | "
              f"{fires(better(roster, 0.15), 0.60, 30, z):>8.1%}")
    print("\nno margin, 30 games a member -- what the bar itself costs")
    print(f"{'bar':>7} | {'no gain':>8} | {'+10 pts':>8} | {'+15 pts':>8}")
    for bar in (0.55, 0.575, 0.60, 0.65):
        print(f"{bar:>7.3f} | {fires(roster, bar, 30):>8.1%} | "
              f"{fires(better(roster, 0.10), bar, 30):>8.1%} | "
              f"{fires(better(roster, 0.15), bar, 30):>8.1%}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
