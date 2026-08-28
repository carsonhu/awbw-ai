# Capture order: a backbone the opponent cannot touch, branches it can

> The question was whether the policy can genuinely perturb its capture
> schedule or plays one memorized order plus noise. Measured: the clone
> holds many coherent schedules, RL narrows the mirror opening to a
> committed plan — and loosens it again the moment the opposition
> actually differs.

**Method.** Mirror games of a checkpoint against itself at temperature
1, day cap 12 (openings only), plus the same checkpoint against
`capturer` and `greedy`; the engine's game log gives every completed
capture. Per seat: distinct first-3/first-5 capture plans, modal plan
share, captures completed by day 12 as the tempo check. 24 seat-games
per cell. Probe: `capture_order.py` (session scratchpad, trivially
recreated from `record=True` + the `Capt`/`captured` fields).

| seat-0 first-3 plans, of 24 | distinct | modal share | caps/game |
|---|---|---|---|
| clone, mirror | 14 | 12% | 13.3 |
| `league2-gen3`, mirror | 7 | 29% | 13.5 |
| `league3`, mirror | 3 | 88% | 13.9 |
| `league3` vs `capturer` | 4 | 58% | 16.2 |
| `league3` vs `greedy` | 4 | 50% | 15.2 |

**Three findings.** First, the clone's diversity is real capability:
fourteen distinct openings, none above 12%, every full sequence in the
same tempo band — generated routing, not a memorized order (the corpus
is thousands of maps; this map was never its lesson). Second, RL
commits rather than forgets: each self-play stage narrows the mirror
opening (12% → 29% → 88% modal) while capturing *faster* — the clone's
slow 10–11-capture tail is gone by `league3` — and by the fifth capture
the plans re-diversify. Third, the commitment is conditional: the modal
three cities are identical against every opponent (days 1–3 are out of
the opponent's reach, so nearest-first should be unconditional), but
the modal *share* drops from 88% to 50–58% against the bots — far
outside noise for p=.88 — and new cities enter the fifth-capture plans
against `capturer`. The schedule branches where the game first lets the
opponent touch it.

**Reading.** On a fixed, known map, "one plan at 88% in the mirror,
looser under perturbation, never tempo-losing" is what proper opening
play looks like — the same standardisation a human shows on a familiar
map. The thing to watch is the trend: if a later run's mirror modal hits
~100% *and* stops loosening against varied opposition, the branching is
gone and diversity needs seeding (map rotation, or opening temperature).
