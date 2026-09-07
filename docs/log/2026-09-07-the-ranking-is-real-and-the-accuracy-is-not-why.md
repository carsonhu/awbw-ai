# The ranking is real, and the critic's accuracy is not why

> Two controls on the reranking gain. A noise critic reproduces the no-search
> baseline and leaves the cap rate alone, so the +8.3 decisive points are the
> value ranking and not the code path -- real critics beat noise by 13.5
> points, z=3.9. But three critics spanning 0.564 to 0.859 accuracy on human
> games buy the same +6.0, +8.3, +7.0, and swapping the policy's shaped head
> for the clone's win-probability head changes nothing measurable except a
> third of the cap inflation. Ranking one-ply-apart positions is an easier
> task than the instrument that scores critics measures.

**Setup.** `mixjm-s43` as Adder against JakeMan, 400 games a cell, `--rerank 4`.
Three rankers: the policy's own value head (shaped, turn-discounted return
with the cap tiebreak in it, 0.807 on `value_diag`), `bc-net2-t4`'s head (a
recorded-outcome win probability, 0.859), and Gaussian noise. Noise still
scores game-ending orders by their real result, since spotting the move that
wins now needs no critic; everything else is ranked at random.

| ranker | `value_diag` | score | decisive | capped |
|---|---|---|---|---|
| none (`--rerank 1`) | -- | 56.1 | 53.2 | 4.5 |
| noise | -- | 51.4 | 48.0 | 4.2 |
| own head | 0.807 | **76.5** | **61.5** | 17.0 |
| `bc-net2-t4` head | 0.859 | 70.9 | 60.2 | 12.2 |

**The control holds and the gain survives it.** Noise lands at 48.0 decisive
against the 53.2 baseline (z=-1.47) and 4.2% capped against 4.5% -- no gain,
no cap inflation, which is what the argmax of noise over four independent
policy samples should do. Real critics beat it by 13.5 decisive points
(z=3.87). The reranking path is not quietly doing something else.

Noise sits slightly *below* baseline for a reason worth writing down: an
unbounded Gaussian draw can outrank a terminal +1.0, so the control
occasionally declines a winning order. The true null is the baseline and the
five-point dip is the control's own scale, not a finding.

**Critic accuracy does not predict the gain.** `mixjm-s101` at 0.564 -- a coin
flip on openings -- gains 6.0 decisive points. `mixjm-s43` at 0.807 gains 8.3.
The clone at 0.859 gains 7.0. The spread across 0.30 of accuracy is inside
the noise of any pair. Whatever makes a critic good at saying who wins a human
game is not what makes it good at separating two positions one order apart,
and `value_diag` measures the first.

**The objective is worth a third of the cap inflation.** Swapping the shaped
head for a win-probability head takes capped games from 17.0% to 12.2%
(z=-1.93) while decisive share holds at 60.2 against 61.5 (z=-0.38). So the
cap tiebreak inside the training reward explains part of the stalling, and
greedy one-ply selection explains the rest: 12.2% is still far above the
baseline's 4.5% (z=3.98) with no cap term anywhere in the ranker.

**What it does not settle.** One width, one arm for the controls, one
opponent. The finding that matters for the plan is negative and awkward: the
critic instrument built this morning does not predict what a search gets from
a critic, so "gated on the critic" cannot be checked with `value_diag`. What a
search-facing critic instrument should measure -- agreement with the *ordering*
of positions one order apart -- does not exist and is a different measurement
from anything on record here.
