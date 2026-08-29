"""Play against a checkpoint in the browser, under the engine's own rules.

The four-stage mask API is already a click interface: the source mask says
which of your units light up, the dest mask where the clicked one may go, the
kind mask what it may do there, the param mask at whom. A human clicking
through those stages cannot make an illegal move, because the masks are the
engine's legality -- the same guarantee the policy plays under, on the same
board the policy trained on. DefendPeace was considered and rejected for
exactly that reason: it is a rules variant, and the agent would be playing a
different game than it learned.

The agent answers with its whole turn at once; the recorder runs throughout,
so a finished match writes a real AWBW replay reviewable in the site's own
viewer.

    py -3.12 python/play_local.py --checkpoint checkpoints/bc-net2.pt
    # then open http://localhost:8642
"""

import argparse
import json
import sys
import threading
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "python"))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import awbw  # noqa: E402
import evaluate  # noqa: E402

PAGE = (ROOT / "python" / "play_local.html").read_text(encoding="utf-8")
SPRITES = ROOT / "data" / "sprites"


def terrain_payload(env):
    """The static board plus everything the renderer needs to draw it with
    AWBW's own art: per-tile sprite names straight from the map's terrain
    ids, and the manifest for buildings, units and badges. Falls back to
    the flat-color renderer when sprites were never fetched."""
    payload = json.loads(env.terrain_json())
    manifest = SPRITES / "manifest.json"
    if manifest.exists():
        m = json.loads(manifest.read_text())
        ids = json.loads(
            (ROOT / "data" / "terrain_ids.json").read_text())
        game_map = json.loads(
            (ROOT / "data" / "maps" / "119544.json").read_text())
        grid = game_map["Terrain Map"]  # column-major: grid[x][y]
        payload["sprites"] = [
            [m["terrain"].get(str(grid[x][y])) for x in range(payload["width"])]
            for y in range(payload["height"])
        ]
        payload["manifest"] = m
    return payload


class Match:
    """One game: a human seat, an agent seat, and the engine between them.

    Everything runs on VecEnv's self-play path (opponent=None), so both
    seats submit orders through the same `step`; whose turn it is decides
    who is asked. A lock serialises the handler threads -- the env is one
    mutable game, and two overlapping requests would interleave its masks.
    """

    def __init__(self, args):
        self.lock = threading.Lock()
        self.args = args
        self.device = torch.device(
            "cuda" if torch.cuda.is_available() else "cpu")
        saved = torch.load(ROOT / args.checkpoint, map_location="cpu",
                           weights_only=True)
        threat = saved["config"]["planes"] > 64
        self.env = awbw.VecEnv(
            num_envs=1, seed=args.seed, max_day=args.max_day,
            co=args.co, threat=threat, record=True,
        )
        self.policy = evaluate.Net(
            evaluate.load(ROOT / args.checkpoint, self.env, self.device),
            self.device, args.temperature)
        self.rng = np.random.default_rng(args.seed)
        self.human = args.seat
        self.tiles = self.env.board_shape[0] * self.env.board_shape[1]
        self.obs = torch.empty((1, self.env.observation_size),
                               dtype=torch.float32)
        self.agent_log = []
        # The agent opens if the human sits second.
        self.run_agent()

    # -- helpers ---------------------------------------------------------

    def state(self):
        payload = json.loads(self.env.state_json(0))
        payload["human"] = self.human
        payload["agent_orders"] = self.agent_log
        payload["source_mask"] = (
            self.env.source_mask()[0].tolist()
            if payload["current"] == self.human and payload["winner"] is None
            else None)
        return payload

    def current(self):
        return int(self.env.current_player()[0])

    def submit(self, s, d, k, p):
        arr = lambda v: np.array([v], dtype=np.uint32)  # noqa: E731
        self.env.step(arr(s), arr(d), arr(k), arr(p))

    def describe(self, s, d, k, p, state_before):
        """One line of what the agent just did, readable in the sidebar."""
        kind = ["wait", "attack", "capture", "supply", "join", "load",
                "unload", "build"][k] if k < 8 else "?"
        w = self.env.board_shape[1]
        if s == self.tiles:
            return "ends the turn"
        if s > self.tiles:
            return "activates " + ("SCOP" if s == self.tiles + 2 else "COP")
        sx, sy = s % w, s // w
        dx, dy = d % w, d // w
        px, py = p % w, p // w
        unit = next((u["type"] for u in state_before["units"]
                     if u["x"] == sx and u["y"] == sy), "unit")
        line = f"{unit} ({sx},{sy})"
        if (dx, dy) != (sx, sy):
            line += f" -> ({dx},{dy})"
        if kind == "attack":
            line += f", attacks ({px},{py})"
        elif kind == "build":
            line += f": builds"
        elif kind != "wait":
            line += f", {kind}"
        return line

    def run_agent(self):
        """Plays the agent's seat until the turn comes back or the game ends."""
        self.agent_log = []
        guard = 0
        while (self.current() != self.human
               and json.loads(self.env.state_json(0))["winner"] is None
               and guard < 500):
            guard += 1
            before = json.loads(self.env.state_json(0))
            self.env.observe_into(self.obs.numpy())
            s, d, k, p = self.policy.choose(
                self.env, self.obs.to(self.device), self.rng)
            self.agent_log.append(
                self.describe(int(s[0]), int(d[0]), int(k[0]), int(p[0]),
                              before))
            self.env.step(s, d, k, p)

    # -- request handlers --------------------------------------------------

    def handle(self, route, body):
        with self.lock:
            if route == "state":
                return self.state()
            if self.current() != self.human:
                return {"error": "not your turn"}
            if route == "dests":
                mask = self.env.dest_mask(
                    np.array([body["s"]], dtype=np.uint32))[0]
                return {"mask": mask.tolist()}
            if route == "kinds":
                mask = self.env.kind_mask(
                    np.array([body["s"]], dtype=np.uint32),
                    np.array([body["d"]], dtype=np.uint32))[0]
                # One line per ask, so a wrong menu is diagnosable from the
                # console: what the engine thinks stands at source and dest.
                w = self.env.board_shape[1]
                st = json.loads(self.env.state_json(0))
                terr = json.loads(self.env.terrain_json())["terrain"]
                def at(i):
                    x, y = i % w, i // w
                    u = next((f"{v['type']}(p{v['player']})" for v in st["units"]
                              if v["x"] == x and v["y"] == y and not v["carried"]), "-")
                    b = next((f"{v['kind']}/own{v['owner']}/cap{v['capture']}"
                              for v in st["buildings"]
                              if v["x"] == x and v["y"] == y), terr[y][x])
                    return f"({x},{y}) {b} unit={u}"
                kinds = [k for k, v in enumerate(mask.tolist()) if v]
                print(f"[kinds] s={at(body['s'])}  d={at(body['d'])}"
                      f"  -> {kinds}", flush=True)
                return {"mask": mask.tolist()}
            if route == "params":
                mask = self.env.param_mask(
                    np.array([body["s"]], dtype=np.uint32),
                    np.array([body["d"]], dtype=np.uint32),
                    np.array([body["k"]], dtype=np.uint32))[0]
                return {"mask": mask.tolist()}
            if route == "preview":
                w = self.env.board_shape[1]
                to_xy = lambda i: (i % w, i // w)  # noqa: E731
                try:
                    return json.loads(self.env.damage_preview(
                        0, to_xy(body["s"]), to_xy(body["d"]),
                        to_xy(body["p"])))
                except ValueError:
                    return {"damage": None, "counter": None}
            if route == "dbg":
                print(f"[client] {body}", flush=True)
                return {"ok": True}
            if route == "act":
                self.submit(body["s"], body["d"], body["k"], body["p"])
                self.run_agent()
                return self.state()
            return {"error": f"unknown route {route}"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--checkpoint", default="checkpoints/bc-net2.pt")
    parser.add_argument("--co", default="Adder")
    parser.add_argument("--seat", type=int, default=0, choices=[0, 1],
                        help="which seat the human plays; 1 lets the agent open")
    parser.add_argument("--temperature", type=float, default=1.0)
    parser.add_argument("--max-day", type=int, default=60)
    parser.add_argument("--seed", type=int, default=None)
    parser.add_argument("--port", type=int, default=8642)
    args = parser.parse_args()
    if args.seed is None:
        args.seed = int.from_bytes(np.random.bytes(4)) % 100000

    match = Match(args)

    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def reply(self, payload, kind="application/json"):
            data = (payload if isinstance(payload, bytes)
                    else json.dumps(payload).encode())
            self.send_response(200)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(data)))
            # Without this Chrome heuristically caches the page, and a stale
            # client against a new server is undebuggable by screenshot.
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            if self.path in ("/", "/index.html"):
                return self.reply(PAGE.encode(), "text/html; charset=utf-8")
            if self.path == "/terrain":
                return self.reply(terrain_payload(match.env))
            if self.path.startswith("/sprites/"):
                name = Path(self.path).name
                target = SPRITES / name
                if target.is_file() and target.suffix == ".gif":
                    return self.reply(target.read_bytes(), "image/gif")
                return self.send_error(404)
            if self.path == "/state":
                return self.reply(match.handle("state", {}))
            self.send_error(404)

        def do_POST(self):
            length = int(self.headers.get("Content-Length", 0))
            body = json.loads(self.rfile.read(length) or b"{}")
            route = self.path.strip("/")
            self.reply(match.handle(route, body))

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    url = f"http://localhost:{args.port}"
    print(f"playing {args.checkpoint} as seat {1 - args.seat}, "
          f"you are seat {args.seat} ({'orange' if args.seat == 0 else 'blue'})")
    print(f"open {url}")
    threading.Timer(0.7, lambda: webbrowser.open(url)).start()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
