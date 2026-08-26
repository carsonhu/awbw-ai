# Workflow

> Regenerating game data, preparing replays, training a policy, running checks.

## Build and test

```
cargo test                                   # engine + harness unit tests
cargo run --release --example selfplay_bench # throughput
python tools/docs.py                         # doc budgets and index
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

`smoke_test.py` is the contract check for `VecEnv`; `replay_demo.py` is the same
for `ReplayTeacher` and needs `data/prepared`. Keep observations in one *pinned*
buffer refilled by `observe_into` — a batch is ten megabytes, and copying it from
pageable memory each step cost more than the engine did.

## Training and rating a policy

```
py -3.12 python/bc.py --teacher human --steps 15000  # clone the corpus
py -3.12 python/evaluate.py --temperature 1.0        # play it against greedy
py -3.12 python/evaluate.py --policy random          # the floor, for scale
py -3.12 python/ppo.py --init checkpoints/bc-scaled.pt  # fine-tune by playing
py -3.12 python/order_diag.py                        # ordering vs judgement
```

`--amp` is off by default and worth measuring first: without fp16 tensor cores it
is four times *slower*. In PPO read `kl` and `clip`, never entropy. Rate at 1.0:
0.3 flatters a clone threefold, and only 1.0 says what PPO starts from.

## Preparing replays

`data/prepared/` and `data/maps/` are gitignored and rebuilt on demand:

```
python tools/prepare_replay.py --glob '<replays>\**\*.zip' \
    --map-id 119544 --exclude-fog --workers 12
```

Filter on `--map-id`, never filenames: some games named `STD` are fog games, and
the archive sorts by tournament, so `--limit N` samples one cluster rather than
the corpus. `probe_*.py` are one-off format diagnostics, kept because the format
is undocumented and easy to re-misread.

## Docs

`docs/README.md` is the index, and states the rules each tier is kept under.

```
python tools/docs.py index        # rebuild the index from each doc's `> ` hook
python tools/docs.py find fog     # which doc talks about this
```
