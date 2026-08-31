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
1. ~~`jakeman2`~~: negative. Every trained arm finished below its
   parent, and the arm that matched it was its own init
   (`log/2026-08-30-the-rung-went-backwards.md`).
2. ~~The cap instrument~~ (`5543bde`): score and decisive share now ride
   together, and re-rating the six arms is what found the frozen arm.
3. ~~`-last.pt` for the six arms~~: the peak wins by 14.3 points of
   decisive wins, five arms of six, and one endpoint rates 65.8% on
   2.0% (`log/2026-08-30-the-peak-holds-and-one-arm-stops-winning.md`).
4. ~~The T4 re-clone~~: `bc-net2-t4` beats `bc-net2` on all four
   panel members on both seeds, and pops 7.52 a game against 4.83
   (`log/2026-08-30-the-clone-was-learning-from-a-blind-engine.md`).
   It is the parent everything downstream should now start from.
5. Auxiliary value targets (next; local and free): `network.md` item
   3, the last of the global bundle. The value-head rails exist --
   `--value-outcomes` already trains it on recorded results -- so what
   is missing is the extra targets and their teacher plumbing, not the
   mechanism. Then the greedy rung as a check that it trains.
6. First mixed-CO rung -- the clone has now seen T4 powers fire -- and
   the first item that rents a box: opponents sampled across the
   five, an Adder-mirror control group. A mixed group that fails while
   its control climbs is what "the policy cannot tell who it is"
   looks like.
7. Capacity, last, only if 5 leaves 96x8 looking like the bound.

**Lane C -- self-play, deferred and not dropped.** Lane B's endpoint is a
ceiling: two scripted bots, `greedy` long saturated. Self-play worked here once
-- `league3`'s pool of four *families* moved every panel number at once
(`log/2026-08-27-the-species-league-holds.md`) -- and otherwise cycled into
specialists that beat their own past and nothing else. Four prerequisites:
1. Games that end. The terminal reward adds the cap tiebreak (`lib.rs`), so
   self-play optimises day-60 income on both sides; `decisive_wins` now
   measures the thing that should be rewarded.
2. Scripted bots as pool members. The env takes one opponent at construction
   and the pool holds only state_dicts, so JakeMan cannot be a member -- and
   `league3`'s `greedy` column stalled for exactly that reason.
3. Seed groups. Every league result on record is one seed, from before the
   27-point spread was measured.
4. Search, which is what makes self-play compound rather than drift.

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
the run-5 critic (a search is worth what its evaluator knows) and
scored on the Elo instrument, the panel having just saturated.

## Keeping this honest

Completed items shrink to a pointer at their log entry. If this doc
and a conversation disagree, this doc was reviewed -- fix it, then
act on it.
