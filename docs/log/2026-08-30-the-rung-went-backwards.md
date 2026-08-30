# The rung went backwards, and one arm never left the start

> Re-rating all six `jakeman2` arms on the counted instrument: **every arm
> that trained is below its own parent** against JakeMan on decisive wins --
> 42.0, 38.5, 38.0, 23.5, 23.5 against 44.8. The sixth is not below it because
> it *is* it: `jm2-s43`'s checkpoint is bit-identical to the parent in all 80
> policy tensors, a warm-up window that claimed the file before training.

**Setup.** `panel.py`, 1660 Ti, 200 games a member, flags as logged (`--co
Adder --temperature 1.0`, cap settled), six arms plus the parent. Cells are
score / decisive / capped (`5543bde`); every score reproduces its logged row to
a tenth, so the counters are pure addition and no past rating moves.

| arm | greedy | JakeMan | clone | `ppo-adder3` |
|---|---|---|---|---|
| *parent* | 84.0/79.5/5.5 | **49.3/44.8**/7.5 | 76.5/71.0/5.5 | 94.5/90.5/4.5 |
| jm2-s7 | 75.0/72.0/4.0 | 43.0/42.0/4.5 | 77.0/74.0/3.0 | 87.5/84.0/6.0 |
| jm2-s43 | 84.0/79.5/5.5 | 49.5/45.0/7.5 | 76.5/71.0/5.5 | 94.5/90.5/4.5 |
| jm2-s101 | 79.0/54.0/26.5 | 33.5/23.5/17.5 | 73.5/48.0/26.5 | 85.0/61.0/28.0 |
| jm2-s202 | 79.5/71.0/9.5 | 41.2/38.0/7.5 | 81.5/73.5/8.0 | 91.8/84.0/9.5 |
| jm2-noanc-s7 | 89.5/25.5/64.5 | **70.5**/23.5/48.0 | 96.5/24.0/73.0 | 97.5/22.5/75.5 |
| jm2-noanc-s43 | 84.0/59.5/24.5 | 49.0/38.5/14.5 | 86.5/61.5/25.5 | 98.0/76.5/21.5 |

**One arm is its own parent.** `jm2-s43` matches `jm-s7par-s7` in every cell
because it matches it in every weight: 80 of 84 tensors differ by exactly 0.0,
and the four that move are `value.*`, which no action depends on. Its
`-last.pt` moved, so the arm trained 200 iterations and threw all of it away --
`kept = closed and score > best` carried no `not warming` guard, so a lucky
window during the critic warm-up saved the init and nothing later beat it. The
comment three lines up already said the first JakeMan run had done this. Now
guarded (`ppo.py`), matching the promote branch that always had it.

**The rung is negative on every arm that ran.** Against the parent's 44.8
decisive: 42.0, 38.5, 38.0, 23.5, 23.5. On raw score only `jm2-noanc-s7`
clears 49.3, and it does it by capping 48% of the games. v1's continuation
rung went 37.5 -> 63.4; v2's continuation from 49.3 produced nothing, and the
bar stands where it stood because the arm holding it never moved.

**The instrument flips the experiment's verdict.** The grid existed to price
`--anchor-kl 0.03` on this rung. By score the unanchored pair beats the
anchored group on JakeMan 59.8 to 39.2 and `ppo-adder3` 97.8 to 88.1; by
decisive wins the anchored group wins both, 34.5 to 31.0 and 76.3 to 49.5. Same
games, opposite conclusion, and the score-leader ties for last on wins.

**Cap-farming is a seed property, not an anchor property.** The previous entry
read one anchored arm (4.5%) against one unanchored (48.0%) and credited the
anchor. Anchored arms span 3.0-28.0% capped and unanchored 14.5-75.5% --
overlapping, with the anchored `jm2-s101` above the unanchored `jm2-noanc-s43`
on all four members. The anchor lowers the mean; it does not control this.

**The blast radius is one arm.** Fingerprinting the play-affecting weights of
all 186 checkpoints gives 164 distinct policies and 19 duplicate groups, all
benign or known but this one: `X-genN == X` is the promote branch writing both
files, `ppo-adder2 == ppo-adder1` is documented
(`log/2026-08-27-adv-floor-null.md`), and `armB` is its own init but its entry
rated `armB-last`. Only `jm2-s43` reached a table. Checkpoints record
architecture, not lineage, which is why this takes a fingerprint, not a lookup.

**Next.** `-last.pt` for all six arms is rating now: the peak file is what
selection just failed on, and item 4 spends nothing until that reads.
