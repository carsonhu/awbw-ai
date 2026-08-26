# PPO's first real run, and why it came apart at the top

> 5.5% to 96.2% against `greedy`. It saturated the opponent by iteration 110,
> then unlearned itself back to 80% — with a traceable cause, and a fix.

**Setup.** `ppo.py --init checkpoints/bc-scaled.pt`, 200 iterations of 32 envs ×
64 steps against `greedy`, shaping 0.1, 25 iterations of critic warm-up. About
21 minutes at ~320 orders/sec, and bit-reproducible on a fixed seed. Rated
afterwards by `evaluate.py` on a different seed than training used.

**The starting point was not what the clone's rating said.** PPO samples
on-policy, at temperature 1.0; every clone rating in this project had been taken
at `--temperature 0.3`. The same weights score 19.0% ±2.0 at 0.3 and 5.5% ±1.6
at 1.0. So PPO started from 5.5% — and the first 25 iterations, policy frozen
for the critic warm-up, confirmed it on the board at 6.0%.

**Result.** `ppo` is iteration 110; `ppo-run1` is the same run allowed to finish
all 200.

| | vs greedy | vs capturer | vs random |
|---|---|---|---|
| `bc-scaled` | 5.5% ±1.6 | 98.0% ±1.0 | — |
| `ppo-run1` (final) | 82.6% ±1.9 | 95.5% ±1.5 | 95.8% ±1.4 |
| `ppo` (peak) | **96.2% ±0.9** | **99.0% ±0.7** | **100.0%** |

Better against every opponent, including the ones the clone already beat, so
this is not an anti-`greedy` trick bought by forgetting how to play. The
temperature dependence is also gone — the peak scores 96.2% at 1.0 where the
clone needed hand-sharpening to reach 19%. Learning to commit was a real part of
the gain.

**Then it unlearned itself.**

| iteration | score | value loss | entropy |
|---|---|---|---|
| 100 | 97.9% | 0.002 | 2.13 |
| 130 | 100.0% | 0.001 | 3.17 |
| 200 | 80.3% | 0.010 | 2.51 |

Saturating the opponent leaves the critic nothing to predict, so the advantage
collapses to residual noise — and `update()` normalises advantages to unit
scale, rescaling that noise back to a full-size step. Entropy climbed at once
and the score followed it down. It is the random-critic failure in
`decisions.md` arriving from the far end: there the critic knew nothing, here
there is nothing left to know.

`ppo.py` now keeps the best-scoring weights instead of the last, writing the
final set beside them as `-last.pt`. That is a guard, not a cure — the real fix
is an opponent that does not run out, which means self-play.

**A wrong guess, recorded.** The 200-iteration weights drew 19.8% of games at
the day cap against the clone's 0%, and I put that down to the 0.997 discount
and the shaping term rewarding a grind over a finish. The peak checkpoint draws
1.5%. The stalling was the diffused policy failing to close, not an incentive to
stall — a symptom of the decay, and it left with it.
