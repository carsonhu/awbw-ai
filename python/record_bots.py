"""Plays two scripted bots against each other and writes the games out.

`record_games.py` records a checkpoint; this records the baselines, which is
what you want when the question is about the *opponent* rather than the policy
— why `jakeman` drops the games it drops, say.

Games can be filtered by result, because the interesting ones are usually the
few that went the other way:

    py -3.12 python/record_bots.py --bot jakeman --opponent greedy --keep loss
    py -3.12 python/record_bots.py --bot jakeman --opponent greedy --games 4
"""

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "python"))
sys.path.insert(0, str(ROOT / "tools"))

import awbw  # noqa: E402
import write_replay  # noqa: E402

BOTS = ["greedy", "jakeman", "capturer", "random"]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bot", default="jakeman", choices=BOTS)
    parser.add_argument("--opponent", default="greedy", choices=BOTS)
    parser.add_argument("--games", type=int, default=2)
    parser.add_argument("--envs", type=int, default=16)
    parser.add_argument("--max-day", type=int, default=60)
    parser.add_argument("--seed", type=int, default=5)
    parser.add_argument("--map-id", type=int, default=119544)
    parser.add_argument("--keep", default="any",
                        choices=["any", "win", "loss", "draw"],
                        help="only write games with this result, for --bot")
    parser.add_argument("--limit", type=int, default=4000,
                        help="give up after this many batched orders")
    parser.add_argument("-o", "--out", default="replays")
    parser.add_argument("--users", default=None)
    parser.add_argument("--game-id", type=int, default=980000)
    args = parser.parse_args()

    env = awbw.TeacherEnv(
        num_envs=args.envs, teacher=args.bot, opponent=args.opponent,
        seed=args.seed, max_day=args.max_day, record=True,
    )
    print(f"{args.bot} vs {args.opponent} on {env.map_name}, "
          f"looking for {args.games} game(s) ending in '{args.keep}'")

    users = [int(u) for u in args.users.split(",")] if args.users else None
    seats = env.agent_seat()
    written, seen = [], {"win": 0, "loss": 0, "draw": 0}
    for _ in range(args.limit):
        if len(written) >= args.games:
            break
        env.act()
        for slot in range(args.envs):
            if len(written) >= args.games:
                break
            log = env.take_replay(slot)
            if log is None:
                continue
            game = json.loads(log)
            winner = game["outcome"]["winner"]
            outcome = ("draw" if winner is None
                       else "win" if winner == int(seats[slot]) else "loss")
            seen[outcome] += 1
            if args.keep != "any" and outcome != args.keep:
                continue
            game_id = args.game_id + len(written)
            stem = (f"{args.bot}-vs-{args.opponent}-{outcome}"
                    f"-d{game['days']}-{game_id}")
            path = write_replay.Writer(
                game, game_id, args.map_id,
                f"{args.bot} vs {args.opponent} ({outcome})", users,
            ).write(args.out, stem)
            written.append(path)
            print(f"  {path}  {game['days']} days, {len(game['turns'])} turns")

    total = sum(seen.values())
    print(f"\n{len(written)} written; {total} games played "
          f"({seen['win']} won, {seen['loss']} lost, {seen['draw']} drawn "
          f"by {args.bot})")
    if len(written) < args.games:
        print("Ran out of orders before filling the quota; raise --limit.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
