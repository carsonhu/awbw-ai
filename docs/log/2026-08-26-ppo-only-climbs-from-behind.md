# PPO climbs from behind and decays from level, and the ladder has no rung left

> Four runs, one rule: a policy clearly worse than its opponent gains a lot, a
> policy level with it or ahead comes apart. Step size is not the cause, and
> `ppo-jake2` has nothing left to be behind.

**The pattern.** Every PPO run in this project, by how the start compared with
the opponent:

| start | rated vs opponent | opponent | outcome |
|---|---|---|---|
| `bc-scaled` | 7% | `greedy` | → 93%, held |
| `ctrl-greedy` | 46% | JakeMan (pre-fix) | → 67%, held |
| `spR` | ~50% | a frozen copy of itself | → 35-41%, decayed |
| `ppo-jake2` | 58% | JakeMan (fixed) | → 36%, decayed |

The two that gained were behind. The two that decayed were level or ahead. No
other variable separates them: same network, same environment, same defaults.

**Step size is not it.** The `ppo-jake2` run was repeated at `--lr 3e-5`
against 1e-4, which took `kl` from ~0.013 to ~0.003 and `clip` from 0.12 to
0.03 — a genuinely much smaller step. It decayed the same way:

| closed window (≥100 games) | lr 1e-4 | lr 3e-5 |
|---|---|---|
| first | 55.1% | 55.0% |
| second | 49.0% | 49.5% |
| third | 35.9% | 40.4% |

Four and a half points apart on the third window, against ±5 each. Nothing.
Recalibration was off in both (`log/2026-08-26-recalibration.md`), and shaping
and advantage normalisation were separately cleared in self-play
(`log/2026-08-26-selfplay-drift.md`).

**Why this closes the scripted ladder.** JakeMan is the top of it, and
`ppo-jake2` already rates 58.4% against JakeMan and 86.4% against `greedy`.
There is no scripted opponent left that the policy is *behind* — which, by the
rule above, means there is no scripted opponent left that it can learn from.
The ladder is exhausted, and not because anything was beaten decisively.

**What this does to the self-play problem.** Self-play at parity is the *worst*
case for this configuration by definition: the learner is exactly level with
its opponent, every time, by construction. The drift recorded in
`log/2026-08-26-selfplay-drift.md` may be this same effect rather than the bug
it was being hunted as — though the 46.1% row share is still unexplained and
still worth explaining.

**What is not established.** *Why* being level is fatal. The obvious mechanism —
a well-predicted rollout leaving only noise, which normalising rescales — was
measured and the spreads are only a factor of two apart, not the order of
magnitude that argument needs. Overfitting to an opponent already exploited is
the other candidate and is untested; it predicts the same decay here, so this
run cannot separate them.
