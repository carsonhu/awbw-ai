# Wally is destroyed, not outbuilt, and the mirrors clear the board

> Re-running the DefendPeace ranking with the control it never had.
> `2026-08-25-defendpeace-ai-ranking.md` blamed WallyAI's fourth place on
> `NotACO` removing the powers it is built around. `JakeMan.java` has the same
> three `PowerActivator` calls and won, `Muriel.java` has none and came second,
> and the games say the loss is not economic at all: Wally dies to JakeMan
> **20 of 20 by CONQUEST at 14.1 turns**, faster than a JakeMan mirror finishes
> and a third quicker than a Wally mirror. The pick of JakeMan stands.

**Setup.** `FightClub`'s `GameSet` driven from a scratch class in package `AI`,
so no tracked file changed, on the local checkout at `/f/awbw/DefendPeace`
(`29dd1fb`). 20 games a pairing, `NotACO` both seats, seats rotating, on the
converted `A River Supreme` — the ranking's own conditions. Every score below
reproduces the original table exactly, which is what licenses reading the end
conditions as belonging to the same measurement.

| pairing | score | end conditions | turns (mean) |
|---|---|---|---|
| JakeMan v JakeMan | 9–11 | 19 CONQUEST, 1 TURN_LIMIT | 14–22 (15.6) |
| JakeMan v Wally | **20–0** | **20 CONQUEST, 0 TURN_LIMIT** | 13–18 (**14.1**) |
| JakeMan v Muriel | 15–5 | 7 CONQUEST, **13 TURN_LIMIT** | 14–37 (20.3) |
| Muriel v Wally | 19–1 | 13 CONQUEST, 7 TURN_LIMIT | 15–35 (21.65) |
| Wally v Wally | 13–7 | 20 CONQUEST | 15–47 (20.75) |

**Powers were never the variable.** WallyAI touches CO abilities in three
lines, `PowerActivator` at `PHASE_TURN_START`, `PHASE_BUY` and `PHASE_TURN_END`
— modules that no-op under `NotACO`. JakeMan has the identical three. Muriel
has zero and finished second. Whatever `NotACO` costs, it costs the winner and
the fourth place the same, so it cannot order them.

**The loss is the army, not the economy.** `FightClub`'s `TURN_LIMIT` is a
unit-count mercy rule — after turn `max(width, height)`, here 18, a side
holding more than twice the opponent's units wins. Against JakeMan it never
fires, because Wally is dead by turn 14. That also kills the reading its own
constants invite: `BANK_EFFICIENCY_FACTOR 1.7` and `MAX_BANK_FUNDS_FACTOR 2.5`
hoard funds, hoarding means fewer units, and fewer units is exactly the
condition that never arrives.

**The mirrors clear the map and the converter.** A JakeMan mirror runs 15.6
turns and a Wally mirror 20.75, up to 47 — Wally is slow by nature, as "can be
overly timid" advertises. It still dies to JakeMan in 14.1, quicker than the
board's own pace between two copies of the winner. Nothing about `A River
Supreme` or its conversion produces that; only the matchup does.

**Two adjacent ranks, opposite failures.** Muriel loses 13 of 20 on
`TURN_LIMIT` at 20.3 turns — it survives and is outproduced. Wally loses 20 of
20 on `CONQUEST`. Against Muriel, Wally lasts 21.65 turns and takes 7 games to
the limit, so the collapse is not general: it is what a committed opponent does
to a policy whose combat math is `CalcType.PESSIMISTIC` on its own attacks and
`OPTIMISTIC` on enemy threat, behind `AGGRO_EFFECT_THRESHOLD 0.55`.

**Unresolved.** `CONQUEST` covers HQ capture and total unit loss alike and the
harness does not separate them, so "destroyed" is established and "HQ rush" is
not. The 14-turn clustering is not evidence either way — the JakeMan mirror
clusters at 14–15 too.
