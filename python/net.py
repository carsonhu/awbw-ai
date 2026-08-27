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


class Residual(nn.Module):
    def __init__(self, channels: int):
        super().__init__()
        self.conv1 = nn.Conv2d(channels, channels, 3, padding=1, bias=False)
        self.norm1 = nn.BatchNorm2d(channels)
        self.conv2 = nn.Conv2d(channels, channels, 3, padding=1, bias=False)
        self.norm2 = nn.BatchNorm2d(channels)

    def forward(self, x):
        y = F.relu(self.norm1(self.conv1(x)), inplace=True)
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
    ):
        super().__init__()
        self.planes = planes
        self.globals_ = globals_
        self.height = height
        self.width = width
        self.tiles = height * width
        self.head_sizes = list(head_sizes)
        self.channels = channels

        # Globals are broadcast into their own planes. A board-wide scalar like
        # "day 14" is genuinely about every tile, and this is cheaper than
        # threading a second pathway through the trunk.
        self.stem = nn.Sequential(
            nn.Conv2d(planes + globals_, channels, 3, padding=1, bias=False),
            nn.BatchNorm2d(channels),
            nn.ReLU(inplace=True),
        )
        self.body = nn.Sequential(*[Residual(channels) for _ in range(blocks)])

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

        # The value head is unused by behaviour cloning and trained by PPO
        # later. It costs almost nothing to carry and saves a reshuffle of the
        # checkpoint format when fine-tuning starts.
        self.value = nn.Sequential(
            nn.Linear(channels, channels),
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
        _, _, pooled = self.trunk(obs)
        return self.value(pooled).squeeze(1)

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
            self.value(pooled).squeeze(1),
        )


def build(env, channels: int = 64, blocks: int = 6) -> Policy:
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
    )
