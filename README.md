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

## Roadmap

1. ~~Data capture + combat formula~~ (done)
2. Game state, movement/pathfinding, full action set (move, attack, capture,
   build, load/unload, join, resupply, powers later), fog
3. Replay-differential verification harness
4. Baseline bots + internal Elo ladder
5. PyO3 bindings, Gym-style env, PPO self-play (restricted ruleset first:
   1v1, no fog, no COs)
