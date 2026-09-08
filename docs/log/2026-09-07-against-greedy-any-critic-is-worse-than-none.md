# Against greedy, any critic is worse than none

> The previous entry left one question: is the `greedy` collapse the shaped
> objective or greedy selection? Both, and the split is measurable. Noise
> reproduces the baseline exactly -- 70.0 decisive against 72.5, cap 7.0
> against 7.0 -- so the reranking path is neutral. A win-probability head
> costs 13.5 decisive points. The shaped head costs 21.5. Against an opponent
> this saturated, *any* critic ranking is worse than not ranking, and the
> objective accounts for a bit under half the damage.

**Setup.** `mixjm-s43` as Adder against `greedy`, `--rerank 4`, 200 games a
cell, three rankers against the plain policy: Gaussian noise, `bc-net2-t4`'s
recorded-outcome head (0.859 on `value_diag`), and the policy's own shaped,
turn-discounted head with the cap tiebreak in it (0.807).

| ranker | score | decisive | vs none | capped |
|---|---|---|---|---|
| none | 79.0 | 72.5 | -- | 7.0 |
| noise | 76.5 | 70.0 | -2.5 (z=-0.55) | 7.0 |
| `bc-net2-t4` head | 77.5 | 59.0 | -13.5 (z=-2.87) | 19.5 |
| own head | 88.0 | 51.0 | -21.5 (z=-4.54) | **37.5** |

**The control is clean here too.** Noise lands on the baseline in both columns
-- and unlike the JakeMan run it does not dip, because against `greedy` a
larger share of candidates end the game and the control still scores those by
their real result. The reranking machinery is not what breaks; ranking is.

**The objective is worth a bit under half of it.** Swapping the shaped head
for a win-probability head recovers 8.0 decisive points (z=1.61) and cuts the
cap rate from 37.5% to 19.5% (z=-4.07). That is the same fraction the JakeMan
cells showed, which is now two independent measurements of the cap tiebreak's
price.

**The rest is what "saturated" means.** A win-probability head that scores
0.859 against recorded human outcomes still costs 13.5 decisive points here.
When the opponent is beaten 79% anyway, every candidate is winning, and both
heads reward *being ahead* rather than *closing* -- the shaped one explicitly,
the clone's because a human game's recorded winner is usually the player who
accumulated the advantage. Maximise "ahead" against an opponent who cannot
punish you and you never close. Noise does not do this because the argmax of
noise is still a policy sample; only systematic selection can chase a
direction the position does not contest.

**So the third form of one pathology.** Training a critic against `greedy`
wrecks it; searching with a critic against `greedy` wrecks the play; and in
both cases the mechanism is that a saturated opponent leaves the value head
nothing to separate but the day cap. Everything downstream of a value head
wants a contested opponent.

**What it does not settle.** One arm, one width, one saturated opponent --
`greedy` is the only bot here weak enough to test the claim, so "saturated"
is one data point dressed as a category.
