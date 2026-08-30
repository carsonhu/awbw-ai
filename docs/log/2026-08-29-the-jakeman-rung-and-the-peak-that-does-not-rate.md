# The JakeMan rung as a seed group, and a peak that does not rate

> Eight arms, two candidate parents x four seeds, 2h12m on a rented
> 4090. The `a003-s7` parent leads on all four panel members by group
> mean but separates on none — a lead, not a verdict — and the rung
> only matches v1's bar: mean 40.0 against JakeMan where `ppo-threat2`
> rated 37.5, no arm beating it. The finding with teeth is elsewhere:
> the checkpoint each arm *saves* is its best rollout, and best
> rollout is uncorrelated with the panel here (r=0.07), twelve points
> high.

**Setup.** `GRID=jakeman CONCURRENT=8 bash tools/grid.sh` on a Vast.ai
4090: the settled recipe (`--threat-planes --opponent jakeman --co
Adder --turn-discount --steps 256 --lam 0.99 --decide-cap
--iterations 200`, `--anchor bc-net2 --anchor-kl 0.03`) from each of
the two `a003` parents the greedy grid left tied. Eight lanes cost
throughput — 285 orders/s early, 238 by the end — and no arm failed.

| seed | from `a003-s7` (g/jm/clone/a3) | from `a003-s101` |
|---|---|---|
| s7 | 84.0/**49.3**/76.6/94.5 | 84.2/29.5/73.0/78.0 |
| s43 | 74.0/37.0/71.0/79.5 | 95.0/39.0/91.0/80.8 |
| s101 | 76.8/33.0/81.0/84.5 | 56.2/19.8/72.5/76.1 |
| s202 | 89.3/40.5/90.5/90.8 | 62.0/38.0/73.0/74.5 |
| mean | 81.0/**40.0**/79.8/**87.3** | 74.3/31.6/77.4/77.3 |
| a3 range | **79.5-94.5** | 74.5-80.8 |

**The parent question stays open.** `a003-s7` wins every column by
group mean and is tighter in each, but the ranges overlap on greedy,
JakeMan and the clone. Only `ppo-adder3` approaches the non-overlap
standard the anchor grid met, and one arm per side crosses it by 1.3
points. Four seeds separated anchor weights; they do not separate
these parents, so the greedy rung's tie survives.

**The rung matched v1 rather than beating it.** Mean 40.0 sits inside
`ppo-threat2`'s 37.5 ±3.4, best arm 49.3, every arm still losing. The
other three columns clear the v1 rung well (79.8 clone and 87.3
`ppo-adder3` against 84.0 and 82.0), so v2 bought breadth here, not
the JakeMan number it was aimed at.

**The saved checkpoint is selected on noise.** `ppo.py` saves the best
*rollout* score, and across these arms that number means nothing: mean
peak 48.2 against mean panel 35.8, gaps of 0.0 to 32.9, r=0.07. The
second-highest peak (`s101par-s101`, 52.7) panelled worst of the eight
at 19.8. The statistic behaved on the anchor grid — against `greedy`,
peak 95.4 against panel 91.6, r=0.54, most gaps under three. The
difference is the opponent's variance: greedy rollouts sit above 90%
every iteration, JakeMan rollouts swing 26-59% inside one run, so the
series maximum is mostly sampling. Whether `-last.pt` rates better is
untested; both are kept for all eight arms.

**Composition and power came through unchanged.** Anti-Air group means
14.6% and 15.5% against the clone's 8.3, indirects 0.6-1.8% of spend,
pops 4.7-8.9 per game at order 7.5-11.5 of 19-28. A harder opponent
moved none of it: the drift is the reward's blindness to composition.

**Next.** Panel `-last.pt` against the peak on two arms before any
further rung. Carry `jm-s7par-s7` (84.0/49.3/76.6/94.5) as the
JakeMan-facing parent, `jm-s7par-s202` (89.3/40.5/90.5/90.8) as the
all-rounder.
