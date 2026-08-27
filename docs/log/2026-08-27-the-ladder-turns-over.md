# The self-play ladder turns over, four generations in one run

> Continuing the from-behind run promoted four times. Rated, not
> windowed: the learner went from losing to `ppo-adder3` 15.4% to beating
> it **62.5% ±3.4**, and the fourth generation beats the first **93.5%
> ±1.7**. Self-play produces something for the first time in this project.

**Setup.** `--init sp-behind2-last --frozen-init ppo-adder3`, otherwise
the recipe of `2026-08-27-selfplay-from-behind.md` — Adder mirror,
`--turn-discount --steps 256 --lam 0.99 --decide-cap --adv-floor 0.3
--refresh-at 0.6`, recalibration off, 200 iterations.

**Rated at 200 games each, `--versus`:**

| | vs `ppo-adder3` |
|---|---|
| `ppo-adder1` — where the chain started | 15.4% ±1.8 |
| `sp-behind2-last` — after 200 iterations | 53.8% ±3.5 |
| `sp-behind3-gen4` — after 200 more | **62.5% ±3.4** |

| | |
|---|---|
| `sp-behind3-gen4` vs `sp-behind3-gen1` | **93.5% ±1.7** |

The second table is the one that matters. A score against a moving
opponent tracks staleness, so the only honest question of a self-play
run is whether a late generation beats an early one, and the answer is
93.5/6.5 across four promotions inside a single run.

**Promotions came at iterations 70, 120, 150 and 190** — accelerating,
not stalling, and the run was still promoting when it hit the limit.
Between promotions the learner sits at parity with what it just became
and has to climb out again, which it did four times; the from-behind
opening appears to be needed only to start the process, not to sustain
it.

**The window told the truth this time.** `sp-behind2-last` windowed at
52.8% and rates 53.8% ±3.5. Windows have flattered by 9-12 points four
times against scripted opponents; against a policy of its own strength
this one did not. Worth watching rather than concluding from — one
observation — but the flattery may be a property of saturating a bot.

**Powers held.** Activation ran 0.9/g early and 0.1-0.45/g through the
promotions, against 0.02/g at the end of the JakeMan run. The button is
in the policy's repertoire now and survived four generations of
self-play selection.
