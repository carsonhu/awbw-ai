# Workflow

## Build and test

```
cargo test                                   # engine + harness unit tests
cargo run --release --example selfplay_bench # throughput
python tools/check_docs.py                   # doc size budgets
```

Anything touching rules should also be run through the replay harness — see
`verification.md`.

## Regenerating game data

Generated Rust is never edited by hand. Each generator reads from `data/` and
writes one file:

```
python tools/parse_charts.py      # units.php / terrain.php HTML -> data/*.json
python tools/gen_terrain_ids.py   # AWBW terrain table          -> data/terrain_ids.json
python tools/gen_tables.py        # all of the above            -> crates/awbw-engine/src/data.rs
python tools/gen_cos.py           # COs.json + co.php           -> crates/awbw-engine/src/co_data.rs
```

`parse_charts.py` and `gen_terrain_ids.py` read local copies in
`data/awbw-site/`, so they work offline. Re-download only to pick up a change
on the site.

Adding a CO ability the source data omits: put it in the `MANUAL` table in
`gen_cos.py` with the wording from `co.php` in a comment, then regenerate.

## The Python environment

Not in the default workspace build: it needs a Python 3.8+ toolchain and
`cargo test` should not. Build against a modern interpreter — the `python` on
PATH here is an Anaconda 3.7 that PyTorch no longer supports:

```
PYO3_PYTHON=".../Python312/python.exe" cargo build --release -p awbw-py
cp target/release/awbw.dll python/awbw.pyd     # awbw.so on Linux
py -3.12 python/smoke_test.py
```

The smoke test is the contract check: masks non-empty, every sampled order
legal, episodes restarting. Keep observations in one reused buffer refilled by
`observe_into`; allocating one per step costs four fifths of the throughput.

## Preparing replays

`data/prepared/` and `data/maps/` are gitignored and rebuilt on demand:

```
python tools/prepare_replay.py --glob '<replays>\*\*STD*.zip' --limit 400
```

Maps are fetched once per id and cached. Filenames are not reliable — some
games named `STD` are fog games — so filter on the `fog` field, not the name.

The `probe_*.py` scripts are one-off diagnostics for the replay format, kept
because it is undocumented and easy to re-misread.

## Docs

`python tools/check_docs.py` enforces a line budget per file. `CLAUDE.md` is
loaded into every session, so it has the tightest one.

Prefer editing an existing doc to adding a new one. When something is settled
after real investigation, add a short entry to `decisions.md` — that file exists
to stop the same question being re-opened.
