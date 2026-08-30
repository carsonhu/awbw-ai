# The peak holds, and one arm stops winning games altogether

> Rating all six `jakeman2` endpoints against their peaks: the peak wins by
> **14.3 points of decisive wins** against JakeMan, better in five arms of six.
> The previous entry measured +5.8 on score for the other grid; read on games
> actually closed the gap is nearly three times that. And `jm2-noanc-s7-last`
> rates 65.8% against `greedy` while winning **2.0%** of those games -- zero
> against JakeMan, in two hundred -- which is the whole instrument problem in
> one row.

**Setup.** `panel.py` on the 1660 Ti, 200 games a member, flags as logged, on
`-last.pt` for all six arms; the peak column is the re-rate from
`log/2026-08-30-the-rung-went-backwards.md`. Cells are score / decisive.

| arm | JakeMan peak | JakeMan last | greedy last (cap) |
|---|---|---|---|
| *parent* | 49.3 / **44.8** | -- | -- |
| jm2-s7 | 43.0 / 42.0 | 46.8 / 29.0 | 87.5 / 47.5 (40.5) |
| jm2-s43 | 49.5 / 45.0 | 26.5 / 23.5 | 57.0 / 33.0 (25.5) |
| jm2-s101 | 33.5 / 23.5 | 40.0 / **30.5** | 80.5 / 66.0 (15.5) |
| jm2-s202 | 41.2 / 38.0 | 43.0 / 25.0 | 66.0 / 38.0 (29.5) |
| jm2-noanc-s7 | 70.5 / 23.5 | 3.8 / **0.0** | 65.8 / **2.0** (66.5) |
| jm2-noanc-s43 | 49.0 / 38.5 | 18.8 / 17.0 | 41.5 / 26.5 (17.0) |
| mean | 47.8 / **35.1** | 29.8 / **20.8** | |

**Keep the peak, and the case is stronger than it was.** The argmax lands on
better weights than iteration 200 by 5.8 points of score on the `jakeman` grid,
while the statistic picking it is noise
(`log/2026-08-29-the-peak-beats-the-endpoint.md`). Both still hold, and on decisive wins the selection is
worth 14.3. The one arm whose endpoint wins, `jm2-s101`, is the arm whose peak
was worst -- regression to its own mean, the same pattern as last time.

**A policy that rates 65.8% and wins 2.0%.** `jm2-noanc-s7-last` closes
essentially nothing: 2.0% decisive against `greedy` on 66.5% capped, 1.5%
against the clone, 1.0% against `ppo-adder3`, and **0 of 200** against JakeMan.
Its scores are almost purely the income tiebreak. The training curve for this
arm was already known to collapse -- 54.5% at iteration 50 to 3.6% at 200 --
but what the collapse produces is not a policy that loses. It is a policy that
stalls, which the old instrument read in the sixties.

**Cap share tracks strength against that member, not training time.** It rises
from peak to endpoint on four arms and falls on two, and the largest fall is
`jm2-noanc-s7` against JakeMan, 48.0% to 7.0% -- by the endpoint it is losing
outright rather than stalling. A policy caps when it is strong enough to
survive and too weak to close, so cap share is a statement about a matchup, not
a phase of training.

**The rung is still negative.** Best endpoint 30.5 decisive, best trained peak
42.0, parent 44.8. Nothing in either checkpoint set reaches the bar.

**Next.** Item 4, the net change and re-clone, is now the first thing that
costs money, and it starts from `jm-s7par-s7` -- unchanged, because nothing
this grid produced beat it.
