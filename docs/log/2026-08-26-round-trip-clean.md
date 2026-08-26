# The written replay reaches 100%, on two bugs about units that stopped existing

> 99.837% to 100.000%, 33 divergences to none. A unit killed by the
> counterattack was recorded as never having moved, and a transport was named
> by a slot number where everything else uses a stable id.

**Where it started.** Round-tripping four written games through
`prepare_replay.py` and the verifier gave 99.837%: 29 `move-fuel` and 4
`unit-position`. The real corpus has *no* move-fuel divergences at all
(`verification.md`), so this was ours, not the engine's.

## The dead attacker never moved

Every fuel divergence read the same way — the engine expected *less* fuel than
the record showed, as if the trip had not been paid for. It had not. The
divergent orders were all `Fire`, and the attacker's record held its origin
tile and its full fuel:

```
unit 800016 Infantry  route (7,6) -> (7,5) -> (7,4) -> (8,4)
  payload position (7,6)   fuel 99   hp 0
```

`Recorder::survivor` falls back to the record read *before* the action when a
combatant is gone, and for the attacker that record predates its own move. The
defender never moves, so it was right all along; only the mover was wrong. The
unit really did arrive, pay three fuel, and die there — but nothing can be read
back off a unit that no longer exists, so `before` now carries the cost of the
walk (`spent`, from the same reachability search that built the path) and
`dead_mover` writes the arrival on by hand. `Join` had the same fault and the
same fix: it already moved the mover to the destination, but not its fuel.

## A transport named by the wrong number

The four remaining divergences were one infantry left standing on the boat that
had just unloaded it. `unit_json` writes every id through `live`, the map that
keeps one number meaning one unit for a whole game — but the transport was
written as the raw `UnitId`, which is a *slot* the engine reuses. So
`transportID` named whichever unit later inherited the slot, the verifier could
not resolve it, and `do_unload` skipped the unload whole, leaving the passenger
on the transport's own tile. Both `Load` and `Unload` now go through `live`.

That is the third bug from recycled slots — after phantom divergences from a
stale mapping, and a build inheriting a casualty's id. Anything written into a
payload has to go through `live`.

| | divergences | agreement | clean games |
|---|---|---|---|
| before | 33 | 99.837% | 0 of 4 |
| after the dead-mover fix | 4 | 99.980% | 2 of 4 |
| **after both** | **0** | **100.000%** | **4 of 4** |

**Scope.** All three fixes are in the *writer* — `record.rs` and
`write_replay.py`. The engine is untouched, so no rating moves and no training
result is affected. Reading the real corpus does not go through this code at
all.
