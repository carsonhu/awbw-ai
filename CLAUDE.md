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

`docs/README.md` indexes them all and is generated, so it is never stale. The
shape: `architecture` `rules` `verification` `workflow` are reference and kept
current, `decisions.md` is settled questions — read it before re-litigating one
— and `docs/log/` holds dated, immutable records of what each experiment
measured.

`python tools/docs.py` checks budgets, conventions and the index; `index`
rewrites it; `find <term>` locates a doc. Prefer editing a reference doc to
adding one. A dated result is a log entry, not a new doc.
