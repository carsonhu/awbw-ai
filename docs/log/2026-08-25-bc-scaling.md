# Is the clone limited by data or by capacity?

> Ten times the corpus bought nothing at fixed steps; a bigger network for
> longer nearly tripled play strength. And accuracy barely tracked either.

**Question.** The first clone scored 4.5% against `greedy`. It was trained on
243 games, which felt obviously too few — worth checking before spending days
on replay preparation.

**Setup.** The archive sweep took the corpus from 243 games to 2,446 games /
68,028 turns / 986,869 orders. Two things then varied, one at a time: the corpus
at fixed network and step count, then the network and step count at the full
corpus. Rated by `evaluate.py --temperature 0.3` against `greedy`, seats
swapped, paired seeds. The temperature matters more than it looks — the same
checkpoint rates 5.5% at 1.0 (`2026-08-25-ppo-first-run.md`).

**Result.**

| change | held-out acc | vs greedy |
|---|---|---|
| 243 games, small net, fixed steps | — | 4.5% ±1.5 |
| 2,446 games, same net, same steps | — | 6.2% ±1.7 |
| 2,446 games, 96×8, 15k steps | 0.450 → 0.464 | 16.5% ±2.6 |
| the same checkpoint, re-rated over 400 games | | 19.0% ±2.0 |

**Reading.** Ten times the data moved play by less than the error bars. That is
not evidence the corpus is useless — it is evidence the *run* was never reaching
for it: 15,000 steps sees roughly 768k orders, less than one pass over the
986,869 the corpus now holds. Data cannot bind while the model has not finished
looking at what it already had.

Capacity and steps did move it, 6.2% → 16.5%, and the same checkpoint measured
19.0% once the sample was doubled — the 200-game rating was simply noisy, which
is worth remembering when reading any single number here.

The accuracy column is the useful surprise. It moved 0.450 → 0.464, a point and
a half, across a change that nearly tripled play strength. Held-out accuracy is
a check that a run is not *broken*; it cannot rank two runs that both work. See
`decisions.md`, "A policy is rated by playing".
