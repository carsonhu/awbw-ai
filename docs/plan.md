# Plan

> The agenda of record: what runs next, in what order, at what cost, and
> where scope deliberately stops. A plan that lives in a conversation
> dies with it -- this one is edited as decisions land, and each item
> that completes points at the log entry that measured it.

## Scope, decided

- **COs: the Tier-4 five -- Adder, Jake, Koal, Jess, Grimm -- and no
  further.** Their powers decompose into four mechanic classes
  (per-unit-type power move, conditional-bonus escalation, power
  attack/defense deltas, range delta + resupply), so five COs cost the
  same engine work as two. **Max is data-only**: his games feed the T4
  corpus the clone trains on, but he is never a seat. No extension
  past Tier 4 is planned; a policy that infers its CO from effects (no
  identity plane) is the bet, and a one-hot would be an observation
  break.
- One map (A River Supreme), standard weather, no fog in training.
  Silos, pipe seams, mid-game weather: out (`rules.md`).

## Lanes

**Lane A -- local, free: widen past the Adder mirror.**
1. ~~Per-seat CO in `awbw-py`~~ done (`afbd4ab`): `--co` takes a comma
   pool, each seat sampled per game from the game's own seed; one name
   is still a mirror, so every logged recipe and rating is untouched.
2. ~~Tier-4 power effects~~ done: four mechanic classes in the
   engine, five data rows in `gen_cos.py`. Power-game agreement
   99.15% -> 99.45%, clean power games 221 -> 610 of 2,062
   (`log/2026-08-29-the-tier-4-powers-land.md`). Mixed-CO training
   is now honest.

**Lane B -- rented grids, about $3 each, seed-grouped always.**
1. `jakeman2` (ready): the continuation rung from `jm-s7par-s7`, four
   anchored seeds plus two unanchored arms pricing `--anchor-kl 0.03`.
   Bar: 49.3. Independent of Lane A; launch any time.
2. First mixed-CO rung (after Lane A): the winning recipe, opponents
   sampled across the five COs, an Adder-mirror control group. A
   mixed group that fails while its control climbs is what "the
   policy cannot tell who it is" looks like.
3. Net change, one bundle (after Lane A, so the re-clone happens
   once): global-pooling bias, mean+max value pooling, auxiliary value
   targets (`network.md` item 3). BC, then the greedy rung as the
   free benchmark.
4. Capacity, last, only if 3 leaves 96x8 looking like the bound.

## Endpoint, and the two books after

A seed-grouped policy with a winning record against every scripted
opponent, across the Tier-4 COs, on the training slice. Not promised
on the way: human-looking composition, or transfer off the slice.
That sweep saturates the panel -- JakeMan is the strongest scripted
AI available -- and hands off to:

**The Elo instrument.** Half exists: `value_diag.py` scores the value
head against recorded human outcomes by phase and rating band, the
one ground truth no bot supplies. Half does not: a real rating needs
live games against AWBW players -- etiquette and engineering.

**Search** -- `network.md` calls it the largest untouched lever -- in
rising order: one-ply value reranking on the 130k micro-steps/s
engine, turn-level low-budget Gumbel, search-as-teacher. Gated on the
run-3 critic (a search is worth what its evaluator knows) and scored
on the Elo instrument, since the panel it would tune against is the
thing that just saturated.

## Keeping this honest

Completed items shrink to a pointer at their log entry. If this doc
and a conversation disagree, this doc was reviewed -- fix it, then
act on it.
