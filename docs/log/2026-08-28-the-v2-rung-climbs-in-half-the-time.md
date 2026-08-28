# The v2 rung climbs in half the time; the peak is a panel question

> Same recipe, planes v2: the clone opens at 25% against greedy where
> the v1 clone opened at 6 — before a single gradient step — and the
> rung reaches 90% by iteration 50 where v1 needed 80. The kept
> window, 90.6 against v1's 93.7, is inside window noise; the rated
> comparison is deliberately left for the next session.

**Setup.** `ppo-t2v1`: the greedy rung a third time (`--opponent
greedy`, standard flags, 200 iterations), init `bc-threat2` — the
clone trained on threat planes v2 (`5c98a37`): expected damage and
P(KO) over each CO's luck range instead of the zero-luck floor.

| greedy rung | v1 (`ppo-threat1`) | v2 (`ppo-t2v1`) |
|---|---|---|
| clone baseline (warm-up window) | ~6% | **25%** |
| window at iteration 40 | 48.4% | **72.5%** |
| iterations to a 90% window | ~80 | **~50** |
| best kept window | 93.7% | 90.6% |
| held-out order accuracy | 0.459 | 0.426 |

**What is settled.** The probability planes help *imitation* directly:
a quadrupled bot-fighting baseline from the same corpus, same recipe,
same seeds — the observation now carries what the demonstrators
conditioned on, and the decoupling rule shows its sharpest case yet
(three points less accuracy, four times the play strength). And they
speed RL: the climb runs ~25 points ahead of v1 at every early
checkpoint.

**What is not.** Whether the v2 *plateau* is higher — 90.6 vs 93.7 on
kept windows is one seed against one seed inside the windows' spread,
and the run ended in the usual saturation oscillation. The 200-game
panel of `ppo-t2v1` is the first command of the next session, followed
by the v2 JakeMan rung (v1's bar: 37.5 rated, then 63.4 continued).

**Session stop.** Stopped here on instruction; nothing running.
`ppo-threat3` (v1, panel-sweeper: 86.5/63.4/87.5/75.5) remains the
checkpoint of record; `bc-threat2` and `ppo-t2v1` open the v2 book.
