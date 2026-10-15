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
1. ~~`jakeman2`~~: negative (`log/2026-08-30-the-rung-went-backwards.md`).
2. ~~The cap instrument~~ (`5543bde`): score and decisive share ride
   together, and re-rating the six arms found the frozen one.
3. ~~`-last.pt` for the six arms~~: the peak wins by 14.3 decisive points
   (`log/2026-08-30-the-peak-holds-and-one-arm-stops-winning.md`).
4. ~~The T4 re-clone~~: `bc-net2-t4` beats `bc-net2` on every panel member,
   both seeds (`log/2026-08-30-the-clone-was-learning-from-a-blind-engine.md`).
5. ~~Auxiliary value targets~~: negative, two fewer orders a turn
   (`log/2026-09-06-the-aux-targets-cost-two-orders-a-turn.md`).
6. ~~First mixed-CO rung~~: it leads, not stalls, saturates `greedy` for all
   five COs, and its pool is the recipe of record
   (`log/2026-09-07-the-mixed-rung-does-not-stall-it-leads.md`).
7. ~~The JakeMan rung~~ (`GRID=mixjm`): three anchored arms clear the
   49.3 record, the leash shows only on decisive share, and Grimm
   blocks a win across all five COs; `mixjm-s43` is the parent
   (`log/2026-09-07-the-rung-clears-the-record-and-grimm-gates-it.md`).
8. ~~The Grimm control~~: the CO, not the policy -- humans lose 18.5
   points switching Adder to Grimm, the arms 19.1
   (`log/2026-09-07-the-corpus-prices-grimm.md`).
9. ~~Capacity~~: not needed -- `s43` and `s202` already meet the endpoint.

**Lane C -- self-play.** Lane B's endpoint is a ceiling: two scripted bots,
`greedy` long saturated. It worked here once -- `league3`'s four *families*
moved every panel number (`log/2026-08-27-the-species-league-holds.md`) -- and
otherwise made specialists that beat their own past. Four prerequisites:
1. ~~Games that end~~: the cap tiebreak is in the terminal reward and
   `decisive_wins` measures it.
2. ~~Scripted bots as pool members~~: `--pool-bots` deals per game and
   balances on games, not seats. Only JakeMan is worth seating -- `greedy`
   93-96%, `capturer` 15/15 -- so the diversity is the frozen half.
3. `GRID=league` ran and never laddered -- one promotion in 800 arm-iterations
   (`log/2026-09-08-the-league-never-promoted.md`), because a blended bar asks
   a new question every seat (`log/2026-09-08-the-pool-is-six-opponents.md`).
   The rule is rebuilt per member and priced: 0.60 over 30 games each fires on
   4.5% of runs that gain nothing, 78% of runs ten points better
   (`log/2026-09-08-the-promotion-bar-is-calibrated.md`). `GRID=league2`
   re-runs it with four controls.
4. Search stays out of the first league: it wants a contested opponent and
   amplifies whatever the critic prefers.
5. **Experience is the gap.** The Showdown #1 bots share this recipe and train
   on 1e8+ games at 262k rows an update; an arm here plays ~1,800 at 8,192
   (`log/2026-10-15-the-recipe-is-right-and-the-run-is-small.md`). In order:
   `GRID=batch` (is "decays from level" noise?); time `--amp` on the rented
   card; then, if batch is the cause, the live learner in half the league's
   seats (both sides on-policy) and terminal-only reward once self-play leads.

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
