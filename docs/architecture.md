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
| `encoding.rs` | Observations and the action codec for RL. |
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

## Encoding for RL

`encoding.rs` turns positions into tensors and tensors back into orders.

**Observations** are written from the moving player's side — ownership channels
are *mine* and *theirs*, never seat 0 and seat 1 — so one policy plays either
seat. Under fog only what that player can see is written, so the observation is
exactly the information the agent may act on. 62 planes plus 11 globals; see
`plane::` for the layout.

**Actions** are four masked choices — `source -> dest -> kind -> param` —
rather than one flat index, since the product of those is enormous and almost
all illegal. `source` is a tile (a unit, or a production property) plus one
index meaning end-turn; `param` carries the attack target, the unit type to
build, or which passenger to drop where. Three of the four are board-shaped,
which is what a convolutional policy emits naturally.

`ActionMasks` is **staged**: the source mask scans units and properties, and
only the chosen tile's orders are enumerated. Building masks from the full
action set instead costs a third of throughput. Masks are derived by encoding
real orders, so they cannot drift from the rules; tests assert the staged path
reaches exactly the orders the flat enumeration finds, and that every masked
path decodes to something `check` accepts.

Random self-play, 15x15 map, 30-day games, single core
(`cargo run --release --example selfplay_bench`):

| path | branching | micro-steps/sec |
|---|---|---|
| flat | ~204 | ~54k |
| factorized | ~26 | ~172k |
| factorized + observation + masks | ~28 | ~130k |

## Fog

`Vision` holds two grids: *lit* tiles and *piercing* tiles (adjacent to a
watcher, where cover fails). Sight is Manhattan distance plus the terrain
bonus underfoot. Under fog an enemy the mover cannot see does not block the
route it plans; walking into one halts the unit on the tile before
(`ActionReport::ambushed`), and you cannot fire on what you have not found.

`Engine` keeps the moving player's view and refreshes it after every action.
Without fog the view is simply everything, so the cost is negligible.
