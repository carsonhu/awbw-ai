# League4 decayed, and no league game was ever against one opponent

> Five promotions, a healthy-looking run, and a product that loses
> 73.5/26.5 to its own init. The pool was solved by iteration 80 and
> the rest of the run decayed from ahead — because the member that
> should have prevented that never actually got to play.

**Setup.** `league4`: init `league3`, pool of clone + `exploit1` +
`exploit2` + `ppo-adder3` + `sp-parity-gen3`, both caps, 200
iterations. Every prior league norm respected; the product regressed
on nearly everything.

| panel, 200 games each | `league3` | `league4` |
|---|---|---|
| vs `greedy` | 42.0% | 44.0 ±3.5 |
| vs JakeMan | 24.5% | **11.5 ±2.3** |
| vs the clone | 89.0% | **73.5 ±3.1** |
| vs `ppo-adder3` | 95.0% | **55.8 ±3.5** |
| vs `league3` (its init) | — | **26.5 ±3.1** |
| vs `exploit2` | 40.0% | **37.5 ±3.4** |

**Mechanism one: solved pool, from-ahead decay.** By iteration 80
every member sat at or below the learner and PFSP had no wall to lean
on. The remaining 120 iterations ran in the regime this project has
already convicted — level-or-ahead decays — with a new attractor:
activations climbed from 1 to 6.9 per game. Charge comes from damage
dealt and taken, so a pool of Adder mirrors co-evolves charge-farming;
that style wins the mirror and feeds material against anyone playing
conventionally, which is the −39 against `ppo-adder3`.

**Mechanism two: the chimera opponent.** `exploit2` beats `league3`
60/40 — it was seeded precisely to be the wall — yet its slot read
79–84% *for the learner* from the run's first windows. The arithmetic:
a game runs ~1,170 orders and a rollout is 256 per env, so a game
spans 4–5 iterations, and `take_opponent` swaps the frozen weights
every iteration. **No league game in any league run has ever been
played against a single member.** Games are blends; a member whose
strength is a whole-game plan (exploit2's power timing) dissolves,
while one whose strength is a per-segment style (exploit1's copters)
survives — which is why league3 still worked. Per-member stats have
been mixtures all along, so PFSP has been steering on smeared signals.

**What league3's success now means.** It was earned against blended
opponents plus a style-pressure exploiter — real, the panel confirms
it — but the league machinery has been weaker than designed the whole
time, and league4 is what happens when the pool's teeth depend on
whole-game plans.

**Fix, in order.** Hold the seated member for ~5 iterations (one game
length), so most games are single-opponent: `--seat-hold`, counting
each held iteration against the seat cap. Cheap, and it also makes the
per-member scores mean something. Per-env members (the AlphaStar per
game sampling) stays rejected on cost: one forward pass per pool
member per step. Then rerun this exact league; `league3` stays the
checkpoint of record meanwhile. If the rerun still spirals into
charge-farming once it solves the pool, the run should simply stop at
the last promotion instead of training from ahead — a stopping rule,
not more members.
