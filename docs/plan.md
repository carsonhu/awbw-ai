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

**Lane A -- widen past the Adder mirror. Done.** ~~Per-seat CO~~ (`afbd4ab`)
and ~~Tier-4 powers~~ (`log/2026-08-29-the-tier-4-powers-land.md`).

**Lane B -- rented grids, about $3 each, seed-grouped always.**
1. ~~`jakeman2`~~: negative, every arm below its parent
   (`log/2026-08-30-the-rung-went-backwards.md`).
2. ~~The cap instrument~~ (`5543bde`): score and decisive share ride
   together, and re-rating the six arms found the frozen one.
3. ~~`-last.pt` for the six arms~~: the peak wins by 14.3 decisive points
   (`log/2026-08-30-the-peak-holds-and-one-arm-stops-winning.md`).
4. ~~The T4 re-clone~~: `bc-net2-t4` beats `bc-net2` on every panel
   member, both seeds, and pops 7.52 a game against 4.83
   (`log/2026-08-30-the-clone-was-learning-from-a-blind-engine.md`).
5. ~~Auxiliary value targets~~: negative, two fewer orders a turn
   (`log/2026-09-06-the-aux-targets-cost-two-orders-a-turn.md`).
6. ~~First mixed-CO rung~~: it leads, not stalls, and saturates
   `greedy` for all five COs
   (`log/2026-09-07-the-mixed-rung-does-not-stall-it-leads.md`).
   The pool is the recipe of record.
7. ~~The JakeMan rung~~ (`GRID=mixjm`): three anchored arms clear the
   49.3 record, the leash shows only on decisive share, and Grimm
   blocks a win across all five COs; `mixjm-s43` is the parent
   (`log/2026-09-07-the-rung-clears-the-record-and-grimm-gates-it.md`).
8. ~~The Grimm control~~: the CO, not the policy -- humans lose 18.5
   points switching Adder to Grimm, the arms 19.1
   (`log/2026-09-07-the-corpus-prices-grimm.md`).
9. Capacity, last, only if 8 leaves 96x8 looking like the bound --
   but with Grimm priced, `s43` and `s202` already meet the endpoint.

**Lane C -- self-play, deferred and not dropped.** Lane B's endpoint is a
ceiling: two scripted bots, `greedy` long saturated. Self-play worked here once
-- `league3`'s pool of four *families* moved every panel number at once
(`log/2026-08-27-the-species-league-holds.md`) -- and otherwise cycled into
specialists that beat their own past and nothing else. Four prerequisites:
1. ~~Games that end~~: the cap tiebreak is in the terminal reward and
   `decisive_wins` measures it.
2. ~~Scripted bots as pool members~~: `--pool-bots` deals per game and
   balances on games, not seats. Only JakeMan is worth seating -- `greedy`
   93-96%, `capturer` 15/15 -- so the diversity is the frozen half.
3. ~~Seed groups~~: `GRID=league`, four seeds against two rungs as control.
4. Search exists but stays out of the first league: it wants a contested
   opponent and amplifies whatever the critic prefers.

## Endpoint, and the two books after

A seed-grouped policy with a winning record against every scripted
opponent, across the Tier-4 COs, on the training slice -- not
promising human-looking composition or off-slice transfer. The sweep
saturates the panel, and hands off to:

**The Elo instrument.** Half exists: `value_diag.py` scores the value
head against recorded human outcomes by phase and rating band, the
one ground truth no bot supplies. Half does not: a real rating needs
live games against AWBW players -- etiquette and engineering.

**Search.** ~~One-ply value reranking~~ is built. It flattens the CO
spread against JakeMan, 31 points to 5, and collapses against `greedy`
(`log/2026-09-07-one-ply-flattens-the-cos-and-collapses-on-greedy.md`).
Not a default. No instrument here predicts what a search gets from a
critic, so the rungs above cannot yet be gated at all.

## Keeping this honest

Completed items shrink to a pointer at their log entry. If this doc
and a conversation disagree, this doc was reviewed -- fix it, then
act on it.
