# An order-invariant target for the source head

> Training the source head on the whole turn's set of units instead of the next
> one. The mechanism worked; play did not move. Null result.

**Question.** The source head picks which unit acts next, and its top-1 accuracy
is 44.7% — by far the worst number in the four heads. But a turn's ~14 orders
are largely interchangeable, so the label names one arbitrary member of a set of
equally good answers. How much of that 44.7% is real error?

`order_diag.py` scores the same predictions three ways:

| the head's pick is | rate |
|---|---|
| the unit the human moved *next* | 44.7% |
| a unit the human moved *somewhere this turn* | 95.2% |
| a uniform pick under the same mask (chance) | 68.2% |

So most of the apparent error is ordering, not judgement — the head knows which
units matter and disagrees about sequence.

**Setup.** `Cursor::with_lookahead` reads a turn before serving it and records
the tile each order acts from; `source_targets()` exposes the set still to be
acted from. `bc.py --source-set W` blends `-log` of the policy's mass anywhere
in that set into the exact cross-entropy. End-turn is excluded from the set
unless the human ended there — it is the last thing every turn does, so leaving
it in would let a policy satisfy the whole loss by ending every turn at once.

A blend rather than a replacement, because order is not always free: a blocker
has to move before the unit it blocks. Validation keeps scoring the exact label,
so the metric cannot move just because the training target did.

96×8, 15k steps, `--source-set 0.5`, otherwise identical. 400 paired games each
at `--temperature 0.3`.

**Result.**

| | baseline | set target |
|---|---|---|
| in-turn rate | 95.2% | 95.7% |
| exact label | 44.8% | 43.6% |
| vs greedy | 19.0% | 19.8% ±2.0 |

**Reading.** The mechanism did exactly what it was built to do — the model
shifted off the exact label and onto the set, visible in both directions. There
was simply nothing to win. 95.2% against 68.2% chance does not only mean "the
error is ordering"; it also means the head was *already nearly perfect* at the
answerable question, leaving at most 4.8 points to recover on a metric that was
not costing play.

The source head is not the bottleneck. Worth knowing — it was the obvious
suspect, being the weakest-looking number in the table. The clone's remaining
weakness is in what it does with a unit, not which unit it picks.

Kept behind `--source-set`, default 0.0, since the diagnostic is worth re-running
if the encoding or the head design changes.
