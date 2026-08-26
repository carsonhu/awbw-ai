# Recalibration was eating the policy

> Refitting batch-norm statistics costs seventeen points of play strength with
> no gradient step taken at all. It is on by default, so it ran in every job
> since it was added — including the JakeMan run whose result it invalidates.

**How it surfaced.** Self-play from `spR` had the learner losing to its own
frozen copy, around 35% against a 70% promotion bar it could never reach. Two
copies of one policy should sit at half — the Rust `results` docstring says so,
and offers it as a check that seats really alternate.

**The harness was not at fault.** `spR` against itself, 200 games: 52.8% ±3.5.

**A wrong turn, recorded.** The first suspect was recalibration, on a 37%
window during the critic warm-up — where the trunk cannot move, so
recalibration is the only thing that differs from the frozen copy. Then a
probe that recalibrated 25 times with no updates scored 45.5% over 33 games,
and I called it benign. Both readings were noise: 27 games carry ±9.6 points,
so 37% sits 1.3σ from parity and says nothing. The project already knows 200
games is the floor for a rating; a rollout window is not one, and neither is a
probe's running total.

**Rated properly, it is not benign.** The same probe checkpoint against the
weights it started from, 200 games:

| weights | vs `spR` |
|---|---|
| `spR` itself | 52.8% ±3.5 |
| `spR` + 25 recalibrations, **no gradient steps** | **33.5% ±3.3** |

Mean relative drift in those statistics was 5.9%, at worst 22.8% — small
numbers that are evidently not small.

**What it invalidates.** `--recalibrate 1` is the default, so it ran in the
JakeMan run, which decayed 44% → 7.6% while `kl` and `clip` stayed nominal
(`log/2026-08-26-jakeman-ppo.md`). Damage that does not arrive through the
gradient is invisible to the gradient's own diagnostics. That entry measured
this bug, not the matchup, and its conclusion should not be relied on.

**PPO itself is fine.** The control — `--opponent greedy --init bc-scaled
--recalibrate 0`, the configuration that worked before the opening fix — went
7% → 93% by iteration 90 and began the familiar saturation decay after. So
neither the opening fix nor the update machinery is at fault.

**What is not established.** Why refitting hurts. The docstring's argument is
real — inherited statistics measured 1.75 standard deviations off — so the
answer is likely a corrected recalibration rather than none: 256 states is a
thin sample to estimate from, and the running average compounds it every
iteration. Until that is understood, run with `--recalibrate 0`.
