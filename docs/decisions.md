# Decisions

> Questions already settled, and why — read before re-litigating one.

Several of these cost real effort to establish, and two were established twice
because the first answer looked reasonable and was wrong.

Append new entries at the bottom. Keep each to a few lines, and link the log
entry holding the numbers rather than reciting them here.

## Sourcing

**The wiki outranks the corpus for what a rule *is*.** The replay corpus is
excellent at finding bugs in how a rule is *implemented*, and a poor arbiter of
the rule itself. It briefly talked the engine out of reef concealment on six
observations out of 6,035 against a wiki page that states the rule plainly. When
a thin sample and the documentation disagree, the documentation wins.

**Game data is generated, never transcribed.** Damage, unit stats, terrain and
CO tables come from AWBW's own files and chart pages. Hand-typing them invites
silent errors in tables nobody re-reads.

## Scope

**COs: day-to-day abilities only, powers excluded.** Self-play uses the
ability-free CO with powers off, so the CO layer resolves to a constant and is
inert during training. It exists so the corpus can test combat at all —
competitive AWBW almost never fields an ability-free CO, and there is not one
Andy-vs-Andy game in the sample. Powers change stats mid-turn, the agent will
never use one, and modelling them buys nothing but a nicer number.

**Luck-range COs are implemented but unverified.** Nell, Flak and Jugger are
banned in Global League play, so no recorded game exercises that path.

## Engine

**Actions are single orders, not whole turns.** One environment step is one
order, ending in `EndTurn`. A whole-turn action space is not enumerable.

**Two enumeration paths, deliberately.** Enumerating every unit's orders after
every order is O(units²) per turn. `legal_actions_for` costs one reachability
search per step instead of one per unit — 38k vs 179k micro-steps/sec — and is
what a factorized policy wants. A test asserts the two agree.

**Repair adds raw HP and is charged per displayed point.** A unit at 5.5 HP
repairs to 7.5, not 8.0, and a player who cannot afford the full heal gets a
partial one. Matches DefendPeace's `healAtCost`.

**Concealment is re-tested at every step of a move**, not only where the unit
stops. Exempting tiles a mover merely crosses was tried and cost four points of
fog agreement.

**Reefs conceal ground and sea units**, exactly as woods do. See Sourcing.

## Training setup

**The board is A River Supreme (AWBW map 119544)**, committed at
`data/maps/119544.json`. Picked over the other popular league maps for the
largest pool of recorded standard games (~1,875), the smallest board of the
clean candidates (17x18), perfect 180-degree rotational symmetry, and — alone
among them — no terrain the engine leaves unimplemented. A real map also lets
the training environment and the corpus share a board, rules and opening.

**Bots must break ties at random.** The action list is built row-major, so
resolving equal-scoring options by enumeration order is a geographic bias in
disguise. On this map it turned a greedy mirror into 15/85; randomising ties
restored it to 40/60, and a random-vs-random control at 43% throughout proved
the board was never at fault. Any scripted teacher used for imitation needs
this, or the bias is cloned with it.

**Its starting units are deliberately asymmetric.** Blue Moon gets an infantry
Orange Star has no counterpart for, on top of the mirrored Black Boats — a
property of the map, not a loading bug. So the seats are *not* interchangeable,
and evaluation has to swap them.

**The observation carries CO information, because imitation needs it.** A policy
is learnable only if the input holds what the demonstrator conditioned on: Kanbei
attacks at +30% and Colin at -10%, so identical-looking boards carried
contradictory labels. Attack and defence ride as *planes*, applying where they
act; cost, capture, income and power charge as globals. Encoding modifiers rather
than a one-hot identity shares strength — Max's +20% sits by Grimm's +30%.

**Powers cost far less data than they look like they will.** 69% of games on the
training map use one, but only 9% of *orders* happen while one is active, so
those can simply be dropped from an imitation loss.

**Imitation labels check themselves**, twice: every translated order goes through
`Engine::check` in the position it was played in, and its code is decoded back to
confirm a masked policy could emit it. 98% pass the first, all pass the second,
and the failures are counted rather than hidden.

**Unloading is a free action, and not part of a move.** AWBW departs from the
cartridge: "transports may unload at any point in their turn, even if they have
already moved, and doing so does not end the unit's turn either" (the wiki). So
`Action::Unload` carries no destination, moving is a separate order, and a
transport that has already moved is still a legal source. Bundling the two, as
the cartridge does, made essentially every recorded unload read as illegal.

**A rejected order must still be forced onto the board.** The imitation cursor
used to fall back to ending the turn, handing play to the opponent mid-turn and
making every later order in that turn illegal too — 2.7% of a turn's first
orders rejected against 13.2% of its last. Forcing them through flattened it.

## Verification

**Each recorded turn is an independent test case.** Load the snapshot, replay
that turn's orders, diff against the next snapshot. Chaining a whole game
instead makes one wrong rule poison every turn after it.

**Damage is checked as a range, not a value.** AWBW rolls 0-9% luck per attack,
which is unreproducible. The recorded outcome must fall inside the engine's own
min/max spread, and HP is resynced from the record afterwards.

**Mid-turn HP is only known to the displayed point.** Combat payloads report
whole HP, so a unit attacked twice in one turn must be re-simulated against the
whole band it could be in, not a single value.

**Fog replays are full-information.** Their per-turn snapshots hold the entire
board, so they cannot test visibility. The per-player *move paths* can, and do.

**A defender's sight shrinks as its units die mid-turn.** Computing the
opponent's view once from the opening snapshot makes the engine look far too
sharp.

**Games need a ~60-day cap, not 30.** Measured with the `decisiveness` example:
random self-play reaches a real win *0 times in 40 games even at 120 days*, and
greedy mirrors 0/40 at a 30-day cap but 27/40 at 60. A win/loss reward is not
merely sparse for an untrained policy, it is absent — which is why training
starts from imitation, and why shaped reward carries the early signal.

**Orders are matched to snapshots by (player, day), not line index.** Some
replays are truncated, and index pairing silently attributed one player's whole
turn to their opponent.

**Replays stream; they are never written to disk as a dataset.** One observation
is 19,603 floats, so a million samples would be seventy-odd gigabytes, and the
engine regenerates one in microseconds. `ReplayTeacher` walks a game per slot,
staggered so a batch is not thirty-two views of day one.

**An order with an empty `Move` still moved nobody, not nothing.** AWBW writes
`"Move": []` when a unit acts where it stands, so a translator that tests
whether the key *exists* loses all of them — half of every capture and one
attack in eight. The cloned policy started captures, never finished one, and sat
at three properties for sixty days. A missing label is not just less data when
a whole kind of decision is the thing missing.

**A policy is rated by playing, not by predicting**, and the two barely track
each other: a point and a half of held-out accuracy across a change that nearly
tripled play strength (`log/2026-08-25-bc-scaling.md`). Judge with
`evaluate.py`; accuracy is for spotting a *broken* run, not a better one. Rate
over 400 games — 200 put ±2.6 on a number that moves by less.

**Batch norm must stay in eval mode during a PPO update.** Updating in train mode
while rolling out in eval mode makes the same weights give different logits on
the same observation — up to 24 apart, argmax flipped on half the rows — so every
importance ratio compares two policies. Gradients flow fine in eval mode.

**PPO inherits a random critic, so fit it first.** Cloning's loss is four
cross-entropies and touches no value head, so advantages start as mostly the
critic's own error, which normalising rescales to unit size. The warm-up steps a
*separate* optimizer over the value head: it shares the trunk, so fitting it
through the full one drags the policy along.

**A rating means nothing without the temperature it was sampled at.**
`bc-scaled` scores 19.0% against `greedy` at `--temperature 0.3` and 5.5% at
1.0 — the same weights, a factor of three apart. PPO samples on-policy and
cannot pick a temperature, so 1.0 is the number that predicts what RL starts
from. Quote both, or quote 1.0 (`log/2026-08-25-ppo-first-run.md`).

**A written replay is checked by reading it back, not by opening it.** Recorded
games return through `prepare_replay.py` and the verifier, which took agreement
99.47% → 99.91% by catching four faults a viewer would have rendered as merely
odd: a killed defender written as null (losing the target tile, so the attack
was dropped whole), Join naming the survivor where AWBW names the mover, HP
rounded to whole points where snapshots carry tenths, carried units omitted.

**The round trip cannot check a field our own parser ignores.** A replay that
verified at 99.9% still failed to open: `nextWeather` takes a weather *code*, so
`"Clear"` is rejected where `"C"` is fine. `symbol` is worse — it is the unit's
domain (G/S/M) in a game state and a per-type letter in an action payload, and
either looks plausible in the other's place. Diff written payloads against real
ones key by key and value by value; the verifier does not see this class at all.

**Games start on nothing, and the first player collects on day one.** The
environment opened on ten thousand — a tank before the first infantry moved —
and ran `begin_turn` only off `end_turn`, so seat one went without day-one income
too. All 300 recorded games start at zero with *both* sides on three thousand at
their first turn. Fixing it took the clone 5.5%→10.8% against `greedy`: it had
been cloned from one opening and tested in another.

**A saturated opponent unlearns the policy.** Once PPO beat `greedy` 100% the
critic had nothing left to predict, value loss fell to 0.001, and normalising
advantages rescaled what remained — noise — back to a full-size step. Entropy
climbed and the score fell 100% → 80% over the next seventy iterations. Keep the
best weights, not the last, and treat a fixed opponent as a finite resource.

**Reported entropy is not a health check.** It rises through a rollout with the
policy provably frozen, because a midgame position holds more units and more real
choices than an opening. It tracks where the games are, not what the policy is
becoming — read `kl` and `clip`.

**The opening fix moved the clone, not the RL checkpoints.** `ppo` rates
96.1% against `greedy` in the fixed environment against 96.2% before it, so
nothing trained by playing needs retraining for the opening alone. The clone
doubled because it is nothing but the corpus opening
(`log/2026-08-26-jakeman-ppo.md`).

**A rollout window is not a rating, and must not set the best-weights bar.**
A 38-game warm-up window claimed 57.9% for weights whose true rating is ~40%,
and no real improvement could clear the fluke — the "best" checkpoint the
guard kept was the starting weights. A window carries ±8 points; treat the
bar it sets accordingly, or rate candidates over enough games to mean it
(`log/2026-08-26-jakeman-ppo.md`).

**The greedy PPO recipe does not transfer to JakeMan.** From a 44% start —
the even matchup PPO is supposed to like — the defaults that took `greedy`
5.5%→96% never improved and decayed to 7.6%, with `kl` and `clip` nominal
throughout. Cause unestablished; a fixed opponent has now failed by
saturating (`greedy`) and by this (`log/2026-08-26-jakeman-ppo.md`).

**Batch-norm recalibration degrades the policy; run with `--recalibrate 0`.**
Twenty-five refits with the weights held still cost seventeen points — 33.5%
against the checkpoint it started from. It was on by default, so it ran in
every job since it was added, and it invalidates the JakeMan result. Damage
that does not arrive through the gradient is invisible to `kl` and `clip`
(`log/2026-08-26-recalibration.md`).

**Neither the opening fix nor the PPO update is broken.** The control that
reproduced the pre-fix recipe — `greedy` from `bc-scaled`, recalibration off —
climbed 7% → 93%. When a run degrades, suspect what surrounds the update
before the update (`log/2026-08-26-recalibration.md`).

**A record read before an action cannot describe a unit that moved during it.**
`survivor`'s fallback gave a killed attacker its origin tile and unspent fuel,
because the only copy left predates its own move. The walk's cost is now kept
in `before` and written on by hand; `Join` had it too
(`log/2026-08-26-round-trip-clean.md`).

**Every id in a payload goes through `live`, transports included.** `UnitId` is
a slot the engine reuses. A transport written raw named whichever unit
inherited the slot, so the unload could not be resolved and was dropped whole —
the third bug from recycled slots (`log/2026-08-26-round-trip-clean.md`).

**`combatInfo` carries five fields, not the unit row.** AWBW puts a delta there
— id, x, y, ammo, HP — and the full row only in `Move` and `Build`. Writing the
row instead re-places the mover, so an attacker killed by the counterattack sat
on the board at 0 HP forever (`log/2026-08-26-two-fixes.md`).

**JakeMan attacks before it travels, and this is not a tunable.** DefendPeace
returns `findBestAttack`'s pick outright and travels only with what did not
act. Scoring the two against each other let a walk outbid an attack 24 times in
412. Ratings against JakeMan from before `ATTACK_FLOOR` are against a weaker
bot: `ppo-jake2` fell 67.2% -> 58.4% (`log/2026-08-26-two-fixes.md`).

**JakeMan is beaten, and the scripted ladder is exhausted.** `ppo-jake2` rates
67.2% ±2.3 against JakeMan and 86.4% ±1.7 against `greedy` — the same run that
scored 7.6% with recalibration on. Nothing scripted is left to climb
(`log/2026-08-26-jakeman-beaten.md`).

**Self-play's learner drifts below its own frozen copy, cause unknown.** 35-41%
across recalibration off, an advantage floor and no shaping alike, so it
promotes nothing. The harness, the opening fix and the update are all cleared
by a working control; what that control never exercises is the two-player
advantage path (`log/2026-08-26-selfplay-drift.md`).

**Most of the source head's error is ordering, not judgement — and fixing that
bought nothing.** Its top pick is a unit the human moved somewhere in the turn
95.2% of the time, against 68.2% for a uniform pick, so its 44.7% top-1 is
mostly disagreement about sequence. Training against the turn's whole set
(`--source-set`) worked and changed no outcome: that head is not the bottleneck,
and its accuracy is not worth optimising (`log/2026-08-25-source-set.md`).
