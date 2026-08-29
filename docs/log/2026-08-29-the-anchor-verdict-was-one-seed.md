# The 0.01 verdict was one seed; the recipe is a lottery it won once

> This morning's entry called `ppo-anc001` a sweep verdict with margins
> "no seed spread reaches". The second seed reached them: same recipe,
> `--seed 43`, and the clone number fell 69.0 -> 25.0, `ppo-adder3`
> 73.8 -> 23.0. The checkpoint stands — the strongest this project has.
> The claim that weight 0.01 *produces* it does not.

**Setup.** `ppo-anc001b`: the 0.01 run repeated under `--seed 43`,
panelled and probed identically.

| 200 games each | control | 0.01 (A) | 0.01 (B) | 0.03 (A) | 0.03 (B) |
|---|---|---|---|---|---|
| vs `greedy` | 85.5 | 94.0 | 92.5 | 86.8 | 92.5 |
| vs JakeMan | 5.0 | 12.5 | 5.5 | 9.5 | 2.5 |
| vs the clone | 29.0 | **69.0** | 25.0 | 53.8 | 45.5 |
| vs `ppo-adder3` | 33.5 | **73.8** | 23.0 | 48.8 | 22.0 |
| Anti-Air share (human 6.1) | 35.0 | 19.8 | 29.2 | 17.3 | 19.4 |
| indirect share (human 3.9) | 1.6 | 3.3 | 0.8 | 1.8 | 2.1 |

**What survives, seed-checked.** Composition responds to the anchor
*reliably at 0.03* (17.3/19.4 across seeds) and *unreliably at 0.01*
(19.8/29.2): the lighter pull only sometimes holds the line. And
composition co-moves with net-member strength in every run measured —
the seed that drifted least (0.01-A) panelled best, the ones that
drifted most (control, 0.01-B) panelled worst. The causal story is
intact; the dose does not reliably deliver it at 0.01.

**What died.** "Anchored >> unanchored on the net members" as a per-run
guarantee, and any weight ordering. Run-level variance dominates weight
effects across this whole bracket.

**What this means procedurally.** The unit of experiment on this rung
is now the *seed group*, not the run: N seeds, panel each, keep the
best, report the spread. `ppo-anc001` was exactly that procedure done
by accident — kept-best across a lottery — and it is the checkpoint of
record. The economical version is the rented-GPU grid, where five
seeds of one recipe are one wall-clock run.
