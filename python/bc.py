"""Behaviour cloning: learn to play by predicting the order a teacher gave.

Reinforcement learning cannot start here from scratch. Random self-play reaches
a real win zero times in forty games even at a 120-day cap, so the win signal is
not sparse but *absent*, and there is nothing for a policy gradient to climb.
Cloning a teacher is what makes the reward reachable at all; see
`docs/decisions.md`.

Two teachers, the same interface. `--teacher greedy` is the scripted bot: weak,
but unlimited and free. `--teacher human` is the replay corpus: much stronger
and strictly finite, about 125k usable orders. The natural curriculum is to
learn the shape of the action space from the first and the actual play from the
second, which is what `--init` is for.

Held-out games, never held-out *orders*: two orders from the same turn are
nearly the same position, so splitting by order would leave the validation set
memorised rather than held out.

    py -3.12 python/bc.py --teacher greedy --steps 3000
    py -3.12 python/bc.py --teacher human --steps 6000 --init checkpoints/greedy.pt
"""

import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402
import torch  # noqa: E402
import torch.nn.functional as F  # noqa: E402

import awbw  # noqa: E402
import net as netmod  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

# Steps between progress lines. Also the window the running numbers cover, since
# reading them back from the GPU is what forces a sync.
REPORT = 100


class Source:
    """A teacher, plus the buffers for reading it, behind one interface.

    `ReplayTeacher` can hand back a row with no game left in it; `TeacherEnv`
    never can. Both report a validity mask here so the training step does not
    have to care which it is talking to.
    """

    def __init__(self, env, batch: int, source_set: bool = False):
        self.env = env
        self.source_set = source_set
        # Pinned, and handed to Rust as a numpy view of the same memory. An
        # observation batch is ten megabytes; copying that from pageable memory
        # every step cost more than the engine did.
        self.staging = torch.empty(
            (batch, env.observation_size),
            dtype=torch.float32,
            pin_memory=torch.cuda.is_available(),
        )
        self.obs = self.staging.numpy()
        self.human = isinstance(env, awbw.ReplayTeacher)

    def next(self, device):
        self.env.observe_into(self.obs)
        # Read before `act`, which advances past the position it describes.
        targets = self.env.source_targets() if self.source_set else None
        ids = self.env.game_ids() if self.human else None
        movers = self.env.current_player() if self.human else None
        turns = self.env.turn_index() if self.human else None
        result = self.env.act()
        codes, valid = result if self.human else (result, None)
        obs = self.staging.to(device, non_blocking=True)
        codes = torch.from_numpy(codes.astype(np.int64)).to(device, non_blocking=True)
        if valid is None:
            keep = torch.ones(codes.shape[0], dtype=torch.bool, device=device)
        else:
            keep = torch.from_numpy(valid).to(device, non_blocking=True)
        if targets is not None:
            targets = torch.from_numpy(targets).to(device, non_blocking=True)
        return obs, codes, keep, targets, ids, movers, turns


def make_source(args, batch: int, validation: bool) -> Source:
    if args.teacher == "human":
        # Lookahead costs a second pass over every turn, so only the training
        # stream pays for it; validation always scores the exact label.
        wants_set = args.source_set > 0 and not validation
        env = awbw.ReplayTeacher(
            replay_dir=str(ROOT / "data" / "prepared"),
            num_envs=batch,
            map_name=args.map_name,
            seed=args.seed + (1 if validation else 0),
            holdout=args.holdout,
            validation=validation,
            lookahead=wants_set,
            threat=args.threat_planes,
        )
        return Source(env, batch, source_set=wants_set)
    else:
        env = awbw.TeacherEnv(
            num_envs=batch,
            teacher=args.teacher,
            seed=args.seed + (10_000 if validation else 0),
            max_day=args.max_day,
            threat=args.threat_planes,
        )
    return Source(env, batch)


def load_outcomes(path):
    """Per-game winner as a small dict: game id -> winning seat index.

    Winners come from the replay's own `GameOver` record for 87.7% of the
    corpus, cross-checked 1,076/1,076 against the mooo archive
    (`data/game-meta-119544.json`). Games without a label simply contribute
    no value gradient; their policy labels are unaffected.
    """
    meta = json.loads(Path(path).read_text(encoding="utf-8"))
    return {int(gid): entry["winner_index"]
            for gid, entry in meta.items()
            if entry.get("winner_index") is not None}


def value_targets(ids, movers, outcomes, device):
    """(+1 mover won, -1 mover lost) per row, and which rows have a label.

    The scale matches PPO's terminal reward exactly, so the critic BC hands
    over is already in the units PPO trains it in — the whole point: PPO
    currently inherits a *random* critic and spends 25 warm-up iterations
    fitting it while the policy idles.
    """
    target = torch.zeros(len(ids), device=device)
    have = torch.zeros(len(ids), dtype=torch.bool, device=device)
    for row, (gid, mover) in enumerate(zip(ids.tolist(), movers.tolist())):
        winner = outcomes.get(gid)
        if winner is None or mover < 0:
            continue
        target[row] = 1.0 if winner == mover else -1.0
        have[row] = True
    return target, have


# What ±1 means on both auxiliary scalars: eight properties of lead, measured
# once as a count and once as the income it pays. Normalising by the *map*
# instead is the interpretable choice and was the first one tried -- it leaves
# the targets with a standard deviation near 0.1, an MSE a head answers by
# predicting zero. Eight puts them near 0.45 and lets the clip do real work on
# blowouts, which the value head already records as wins.
AUX_PROPERTIES = 8.0


def _versus(values):
    """Each seat's edge over its best opponent, along the last axis.

    Two seats is what the training slice has, and there the subtraction is the
    obvious one; written for any number so a target does not quietly mean
    something else on a three-player replay.
    """
    out = np.empty_like(values)
    for seat in range(values.shape[-1]):
        others = np.delete(values, seat, axis=-1)
        out[..., seat] = values[..., seat] - others.max(axis=-1)
    return out


class Aux:
    """Auxiliary targets for the value trunk, joined to rows by (game, turn).

    Three of them, all from the mover's side and all futures the teacher cannot
    see -- it streams a game forwards, and these are read off its end. So
    `aux-targets` (the Rust binary) dumps them once per corpus and this joins
    them back on the key `ReplayTeacher` already reports.

    - *final property differential*, the property race's outcome;
    - *income differential `horizon` days out*, the same race at a range the
      policy can still act on. A game that ends first freezes at its last turn,
      which is the honest answer: the race was over;
    - *final ownership per tile*, three classes -- the mover's, the opponent's,
      nobody's -- and `ignore_index` on every tile that is not a property.
      This is KataGo's ownership target, and the reason the head is per tile.

    Both scalars are normalised to roughly [-1, 1] so the auxiliary MSE cannot
    swamp the losses it rides alongside.

    Measured negative, and left in place for the follow-up that separates the
    dense ownership term from the two cheap globals --
    `log/2026-09-06-the-aux-targets-cost-two-orders-a-turn.md`.
    """

    def __init__(self, path, shape, horizon: int):
        self.height, self.width = shape
        self.tiles = self.height * self.width
        self.horizon = horizon
        self.games = {}
        self.off_map = 0
        raw = json.loads(Path(path).read_text(encoding="utf-8"))
        for gid, entry in raw.items():
            # The dump covers the whole corpus and a trainer serves one map.
            # Tile indices only mean anything on the board they were written
            # for, so a game on another one is dropped, not reinterpreted.
            if (entry["height"], entry["width"]) != (self.height, self.width):
                self.off_map += 1
                continue
            game = self._prepare(entry, self.tiles, horizon)
            if game is not None:
                self.games[int(gid)] = game

    @staticmethod
    def _prepare(entry, tiles: int, horizon: int):
        seats = entry["seats"]
        props = np.asarray(entry["props"], dtype=np.int64)
        rows = entry["turns"]
        if props.size == 0 or not rows:
            return None

        days = np.zeros(len(rows), dtype=np.float64)
        income = np.zeros((len(rows), seats), dtype=np.float64)
        good = np.zeros(len(rows), dtype=bool)
        # A snapshot that would not load carries its predecessor's numbers, so
        # the day column stays sorted for the search below; its own row is
        # still marked bad and contributes nothing.
        for i, row in enumerate(rows):
            if row is None:
                if i:
                    days[i], income[i] = days[i - 1], income[i - 1]
                continue
            days[i] = row[0]
            income[i] = row[1:1 + seats]
            good[i] = True
        if not good.any():
            return None

        # The first turn at least `horizon` days out, or the last turn there is.
        ahead = np.searchsorted(days, days + horizon, side="left")
        ahead = np.minimum(ahead, len(rows) - 1)

        counts = np.asarray(entry["final_counts"], dtype=np.float64)
        scale_income = AUX_PROPERTIES * max(entry["funds_per_property"], 1)
        prop = np.clip(_versus(counts) / AUX_PROPERTIES, -1.0, 1.0)
        inc = np.clip(_versus(income[ahead]) / scale_income, -1.0, 1.0)

        final_owner = np.asarray(entry["final_owner"], dtype=np.int64)
        owner = np.full((seats, tiles), -100, dtype=np.int64)
        for seat in range(seats):
            classes = np.where(final_owner < 0, 2, np.where(final_owner == seat, 0, 1))
            owner[seat, props] = classes

        return {
            "good": good,
            "prop": prop.astype(np.float32),
            "inc": inc.astype(np.float32),
            "owner": owner,
        }

    def targets(self, ids, movers, turns, device):
        """`(globals, ownership, mask)` for one batch of rows."""
        n = len(ids)
        glob = np.zeros((n, netmod.AUX_GLOBALS), dtype=np.float32)
        owner = np.full((n, self.tiles), -100, dtype=np.int64)
        have = np.zeros(n, dtype=bool)
        rows = zip(ids.tolist(), movers.tolist(), turns.tolist())
        for row, (gid, mover, turn) in enumerate(rows):
            game = self.games.get(gid)
            if game is None or mover < 0 or turn < 0 or turn >= len(game["good"]):
                continue
            if not game["good"][turn]:
                continue
            glob[row, 0] = game["prop"][mover]
            glob[row, 1] = game["inc"][turn, mover]
            owner[row] = game["owner"][mover]
            have[row] = True
        return (
            torch.from_numpy(glob).to(device),
            torch.from_numpy(owner).to(device),
            torch.from_numpy(have).to(device),
        )


def source_loss(logits, target, targets_set, mask, weight: float,
                pop_floor: int = 0, pop_weight: float = 1.0):
    """Cross-entropy against the exact unit, blended with the turn's whole set.

    The set term is `-log` of the probability the policy puts *anywhere* in the
    set — "name a unit that still has something to do" rather than "name the one
    the human happened to move next". It is a relaxation, not a replacement: the
    exact label is always a member, and order is not always free (a blocker has
    to move before the unit it blocks), so the exact term keeps a share.
    """
    if pop_weight != 1.0 and pop_floor:
        # 213 activation labels in 1.9M orders seeded nothing at uniform
        # weight, and the anchor provably propagates whatever the clone
        # holds -- so the timing has to be *in* the clone. Upweighting is
        # the standard fix for a vanishing label class.
        per = F.cross_entropy(logits[mask], target[mask], reduction="none")
        w = torch.where(target[mask] >= pop_floor,
                        per.new_full((), pop_weight), per.new_ones(()))
        exact = (per * w).sum() / w.sum()
    else:
        exact = F.cross_entropy(logits[mask], target[mask])
    if targets_set is None or weight <= 0:
        return exact
    logp = torch.log_softmax(logits[mask], dim=1)
    allowed = targets_set[mask]
    # A row with an empty set has nothing to say; fall back to the exact label.
    covered = allowed.any(dim=1)
    if not covered.any():
        return exact
    inside = torch.logsumexp(
        logp[covered].masked_fill(~allowed[covered], float("-inf")), dim=1)
    return (1 - weight) * exact + weight * (-inside.mean())


def losses(policy, obs, codes, keep, end_turn_index: int,
           targets_set=None, set_weight: float = 0.0,
           value_target=None, value_mask=None, value_coef: float = 0.0,
           pop_weight: float = 1.0, aux=None, aux_coef: float = 0.0):
    """Cross-entropy per head, over the rows where that head means anything.

    Ending the turn names no destination, and most orders carry no parameter.
    Training those heads on the engine's filler zero would teach the network to
    be confident about a value nothing reads, and would flatter the accuracy
    report besides.
    """
    source, dest, kind, param = codes.unbind(dim=1)
    # One trunk pass feeds whichever extra heads have targets this step.
    if aux is not None:
        logits, value, (aux_global, aux_owner) = policy.evaluate_actions(
            obs, source, dest, kind, aux=True)
    elif value_target is not None:
        logits, value = policy.evaluate_actions(obs, source, dest, kind)
    else:
        logits = policy(obs, source=source, dest=dest, kind=kind)

    acting = keep & (source != end_turn_index)
    has_param = keep & torch.isin(
        kind, torch.tensor(sorted(netmod.PARAM_KINDS), device=kind.device)
    )
    masks = [keep, acting, acting, has_param]
    targets = [source, dest, kind, param]

    total = obs.new_zeros(())
    # Counts stay on the GPU as one tensor: reading any of them back per step
    # stalls the pipeline waiting for the backward pass to land, which cost
    # half the throughput when these were plain floats.
    tally = obs.new_zeros(2 * len(STATS))
    whole = keep.clone()
    for i, (logit, target, mask) in enumerate(zip(logits, targets, masks)):
        hit = (logit.argmax(dim=1) == target) & mask
        if mask.any():
            if i == 0:
                total = total + source_loss(logit, target, targets_set, mask,
                                            set_weight, end_turn_index + 1,
                                            pop_weight)
            else:
                total = total + F.cross_entropy(logit[mask], target[mask])
        tally[2 * i] = hit.sum()
        tally[2 * i + 1] = mask.sum()
        # A head that does not apply to this order cannot make it wrong.
        whole &= hit | ~mask

    # The whole order, right end to end. This is the number that matters:
    # three heads out of four is a different move.
    tally[2 * ORDER] = (whole & keep).sum()
    tally[2 * ORDER + 1] = keep.sum()

    if value_target is not None:
        rows = keep & value_mask
        if rows.any():
            total = total + value_coef * F.mse_loss(
                value[rows], value_target[rows])

    if aux is not None:
        aux_glob, aux_tiles, aux_mask = aux
        rows = keep & aux_mask
        if rows.any():
            total = total + aux_coef * F.mse_loss(
                aux_global[rows], aux_glob[rows])
            # Rows with no label join the non-property tiles in `ignore_index`,
            # so the ownership mean is over labelled property tiles alone.
            target = aux_tiles.masked_fill(~rows.unsqueeze(1), -100)
            scored = target >= 0
            total = total + aux_coef * F.cross_entropy(
                aux_owner, target, ignore_index=-100)
            tally[2 * OWNER] = ((aux_owner.argmax(dim=1) == target) & scored).sum()
            tally[2 * OWNER + 1] = scored.sum()
    return total, tally.detach()


HEADS = ("source", "dest", "kind", "param")
STATS = HEADS + ("order", "owner")
ORDER = STATS.index("order")
OWNER = STATS.index("owner")


def unpack(tally) -> dict:
    """Accuracy per head from accumulated (hits, seen) pairs."""
    values = tally.tolist()
    return {
        name: values[2 * i] / max(values[2 * i + 1], 1)
        for i, name in enumerate(STATS)
    }


@torch.no_grad()
def validate(policy, source: Source, batches: int, device, end_turn_index: int):
    policy.eval()
    # Score the same orders every time, so a rising number cannot be an easier
    # sample. The scripted teacher generates fresh games and cannot rewind.
    if source.human:
        source.env.reset()
    total = None
    for _ in range(batches):
        obs, codes, keep, _, _, _, _ = source.next(device)
        # Scored on the exact label always: the set target changes what the
        # policy is taught, and a metric that moved with it could not say
        # whether that helped.
        _, tally = losses(policy, obs, codes, keep, end_turn_index)
        total = tally if total is None else total + tally
    policy.train()
    return unpack(total)


def save(policy, args, map_name):
    """Weights plus the shape they were built to, so a checkpoint can be
    reloaded without being told how it was configured — and so an architecture
    change is a loud shape error rather than a quiet half-load."""
    out = ROOT / args.out
    out.parent.mkdir(parents=True, exist_ok=True)
    torch.save(
        {
            "policy": policy.state_dict(),
            "config": {
                "planes": policy.planes,
                "globals": policy.globals_,
                "height": policy.height,
                "width": policy.width,
                "head_sizes": policy.head_sizes,
                "channels": args.channels,
                "blocks": args.blocks,
                "norm": args.norm,
                "pool_bias": args.pool_bias,
                "value_pool": args.value_pool,
                "aux": args.value_aux is not None,
            },
            "teacher": args.teacher,
            "source_set": args.source_set,
            "map_name": map_name,
        },
        out,
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--teacher", default="human",
                        choices=["human", "greedy", "capturer", "random"])
    parser.add_argument("--steps", type=int, default=4000)
    parser.add_argument("--batch", type=int, default=128)
    parser.add_argument("--channels", type=int, default=64)
    parser.add_argument("--blocks", type=int, default=6)
    parser.add_argument("--lr", type=float, default=2e-3)
    parser.add_argument("--weight-decay", type=float, default=1e-4)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--holdout", type=float, default=0.12)
    parser.add_argument("--val-batches", type=int, default=16)
    parser.add_argument("--val-every", type=int, default=500)
    parser.add_argument("--map-name", default="A River Supreme")
    parser.add_argument("--max-day", type=int, default=60)
    # How much of the source head's loss asks "a unit that still has something
    # to do" rather than "the unit the human moved next".
    #
    # Off, having been tried. Most of that head's error really is the human's
    # arbitrary ordering — 44.7% exact against 95.2% in-turn, 68.2% by chance —
    # but relieving it changed play by 19.0% to 19.8% (+-2.0), because 95.2% had
    # almost no headroom above it. Kept because the measurement is worth
    # repeating if the encoding or the head design changes.
    parser.add_argument("--source-set", type=float, default=0.0,
                        help="0 exact label, 1 the whole turn's set")
    # The observation grows four planes of applied damage-chart arithmetic;
    # the checkpoint's stored plane count is what versions the layout, and
    # evaluate.py reconstructs its env to match. A/B against the same recipe
    # without the flag -- the success criterion is engagement pricing
    # (`log/2026-08-27-the-exploit-is-legible.md`), not accuracy alone.
    parser.add_argument("--threat-planes", action="store_true")
    parser.add_argument("--norm", default="batch", choices=["batch", "group"],
                        help="group has no running statistics: no recalibrate "
                             "class of bug, no train/eval gap")
    parser.add_argument("--pool-bias", action="store_true",
                        help="global-pooling bias on alternating blocks")
    parser.add_argument("--value-pool", default="mean",
                        choices=["mean", "meanmax"])
    parser.add_argument("--pop-weight", type=float, default=1.0,
                        help="loss weight on CO-activation source labels, "
                             "so 213 labels in 1.9M orders can register")
    parser.add_argument("--value-outcomes", default=None,
                        help="game-meta json; trains the value head on real "
                             "winners so PPO does not inherit a random critic")
    parser.add_argument("--value-coef", type=float, default=0.5)
    # `network.md` item 3: the last of KataGo's global bundle. The targets are
    # futures, so they are dumped once per corpus by the `aux-targets` binary
    # and joined by (game, turn) here.
    parser.add_argument("--value-aux", default=None,
                        help="aux-targets json; trains two extra heads on how "
                             "the property race ends and where it stands")
    parser.add_argument("--aux-coef", type=float, default=0.3)
    parser.add_argument("--aux-horizon", type=int, default=5,
                        help="days ahead the income-differential target reads")
    parser.add_argument("--out", default="checkpoints/bc.pt")
    parser.add_argument("--init", default=None,
                        help="checkpoint to start from, for staged training")
    # Off by default, and measured rather than assumed: on a card without fp16
    # tensor cores (a 1660 Ti here) autocast buys nothing and pays a conversion
    # on every op -- 42 ms/step became 155. Worth trying on anything newer.
    parser.add_argument("--amp", action="store_true",
                        help="half precision; only helps with tensor cores")
    args = parser.parse_args()

    torch.manual_seed(args.seed)
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    amp = args.amp and device.type == "cuda"

    train = make_source(args, args.batch, validation=False)
    holdout = args.holdout if args.teacher == "human" else 0.0
    val = make_source(args, min(args.batch, 64), validation=holdout > 0)

    policy = netmod.build(train.env, args.channels, args.blocks,
                          norm=args.norm, pool_bias=args.pool_bias,
                          value_pool=args.value_pool,
                          aux=args.value_aux is not None).to(device)
    parameters = sum(p.numel() for p in policy.parameters())
    if args.init:
        state = torch.load(args.init, map_location=device, weights_only=True)
        # The auxiliary heads are the one part a checkpoint is allowed to be
        # missing -- they are new, they are cloning-only, and starting them
        # from their init costs nothing. Everything else stays a loud error.
        missing, unexpected = policy.load_state_dict(state["policy"], strict=False)
        stray = [k for k in list(missing) + list(unexpected)
                 if not k.startswith("aux_")]
        if stray:
            raise SystemExit(f"--init does not match this net: {stray[:4]}")

    optimizer = torch.optim.AdamW(
        policy.parameters(), lr=args.lr, weight_decay=args.weight_decay
    )
    schedule = torch.optim.lr_scheduler.OneCycleLR(
        optimizer, max_lr=args.lr, total_steps=args.steps, pct_start=0.1
    )
    scaler = torch.amp.GradScaler("cuda", enabled=amp)
    end_turn_index = train.env.end_turn_index
    outcomes = None
    if args.value_outcomes:
        if args.teacher != "human":
            raise SystemExit("--value-outcomes needs the human teacher")
        outcomes = load_outcomes(ROOT / args.value_outcomes)
        print(f"value head trains on {len(outcomes)} labeled games")
    aux = None
    if args.value_aux:
        if args.teacher != "human":
            raise SystemExit("--value-aux needs the human teacher")
        aux = Aux(ROOT / args.value_aux, train.env.board_shape, args.aux_horizon)
        print(f"aux heads train on {len(aux.games)} games "
              f"({aux.off_map} dropped as another map), "
              f"income read {args.aux_horizon} days out")

    print(f"device {device}, {parameters / 1e6:.2f}M parameters, "
          f"{args.channels}ch x {args.blocks} blocks")
    print(f"teacher {args.teacher}, batch {args.batch}, board {train.env.board_shape}")
    if args.teacher == "human":
        print(f"  train games {train.env.replay_count}, "
              f"held out {val.env.replay_count}")

    running = None
    loss_sum = torch.zeros((), device=device)
    data_time = 0.0
    start = time.perf_counter()

    for step in range(1, args.steps + 1):
        mark = time.perf_counter()
        obs, codes, keep, targets_set, ids, movers, turns = train.next(device)
        data_time += time.perf_counter() - mark
        if not keep.any():
            continue
        value_target = value_mask = None
        if outcomes is not None:
            value_target, value_mask = value_targets(
                ids, movers, outcomes, device)
        aux_batch = aux.targets(ids, movers, turns, device) if aux else None

        with torch.amp.autocast("cuda", enabled=amp):
            loss, tally = losses(policy, obs, codes, keep, end_turn_index,
                                 targets_set, args.source_set,
                                 value_target, value_mask, args.value_coef,
                                 args.pop_weight, aux_batch, args.aux_coef)

        optimizer.zero_grad(set_to_none=True)
        scaler.scale(loss).backward()
        scaler.unscale_(optimizer)
        torch.nn.utils.clip_grad_norm_(policy.parameters(), 5.0)
        scaler.step(optimizer)
        scaler.update()
        schedule.step()

        running = tally if running is None else running + tally
        loss_sum = loss_sum + loss.detach()

        if step % REPORT == 0 or step == args.steps:
            scores = unpack(running)
            rate = step * args.batch / (time.perf_counter() - start)
            since = step % REPORT or REPORT
            print(
                f"  {step:>5}/{args.steps}  loss {loss_sum.item() / since:.3f}  "
                + " ".join(f"{k[:3]} {scores[k]:.3f}" for k in HEADS)
                + f" | order {scores['order']:.3f}"
                + (f"  own {scores['owner']:.3f}" if aux else "")
                + f"  {rate:,.0f}/s"
            )
            running = None
            loss_sum = torch.zeros((), device=device)

        if step % args.val_every == 0 or step == args.steps:
            scores = validate(policy, val, args.val_batches, device, end_turn_index)
            # Written at every validation, not only at the end: half an hour of
            # training should not be lost to a crash in its last minute.
            save(policy, args, train.env.map_name)
            label = "held-out" if args.teacher == "human" else "fresh games"
            print(
                f"      {label}: order {scores['order']:.3f}  "
                + " ".join(f"{k} {scores[k]:.3f}" for k in HEADS)
            )

    elapsed = time.perf_counter() - start
    print(f"\n{args.steps * args.batch:,} orders in {elapsed:.0f}s "
          f"({data_time / elapsed:.0%} of it waiting on data)")

    save(policy, args, train.env.map_name)
    print(f"saved {ROOT / args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
