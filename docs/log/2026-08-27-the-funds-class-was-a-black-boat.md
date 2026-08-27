# The funds class was a Black Boat all along

> `funds` was the largest power-free divergence class. It is now 5, and
> power-free agreement is **99.993% with 732 of 780 games clean** — the
> best measured here. The cause was not arithmetic: the engine has no
> Black Boat repair, and its cost was surfacing as a funds bug.

**The trail.** Fixing the join refund
(`2026-08-27-join-refunds-in-displayed-hp.md`) left 88, all of them whole
displayed points with the engine *richer*: +100 sixty-three times, +700
fourteen. Two readings were tested and both lost:

| tried | funds | clean of 780 |
|---|---|---|
| propagate join HP slack into funds | 88 (never fired) | 687 |
| charge the fractional repair point | 314 | 570 |

The second contradicted `decisions.md` — "repair adds raw HP and is
charged per displayed point", sourced to DefendPeace — and the corpus
sided with the decision, decisively. Worth recording as the case where
the corpus *confirmed* a settled rule instead of talking the engine out
of one.

**What it actually was.** 83 of the 88 turns contain a `Repair` action:
a Black Boat mending an adjacent unit. AWBW charges the owner for it and
the engine has no such order at all, so the engine simply kept the money
— one displayed point of whatever it mended, which is exactly the
signature. The record carries both halves:

```json
{"action":"Repair","Move":[],
 "Repair":{"unit":{"global":105334634},
           "repaired":{"global":{"units_id":105334816,"units_hit_points":8}},
           "funds":{"global":1700}}}
```

So the verifier adopts them rather than simulating a rule the engine has
not got, the way it already adopts `playersCOP` for powers. `rules.md`
lists it with the other omissions; self-play never issues the order.

| power-free games | before | after |
|---|---|---|
| funds | 88 | **5** |
| all divergences | 151 | **62** |
| clean | 687 | **732** |
| agreement | 99.983% | **99.993%** |

`unit-hp`, `damage-range`, `building-capture` and `capture-progress` all
fell too — a unit whose repair was never applied read as damaged for the
rest of the turn. **The first attempt at this changed nothing**, because
the record's `Move` is empty, so the acting unit was `None` and the funds
were never adopted; the repair is always the mover's own.
