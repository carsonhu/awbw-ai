# The v2 planes climb faster and land lower

> The pre-registered bar was 93.0% against `greedy`, and `ppo-t2v1`
> rates **85.5 ±2.5** — a miss by more than either interval. Same
> recipe, same rung, same 200 iterations: the probability planes open
> four times higher and reach a 90% window in half the time, and then
> saturate seven points below the planes they replaced.

**Setup.** `panel.py --checkpoint checkpoints/ppo-t2v1.pt`, 200 games
per opponent, CO Adder. `ppo-t2v1` is the greedy rung on planes v2
(`--opponent greedy --co Adder --turn-discount --steps 256 --lam 0.99
--decide-cap`, 200 iterations, init `bc-threat2`); the comparison is
`ppo-threat1`, the identical rung on v1 from `bc-threat`.

| 200 games each | `ppo-threat1` (v1) | `ppo-t2v1` (v2) |
|---|---|---|
| vs `greedy` | **93.0 ±1.8** | 85.5 ±2.5 |
| vs JakeMan | 5.5 ±1.6 | 5.0 ±1.5 |
| vs the clone | **48.5 ±3.5** | 29.0 ±3.2 |
| vs `ppo-adder3` | 19.5 | **33.5 ±3.3** |
| kept rollout window | 93.7 | 90.6 |
| held-out order accuracy | 0.459 | 0.426 |

**Speed and plateau came apart.** Everything the previous entry
measured still holds — the v2 clone opens at 25% against `greedy` where
v1 opened at 6, before a gradient step, and the rung reaches a 90%
window by iteration 50 against v1's 80. None of it survived to the
plateau. A cheaper climb to a lower ceiling is a real shape, and it is
the opposite of what the extra arithmetic was bought for.

**The window mis-estimated v2 much worse than v1.** 93.7 → 93.0 is
within noise; 90.6 → 85.5 is five points of shrinkage. The kept-best
rule banks whichever window luck inflated most, so a lineage whose
windows are noisier keeps a worse checkpoint at the same true strength
— and the two numbers this project reads a rung by, window and rating,
are not interchangeable across an observation change.

**Not settled: planes or seed.** One run each. The deficit is larger
than the panel intervals but the *run-to-run* spread on this rung has
never been measured, and the v1 rung is known to oscillate at
saturation. A second v2 seed is what would separate them, and it costs
one run.

**Where v2 is genuinely ahead** is `ppo-adder3` — 33.5 against 19.5,
the one panel member both lineages face as a stranger. Against the
member they are both *nearest* to, the clone, v2 is 19 points worse.
That pattern is what a policy that has specialised harder looks like,
not what a better observation looks like.
