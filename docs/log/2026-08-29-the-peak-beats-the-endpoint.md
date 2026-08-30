# The peak beats the endpoint, without knowing anything

> The answer to the previous entry's open question, and it is not the
> one its correlation implied: rating all eight arms' final weights
> against their peak-kept ones, the peak wins by 5.8 points on JakeMan
> (better in six of eight) and 5.0 on `ppo-adder3`. Keep saving the
> peak. Keep never reporting it -- the value is still twelve points
> high and r=0.07 against the panel. And the parent ordering *flips*
> between the two checkpoint sets, so that question is undecided by
> more than seeds.

**Setup.** `evaluate.py` on the local 1660 Ti, 16 runs in 54 minutes,
flags matched to `panel.py` so the pairs are comparable (200 games,
`--co Adder --decide-cap --temperature 1.0`, default seed, same map).
Two members only: JakeMan, the rung's target, and `ppo-adder3`, the
member that came nearest to separating the parents.

| arm | JakeMan peak -> last | `ppo-adder3` peak -> last |
|---|---|---|
| jm-s7par-s7 | 49.3 -> 32.5 (-16.8) | 94.5 -> 71.5 (-23.0) |
| jm-s7par-s43 | 37.0 -> 25.5 (-11.5) | 79.5 -> 76.5 (-3.0) |
| jm-s7par-s101 | 33.0 -> 20.6 (-12.4) | 84.5 -> 57.5 (-27.0) |
| jm-s7par-s202 | 40.5 -> 37.0 (-3.5) | 90.8 -> 88.2 (-2.6) |
| jm-s101par-s7 | 29.5 -> **42.0** (+12.5) | 78.0 -> 80.0 (+2.0) |
| jm-s101par-s43 | 39.0 -> 23.5 (-15.5) | 80.8 -> 76.0 (-4.8) |
| jm-s101par-s101 | 19.8 -> **31.0** (+11.2) | 76.1 -> **89.5** (+13.4) |
| jm-s101par-s202 | 38.0 -> 27.8 (-10.2) | 74.5 -> 79.5 (+5.0) |
| mean | 35.8 -> 30.0 | 82.3 -> 77.3 |

**Selection works; the statistic still does not.** These are two
different claims and the previous entry only tested one. A rollout
peak of 52.7 says nothing about strength -- that survives. But the
*argmax* it picks lands on better weights than iteration 200 does, by
about six points on both members. `grid.sh` keeps saving the peak, and
a rung reading its own progress from peaks keeps lying to itself.

**Neither checkpoint is "the" policy.** The per-arm deltas run -27.0
to +13.4. A policy oscillating 26-59% in training, sampled at two
arbitrary moments, gives two draws that differ by more than any effect
this project has measured on a rung. The two arms whose finals beat
their peaks are the two whose peaks most overstated them -- regression
to the arm's own mean, not a better endpoint.

**The parent ordering flips with the checkpoint set.** On peak weights
`a003-s7` led every column; on final weights `a003-s101` leads both
measured ones (JakeMan 31.1 against 28.9, `ppo-adder3` 81.2 against
73.4), ranges overlapping throughout. Four seeds could not separate
these parents and neither can two snapshots of each -- the difference,
if any, is smaller than the noise two independent sources of it
supply. Pick by cost, not by panel: `jm-s7par-s7` is still the single
best JakeMan number anyone has (49.3), and that is a draw, not a
property of its parent.

**Next is unchanged and now better motivated.** A continuation rung
(`--init jm-s7par-s7`) is what took v1 from 37.5 to 63.4; splitting
hairs between parents is measuring noise. Pair it with one unanchored
arm to price `--anchor-kl 0.03` on this rung, which no run has done.
