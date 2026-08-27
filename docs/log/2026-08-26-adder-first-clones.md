# The powers era opens at parity, once --amp is taken off its throat

> Phase 5 of Adder powers: BC retrained from scratch on the new encoding
> matches the old generation exactly — after finding that the --amp flag
> alone halves play strength at unchanged held-out accuracy. And the clone
> never presses the button: 213 activation labels in 1.9M orders is
> invisible to a cross-entropy loss.

**--amp is not free on this card, in quality as well as speed.** Its own
comment warned it buys nothing without fp16 tensor cores; measured, it is
worse than that. Same 96x8 recipe, same 15,000 steps, same data:

| run | orders/sec | held-out | vs greedy @0.3 | @1.0 |
|---|---|---|---|---|
| `bc-powers-scaled` (amp) | 196 | 0.468 | 7.8% ±1.3 | 1.8% ±0.9 |
| `bc-powers-scaled2` (fp32) | 1,164 | 0.469 | **19.8% ±2.0** | 7.5% ±1.9 |
| `bc-scaled` (old encoding, fp32) | | 0.464 | 19.0% ±2.0 | 5.5% ±1.6 |

Six times slower *and* half the play strength, with held-out accuracy
unable to tell the two apart — the classic accuracy-blind failure, shaped
like the batch-norm sensitivity already on record in `decisions.md`. The
64x6 `bc-powers.pt` (2.0-2.5% vs greedy) was amp-trained too and its
number is tainted the same way. Mechanism unproven; the measurement is the
decision: fp32 for BC on this machine.

**The encoding break cost nothing.** `bc-powers-scaled2` against the old
`bc-scaled` at matched temperature: 19.8 vs 19.0, identical within error.
Three extra source logits, four power globals, and served power turns
neither helped nor hurt vanilla play.

**The Adder rung is real: 9.6% ±1.5** (`--co Adder`, temp 0.3) against
19.8% vanilla for the same clone. About half the gap's story: greedy now
fires the biggest charged power on sight, and the clone never fires at
all — probed over 6,000 self-play steps x 50 envs, powers were legal on
~240k decisions and the source head's mass on them is 0.000. The cause is
arithmetic, not architecture: only Adder-mirror games serve activation
labels, and the training map holds 65 of them — **213 activations among
1.9M served orders**. The imitation shortcut for power timing does not
materialise at this corpus size; PPO exploration has to carry it, and the
first `--co Adder` run should instrument activation frequency as a
headline metric. If entropy never lifts it off zero, upweighting the 213
labels in the BC loss is the next lever.
