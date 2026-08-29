# Ten seeds on a rented card, and the anchor verdict survives

> The first seed-grouped experiment this project has run: anchor
> {0.01, 0.03} x four seeds plus two controls, trained concurrently on
> a rented 4090 in 2h47m for $2.71. At 0.03 the anchored group beats
> the control group with **non-overlapping ranges on all three
> discriminating panel members** — the claim one-seed runs could never
> make. At 0.01 the spread runs 57 points: the light weight is a
> lottery on the new net too, and retires.

**Setup.** `tools/grid.sh` on a Vast.ai RTX 4090 (32 vCPU): the v2
recipe from `bc-net2` (net-v2: GroupNorm, pooled bias, calibrated
critic; T4 corpus; pop-weighted labels), `--anchor bc-net2`, eight
lanes. Panels of 200 games per member, per arm.

| seed | anchor 0.01 (g/jm/clone/a3) | anchor 0.03 | control |
|---|---|---|---|
| s7 | 93.5/2.5/40.5/56.0 | 96.0/8.0/54.8/64.0 | 96.0/4.0/36.5/37.0 |
| s43 | 98.0/4.0/31.5/25.5 | 83.8/7.5/61.5/63.5 | 90.8/0.5/11.0/15.0 |
| s101 | 75.0/1.5/10.5/21.0 | 94.5/19.2/56.5/56.5 | |
| s202 | 93.0/11.0/67.2/65.8 | 95.8/6.0/51.0/56.5 | |
| clone range | 10.5-67.2 | **51.0-61.5** | 11.0-36.5 |
| adder3 range | 21.0-65.8 | **56.5-64.0** | 15.0-37.0 |
| JakeMan range | 1.5-11.0 | **6.0-19.2** | 0.5-4.0 |

**What is settled.** `--anchor-kl 0.03` is the recipe default: four
seeds sit above both controls on JakeMan, the clone and `ppo-adder3`
simultaneously, with group spreads of ten points where 0.01 spreads
fifty-seven. The net-v2 bundle raised the floor besides — the old
net's two 0.03 seeds scored 53.8/48.8 and 45.5/22.0 on clone/adder3;
the new group's *worst* is 51.0/56.5. Bundle credited as a bundle;
ablate only if something surprises later.

**Power timing closed, ten for ten.** Every arm — the unanchored
controls included — pops 3.3-9.7 per game at order 6-10 of a ~25-order
turn. The fix was never an RL intervention: pop-weighted BC labels put
the behaviour into `bc-net2`, and a policy that *starts* popping keeps
popping, because it pays. The anchor's residual contribution is
selectivity: anchored arms hold charge (`offered` 35-46%) where
controls slam the button the moment it lights (4-8%).

**Composition drifts under all weights, ordered by dose.** Anti-Air
group means 15.4% (0.03), 19.4% (0.01), 21.2% (control), against the
clone's 8.3 and the human 6.1; indirects sag to ~1%. The anchor
moderates the drift and does not prevent it — the dial exists if the
JakeMan rung says composition is what is missing.

**The greedy column would have called every one of these wrong.**
Controls scored 90.8-96.0 on greedy while losing to the clone group by
twenty-five points. Its retirement as an instrument stands.

`ppo-anc001` (the old lottery's best draw, 69.0/73.8 on the net
members) edges the new best singles on those columns — and loses the
duel that matters: `n2-a003-s7` beats it head-to-head **56.5% ±3.5**
over 200 games. The recipe's ordinary output now beats the old
process's best-ever draw. Next rung: JakeMan, from the 0.03 recipe, as
a seed group.
