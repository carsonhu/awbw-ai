# Two threat-lineage rungs beat the old ladder end to end

> The JakeMan rung on the threat lineage rates 37.5% — the old
> lineage's same rung managed 15.5%, and its best checkpoint after
> seven runs of laddering managed 24.5%. The greedy gain held through
> it (89.5%), and the two-rung product beats the old three-rung
> veteran 82/18.

**Setup.** `ppo-threat2`: the old JakeMan rung's recipe minus the
retired `--adv-floor` (`--init ppo-threat1 --threat-planes --opponent
jakeman --co Adder --turn-discount --steps 256 --lam 0.99
--decide-cap`, 200 iterations). Start line 5.5%. Same two-phase shape
as the greedy rung: fifty flat iterations, then jumps — 5→24 at
iteration 90, 22→38 at 190 — ending mid-climb, not saturated.

| panel, 200 games each | `ppo-adder3` (old, 3 rungs) | `league3` (old best) | `ppo-threat2` (2 rungs) |
|---|---|---|---|
| vs `greedy` | 65.1% | 42.0% | **89.5 ±2.2** |
| vs JakeMan | 15.5% | 24.5% | **37.5 ±3.4** |
| vs the clone | 39.0% | 89.0% | 84.0 ±2.6 |
| vs `ppo-adder3` | — | 95.0% | 82.0 ±2.7 |

**Reading.** The replication holds — the greedy rung's advantage was
not a one-seed fluke; the same lineage, next rung, doubles the
project's best JakeMan number while keeping the greedy one. Curriculum
also transfers better on this lineage: the old family gave back its
greedy strength slowly across rungs, this one kept 89.5 of 93.

**Next, and why the lineage resets again.** The run ended climbing, so
a continuation rung runs as the v1 reference. But the planes carry the
deterministic damage *floor* — zero luck — and the tactics humans
actually play by are probabilities over the ten luck rolls: a
tank+infantry combo at ~98% to KO an infantry on a city, a 33% 2HKO
raised by sacrificing a chip attack first (which also strips terrain
stars through the displayed-HP scaling). The env re-encodes after
every *order*, so a policy holding correct per-attack P(KO) planes can
play those combos as greedy steps over refreshed probabilities — no
internal tree needed. Threat planes v2 (expected damage and P(KO),
both directions, per-CO luck ranges via `damage_spread`) are the next
observation, and the greedy rung is the free benchmark: v2 should
match 93% there, and anything above it is the probability axis paying.
