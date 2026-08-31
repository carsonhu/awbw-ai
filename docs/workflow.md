# Workflow

> Regenerating game data, preparing replays, training a policy, running checks.

## Build and test

```
cargo test                                   # engine + harness unit tests
cargo run --release --example selfplay_bench # throughput
python tools/docs.py                         # doc budgets and index
```

Anything touching rules also goes through the replay harness — see `verification.md`.

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
py -3.12 python/bc.py --teacher human --steps 15000 --channels 96 --blocks 8   --threat-planes --norm group --pool-bias --value-pool meanmax --pop-weight 100   --value-outcomes data/game-meta-119544.json --out checkpoints/bc-net2.pt
py -3.12 python/ppo.py --init checkpoints/bc-net2.pt   # improve it by playing
py -3.12 python/panel.py --checkpoint <ckpt>          # the fixed panel
py -3.12 python/play_diag.py --checkpoint <ckpt> --baseline  # what it builds
py -3.12 python/order_diag.py                         # ordering vs judgement
```

The rung of record, under every logged run: `--threat-planes --opponent greedy
--co Adder --turn-discount --steps 256 --lam 0.99 --decide-cap`. Experiments are
run as seed groups through `tools/grid.sh`, never as single runs (`plan.md`).

Read `kl` and `clip`, never entropy. Rate at 1.0 (0.3 flatters a clone
threefold) through `panel.py`, since a ladder's head-to-head only proves it
beats itself; `--frozen-init` takes a comma-separated list to make self-play a
league. `--recalibrate` defaults to 0 and belongs there.

Two PPO defaults are Atari's units. `--turn-discount` discounts once per *turn*:
`1/(1 - gamma*lam)` is 19 orders and a turn is 17, so credit otherwise never
crosses one, and `--steps 256` outruns that horizon. `--potential worth` counts
money and unspent property income, which `material` cannot — it doubles the
reward scale, so halve `--shaping`. Watch `cut`: those games carry no result.

The reward cannot see *composition* — a unit is priced at cost times HP — so
PPO drifts off the human build mix and off human power timing while improving
every engagement number. `--anchor <clone> --anchor-kl w` pulls it back by KL,
`--pop-force 0` opens the turn under a power; judge both through `play_diag.py`
against the corpus, not by the score.

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

## Watching a policy play

```
py -3.12 python/record_games.py --checkpoint checkpoints/ppo.pt --games 2
py -3.12 python/record_games.py --checkpoint checkpoints/ppo.pt \
    --versus checkpoints/bc-scaled.pt   # two nets, seat against seat
```

Writes real AWBW replay files to `replays/` (gitignored), which open in AWBW's
own viewers — a win rate says a policy improved, only watching says what it
learned. Round-trip a written replay through `prepare_replay.py` and the
verifier to check it is faithful.

## Docs

`docs/README.md` is the index, and states the rules each tier is kept under.

```
python tools/docs.py index        # rebuild the index from each doc's `> ` hook
python tools/docs.py find fog     # which doc talks about this
```
