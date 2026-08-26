"""Plays a checkpoint and writes the games out as AWBW replay files.

A win rate says a policy got better; it does not say what it learned to do.
Watching a game does, and AWBW's own replay viewers are far better than anything
worth writing here -- so the games are written in AWBW's format and opened
there. Handy for the same checkpoint at different stages, which is what makes
the change legible.

    py -3.12 python/record_games.py --checkpoint checkpoints/ppo.pt
    py -3.12 python/record_games.py --checkpoint checkpoints/bc-scaled.pt --games 3
    py -3.12 python/record_games.py --checkpoint checkpoints/ppo.pt \
        --versus checkpoints/bc-scaled.pt          # two nets, seat against seat

Games are recorded from every slot in the batch, so a few envs fill the quota
faster than one played to the end.
"""

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "python"))
sys.path.insert(0, str(ROOT / "tools"))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import awbw  # noqa: E402
import evaluate  # noqa: E402
import write_replay  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", default="checkpoints/ppo.pt")
    parser.add_argument("--versus", default=None,
                        help="a second checkpoint for the other seat")
    parser.add_argument("--opponent", default="greedy",
                        choices=["greedy", "jakeman", "capturer", "random"])
    parser.add_argument("--games", type=int, default=2)
    parser.add_argument("--envs", type=int, default=2)
    parser.add_argument("--max-day", type=int, default=60)
    parser.add_argument("--temperature", type=float, default=1.0)
    parser.add_argument("--seed", type=int, default=17)
    parser.add_argument("--map-id", type=int, default=119544)
    parser.add_argument("-o", "--out", default="replays")
    # Shown as the game's title. Player *names* cannot be set: an AWBW
    # replay stores only a users_id and a viewer resolves it against the
    # site, so synthetic ids come up blank however they are labelled here.
    parser.add_argument("--name", default=None,
                        help="game title; defaults to '<policy> vs <opponent>'")
    parser.add_argument("--users", default=None,
                        help="comma-separated AWBW users_id, one per seat; "
                             "a viewer resolves player names from these")
    parser.add_argument("--game-id", type=int, default=990000,
                        help="first id; each game takes the next one")
    args = parser.parse_args()

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    rng = np.random.default_rng(args.seed)
    torch.manual_seed(args.seed)

    env = awbw.VecEnv(
        num_envs=args.envs, seed=args.seed, max_day=args.max_day,
        opponent=None if args.versus else args.opponent, record=True,
    )
    agent = evaluate.Net(
        evaluate.load(ROOT / args.checkpoint, env, device), device, args.temperature)
    if args.versus:
        other = evaluate.Net(
            evaluate.load(ROOT / args.versus, env, device), device, args.temperature)
        agent = evaluate.Duel(agent, other, env, device)

    label = Path(args.checkpoint).stem
    against = Path(args.versus).stem if args.versus else args.opponent
    print(f"{label} vs {against} on {env.map_name}, recording {args.games} games")

    obs = torch.empty((args.envs, env.observation_size), dtype=torch.float32,
                      pin_memory=device.type == "cuda")
    view = obs.numpy()
    written = []
    while len(written) < args.games:
        env.observe_into(view)
        s, d, k, p = agent.choose(env, obs.to(device, non_blocking=True), rng)
        env.step(s, d, k, p)
        for slot in range(args.envs):
            log = env.take_replay(slot)
            if log is None or len(written) >= args.games:
                continue
            game = json.loads(log)
            game_id = args.game_id + len(written)
            path = write_replay.Writer(
                game, game_id, args.map_id, args.name or f"{label} vs {against}",
                [int(u) for u in args.users.split(",")] if args.users else None,
            ).write(args.out)
            winner = game["outcome"]["winner"]
            seat = int(env.agent_seat()[slot])
            result = ("a draw" if winner is None
                      else f"seat {winner} won" + (" (ours)" if winner == seat else ""))
            written.append(path)
            print(f"  {path}  {game['days']} days, {len(game['turns'])} turns, {result}")

    print(f"\n{len(written)} replays in {Path(args.out).resolve()}")
    print("Open them in AWBW Replay Player, or upload to awbw.amarriner.com.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
