# Architecture

## Crates

`crates/awbw-engine` — the rules engine, no I/O and no dependencies.

| module | holds |
|---|---|
| `types.rs` | `UnitType`, `TerrainKind`, `MoveType`, `Weather`. Discriminants are table indices, so their order must match the generators. |
| `data.rs` | **Generated.** Damage matrix, unit stats, movement costs, terrain ids. |
| `co_data.rs` | **Generated.** CO day-to-day abilities. |
| `map.rs` | `Pos`, `Map`. Static terrain, shared behind an `Arc`. |
| `state.rs` | `GameState`: units, property ownership, funds, turn bookkeeping, win conditions. |
| `movement.rs` | `Reach`: bucket-queue reachability and paths. |
| `vision.rs` | `Vision`: fog, cover, concealment. |
| `combat.rs` | AWBW's damage formula and CO modifiers. |
| `actions.rs` | `Engine`: the action set, legality, application, enumeration. |
| `rng.rs` | Seeded xorshift, so games are reproducible. |

`crates/awbw-replay` — verification. `verify-replays` diffs recorded games
against the engine; `check-fog` checks visibility. See `verification.md`.

## State

`GameState` is cheap to clone: the map is an `Arc`, everything else is a flat
vector of `Copy` records. Units live in a slot vector and **slots are recycled**
when a unit dies — anything holding an external id must track which slot it
currently owns, or a build will silently inherit a casualty's identity.

## Action space

A turn is a variable-length sequence of single orders ending in `EndTurn`, so
one environment step is one order. `Action` covers move, move-and-attack,
capture, build, load, unload, join and supply.

Two enumeration paths:

- `legal_actions_into` — every legal order for every unit. What a flat policy or
  a search needs; costs one reachability search *per unit per step*.
- `legal_actions_for(unit)` — one unit's orders, for a factorized policy that
  picks which unit acts and then what it does. One search per step.

Random self-play, 15x15 map, 30-day games, single core:

| path | branching | micro-steps/sec |
|---|---|---|
| flat | ~204 | ~54k |
| factorized | ~26 | ~179k |

Rerun with `cargo run --release --example selfplay_bench`.

## Fog

`Vision` holds two grids: *lit* tiles and *piercing* tiles (adjacent to a
watcher, where cover fails). Sight is Manhattan distance plus the terrain
bonus underfoot. Under fog an enemy the mover cannot see does not block the
route it plans; walking into one halts the unit on the tile before
(`ActionReport::ambushed`), and you cannot fire on what you have not found.

`Engine` keeps the moving player's view and refreshes it after every action.
Without fog the view is simply everything, so the cost is negligible.
