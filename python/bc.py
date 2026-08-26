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
        return obs, codes, keep, targets


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
        )
        return Source(env, batch, source_set=wants_set)
    else:
        env = awbw.TeacherEnv(
            num_envs=batch,
            teacher=args.teacher,
            seed=args.seed + (10_000 if validation else 0),
            max_day=args.max_day,
        )
    return Source(env, batch)


def source_loss(logits, target, targets_set, mask, weight: float):
    """Cross-entropy against the exact unit, blended with the turn's whole set.

    The set term is `-log` of the probability the policy puts *anywhere* in the
    set — "name a unit that still has something to do" rather than "name the one
    the human happened to move next". It is a relaxation, not a replacement: the
    exact label is always a member, and order is not always free (a blocker has
    to move before the unit it blocks), so the exact term keeps a share.
    """
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
           targets_set=None, set_weight: float = 0.0):
    """Cross-entropy per head, over the rows where that head means anything.

    Ending the turn names no destination, and most orders carry no parameter.
    Training those heads on the engine's filler zero would teach the network to
    be confident about a value nothing reads, and would flatter the accuracy
    report besides.
    """
    source, dest, kind, param = codes.unbind(dim=1)
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
    tally = obs.new_zeros(2 * len(HEADS) + 2)
    whole = keep.clone()
    for i, (logit, target, mask) in enumerate(zip(logits, targets, masks)):
        hit = (logit.argmax(dim=1) == target) & mask
        if mask.any():
            if i == 0:
                total = total + source_loss(logit, target, targets_set, mask,
                                            set_weight)
            else:
                total = total + F.cross_entropy(logit[mask], target[mask])
        tally[2 * i] = hit.sum()
        tally[2 * i + 1] = mask.sum()
        # A head that does not apply to this order cannot make it wrong.
        whole &= hit | ~mask

    # The whole order, right end to end. This is the number that matters:
    # three heads out of four is a different move.
    tally[-2] = (whole & keep).sum()
    tally[-1] = keep.sum()
    return total, tally.detach()


HEADS = ("source", "dest", "kind", "param")
STATS = HEADS + ("order",)


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
        obs, codes, keep, _ = source.next(device)
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
    # to do" rather than "the unit the human moved next". Measured with
    # order_diag.py: the exact label is right 44.7% of the time while the set is
    # right 95.2%, against 68.2% for a uniform pick — most of that head's error
    # is which order the human happened to use, which nothing can learn.
    parser.add_argument("--source-set", type=float, default=0.0,
                        help="0 exact label, 1 the whole turn's set")
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

    policy = netmod.build(train.env, args.channels, args.blocks).to(device)
    parameters = sum(p.numel() for p in policy.parameters())
    if args.init:
        state = torch.load(args.init, map_location=device, weights_only=True)
        policy.load_state_dict(state["policy"])

    optimizer = torch.optim.AdamW(
        policy.parameters(), lr=args.lr, weight_decay=args.weight_decay
    )
    schedule = torch.optim.lr_scheduler.OneCycleLR(
        optimizer, max_lr=args.lr, total_steps=args.steps, pct_start=0.1
    )
    scaler = torch.amp.GradScaler("cuda", enabled=amp)
    end_turn_index = train.env.end_turn_index

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
        obs, codes, keep, targets_set = train.next(device)
        data_time += time.perf_counter() - mark
        if not keep.any():
            continue

        with torch.amp.autocast("cuda", enabled=amp):
            loss, tally = losses(policy, obs, codes, keep, end_turn_index,
                                 targets_set, args.source_set)

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
                + f" | order {scores['order']:.3f}  {rate:,.0f}/s"
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
