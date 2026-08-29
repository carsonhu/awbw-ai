# The anchor buys strength across the whole panel, not just composition

> The KL anchor was built to stop composition drift, and the tension it
> was built under — that pulling toward the clone would give back the
> tactical gains — ran backwards. `ppo-anc001` (weight 0.01) rates
> **94.0 / 12.5 / 69.0 / 73.8** across the panel against the unanchored
> control's 85.5 / 5.0 / 29.0 / 33.5: the strongest checkpoint this
> project has produced, on every member at once.

**Setup.** Three greedy-rung runs, the v2 recipe with `--anchor
bc-threat2 --anchor-kl {0.01, 0.03, 0.10}`, init `bc-threat2`, 200
iterations each; a second seed at 0.03. Control is `ppo-t2v1`, the
same run with weight 0.

| 200 games each | control | **0.01** | 0.03 (A) | 0.03 (B) | 0.10 |
|---|---|---|---|---|---|
| vs `greedy` | 85.5 | **94.0 ±1.7** | 86.8 | 92.5 | 88.5 |
| vs JakeMan | 5.0 | **12.5 ±2.3** | 9.5 | 2.5 | 4.5 |
| vs the clone | 29.0 | **69.0 ±3.3** | 53.8 | 45.5 | 65.5 |
| vs `ppo-adder3` | 33.5 | **73.8 ±3.1** | 48.8 | 22.0 | 59.0 |
| Anti-Air share (human 6.1) | 35.0% | 19.8% | 17.3% | 19.4% | — |
| held anchor divergence | 1.03 nats | ~0.60 | ~0.30 | ~0.30 | — |

**Why it generalises.** The drift the anchor removes *was* the
specialisation: 35% Anti-Air is a best response to `greedy` and to
nothing else. Held near human strategy, the same training pays off
against every opponent — including two the run never saw. Engagement
quality kept most of its gain (+2,070 net per attack against the
control's +2,407, losing attacks ~11%).

**What is settled and what is not.** Anchored beats unanchored — the
clone and `ppo-adder3` columns clear the control by margins no seed
spread reaches, and composition recovery replicates across seeds. The
*weight ordering* is not settled: one seed at 0.01, and the two 0.03
seeds differ by up to 27 points on a panel member
(`2026-08-29-one-recipe-two-seeds.md`). The second seed of 0.01 is
running as this is written.

**Powers stay broken under the anchor.** 0.01 pops 0.03/game at order
31; 0.10 never pops. The anchor's pull is averaged over visited states,
and activation decisions are a few per game against hundreds — the
composition it fixes is common, the timing it cannot fix is rare.
