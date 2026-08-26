# JakeMan beaten, by running yesterday's experiment without the bug

> The same run that scored 7.6% scores 67.2% with recalibration off. A sixty
> point swing from one default, and the first policy here to beat the strongest
> scripted opponent.

**Setup.** `ppo.py --init checkpoints/ctrl-greedy.pt --opponent jakeman
--recalibrate 0`, 200 iterations of 32 envs x 64 steps, shaping 0.1, seed 11.
Started from `ctrl-greedy` — the checkpoint the recalibration control produced,
and the strongest thing available at 46.0% against JakeMan.

**Result**, 400 games each at temperature 1.0:

| checkpoint | vs JakeMan | vs greedy |
|---|---|---|
| `ppo-jake` (yesterday, recalibration on) | 7.6% ±1.3 | — |
| `ctrl-greedy` (this run's start) | 46.0% ±2.5 | 83.1% ±1.9 |
| `ppo-jake2-last` (iteration 200) | 52.1% ±2.5 | — |
| **`ppo-jake2`** (kept) | **67.2% ±2.3** | **86.4% ±1.7** |

Better against both opponents, so this is not an anti-JakeMan trick bought by
forgetting how to play `greedy`. It is also the first checkpoint to beat
JakeMan at all, and the first to hold a winning record from *both* seats of an
asymmetric map.

**The run behaved, where yesterday's did not.** 620 games during training at
55.8%, against 745 at 39.1%. Windows climbed 25% -> 78.6% and were still
above the start at iteration 200; yesterday's slid from 44% to 7.6% with `kl`
and `clip` reading normal the whole way, because the damage was arriving
through the batch-norm statistics rather than the gradient
(`log/2026-08-26-recalibration.md`). Nothing else changed.

**The decay is still there, and the guard still matters.** The kept weights
rate 67.2% and the final ones 52.1%, so fifteen points were given back over
the last thirty iterations. `--recalibrate 0` does not cure the
fixed-opponent decay described in `log/2026-08-25-ppo-first-run.md`; it
removes a second, larger loss that was sitting on top of it.

**Where the ladder stands.** `ppo-jake2` beats JakeMan 67%, and JakeMan beats
`greedy`, `capturer` and `random` 20-0 each. The scripted ladder is now
exhausted from the top, which is the same wall the first PPO run hit at the
bottom — and the reason self-play still matters, once
`log/2026-08-26-selfplay-drift.md` is resolved.
