# Architecture

## Crates

`crates/awbw-engine` — the rules engine, no I/O and no dependencies.

| module | holds |
|---|---|
| `types.rs` | Core enums. Discriminants are table indices, so their order must match the generators. |
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

`crates/awbw-replay` — verification: `verify-replays` diffs recorded games,
`check-fog` checks visibility. See `verification.md`.

`crates/awbw-bots` — baselines and the arena, to rate a policy in absolute terms
(self-play Elo is self-referential). `greedy` scores every legal order one ply
deep, `capturer` is the same with combat off, `random` is the floor; `arena`
runs a round robin on a `Board`, which is a real AWBW map by default and a
synthetic symmetric one on request (`arena --show-map`).

`crates/awbw-py` — a batched Python environment, three classes. `VecEnv` steps
games in lockstep, a batch per call, sampling the four heads autoregressively:
`source_mask`, `dest_mask(sources)`, `kind_mask`, `param_mask`, `step`.
`TeacherEnv` and `ReplayTeacher` both answer `observe_into` / `act` with the
order a teacher chose — the scripted bots for one, recorded humans for the
other. Not in the default build; see `workflow.md`.

## State

`GameState` is cheap to clone: the map is an `Arc`, everything else flat vectors
of `Copy` records. Units live in a slot vector and **slots are recycled** when a
unit dies — anything holding an external id must track which slot it owns now,
or a build silently inherits a casualty's identity.

## Action space

A turn is a variable-length sequence of single orders ending in `EndTurn`, so
one environment step is one order. `Action` covers move, move-and-attack,
capture, build, load, unload, join and supply. Two enumeration paths:

- `legal_actions_into` — every legal order for every unit. What a flat policy or
  a search needs; costs one reachability search *per unit per step*.
- `legal_actions_for(unit)` — one unit's orders, for a factorized policy that
  picks which unit acts and then what it does. One search per step.

## Encoding for RL

`encoding.rs` turns positions into tensors and tensors back into orders.

**Observations** are written from the moving player's side — ownership channels
are *mine* and *theirs*, not seat 0 and seat 1 — so one policy plays either seat.
Under fog only what that player sees is written, so it is exactly what the agent
may act on. 64 planes plus 19 globals; layout in `plane::`. Two planes carry the
acting CO's per-unit attack and defence, and the globals carry each side's build
cost, capture rate, property income and power charge — so a policy trained on
mixed-CO human games can tell whose numbers it is looking at.

**Actions** are four masked choices — `source -> dest -> kind -> param` —
rather than one flat index, whose product is enormous and almost all illegal.
`source` is a tile (a unit or a production property) plus one index meaning
end-turn; `param` carries the attack target, the unit type to build, or which
passenger to drop where. Three of the four are board-shaped, which is what a
convolutional policy emits anyway.

`ActionMasks` is **staged**: the source mask scans units and properties, and
only the chosen tile's orders get enumerated — building them from the full
action set costs a third of throughput. Masks come from encoding real orders, so
they cannot drift from the rules; tests pin the staged path to exactly what flat
enumeration finds, and every masked path to something `check` accepts.

Random self-play, 15x15, 30-day games, one core (`--example selfplay_bench`):

| path | branching | micro-steps/sec |
|---|---|---|
| flat | ~204 | ~54k |
| factorized | ~26 | ~172k |
| factorized + observation + masks | ~28 | ~130k |
| the same, batched through Python | ~28 | ~144k |

## Fog

`Vision` holds two grids: *lit* tiles and *piercing* tiles (adjacent to a
watcher, where cover fails). Sight is Manhattan distance plus the terrain bonus
underfoot. An enemy the mover cannot see does not block the route it plans;
walking into one halts the unit on the tile before (`ActionReport::ambushed`),
and you cannot fire on what you have not found. `Engine` keeps the moving
player's view and refreshes it after every action; without fog it is simply
everything, so the cost is negligible.
