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

## Verification plan

`F:\awbw\awbw-replay-parser\awbw replays` holds ~300k replay zips (each: a
gzipped PHP-serialized per-turn state file + a gzipped action log). The engine
will be validated by replaying recorded actions and diffing predicted state
against recorded state, turn by turn. ~28k are filename-tagged standard 1v1
("GL STD"); FOG/HF variants tagged likewise; the rest need content inspection.

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
3. Fog of war, CO powers, missile silos, pipe seams
4. Replay-differential verification harness
5. Baseline bots + internal Elo ladder
6. PyO3 bindings, Gym-style env, PPO self-play (restricted ruleset first:
   1v1, no fog, no COs)

## Rules not yet implemented

Fog of war and vision, CO powers and CO-specific stats, missile silos,
pipe-seam destruction, teleporters, weather changing mid-game, and Black Bomb
detonation. Turn-order details flagged for the replay harness to confirm:
the exact ordering of income vs. repair vs. fuel upkeep, and repair funding
when a player cannot afford a full heal.
