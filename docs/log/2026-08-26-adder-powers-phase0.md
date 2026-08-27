# The CO power meter, pinned from three sources before any code

> Phase 0 of Adder-vs-Adder powers: the charge rules, cross-checked between
> DefendPeace's AWBW port and the recorded meters in the corpus, which agree
> to the digit. Plus the supply count: 68 Adder mirrors, 852 tier-4-only games.

## The rules

Meter arithmetic, in the server's own units (the corpus records these):

- **One star is 90,000 units** — 9,000 funds x 10. Confirmed twice over:
  Jess's COP threshold is 270,000 (3 stars), Adder's 180,000 (2 stars).
- **Charge gained from combat** is *displayed*-HP damage priced in funds —
  full rate for damage taken, half for dealt — x10 for server units
  (DefendPeace's `calculateCombatCharge`; the phase-2 oracle re-checks it).
- **No charge accrues while your own power is active.**
- **Each activation raises every star by 1/5 of its base cost**, settling at
  3x base after 10 activations. Watched directly: Adder's COP threshold walks
  180,000 -> 216,000 -> 252,000 -> 288,000 across three pops.
- **Activation subtracts the cost and keeps the leftover** — game 601804
  shows it to the digit, twice (216,500 - 180,000 = 36,500; 404,500 - 270,000
  = 134,500). DefendPeace's blurb does not state this; the corpus proves it.
- **Charge runs past the COP threshold** (Jess banks 404,500 against a
  270,000 COP), i.e. the bar fills toward the SCOP. Whether it hard-caps at
  the SCOP cost is unconfirmed — check in phase 2.
- **One power at a time** (RizeBot's port of AWBW's own button logic,
  `game.js:6158-6177`), expiring at the owner's next turn start.
- **Every power grants +10 attack, +10 defence** while active, on top of its
  listed effect (DefendPeace applies this to all AWBW abilities uniformly).

Adder: no day-to-day, standard luck. Sideslip 2 stars, all units +1 movement;
Sidewinder 5 stars, +2 — confirmed by `co.php`, DefendPeace, and the corpus's
180,000 COP threshold independently.

## The supply

Of the 2,584 games in `data/prepared`: **446 involve Adder**, **68 are Adder
mirrors**, and **852 are tier-4-only pairs** — a third of the corpus, with
Adder-Jake its single most common pairing (177). BC has real food here.

Two gaps in the prepared data for phase 2 to close:

- Only about a third of prepared games carry *numeric* meter values
  (67 of the first 200 sampled; the rest have empty `co_power` maps). The
  accrual verification runs on that subset — still hundreds of games.
- The schema carries `co_max_power` (the COP threshold) but not
  `co_max_spower`; the normalizer should pass the SCOP threshold through too.

## Phase 1: the meter in the engine, corpus-neutral

Implemented same day: the meter on `Player` (raw server units), accrual in
`resolve_battle`, activation with escalation, expiry at the owner's turn
start, the universal +10/+10 in `co_modifiers`, and Adder's movement in
`Reach`. The full no-fog sweep is the regression gate, stash-compared on
identical code either side: the no-powers split went 305 divergences -> 304
(665 -> 666 clean of 780), everything else identical. The one improvement is
a snapshot that carries an active-power flag in a game the power detector
calls power-free — the simulated +10/+10 now matches AWBW there. Powered
games still diverge wholesale, as they must until the verifier applies the
5,863 recorded `Power` actions it currently skips: that is phase 2.
