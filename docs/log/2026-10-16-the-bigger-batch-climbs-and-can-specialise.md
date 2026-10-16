# The bigger batch climbs where the small one decays, and can specialise

> `GRID=batch`: the lg2ctl recipe at 32 and 128 envs, minibatch scaled so
> both take 96 gradient steps an iteration. At final weights against JakeMan
> every 32-env arm ends below its parent (mean of five COs 37.7-43.5 against
> 57.8) and every 128-env arm far above it (72.6-75.8), seeds within 3.2
> points. "Decays from level" was update noise. But every 128-env arm's
> final weights lose to the parent head to head, 33.0-38.5%, while beating
> `greedy` 82.5-88.5 to its 79.0: optimised well, one scripted opponent
> makes a specialist in bot-shaped play.

**Setup.** `GRID=batch` (`3c1998b`) on the rented 4090 48GB, after
`league2`, six lanes, `OMP_NUM_THREADS=2`, 01:09 -> 07:12 UTC. Recipe of
`lg2ctl` exactly: from `mixjm-s43`, anchored to `bc-net2-t4` at 0.03, all
five COs, `--opponent jakeman`, 200 iterations, 256 steps. Arms differ only
in `--envs 32 --minibatch 256` (8,192 rows an update, 3,066 games a run)
against `--envs 128 --minibatch 1024` (32,768 rows, 10,769 games).

**Training windows against JakeMan** (~150 games a window at 32 envs, ~500 at 128):

| arm | early | end |
|---|---|---|
| b32 s7/s43/s101/s202 | 57-61% at it. 30 | 52.5 / 47.3 / 30.6 / 36.4 |
| b128 s7/s43/s101/s202 | ~58% at it. 20 | 71.7 / 71.2 / 74.9 / 76.9 |

**Final weights (`-last.pt`), 200 games a CO against JakeMan,** re-rated on
the same box (`rate_last.sh`), parent included:

| | Adder | Jake | Koal | Jess | Grimm | mean |
|---|---|---|---|---|---|---|
| parent `mixjm-s43` | 59.2 | 59.0 | 59.0 | 72.0 | 40.0 | 57.8 |
| b32-s7 | 45.0 | 47.5 | 50.2 | 53.5 | 21.5 | 43.5 |
| b32-s43 | 35.0 | 37.0 | 44.0 | 52.5 | 20.0 | 37.7 |
| b32-s101 | 40.5 | 42.8 | 36.5 | 45.0 | 29.5 | 38.9 |
| b32-s202 | 37.0 | 40.0 | 43.0 | 55.5 | 24.5 | 40.0 |
| **b128-s7** | 78.0 | 77.8 | 77.0 | 86.0 | 49.8 | **73.7** |
| **b128-s43** | 81.5 | 81.0 | 78.0 | 79.0 | 45.5 | **73.0** |
| **b128-s101** | 79.5 | 80.8 | 81.0 | 81.0 | 56.5 | **75.8** |
| **b128-s202** | 80.0 | 82.0 | 75.5 | 80.5 | 45.0 | **72.6** |

The lg2ctl arms, the same recipe at 32 envs in the grid before, ended at
24.6-54.3 (`2026-10-15-the-league-lost-to-a-pool-that-never-moved.md`):
eight 32-env arms, all below the parent, spread 30 points. Four 128-env arms,
all above it, spread 3.2. Grimm, the CO that gated every rung, reaches
45.0-56.5 against the parent's 40.0.

**Kept checkpoints and the panel.** b32 kept iteration 30-110, near the
parent (`b32-s43` per CO is its to 1.5 points); b128 kept 80-180. As Adder:

| arm | greedy | JakeMan | bc-powers-scaled2 | ppo-adder3 |
|---|---|---|---|---|
| b32-s7 / s43 / s101 / s202 | 56.0 / 83.5 / 82.5 / 77.5 | 35.5 / 50.2 / 49.0 / 53.5 | 67.0 / 85.5 / 73.0 / 84.0 | 76.5 / 89.0 / 87.5 / 95.0 |
| b128-s7 / s43 / s101 / s202 | 75.0 / **93.5** / 79.5 / 73.5 | 76.6 / 75.5 / 75.5 / 74.5 | 82.5 / **92.0** / 85.5 / 77.0 | 90.0 / **96.5** / 92.5 / 80.0 |

**Head to head, against nets no arm trained on** (Adder, 200 games, seats
alternating). Kept checkpoints:

| | vs `mixjm-s43` (parent) | vs `mixjm-s202` | vs `mixjm-noanc-s43` | vs `bc-net2-t4` |
|---|---|---|---|---|
| b128-s43 (it. 80) | **61.0** / 56.5 | 67.0 / 63.0 | 65.5 / 59.5 | 85.5 / 78.5 |
| b128-s7 (it. 180) | 36.5 / 32.0 | | | |
| b128-s101 (it. 140) | 42.5 / 35.0 | | | |

Final weights, iteration 200, against the parent (played locally on the
1660 Ti, same evaluator and seeds):

| | b128-s7 | b128-s43 | b128-s101 | b128-s202 |
|---|---|---|---|---|
| vs `mixjm-s43`, score / decisive | 36.8 / 35.0 | 37.5 / 35.5 | 33.0 / 29.0 | 38.5 / 30.0 |

Every one loses to its parent, `b128-s43` included (61.0% at iteration 80,
37.5% at 200): not a seed's bad luck, but what more iterations buy.

Not a general decline: the same final weights against `greedy`, which no arm
trained on (parent 79.0 on the same machine):

| | b128-s7 | b128-s43 | b128-s101 | b128-s202 |
|---|---|---|---|---|
| vs `greedy`, score / decisive | 82.5 / 79.0 | 82.5 / 79.5 | 88.5 / 82.0 | 83.5 / 73.5 |

Better than the parent against both scripted bots, worse against a net. They
learned to beat bot-shaped play, and what they gave up for it a learned
opponent finds.

**Reading it.** Batch size was the bound on optimising: at ~15 finished
games an update the arms random-walk down from level, at ~54 they climb the
objective and every seed lands in the same place. The objective is one
opponent, and climbing it well is not the same as getting stronger: by
iteration 200 every 128-env arm beats JakeMan by sixteen points a CO more
than its parent does and loses to that parent head to head. A bad optimiser
hid the problem -- it could not exploit JakeMan either. A rung of one
opponent is finished as a method; opponent diversity at 128 envs, the league,
is the next question, rated head to head against the parent from the start.

**Cost.** ~$6 for both grids at $0.645/hr. Six lanes left `b128-s202` alone
for its last hours though all eight fit; `grid.sh` now packs lanes by memory,
longest first. `--amp` here: 953 -> 1,091 orders/s (+15%); left off.

**Not settled:** noise per update or total experience (b128 played 3.5x the
games); 32 envs for 800 iterations separates them.
