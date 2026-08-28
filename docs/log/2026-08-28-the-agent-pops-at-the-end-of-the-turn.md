# The agent pops at the end of the turn, wasting half of every power

> Read out of the recorded replays: in every activation across five
> games, the agent fires Sideslip as its turn's last order — 21 to 36
> orders before it, zero or one after. The +1 movement applies to no
> move, the +10 attack applies to no attack. The bots, meanwhile, pop
> at turn start like players do.

**How it surfaced.** A human watched `930000` in AWBW Replay Player
and noticed the agent's meter drop to zero with no POWER banner. The
banner comes from the turn-start snapshot, and the agent's power is
never on at one of its own turn starts: activated at turn's end, it
runs through the opponent's turn and expires exactly when the next
snapshot is taken. The viewer is fine, the writer is fine — the
*timing* is the anomaly the display was faithfully reporting.

**The measurement** (orders before / after each activation):

| game | agent (`ppo-threat3`) | scripted opponent |
|---|---|---|
| vs JakeMan d35 | 21/0 22/0 23/0 22/1 23/1 | 5/7 0/16 0/16 1/11 2/10 ... |
| vs JakeMan d17 | 22/0 | 0/11 2/12 0/11 2/11 |
| vs greedy d29 | 26/1 36/0 | 10/11 0/16 0/9 0/5 ... |

**Why a learner lands here.** At the end of a turn everything has
moved and the marginal action set is {end turn, pop}. Popping there is
free: it costs no re-sequencing, and it still buys the +10/+10 through
the opponent's whole turn — a real, immediately-credited gain. Popping
*first* buys strictly more (movement and attack for 20+ orders), but
discovering that requires re-ordering the entire turn behind an
exploratory activation, a far longer credit path. End-of-turn popping
is a textbook local optimum, and every pop-rate number this project
has celebrated (up to 6.9/game in league4) was at most half-valued —
defensive stat-boost and meter cycling, never an alpha strike.

**What it retro-explains.** The league runs' charge-farming meta
(defensive pops compound with charge cycling), and why pop rate rose
without pop *placement* improving: rate is visible to the reward,
placement's opportunity cost is counterfactual and invisible.

**Levers, in rough order of promise.** (1) Measure it always: pop
position (orders-after / turn length) joins the probe suite; any claim
that a run "learned powers" needs this number. (2) The human corpus
pops correctly and power-turn activations are already served as BC
labels — but 213 labels in 1.9M orders seeded nothing; upweighting
activation labels at BC (the old untouched lever) now has a concrete
target: clone in the *timing*, not just the act. (3) An exploiter
trained from a checkpoint whose pops are forcibly early (e.g. masking
activation sources to the turn's first k orders during rollouts) would
show what full-value popping is worth — if that gap is large, the
main line is leaving real strength on the table every game.
