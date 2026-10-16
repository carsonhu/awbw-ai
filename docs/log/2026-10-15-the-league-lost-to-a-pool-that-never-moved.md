# The league lost to a pool that never moved, and so did its controls

> `GRID=league2`: the per-member promotion rule, four league arms, four
> controls. **Zero promotions**, and not because the bar sat too high: across
> each whole run the learner scored 35-58% against members its parent beats
> 50-73%. The panels below say the controls won, and they did not: the
> controls were rated at their kept checkpoints, iteration 30-60, and the
> league arms at their final weights. Re-rated at final weights both groups
> end below the parent -- controls 24.6-54.3 a CO on JakeMan, league
> 23.6-44.3, parent 57.8. Every 32-env arm decays from level.

**Setup.** `GRID=league2` (`c8c72f7`) on a rented 4090 48GB (Vast, 21-core
share), all eight arms at once, `OMP_NUM_THREADS=2`, 22:58 -> 01:09.
Parent `mixjm-s43`, anchored to `bc-net2-t4` at 0.03, all five COs, 200
iterations. League arms: `--selfplay --frozen-init` (clone, `mixjm-s202`,
`mixjm-s7`, `mixjm-noanc-s43`) `--pool-bots jakeman --pool-bot-share 0.25
--seat-cap 50`, promotion at 0.60 against every member over 30 games each.
Controls: `--opponent jakeman`. Panels as Adder, 200 games a member.

| arm | greedy | JakeMan | `bc-powers-scaled2` | `ppo-adder3` |
|---|---|---|---|---|
| lg2-s7 | 72.5/64.0 | 25.0/22.5 | 82.0/71.0 | 89.0/82.5 |
| lg2-s43 | 73.0/66.5 | 20.5/19.5 | 65.8/62.0 | 70.5/60.5 |
| lg2-s101 | 54.5/47.0 | 30.5/29.0 | 73.0/56.5 | 89.0/86.0 |
| lg2-s202 | 57.5/55.0 | 44.0/43.0 | 76.5/71.0 | 82.5/78.0 |
| lg2ctl-s7 | 80.8/76.5 | 62.0/60.5 | 82.0/78.5 | 95.5/87.5 |
| lg2ctl-s43 | 70.5/67.0 | 53.5/51.0 | 77.0/70.0 | 94.0/84.5 |
| lg2ctl-s101 | 82.5/79.0 | 49.0/47.0 | 73.0/64.5 | 87.5/84.0 |
| **lg2ctl-s202** | 82.0/77.5 | **65.2/60.5** | **91.5/83.0** | **96.5/94.0** |
| mean, league | 64.4 | 30.0 | 74.3 | 82.8 |
| mean, control | 79.0 | 57.4 | 80.9 | 93.4 |

Per CO against JakeMan, score (parent `mixjm-s43`: 50.7/58.0/59.5/69.5/46.5):

| arm | Adder | Jake | Koal | Jess | Grimm |
|---|---|---|---|---|---|
| lg2-s7 | 25.0 | 24.5 | 32.0 | 39.5 | 15.9 |
| lg2-s43 | 21.0 | 21.5 | 20.4 | 32.0 | 23.0 |
| lg2-s101 | 30.5 | 29.8 | 29.5 | 33.2 | 21.0 |
| lg2-s202 | 44.0 | 46.8 | 54.0 | 50.0 | 26.5 |
| lg2ctl-s7 | 62.0 | 59.0 | 54.0 | 69.5 | 36.5 |
| lg2ctl-s43 | 53.5 | 47.0 | 46.5 | 60.0 | 35.5 |
| lg2ctl-s101 | 49.0 | 56.2 | 55.5 | 60.0 | 45.5 |
| **lg2ctl-s202** | **65.2** | **71.5** | **71.0** | **78.0** | 45.3 |

**The bar never got a question it could answer yes to.** Whole-run training
totals, per member: `lg2-s7` 58.1/45.8/54.6/50.5, `lg2-s43` 50.8/51.0/34.9/52.5,
`lg2-s101` 52.7/46.9/43.3/56.5, `lg2-s202` 43.8/55.7/57.2/50.7 -- against a
parent that opened at 70.5 on the clone and 50-73 on the rest
(`2026-09-08-the-pool-is-six-opponents.md`). The windows read 60-87% on some
members at iteration 30 and drifted down from there. A learner that loses
ground against weights that never change has nothing for a promotion rule to
detect; 0.60 is calibrated and was not the binding constraint.

**The controls appear to go the other way, and the next section retracts
it.** On these numbers all four clear the league group on JakeMan, and
`lg2ctl-s202` beats the parent on four COs by 6.5-15.5 points.

**Correction: the controls were rated near their parent.** A control writes
`--out` at its best rollout window, and all four kept iteration 30 or 40-60,
five to thirty-five updates past the warm-up; a league arm that never
promotes writes its final weights. Re-rated at `-last.pt` on the same box,
200 games a CO against JakeMan, mean of five COs:

| | s7 | s43 | s101 | s202 |
|---|---|---|---|---|
| parent `mixjm-s43` | 57.8 | | | |
| lg2ctl, final | 47.1 | 24.6 | 54.3 | 26.4 |
| lg2, final | 27.4 | 23.6 | 28.8 | 44.3 |

So the table above compares the parent re-rated with trained arms. Both groups
decay; the controls perhaps less, with ranges that overlap. `grid.sh` now
rates final weights (`2026-10-16-the-bigger-batch-climbs-and-can-specialise.md`).

**What it did not settle.** Why the 32-env arms decay, league or not. An
update averages ~9 finished games; `GRID=batch` ran next on the same box to
ask whether that is the cause.
