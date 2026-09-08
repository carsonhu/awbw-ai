# The pool is six opponents, and the bar asked about the wrong one

> `mixjm-s43` rated against every member of the league pool it was dealt: 90.0%
> against one sibling, 50.0% against another, 54.0% against JakeMan. The league
> seats **one** frozen member per iteration and `--seat-cap 50` prefers whoever
> is winning, so `--refresh-at 0.7` is checked against a ~52% mixture in
> practice and a ~64% one on paper. An absolute bar over a roster this uneven
> asks a different question every five iterations. The diversity was fine; the
> promotion rule was not.

**Setup.** `evaluate.py --versus` for the five frozen members, `--opponent
jakeman` for the bot, 100 games each, `--co Adder,Jake,Koal,Jess,Grimm` dealt
per game, `--temperature 1.0 --decide-cap` -- the conditions the league arms
actually trained under. Run locally; six at once exhausted the card, so two
were re-run alone.

| pool member | parent's score | decisive |
|---|---|---|
| `mixjm-s101` | 90.0 +-3.0 | 87.0 |
| `mixjm-s7` | 73.0 +-4.4 | 69.0 |
| `bc-net2-t4` (clone) | 70.5 +-4.6 | 68.0 |
| `mixjm-noanc-s43` | 54.0 +-5.0 | 48.0 |
| JakeMan | 54.0 +-5.0 | 52.0 |
| `mixjm-s202` | 50.0 +-5.0 | 49.0 |

**The seat decides the question.** Training logs read `vs a league of 5, one
per iteration`: each iteration deals 25% JakeMan plus one frozen member, held
five iterations. A uniform deal puts the parent at 0.25(54.0) + 0.75(67.5) =
**64.1%**, six points under a 0.7 bar -- a bar improvement could clear. But
`--seat-cap 50` exists to prefer the member beating the learner, which is
`mixjm-s202`, `mixjm-noanc-s43` and JakeMan; against those the parent averages
**~52%**, and 0.7 is eighteen points away and unreachable by any improvement.

**Two things corroborate the seating reading.** `lg-s43`'s one promotion came
at iteration 30, before seating had concentrated
(`2026-09-08-the-league-never-promoted.md`). And its rollout score oscillated
73.2, 52.3, 46.0, 27.4, 43.2, 55.9, 26.9 -- a range that tracks the roster's
own 50-to-90 spread, not instability. Which sibling was in the seat is the
whole signal.

**Three of six hold the parent to even.** 50.0, 54.0, 54.0 is a contested
pool, which is what the frozen half was assembled to provide. `mixjm-s101` at
90.0 and the clone at 70.5 are the near-free wins, and under seat-cap they are
also the members that stop being dealt -- they inflate the paper average while
contributing little to the run.

**What to change, in order.** Promote on the *seated* member rather than a
mixed window: beating whoever is in front of you is invariant to the roster's
spread, and it is the question a league wants. Failing that, set the bar near
**0.60**, a few points over the contested floor. Trimming `mixjm-s101` is
optional and cheap.

**What it does not settle.** Whether any of this makes a league ladder -- it
prices the bar, it does not test it. And the 50-to-90 spread across four
siblings of one rung is the seed spread again, measured here head-to-head
rather than through a panel.
