# The seat hold fixed the measurement; the regime was the disease

> League4 rerun with games played against single members, exactly as
> the blend diagnosis prescribed — and the product still loses 77/23
> to its init. Two failures with one shape: a league whose learner
> opens ahead of its pool decays, exactly as every from-ahead run in
> this project has, and no seating scheme can fix who is ahead.

**Setup.** `league4b` = league4 with `--seat-hold 5` (`50bf47d`), so a
game is mostly one opponent. Four promotions, kept artifact from
iteration 130.

| panel, 200 games each | `league3` | `league4` (blend) | `league4b` (hold) |
|---|---|---|---|
| vs `greedy` | 42.0% | 44.0 | 22.5 ±3.0 |
| vs JakeMan | 24.5% | 11.5 | 7.2 ±1.8 |
| vs the clone | 89.0% | 73.5 | 65.5 ±3.4 |
| vs `ppo-adder3` | 95.0% | 55.8 | 89.0 ±2.2 |
| vs `league3` (init) | — | 26.5 | **23.0 ±3.0** |

**What the hold did do.** The per-member reads became real: exploit2 —
smeared to a nonsensical 80% under blending — sat near 50% early, was
solved mid-run, then tore the drifted learner apart at iterations
160–190 (windows of 13% and 7%, its slot falling to 34%). The
promotion gate kept the artifact from before that collapse, which is
the stopping rule working. Charge-farming stayed bounded (pops ~1–2
per game against real walls, not 6.9). The machinery is now honest.

**What it could not do.** `league3` opens at or above parity with
everything seeded against it except exploit2's 60/40 — and a 60/40
wall earns, under loss-rate seating with a cap, only a share of the
run. The learner spent most of 200 iterations level or ahead, which
is the one regime this project has never once trained through without
decaying (`log/2026-08-26-ppo-only-climbs-from-behind.md` onward).
League3 succeeded from an init its pool *beat* — `exploit1` took
78.5% off `league2-gen3`. The ladder only climbs while something in
the pool genuinely outranks the learner, and after exploit2's
shrunken margin, nothing did.

**The rule this buys.** Do not run a league whose learner opens above
~55% against the pool average; it will spend the run decaying from
ahead. Renew the pool's teeth first: exploiters from different species
roots (the clone root now yields only 60/40 — try `ppo-adder3` as an
exploiter init, or wait for the threat-planes lineage), or scripted
bot members (engine support). `league3` remains the checkpoint of
record, now against three challengers' worth of evidence.
