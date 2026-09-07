# The corpus prices Grimm, and it is the CO

> Plan item 8, free and local: is the JakeMan rung's 19-point Grimm hole the
> CO or the policy? The CO. On this map humans win 29% as Grimm on AWBW's own
> table, 31.8% across the archive, and 35.0% against the other Tier-4 four at
> matched Elo where Adder wins 53.5 -- an 18.5-point penalty against the
> anchored arms' 19.1. The no-identity-plane bet has no counterexample here.
> Jess does: the policy is 19 points *better* at her than human play is.

**Setup.** `data/mooo-119544.json`, 2,976 archived games on A River Supreme,
the training map. 5,280 player-sides survive dropping the 310 games missing an
Elo and the 26 with no decided result. Two instruments, because the raw win
rate is confounded -- Grimm's players average 29 Elo *below* their opponents.
First, performance against Elo expectation, `1/(1+10^(-diff/400))`. Second, a
model-free control: head-to-head inside the Tier-4 five, both players within
50 Elo. The policy numbers are the four anchored arms' decisive share against
JakeMan, averaged (`log/2026-09-07-the-rung-clears-the-record-and-grimm-gates-it.md`).

| CO | n | win% | expected | residual | h2h at matched Elo |
|---|---|---|---|---|---|
| `adder` | 678 | 49.0 | 49.8 | -0.8 +-1.9 | 53.5 +-4.0 |
| `jake` | 579 | 50.4 | 50.0 | +0.4 +-2.1 | 52.6 +-4.0 |
| `koal` | 165 | 50.3 | 49.3 | +1.0 +-3.9 | 56.5 +-6.3 |
| `jess` | 304 | 46.7 | 50.0 | -3.3 +-2.9 | 41.2 +-5.3 |
| `grimm` | 132 | 31.8 | 45.9 | **-14.1 +-4.1** | **35.0 +-7.5** |

For scale outside the five: `kindle` +6.6 and `max` +6.0 lead, `sami` -16.1 is
the only CO worse than Grimm. AWBW's own per-CO table for the map gives Grimm
61-148-2, 29% -- a fourth number, from a different denominator, in the same
place.

**The Grimm penalty is the same size for the policy and for people.** Measured
as the drop from each side's own Adder number: humans -18.5 head-to-head
(53.5 -> 35.0), the anchored arms -19.1 (52.3 -> 33.1). The Elo-residual
instrument is more conservative at -13.3 and points the same way. Nothing is
left over for "the policy cannot tell it is fragile," and the residual that
would carry it, ~0.6 to 5.8 points, is inside the noise of both measurements.
If anything -14.1 is a floor: a Grimm main's Elo already contains their Grimm
play, which shrinks the measured penalty rather than inflating it.

**Jess is the real divergence, in our favour.** She is human play's second-
worst of the five at 41.2, and the policy's *best* at 59.0 -- a 19-point
inversion where the other four track within 3. Her bonus is direct vehicles
and the policy builds 29% Tank, 15% Anti-Air and 1.2% indirects, so it is
holding a hand her power was written for and human players are not.

**What it does not settle.** The two sides are not the same measurement --
humans against humans on a ladder, the policy against one scripted bot -- so
only the Adder-relative deltas compare, not the levels. Grimm's h2h cell is 40
games. And this prices Grimm on *this map*; nothing here says the penalty
travels, and the corpus cannot say, since the corpus is one map.
