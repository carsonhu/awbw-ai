# The recipe is right, and the run is small

> A review against the two self-play PPO bots that took #1 on Pokemon
> Showdown ladders finds this project's design in their family -- PPO, a
> league of past selves, a small net, search only at test time -- and its
> experience budget four to five orders of magnitude below theirs. A league
> arm plays 1,833 games; mikumiku37 played ~330 million. Every update here
> averages ~9 finished games, theirs 8,192 battles. "Decays from level" is
> what small-batch noise looks like, and it is now a grid (`GRID=batch`).

**The comparison.** From the authors' own write-ups (Jaxcalibur:
jaxcalibur.github.io; mikumiku37: Smogon thread 3789199). Both are
single-format specialists, so one map is not the divergence.

| | Jaxcalibur (Gen 9 randbats) | mikumiku37 (VGC Reg M-C) | this project, one league arm |
|---|---|---|---|
| result | #1 on and off, peak 2557 | #1 in its 358th game | -- |
| human data | none | none (1,260 public teams only) | BC on ~2,400 games, KL anchor |
| reward | terminal win/loss | terminal only | terminal + 0.1 shaping + cap tiebreak |
| games trained | ~100M, a week on one H100 | ~330M, 48h on one 5090 | 1,833, ~1.5h of a shared 4090 |
| rows per update | not stated | 8,192 battles x 32 = 262k | 32 envs x 256 = 8,192 |
| collection speed | GPU-vmapped JAX engine | ~100k steps/s with inference | ~320-400 orders/s |
| opponents | self-play | ~50% current self, rest past selves by PFSP | frozen pool + 25% JakeMan |
| net | 8.5M transformer | 8.7M transformer | 1.73M resnet |
| search | test-time pUCT, worth 100-150 Elo | test-time 1-ply, regret matching | none (one-ply rerank built, off) |

A Pokemon battle is tens of decisions and an AWBW game here is ~900 orders,
so in decisions the gap is wider still. Neither bot needed human data or
shaping -- but neither faces a game random play cannot finish
(`decisions.md`: 0 of 40 random games decided at 120 days), so the clone and
the anchor stay. What does not survive the comparison is the batch.

**Throughput, measured.** Local 1660 Ti, three iterations each, self-play
against two pool members from `mixjm-s43`:

| `--envs` | orders/s |
|---|---|
| 32 | 378 |
| 128 | ~460 |
| 256 | ~495 |

Batching more games barely helps, because the cost is per row, not per
step. cProfile over two iterations: update 21.6s, collect 13.0s; the engine
itself runs 73k steps/s and is not the bound. After the changes below the
report line carries the split: the update is **56%** of wall time in
self-play and **82%** against JakeMan with the anchor, which adds a forward
pass per minibatch.

**What changed in `ppo.py`.**
- The frozen side runs only on its own rows. It ran over the whole batch,
  bot rows included, and its sample was then discarded on the learner's
  rows. Logits on the subset match the full batch to 9.5e-7. Worth 5%
  (378 -> 397/s), because collection is the smaller half.
- Masks are fetched once per head and shared by both sides. The frozen side
  asked the env again, and `offered` counted every power offer twice.
- `--amp` runs the trunk in bfloat16, heads in float32. Against fp32 on 64
  midgame positions: per-order log-prob moves 0.006 mean, 0.020 max (a 2%
  ratio, against a 0.2 clip), argmax agrees on every row of every head,
  |dV| <= 0.026. Off by default: no card here has bf16 tensor cores, convs
  already run TF32 on Ampere and later, and the strength cost is unmeasured.
- The report prints `upd NN%`, the update's share of wall time.

**What follows.**
1. `GRID=league2` runs as committed -- it was designed before this review.
2. `GRID=batch` asks whether the decay is noise: the `lg2ctl` recipe at 32
   and 128 envs, minibatch scaled with it so both groups take 96 gradient
   steps an iteration. Minibatch 1024 peaks at 5.9 GiB against 1.5.
3. Before trusting `--amp`, time three iterations with and without it on the
   rented card, then rate one arm both ways.
4. Live self in the league. mikumiku37 seats the current policy in half its
   games; both seats are then on-policy and every order trains, where a
   frozen opponent's half of the batch is discarded.
5. Terminal-only reward once self-play leads. Both bots train on win/loss
   alone; shaping is what took Anti-Air to 35%, and the cap tiebreak is what
   search farmed.

**More human data: the map is already spent.** The clone trains on 2,945
prepared games; the archive (`data/mooo-119544.json`) lists 2,976 on this
board, every CO already in (only power turns of unmodelled COs drop). The bots
say human data is a bootstrap, not the engine of strength, so it ranks below
the batch anyway. What is left, in order of cost: the 180-degree rotation,
which maps the board onto itself and gives every recorded position a second,
geometrically distinct copy (actions rotate with it); then off-map games as a
*tactics* prior only -- pretrain, fine-tune here, never train RL off-map --
and only after a clone trained without this map is rated on it, which prices
how much of the skill is map-general before an encoding break is spent on
it. Past that, the data that scales is the policy's own games.

**What it does not settle.** Whether the batch is the cause: that is the
grid. Whether a 4090 bound by the update gains the 2x bf16 promises, or is
bound by its CPU share -- `upd NN%` will say.
