# The Tier-4 powers land, and the corpus signs off

> Four mechanic classes -- per-unit-type power movement, conditional
> escalation, power attack deltas, range delta plus resupply -- cover
> every power of the Tier-4 five, so the engine gained Jake, Koal,
> Jess and Grimm for the price of the schema. Against 2,930 prepared
> games the change removes 17,711 divergences: power-game agreement
> 99.154% -> 99.451%, clean power games 221 -> 610, and the power-free
> column does not move at all.

**Setup.** `gen_cos.py` grows a `POWER_EFFECTS` table quoting co.php
per power; `co_data.rs` regenerates with per-unit `cop/scop_move_delta`,
`cop/scop_attack`, `cop/scop_conditional_bonus`, `cop/scop_range_delta`
and `resupply_on_power`; the engine consumes them in `co_modifiers`
(the formula already reads power attack as points over 100, so Grimm's
+50 adds straight into the universal +10's term), `effective_range`
(now power-aware), `power_move_bonus` (now per unit type) and
`activate_power` (Jess refills fuel and ammo on the spot). One data
judgment recorded in the generator: Jess's attack array cannot define
"vehicles" -- her APC shows 0 for want of a weapon, not membership --
so the class is transcribed from co.php and cross-checked against her
array at generation time. Firepower totals are pinned by unit tests:
Grimm 130/160/190, Jake on plains 110/130/150 and 110 off them,
Koal on roads 110/130/140, Jess vehicles 110/130/150 with footsoldiers
held to 100.

**Verification, before and after**, `verify-replays data/prepared
--no-fog`, 2,930 games:

| | before | after |
|---|---|---|
| fully clean games | 1,040 | **1,429** |
| overall agreement | 99.272% | **99.527%** |
| power games clean | 221 of 2,062 (10.7%) | **610 (29.6%)** |
| power-game divergences | 50,518 | **32,807** |
| power-free games | 99.994% | 99.994% |

The power-free row is the control: untouched, so the change did what
it claims and nothing else. The remaining 32,807 are dominated by the
COs still unmodelled -- the corpus is T4 games, so Max, Sami, Grit and
company fire powers the engine still plays as +10/+10 -- plus the
standing exclusions (silos, weather).

**What this unlocks.** `--co "Adder,Jake,Koal,Jess,Grimm"` now plays
every seat's powers for real, which makes the mixed-CO rung
(`docs/plan.md`, Lane B run 2) honest: no arm trains against a power
that fires as a no-op. Lane A is complete, same day it started.
