# Self-play drifts below its own frozen copy, and three fixes did nothing

> The learner loses to a snapshot of itself at 35-41% across every
> configuration tried. Recalibration, step size and shaping are all cleared.
> The one path the working control never exercises is the two-player advantage.

**The symptom.** In self-play the learner is rated against a frozen copy of
itself, and two copies of one policy should sit at half. From `spR` it sits
well below, so it can never reach the 70% that promotes a generation, and a
run produces nothing.

**What was tried**, all from `spR`, all with `--recalibrate 0` after that was
found to cost seventeen points on its own (`log/2026-08-26-recalibration.md`):

| configuration | post-update score | games |
|---|---|---|
| recalibration off | 39% | 89 |
| `--adv-floor 0.12` | 41% | 71 |
| `--shaping 0` | 35% | 84 |

Each is two to three sigma below parity, and they are indistinguishable from
each other. The floor was aimed at advantage normalisation rescaling a
well-predicted rollout to full-size noise; measured, the healthy regime runs
at spread 0.12-0.13 and self-play at 0.05-0.09, a factor of two rather than
the order of magnitude the theory wanted, and damping it changed nothing.
Removing shaping was aimed at the win signal being drowned by material; it
changed nothing either, and the terminal signal alone still produced spread
around 0.04, so it is present.

**What is cleared.** The harness (`spR` against itself rates 52.8% ±3.5 over
200 games), the opening fix, and the PPO update — a control reproducing the
pre-fix recipe against `greedy` with recalibration off climbed from 7%, and
its kept weights rate 83.1% ±1.9 against `greedy` and **46.0% ±2.5 against
JakeMan**, the best of any checkpoint here. It is the one usable artifact of
the day, and the natural start for the next self-play attempt.

That checkpoint also pays the window lesson again: the rollout window that
saved it read 93.1%, and its rating is 83.1%. Ten points of fluke, in the
direction that makes a run look finished.

**The remaining lead.** That control is also the reason to look at the
two-player advantage path. With a scripted opponent the caller only ever sees
its own seat, so `buf.mine` and the sign flip in `advantages()` do nothing;
in self-play both carry the whole update. The code the working configuration
never touches is the code the failing one depends on. Inspection found no
defect — `flip` is +1 within a turn and -1 across a change of turn, applied to
both the bootstrap and the accumulator — so the next step is a test rather
than more reading: with learner and frozen weights identical, the learner's
mean advantage over a rollout should be zero, and a systematic sign would
show up directly.

**A caution for whoever picks this up.** Three separate calls in this session
were made on 27, 32 and 33-game samples and all three were wrong; the
project's own rule is 200 games for a rating. A rollout window is worth
roughly ±9 points and cannot distinguish any of the numbers in the table
above from any other.
