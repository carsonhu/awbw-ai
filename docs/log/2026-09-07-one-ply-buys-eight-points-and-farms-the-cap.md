# One ply buys eight points, and farms the cap

> The first rung of the search ladder, built and measured. Sampling four
> candidate orders and keeping the one the value head likes best is worth
> **+8.3 decisive points** to `mixjm-s43` against JakeMan, for four times the
> orders. It also triples that arm's day-cap rate, 4.5% to 17.0%, because the
> value head predicts a shaped return with the cap tiebreak inside it and one
> ply of search pursues exactly what it is told to. And the standing gate --
> a search is worth what its evaluator knows -- comes out unsupported: the arm
> with a near-chance critic gains as much, within the noise.

**Setup.** `lookahead_into` applies a candidate order to a copy of each game
and returns the observation that would follow, with a sign for whether the
turn passed (that view belongs to the opponent) and a result where the order
ended the game. The luck source is copied with the state, so the preview is
exact -- `smoke_test.py` asserts equality against the real order. `Rerank`
samples `--rerank K` candidates and keeps the best. Two arms of the same rung
with critics 0.24 apart (`log/2026-09-07-the-critic-falls-with-greedy-and-returns-with-jakeman.md`),
rated as Adder against JakeMan, 400 games each but `s101` at K=4 (201).

| arm | critic | metric | K=1 | K=4 | diff | z |
|---|---|---|---|---|---|---|
| `mixjm-s43` | 0.807 | score | 56.1 | 76.5 | **+20.4** | 6.25 |
| | | decisive | 53.2 | 61.5 | **+8.3** | 2.38 |
| | | capped | 4.5 | 17.0 | **+12.5** | 5.83 |
| `mixjm-s101` | 0.564 | score | 36.5 | 50.7 | **+14.2** | 3.33 |
| | | decisive | 35.8 | 41.8 | +6.0 | 1.42 |
| | | capped | 6.5 | 10.4 | +3.9 | 1.57 |

**Read the score column and search looks like a free 20 points.** Read the
decisive column and it is worth 8.3, still the largest single-change gain
this project has measured, and read the cap column to see where the other 12
went. This is the six-arm grid's lesson arriving from a new direction: the
number that moves most is the one that counts games nobody finished.

**The critic's objective is what search amplifies, not its accuracy.** The
value head is trained on a shaped, turn-discounted return with `--decide-cap`
in the terminal reward -- it was never a win probability, and PPO only ever
used it as a baseline. Reranking promotes it to a selector, and a selector
gets exactly what it optimises. The tell is that the *better* critic farms
the cap harder: +12.5 points of capped games against +3.9. A critic that
reads the position better pursues its own mismatched objective more
faithfully.

**The gate is not supported at this power.** `s101` plays with a critic at
0.564 accuracy -- a coin flip on openings -- and still gains 6.0 decisive
points. The gap between the two arms' gains is +2.3 against a standard error
of 5.5, so they are indistinguishable. Whatever one ply of reranking is
buying, it is not being bought with the critic's grip on human positions.

**What it does not settle.** One width, one opponent, one CO, two arms. K=4
costs four times the orders (247/s against 1,006/s) and nothing here says
where the curve turns over. The obvious follow-up is not a wider search: it
is whether a value head trained as a win probability -- the clone's, at 0.859
-- reranks better than one trained on shaped return, which would separate the
two explanations this entry cannot.
