# awbw-ai

Rust rules engine for Advance Wars by Web, built to run RL self-play. See
`docs/` for detail — this file is loaded every session, so it stays short.

## Where things are

- `crates/awbw-engine` — the engine. Rules, combat, movement, fog.
- `crates/awbw-replay` — verifies the engine against recorded games.
- `crates/awbw-py` — the batched environment Python trains against.
- `python/` — the network, the cloning loop, and rating a checkpoint by play.
- `tools/` — Python: generates data tables, normalizes replays.
- `data/` — game data and cached maps. `data/prepared/` and `data/maps/` are
  gitignored and rebuilt on demand.

## Rules of the road

- **Never hand-edit generated files.** `data.rs` and `co_data.rs` come from
  `tools/gen_tables.py` and `tools/gen_cos.py`. Edit the generator.
- **The wiki is authoritative for game rules**, then AWBW's own pages
  (`co.php`, `terrain.php`, mirrored in `data/awbw-site/`). The replay corpus
  checks the *implementation*, and is a poor arbiter of what a rule should be —
  it has already talked the engine out of one correct rule. See
  `docs/decisions.md` before re-deciding anything.
- **Powers, silos, pipe seams and mid-game weather are unimplemented on
  purpose.** Divergences involving them are expected, not bugs.
  `docs/rules.md` has the full list.
- Run `cargo test` and, for anything touching rules,
  `cargo run --release -p awbw-replay -- data/prepared --no-fog`.

## Docs

| file | when to read |
|---|---|
| `docs/architecture.md` | crate layout, key types, action space |
| `docs/rules.md` | what AWBW rules are and are not modelled |
| `docs/verification.md` | how the replay harness works, current numbers |
| `docs/decisions.md` | questions already settled — read before re-litigating |
| `docs/workflow.md` | regenerating data, preparing replays, running checks |

Keep every doc under its budget; `python tools/check_docs.py` enforces it.
Prefer editing an existing doc over adding one.
