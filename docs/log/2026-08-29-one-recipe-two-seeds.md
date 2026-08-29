# One recipe, two seeds, 27 points apart

> The first seed-variance measurement this project has ever taken:
> `anchor-kl 0.03` run twice, identical but for `--seed`, panels
> **86.8 / 9.5 / 53.8 / 48.8** and **92.5 / 2.5 / 45.5 / 22.0**. Every
> one-seed comparison in this log has been read against error bars a
> fraction of the real ones.

**Setup.** `ppo-anc003` (seed default) and `ppo-anc003b` (`--seed 43`),
otherwise identical greedy-rung runs; 200-game panels each.

| panel member | seed A | seed B | spread |
|---|---|---|---|
| `greedy` | 86.8 | 92.5 | 5.7 |
| JakeMan | 9.5 | 2.5 | 7.0 |
| the clone | 53.8 | 45.5 | 8.3 |
| `ppo-adder3` | 48.8 | 22.0 | **26.8** |

The panel's ±2.5-3.5 intervals measure game-sampling noise around a
*fixed* policy. They say nothing about which policy the run produces,
and that variance is five to eight times larger on the net members.

**What replicates and what does not.** Composition replicates well —
Infantry 42.6 vs 38.6, Tank 11.9 vs 11.3, Anti-Air 17.3 vs 19.4,
indirects 1.8 vs 2.1 — and engagement quality exactly (+2,064 vs
+2,093, losing 10.8 vs 11.2). Power behaviour does not: seed A pops
0.20/game at order 11, seed B never pops. Build distributions are
driven by thousands of decisions per run and are stable; pop policy
rests on a handful and is a lottery.

**What this retro-qualifies.** Any one-seed panel delta under ~10
points on a net member decided nothing: the v2-planes "miss" (85.5 vs
the 93.0 bar) is inside this spread on `greedy`, and the rung-to-rung
`greedy` giveback the threat lineage was read by is far inside it. The
sweep verdict survives because its margins (29 -> 69 on the clone) do
not fit in the spread. Scoreboard claims need paired seeds or margins
that dwarf this table.
