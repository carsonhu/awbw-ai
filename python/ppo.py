"""PPO fine-tuning, from a cloned policy.

Cloning cannot exceed the player it copies: a net that predicts greedy's own
order 81% of the time still lost 39 of 40 games *to greedy*. Only playing can
get past the demonstrator, and only now is that possible — random self-play
reaches a real win zero times in forty games, so before cloning there was no
gradient to climb at all. There is one now: the cloned policy beats `random` and
`capturer` outright and takes a few games off `greedy`.

The action is factorized, so the log-probability of an order is the sum over the
four heads, each scored under the engine's mask. Masks are recorded during the
rollout and replayed in the update: score a head against a different mask than
the one it was sampled under and the importance ratio is measuring two different
distributions.

    py -3.12 python/ppo.py --init checkpoints/bc-scaled.pt
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
HEADS = 4


def masked_logits(logits, mask):
    """Forbidden entries get -inf, so they hold no probability and no gradient.

    Clamped rather than set outright: a row whose mask is empty would otherwise
    be all -inf and produce NaN through the softmax. That should not happen —
    ending the turn is always legal — but a NaN here is silent and poisons every
    later update.
    """
    out = logits.masked_fill(~mask, float("-inf"))
    empty = ~mask.any(dim=1)
    if empty.any():
        out[empty] = 0.0
    return out


def head_stats(logits, mask, action):
    """Log-probability of a taken index and the head's entropy."""
    logp = torch.log_softmax(masked_logits(logits, mask), dim=1)
    taken = logp.gather(1, action.unsqueeze(1)).squeeze(1)
    probs = logp.exp()
    entropy = -(probs * torch.where(torch.isfinite(logp), logp,
                                    torch.zeros_like(logp))).sum(dim=1)
    return taken, entropy


class Rollout:
    """Fixed-size storage for one batch of experience.

    Observations dominate: 19,603 floats each, so `envs * steps` of them is the
    whole memory budget on a small card. That is what sets the rollout size, not
    anything about the algorithm.
    """

    def __init__(self, envs, steps, obs_size, sizes, device):
        self.obs = torch.zeros((steps, envs, obs_size), device=device)
        self.actions = torch.zeros((steps, envs, HEADS), dtype=torch.long,
                                   device=device)
        self.masks = [
            torch.zeros((steps, envs, n), dtype=torch.bool, device=device)
            for n in sizes
        ]
        self.logp = torch.zeros((steps, envs), device=device)
        self.values = torch.zeros((steps, envs), device=device)
        self.rewards = torch.zeros((steps, envs), device=device)
        self.dones = torch.zeros((steps, envs), device=device)
        self.steps = steps
        self.envs = envs


class Trainer:
    def __init__(self, args, device):
        self.args = args
        self.device = device
        self.env = awbw.VecEnv(
            num_envs=args.envs, seed=args.seed, max_day=args.max_day,
            shaping=args.shaping, opponent=args.opponent,
        )
        self.policy = self.load_policy()
        self.optimizer = torch.optim.AdamW(self.policy.parameters(), lr=args.lr,
                                           weight_decay=0.0)
        # The warm-up steps this one instead. The value head shares the trunk,
        # so fitting the critic through the full optimizer would drag the policy
        # along with it — entropy climbed from 1.9 to 2.8 during a warm-up that
        # was supposed to leave the policy untouched.
        self.critic_optimizer = torch.optim.AdamW(
            self.policy.value.parameters(), lr=args.critic_lr, weight_decay=0.0)
        self.sizes = list(self.env.action_sizes)
        self.buffer = Rollout(args.envs, args.steps, self.env.observation_size,
                              self.sizes, device)
        self.staging = torch.empty((args.envs, self.env.observation_size),
                                   dtype=torch.float32,
                                   pin_memory=device.type == "cuda")
        self.view = self.staging.numpy()

    def load_policy(self):
        saved = torch.load(ROOT / self.args.init, map_location=self.device,
                           weights_only=True)
        config = saved["config"]
        policy = netmod.Policy(
            planes=config["planes"], globals_=config["globals"],
            height=config["height"], width=config["width"],
            head_sizes=config["head_sizes"], channels=config["channels"],
            blocks=config["blocks"],
        ).to(self.device)
        policy.load_state_dict(saved["policy"])
        self.config = config
        return policy

    def observe(self):
        self.env.observe_into(self.view)
        return self.staging.to(self.device, non_blocking=True)

    def current_masks(self, s=None, d=None, k=None):
        """One head's mask at a time, since each depends on the last choice."""
        if s is None:
            return torch.from_numpy(self.env.source_mask()).to(self.device)
        if d is None:
            return torch.from_numpy(self.env.dest_mask(s)).to(self.device)
        if k is None:
            return torch.from_numpy(self.env.kind_mask(s, d)).to(self.device)
        return torch.from_numpy(self.env.param_mask(s, d, k)).to(self.device)

    @torch.no_grad()
    def collect(self):
        buf = self.buffer
        self.policy.eval()
        for t in range(buf.steps):
            obs = self.observe()
            buf.obs[t] = obs
            features, flat, pooled = self.policy.trunk(obs)
            buf.values[t] = self.policy.value(pooled).squeeze(1)

            chosen, total_logp, raw = [], 0.0, []
            for head in range(HEADS):
                if head == 0:
                    logits = self.policy.source_logits(features, pooled)
                    mask = self.current_masks()
                elif head == 1:
                    logits = self.policy.dest_logits(features, flat, pooled,
                                                     chosen[0])
                    mask = self.current_masks(raw[0])
                elif head == 2:
                    context = self.policy.context_of(flat, pooled, chosen[0],
                                                     chosen[1])
                    logits = self.policy.kind_logits(context)
                    mask = self.current_masks(raw[0], raw[1])
                else:
                    logits = self.policy.param_logits(features, context,
                                                      chosen[2])
                    mask = self.current_masks(raw[0], raw[1], raw[2])

                distribution = torch.distributions.Categorical(
                    logits=masked_logits(logits, mask))
                action = distribution.sample()
                total_logp = total_logp + distribution.log_prob(action)
                buf.masks[head][t] = mask
                buf.actions[t, :, head] = action
                chosen.append(action)
                raw.append(action.to(torch.int32).cpu().numpy().astype(np.uint32))

            buf.logp[t] = total_logp
            rewards, dones, _ = self.env.step(*raw)
            buf.rewards[t] = torch.from_numpy(rewards).to(self.device)
            buf.dones[t] = torch.from_numpy(dones).to(self.device).float()

        # Bootstrap from the position play actually continues from.
        with torch.no_grad():
            last = self.policy.value_of(self.observe())
        return last

    def advantages(self, last_value):
        """Generalized advantage estimation.

        A game is hundreds of orders long, so `gamma` sits very close to one:
        the terminal win has to survive being discounted across the whole game
        or the policy only ever sees the shaped signal.
        """
        buf = self.buffer
        adv = torch.zeros_like(buf.rewards)
        running = torch.zeros(buf.envs, device=self.device)
        for t in reversed(range(buf.steps)):
            nxt = last_value if t == buf.steps - 1 else buf.values[t + 1]
            alive = 1.0 - buf.dones[t]
            delta = buf.rewards[t] + self.args.gamma * nxt * alive - buf.values[t]
            running = delta + self.args.gamma * self.args.lam * alive * running
            adv[t] = running
        return adv, adv + buf.values

    def update(self, adv, returns, critic_only=False):
        buf = self.buffer
        # Deliberately NOT train(): the batch-norm layers must use the same
        # running statistics the rollout was sampled under. In train mode they
        # use the minibatch's own statistics instead, which on identical
        # observations moved logits by up to 24 and flipped the argmax on half
        # of them -- so every importance ratio was comparing two different
        # policies and the first update destroyed the cloned one. Gradients flow
        # in eval mode; only normalisation behaves differently.
        self.policy.eval()
        flat_obs = buf.obs.reshape(-1, buf.obs.shape[-1])
        flat_actions = buf.actions.reshape(-1, HEADS)
        flat_masks = [m.reshape(-1, m.shape[-1]) for m in buf.masks]
        flat_logp = buf.logp.reshape(-1)
        flat_adv = adv.reshape(-1)
        flat_returns = returns.reshape(-1)
        flat_adv = (flat_adv - flat_adv.mean()) / (flat_adv.std() + 1e-8)

        total = flat_obs.shape[0]
        stats = {"policy": 0.0, "value": 0.0, "entropy": 0.0, "kl": 0.0,
                 "clipped": 0.0, "n": 0}
        recent = 0.0
        for _ in range(self.args.epochs):
            order = torch.randperm(total, device=self.device)
            for start in range(0, total, self.args.minibatch):
                idx = order[start:start + self.args.minibatch]
                source, dest, kind, param = flat_actions[idx].unbind(dim=1)
                logits, value = self.policy.evaluate_actions(
                    flat_obs[idx], source, dest, kind)

                logp, entropy = 0.0, 0.0
                taken = (source, dest, kind, param)
                for head in range(HEADS):
                    part, ent = head_stats(logits[head], flat_masks[head][idx],
                                           taken[head])
                    logp = logp + part
                    entropy = entropy + ent

                ratio = (logp - flat_logp[idx]).exp()
                clipped = ratio.clamp(1 - self.args.clip, 1 + self.args.clip)
                policy_loss = -torch.min(ratio * flat_adv[idx],
                                         clipped * flat_adv[idx]).mean()
                value_loss = F.mse_loss(value, flat_returns[idx])
                if critic_only:
                    loss = value_loss
                else:
                    loss = (policy_loss
                            + self.args.value_coef * value_loss
                            - self.args.entropy_coef * entropy.mean())

                stepped = self.critic_optimizer if critic_only else self.optimizer
                self.optimizer.zero_grad(set_to_none=True)
                self.critic_optimizer.zero_grad(set_to_none=True)
                loss.backward()
                torch.nn.utils.clip_grad_norm_(
                    [p for group in stepped.param_groups for p in group["params"]],
                    self.args.max_grad)
                stepped.step()

                with torch.no_grad():
                    # Schulman's low-variance estimator, which unlike the plain
                    # difference of log-probs cannot come out negative and so
                    # can actually be compared against a threshold.
                    log_ratio = logp - flat_logp[idx]
                    kl = ((log_ratio.exp() - 1) - log_ratio).mean().item()
                    stats["policy"] += policy_loss.item()
                    stats["value"] += value_loss.item()
                    stats["entropy"] += entropy.mean().item()
                    stats["kl"] += kl
                    stats["clipped"] += (
                        (ratio - 1).abs() > self.args.clip).float().mean().item()
                    stats["n"] += 1
                    recent = kl

            # Stop the moment the policy has moved as far as this rollout can
            # justify. Without it a cloned policy is unlearned in a handful of
            # updates, since the early advantage signal is small and noisy.
            if not critic_only and recent > self.args.target_kl:
                stats["stopped"] = stats.get("stopped", 0) + 1
                break
        n = max(stats.pop("n"), 1)
        stopped = stats.pop("stopped", 0)
        out = {k: v / n for k, v in stats.items()}
        out["stopped"] = stopped
        return out

    def save(self, path):
        path = ROOT / path
        path.parent.mkdir(parents=True, exist_ok=True)
        torch.save({"policy": self.policy.state_dict(), "config": self.config,
                    "teacher": "ppo", "map_name": self.env.map_name}, path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--init", default="checkpoints/bc-scaled.pt")
    parser.add_argument("--out", default="checkpoints/ppo.pt")
    parser.add_argument("--opponent", default="greedy",
                        choices=["greedy", "capturer", "random"])
    parser.add_argument("--iterations", type=int, default=200)
    parser.add_argument("--envs", type=int, default=32)
    parser.add_argument("--steps", type=int, default=64)
    parser.add_argument("--minibatch", type=int, default=256)
    parser.add_argument("--epochs", type=int, default=3)
    parser.add_argument("--lr", type=float, default=1e-4)
    parser.add_argument("--critic-lr", type=float, default=1e-3,
                        help="warm-up rate; the critic starts from nothing")
    parser.add_argument("--gamma", type=float, default=0.997)
    parser.add_argument("--lam", type=float, default=0.95)
    parser.add_argument("--clip", type=float, default=0.2)
    # Zero, which is unusual and deliberate. An entropy bonus exists to stop a
    # policy converging before it has explored, and a cloned one has the
    # opposite problem: it is already peaked, and what it knows is the only
    # reason the reward is reachable at all. Raise it if the policy is seen
    # collapsing onto a single line of play.
    #
    # Do not read the reported entropy as a health check. It rises through a
    # rollout even with the policy provably frozen, because a midgame position
    # holds more units and more real choices than an opening does — it tracks
    # where the games are, not what the policy is becoming.
    parser.add_argument("--entropy-coef", type=float, default=0.0)
    parser.add_argument("--target-kl", type=float, default=0.02,
                        help="stop an update once the policy has moved this far")
    parser.add_argument("--value-warmup", type=int, default=25,
                        help="iterations fitting the critic before the policy moves")
    parser.add_argument("--value-coef", type=float, default=0.5)
    parser.add_argument("--max-grad", type=float, default=1.0)
    parser.add_argument("--shaping", type=float, default=0.1)
    parser.add_argument("--max-day", type=int, default=60)
    parser.add_argument("--seed", type=int, default=11)
    parser.add_argument("--report-every", type=int, default=10)
    args = parser.parse_args()

    torch.manual_seed(args.seed)
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    trainer = Trainer(args, device)

    per = args.envs * args.steps
    print(f"device {device}, {sum(p.numel() for p in trainer.policy.parameters())/1e6:.2f}M "
          f"parameters, from {args.init}")
    print(f"vs {args.opponent}, {args.envs} envs x {args.steps} steps = "
          f"{per} orders per iteration, shaping {args.shaping}")

    start = time.perf_counter()
    seen = (0, 0, 0)
    for iteration in range(1, args.iterations + 1):
        last = trainer.collect()
        adv, returns = trainer.advantages(last)
        # Cloning never trains the value head -- its loss is four
        # cross-entropies and nothing else -- so PPO inherits a *random* critic.
        # Advantages are then mostly the critic's own error, and normalising
        # them rescales that noise to unit size, which diffuses the cloned
        # policy toward uniform. Fit the critic first, policy frozen.
        warming = iteration <= args.value_warmup
        stats = trainer.update(adv, returns, critic_only=warming)

        if warming and iteration == args.value_warmup:
            print(f"  critic warm-up done after {iteration} iterations "
                  f"(value loss {stats['value']:.4f})")
        if iteration % args.report_every == 0 or iteration == 1:
            # Windowed, not cumulative: `results` counts from construction, so a
            # running average of every game ever played would still be showing
            # the initial policy's score long after it had improved.
            played, won, drawn = trainer.env.results
            games = played - seen[0]
            score = ((won - seen[1]) + 0.5 * (drawn - seen[2])) / max(games, 1)
            seen = (played, won, drawn)
            rate = iteration * per / (time.perf_counter() - start)
            print(f"  {iteration:>4}/{args.iterations}  "
                  f"score {score:.1%} over {games} games  "
                  f"| pi {stats['policy']:+.4f} v {stats['value']:.3f} "
                  f"H {stats['entropy']:.2f} kl {stats['kl']:.4f} "
                  f"clip {stats['clipped']:.2f} stop {stats['stopped']}  "
                  f"{rate:,.0f}/s")
            trainer.save(args.out)

    trainer.save(args.out)
    played, won, drawn = trainer.env.results
    print(f"\n{played} games during training, "
          f"score {(won + 0.5*drawn)/max(played,1):.1%}")
    print(f"saved {ROOT / args.out} -- rate it with evaluate.py")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
