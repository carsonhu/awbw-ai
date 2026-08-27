# Parity was never the problem — recalibration was

> The control that should have drifted promoted three times and produced
> the strongest policy of the night: 90.3% against `ppo-adder3`, and
> 84.0% against the from-behind ladder that took twice the compute to
> get to 87.0%. `--frozen-init` bought nothing. Correcting today's claim.

**Setup.** `--selfplay --init ppo-adder3` with **no** `--frozen-init`, so
the run opens at exact parity — the configuration
`2026-08-26-selfplay-drift.md` recorded drifting to 35-41% and never
promoting. Otherwise identical to the from-behind runs, recalibration
off. Promotions at iterations 40, 80 and 190.

| rated, 200 games, `--versus` | |
|---|---|
| `sp-parity-gen3` vs `ppo-adder3` (its own start) | **90.3% ±2.1** |
| `sp-parity-gen3` vs `sp-behind4-gen4` | **84.0% ±2.6** |
| for comparison: `sp-behind4-gen4` vs `ppo-adder3` | 87.0% ±2.4 |

One 200-iteration run at parity beat a 400-iteration ladder that opened
from behind, on the same anchor, and beats that ladder's top head to
head.

**What this corrects.** `2026-08-27-selfplay-from-behind.md` changed two
things at once — the from-behind opening and recalibration — and credited
the opening. This run holds recalibration off and removes the opening,
and self-play still works. The honest reading of both entries together:
**recalibration was the entire blocker on self-play**, and it had already
been measured and written up a day earlier
(`2026-08-26-recalibration.md`) without being made the default. The
from-behind rule is not refuted — every run it was drawn from still
decayed — but it was never what stood between this project and a working
self-play loop, and `--frozen-init` is now a convenience, not a fix.

**Parity is not free, only survivable.** The run dipped to 11.6-17.3%
around iterations 110-130 before recovering to 72.1% and promoting
again. So "level decays" is visible in the trace; with the batch-norm
damage gone it is an episode the run climbs out of rather than a floor
it settles on.

**Powers again, in a second lineage.** Activation went 0.02/g -> 0.5-1.1/g
here too, on a run with no from-behind opening. Two independent lineages
learned the button once recalibration stopped running, which is now the
better explanation of `ppo-adder1`'s 0.000 mass than anything about
exploration cost.

**The strongest checkpoint is `sp-parity-gen3.pt`.** The cycle warning in
`2026-08-27-ladder-rung-two-and-the-cycle.md` applies to it too: it is
the best against what it has played, which is not the same as best.
