# Decisions

Questions that are settled, and why. Read before re-deciding one — several of
these cost real effort to establish, and two were established twice because the
first answer looked reasonable and was wrong.

Append new entries at the bottom. Keep each to a few lines.

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
`data/maps/119544.json`. Picked over the other popular league maps because it
has the largest pool of recorded standard games (~1,875), the smallest board of
the clean candidates (17x18), perfect 180-degree rotational symmetry, and —
alone among the popular maps — no terrain the engine leaves unimplemented. A
real map also removes the mismatch between the training environment and the
replay corpus: same board, same rules, same opening.

**Bots must break ties at random.** The action list is built row-major, so
resolving equal-scoring options by enumeration order is a geographic bias in
disguise. On this map it turned a greedy mirror match into 15/85; randomising
ties restored it to 40/60. Verified against a random-vs-random control, which
sat at 43% throughout and proved the board and engine were never at fault. Any
scripted teacher used for imitation needs this, or the bias is cloned with it.

**Its starting units are deliberately asymmetric.** Blue Moon gets an infantry
Orange Star has no counterpart for, on top of the mirrored Black Boats. That is
a property of the map, not a loading bug, and it means the seats are *not*
interchangeable — evaluation has to swap them, which the arena does.

**The observation carries CO information, because imitation needs it.** A policy
is only learnable if the input contains what the demonstrator was conditioning
on. Kanbei attacks at +30% and Colin at -10%, a forty-point swing that flips
which trades are correct, and Colin's 8,000 funds buys what Andy's 10,000 does —
so identical-looking boards carried contradictory labels. The CO's attack and
defence modifiers ride as *planes*, applying exactly where they act, and its
cost, capture, income and power-meter values as globals. Encoding the modifiers
rather than a one-hot CO identity shares statistical strength: Max's +20% and
Grimm's +30% sit next to each other, and no single CO needs its own data.

**Powers cost far less data than they look like they will.** 69% of games on the
training map use one, but only 7.4% of *turns* and 10.9% of *orders* happen while
one is active, so those orders can simply be dropped from an imitation loss.

**Imitation labels check themselves.** Every translated human order is put
through `Engine::check` in the position it was played in before being offered as
a label. 94.5% pass across 366 non-fog games and 128k orders, which is the
strongest available evidence that the translation and the engine agree. The
failures are counted, not hidden, and the flag lets a trainer drop them.

**Unloading is a free action, and not part of a move.** AWBW departs from the
cartridge: "transports may unload at any point in their turn, even if they have
already moved, and doing so does not end the unit's turn either" (the wiki). So
`Action::Unload` carries no destination — the transport unloads from where it
stands, moving is a separate order, and a transport that has already moved is
still a legal source. Bundling the two, as the cartridge does, made essentially
every recorded unload read as illegal.

**A rejected order must still be forced onto the board.** The imitation cursor
used to fall back to ending the turn, which handed play to the opponent
mid-turn and made every later order in that turn illegal too. The damage was
visible as a gradient — 2.7% of a turn's first orders rejected against 13.2% of
its last — and disappeared once rejected orders were forced through instead.

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
greedy mirrors 0/40 at a 30-day cap but 27/40 at 60. So a win/loss reward is not
merely sparse for an untrained policy, it is absent — which is why training has
to start from imitation rather than from scratch, and why shaped reward carries
the early signal.

**Orders are matched to snapshots by (player, day), not line index.** Some
replays are truncated, and index pairing silently attributed one player's whole
turn to their opponent.
