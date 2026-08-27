# Written replays carry the power meter, and a movement bug falls out

> A game where a power fired could not be written at all. It now writes,
> and one round-tripped power game verifies **100.0% over 7,255
> assertions**. Batch-verifying ten turned up a separate defect: the
> movement a power grants does not survive the trip.

**What was missing.** `write_replay.py` raised on the `Power` order kind,
so exactly the games worth watching were the ones it refused. Three
things were needed, each caught by the round trip rather than by reading:

| | |
|---|---|
| the `Power` payload | shaped like a real record (`564287.json`) |
| `playerID` | the *player* id, not the account — with the account id a re-parse drops the action silently, no error anywhere |
| the per-turn meter | `record.rs` snapshotted funds, units and buildings and no charge, so every turn wrote an empty bar |

The meter needed the recorder as well as the writer: `charge`, the
active-power flag, and — the part that is easy to miss — the **escalated**
thresholds. A star costs `1/5` more after every activation, so a pair
taken from the CO's star counts is right only until the first power
fires, and a reader reconstructs the wrong number of uses from it
afterwards. Writing the live costs took the same game from 31
divergences to none.

| one power game, re-parsed and verified | divergences |
|---|---|
| meter absent, thresholds static | 121 (98.33%) |
| meter written, thresholds static | 31 (99.57%) |
| meter written, thresholds live | **0 (100.00%)** |

**What the batch found.** Ten recorded games, five with powers:

| | agreement | clean |
|---|---|---|
| power-free | 99.992% | 4 of 5 |
| powers used | 99.288% | 1 of 5 |

Every remaining divergence in the power games is one kind —
`move-over-budget`, e.g. "route costs 4, engine allows 3" — and they
cluster in the games against JakeMan, which pops on sight. A re-read
game is not granting Adder's `+1`/`+2` movement where the original did,
so the activation and the movement bonus disagree about *when* the power
is on. That is a real defect and it is not the meter's: the meter is now
verified exact. Whether it sits in the writer's ordering, the reader, or
`power_move_bonus` at a turn boundary is open.
