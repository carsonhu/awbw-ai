# The aux targets cost two orders a turn

> KataGo's third global-bundle answer, built and measured: three auxiliary
> targets (final property differential, income five days out, per-tile final
> ownership) **lose**. No panel column wins, `greedy` loses as a group, and both
> seeds take **14.0 and 13.5-order turns against the parent's 16.0** -- building
> 62.9 units a game against 68.8 and spending 3.4M fewer funds. The expensive
> part is not the loss of points: the clone member's seed spread went **0.5 to
> 18.2**, so the one reproducible end of this project stopped being reproducible.

**What was built.** `network.md` item 3. The targets are *futures*, which a
teacher streaming a game forwards cannot see, so `aux-targets` (a new
`awbw-replay` binary) dumps them once per corpus -- 2,945 games, 83,256 turns,
none that would not load, 2.3 MB, 103 seconds -- and `bc.py --value-aux` joins
them by `(game, turn)`, the key `ReplayTeacher.turn_index()` already reports.

**Setup.** The `bc-net2-t4` recipe unchanged, plus `--value-aux` at
`--aux-coef 0.3`, seeds 7 and 43. Panels are 200 games a member, `--co Adder
--temperature 1.0`, score / decisive. The control is the two `bc-net2-t4` seeds
at identical settings (`log/2026-08-30-the-clone-was-learning-from-a-blind-engine.md`).

| member | t4 s7 | t4 s43 | aux s7 | aux s43 |
|---|---|---|---|---|
| greedy | **46.2**/40.5 | 37.5/29.5 | 34.5/30.0 | 27.5/23.0 |
| jakeman | 10.4/9.5 | **11.0**/10.5 | 4.5/4.5 | 10.5/10.5 |
| clone | **75.5**/68.5 | 75.0/68.0 | 57.0/54.5 | 75.2/59.0 |
| `ppo-adder3` | 83.5/79.5 | 80.0/70.5 | 78.5/74.0 | **86.0**/77.5 |

**One column loses outright, and the rest are unreadable.** Both aux seeds sit
below both control seeds on `greedy` and nowhere else; `jakeman` and
`ppo-adder3` straddle. The clone member is the trap: aux s43's 75.2 reads level
with 75.0 and 75.5 until the decisive share is read beside it -- 59.0 against
68.0, on 17.5% of games capped. That is `--decide-cap` earning its place again
(`log/2026-08-29-the-day-cap-paid-for-the-rung.md`).

**The reproducibility is the real cost.** Seed spread, control against aux:
`greedy` 8.7 -> 7.0, `jakeman` 0.6 -> 6.0, clone **0.5 -> 18.2**, `ppo-adder3`
3.5 -> 7.5. Clone seeds being tight is what made a two-seed group a cheap
instrument; under this recipe it is not one, and a follow-up cannot resolve a
small effect with two seeds.

**Accuracy said nothing again.** aux s43 held out order 0.409 against t4 s43's
0.402, from a checkpoint that plays worse on three members.

**The mechanism is that it does less.** 60 games against `greedy`, `--co Adder`,
aux s43 against `bc-net2-t4`: 62.9 builds a game against 68.8, 22.06M funds
against 25.50M, 6,621 direct attacks against 7,361 and each worth +1,435 against
+1,557, indirects 72 attacks at +1,357 against 111 at +2,446. Turns run 14.0
orders against 16.0, and aux s7 runs 13.5 -- the one thing both seeds agree on.
Power timing survived the change: 6.77 and 7.28 pops a game against the parent's
7.52, far above the pre-T4 clone's 4.83, so this is not the T4 gain being lost.

**What it does not settle, and what the next arm has to do.** The three targets
were measured together and only one of them is dense: the ownership map writes a
gradient into all 306 spatial positions every step, where the two globals are one
2-vector a row. The head whose behaviour moved -- the source head, which decides
end-turn -- is the other head that reads every tile. That is a hypothesis, not a
finding, and separating it needs the ownership term's own coefficient, which the
one `--aux-coef` does not give it. Nor was a smaller weight tried: 0.3 was picked
to sit alongside `--value-coef 0.5`, never measured.
