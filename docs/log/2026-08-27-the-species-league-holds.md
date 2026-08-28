# The species league holds: every number up, nothing eaten

> The same 200 iterations that consumed league2 produce the best
> checkpoint the project has: 24.5% against JakeMan, 95.0% against
> `ppo-adder3`, the exploiter flipped from 21.5% to 54.0% — and the
> `greedy` number held instead of collapsing.

**Setup.** `league3`: init `league2-gen3`, and a pool that is finally
not one family — the clone, `exploit1` (the net-killer), `ppo-adder3`
(the strongest bot-line player), `sp-parity-gen3`. PFSP seating with
both caps live: `--pool-self-cap 0.4` (promotions replace the oldest of
the learner's own) and the new `--seat-cap 25` (`8e4f0e8`), which stops
any member — the exploiter included — from monopolising the run. Six
promotions in 200 iterations.

| panel, 200 games each | `ppo-adder3` | `league2-gen3` | `league3` |
|---|---|---|---|
| vs `greedy` | 62.5% | 42.8% | 42.0 ±3.5 |
| vs JakeMan | 15.5% | 19.0% | **24.5 ±3.0** |
| vs the clone | 39.0% | 58.0% | **89.0 ±2.2** |
| vs `ppo-adder3` | — | 91.5% | **95.0 ±1.5** |
| vs `exploit1` | — | 21.5% | **54.0 ±3.5** |

**Why this worked where league2 failed.** League2's members that beat
the learner were always its own newest generations, so loss-rate
seating fed it its own past. Here the wall was `exploit1` — a different
family — and the seat cap forced rotation once any member had taken its
share. The report lines show the difference: league2 ended with its
own line at 28–47% and everything else solved; league3 ended with all
six members in a 48–61% band, hard the whole way, none solved and none
dominant. The learner also fired CO powers at 1–2 per game by the end —
the highest sustained rate of any run, learned against a pool where
popping actually decides games.

**What did not move.** `greedy` sits at 42%, twenty points under
`ppo-adder3`. The bot's line is in the pool only through `ppo-adder3`'s
imitation of it; if that number matters, the pool needs the bot itself,
which means engine support for scripted members. `docs/decisions.md`
territory, not a lever this run failed to pull.

**The recipe that stands.** Species beat sampling twice over: the
exploiter cracked what the league could not (one run, from the clone),
and the league then absorbed the exploiter's lesson at parity while
keeping everything else. Iterate: `exploit2` against `league3`, then
`league4` seeding it — the AlphaStar loop at 1.65M parameters.
