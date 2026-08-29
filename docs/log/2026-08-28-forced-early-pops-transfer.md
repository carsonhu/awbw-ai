# Forcing the pop early pays, and masking it late does nothing

> `--pop-window` — activation made illegal after order 3 — took the pop
> rate to **0.00/game**: the policy holds no mass on activation in a
> turn's opening, so removing it late deletes popping rather than
> relocating it. Entering the state outright instead, `ppo-t2pop` rates
> **90.2 ±2.1** against `greedy` where its control managed 85.5, and
> **40.5 ±3.5** against the clone where the control managed 29.0 —
> rated *unforced*, so the buff-state experience transferred.

**Setup.** `ppo-t2pop`: the v2 greedy rung with `--pop-force 0` and
nothing else changed, init `bc-threat2`, 200 iterations. The control is
free: `ppo-t2v1` is that exact run without the flag.

| 200 games each | `ppo-t2v1` (control) | `ppo-t2pop` (forced) |
|---|---|---|
| vs `greedy` | 85.5 ±2.5 | **90.2 ±2.1** |
| vs JakeMan | 5.0 ±1.5 | 4.8 ±1.5 |
| vs the clone | 29.0 ±3.2 | **40.5 ±3.5** |
| vs `ppo-adder3` | 33.5 ±3.3 | 25.5 ±3.1 |
| kept window | 90.6 | 96.5 |

**The circularity is real and it is one-way.** Popping early only pays
if the orders after it exploit the buff; using the buff is only
learnable if it pops early. The agent sits exactly where neither
gradient exists — and the natural lever, masking activation out late,
cannot move it, because there is nothing early to preserve. Only
entering the state outright works. Both directions were measured:
window 3 gives 0.00 pops/game, force 0 gives 7.7 rising to 9.6.

**What the probes now report every run.** Pop *position* joins pop
rate (`pop N/g @index/turnlen`), and `offered` says how often
activation was legal at all — 77.8% for the control, which settles that
the end-of-turn habit was never a charge problem. The button was there
three orders in four and the policy waited until the turn was spent.

**Two caveats, both built in deliberately.** Forced popping spends
charge the moment it reaches COP, so SCOP rarely appears and this
measures early *COP*; and on a forced row every other source is -inf,
so "pop or do something else" is a comparison the softmax never sees.
The arm prices what correctly-timed powers are worth, not how to time
them.

**Not settled: whether an anchor gets this for free.** The clone pops
at order 1.6 of a 13.3-order turn — human timing, straight out of the
demonstrations — and PPO is what destroys it
(`log/2026-08-28-ppo-improves-tactics-and-wrecks-strategy.md`). A KL
anchor to the clone may recover the timing without forcing anything,
which would make `--pop-force` a diagnostic rather than a recipe.
