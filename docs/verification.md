# Verification

> How the replay harness checks the engine against recorded games, and what it
> currently agrees on.

## How it works

A replay zip holds two gzip members: a PHP-serialized game state per *turn*,
and that turn's orders. `state[i]` is the board before its own orders, so
`state[i+1]` is ground truth for replaying them.

`tools/prepare_replay.py` normalizes a replay — plus its map, fetched from
AWBW's `api/map/map_info.php` and cached — into flat JSON. `verify-replays`
then treats **each turn as an independent test case**: load the snapshot, apply
the recorded orders, diff against the next snapshot. Chaining a whole game
instead would let one wrong rule poison every turn after it.

Luck cannot be reproduced, so damage is checked as a *range* the record must
fall inside, and HP is resynced from the record afterwards.

## Running it

```
python tools/prepare_replay.py --glob '<replays>\*\*STD*.zip' --limit 400
cargo run --release -p awbw-replay --bin verify-replays -- data/prepared --no-fog
cargo run --release -p awbw-replay --bin check-fog -- data/prepared
```

Flags: `--no-fog`, `--no-powers`, `--vanilla`, `--verbose`, `--limit N`.

Always read the **split** the summary prints. Fog, and every power outside the
Tier-4 five, are unmodelled by design — a headline that mixes them in measures
features deliberately left out rather than correctness.

## Where it stands

| subset | games | exact | assertions | agreement |
|---|---|---|---|---|
| no powers, no fog | 868 | 819 (94%) | 975k | 99.994% |
| powers used | 2,062 | 610 (30%) | 5.97M | 99.451% |

Fog visibility, judged per path step: **99.39%** of 6,035 steps. Of the
mismatches, 31 are the engine seeing too much — almost all on the single turn
Drake fires Typhoon — and 6 are unexplained.

Residual in the clean subset is 63 divergences over 12,774 turns: 29 unit HP,
12 damage-range, 10 move-fuel, 5 unit-extra, 5 funds, 2 capture — luck-adjacent.

## Imitation data

`bc-stats` reports how much usable training data the replays yield:

```
cargo run --release -p awbw-replay --bin bc-stats -- data/prepared
```

Across 2,930 games: **1.19M labelled orders, 407 per game, 98.9% legal**, 94.2%
usable once power-affected orders are dropped. Humans spend 52% of orders
moving, 16% attacking, 16% capturing, 15% building.

It also reports rejections by position within the turn, which is how state
corruption shows itself: a flat profile means a rejected order is not poisoning
the ones after it.

Every legal order also **round-trips** through the action codec — 0 of 1,179,797
decode back to something else — so no label asks for an output the masks forbid.
`ReplayTeacher` serves the same labels to a trainer at ~20k orders/sec.

## Bugs it has caught

Worth knowing: more than half were in the *harness*, not the engine.

- Fuel charged along the engine's cheapest path instead of the route the player
  actually took.
- Writer: a unit killed by the counterattack recorded where it set off and
  unspent, and a transport named by its engine *slot*. A written game now
  round-trips at 100%.
- Orders paired to snapshots by line index, silently attributing one player's
  turn to their opponent on truncated replays.
- Recycled unit slots, three times over: a build inheriting a casualty's id made
  the casualty look alive — 8,311 phantom divergences from one stale mapping.
- Fog vision wrappers: AWBW gives the seat that could not see an action an
  *empty string*, not null, so "first non-null" picked the blind seat's blank.
- Mid-turn HP resynced from displayed HP, re-simulating later attacks from too
  high a baseline.
- A unit acting *without moving* gets an empty `Move`, not a missing one, so the
  translator dropped half of all captures and one attack in eight; see
  `decisions.md`.
- Engine: repair rounding up to the display step; Sami's capture rate and
  transport movement; Rachel's repair bonus.
