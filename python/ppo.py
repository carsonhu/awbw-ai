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
import copy
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

    Observations dominate: 19,603 floats each, at 78KB apiece. They are kept in
    *host* memory and shipped to the card a minibatch at a time, because on a
    6GB card they otherwise set the rollout length, and the rollout length is
    the algorithm's credit horizon -- 64 orders is three turns of a game that
    runs four hundred to seven hundred. Host RAM does not care: a 256-step
    rollout is 640MB there and would not fit here.

    The cost is a gather and a transfer per minibatch, about 20MB, against an
    iteration that already spends seconds in the trunk. Everything else in the
    buffer is one number per step and stays on the card.
    """

    def __init__(self, envs, steps, obs_size, sizes, device):
        self.obs = torch.zeros((steps, envs, obs_size))
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
        # Of the finished games, the ones the day cap stopped. They end the
        # episode like any other but they carry no result, so they are not
        # allowed to teach the critic one.
        self.cut = torch.zeros((steps, envs), device=device)
        # Who moved, and whether it was us. Against a scripted opponent every
        # step is ours and every step in a row belongs to the same player; in
        # self-play neither holds, and both facts are needed to read the
        # rewards, which are always from the mover's own side.
        self.actors = torch.zeros((steps, envs), dtype=torch.long, device=device)
        self.mine = torch.ones((steps, envs), dtype=torch.bool, device=device)
        self.steps = steps
        self.envs = envs


class Trainer:
    def __init__(self, args, device):
        self.args = args
        self.device = device
        # Self-play takes no scripted opponent at all: the caller moves both
        # seats, and `agent_seat` says which rows are the learner's.
        self.env = awbw.VecEnv(
            num_envs=args.envs, seed=args.seed, max_day=args.max_day,
            shaping=args.shaping, potential=args.potential,
            decide_cap=args.decide_cap,
            opponent=None if args.selfplay else args.opponent,
        )
        self.policy = self.load_policy()
        # A frozen copy of the policy holds the other seat. Not the live policy
        # on both sides: its transitions would be off-policy for the update, and
        # a score against yourself is 50% by construction and says nothing. A
        # snapshot gives a real rating, and refreshing it when the learner pulls
        # ahead is what stops the opponent from running out -- which is exactly
        # how the first run against `greedy` came apart.
        self.frozen = None
        self.refreshes = 0
        self.recalibrating = 0
        if args.selfplay:
            self.frozen = copy.deepcopy(self.policy).eval()
            self.frozen.requires_grad_(False)
        self.seat = torch.from_numpy(self.env.agent_seat()).to(device)
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

    def heads_of(self, policy, obs, chosen, raw, head, carried):
        """One head's logits, conditioned on what the earlier heads chose."""
        features, flat, pooled = carried
        if head == 0:
            return policy.source_logits(features, pooled), self.current_masks()
        if head == 1:
            return (policy.dest_logits(features, flat, pooled, chosen[0]),
                    self.current_masks(raw[0]))
        if head == 2:
            return (policy.kind_logits(policy.context_of(
                flat, pooled, chosen[0], chosen[1])),
                self.current_masks(raw[0], raw[1]))
        context = policy.context_of(flat, pooled, chosen[0], chosen[1])
        return (policy.param_logits(features, context, chosen[2]),
                self.current_masks(raw[0], raw[1], raw[2]))

    @torch.no_grad()
    def collect(self):
        buf = self.buffer
        self.policy.eval()
        for t in range(buf.steps):
            obs = self.observe()
            # From the staging buffer rather than back off the card: the host
            # already has this observation, and it is the same bytes.
            buf.obs[t].copy_(self.staging)
            carried = self.policy.trunk(obs)
            buf.values[t] = self.policy.value(carried[2]).squeeze(1)
            # The value head is the learner's on every row, including the
            # opponent's: it is asked what the position is worth to whoever
            # moves next, which is what the bootstrap wants.
            mine = (torch.from_numpy(self.env.current_player()).to(self.device)
                    == self.seat) if self.frozen is not None else None
            theirs = self.frozen.trunk(obs) if self.frozen is not None else None

            chosen, total_logp, raw = [], 0.0, []
            for head in range(HEADS):
                logits, mask = self.heads_of(
                    self.policy, obs, chosen, raw, head, carried)
                distribution = torch.distributions.Categorical(
                    logits=masked_logits(logits, mask))
                action = distribution.sample()
                if self.frozen is not None:
                    # Both networks run on the whole batch and the rows are
                    # selected between, because each head's mask depends on what
                    # the previous head actually chose -- the two sides have to
                    # advance in step.
                    other, _ = self.heads_of(
                        self.frozen, obs, chosen, raw, head, theirs)
                    other = torch.distributions.Categorical(
                        logits=masked_logits(other, mask)).sample()
                    action = torch.where(mine, action, other)
                # Scored under the learner regardless of who chose, so an
                # opponent row carries a coherent number; it is masked out of
                # the update anyway.
                total_logp = total_logp + distribution.log_prob(action)
                buf.masks[head][t] = mask
                buf.actions[t, :, head] = action
                chosen.append(action)
                raw.append(action.to(torch.int32).cpu().numpy().astype(np.uint32))

            buf.logp[t] = total_logp
            if mine is not None:
                buf.mine[t] = mine
            rewards, dones, actors, cut = self.env.step(*raw)
            buf.rewards[t] = torch.from_numpy(rewards).to(self.device)
            buf.dones[t] = torch.from_numpy(dones).to(self.device).float()
            buf.actors[t] = torch.from_numpy(actors).to(self.device)
            buf.cut[t] = torch.from_numpy(cut).to(self.device).float()

        # Bootstrap from the position play actually continues from, and from
        # whose side it will be read.
        with torch.no_grad():
            last = self.policy.value_of(self.observe())
        last_actor = torch.from_numpy(self.env.current_player()).to(self.device)
        return last, last_actor

    def advantages(self, last_value, last_actor):
        """Generalized advantage estimation, across two players.

        A game is hundreds of orders long, so `gamma` sits very close to one:
        the terminal win has to survive being discounted across the whole game
        or the policy only ever sees the shaped signal. It does not, quite. At
        0.997 over the 690 orders a game against JakeMan takes, a win is worth
        0.13 by the time it reaches the opening, and `--turn-discount` is the
        answer to that -- see the flag.

        Every quantity here is written from the point of view of whoever moved,
        because that is how the environment reports them -- the observation is
        the mover's view of the board and the reward is their side of it. So a
        step whose successor belongs to the *other* player is looking at a value
        in the opposite frame, and the game is zero-sum, so it enters negated.
        Without that flip a policy is told that handing the opponent a good
        position is good for it, and both seats train against each other.
        """
        buf = self.buffer
        adv = torch.zeros_like(buf.rewards)
        running = torch.zeros(buf.envs, device=self.device)
        for t in reversed(range(buf.steps)):
            if t == buf.steps - 1:
                nxt, nxt_actor = last_value, last_actor
            else:
                nxt, nxt_actor = buf.values[t + 1], buf.actors[t + 1]
            # +1 while the same player is still moving, -1 across a change of
            # turn. A finished game bootstraps from nothing, so the sign there
            # is irrelevant.
            same = nxt_actor == buf.actors[t]
            flip = torch.where(same, 1.0, -1.0)
            # Per order by default; per *turn* under `--turn-discount`, which
            # charges nothing inside a turn and the full rate across one. The
            # orders in a turn are very nearly simultaneous -- a player picks
            # the sequence, and mostly it does not matter -- so discounting
            # between them prices a difference that is not there, and taxes
            # using your units at 0.3% of the win apiece.
            if self.args.turn_discount:
                one = torch.ones_like(flip)
                gamma = torch.where(same, one, one * self.args.gamma)
                lam = torch.where(same, one, one * self.args.lam)
            else:
                gamma, lam = self.args.gamma, self.args.lam
            alive = 1.0 - buf.dones[t]
            delta = (buf.rewards[t]
                     + gamma * flip * nxt * alive - buf.values[t])
            # A game the day cap stopped is not a game that ended in nothing.
            # Left alone the step above scores it against a bootstrap of zero,
            # so its advantage is -V(s) -- the critic is told, in proportion to
            # how good it thought the position was, that the position was
            # worthless. That is a *systematic* error and its sign follows the
            # matchup: for a policy losing 89% of its games a stopped game
            # really is better than the loss it was heading for, and for one
            # winning 51% it is worse. It is a reward for being behind and a
            # penalty for being ahead, which is the shape of every run so far.
            #
            # There is nothing to bootstrap from -- the position is thrown away
            # and replaced with a fresh game before anything can be observed --
            # so the honest value is the one already on hand, V(s) itself. That
            # makes the surprise zero and the return the critic's own estimate,
            # which teaches it nothing rather than teaching it a falsehood.
            delta = delta * (1.0 - buf.cut[t])
            running = delta + gamma * lam * alive * flip * running
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
        # Only the learner's own steps. The opponent's are needed to carry the
        # advantage across its turn, and would be off-policy in the loss: they
        # were sampled from a frozen snapshot, so their importance ratio is
        # measuring the wrong pair of distributions.
        #
        # Selected by *indexing the minibatch*, not by slicing the buffer up
        # front: an observation is 19,603 floats and slicing would copy the
        # whole rollout, which is the one thing that does not fit twice.
        keep = buf.mine.reshape(-1).nonzero(as_tuple=True)[0]
        flat_obs = buf.obs.reshape(-1, buf.obs.shape[-1])
        flat_actions = buf.actions.reshape(-1, HEADS)
        flat_masks = [m.reshape(-1, m.shape[-1]) for m in buf.masks]
        flat_logp = buf.logp.reshape(-1)
        flat_returns = returns.reshape(-1)
        # Normalised over the learner's rows alone, so the opponent's advantages
        # cannot shift the mean the learner's steps are measured against.
        #
        # Dividing by the batch's own spread rescales whatever is there to unit
        # size, including nothing. When the critic predicts well the residual is
        # noise, and normalising hands it a full-size step -- which is how the
        # first run came apart once `greedy` was saturated, and the same regime
        # an even matchup reaches from the other side. The floor keeps a
        # genuinely small advantage small; at 0.0 this is the old behaviour.
        flat_adv = torch.zeros_like(adv.reshape(-1))
        kept = adv.reshape(-1)[keep]
        spread = kept.std()
        flat_adv[keep] = ((kept - kept.mean())
                          / (spread.clamp(min=self.args.adv_floor) + 1e-8))

        total = keep.numel()
        stats = {"policy": 0.0, "value": 0.0, "entropy": 0.0, "kl": 0.0,
                 "clipped": 0.0, "n": 0, "spread": spread.item()}
        recent = 0.0
        for _ in range(self.args.epochs):
            order = keep[torch.randperm(total, device=self.device)]
            for start in range(0, total, self.args.minibatch):
                idx = order[start:start + self.args.minibatch]
                source, dest, kind, param = flat_actions[idx].unbind(dim=1)
                # The observations live on the host; everything else is already
                # here, so only this one gather has to cross.
                obs = flat_obs[idx.cpu()].to(self.device, non_blocking=True)
                logits, value = self.policy.evaluate_actions(
                    obs, source, dest, kind)

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
        # Measured once over the rollout, not per minibatch, so it is carried
        # past the averaging rather than through it.
        spread_of = stats.pop("spread")
        out = {k: v / n for k, v in stats.items()}
        out["stopped"] = stopped
        out["spread"] = spread_of
        return out

    @torch.no_grad()
    def recalibrate(self):
        """Refits the batch-norm running statistics to states now being visited.

        Cloning leaves them describing the human corpus, and the update runs in
        eval mode -- deliberately, so the rollout and the update normalise
        identically -- which means nothing ever refreshes them again. Measured
        on self-play states they were out by 1.75 standard deviations on
        average and 3.15 at worst, with variances off by up to six times, and
        the drift compounds with depth. Every gradient is then scaled by the
        wrong constant, and the further the policy moves the wronger it gets.

        Done *after* the update, never between rollout and update: the two must
        see the same statistics or every importance ratio compares two
        different policies, which is the failure that eval mode exists to stop.
        """
        flat = self.buffer.obs.reshape(-1, self.buffer.obs.shape[-1])
        # A minibatch, not the rollout: the whole buffer through the trunk at
        # once needs as much memory as the update does, and on a small card the
        # two together simply stop. Batch statistics over a few hundred states
        # are plenty, and the running average is what actually moves.
        pick = torch.randperm(flat.shape[0])[:self.args.minibatch]
        take = flat[pick].to(self.device, non_blocking=True)
        self.policy.train()
        self.policy.trunk(take)
        self.policy.eval()

    def promote(self):
        """Makes the current policy the opponent to beat.

        And re-fits the critic afterwards. Promotion moves the opponent's
        strength in one step, so every return the critic learned to predict
        shifts at once and it is stale in exactly the way the opening warm-up
        exists to fix -- advantages become mostly its own error, which
        normalising rescales to unit size. Running that warm-up only once, at
        the start, cost the first self-play run everything it had gained: it
        promoted at 78%, then fell to 20% against the weights it had just been.
        """
        self.frozen.load_state_dict(self.policy.state_dict())
        self.frozen.eval()
        self.frozen.requires_grad_(False)
        self.refreshes += 1
        self.recalibrating = self.args.refresh_warmup

    def save(self, path):
        path = ROOT / path
        path.parent.mkdir(parents=True, exist_ok=True)
        torch.save({"policy": self.policy.state_dict(), "config": self.config,
                    "teacher": "ppo", "map_name": self.env.map_name}, path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--init", default="checkpoints/bc-scaled.pt")
    # The best rollout score, not the last. Against a fixed opponent a run
    # saturates and then comes apart -- the first one peaked at 100% by
    # iteration 130 and finished at 80% -- so the final weights are reliably
    # not the ones worth keeping. The final set is still written, beside it.
    parser.add_argument("--out", default="checkpoints/ppo.pt")
    parser.add_argument("--min-games", type=int, default=100,
                        help="games a window needs to claim it is the best")
    parser.add_argument("--opponent", default="greedy",
                        choices=["greedy", "jakeman", "capturer", "random"])
    # A scripted opponent is a finite resource -- `greedy` is beaten 96%, and
    # the run that saturated it then spent seventy iterations unlearning itself.
    # Self-play replaces it with a frozen copy of the learner, refreshed
    # whenever the learner pulls far enough ahead, so there is always something
    # left to beat.
    parser.add_argument("--selfplay", action="store_true",
                        help="play a frozen copy of the policy, not a bot")
    parser.add_argument("--refresh-at", type=float, default=0.7,
                        help="score over a window that promotes the learner")
    parser.add_argument("--refresh-games", type=int, default=30,
                        help="games that window needs before it may promote")
    parser.add_argument("--refresh-warmup", type=int, default=10,
                        help="critic-only iterations after each promotion")
    parser.add_argument("--recalibrate", type=int, default=1,
                        help="refit batch-norm statistics to visited states")
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
    # Both of the above are per *order* by default, which is the wrong unit for
    # this game. The credit horizon is 1/(1 - gamma*lam) = 19 orders and a turn
    # is about 17, so credit does not reach across a single change of turn --
    # everything slower than one turn has to arrive through the critic alone.
    # With this set they apply once per turn instead: full Monte Carlo inside a
    # turn, the stated rates between turns, and a horizon of 1/(1 - gamma*lam)
    # *turns*. Pass a per-turn gamma with it -- 0.99, not 0.997.
    #
    # Still capped by `--steps`, which is why that wants raising too: a horizon
    # of seventeen turns is worth nothing in a rollout three turns long.
    parser.add_argument("--turn-discount", action="store_true",
                        help="discount per turn, not per order")
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
    # Advantages are normalised to unit scale, which turns a rollout the critic
    # already predicts well into full-size steps of noise. A floor under the
    # divisor keeps a small advantage small. Zero is the old behaviour; read
    # `spread` in the report to pick one, it is the raw scale being divided by.
    parser.add_argument("--adv-floor", type=float, default=0.0,
                        help="smallest advantage spread the update will divide by")
    parser.add_argument("--shaping", type=float, default=0.1)
    # What the shaping measures. `material` counts what is standing on the
    # board, which cannot see money: building a unit reads as a free gain of
    # its whole price and income is invisible until spent. `funds` adds the
    # bank. `worth` also values a property at the income it has left to pay
    # rather than a flat 5,000 -- a property taken on day 5 of 60 is worth ten
    # of one taken on day 55 -- which makes the opening numbers several times
    # larger, so bring `--shaping` down with it.
    parser.add_argument("--potential", default="material",
                        choices=["material", "funds", "worth"],
                        help="what the shaped reward measures a position by")
    # The day cap is the harness's rule, not the game's, and left undecided it
    # is a free half point. A policy will find that: over one run with a long
    # enough credit horizon to see it, the share of games stopped by the cap
    # went 13% -> 33% while the win rate fell twelve points and the reported
    # score did not move at all, because a draw counts half. This settles a
    # capped game on properties then material, the way a turn-limited league
    # game is settled. Off by default -- it changes what every rating means.
    parser.add_argument("--decide-cap", action="store_true",
                        help="settle a day-capped game instead of drawing it")
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
    against = "a frozen copy of itself" if args.selfplay else args.opponent
    print(f"vs {against}, {args.envs} envs x {args.steps} steps = "
          f"{per} orders per iteration, shaping {args.shaping}")

    start = time.perf_counter()
    seen = (0, 0, 0)
    best = -1.0
    for iteration in range(1, args.iterations + 1):
        last, last_actor = trainer.collect()
        adv, returns = trainer.advantages(last, last_actor)
        # Cloning never trains the value head -- its loss is four
        # cross-entropies and nothing else -- so PPO inherits a *random* critic.
        # Advantages are then mostly the critic's own error, and normalising
        # them rescales that noise to unit size, which diffuses the cloned
        # policy toward uniform. Fit the critic first, policy frozen.
        # A promotion moves the opponent, so the critic is re-fitted after one
        # exactly as it is at the start -- see Trainer.promote.
        warming = iteration <= args.value_warmup or trainer.recalibrating > 0
        if trainer.recalibrating > 0:
            trainer.recalibrating -= 1
        stats = trainer.update(adv, returns, critic_only=warming)
        if args.recalibrate:
            trainer.recalibrate()

        if warming and iteration == args.value_warmup:
            print(f"  critic warm-up done after {iteration} iterations "
                  f"(value loss {stats['value']:.4f})")
        if iteration % args.report_every == 0 or iteration == 1:
            # Windowed, not cumulative: `results` counts from construction, so a
            # running average of every game ever played would still be showing
            # the initial policy's score long after it had improved.
            #
            # The window *accumulates* until it is worth reading. A thirty-game
            # window carries about ten points, which is more than any real
            # improvement between two reports, so a bar set from one is decided
            # by luck: the run that kept `ctrl-greedy` read 93.1% on a window
            # whose rating is 83.1%, and the first JakeMan run kept a warm-up
            # window in which the policy had not moved at all.
            played, won, drawn = trainer.env.results
            games = played - seen[0]
            stalled = (drawn - seen[2]) / max(games, 1)
            score = ((won - seen[1]) + 0.5 * (drawn - seen[2])) / max(games, 1)
            needed = args.refresh_games if args.selfplay else args.min_games
            closed = games >= needed
            if closed:
                seen = (played, won, drawn)
            rate = iteration * per / (time.perf_counter() - start)
            if args.selfplay:
                # The score is against a moving opponent, so a high one means
                # the snapshot is stale rather than that the learner is good --
                # `best` would just track staleness. Promote instead: beating
                # the last generation convincingly is the improvement, and the
                # promoted weights are what gets kept.
                kept = not warming and closed and score >= args.refresh_at
                if kept:
                    trainer.promote()
                    trainer.save(args.out)
                    # Each generation is kept under its own name as well, so the
                    # ladder can be played against itself afterwards. Whether
                    # generation three actually beats generation one is the only
                    # honest measure of a self-play run, and it cannot be asked
                    # of a single overwritten file.
                    trainer.save(Path(args.out).with_name(
                        f"{Path(args.out).stem}-gen{trainer.refreshes}.pt"))
            else:
                # Keep the best weights seen, not the latest. A saturated
                # opponent stops producing advantage, normalisation rescales
                # what is left -- noise -- to a full-size step, and the policy
                # diffuses back down. A minimum window stops a lucky handful of
                # games claiming the checkpoint.
                kept = closed and score > best
                if kept:
                    best = score
                    trainer.save(args.out)
            mark = f"  gen {trainer.refreshes}" if kept else ""
            # `cut` is the share the day cap stopped. It is reported because it
            # is not a neutral number: those games carry no result, and how
            # many there are moved from 0.5% against `greedy` to 10.5% against
            # JakeMan -- the difference between the runs that gained and the
            # runs that came apart.
            print(f"  {iteration:>4}/{args.iterations}  "
                  f"score {score:.1%} over {games} games  cut {stalled:.0%}  "
                  f"| pi {stats['policy']:+.4f} v {stats['value']:.3f} "
                  f"H {stats['entropy']:.2f} kl {stats['kl']:.4f} "
                  f"clip {stats['clipped']:.2f} stop {stats['stopped']} "
                  f"spread {stats['spread']:.4f}  "
                  f"{rate:,.0f}/s{mark}")

    last = Path(args.out).with_name(Path(args.out).stem + "-last.pt")
    trainer.save(last)
    played, won, drawn = trainer.env.results
    print(f"\n{played} games during training, "
          f"score {(won + 0.5*drawn)/max(played,1):.1%}")
    if args.selfplay:
        print(f"{trainer.refreshes} generations, saved {ROOT / args.out}")
        print("Rate it against what it started from:")
        print(f"  py -3.12 python/evaluate.py --checkpoint {args.out} "
              f"--versus {args.init}")
    else:
        kept_any = "never improved on the start" if best < 0 else f"{best:.1%}"
        print(f"best rollout score {kept_any}, saved {ROOT / args.out}")
    print(f"final weights {ROOT / last} -- rate either with evaluate.py")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
