# The promotion bar is calibrated, and a margin costs more than it saves

> The league's new rule -- promote when *every* pool member is beaten at
> `--refresh-at`, on `--refresh-games` games each since the last promotion --
> simulated against the measured roster. At **0.60 and 30 games, with no
> confidence margin**, it fires on 4.5% of runs that improve nothing and 78% of
> runs ten points better on every member. One standard error of margin drops
> that 78% to 28%, which is the old never-promotes failure reached from the
> other side. The bare rule is the operating point.

**Setup.** `tools/promote_sim.py`. Members' true rates are the roster measured
against `mixjm-s43` (`2026-09-08-the-pool-is-six-opponents.md`): 90.0, 73.0,
70.5, 54.0, 50.0. One member holds the seat five iterations and takes that
iteration's six games; the rule is checked every iteration against a book that
only accumulates. 200 iterations, 600 runs a cell. "No gain" is a learner that
has improved nothing -- every false fire there is a generation promoted for
noise.

| margin | no gain | +10 pts | +15 pts |
|---|---|---|---|
| **none** | **4.5%** | **78.0%** | 97.7% |
| 0.25 SE | 1.5% | 66.3% | 95.0% |
| 0.50 SE | 0.3% | 52.8% | 93.2% |
| 1.00 SE | 0.0% | 27.8% | 83.7% |

| bar (no margin) | no gain | +10 pts | +15 pts |
|---|---|---|---|
| 0.550 | 25.7% | 97.3% | 100.0% |
| 0.575 | 9.5% | 91.0% | 99.7% |
| **0.600** | **4.5%** | **78.0%** | 97.7% |
| 0.650 | 0.2% | 26.3% | 76.0% |

**The margin was a correction to an argument that was wrong.** A member the
learner is level with clears 0.60 on 18% of single 30-game checks, and the
check runs every iteration, so a margin looks obligatory. It is not: *every*
member must clear at once, and an accumulating estimate converges rather than
wandering. Five members with two of them near the bar make the joint condition
strict on its own, and 4.5% is what it is worth. Paying 50 points of power to
buy the last 4.5 points of false alarms is the wrong side of that trade -- a
false promotion costs one stale generation in the pool, and a missed one costs
the run.

**The false-alarm rate belongs to the roster, not the rule.** The same bar on a
two-member pool of 0.90 and 0.50 fires on 23.5% of no-gain runs, because a
short roster gives the joint condition little to bite on. Trimming the pool --
`mixjm-s101` at 90.0 is a near-free win and an obvious candidate -- moves this
number and wants re-simulating first. That is what the tool is for.

**A smoke test that could not test anything.** The end-to-end check that
prompted all this ran two members at `--refresh-games 8` for 26 iterations, and
promoted. Simulated, that configuration fires on **36.7%** of no-gain runs. It
was measuring its own settings, not the rule; nothing about the defaults was in
play.

**What it does not settle.** The simulation holds each member's true rate fixed
while the learner trains, which is exactly what a promotion is supposed to
break; six games an iteration is read off one grid's logs; and none of it says
a league that promotes correctly gains anything. It prices the instrument.
