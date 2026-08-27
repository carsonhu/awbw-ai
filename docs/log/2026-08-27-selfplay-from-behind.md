# Self-play works from behind, and the policy starts pressing the button

> The same run that collapsed to 0.0% with recalibration on climbs
> 20.8% -> 52.8% with it off, against a frozen opponent that beat its
> starting weights 84.6/15.4. And activations rise from 0.02/g to a
> sustained 0.4-0.7/g — the first thing that has ever taught a CO power.

**Setup.** `--selfplay --init ppo-adder1 --frozen-init ppo-adder3 --co
Adder --turn-discount --steps 256 --lam 0.99 --decide-cap --adv-floor
0.3 --refresh-at 0.6 --recalibrate 0`, 200 iterations. `--frozen-init`
is new: it puts different weights on the opponent seat, so the run opens
*behind* rather than at the parity every previous self-play run started
from. The paired run with recalibration on is
`2026-08-27-recalibration-ran-again.md`.

| iteration | 10 | 30 | 50 | 80 | 120 | 160 | 200 |
|---|---|---|---|---|---|---|---|
| recalibrate 1 | 39.2% | 4.5% | 0.0% | 0.0% | 0.0% | 0.0% | 0.0% |
| **recalibrate 0** | 20.8% | 10.7% | 21.7% | 41.5% | 34.9% | 26.0% | **52.8%** |
| entropy, off | 2.94 | 3.32 | 2.68 | 2.56 | 2.69 | 2.66 | 2.53 |

Still rising at the end and never promoted — the bar is 60% — so the run
is unfinished rather than done, and `sp-behind2-last.pt` is the artifact.

**The from-behind rule survives its first test against a policy.** Every
prior confirmation used a scripted opponent, which could always be
answered "greedy is exploitable, that is all this shows". Here the
opponent is a network that beat the learner's own starting weights
84.6/15.4, and the learner gained on it anyway. Self-play's drift was
never a defect in the self-play path (`2026-08-27-row-share-retired.md`
cleared the last one): it was parity, and opening from behind is enough
to move it.

**Powers, unprompted.** Imitation could not teach activation (213 labels
in 1.9M orders) and PPO against bots pruned it to 0.000 mass. Against a
policy, with recalibration off, it rises: 0.06/g at iteration 10, 1.38/g
at 60, settling 0.4-0.7/g. Both changes landed at once, so which one did
it is not established — but "the policy never presses the button" was
measured entirely under a setting now known to cost seventeen points,
and it no longer holds.

**Day-capped games appear.** `cut` reaches 2-6% late, having been 0%
against every bot. Two policies of similar strength stall where a bot
loses, which is what makes `--decide-cap` load-bearing from here.
