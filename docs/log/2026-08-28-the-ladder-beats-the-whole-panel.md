# The ladder beats the whole panel, and the v1 threat book closes

> Three rungs on the threat lineage: JakeMan 5.5% → 37.5% → 63.4%.
> `ppo-threat3` is the first checkpoint in this project with a winning
> record against every panel member — the bot that was unbeatable five
> rungs of old-lineage work ago now loses two games in three.

**Setup.** `ppo-threat3`: the JakeMan rung continued (`--init
ppo-threat2`, same recipe, 200 iterations). Crossed 50% against
JakeMan at iteration 80, banked a 74.4% window at 160.

| panel, 200 games each | `ppo-threat1` | `ppo-threat2` | `ppo-threat3` |
|---|---|---|---|
| vs `greedy` | 93.0% | 89.5% | 86.5 ±2.4 |
| vs JakeMan | 5.5% | 37.5% | **63.4 ±3.4** |
| vs the clone | 48.5% | 84.0% | **87.5 ±2.3** |
| vs `ppo-adder3` | 19.5% | 82.0% | 75.5 ±3.0 |

**The lineage's shape held for three straight rungs**: long flat
start, explosive middle, mid-climb finish; greedy giving back ~3
points per rung while the trained-against number leaps 30. The whole
climb from clone to panel-sweeper took three standard runs — the old
lineage never swept the panel in eleven.

**Closed with the book.** The v1 threat planes (the zero-luck floor)
retire with this checkpoint: v2 (expected damage and P(KO) over each
CO's luck range, `5c98a37`) versions the observation to 70 planes, and
cross-version slicing is refused loudly (`32455fe`) after silently
degrading a v1 policy once. `bc-threat2` (v2 clone, held-out 0.426 —
three points under v1's, which the decoupling rule says to ignore)
opens the v3 book; the greedy rung reruns next with 93.0% as the bar,
and anything above it is the probability axis paying.
