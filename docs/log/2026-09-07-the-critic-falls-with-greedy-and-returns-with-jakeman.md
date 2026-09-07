# The critic falls with greedy and returns with JakeMan

> Search is gated on the critic, so the critic got measured across the
> lineage. The clone has the best one on record -- 0.859 accuracy on
> endgames, Brier 0.108. One rung against `greedy` destroyed it: `mix-s43`
> reads 0.553 and Brier 0.406, *worse than predicting 0.5 everywhere*. The
> JakeMan rung brought it back to 0.807/0.140 without being asked to. Across
> the four PPO arms measured, critic quality and rating move together.

**Setup.** `py -3.12 python/value_diag.py --checkpoint <ckpt>`, defaults --
86,498 positions from 260 held-out corpus games, labelled with the recorded
winner, scored by accuracy (sign agreement) and Brier. The lineage is
`bc-net2-t4` -> `mix-s43` (200 iterations against `greedy`) -> `mixjm-s43`
(200 more against JakeMan), plus two arms of that second rung for contrast.
Ratings are the Adder score / decisive share against JakeMan.

| checkpoint | rated | 0-25% | 25-50% | 50-75% | 75-100% | Brier (end) |
|---|---|---|---|---|---|---|
| `bc-net2-t4` | -- | 0.551 | 0.655 | 0.750 | **0.859** | **0.108** |
| `mix-s43` | 16.5 | 0.498 | 0.505 | 0.515 | 0.553 | 0.406 |
| `mixjm-s43` | 59.2/55.5 | 0.515 | 0.631 | 0.701 | 0.807 | 0.140 |
| `mixjm-noanc-s43` | 51.7/36.5 | 0.546 | 0.584 | 0.646 | 0.770 | 0.176 |
| `mixjm-s101` | 39.5/39.5 | 0.517 | 0.508 | 0.520 | 0.564 | 0.384 |

**A Brier above 0.250 is not a weak critic, it is a wrong one.** Predicting
0.5 everywhere scores 0.250. `mix-s43` scores 0.475 in the opening quarter and
0.406 at the end, with accuracy at 0.50 -- saturated outputs whose sign is a
coin flip. Its policy was meanwhile the best thing this project had built.
The panel could not see this: `mix-s43` swept its rung.

**The plausible cause is the saturated opponent.** `greedy` was being beaten
93-96% by the end of that rung, so nearly every position carries the same
value and the head has nothing to separate. JakeMan sits at 55-60%, and both
arms trained against it recovered -- the anchored one further (0.807) than the
unleashed one (0.770), which fits but does not prove that the leash helps.
`mixjm-s101` is the counterexample that keeps this a hypothesis: same
opponent, same rung, and its critic stayed broken at 0.564.

**Critic and rating move together across all four PPO arms.** Ordered by
score against JakeMan -- 59.2, 51.7, 39.5, 16.5 -- the endgame accuracies run
0.807, 0.770, 0.564, 0.553, monotone. Four points is four points, and the
direction of causation is open: a policy that cannot tell it is winning plays
worse, and a policy that plays worse sees less informative positions.

**Search has a usable evaluator, and one rung ago it did not.** `mixjm-s43`
is within 0.05 accuracy and 0.03 Brier of the clone, which is the standing
gate on search -- a search is worth what its evaluator knows. Had the gate
been checked after the mixed rung, the answer would have been no.

**What it does not settle.** The corpus is human positions and a PPO critic
has only seen its own games against two bots, so some of the fall is
distribution shift rather than damage; separating them needs this measurement
on the policy's own trajectories, which `value_diag` does not do. The value
head also predicts a shaped, turn-discounted return with the cap tiebreak in
it, not a win probability -- which explains a bad Brier but not an accuracy of
0.50, since sign agreement is scale-invariant.
