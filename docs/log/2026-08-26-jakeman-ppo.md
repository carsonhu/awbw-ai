# The recipe that beat greedy goes backwards against JakeMan

> The RL checkpoints survived the opening fix, and PPO from the best of them
> never beat its own start against JakeMan, decaying to 7.6%. Two lessons.

**Question.** `4e613aa` fixed the opening economy *after* every RL checkpoint
was trained, re-rating the clone 5.5% → 10.8%. Did the RL checkpoints move
too? And can PPO, from the best of them, climb JakeMan as it climbed `greedy`?

**Re-rating.** `evaluate.py`, temperature 1.0, 400 games each, fixed
environment:

| checkpoint | vs greedy | vs jakeman |
|---|---|---|
| `ppo` | 96.1% ±1.0 | 37.8% ±2.4 |
| `spR` (self-play, refresh) | 92.4% ±1.3 | **43.9% ±2.5** |
| `bc-scaled` | 10.8% (decisions.md) | 4.0% ±1.0 |

The RL checkpoints survived the fix almost untouched — `ppo` scores 96.1%
against `greedy` where it peaked at 96.2% before it. RL had already walked
away from the corpus opening; the clone, which is nothing *but* the corpus
opening, doubled. (`sp` and `spR` were the pre-fix self-play runs; their
training arguments went unrecorded, and these ratings are what stand for
them.) Post-fix, `arena` has JakeMan 20-0 over every other bot, and the
greedy mirror sits at 28.7% for seat 0 against 40/60 before the fix — paying
the first player on day one moved the seat balance.

**The run.** `ppo.py --init checkpoints/spR.pt --opponent jakeman`, all
defaults (200 iterations of 32 envs × 64 steps, warm-up 25, shaping 0.1,
lr 1e-4, seed 11), ~24 minutes. Rollout opened at 36.2%, as `spR`'s rating
said it would.

| weights | vs jakeman |
|---|---|
| `spR` (the start) | 43.9% ±2.5 |
| `ppo-jake` (kept "best") | 38.8% ±2.4 |
| `ppo-jake-last` (iteration 200) | **7.6% ±1.3** |

**It never improved on its start.** The kept "best" is iteration 20 — inside
the critic warm-up, policy still frozen `spR` — and no later window beat it.
Post-warm-up windows oscillated 17–56% around a flat-to-sinking mean, the
last seventy iterations slid for real (13.2% over the final 72 rollout
games), and draws at the day cap doubled. This is not the saturation failure:
JakeMan was never close to beaten, value loss held near 0.01, and `kl`/`clip`
sat nominal while the policy walked downhill. Why is not established —
candidates are the shaping term paying for expansion JakeMan punishes, and
three epochs at lr 1e-4 being too hot for a signal this even.

**The guard was disarmed by its own bar.** "Keep the best window" let a lucky
38-game warm-up window claim 57.9% for weights whose true rating is ~40%, and
a ±8-point window sets a bar no genuine +5 could clear. The floor a
checkpoint must beat needs more games than `--min-games 20` gives it, or the
saved "best" is the start wearing a lucky number.

**Where this leaves training.** A fixed opponent has now failed at both ends:
`greedy` by saturating (`log/2026-08-25-ppo-first-run.md`), JakeMan by this.
The standing candidates are self-play with refresh from `spR` in the fixed
environment, with JakeMan as the yardstick rather than the teacher, or a
hyperparameter hunt on this run. The first needs no new answers to be wrong
about; the second does.
