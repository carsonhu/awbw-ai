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

**Lane A -- local, free: widen past the Adder mirror. Done.**
~~Per-seat CO~~ (`afbd4ab`) and ~~Tier-4 power effects~~
(`log/2026-08-29-the-tier-4-powers-land.md`): mixed-CO training is honest.

**Lane B -- rented grids, about $3 each, seed-grouped always.**
1. ~~`jakeman2`~~: no climb. Its 70.5 was the day-cap tiebreak, 47.5
   without; both unanchored arms collapsed in training; `cut` is blind
   to a settled cap (`log/2026-08-29-the-day-cap-paid-for-the-rung.md`).
2. The cap instrument (next, free): make `cut` count capped games
   however settled, rate the panel both ways, re-rate the six arms. No
   rung is worth $3 until the score means what it says.
3. Net change + re-clone, one bundle: auxiliary value targets
   (`network.md` item 3; pooled bias and mean+max shipped in net-v2),
   on a corpus replayed through the Tier-4 engine. BC, then the greedy
   rung as the free benchmark.
4. First mixed-CO rung, after 3 so the clone has seen T4 powers fire:
   opponents sampled across the five, an Adder-mirror control group. A
   mixed group that fails while its control climbs is what "the policy
   cannot tell who it is" looks like.
5. Capacity, last, only if 3 leaves 96x8 looking like the bound.

## Endpoint, and the two books after

A seed-grouped policy with a winning record against every scripted
opponent, across the Tier-4 COs, on the training slice -- not
promising human-looking composition or off-slice transfer. The sweep
saturates the panel, and hands off to:

**The Elo instrument.** Half exists: `value_diag.py` scores the value
head against recorded human outcomes by phase and rating band, the
one ground truth no bot supplies. Half does not: a real rating needs
live games against AWBW players -- etiquette and engineering.

**Search** -- `network.md` calls it the largest untouched lever -- in
rising order: one-ply value reranking on the 130k micro-steps/s
engine, turn-level low-budget Gumbel, search-as-teacher. Gated on
the run-3 critic (a search is worth what its evaluator knows) and
scored on the Elo instrument, the panel having just saturated.

## Keeping this honest

Completed items shrink to a pointer at their log entry. If this doc
and a conversation disagree, this doc was reviewed -- fix it, then
act on it.
