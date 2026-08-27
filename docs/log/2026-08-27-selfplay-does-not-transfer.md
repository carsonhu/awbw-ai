# The self-play gains do not transfer, and head-to-head hid it

> `sp-parity-gen3` beats `ppo-adder3` 90.3% head to head — and scores
> **22.8% against `greedy`, where `ppo-adder3` scores 65.1%**. The ladder
> has been producing counter-lineage specialists, not stronger play, and
> every measurement that would have caught it was inside the lineage.

**Rated, 200 games each, `--co Adder --decide-cap`:**

| checkpoint | vs `greedy` | vs JakeMan | vs `ppo-adder3` |
|---|---|---|---|
| `ppo-adder3` | **65.1% ±2.4** | **15.5% ±1.8** | — |
| `sp-parity-gen3` | 22.8% ±3.0 | 13.5% ±2.4 | 90.3% ±2.1 |
| `sp-behind4-gen4` | 15.0% ±2.5 | 0.5% ±0.5 | 87.0% ±2.4 |
| `bc-powers-scaled2` | 2.0% ±1.0 | 1.0% ±0.7 | — |

Two orderings of the same four policies disagree completely. By head to
head the self-play tops are far the strongest; by either scripted
opponent they are far the weakest thing that is not the clone.
`sp-behind4-gen4` wins one game in two hundred against a bot its own
ancestor beat 15.5% of the time.

**This is the cycle finding, larger.** `2026-08-27-ladder-rung-two-and-
the-cycle.md` caught a newer top scoring 21 points worse against a
five-generation-old opponent, and read it as staleness inside the
ladder. It is not confined to the ladder: the same specialisation costs
the policy 42 points against `greedy` and everything it had against
JakeMan. `sp-parity-gen3` also beats the imitation clone only 68.3%
±6.0 — a policy that scores 2% against `greedy` takes a third of the
games off the checkpoint that beats `ppo-adder3` nine times in ten.

**What it invalidates.** Nothing measured wrongly, but
`2026-08-27-the-ladder-turns-over.md` and its successor read
"generation four beats generation one 93.5%" as progress. It is
progress *at beating generation one*. A self-play run's own promotion
rule cannot see this, because the frozen side is always its own recent
past. The strongest all-round checkpoint on the board is still
`ppo-adder3`, which loses 90/10 to the thing that replaced it.

**What to do.** Opponent diversity, not a single moving mirror: sample
the frozen side from past generations *and* the bots *and* the clone,
which the saved gen files already allow. And rate every checkpoint
against a fixed external panel — `greedy`, JakeMan, the clone — never
only against what it trained on. Head-to-head remains the right
instrument for two policies; it is the wrong one for progress.

**Unresolved.** `bc-powers-scaled2` rates 2.0% against `greedy` here
against 9.6% ±1.5 in `2026-08-26-adder-first-clones.md`. Same CO and
temperature; this run adds `--decide-cap`. Worth pinning before the
clone is used as a panel opponent.
