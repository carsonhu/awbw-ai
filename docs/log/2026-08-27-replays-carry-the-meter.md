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

**What the batch found: a bug in the verifier itself.** Ten games, five
with powers, threw 187 `move-over-budget` divergences — "route costs 4,
engine allows 3" — clustered where JakeMan pops on sight. Not the
writer, and not the timing it looked like: the verifier keeps *its own
copy* of the movement-budget sum, and that copy was never taught about
powers.

```rust
// crates/awbw-replay/src/lib.rs, before
let move_points = (stats.move_points + co_move).clamp(0, 255)
// crates/awbw-engine/src/movement.rs, all along
let move_points = (stats.move_points + co_move
                   + state.power_move_bonus(unit.owner)).clamp(0, 255)
```

So every move a popped Adder made was marked over budget by a verifier
that was measuring the wrong thing. With the third term restored:

| ten recorded games | agreement | clean |
|---|---|---|
| powers used | **100.000%** | 5 of 5 |
| power-free | 99.992% | 4 of 5 |

**Nothing else moved.** Full corpus after the fix: power-free games
99.982%, 683 of 780 clean — the same figure as before it, since a game
with no power has no bonus to add. The two remaining divergences in the
recorded set are one `funds` disagreement and the rejected build that
follows from it, which is the corpus's own largest power-free class (93)
and not new here.
