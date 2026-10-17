# The power moves half as far as a human's, and fires six times as often

> End-of-turn popping is long fixed: tonight's league arm pops after 4.3 unit
> orders, humans after 2.2. But a pop spends about half the movement a
> human's does -- 26% of powered moves need the bonus against 51%, 3.2 a pop
> against 6.9 -- and the policy fires ~9.7 COPs a game as Adder where humans
> fire 1.46 and save 17% of pops for the SCOP; the policy saves ~1%.

**The probe.** `play_diag.py` now reads, for a power that adds movement
(Adder, Koal, Jess, Jake's SCOP), every move a boosted unit makes after the
pop and asks whether its destination was out of base range. Reach is judged
on terrain alone from the cheapest route, for both sides: humans can draw
roundabout paths in AWBW, so a recorded path would overcount them, and a
move pushed long by blockers reads as in range -- a floor, the same floor
for the engine and the corpus. Step costs come from `gen_tables.py`'s own
tables; against the engine's fuel use they matched 338 of 338 moves. The
human side is the prepared corpus on A River Supreme without fog, by CO.
60 games a cell against JakeMan.

**COP, as Adder** (Sideslip, +1 move):

| | needed the bonus | bonus moves a pop | pops that used it | unit orders before the pop |
|---|---|---|---|---|
| `mixjm-s43` (parent) | 28.0% | 2.6 | 80% | 6.3 |
| `b128-s43-last` | 26.1% | 3.5 | 90% | 4.8 |
| `lg128-s7-last` | 26.1% | 3.2 | 90% | 4.3 |
| `lg128-s43-last` | 22.4% | 3.2 | 91% | 4.3 |
| human, 1,227 pops | 51.0% | 6.9 | 99% | 2.2 |

Koal is the same shape (`lg128-s7`: 25.7% and 3.3 a pop against 51.7% and
6.9). Jess, whose powers move vehicles only, is the widest gap: 22.6%, 1.1
a pop and 66% of pops against 37.2%, 2.2 and 91%; the parent was at 0.9 and
56%. Each rung has moved the pop earlier and used more of it.

**How often.** Movement-power pops a game, corpus against policy:

| | COP | SCOP |
|---|---|---|
| human Adder (838 games) | 1.46 | 0.31 |
| human Koal (244) | 0.70 | 0.58 |
| human Jess (475) | 0.80 | 0.42 |
| `lg128-s7`, as Adder | ~9.7 | ~0.03 |

**Reading it.** The policy spends its meter on the cheap power the moment
it can, and many of those pops come with nothing that needs to go far, which
pulls the per-move share down. Humans pop seldom, save for the SCOP, and
move almost everything further when they do. A shaped reward credits a COP's
stat boost at once and a SCOP saved for later never; terminal-only reward is
the plan item that removes that preference. Pops a game are not normalised
for game length, and human games are against humans, not JakeMan.
