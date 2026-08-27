# Recalibration defaulted on, and ran in every powers-era run

> `2026-08-26-recalibration.md` measured seventeen points of damage from
> refitting batch-norm and closed with "run with `--recalibrate 0`". It
> was never made the default, so it ran in `ppo-adder1`, `ppo-adder2`,
> `ppo-adder3` and the first self-play attempt. The default is now 0.

**How it surfaced.** The first from-behind self-play run (`sp-behind`:
learner `ppo-adder1`, frozen `ppo-adder3`, which beats it 84.6/15.4)
did not climb — it collapsed and stayed collapsed:

| iteration | 10 | 20 | 30 | 40 | 50 → 200 |
|---|---|---|---|---|---|
| score vs frozen | 39.2% | 25.0% | 4.5% | 0.0% | **0.0% throughout** |
| policy entropy | 2.97 | 3.03 | 2.97 | 1.89 | 0.61 → **0.09** |

Zero over roughly seven hundred games across 160 iterations, on a policy
collapsing toward deterministic — and games stretched to about 1,860
orders against the 290 a `greedy` game takes, so it was surviving
longer while winning nothing. Checking the flags against the prior
finding rather than theorising about reward starvation found the cause
had already been measured a day earlier.

**Why it is invisible.** The damage does not arrive through the
gradient, so the update diagnostics stay nominal while it happens: `kl`
0.002-0.02 and `clip` 0.00-0.16 across the whole collapse, exactly as
in the JakeMan run this bug invalidated the first time.

**What is now suspect.** Every powers-era result carries the setting.
`ppo-adder3`'s climb is the least affected reading — it gained anyway,
so the finding is a lower bound — but the from-ahead decay in
`2026-08-26-ppo-adder-first-climb.md` and the null in
`2026-08-27-adv-floor-null.md` were both attributed to the update, and
17 points of per-iteration drift that no gradient diagnostic can see is
a live alternative explanation for both. Neither entry should be relied
on for *why* a run decayed until a recalibration-free control is run.
The head-to-head ratings are unaffected: they compare saved weights and
never call the trainer.

**The fix is the default.** `--recalibrate` now defaults to 0 with the
finding cited at the flag. A conclusion that lives only in a log entry
gets re-run by whoever does not re-read it.
