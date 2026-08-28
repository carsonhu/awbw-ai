# The threat rung blows through the greedy ceiling

> Every checkpoint of the old lineage saturated against `greedy` in the
> low sixties — 63.2% for `ppo-adder1`, 65.1% after two more rungs, so
> stable it retired `greedy` as an instrument. The same recipe on the
> threat-plane clone rates **93.0% ±1.8**, and the climb took ~65
> policy iterations, not 200.

**Setup.** `ppo-threat1`: the `ppo-adder1` recipe verbatim (`--opponent
greedy --co Adder --turn-discount --steps 256 --lam 0.99 --decide-cap`,
200 iterations) on `--init bc-threat --threat-planes`. Kept checkpoint
from the 93.7% rollout window at iteration 90; the run then oscillated
and decayed at saturation exactly as the old family did — the regime
rules survive the lineage swap, and the kept-best logic banked the peak.

| 200 games each | `ppo-adder1` (old lineage, same rung) | `ppo-threat1` |
|---|---|---|
| vs `greedy` | 63.2 ±2.4 | **93.0 ±1.8** |
| vs JakeMan | ~2.5 (its recorded start line) | **5.5 ±1.6** |
| vs the clone | — (`ppo-adder3` manages 39.0) | 48.5 ±3.5 |
| vs `ppo-adder3` (3 rungs old lineage) | 15.4 | **19.5 ±2.8** |

**Reading.** The sixty-percent greedy plateau was never a fact about
`greedy` — it was the trunk's engagement arithmetic running out. Give
the policy the damage chart applied (`ce17ab7`) and the same PPO
pressure converts it into thirty more points against the bot the old
family could not separate itself from, plus a lead on every
stage-matched comparison. The climb dynamics also changed shape: 6% to
88% in fifty iterations, entropy *rising* through the climb — the
planes carry the where-to-shoot answer, so exploration spends itself
on everything else.

**Kept honest.** One seed; and `ppo-threat1` still loses 80.5/19.5 to
`ppo-adder3`, which has two extra rungs of curriculum — the lineage is
young, not yet strong. What this rung licenses is the swap itself: the
threat lineage climbs faster and higher on the same fuel, so the whole
ladder — JakeMan rung, self-play, exploiters, a species league run
from *behind* per the league4b rule — is worth rebuilding on it.
`bc-threat` and `ppo-threat1` are its first two members.
