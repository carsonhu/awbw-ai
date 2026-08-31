# The clone was learning from an engine that ignored four COs in five

> Re-cloning on the Tier-4-aware engine, same corpus and recipe, beats `bc-net2`
> on **all four panel members with both seeds** -- greedy 30.2 -> 37.5/46.2, the
> clone 61.5 -> 75.0/75.5, JakeMan 6.5 -> 11.0/10.4, `ppo-adder3` 78.5 ->
> 80.0/83.5. The cause is a filter nobody re-read: power turns are dropped
> unless *every* CO in the game is modelled, which meant 147 games until the
> Tier-4 five landed and 1,137 after. The clone now pops 7.52 a game, not 4.83.

**What changed under the recipe.** `Cursor::with_lookahead` computes
`powers_modelled` as "every player's CO has `power_effects_modelled`", and drops
the whole power turn otherwise (`imitate.rs`). That predicate is dynamic and its
doc comment still says "(Adder)", because Adder was once the only one. `b8c584a`
flagged four more on 2026-08-29 at 19:37 -- ten hours *after* `bc-net2` was
cloned at 09:34. Every checkpoint in use descends from that clone.

| | games whose power turns enter the loss |
|---|---|
| Adder modelled only | 147 |
| all five modelled | **1,137** (147 all-Adder + 990 mixed) |
| still dropped (a non-T4 CO present) | 1,808 |

**Setup.** The recipe recovered this session and now in `workflow.md`, run
unchanged on the same 2,945 prepared files -- verified, none postdates
`bc-net2`. Only the engine differs. Panels are 200 games a member,
`--co Adder --temperature 1.0`, score / decisive.

| member | `bc-net2` (s7) | t4 s7 | t4 s43 |
|---|---|---|---|
| greedy | 30.2/26.5 | **46.2/40.5** | 37.5/29.5 |
| JakeMan | 6.5/5.0 | 10.4/9.5 | **11.0/10.5** |
| clone | 61.5/54.5 | **75.5/68.5** | 75.0/68.0 |
| `ppo-adder3` | 78.5/74.0 | **83.5/79.5** | 80.0/70.5 |

**Both seeds win every column, and the tight columns are the honest ones.**
The clone member reads 75.0 and 75.5 against 61.5 -- a 0.5-point seed spread
against a 14-point effect, non-overlapping at ±3.4. JakeMan is 11.0 and 10.4
against 6.5. `greedy` spreads 8.7 points between seeds and is the one column
that would support a wrong story on its own, which is its retirement working.

**Clone seeds are far tighter than RL seeds, measured here first.** Two seeds
of one clone sit 0.5 apart on the clone member, 0.6 on JakeMan, 3.5 on
`ppo-adder3`, 8.7 on greedy; two seeds of a PPO recipe were 27 apart
(`log/2026-08-29-one-recipe-two-seeds.md`). BC is the reproducible end of this
project and RL is not.

**Accuracy ran backwards, again.** Seed 43 held out order 0.402 -- below the
0.454-0.469 band of every 15,000-step run on record -- and still rates level
with or above seed 7 on JakeMan and the clone.

**The mechanism shows up where it should.** Against `greedy`, 60 games:
`bc-net2` fires 4.83 powers a game at order 4.7 of a 15.7-order turn, the
re-clone **7.52 at order 4.1 of 16.0**. Indirect spending goes 4.4% to 5.9%
against the corpus's 3.9%; composition is otherwise unmoved, Anti-Air 8.3% to
8.2% against a human 6.1% and the RL policies' 15%.

**What it does not settle.** `bc-net2` is one seed, so this is a two-seed group
against a one-seed control -- the group's worst still wins every column. And the
engine change's two effects, more admitted power turns and richer labels within
them, are not separated.
