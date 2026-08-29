# Forcing the pop teaches the rate, not the timing

> Rated without the crutch, `ppo-t2pop` pops **1.33/game — thirteen
> times its control — at order 46 of a 30.6-order turn**: it learned
> *to* pop, and did not learn *when*. And combined with the anchor,
> even the rate vanishes (0.07/game). Yesterday's entry credited the
> arm's panel gain to transferred buff-state experience; that now reads
> as exactly right, and narrower than hoped.

**Measured, all unforced, 30 games each.**

| arm | pops/game | at order / turn length |
|---|---|---|
| control (`ppo-t2v1`) | 0.10 | 25 / 20.4 |
| force alone (`ppo-t2pop`) | **1.33** | 46 / 30.6 |
| anchor 0.03 alone, seed A | 0.20 | 11 / 21.1 |
| anchor 0.03 alone, seed B | 0.00 | — |
| force + anchor (`ppo-a3pop`) | 0.07 | 23 / 20.6 |

**Why the timing cannot transfer.** During training the pop is the
turn's first order because every alternative is masked; the source
head never scores "pop now" against "move first", so no preference
forms. Unforced, the policy reverts to spending the turn before
pressing the button — more often than before, because the buff states
became familiar, but no earlier. The forcing also lengthens turns
(30.6 orders against 20.4), which is buff-state play showing through.

**The stack fails both ways.** `ppo-a3pop` (force + anchor 0.03)
panels at 89.5 / 8.0 / 57.5 / 53.5 — a few points over anchor-0.03
alone, far under anchor-0.01 alone, and its pops collapse to control
level. The anchor pulls toward a clone that almost never pops (213
labels in 1.9M orders), so on rare decisions the two interventions
tug in opposite directions and the anchor wins.

**Where this leaves powers.** Rate responds to exploration; timing has
resisted the mask, the window, the anchor, and their combinations. The
untried lever remains upweighting the corpus's 213 activation labels
at BC — putting the timing into the *clone*, which the anchor now
provably propagates into RL. That is a bc.py change, and it rides the
next re-clone for free.
