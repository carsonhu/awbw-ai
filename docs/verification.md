# Verification

The engine is checked by replaying real AWBW games through it and diffing
against what actually happened.

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
cargo run --release -p awbw-replay -- data/prepared --no-fog
cargo run --release -p awbw-replay --bin check-fog -- data/prepared
```

Flags: `--no-fog`, `--no-powers`, `--vanilla`, `--verbose`, `--limit N`.

Always read the **split** the summary prints. Powers and fog are unmodelled by
design, so a headline number that mixes them in measures features that were
deliberately left out rather than correctness.

## Where it stands

| subset | games | exact | assertions | agreement |
|---|---|---|---|---|
| no powers, no fog | 127 | 108 (85%) | 147k | 99.979% |
| powers used | 239 | 2 | 721k | 98.81% |

Fog visibility, judged per path step: **99.39%** of 6,035 steps. Of the
mismatches, 31 are the engine seeing too much — almost all on the single turn
Drake fires Typhoon — and 6 are unexplained.

Residual in the clean subset is 31 divergences over 1,916 turns: 13 funds, 13
unit HP, 5 stragglers. No common cause found; diminishing returns.

## Imitation data

`bc-stats` reports how much usable training data the replays yield:

```
cargo run --release -p awbw-replay --bin bc-stats -- data/prepared
```

Across 366 non-fog games: **138k labelled orders, 377 per game, 97.4% legal**,
90.7% usable once power-affected orders are dropped. Humans spend 57% of orders
moving, 17% building, 16% attacking, 9% capturing.

It also reports rejections by position within the turn, which is how state
corruption shows itself: a flat profile means a rejected order is not poisoning
the ones after it.

Every legal order also **round-trips** through the action codec: 0 of 134,518
encode to a code that decodes back to something else. A label that failed this
would teach the policy to reach for an output its masks can never produce.

`ReplayTeacher` in `awbw-py` serves the same labels to a trainer, one game per
slot, at ~20k orders/sec — roughly half of that spent parsing replay JSON.

## Bugs it has caught

Worth knowing, because more than half were in the *harness* and would otherwise
have read as engine faults:

- Fuel charged along the engine's cheapest path instead of the route the player
  actually took.
- Orders paired to snapshots by line index, silently attributing one player's
  turn to their opponent on truncated replays.
- Recycled unit slots: a build inheriting a casualty's id made the casualty look
  alive — 8,311 phantom divergences from one stale mapping.
- Fog vision wrappers: AWBW gives the seat that could not see an action an
  *empty string*, not null, so "first non-null" picked the blind seat's blank.
- Mid-turn HP resynced from displayed HP, re-simulating later attacks from a
  baseline up to a point too high.
- Engine: repair rounding up to the display step; Sami's capture rate and
  transport movement; Rachel's repair bonus.
