# Self-play's missing rows are not missing

> `2026-08-26-selfplay-drift.md` closed on a 46.1% learner row share —
> "unexplained, reproducible, start here". Re-measured on the powers-era
> build it is 50.3%, and it decomposes exactly as that log argued it must.

**The measurement.** Self-play, learner and frozen weights identical (a
fresh `deepcopy`, never updated), Adder mirror from `ppo-adder1`, 8
rollouts of 32 envs x 256 steps — 262k rows, against the 20 rollouts the
original probe used.

| | learner's share of rows |
|---|---|
| overall | **50.3%** |
| envs where the learner holds P0 | 52.3% |
| envs where the learner holds P1 | 48.4% |
| player 0's share, either group | 52.3% / 51.6% |

Player 0 really does take about 52% of the orders — first move, and the
map is not symmetric in what a turn costs — and the two seat groups
mirror around a half and cancel, which is precisely the argument the old
log made for why 46% could not be right. Nothing is misaligned: `mine`
is read before the action is applied and lands where it should.

**Why the old number cannot be re-run.** `spR.pt` predates the powers
encoding break and will not load against this engine at all (19 globals
against 23), so the original cannot be reproduced except by reverting
the build. Two readings survive and this measurement cannot separate
them: a defect fixed incidentally between the eras, or the artifact the
old entry's own closing caution warns about — three calls that session
were made on small samples and all three were wrong. Either way there is
no anomaly in the code that runs today, and no reason to start there.

**What this does to the self-play problem.** It removes the last
suspected defect from the self-play path, which leaves
`2026-08-26-ppo-only-climbs-from-behind.md` holding the whole
explanation: the learner drifts because it is *level*, by construction,
every iteration — the worst case for this configuration, and now the
only case on the table. Combined with `2026-08-27-adv-floor-null.md`
retiring the normalisation story, the shape of the fix is no longer a
bug hunt but a scheduling question: arrange for the learner to be behind
the thing it plays. The frozen side is already a separate set of
weights; nothing but a flag stops it from starting *stronger* than the
learner instead of equal to it.
