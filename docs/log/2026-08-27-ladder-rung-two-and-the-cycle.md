# A second rung, powers everywhere, and the ladder is not transitive

> Four more generations. The new top beats the old top 82.5% and
> `ppo-adder3` 87.0% — but only 72.0% against a generation the old top
> beat 93.5%. Strength here is not a total order, and a ladder that keeps
> only its newest weights will not notice.

**Setup.** `--init sp-behind3-last --frozen-init sp-behind3-gen4`,
otherwise the recipe of `2026-08-27-the-ladder-turns-over.md`. Promotions
at iterations 110, 140, 160 and 180.

**Rated, 200 games each, `--versus`:**

| `sp-behind4-gen4` against | rated |
|---|---|
| `sp-behind3-gen4` — the rung below it | 82.5% ±2.7 |
| `ppo-adder3` — the chain's anchor | **87.0% ±2.4** |
| `sp-behind3-gen1` — five generations back | 72.0% ±3.2 |

The chain against `ppo-adder3` now reads 15.4% -> 53.8% -> 62.5% ->
87.0%, which is real progress by any reading.

**The cycle.** `sp-behind3-gen4` beat `sp-behind3-gen1` 93.5%. Its
successor, which beats *it* 82.5%, manages only 72.0% against that same
old generation — twenty-one points worse against an opponent five
generations stale, while being clearly stronger than the policy that
posted the better number. That is the standard self-play failure mode:
each generation specialises against what it currently plays and quietly
sells off answers to styles nobody has played lately. Nothing in the
current loop would detect it, because the frozen side only ever holds
the newest weights. The fix is a league — sample the opponent from all
past generations rather than the latest — and the gen files are already
written for exactly that.

**Powers went from used to central.** Activation ran 0.47/g at the start
and rose through 1.45, 2.57 and 3.54 to sit at 2.2-2.9/g. The climb out
of the run's low point (26.7% at iteration 80) to its first promotion
(61.5% at 110) coincides with that rise. Correlation inside one run, not
a controlled result — but this is a policy that six runs ago fired a
power twelve times in 216,000 legal opportunities, and it now presses
the button two or three times a game.
