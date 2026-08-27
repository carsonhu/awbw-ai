# The meter meets the oracle, and Sonja had been blinding the verifier

> Phase 2 of Adder powers: the verifier now applies recorded `Power` actions
> and checks the meter turn by turn. Timing verifies perfectly, charge to the
> digit in power-free games — and a Sonja bug found on the way pushes the
> power-free split to its best agreement ever, 99.982%.

**The Sonja bug.** Combat is recorded once per player's view, and Sonja hides
her HP — the blinded side's record reads `"?"`. `unwrap_vision` took the
*first* view, so in Sonja games whole `Fire` actions silently failed to
parse: no damage check, no HP snap, nothing. Picking the view that parses
took the power-free split from 549 divergences to 199 before the meter work
even started — `unit-hp` fell 146 to 30 — and it had been hiding since
before powers existed. Power-free agreement now 99.982%, 683 of 780 clean.

**Timing is exact.** The `co_power_on` flag is checked every turn against
the simulated active power: zero divergences across all 2,569 games —
activation mid-turn, running through the opponent's turn, expiry at the
owner's next start.

**Charge is exact to within the record's own precision.** The record carries
displayed HP, but the server charges on precise damage it does not preserve,
so each strike whose numbers could round either way is uncertain by one
displayed point; the verifier banks the displayed-HP difference and carries
that slack. Under it, power-free games show **zero** charge divergences.
The convention itself is pinned to within a point: pure display-diff
over-banks (game 601804 turn 26, one Black Boat point exactly), pure
floor-of-precise under-banks kills. The engine uses floor-of-precise —
DefendPeace's sourced reading, symmetric in self-play either way.

**What remains diverges for known reasons.** 178 charge mismatches and 3
refused activations, all in powered games of COs whose *effects* are
unmodelled (a Jess refuel changes nothing here, but a Sasha or Sonja power
does) — the same expected class as their movement divergences. The SCOP cap
assumption never bound in a checkable way; it stays marked unconfirmed.

`--kind <name>` on `verify-replays` now prints every divergence of one kind
across the corpus, which is how each of these patterns was isolated.
