# PPO improves the tactics and wrecks the strategy

> The clone builds what humans build — Anti-Air 5.3% against the
> corpus's 6.1%, Tank 24.3% against 23.4%, indirects 3.9% against 4.0%
> — and pops at order 1.6 of a 13.3-order turn. Two hundred iterations
> of PPO against `greedy` take Anti-Air to **35.0%** and the pop to
> order 25 of a 20-order turn, while *improving* every engagement
> number. The reward can see material and cannot see composition.

**Setup.** `play_diag.py`, new: it plays a checkpoint, reads the
recorded games back, and reports what it built, what each attack
traded, and where its powers landed. The human column is every
prepared game on the board (2,446 on A River Supreme, all COs).

| build share | human | `bc-threat2` (clone) | `ppo-t2v1` (after PPO) |
|---|---|---|---|
| Infantry | 50.4% | 40.6% | 31.6% |
| Tank | 23.4% | 24.3% | 12.0% |
| B-Copter | 9.7% | 11.4% | 7.7% |
| **Anti-Air** | **6.1%** | **5.3%** | **35.0%** |
| Artillery | 3.5% | 3.2% | 1.6% |
| indirects | 4.0% | 3.9% | 1.6% |
| pop position | — | order 1.6 of 13.3 | order 25.0 of 20.4 |
| losing attacks | — | 17.9% | 10.3% |
| net per attack | — | +1,345 | +2,407 |

**The split is clean and it has one cause.** The shaping potential
prices a unit at `cost * hp100 / 100`, so eight thousand funds of
Anti-Air and eight thousand of Tank are *identical* to it. Material
exchange is exactly what it can see, and that is exactly what improved:
losing attacks 17.9% -> 10.3%, net per attack +1,345 -> +2,407.
Composition reaches the policy only through the terminal result,
discounted across a ~700-order game, so PPO is free to drift anywhere
that preserves material — and drifts toward whatever beats the single
opponent it trains against. 35% Anti-Air is a best response to
`greedy`, which builds copters. It is not a build order.

**Imitation is not the weak link here.** The clone reproduces the human
mix to within a point on the units that matter and carries human power
timing with it. Everything that went wrong went wrong downstream.

**What it changes.** "Beat JakeMan" and "beat greedy" cannot distinguish
a policy that plays well from one that has memorised an opponent, and
the panel is four members of which two are the agent's own ancestors.
The corpus build mix is the first human-referenced instrument this
project has, and it costs one 30-game run.

**Next.** A KL anchor to the clone (`--anchor`, `--anchor-kl`) is the
direct treatment: the clone already holds the composition and the
timing the reward cannot score. Measured drift between `ppo-t2v1` and
the clone is 1.03 nats, and 0.000 between a network and itself, so the
weight sweep has a scale to work against. The tension to watch is that
the clone's *tactics* are worse, so too strong a pull buys composition
by giving back the engagement gains.
