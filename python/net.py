"""The policy network: one trunk, four autoregressive heads.

The action space is `source -> dest -> kind -> param`, and the obvious
implementation runs the trunk once per head. That costs four times as much for
nothing: the board does not change between the four choices, only the question
does. So the trunk runs once and each head reads the *same* feature map,
conditioned on what the earlier heads picked.

Three of the four choices are tiles, and a tile choice is a pointer: score every
tile against a query vector built from the context. That shares the trunk's
spatial features instead of learning a separate 306-way classifier per head, and
it is what lets a head generalise from "attack this tank" to "attack that one".

`param` is the exception, because it means three different things depending on
the kind: a target tile for an attack, a unit type for a build, a passenger and
direction for an unload. It gets both a pointer and a plain projection, added —
one path for the spatial case, one for the small-integer cases.
"""

import math

import torch
import torch.nn as nn
import torch.nn.functional as F

# Order must match `OrderKind` in the engine's encoding.rs.
KIND_NAMES = ["wait", "attack", "capture", "supply", "join", "load", "unload", "build"]
# The kinds whose `param` carries anything. For the rest the engine writes 0,
# and scoring a head on a constant would flatter it.
PARAM_KINDS = {1, 6, 7}  # attack, unload, build


def make_norm(kind: str, channels: int) -> nn.Module:
    """BatchNorm for the checkpoints that already exist, GroupNorm for new
    ones. BN's batch-coupled statistics have cost this project two real
    incidents -- recalibration silently eating seventeen points of play, and
    train-mode minibatch statistics flipping half a batch's argmaxes -- both
    the known RL failure class GN does not have: it normalises each sample
    alone, so there are no running statistics to refit and no train/eval gap.
    """
    if kind == "batch":
        return nn.BatchNorm2d(channels)
    if kind == "group":
        return nn.GroupNorm(8, channels)
    raise ValueError(f"unknown norm {kind!r}")


class Residual(nn.Module):
    """A residual block, optionally with a global-pooling bias.

    A convolution is local: derived global state -- the material balance, the
    meter race -- has to be re-synthesised layer by layer and reaches the
    heads only through the final pooling. KataGo found this exact weakness
    in pure-conv trunks and fixed it cheaply: pool the block's own features,
    project to per-channel biases, add back in. `pool_bias` puts that branch
    between the block's two convolutions.
    """

    def __init__(self, channels: int, norm: str = "batch",
                 pool_bias: bool = False):
        super().__init__()
        self.conv1 = nn.Conv2d(channels, channels, 3, padding=1, bias=False)
        self.norm1 = make_norm(norm, channels)
        self.conv2 = nn.Conv2d(channels, channels, 3, padding=1, bias=False)
        self.norm2 = make_norm(norm, channels)
        self.bias = nn.Linear(2 * channels, channels) if pool_bias else None

    def forward(self, x):
        y = F.relu(self.norm1(self.conv1(x)), inplace=True)
        if self.bias is not None:
            pooled = torch.cat([y.mean(dim=(2, 3)), y.amax(dim=(2, 3))], dim=1)
            y = y + self.bias(pooled).unsqueeze(-1).unsqueeze(-1)
        y = self.norm2(self.conv2(y))
        return F.relu(x + y, inplace=True)


class Policy(nn.Module):
    """Maps an observation to the four heads' logits.

    `planes`, `globals_`, `height`, `width` and the head sizes all come from the
    environment rather than being hard-coded, so a change to the encoding shows
    up as a shape error at construction instead of as silent nonsense.
    """

    def __init__(
        self,
        planes: int,
        globals_: int,
        height: int,
        width: int,
        head_sizes,
        channels: int = 64,
        blocks: int = 6,
        norm: str = "batch",
        pool_bias: bool = False,
        value_pool: str = "mean",
    ):
        super().__init__()
        self.planes = planes
        self.globals_ = globals_
        self.height = height
        self.width = width
        self.tiles = height * width
        self.head_sizes = list(head_sizes)
        self.channels = channels
        self.norm = norm
        self.pool_bias = pool_bias
        self.value_pool = value_pool

        # Globals are broadcast into their own planes. A board-wide scalar like
        # "day 14" is genuinely about every tile, and this is cheaper than
        # threading a second pathway through the trunk.
        self.stem = nn.Sequential(
            nn.Conv2d(planes + globals_, channels, 3, padding=1, bias=False),
            make_norm(norm, channels),
            nn.ReLU(inplace=True),
        )
        # The pooling-bias branch on alternating blocks, KataGo's spacing:
        # often enough that global state stays a step away, cheap enough
        # that the trunk stays convolutional.
        self.body = nn.Sequential(*[
            Residual(channels, norm, pool_bias and i % 2 == 1)
            for i in range(blocks)
        ])

        self.source_tile = nn.Conv2d(channels, 1, 1)
        # End-turn and the CO powers are not tiles, so they get their own
        # logits off pooled features — however many the encoding declares.
        self.source_end = nn.Linear(channels, self.head_sizes[0] - self.tiles)

        self.dest_key = nn.Conv2d(channels, channels, 1)
        self.dest_query = nn.Linear(2 * channels, channels)

        self.kind = nn.Sequential(
            nn.Linear(3 * channels, channels),
            nn.ReLU(inplace=True),
            nn.Linear(channels, self.head_sizes[2]),
        )

        self.kind_embed = nn.Embedding(self.head_sizes[2], channels)
        self.param_key = nn.Conv2d(channels, channels, 1)
        self.param_query = nn.Sequential(
            nn.Linear(4 * channels, channels),
            nn.ReLU(inplace=True),
            nn.Linear(channels, channels),
        )
        self.param_direct = nn.Linear(4 * channels, self.head_sizes[3])

        # The value head is trained by PPO, and by cloning too when
        # `--value-outcomes` supplies recorded game results -- `bc-net2` was
        # made that way. Without them cloning leaves it at its init, which is
        # what PPO's warm-up exists for.
        # Mean pooling alone tells the value head the average tile, and a
        # game can hinge on the best or worst one; mean+max is KataGo's
        # cheap version of the fix. Heads keep the mean-only `pooled`.
        value_in = 2 * channels if value_pool == "meanmax" else channels
        self.value = nn.Sequential(
            nn.Linear(value_in, channels),
            nn.ReLU(inplace=True),
            nn.Linear(channels, 1),
        )

    def trunk(self, obs):
        """Feature map and pooled summary from a flat observation batch."""
        n = obs.shape[0]
        split = self.planes * self.tiles
        board = obs[:, :split].view(n, self.planes, self.height, self.width)
        scalars = obs[:, split:].view(n, self.globals_, 1, 1)
        scalars = scalars.expand(n, self.globals_, self.height, self.width)

        features = self.body(self.stem(torch.cat([board, scalars], dim=1)))
        flat = features.flatten(2)  # (N, C, tiles)
        pooled = flat.mean(dim=2)  # (N, C)
        return features, flat, pooled

    def value_features(self, flat, pooled):
        """What the value head reads: `pooled`, widened when configured."""
        if self.value_pool == "meanmax":
            return torch.cat([pooled, flat.amax(dim=2)], dim=1)
        return pooled

    @staticmethod
    def _pointer(keys, query):
        """Scores every tile against a query. Scaled like attention, so the
        logits do not blow up with channel count."""
        return torch.einsum("nct,nc->nt", keys, query) / math.sqrt(keys.shape[1])

    def _gather(self, flat, pooled, index):
        """The feature vector at a chosen tile, falling back to the pooled
        summary for the end-turn index, which is not a tile at all."""
        tiles = flat.shape[2]
        safe = index.clamp(max=tiles - 1)
        picked = flat.gather(2, safe.view(-1, 1, 1).expand(-1, flat.shape[1], 1))
        picked = picked.squeeze(2)
        off_board = (index >= tiles).unsqueeze(1)
        return torch.where(off_board, pooled, picked)

    # The heads are separate methods, not one call, because playing a game has
    # to interleave them with the engine's masks: what destinations are legal
    # depends on which source was chosen, and the mask for the next head can
    # only be asked for once the previous one is decided.

    def source_logits(self, features, pooled):
        tiles = self.source_tile(features).flatten(1)
        return torch.cat([tiles, self.source_end(pooled)], dim=1)

    def dest_logits(self, features, flat, pooled, source):
        f_source = self._gather(flat, pooled, source)
        query = self.dest_query(torch.cat([f_source, pooled], dim=1))
        return self._pointer(self.dest_key(features).flatten(2), query)

    def context_of(self, flat, pooled, source, dest):
        return torch.cat(
            [
                self._gather(flat, pooled, source),
                self._gather(flat, pooled, dest),
                pooled,
            ],
            dim=1,
        )

    def kind_logits(self, context):
        return self.kind(context)

    def param_logits(self, features, context, kind):
        full = torch.cat([context, self.kind_embed(kind)], dim=1)
        keys = self.param_key(features).flatten(2)
        return self._pointer(keys, self.param_query(full)) + self.param_direct(full)

    def forward(self, obs, source=None, dest=None, kind=None):
        """All four heads' logits in one pass.

        Pass the *true* earlier choices to condition on them — teacher forcing,
        which is what cloning wants. Left as `None`, each head conditions on the
        previous head's own best guess.
        """
        features, flat, pooled = self.trunk(obs)

        source_logits = self.source_logits(features, pooled)
        if source is None:
            source = source_logits.argmax(dim=1)

        dest_logits = self.dest_logits(features, flat, pooled, source)
        if dest is None:
            dest = dest_logits.argmax(dim=1)

        context = self.context_of(flat, pooled, source, dest)
        kind_logits = self.kind_logits(context)
        if kind is None:
            kind = kind_logits.argmax(dim=1)

        return (
            source_logits,
            dest_logits,
            kind_logits,
            self.param_logits(features, context, kind),
        )

    def value_of(self, obs):
        _, flat, pooled = self.trunk(obs)
        return self.value(self.value_features(flat, pooled)).squeeze(1)

    def evaluate_actions(self, obs, source, dest, kind):
        """Logits for orders already chosen, plus the value, in one trunk pass.

        What a PPO update needs: the heads are re-scored against the actions
        the rollout actually took, so every head conditions on the *taken*
        earlier choices rather than on a fresh guess. Sharing the trunk with
        the value head halves the update's cost.
        """
        features, flat, pooled = self.trunk(obs)
        context = self.context_of(flat, pooled, source, dest)
        return (
            (
                self.source_logits(features, pooled),
                self.dest_logits(features, flat, pooled, source),
                self.kind_logits(context),
                self.param_logits(features, context, kind),
            ),
            self.value(self.value_features(flat, pooled)).squeeze(1),
        )


def build(env, channels: int = 64, blocks: int = 6, norm: str = "batch",
          pool_bias: bool = False, value_pool: str = "mean") -> Policy:
    """A policy shaped to whatever the environment actually emits."""
    height, width = env.board_shape
    tiles = height * width
    globals_ = env.observation_size - (env.observation_size // tiles) * tiles
    planes = (env.observation_size - globals_) // tiles
    return Policy(
        planes=planes,
        globals_=globals_,
        height=height,
        width=width,
        head_sizes=env.action_sizes,
        channels=channels,
        blocks=blocks,
        norm=norm,
        pool_bias=pool_bias,
        value_pool=value_pool,
    )


def from_config(config) -> Policy:
    """A policy matching a checkpoint's stored config. The three net-v2
    fields default to the values every pre-v2 checkpoint was built with, so
    old checkpoints load without carrying them."""
    return Policy(
        planes=config["planes"],
        globals_=config["globals"],
        height=config["height"],
        width=config["width"],
        head_sizes=config["head_sizes"],
        channels=config["channels"],
        blocks=config["blocks"],
        norm=config.get("norm", "batch"),
        pool_bias=config.get("pool_bias", False),
        value_pool=config.get("value_pool", "mean"),
    )
