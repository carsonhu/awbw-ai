# One ply flattens the COs, and collapses against greedy

> Reranking rated across the board, not just as Adder. Two findings. Against
> JakeMan it **compresses the CO spread**: the five COs run 39.0-70.0 decisive
> without search and 61.5-66.5 with it, Grimm gaining 26.0 points (z=5.4) and
> Jess losing 3.5. And against `greedy` it **collapses** -- 72.5 decisive down
> to 51.0 (z=-4.5) on 37.5% capped games, up from 7.0%. One ply is worth most
> against the opponent that contests the position and turns into cap-farming
> against the one that does not.

**Setup.** `mixjm-s43`, `--rerank 4`, its own value head, 200 games a cell
(Adder against JakeMan at 400). K=1 columns are the rented grid's ratings.
`bc-powers-scaled2` and `ppo-adder3` are missing on purpose: `Duel` drives the
heads itself, so it would have discarded the search and rated the plain policy
while printing a reranked-looking number. `evaluate.py` now refuses that
combination rather than answering it wrongly.

| opponent / CO | K=1 dec | K=4 dec | diff | z | K=1 cap | K=4 cap |
|---|---|---|---|---|---|---|
| `greedy` (Adder) | 72.5 | **51.0** | **-21.5** | -4.54 | 7.0 | **37.5** |
| JakeMan, Adder | 55.5 | 61.5 | +6.0 | 1.40 | 5.5 | 17.0 |
| JakeMan, Jake | 54.5 | 65.5 | +11.0 | 2.26 | 9.5 | 12.5 |
| JakeMan, Koal | 56.0 | 61.5 | +5.5 | 1.12 | 6.5 | 15.0 |
| JakeMan, Jess | 70.0 | 66.5 | -3.5 | -0.75 | 3.5 | 20.5 |
| JakeMan, Grimm | 39.0 | **65.0** | **+26.0** | 5.39 | 2.0 | 8.0 |

Against the local 400-game Adder baseline (53.2) the gain is +8.3, z=2.38; the
table uses the grid's 200 games so the six rows are measured alike.

**The gain is largest where the arm was worst, and it lands everything in the
same place.** Ordered by K=1 decisive -- 39.0, 54.5, 55.5, 56.0, 70.0 -- the
gains run +26.0, +11.0, +6.0, +5.5, -3.5, monotone. A 31-point spread across
the five COs becomes a 5-point one. One ply is not adding a fixed increment of
skill; it is removing a per-CO tactical deficit, and Grimm -- +30% attack,
-20% defense, so every trade prices differently -- had the most of it to
remove. That also revises this morning's reading: Grimm's hole is tactical and
a bad-trade filter closes it, which is not what "the CO is simply weaker"
predicted, even though humans take a penalty the same size.

**`greedy` fails the same way `mix-s43`'s critic did.** An opponent beaten
79-88% leaves nearly every position winning, so the value head has nothing to
separate but income and the cap tiebreak -- and a selector gets what it
optimises. This is the morning's finding in a second form: a saturated
opponent starves the value head, and everything downstream degrades, whether
that is training it (`log/2026-09-07-the-critic-falls-with-greedy-and-returns-with-jakeman.md`)
or searching with it.

**The endpoint claim changes, and not cleanly.** With reranking `mixjm-s43`
holds a winning record on decisive share against JakeMan as *all five COs* --
61.5, 65.5, 61.5, 66.5, 65.0 -- so Grimm no longer needs pricing against CO
strength to clear the bar. But `greedy` falls to 51.0 on 37.5% capped, which
is a worse artifact than the one it replaces. Reranking is not a default.

**What it does not settle.** Two net panel members unrated, one width, one
arm. Whether the `greedy` collapse is the shaped objective or greedy selection
is testable with the clone's head, which cut a third of the cap inflation
against JakeMan and costs one run here.
