# PFSP made the best checkpoint yet, then spent 120 iterations eating it

> One run, one pool, two verdicts: at generation three the league rates
> 19.0% against JakeMan — the best this project has scored — and by
> generation six it rates 1.5%. Seating by loss rate concentrates the
> run on whoever beats the learner, and past the first promotions that
> is always its own newest generation.

**Setup.** `league2`: the league1 recipe under the PFSP fix (`3a41422`)
— `(1-score)^2` seating, self-lineage cap 0.4, same five seeds, init
`ppo-adder3`. Paused at ~80/200 iterations (3 promotions) to free the
GPU; resumed from `league2-gen3` with the pool restored, remaining 120
iterations saved under `league2b`. Six promotions total across the line.

| panel, 200 games each | `ppo-adder3` | `league1-gen7` | `league2-gen3` (~iter 80) | `league2b` (gen6, ~iter 140) |
|---|---|---|---|---|
| vs `greedy` | 62.5% | 14.5% | 42.8 ±3.5 | **4.0 ±1.4** |
| vs JakeMan | 15.5% | 3.5% | **19.0 ±2.8** | 1.5 ±0.9 |
| vs the clone | 39.0% | 24.5% | **58.0 ±3.5** | 19.0 ±2.8 |
| vs `ppo-adder3` | — | 50.5% | **91.5 ±2.0** | 56.5 ±3.5 |

**The first 80 iterations vindicate PFSP over round robin.** Where
league1 spent 200 iterations arriving back at its own start, gen3 beats
its init 91.5% while *improving* on it against JakeMan and the clone —
the first league product that transferred to opponents outside the
pool. The one loss, 20 points of `greedy`, is the bot line it no longer
practices against.

**The next 120 iterations are the same mechanism failing.** Once the
learner's own promotions are the only members that beat it, `(1-score)^2`
seats little else: the report lines show its fresh generations at 35–47%
drawing the iterations while solved seeds sit at 62–83%. The second half
of the run was played from behind against its own copies — climb-from-
behind pointed at the one opponent that guarantees specialisation — and
the panel collapsed across the board while the in-run window still read
51.3%. Round robin drifted by *composition*; PFSP drifts by *attention*.
The cousin problem survived the fix aimed at it.

**Caveat.** The resume re-entered gen1–3 via `--frozen-init`, so the
lineage cap did not count them and the pool ended at 11 with six of one
family. That amplified the drift; it did not invent it — the members
beating the learner were its own generations before and after the seam.

**What this settles, and what it opens.** `league2-gen3` is the
checkpoint to build from: best on three of four panel members, kept as
`checkpoints/league2-gen3.pt`. The lever that has not moved is species:
every pool member is one network family, and no sampling scheme can
seat an opponent that is not there. Next is exploiters — fresh clones
trained against the current best — and keeping a scripted-bot-shaped
opponent in the pool, so "who beats the learner" is not always "itself."
A cap on iterations-per-generation (stop feeding a gen once seated N
times) would bound the failure even with today's pool.
