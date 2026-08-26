# PPO's defaults are in the wrong units for this game, and one of them mattered

> The credit horizon was one turn. Widening it stops the decay that four runs
> showed. Two other candidates — a day cap scored as a real result, and a
> potential that cannot see money — were wrong and unmeasured respectively.

**Where this started.** `log/2026-08-26-ppo-only-climbs-from-behind.md` left a
pattern with no mechanism. The question here: which PPO settings are Atari's
units rather than AWBW's? Against `greedy`, `bc-scaled` plays 399 orders a game
and 0.5% hit the day cap; against JakeMan, `ppo-jake2` plays 690 and 10.5% do —
the runs that gained and the runs that came apart.

## The candidates

**The day cap was scored as a real result.** A truncated game bootstrapped from
zero, so its advantage was `-V(s)`: a reward for being behind, a penalty for
being ahead, firing 21x more often in the setting that decays. `step` now
returns `truncated` and GAE gives those steps no surprise.

**The credit horizon was one turn.** `1/(1 - gamma*lam)` at 0.997 and 0.95 is 19
orders; a turn is about 17. Nothing slower than one turn reached the policy
except through the critic. `--turn-discount` applies both rates once per *turn*,
and `--steps 256` makes the rollout longer than the horizon — affordable because
the rollout buffer moved to host memory, which cost no throughput at all (it
gained: 287 -> 386 orders/sec, longer rollouts amortise better).

**The potential cannot see money.** `--potential funds` and `worth` were built
and are **untested**; no arm ran them.

## What the arms said

Both from `ppo-jake2` against fixed JakeMan, `--recalibrate 0`. Closed windows
of >=100 games, against the recorded control:

| window | control | A: cap fixed | B: A + long horizon |
|---|---|---|---|
| 1 | 55.1% | 61.3% | 56.4% |
| 2 | 49.0% | 47.8% | 60.2% |
| 3 | 35.9% | 33.8% | 58.0% |
| 4 | — | — | 49.1% |
| 5 | — | — | 57.8% |

**A reproduces the decay exactly.** The day-cap bias is real and its sign does
match the four-run pattern, but it is not the cause of anything.

**B does not decay.** At 200 games, with capped games settled on properties then
material so a stall is not a free half point, `ppo-jake2` scores 59.5% ±3.5 and
`armB-last` 56.5% ±3.5. Flat, where the control and arm A fell twenty.

**What B gave up.** Undecided, the same weights rate 49.2%: outright wins fell
51.0% -> 39.0% and draws doubled, 10.5% -> 20.5%. B reaches winning positions
and does not close them — under the tiebreak it is ahead in 35 of its 41 capped
games. The reported score hid this entirely, because a draw counts half.

**Unsettled.** Whether B's three changes matter separately; whether flat becomes
a climb with more iterations; and `--decide-cap`, added because of the above and
never trained against.
