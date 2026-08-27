# The decay was the bug, and the climb that replaced it is specialisation

> Recalibration off, and the run that used to peak at 75% and collapse now
> holds 73-93% for a hundred and forty iterations. The rule this project
> ran on — climbs from behind, decays from level — is an artifact of a
> batch-norm bug. And the held score is worth almost nothing: the same
> policy loses to the imitation clone 84/16.

**The control.** `bc-powers-scaled2` against `greedy`, every flag as
`ppo-adder1` had it (`2026-08-26-ppo-adder-first-climb.md`), recalibration
now off by default:

| iteration | 40 | 60 | 90 | 120 | 150 | 180 | 200 |
|---|---|---|---|---|---|---|---|
| `ppo-adder1`, recalibrate on | 30.1% | 69.4% | 12.6% | 32.4% | 3.3% | 19.8% | 17.5% |
| **control, off** | 33.1% | 78.6% | 84.5% | 81.6% | 74.5% | 90.5% | **86.6%** |

No decay phase at all. Every run the from-ahead rule was drawn from had
the bug active, so the rule goes with it — as does the reasoning built on
top: keep-best as a necessity, `--adv-floor`, and "a fixed opponent is a
finite resource" as a claim about advantage rather than about coverage.

**Powers, too.** Activation ran 0.90-1.14/g by the end against `greedy`,
where `ppo-adder1` finished on 0.000 mass and twelve firings in 216,000
chances. "PPO prunes the button while improving" was the same artifact.

**What the score was hiding.** First use of `panel.py`, 200 games each:

| | vs `greedy` | vs JakeMan | vs the clone | vs `ppo-adder3` |
|---|---|---|---|---|
| `ctrl-recal-off` | **91.5%** | **1.0%** | 15.5% | 5.8% |
| `ppo-adder3` | 62.5% | 15.5% | 39.0% | — |
| `sp-parity-gen3` | 22.8% | 13.5% | 68.3% | 90.3% |

A policy that beats `greedy` nine times in ten wins one game in a hundred
against JakeMan and loses to the *imitation clone* — which itself scores
2% against `greedy` — five games in six. Training against one opponent
does not make a policy stronger; it makes it a better answer to that
opponent, and the score against it climbs the whole way, which is exactly
what makes it a bad instrument.

**No row dominates.** `ctrl-recal-off` owns `greedy`, `ppo-adder3` owns
JakeMan, `sp-parity-gen3` owns the head-to-heads, the clone beats two
policies that crush the bot it cannot touch. This is a cycle, not a
ladder, and the earlier call that `ppo-adder3` is "strongest all-round"
was made on two panel members out of four.

**What this changes.** The response is the league and the panel, both now
in. One gap: a league draws from checkpoints, and the env takes its
scripted opponent at construction, so JakeMan — the member everything is
worst against — cannot be in the pool without per-env opponents in
`VecEnv`. That is the next real piece of work.
