# PPO climbs the Adder rung without ever pressing the button

> The powers era's first run: 9.6% -> 63.2% against greedy in the Adder
> mirror, the from-behind rule delivering again — then the from-ahead decay,
> unguarded because `--adv-floor` sat at zero. And the winner never pops:
> it beats an always-popping opponent with 0.000 mass on activation.

**Setup.** `ppo.py --init checkpoints/bc-powers-scaled2.pt --co Adder
--opponent greedy --turn-discount --steps 256 --lam 0.99 --decide-cap`,
200 iterations. Greedy fires its biggest charged power on sight; the clone
starts at 9.6% ±1.5 and has never fired one.

**The climb, and the collapse.** Windowed scores: 4.8% at iteration 10,
30.1% at 40, 54.5% at 50, **75.5% at 70** — then 62%, 12.6% at 90, and
oscillation between 3% and 39% to the end. The kept checkpoint rates
**63.2% ±2.4** over 400 games (the 75.5% window flattered by twelve
points, the third time a kept window has worn a lucky number). The decay
signature is the familiar one: `spread` ran 0.5-0.86 through the climb and
0.15-0.19 in the crash windows — a well-predicted rollout leaving noise
for the normaliser to inflate — and `--adv-floor`, the guard built for
exactly this, was left at 0.0. So: `--turn-discount` carried the policy
from behind to well *ahead*, and from-ahead decay then arrived anyway.
The next run arms the floor (healthy spread here suggests ~0.3).

**Nobody taught it to pop, and it chose not to.** Activations per game
*fell* during the climb — 0.17/g at the start, 0.04/g at the peak — and
rose to 0.7/g only inside the collapse. Probed at the kept checkpoint:
powers legal on 216k decisions, fired 12, source-head mass 0.000. The
policy won 63% against an opponent that pops every bar it fills, while
never popping its own. Imitation could not teach the button (213 labels,
`2026-08-26-adder-first-clones.md`), and exploration did not merely fail
to find it — it pruned it while improving. Plausibly fair: a pop at a
random moment spends a full bar for one +10/+10 turn, and the advantage
estimate around it is dominated by the turn's other orders. Whether the
skill is worth its exploration cost is now a measured open question;
upweighting the 213 human activation labels in BC, or a small shaping
term on the bar being spent rather than wasted at cap, are the levers.

**What the rung is now.** 63.2% leaves greedy neither beaten decisively
nor a source of further from-behind signal for long; JakeMan under Adder,
and self-play with the floor armed, are the next opponents up.
