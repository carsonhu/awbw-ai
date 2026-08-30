# The day cap paid for the rung, and the anchor was holding a collapse

> Re-rating the grid's best arm without `--decide-cap` turns 70.5 against
> JakeMan into **47.5**: 48.0% of its games reach the 60-day cap and settle
> on income, against the anchored control's 4.5%. On outright wins the
> anchored arm is the better policy, 42.0% to 23.5%. Worse, both unanchored
> arms *collapsed* in training -- `jm2-noanc-s7` ran 54.5% at iteration 50
> to 3.6% at 200 -- and only peak-selection saved a checkpoint worth rating.
> The anchor was never about composition here. It was holding the decay.

**Setup.** `PARENT=jm-s7par-s7 GRID=jakeman2 CONCURRENT=6` on a Vast.ai 4090,
1h50m: the settled recipe at `--anchor-kl 0.03` on four seeds against two with
the flag omitted. Re-rating is `evaluate.py`, flags matched to `panel.py` (200
games, `--co Adder --temperature 1.0`), run with `--decide-cap` and without.

| arm | panel, g/jm/clone/a3 | JakeMan, no cap | capped |
|---|---|---|---|
| jm2-noanc-s7 | 89.5/**70.5**/96.5/97.5 | **47.5 +-3.5** | **48.0%** |
| jm2-s7 | 75.0/43.0/77.0/87.5 | 44.2 +-3.5 | 4.5% |
| jm2-s43 | 84.0/49.5/76.6/94.5 | | |
| jm2-s101 | 78.5/33.5/73.5/85.0 | | |
| jm2-s202 | 79.5/41.2/81.5/91.8 | | |
| jm2-noanc-s43 | 84.0/49.0/86.5/98.0 | | |

**The tiebreak paid for the whole row.** `--decide-cap` settles a capped game
on income, then property count, and `jm2-noanc-s7` never closes one: it wins
25.5% against greedy, 23.5% against JakeMan and 22.5% against `ppo-adder3` by
elimination, while 64.5 / 48.0 / **75.5%** of those games reach the cap and
settle its way -- printing 89.5, 70.5 and 97.5 on the panel. It rarely loses
either (2.0% to `ppo-adder3`); it draws. It plays 33.2-order turns on 28.3M
funds a game against `jm2-s7`'s 22.0 and 14.7M. Strip the tiebreak on JakeMan
and it is level with the anchored control, 47.5 to 44.2 at +-3.5, and well
behind on eliminations, 23.5 to 42.0.

**Both unanchored arms collapsed.** Rollout score against JakeMan, by
iteration, for `jm2-noanc-s7`: 54.1 (10), 54.5 (50), 50.6 (100), 35.0 (150),
19.5 (180), 5.4 (190), **3.6 (200)**. Final scores across the grid --
anchored 50.0 / 27.6 / 19.0 / 40.1, unanchored **3.6 / 17.5** -- put both
unanchored arms below every anchored one. `jm2-s7` went the other way, 30.3
at iteration 100 to 50.0 at 200. The only reason the collapsed arm rates at
all is that `ppo.py` keeps the best rollout, which froze weights from before
the fall. This is "decays from level" (`decisions.md`) with the anchor
identified as what postpones it.

**`cut` cannot see a settled cap, by construction.** `stalled` is
`(drawn - seen) / games` (`ppo.py`), and `--decide-cap` converts every capped
game into a win or a loss, so the drawn count stays at zero and `cut` prints
0% however many games the clock decided. It read 0-1% through all six arms
while the winner was capping half its games. Every run since `--decide-cap`
became standard has been blind to its own cap share.

**What survives.** The anchor still does not move composition -- Anti-Air
14.4% against 14.6%, both far above the clone's 8.3 and the human's 6.1 --
so it does a different job than the one it was hired for. And containers
report the host's core count: torch took 128 threads per arm on a 19.7-core
share, and `OMP_NUM_THREADS=3` moved 43 orders/s to 353.

**Next.** The instrument, before another rung. Rate without `--decide-cap`
or report the split; make `cut` count capped games whether or not they were
settled. `--anchor-kl 0.03` stands, and the bar stands at 49.3.
