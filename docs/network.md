# Network

> The policy network, where each piece sits in the literature, and what to
> change next, in order.

## What it is

One trunk, four autoregressive heads, one *order* per step. Deployed shape:
96 channels, 8 residual blocks, 1.73M parameters (`build()`'s 64x6 is only
the default; the checkpoint config is authoritative).

```
obs (70 planes + 23 globals broadcast, 18x17)
  -> 3x3 stem -> 8 residual blocks (96ch, BatchNorm)
  -> features (96x306), pooled = spatial mean
source: 1x1 conv per-tile logit + linear off pooled for end-turn/COP/SCOP
dest:   pointer -- tiles scored against query(f_source, pooled)
kind:   MLP(f_source, f_dest, pooled) -> 8
param:  pointer + direct projection, conditioned on a kind embedding
value:  MLP(pooled) -> scalar
```

Heads are masked by the engine and conditioned on the taken earlier choices;
sampling interleaves with the env because each mask depends on the last pick.

## Where it sits in the literature

**The trunk is AlphaZero's, miniaturized.** Conv stem, residual tower,
per-tile policy via 1x1 conv is the AZ/KataGo lineage; KataGo's early-run
nets (b6c96) are this size class. Nominal receptive field covers the board.

**The heads are AlphaStar's action decoder, scaled down.** Factorize the
joint action, make tile choices pointers (attention against the shared
feature map) so "attack this tank" generalises to "attack that one".
OpenAI Five factorized the same way. This is the consensus solution for
structured action spaces.

**The recipe converged on AlphaStar's independently**, failure by failure:
BC init from human games; KL to the clone during RL (`--anchor`, built for
the measured composition drift -- their medicine for the same disease);
exploiters and a league; PFSP collapsing onto the learner's own lineage,
fixed by seat caps as they fixed it by matchmaking weights. The convergence
is the strongest available evidence the system shape is right.

**Deliberate divergences, all justified at this scale.** No entity
transformer: AWBW is 306 tiles and <=50 units, and the threat planes
precompute the pairwise damage arithmetic a transformer would have to
rediscover -- worth 63->93 against greedy in one lineage swap. No LSTM:
no-fog AWBW is fully observed and the observation is Markov; memory becomes
mandatory only when fog does (AlphaStar's core; DeepNash for Stratego-grade
information). No search: stochastic combat means chance nodes and games run
~500 orders -- but the engine's 130k micro-steps/s is a good substrate for
low-budget Gumbel-style search later, the largest untouched lever here.

## Known deficiencies, in priority order

**1. BatchNorm is wrong for this regime, twice measured.** Recalibration
silently cost seventeen points; train-mode minibatch statistics moved
logits by 24 and flipped half the argmaxes. Both are BN's batch-coupled
statistics interacting with policy-gradient training -- a known RL failure
class; AlphaStar and OpenAI Five use layer-type norms, KataGo moved off
plain BN. GroupNorm is a drop-in swap, per-sample, and retires the whole
bug class for the cost of one re-clone.

**2. Surgery across observation breaks -- built (`surgery.py`).** Every
plane-count change had orphaned every checkpoint, the most expensive event
class in the project; OpenAI Five transferred weights instead. For *added*
planes the stem conv widens with zero-init channels and the function is
unchanged -- verified against `PlaneSlice` to 2e-5 on real play. Additions
only: a change that redefines planes (threat v1->v2) still costs the book.

**3. Auxiliary value targets, all that is left of the global bundle.**
Derived global state (material balance, meter race) must be re-synthesized
by 3x3 convs. Two of KataGo's three answers shipped inside net-v2 and are
in `bc-net2`'s stored config -- pooled bias on alternating blocks,
mean+max for the value head. The third has not: auxiliary targets, worth
roughly double training efficiency there. The AWBW analogs (final property
differential, income at t+N) are already computed by the shaping code, so
what remains is one head and one re-clone.

**4. Capacity last.** Nothing measured says 96x8 is the bound; widen when a
rented grid makes retrains cheap. The net is not a contract boundary -- a
net change costs a retrain, an encoding change costs every checkpoint.
