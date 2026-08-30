# Which DefendPeace AI is worth porting

> Ran their own AIs against each other before porting one. The 2,537-line
> WallyAI came fourth of five; JakeMan won. Sample size flipped the answer once.

**Question.** The scripted ladder here tops out — `greedy` is beaten 96% — so a
stronger opponent has to come from somewhere, and DefendPeace ships five AIs.
Picking by file size would have chosen WallyAI, the largest and most elaborate.

**Setup.** DefendPeace ships `FightClub`, an AI-vs-AI harness. Its own
`AWBWMapConverter.py` turned AWBW map 119544 into their format, so the ranking
runs on the board the policy actually trains on. Ability-free CO (`NotACO`)
throughout, matching an engine that models no powers. Seats rotate per game.

Building it needs care: the project targets Java 8, vendors `lombok.jar`, and
`KaijuWarsUnits.java` imports a class Lombok generates *inside that same file* —
which Eclipse tolerates and plain `javac` cannot. `delombok --encoding=UTF-8`
first, then compile the expanded sources, and it builds clean.

**Result**, wins per AI, 20 games per pairing, all ten pairings:

| AI | lines | / 80 |
|---|---|---|
| JakeMan | 1,232 | **68** |
| Muriel | 1,134 | 64 |
| SpenderAI | 383 | 41 |
| WallyAI | 2,537 | 27 |
| InfantrySpamAI | 243 | 0 |

**Reading.** WallyAI loses 0–20 to JakeMan and 1–19 to Muriel. The likely reason
is that it is built around CO powers and `NotACO` takes them away — which is
exactly the condition this engine runs under, so for these purposes the ranking
stands however well Wally plays elsewhere.

> **Corrected 2026-08-30: the powers explanation is wrong.** WallyAI touches CO
> abilities in three lines — `PowerActivator` at `PHASE_TURN_START`,
> `PHASE_BUY`, `PHASE_TURN_END`. `JakeMan.java` has the same three and won;
> `Muriel.java` has none and came second. Under `NotACO` the winner and the
> fourth-placed AI lose the identical thing. What Wally has instead is
> conservatism by construction: `CALC = CalcType.PESSIMISTIC` for its own
> attacks against `OPTIMISTIC` for enemy threat, plus
> `AGGRO_EFFECT_THRESHOLD 0.55`, `YEET_FUNDS_BIAS 1000`,
> `BUILD_SCARE_PERCENT 50` and fund-banking to `MAX_BANK_FUNDS_FACTOR 2.5`.
> The choice of JakeMan stands; the reason given here for Wally's placement
> does not.

An earlier pass at 8 games per pairing put Muriel ahead of JakeMan 5–3. At 20 it
is JakeMan 15–5. Eight games was noise, the same way 200 evaluation games were
noise earlier in the project — and the cheap pass would have picked the wrong
target while looking decisive.

JakeMan is the one to port, with `AIUtils` and `AICombatUtils` (713 lines
together) underneath it.
