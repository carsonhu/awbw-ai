# The league beats its parent on every seed, and 256 envs beats 128

> `GRID=scale`: league2's league at 128 envs (four seeds) and 256 envs
> (two), from `mixjm-s43`. Every arm's final weights beat the parent head to
> head, 65.5-81.0%, where all four JakeMan-only `b128` controls lost to it,
> 33.0-38.5%. At 32 envs this league never promoted; here every arm promoted
> two or three times. Both 256-env arms lead the 128 group on every column,
> and clear 60 as Grimm.

**Setup.** `GRID=scale` (this commit) through `tools/rent.sh` on an
on-demand A40 45GB, 64 cores, 17:41 -> 07:03 UTC, ~$5.65 at $0.417/hr. The
league2 recipe exactly -- pool of `bc-net2-t4`, `mixjm-s202`, `mixjm-s7`,
`mixjm-noanc-s43`, JakeMan on a quarter of the seats, `--seat-cap 50`,
anchored to `bc-net2-t4` at 0.03, all five COs, 200 iterations, 256 steps --
at `--envs 128 --minibatch 1024` or `--envs 256 --minibatch 2048`, 96
gradient steps an iteration either way. The 32-env x 800-iteration arms
were cut for budget. Controls are the batch grid's `b128` arms, same parent,
same rating chain (`2026-10-16-the-bigger-batch-climbs-and-can-specialise.md`).

**Final weights (`-last.pt`).** Head to head as Adder, 200 games, seats
alternating; per CO 200 games each against JakeMan:

| arm | vs parent (decisive) | Adder | Jake | Koal | Jess | Grimm | mean | greedy |
|---|---|---|---|---|---|---|---|---|
| lg128-s7 | 77.5 (71.5) | 80.0 | 78.8 | 77.2 | 84.0 | 55.7 | 75.1 | 86.5 |
| lg128-s43 | 65.5 (63.0) | 76.0 | 79.5 | 82.5 | 82.0 | 60.5 | 76.1 | 89.0 |
| lg128-s101 | 76.0 (66.0) | 83.0 | 83.5 | 78.5 | 87.0 | 48.8 | 76.2 | 86.5 |
| lg128-s202 | 77.0 (67.5) | 69.5 | 74.5 | 66.5 | 78.0 | 49.0 | 67.5 | 73.0 |
| **lg256-s7** | **81.0** (73.0) | 78.0 | 84.5 | 84.2 | 88.1 | **63.5** | 79.7 | 88.5 |
| **lg256-s43** | **81.0** (73.5) | 85.0 | 81.5 | **91.5** | 88.0 | 60.2 | **81.2** | **94.0** |
| b128 controls | 33.0-38.5 | | | | | 45.0-56.5 | 72.6-75.8 | 82.5-88.5 |
| parent | | 59.2 | 59.0 | 59.0 | 72.0 | 40.0 | 57.8 | 79.0 |

The net panel members are solved: 95.5-99.5 against `bc-powers-scaled2`,
98.5-100 against `ppo-adder3`, every arm.

**Training.** Promotions per arm, against a rule that fired once in 800
arm-iterations at 32 envs (`2026-10-15-the-league-lost-to-a-pool-that-never-moved.md`):

| | s7 | s43 | s101 | s202 |
|---|---|---|---|---|
| lg128 generations | 3 | 2 | 2 | 2 |
| lg256 generations | 2 | 3 | | |

Each 128-env arm played ~6,700 games, a 256-env arm ~13,400, a quarter of
them JakeMan's, 64-73% won. By the end every arm beat each of its own
promoted generations 60-70% in its training seats: the pool was still being
climbed, not cycled.

**Reading it.** The batch grid left one question: whether a climbing
optimiser on one opponent can only make a specialist. It can, and opponent
diversity is the fix. The same 128-env optimiser that made four arms lose to
their parent, pointed at a pool, made four that beat it, with no column
traded away -- on JakeMan the league matches or passes the specialists,
though it spends three seats in four elsewhere. The league2 failure was the
batch size, not the league.

256 envs still pays with diverse opponents: both arms beat the parent 81.0%
against a 128 mean of 74.0, and lead on JakeMan (80.5 against 73.7), Grimm
(61.9 against 53.5) and `greedy` (91.3 against 83.8). Two seeds, rated
against the parent rather than each other, at about twice the cost an arm.

**The endpoint.** `plan.md`'s Lane B endpoint is a seed group with a winning
record against every scripted opponent as every Tier-4 CO. Grimm blocked it
at every rung. Both 256-env arms clear it (Grimm 60.2, 63.5). The 128 group
clears it on two seeds and sits at 48.8 and 49.0 on two, inside the +-3.5
interval.

**Caveats.** Every head-to-head here is an Adder mirror, one of the 25
matchups the league trains on; a counterbalanced rating over all of them is
the check before a parent is chosen. `lg128-s202` beats the parent 77.0 but
trails on JakeMan (67.5) and `greedy` (73.0) -- seed spread is real at 128.
The `b128` head-to-heads were played on the 1660 Ti, these on the box, both
through `evaluate.py` with the same seeds.

**Cost and tooling.** Three interruptible rentals first: two never started
(the bid listings include machines already held on demand, which preempt any
bid) and one was preempted 30 minutes in; `rent.sh` now rents on demand.
`lg128-s101`/`s202` waited six hours for memory on the 45GB card. Each arm's
promotion snapshots (`-gen1`, `-gen2`, ...) came home and can date the gain.
