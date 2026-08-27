# The advantage floor does not stop the from-ahead decay

> Run 2 re-ran the Adder climb with `--adv-floor 0.3` armed and falsified
> the noise-inflation story: the collapse arrived on schedule anyway.
> Spread collapse is a symptom of the crashed regime, not its cause.

**Setup.** Identical to `2026-08-26-ppo-adder-first-climb.md` — same init,
opponent, seeds, 200 iterations — with `--adv-floor 0.3`, chosen because
run 1's climb held spread 0.5–0.86 and its crash windows 0.15–0.19.

**The trajectories are the same run until the crash is over.** The floor
only changes any computed number once a batch's spread drops below 0.3,
and that never happened during the climb *or the fall*: the printed lines
are bit-identical through iteration 80, where the score has already
turned (75.5% at 70, 62.3% at 80, spreads 0.74 and 0.55 — both above the
floor). The first sub-0.3 batch appears around iteration 90, when the
score is already 12.8%. Consequence: the keep-best checkpoint, saved at
the same iteration-70 window, is **bit-identical to `ppo-adder1.pt`**
(verified tensor-by-tensor), so its independent rating — 63.2% ±2.4,
zero pops — carries over unchanged and was not re-measured.

**What the floor bought after the crash: nothing conclusive.** Training
average 27.3% vs 24.0%, best recovery window 43.9% vs 39.2% — noise.
The oscillating post-crash regime looks the same with the divisor
clamped.

**The hypothesis is dead; the suspects that remain.** The decay begins
while spread is healthy, so normalising a noise-dominated batch is not
what starts it. Left standing, from run 1's own telemetry over the fall
(iterations 60–80): KL spiking to 0.034 with clip 0.27 while *winning*
— large steps taken on saturated-opponent batches; entropy rising
monotonically 2.5 → 3.4 across the climb — the entropy bonus diffusing
the policy once the win-rate gradient flattens; and plain opponent
saturation — at 75% vs greedy the from-behind signal the run was built
on is spent, and continuing to train on it is all downside. The
cheapest discriminating experiment: stop or switch opponents at
saturation (curriculum to JakeMan-under-Adder or self-play) rather than
grinding 130 more iterations against a beaten opponent; second, try
decaying the entropy bonus. Keep-best already fences the damage, so the
floor stays available but is no longer load-bearing.
