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

**Current: 98.4% agreement** over 381 games / 10,386 turns / 905k assertions,
with 80 games reproduced exactly.

### What the divergences say

Ranked by count, and what they actually mean:

| divergence | cause |
|---|---|
| `damage-range`, `move-over-budget`, `move-unreachable` | **CO powers**, which the engine does not model. A power changes attack, defence and movement for one turn — Max Force is the B-Copter moving 7 tiles on 6 movement. |
| `unit-hp`, `unit-position` | Mostly downstream of the same. |
| `capture-progress` off by one | Sami captures at 1.5x, which the CO table does not encode. |
| `funds`, `build-illegal` | Residual power effects (Hachi's power halves unit cost) and repair spend. |

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
| flat | ~204 | ~38k |
| factorized | ~26 | ~121k |

## Roadmap

1. ~~Data capture + combat formula~~ (done)
2. ~~Game state, movement/pathfinding, core action set~~ (done: move, attack,
   capture, build, load/unload, join, supply, turn bookkeeping, win conditions)
3. ~~Replay-differential verification harness~~ (done)
4. ~~CO day-to-day abilities~~ (done: 98.4% agreement)
5. CO powers, or explicitly excluding power-affected turns from strict checks
6. Fog of war, missile silos, pipe seams
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

Not modelled: **CO powers** (they change stats mid-turn, and the agent will
never use them), Olaf's weather remap, Sonja's fog effects, Javier's
defence-against-indirects, and Sami's capture rate. The luck-range COs (Nell,
Flak, Jugger) *are* implemented but are **unverified** — they are banned in
Global League play, so no game in the corpus exercises them.

## Rules not yet implemented

Fog of war and vision, CO powers and CO-specific stats, missile silos,
pipe-seam destruction, teleporters, weather changing mid-game, and Black Bomb
detonation. Turn-order details flagged for the replay harness to confirm:
the exact ordering of income vs. repair vs. fuel upkeep, and repair funding
when a player cannot afford a full heal.
