# The curriculum rung holds, and `greedy` stops being able to see it

> `ppo-adder3` climbed 2.5% -> 15.5% against JakeMan-under-Adder over 200
> iterations without ever decaying — behind the whole way, which is the
> point. It beats its own parent 84.6%. Against `greedy` the two are
> indistinguishable, which is a fact about `greedy`.

**Setup.** `--init ppo-adder1 --opponent jakeman --co Adder --turn-discount
--steps 256 --lam 0.99 --decide-cap --adv-floor 0.3`, 200 iterations, after
`greedy` saturated at 63.2% (`2026-08-26-ppo-adder-first-climb.md`).

| rated, 400 games each | ppo-adder1 | ppo-adder3 |
|---|---|---|
| vs `greedy` (Adder mirror) | 63.2% ±2.4 | 65.1% ±2.4 |
| vs JakeMan (Adder mirror) | — | 15.5% ±1.8 |
| head to head | — | **84.6% ±1.8** |

**The measuring stick is spent, not just the training signal.** Two
points of separation against `greedy`, inside the error bars, between
two checkpoints that are 84.6/15.4 against each other. A saturated
opponent compresses everything above it into noise, so from here a
number against `greedy` cannot rank two policies and should not be
quoted as if it could. `--versus` is the instrument now.

**The rule holds a fifth time.** Windows rose 2.5 -> 8.3 -> 11.7 ->
15.7 -> 22.1 -> 23.2%, then oscillated 3.6-21.7% with no collapse. The
learner never reached parity, so there was no from-ahead phase to decay
from; every run that decayed had got level first.

**JakeMan under Adder is a much harder opponent than JakeMan was.** The
pre-powers ladder ended with `ppo-jake2` at 58.4% against it
(`2026-08-26-ppo-only-climbs-from-behind.md`); this policy starts at
2.5% and ends at 15.5%. The bot pops the biggest bar it holds on sight,
and +2 movement with +10/+10 is worth more to a scripted attacker than
declining the button costs the policy. That makes the same bot a usable
rung again where it had been exhausted.

**The window flattered by eight points, again.** Kept on 23.2%, rates
15.5%. Fourth occurrence.

**Powers stayed unpressed.** Activations fell 0.38/g -> 0.02/g across
the run — pruned while improving, now against an opponent whose entire
edge is that it pops.
