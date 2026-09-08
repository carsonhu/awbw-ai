# The league never promoted, and the seat share cost it JakeMan

> Four self-play arms against two plain JakeMan rungs from `mixjm-s43`.
> `--refresh-at 0.7` is unreachable against five frozen siblings plus a
> quarter-share of JakeMan: one promotion in 800 arm-iterations, so the arms
> trained against a **static** pool and -- every self-play `--out` write hanging
> off a promotion -- three have only `-last.pt`. They give back ~20 points a CO
> from their own iteration-30 weights. Item 3 stands open.

**Setup.** `GRID=league` on a rented 48GB 4090, 16-core share, six arms at
once, 1h35m, about $0.60. Recipe of record from `mixjm-s43`, anchored to
`bc-net2-t4` at 0.03, all five COs, 200 iterations. League arms: `--selfplay
--frozen-init` (clone, `mixjm-s7/s101/s202`, `mixjm-noanc-s43`) `--pool-bots
jakeman --pool-bot-share 0.25 --seat-cap 50`. Controls: `--opponent jakeman`.
Panels as Adder, 200 games a member, score/decisive.

| arm | greedy | JakeMan | clone | `ppo-adder3` |
|---|---|---|---|---|
| lg-s7 | 71.5/64.0 | 26.5/21.5 | 78.8/61.5 | 87.5/80.0 |
| lg-s43 | 59.0/55.5 | 23.0/22.0 | 76.5/69.0 | 82.0/79.5 |
| lg-s101 | 71.5/60.0 | 24.8/24.0 | 77.5/53.0 | 80.0/71.5 |
| lg-s202 | 63.2/61.0 | 15.0/13.0 | 85.8/77.5 | 73.0/69.0 |
| **lgctl-s7** | **80.5/74.0** | **55.0/51.5** | 85.2/68.0 | **88.0/82.0** |
| lgctl-s43 | 48.5/47.0 | 35.5/35.0 | 71.5/64.5 | 64.0/62.5 |

**The bar was never met.** `ppo.py` promotes on `score >= args.refresh_at` over
30+ games, default **0.7**, and in self-play that line is also the only thing
that writes `--out`. `lg-s43` cleared it once, at iteration 30 on 74.1%, and
never saw 70 again in 170 more: 52.3, 49.5, 46.0, 43.2, 27.4, 43.2, 55.9, 26.9.
The other three never cleared it at all. Five near-siblings plus a quarter-share
of JakeMan do not get beaten 70% of the time, and 0.7 is a default from
`league3`, whose pool was checkpoints only and whose learner beat its own past.

**The arms decay from their own start.** `lg-s43`'s one promoted checkpoint,
iteration 30, rates 44.2/58.0/46.5/62.5/40.8 per CO against JakeMan. The same
arm at iteration 200 reads 23.0/24.2/24.0/33.0/23.0 -- about 20 points a CO
given back over 170 iterations against a pool that never moved, within one arm,
so no training-opponent confound. Across the groups JakeMan reads 22.3 against
45.3, which carries that plus the dose: controls spend every game on the test
opponent, league arms a quarter.

**Level wherever the instrument is neutral.** Group means, score: `greedy` 66.3
against 64.5, clone 79.7 against 78.4, `ppo-adder3` 80.6 against 76.0 -- no arm
trained on any of the three. The league group is also *tighter* on all four
columns, 12.5 points of spread on `greedy` against the controls' 32.0.

**Nothing beat the parent.** `lgctl-s43.pt`, kept at iteration 30, is
`mixjm-s43` five updates past the warm-up: a near-free re-rating of the parent
at 83.5/80.0 on `greedy` and 50.7/58.0/59.5/69.5/**46.5** per CO. The best arm,
`lgctl-s7-last`, reads 55.0/52.2/63.5/67.8/41.0 -- better as Adder and Koal,
worse as Jake and Grimm. Two hours bought no rung.

**Peak selection failed both ways.** `lgctl-s7` kept its peak at iteration 110
and rates 56.0 there against 80.5 at its endpoint; `lgctl-s43`'s peak *is* its
parent. A 30-game rollout window is not the selector
`2026-08-30-the-peak-holds-and-one-arm-stops-winning.md` measured.

**What it does not settle.** Whether a league helps: this one never laddered.
The next attempt must price the bar against the pool it deals, and needs more
than two controls -- the control group spans 32 points, the group gap under 5.
