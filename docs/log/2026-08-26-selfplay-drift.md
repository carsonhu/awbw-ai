# Self-play drifts below its own frozen copy, and three fixes did nothing

> The learner loses to a snapshot of itself at 35-41% across every
> configuration tried. Recalibration, step size and shaping are all cleared.
> The one path the working control never exercises is the two-player advantage.

**The symptom.** The learner is rated against a frozen copy of itself, and two
copies of one policy should sit at half. From `spR` it sits well below, never
reaches the 70% that promotes a generation, and the run produces nothing.

**What was tried**, all from `spR`, all with `--recalibrate 0` after that was
found to cost seventeen points on its own (`log/2026-08-26-recalibration.md`):

| configuration | post-update score | games |
|---|---|---|
| recalibration off | 39% | 89 |
| `--adv-floor 0.12` | 41% | 71 |
| `--shaping 0` | 35% | 84 |

Each is two to three sigma below parity and they are indistinguishable from
each other. The floor was aimed at normalisation rescaling a well-predicted
rollout to full-size noise; measured, the healthy regime runs at spread
0.12-0.13 against self-play's 0.05-0.09 — a factor of two, not the order of
magnitude the theory wanted — and damping it changed nothing. Removing
shaping was aimed at the win signal being drowned by material; it changed
nothing either, and the terminal signal alone still spread around 0.04.

**What is cleared.** The harness (`spR` against itself rates 52.8% ±3.5 over
200 games), the opening fix, and the PPO update — a control reproducing the
pre-fix recipe against `greedy` with recalibration off climbed from 7%, and
its kept weights rate 83.1% ±1.9 against `greedy` and **46.0% ±2.5 against
JakeMan**, the best of any checkpoint here. It is the one usable artifact of
the day, and the natural start for the next self-play attempt.

**The remaining lead.** That control is also why to look at the two-player
advantage path: with a scripted opponent the caller only sees its own seat, so
`buf.mine` and the sign flip in `advantages()` do nothing, while in self-play
they carry the whole update. Inspection found no defect, so the test was run
instead — with learner and frozen weights identical the two sides are one
player, and their advantages must come from one distribution.

| | 20 rollouts, weights identical |
|---|---|
| critic spread | 0.41 — not collapsed, so advantages are not noise |
| advantage gap | -0.0046 ±0.0036, marginal |
| **learner's row share** | **46.1%**, and 47.1% on a separate run |

The row share is the one that does not go away. Seats alternate 16/16, so the
learner's share is `(seat 0's share + seat 1's share) / 2` — exactly a half
whatever the map's asymmetry does to orders per turn, because the two groups
cancel. It is not a half. `actors` is read before the action is applied, so
the flip is aligned and that is not it. Unexplained, reproducible, and in the
path a working vs-a-bot run never touches: start here.

**A caution for whoever picks this up.** Three calls in this session were made
on 27, 32 and 33-game samples and all three were wrong; the first version of
the probe above called its gap significant by pooling correlated rows; and the
window that kept `ctrl-greedy` read 93.1% against a rating of 83.1%. A rollout
window is worth about ±9 points, always flattering. The rule is 200 games.
