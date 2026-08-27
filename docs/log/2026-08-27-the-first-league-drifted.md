# The first league drifted, because promotion refilled it with cousins

> Seven promotions, 200 iterations, and the result is worse than the
> checkpoint it started from against every opponent outside the pool —
> and exactly level with that checkpoint head to head. A league of
> near-neighbours is not diversity.

**Setup.** `--init ppo-adder3 --frozen-init` on five: the clone,
`ctrl-recal-off`, `sp-parity-gen3`, `ppo-adder1`, `sp-behind3-gen1`.
Round-robin, one member per iteration, each promotion appending.

| panel, 200 games each | `ppo-adder3` (the init) | `league1-gen7` |
|---|---|---|
| vs `greedy` | 62.5% | **14.5%** |
| vs JakeMan | 15.5% | **3.5%** |
| vs the clone | 39.0% | **24.5%** |
| vs `ppo-adder3` | — | 50.5% |

Two hundred iterations to arrive back at its own starting strength, by
the only measure that was ever going to be flattering, while losing
forty-eight points against `greedy`.

**Why.** Promotion appends the generation just made, which is right for
not forgetting, and wrong at this ratio: by the end **seven of the
twelve members were its own promoted lineage**. Round robin then spends
most of the run against recent copies of itself — the collapse the
league exists to prevent, reintroduced through the mechanism meant to
grow it. The five seeded members were also not a hard bar: the clone
rates 2% against `greedy`, and two of the others are earlier rungs of
the same family. Beating that pool is not the same skill as beating a
bot.

**What this does not show.** That leagues do not work. The pool was
mostly one species and the sampling gave every member equal time
regardless of whether it was a threat. Both are fixable and neither was
tested here.

**What to change, in order.** Cap the self-lineage share of the pool, or
weight sampling by loss rate so the members that beat the learner get
the iterations — prioritised fictitious self-play, a few lines in
`take_opponent`. Then get genuinely different species in: exploiters
started fresh from the clone and pointed at the current best, and clones
trained on separate slices of the corpus rather than one average over
all of it. `2026-08-27-the-decay-was-the-bug...` has the panel that
makes any of this legible; without it this run would have read as a
success at 59%.
