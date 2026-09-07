# The rung clears the record, and Grimm gates it

> The JakeMan rung from `mix-s43`: four anchored seeds against two unleashed.
> Three anchored arms clear the 49.3 record -- best 59.5/59.0, from a parent
> that rated 16.5 -- and the leash is vindicated, but only on decisive share.
> On score the unleashed arms *lead*, 53.1 to 51.2; on decisive they lose,
> 43.1 to 49.6, capping a fifth of their games. Rated per CO, `s43` and `s202`
> win as Adder, Jake, Koal and Jess and fail only as Grimm. New parent: `s43`.

**Setup.** `GRID=mixjm` on a rented 48GB 4090, six arms at once, 1h40m, about
$1. Recipe of record from `mix-s43`, dealt the Tier-4 pool, `--opponent
jakeman`, 200 iterations, leashed to `bc-net2-t4` at `--anchor-kl 0.03`. Two
`noanc` arms price that leash, v1 having reached 63.4 with none. Panels 200
games a member as Adder, and each arm rated as each CO against JakeMan, 200
games -- the mixed rung left `greedy` saturated for all five. Score/decisive.

| arm | greedy | JakeMan | `bc-powers-scaled2` | `ppo-adder3` |
|---|---|---|---|---|
| **`mixjm-s43`** | 79.0/72.5 | 59.2/55.5 | 86.0/82.0 | 97.0/92.5 |
| `mixjm-s202` | 73.0/71.0 | **59.5/59.0** | 89.5/84.5 | 95.0/92.5 |
| `mixjm-s7` | **90.0/89.0** | 55.5/55.5 | 86.5/83.0 | 91.2/89.0 |
| `mixjm-s101` | 57.2/55.2 | 39.5/39.5 | 57.0/52.0 | 66.0/64.5 |
| `mixjm-noanc-s43` | 85.5/63.0 | 51.7/36.5 | 80.5/56.0 | 98.5/78.5 |
| `mixjm-noanc-s7` | 67.2/44.5 | 36.5/28.0 | 77.5/51.5 | 65.5/42.5 |

**Score would have called it for the unleashed arms.** They cap 12.5-26.5% of
their games against every panel member against the anchored arms' 1.5-9.5%:
`noanc-s43` reads 98.5 on `ppo-adder3` and 78.5 decisive, twenty points of
score it never finished for. The bar was 49.3, the best single arm ever rated
against JakeMan, 49.5/45.0 for the best honest one (`jm2-s43`). Decisive
share as each CO, against JakeMan, 200 games each:

| arm | Adder | Jake | Koal | Jess | Grimm | mean |
|---|---|---|---|---|---|---|
| `mixjm-s43` | 55.5 | 54.5 | 56.0 | **70.0** | 39.0 | **55.0** |
| `mixjm-s202` | 59.0 | 61.0 | 56.0 | 60.0 | 34.5 | 54.1 |
| `mixjm-s7` | 55.5 | 48.5 | 54.5 | 61.0 | 36.5 | 51.2 |
| `mixjm-noanc-s43` | 36.5 | 52.0 | 41.5 | 56.0 | **54.2** | 48.0 |
| `mixjm-s101` | 39.0 | 41.0 | 43.5 | 45.0 | 22.5 | 38.2 |
| `mixjm-noanc-s7` | 28.0 | 45.0 | 33.0 | 44.8 | 39.5 | 38.1 |

**Grimm is the whole remaining gap to the Lane B endpoint.** No arm wins
across all five COs, and Grimm blocks every one: -16.5, -19.0, -24.5 and
-16.5 against each anchored arm's own Adder number, a near-flat offset over
four seeds. It is not combat efficiency -- as Grimm every arm *nets more* per
attack (`s43`: +1,937 against +1,293) and loses a smaller share (17.5%
against 19.7%). It builds less and plays shorter turns instead (63.7 units a
game against 71.1, 12.4 orders a turn against 15.3), which is what losing the
attrition war looks like, not what misplaying a CO looks like.

**The unleashed arms invert it, and that is the stall.** `noanc-s43` rates
54.2 as Grimm against 36.5 as Adder, the grid's best Grimm number by 15
points -- and caps 6.5% there against 16.5-19.5% as the other four. Grimm is
where its pathology stops paying, not where it plays well. `noanc-s7`'s cap
share is flat across COs, so that reading does not carry.

**What it does not settle.** Two unleashed arms against four is thin. Grimm's
cause is unmeasured and the corpus can control for it: human Grimm win rates
at matched Elo would price how much of the hole is the CO. `s101` says the
seed spread has not closed -- 38.2 against `s43`'s 55.0 on no flag at all.
