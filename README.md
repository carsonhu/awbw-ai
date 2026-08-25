# awbw-ai

A Rust rules engine for [Advance Wars by Web](https://awbw.amarriner.com/),
built to run reinforcement-learning self-play, with the engine's correctness
measured against thousands of recorded human games.

## Where it stands

- **Rules verified at 99.979%** against 127 real games on the ruleset the agent
  trains on, with 108 of them reproduced move-for-move. Fog visibility agrees
  with 99.39% of 6,035 per-tile judgements.
- **144k env-steps/sec/core** through the batched Python environment, with
  observations and legality masks included.
- CO day-to-day abilities and fog of war are modelled; CO powers, silos and
  pipe seams are not, on purpose.

## Quickstart

```
cargo test
cargo run --release --example selfplay_bench
# verify against recorded games
python tools/prepare_replay.py --glob '<replays>\*\*STD*.zip' --limit 400
cargo run --release -p awbw-replay -- data/prepared --no-fog
```

## Layout

- `crates/awbw-engine` — the engine: rules, combat, movement, fog.
- `crates/awbw-replay` — differential verification against recorded games.
- `tools/` — Python that generates the data tables and normalizes replays.
  Generated Rust is never edited by hand.
- `data/` — game data and raw copies of AWBW's own chart pages.

## Docs

| file | contents |
|---|---|
| [docs/architecture.md](docs/architecture.md) | crate layout, state, action space, fog |
| [docs/rules.md](docs/rules.md) | what is and is not modelled, and the sources |
| [docs/verification.md](docs/verification.md) | the replay harness, and what it has caught |
| [docs/decisions.md](docs/decisions.md) | settled questions, with reasons |
| [docs/workflow.md](docs/workflow.md) | regenerating data, preparing replays |

## Roadmap

Done: game data, combat formula, state, movement, the action set,
replay-differential verification, CO day-to-day abilities, fog of war.

Done too: RL observation and action encoding, baseline bots and an arena, and
a batched Python environment. Next: PPO self-play.

## Credit

Rules and data come from AWBW itself and its [wiki](https://awbw.fandom.com/),
with [RizeBot](https://github.com/soul4rent/UnofficialAWBWRizeBot),
[DefendPeace](https://github.com/ThislsAUsername/DefendPeace) and
[AWBW-Replay-Player](https://github.com/DeamonHunter/AWBW-Replay-Player) as
references. Unaffiliated with AWBW.
