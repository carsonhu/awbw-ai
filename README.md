# awbw-ai

RL experiments for [Advance Wars by Web](https://awbw.amarriner.com/), starting
with a Rust rules engine built for fast self-play.

## Layout

- `crates/awbw-engine` — the rules engine. `types.rs` (enums), `data.rs`
  (generated tables), `combat.rs` (the exact AWBW damage formula).
- `data/` — parsed game data (`units.json`, `terrain_chart.json`,
  `terrain_ids.json`) and raw site downloads (`data/awbw-site/`).
- `tools/` — Python scripts that regenerate everything:
  - `parse_charts.py`: `units.php` / `terrain.php` HTML → JSON
  - `gen_terrain_ids.py`: RizeBot's terrain table (from the AWBW DB dump) → JSON
  - `gen_tables.py`: all JSON → `crates/awbw-engine/src/data.rs`

## Data provenance

| Data | Source |
|------|--------|
| Damage table | `awbw.amarriner.com/js/damage_inc.json` (the site's own file, verbatim) |
| Unit stats | `units.php` chart page |
| Movement costs / terrain defense | `terrain.php` chart page (Clear/Rain/Snow) |
| Terrain id → kind (196 ids) | AWBW DB dump via [RizeBot](https://github.com/soul4rent/UnofficialAWBWRizeBot)'s generated `terrain-table.ts` |
| Damage formula | AWBW's server engine (`helper/fire.rs`), as documented line-by-line in RizeBot's `damage.ts` port |

Formula notes that differ from cartridge AW2: luck is +0..9% (inclusive), damage
is computed to one decimal then truncated, air units and pipe seams get zero
terrain stars, displayed HP `ceil(hp/10)` feeds both attack scaling and terrain
stars.

## Verification against real games

`F:\awbw\awbw-replay-parser\awbw replays` holds ~300k replay zips. Each is two
gzip members: a PHP-serialized game state per *turn*, and the matching orders
for that turn. `state[i]` is the board before `state[i]`'s orders, so
`state[i+1]` is ground truth for replaying them.

`tools/prepare_replay.py` normalizes a replay (plus its map, fetched from
AWBW's `api/map/map_info.php` and cached in `data/maps/`) into flat JSON, and
`verify-replays` replays it:

```
python tools/prepare_replay.py --glob '<...>\*\*STD*.zip' --limit 400
cargo run --release -p awbw-replay -- data/prepared
```

Each turn is an independent test case — load the snapshot, apply the recorded
orders, diff against the next snapshot — so one wrong rule shows up only on the
turns that exercise it instead of poisoning a whole game. Combat luck (0-9%)
can't be reproduced, so damage is checked as a *range* the record must fall
inside, and HP is resynced from the record afterwards.

### Results

The headline number has to be split, because CO powers and fog are unmodelled
**by design** — holding the engine to games that use them measures features that
were deliberately left out, not correctness. `--no-fog` and `--no-powers` do
the splitting.

| subset | games | exact | assertions | agreement |
|---|---|---|---|---|
| no powers, no fog | 127 | **108 (85%)** | 147k | **99.979%** |
| powers used | 239 | 2 | 721k | 98.81% |

The power figure is the cost of not modelling powers, and it is the only large
divergence left. Everything else is down to 31 scattered cases across 1,916
turns: 13 funds, 13 unit HP, and 5 stragglers.

### Bugs this found

Worth recording, because several were in the harness rather than the engine and
would otherwise have read as engine faults:

- **Fuel charged along the wrong path.** AWBW charges the route the player
  clicked; the harness charged the engine's cheapest path, so every detour
  looked like a fuel bug.
- **Orders paired to snapshots by line index.** Some replays are truncated, so
  one player's whole turn got attributed to their opponent. Now matched on
  (player, day).
- **Recycled unit slots.** The engine reuses a destroyed unit's slot, so a build
  later in the same turn inherited a casualty's id and the casualty read as a
  survivor: 8,311 phantom divergences from one stale mapping.
- **Fog vision wrappers.** AWBW keys some payloads by player and gives the seat
  that could not see the action an *empty string*, not null. Picking the first
  non-null value selected the blind seat's blank and silently dropped the order.
- **Display-HP resync.** Combat payloads report whole displayed HP, so a unit
  attacked twice in one turn was re-simulated from a baseline up to a point too
  high. Damage is now checked against the whole band the defender could be in.
- **Repair rounded up to the display step** (engine): a unit at 5.5 HP repaired
  to 8.0 instead of 7.5.

Competitive AWBW almost never uses ability-free COs — Max alone is 209 of 762
seats in the sample and Andy just 38, with no Andy-vs-Andy games at all — so
there is no "vanilla CO" subset to hide behind, and day-to-day abilities had to
be modelled before the corpus could say anything about combat.

## Action space

A turn is a variable-length sequence of single orders ending in `EndTurn`, so
one environment step is one order rather than a whole turn. Two enumeration
paths exist:

- `legal_actions_into` — every legal order for every unit. What a flat policy
  or a search needs, but it costs one reachability search *per unit per step*.
- `legal_actions_for(unit)` — one unit's orders, for a factorized policy that
  picks which unit acts first and what it does second. One search per step.

Both are exercised against each other in tests. Random self-play throughput on
a 15x15 map, 30-day games, single core:

| path | branching | micro-steps/sec |
|------|-----------|-----------------|
| flat | ~204 | ~54k |
| factorized | ~26 | ~179k |

## Roadmap

1. ~~Data capture + combat formula~~ (done)
2. ~~Game state, movement/pathfinding, core action set~~ (done: move, attack,
   capture, build, load/unload, join, supply, turn bookkeeping, win conditions)
3. ~~Replay-differential verification harness~~ (done)
4. ~~CO day-to-day abilities~~ (done: 99.98% agreement on power-free games)
5. ~~Fog of war~~ (done: 99.4% agreement on per-tile visibility)
6. CO powers, missile silos, pipe seams
7. Baseline bots + internal Elo ladder
8. PyO3 bindings, Gym-style env, PPO self-play

## COs

Day-to-day abilities are generated from the AWBW-Replay-Player's `COs.json` by
`tools/gen_cos.py` into `co_data.rs`: per-unit attack, defence and range
deltas, build-cost multipliers, terrain-conditional bonuses (Kindle on
properties, Koal on roads, Jake on plains, Lash per terrain star), Sasha's
property income and Eagle's air fuel saving.

**The agent does not use any of this.** Self-play runs on the ability-free CO
with powers off, so `CoData::VANILLA` is a constant and the CO layer is inert.
COs exist to make the replay corpus usable as a correctness signal, and later
so an agent facing a human can predict their damage correctly.

Four abilities COs.json omits are hand-entered from
[AWBW's own CO page](https://awbw.amarriner.com/co.php) (mirrored at
`data/awbw-site/co.php.html`), which is the authoritative prose description:
Sami's 1.5x capture and transport movement, Rachel's extra repair point, and
Lash's per-terrain-star attack. Each was pinpointed by the harness first — the
Sami capture bonus, for instance, showed up as every full-health infantry
leaving 5 capture points instead of 10.

Not modelled: **CO powers** (they change stats mid-turn, and the agent will
never use them), Olaf's weather remap, Sonja's fog effects, and Javier's
defence-against-indirects. The luck-range COs (Nell, Flak, Jugger) *are*
implemented but are **unverified** — they are banned in Global League play, so
no game in the corpus exercises them.

## Fog of war

Sight is Manhattan distance from each unit, extended by the terrain it stands
on (mountains add three, and aircraft get nothing from the ground below).
Adjacent tiles *pierce*: cover and concealment fail at arm's length. Further
out, woods and reefs hide ground and sea units, as does diving a sub or hiding
a stealth, while aircraft stay visible because they fly above cover. Owned
properties watch their own tile.

In play this means an enemy you cannot see does not block the route you plan —
walking into one halts you on the tile before it (`ActionReport::ambushed`) —
and you cannot fire on what you have not found.

### How it was checked

Fog replays store the *full* board in their snapshots, so those cannot test
vision. The move records can: AWBW writes one path per player, and the
opposing player's copy flags every step with whether they could see the unit
standing there. That is a per-tile statement of what the defender's fog
allowed.

**99.39% agreement over 6,035 judged path steps**, with 6 cases where the
engine saw too little and 31 where it saw too much. Almost all of the latter
fall on a single turn — the one where Drake fires Typhoon, which brings rain
and shortens sight. Powers are unmodelled, so that is the expected cost. The
6 remaining are unexplained, at 0.1% of judgements.

Two things the corpus settled, and one it got wrong:

- A defender's sight **shrinks as its units die mid-turn**. Computing the view
  once from the opening snapshot made the engine look far too sharp; it was the
  checker at fault, not the engine.
- **Concealment is re-tested at every step of a move**, not only where the unit
  stops. Exempting the tiles a unit merely passes through cost four points of
  agreement, so AWBW really does re-hide a mover behind each cover tile it
  crosses.
- **Reefs do conceal**, per [the wiki](https://awbw.fandom.com/wiki/Reefs).
  Six observations in this corpus pointed the other way and briefly talked the
  engine out of the rule; the documented behaviour is authoritative and a
  handful of unexplained samples is not. When the wiki and an inference from a
  thin sample disagree, the wiki wins.

## Rules not yet implemented

CO powers, missile silos, pipe-seam destruction, teleporters, weather changing
mid-game, and Black Bomb detonation.
