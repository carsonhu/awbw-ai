# Join refunds are counted on screen, not in hundredths

> The `funds` class was the largest power-free divergence in the corpus.
> Part of it was a real rule error: joining refunded the overflow computed
> from summed hundredths, where AWBW rounds each unit to displayed HP
> first. Fixing it: 93 -> 88 funds, 683 -> 687 clean, `build-illegal` to 0.

**How it surfaced.** A written replay disagreed with the engine that
produced it. Chasing that found turns whose builds cost more than the
snapshot they opened with held — 20,000 opening a turn that spent 27,000
— and the gap was exactly 7,000, a Tank, in every case. Not a phantom
build: a mid-turn *join refund* the probe had not accounted for. The
recorder was right and the probe was wrong, but the refund itself was
not.

**The rule.** Merging two units refunds the HP above ten at the unit's
per-point price. The engine summed `hp100` and rounded the overflow once;
AWBW's records carry HP as 1-10 and nothing finer ever reaches its funds,
so it rounds each unit *first*. The two agree whenever either unit is
whole, and drift otherwise:

| joining / target | summed hundredths | displayed HP |
|---|---|---|
| 10.0 into 5.5 | 6 points | 6 points |
| 4.5 into 7.5 | 2 points | **3 points** |

**Measured, whole corpus, power-free games:**

| | funds | clean of 780 |
|---|---|---|
| summed hundredths, rounded once | 93 | 683 |
| **displayed HP, each rounded up** | **88** | **687** |
| displayed HP, each rounded down | 149 | 648 |

Rounding down is decisively worse, which also rules out the reading that
AWBW truncates. `build-illegal` fell from 1 to 0: a build the engine had
called unaffordable was affordable once the refund was right.

**What the remaining 88 are, and why they are not a rule.** They are
one displayed point of refund, +100 on an Infantry join and +700 on a
Tank, with the engine high. That is what a join does when our HP for a
unit sits a fraction above AWBW's — and the verifier already tolerates
exactly that gap for HP itself, as `luck slack` (168,709 of them). It
does not carry the tolerance into the funds a join derives from that HP.
So the class is now mostly *slack that is not propagated*, the same
shape as `charge_slack` in `2026-08-26-power-oracle.md`, and the fix
belongs in the verifier rather than the engine.
