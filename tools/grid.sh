#!/bin/bash
# The seed-group grid: N training arms in parallel on one big card, each
# panelled the moment it finishes. Two seeds of one recipe panelled up to 50
# points apart on a net member (docs/decisions.md), so the unit of experiment
# is the seed group -- this is that unit, made affordable.
#
#   bash tools/grid.sh                  # the default grid, see GRID below
#   GRID=mixjm bash tools/grid.sh       # a named grid, see the case list
#   CONCURRENT=4 bash tools/grid.sh     # cap the lanes by hand
#   PLAN=1 GRID=batch bash tools/grid.sh  # launch order, cost, memory; exit
#
# Each arm is  train -> panel -> play_diag -> per-CO ratings  as one chain.
# Logs and results land in logs/, checkpoints in checkpoints/.
# Watch progress:  tail -f logs/grid.log
#
# Rerunnable. A finished arm leaves logs/rated-<name> holding its exact
# command and is skipped next time; an arm the box lost mid-training resumes
# from its own state file. So an interrupted box -- or an interruptible one,
# at a fraction of the price -- costs the games in flight, not the grid, and
# an arm two grids share by name (a control, say) runs once. A name reused
# for a different recipe refuses rather than reuse the wrong result.
#
# Lanes: arms launch longest first, as many at once as the CPU share and the
# card's memory allow (CONCURRENT=auto). The batch grid ran six lanes of
# eight on a card with room for all of them, and its last arm then ran alone
# for three hours on a mostly idle GPU
# (log/2026-10-15-the-league-lost-to-a-pool-that-never-moved.md).
#
# Run it under nohup or tmux: an ssh drop on a box billing by the minute
# otherwise kills every arm at once.
set -uo pipefail
cd "$(dirname "$0")/.."

CONCURRENT="${CONCURRENT:-auto}"
GRID="${GRID:-mixed}"
# Which weights the ratings read. `last` is what the run produced. `kept` is
# the best rollout window, and for most 32-env arms that was iteration 30-60,
# a handful of updates past the warm-up: their panels re-rated the parent,
# and league2's comparison set those against league arms' final weights
# (log/2026-10-15-the-league-lost-to-a-pool-that-never-moved.md).
RATE_CKPT="${RATE_CKPT:-last}"
# Games per rating. 200 is the floor a rating needs (panel.py); only the
# smoke grid lowers it, to exercise the chain rather than measure anything.
# Defaulted after the case list, so a case can set its own.
RATE_GAMES="${RATE_GAMES:-}"
# The anchor is per grid: the three August grids leash to bc-net2, anything
# after the T4 re-clone leashes to bc-net2-t4, and a stale default here pulls
# a rung toward a clone that barely pops. Each case sets its own.
ANCHOR="${ANCHOR:-}"
# Which scripted bot the per-CO ratings are read against. `greedy` for every
# grid up to the mixed rung, which then saturated it for all five COs at once
# (log/2026-09-07-the-mixed-rung-does-not-stall-it-leads.md): a rung whose
# arms all rate 90-97 as every CO has not been measured. Each case that needs
# a harder instrument sets its own.
RATE_OPP="${RATE_OPP:-greedy}"
PY="${PY:-python3}"
# Vast's PyTorch image keeps torch in a venv; a grid launched from a fresh
# shell would otherwise start eight arms that die on `import torch`.
if [ "$PY" = python3 ] && ! python3 -c "import torch" 2>/dev/null     && [ -f /venv/main/bin/activate ]; then
  # shellcheck disable=SC1091
  source /venv/main/bin/activate
fi
# The five COs whose powers the engine models, and so the only five a rung
# can honestly be dealt (plan.md).
T4="Adder,Jake,Koal,Jess,Grimm"

# Rented containers report the *host's* core count, not the cgroup share, so
# torch defaults to ~128 threads per arm and six arms thrash a twenty-core
# slice: 43 orders/s against 353 with this set, a 6x tax every grid before
# 2026-08-29 paid silently. Roughly one thread per core per arm; the env
# itself is single-threaded (log/2026-08-29-the-day-cap-paid-for-the-rung.md).
export OMP_NUM_THREADS="${OMP_NUM_THREADS:-3}"
export MKL_NUM_THREADS="${MKL_NUM_THREADS:-$OMP_NUM_THREADS}"

ARMS=()
# Files a grid needs beyond the parent, anchor and panel members.
EXTRA=()
case "$GRID" in
  phase1)
    : "${ANCHOR:=checkpoints/bc-net2.pt}"
    # The anchor sweep that settled the recipe: weights {0.01, 0.03} x four
    # seeds against two controls, from the clone
    # (log/2026-08-29-the-grid-confirms-the-anchor.md).
    RECIPE=(--threat-planes --opponent greedy --co Adder --turn-discount
            --steps 256 --lam 0.99 --decide-cap --iterations 200
            --init "$ANCHOR")
    for seed in 7 43 101 202; do
      ARMS+=("n2-a001-s$seed|--anchor $ANCHOR --anchor-kl 0.01 --seed $seed")
      ARMS+=("n2-a003-s$seed|--anchor $ANCHOR --anchor-kl 0.03 --seed $seed")
    done
    ARMS+=("n2-plain-s7|--seed 7")
    ARMS+=("n2-plain-s43|--seed 43")
    ;;
  jakeman)
    : "${ANCHOR:=checkpoints/bc-net2.pt}"
    # The JakeMan rung on the settled recipe, as a seed group -- and two
    # inits, because the greedy rung left a real question: does the
    # all-round best (a003-s7: 96/8/54.8/64) or the JakeMan-best
    # (a003-s101: 94.5/19.2/56.5/56.5) make the better rung parent?
    # v1's bar on this rung: 37.5 rated (ppo-threat2).
    RECIPE=(--threat-planes --opponent jakeman --co Adder --turn-discount
            --steps 256 --lam 0.99 --decide-cap --iterations 200)
    for seed in 7 43 101 202; do
      ARMS+=("jm-s7par-s$seed|--init checkpoints/n2-a003-s7.pt --anchor $ANCHOR --anchor-kl 0.03 --seed $seed")
      ARMS+=("jm-s101par-s$seed|--init checkpoints/n2-a003-s101.pt --anchor $ANCHOR --anchor-kl 0.03 --seed $seed")
    done
    ;;
  jakeman2)
    : "${ANCHOR:=checkpoints/bc-net2.pt}"
    # The continuation rung: v1 went 37.5 -> 63.4 by running the JakeMan rung
    # twice, and the first v2 rung ended oscillating rather than saturated
    # (log/2026-08-29-the-jakeman-rung-and-the-peak-that-does-not-rate.md).
    # Four seeds because one arm's number is a draw: the same eight arms
    # ordered their two parents one way on peak weights and the other way on
    # final weights (log/2026-08-29-the-peak-beats-the-endpoint.md).
    #
    # The two `noanc` arms price `--anchor-kl 0.03` on this rung, which no run
    # has done -- v1 reached 63.4 with no anchor at all, and the leash was
    # pulling (anchor term 0.34-0.43) through every arm of the last grid.
    # Bar to beat: 49.3 vs JakeMan, the best single arm so far.
    PARENT="${PARENT:-checkpoints/jm-s7par-s7.pt}"
    RECIPE=(--threat-planes --opponent jakeman --co Adder --turn-discount
            --steps 256 --lam 0.99 --decide-cap --iterations 200
            --init "$PARENT")
    for seed in 7 43 101 202; do
      ARMS+=("jm2-s$seed|--anchor $ANCHOR --anchor-kl 0.03 --seed $seed")
    done
    ARMS+=("jm2-noanc-s7|--seed 7")
    ARMS+=("jm2-noanc-s43|--seed 43")
    ;;
  mixed)
    # Plan item 6, the first mixed-CO rung: the first honest test of the bet
    # that a policy can infer its CO from effects with no identity plane. Two
    # groups one flag apart -- dealt a random Tier-4 CO each game against the
    # same pool, or the Adder mirror of record -- from bc-net2-t4, the parent
    # that has seen T4 powers fire, and leashed to it. A mixed group that
    # stalls while its control climbs is what "the policy cannot tell who it
    # is" looks like; the control's own bar is the greedy rung's ~96.
    PARENT="${PARENT:-checkpoints/bc-net2-t4.pt}"
    : "${ANCHOR:=$PARENT}"
    RECIPE=(--threat-planes --opponent greedy --turn-discount
            --steps 256 --lam 0.99 --decide-cap --iterations 200
            --init "$PARENT" --anchor "$ANCHOR" --anchor-kl 0.03)
    for seed in 7 43 101 202; do
      ARMS+=("mix-s$seed|--co $T4 --seed $seed")
      ARMS+=("mixctl-s$seed|--co Adder --seed $seed")
    done
    ;;
  mixjm)
    # Plan item 7, the JakeMan rung from the mixed lineage. The mixed rung
    # left `greedy` saturated for all five COs -- an Adder-only control
    # already rates 87.0-97.5 as Jake, Koal, Jess and Grimm
    # (log/2026-09-07-the-mixed-rung-does-not-stall-it-leads.md) -- so JakeMan
    # is the one panel member with headroom left, 7.8-20.0 for every arm ever
    # rated, and it is where the Adder lineage went *backwards*
    # (log/2026-08-30-the-rung-went-backwards.md).
    #
    # Four seeds from `mix-s43`, dealt the pool the recipe of record now uses,
    # leashed to the clone rather than to the immediate parent -- jakeman2's
    # shape, and the anchor is a leash to human play, not to the last rung.
    # The two `noanc` arms are here for the same reason they were there: v1
    # reached 63.4 with no anchor at all, and the anchor term was pulling
    # through every arm of the last JakeMan grid.
    #
    # Bars: 49.3 vs JakeMan is the best single arm on record (Adder lineage,
    # log/2026-08-26-jakeman-beaten.md); this lineage's own is 16.5.
    PARENT="${PARENT:-checkpoints/mix-s43.pt}"
    : "${ANCHOR:=checkpoints/bc-net2-t4.pt}"
    # Rated per CO against JakeMan, not `greedy`: a saturated instrument
    # cannot separate the arms, and "wins across the Tier-4 COs" is the claim
    # this rung has to support.
    RATE_OPP=jakeman
    RECIPE=(--threat-planes --opponent jakeman --co "$T4" --turn-discount
            --steps 256 --lam 0.99 --decide-cap --iterations 200
            --init "$PARENT")
    for seed in 7 43 101 202; do
      ARMS+=("mixjm-s$seed|--anchor $ANCHOR --anchor-kl 0.03 --seed $seed")
    done
    ARMS+=("mixjm-noanc-s7|--seed 7")
    ARMS+=("mixjm-noanc-s43|--seed 43")
    ;;
  league)
    # Lane C's first honest league. Every self-play run on record drifted into
    # specialists that beat their own past and nothing else; the one that did
    # not held four *families*
    # (log/2026-08-27-the-species-league-holds.md). Two things are different
    # now. The pool can hold a scripted bot beside the checkpoints, which is
    # what `league3` lacked and why its `greedy` column stalled. And the
    # measurements of 2026-09-07 say only JakeMan is worth seating: `greedy` is
    # beaten 93-96% and `capturer` 15/15, and a member that cannot contest the
    # position leaves the value head nothing to separate but the day cap.
    #
    # So the scripted half is JakeMan alone and the diversity has to come from
    # the frozen half: the clone, three anchored siblings of the parent, and
    # the unleashed cap-farmer, which is the nearest thing to a second species
    # this lineage has. `ppo-adder3` and `bc-powers-scaled2` cannot join -- 64
    # planes against the pool's 70 -- so they stay panel members.
    #
    # `--seat-cap 50` is a guess, not a measurement: without a cap the member
    # beating the learner is seated almost exclusively and past the first
    # promotions that is always the learner's own newest generation
    # (log/2026-08-27-pfsp-peaks-at-gen3-then-eats-itself.md). 200 iterations
    # at `--seat-hold 5` is 40 seatings, so 50 leaves room to prefer without
    # letting one member own the run.
    #
    # The two controls are the point of the grid. A league that improves has to
    # beat the alternative use of the same money, which is another JakeMan rung
    # from the same parent -- and that lineage has gone backwards before
    # (log/2026-08-30-the-rung-went-backwards.md).
    PARENT="${PARENT:-checkpoints/mixjm-s43.pt}"
    : "${ANCHOR:=checkpoints/bc-net2-t4.pt}"
    RATE_OPP=jakeman
    POOL="checkpoints/bc-net2-t4.pt,checkpoints/mixjm-s202.pt"
    POOL="$POOL,checkpoints/mixjm-s7.pt,checkpoints/mixjm-s101.pt"
    POOL="$POOL,checkpoints/mixjm-noanc-s43.pt"
    EXTRA=(${POOL//,/ })
    RECIPE=(--threat-planes --turn-discount --co "$T4"
            --steps 256 --lam 0.99 --decide-cap --iterations 200
            --init "$PARENT" --anchor "$ANCHOR" --anchor-kl 0.03)
    for seed in 7 43 101 202; do
      ARMS+=("lg-s$seed|--selfplay --frozen-init $POOL --pool-bots jakeman --pool-bot-share 0.25 --seat-cap 50 --seed $seed")
    done
    ARMS+=("lgctl-s7|--opponent jakeman --seed 7")
    ARMS+=("lgctl-s43|--opponent jakeman --seed 43")
    ;;
  league2)
    # The league again, on a promotion rule that can fire. `league` measured a
    # static pool: `--refresh-at 0.7` read one blended score against whoever
    # held the seat, the same weights read 73% and 27% five iterations apart,
    # and the bar cleared once in 800 arm-iterations
    # (log/2026-09-08-the-league-never-promoted.md). `ppo.py` now asks it per
    # member -- every member beaten at `--refresh-at`, 30 games each since the
    # last promotion -- which is invariant to who sat down.
    #
    # `mixjm-s101` leaves the pool. The parent beats it 90.0% and the four
    # that stay hold it to 73.0, 70.5, 54.0 and 50.0
    # (log/2026-09-08-the-pool-is-six-opponents.md); simulated, dropping it
    # buys 4.5 points of promotion power for 2 of false alarms, because a seat
    # spent on a solved member is a seat not spent measuring a contested one
    # (log/2026-09-08-the-promotion-bar-is-calibrated.md, tools/promote_sim.py).
    # The clone stays despite rating 70.5: it is the only member not of the
    # `mixjm` lineage, and diversity is what the one league that worked had.
    #
    # JakeMan keeps its quarter of the seats. It is a bot, `credit` books it
    # apart from the pool, so the new rule never sees it -- its share shapes
    # what the learner trains against and no longer taxes what it must beat.
    # `--seat-cap 50` stays and now earns itself: every member needs 30 games
    # before any promotion, so forcing the seat around is the rule's own work.
    #
    # Four controls, not two. `league`'s pair landed 32 points apart on
    # `greedy` -- wider than the gap between the groups it was there to
    # measure -- so the comparison could not be made at all. Eight arms.
    PARENT="${PARENT:-checkpoints/mixjm-s43.pt}"
    : "${ANCHOR:=checkpoints/bc-net2-t4.pt}"
    RATE_OPP=jakeman
    POOL="checkpoints/bc-net2-t4.pt,checkpoints/mixjm-s202.pt"
    POOL="$POOL,checkpoints/mixjm-s7.pt,checkpoints/mixjm-noanc-s43.pt"
    EXTRA=(${POOL//,/ })
    RECIPE=(--threat-planes --turn-discount --co "$T4"
            --steps 256 --lam 0.99 --decide-cap --iterations 200
            --init "$PARENT" --anchor "$ANCHOR" --anchor-kl 0.03)
    for seed in 7 43 101 202; do
      ARMS+=("lg2-s$seed|--selfplay --frozen-init $POOL --pool-bots jakeman --pool-bot-share 0.25 --seat-cap 50 --seed $seed")
      ARMS+=("lg2ctl-s$seed|--opponent jakeman --seed $seed")
    done
    ;;
  batch)
    # Is "decays from level" noise? An update here learns from ~15 finished
    # games (~9 in a league arm): 32 envs x 256 orders, 8,192 rows. The two
    # Pokemon bots that
    # reached #1 on Showdown by self-play PPO, mikumiku37 and Jaxcalibur,
    # update on 8,192 battles x 32 steps = 262k rows, and played 1e8-3e8
    # games (log/2026-10-15-the-recipe-is-right-and-the-run-is-small.md).
    # At level the win/loss signal is small, the noise is not, and every rung
    # since August has drifted down from its own start -- lowering the step
    # size could not help, since it scales signal and noise together. Only
    # more games per update moves the ratio.
    #
    # So: the lg2ctl recipe exactly (JakeMan, from mixjm-s43, which starts
    # level with it), at 32 envs and at 128. The minibatch scales with the
    # rollout, so both groups take the same 96 gradient steps an iteration at
    # the same learning rate -- the only thing that changes is how many games
    # each step averages over. b128 plays 4x the games, which is the point,
    # and costs 4x the rollout. If b128 ends further above its parent
    # and spreads less across seeds, the fix is batch, not bars.
    #
    # Memory: minibatch 1024 peaks at 5.9 GiB allocated against 1.5 at 256,
    # and a b128 rollout holds 2.6 GB of observations on the host. On a 24GB
    # card run CONCURRENT=3; on 48GB, 6.
    PARENT="${PARENT:-checkpoints/mixjm-s43.pt}"
    : "${ANCHOR:=checkpoints/bc-net2-t4.pt}"
    RATE_OPP=jakeman
    RECIPE=(--threat-planes --turn-discount --co "$T4"
            --steps 256 --lam 0.99 --decide-cap --iterations 200
            --init "$PARENT" --anchor "$ANCHOR" --anchor-kl 0.03
            --opponent jakeman)
    for seed in 7 43 101 202; do
      ARMS+=("b32-s$seed|--envs 32 --minibatch 256 --seed $seed")
      ARMS+=("b128-s$seed|--envs 128 --minibatch 1024 --seed $seed")
    done
    ;;
  smoke)
    # The harness, in minutes: two tiny arms through the whole chain --
    # train, resume state, panel, per-CO ratings, markers. Run it first on a
    # fresh box: it costs pennies and catches a broken build, a missing
    # checkpoint or a bad flag before eight arms die on it. Not a result.
    PARENT="${PARENT:-checkpoints/mixjm-s43.pt}"
    : "${ANCHOR:=checkpoints/bc-net2-t4.pt}"
    RATE_OPP=jakeman
    : "${RATE_GAMES:=10}"
    EXTRA=(checkpoints/bc-net2-t4.pt)
    RECIPE=(--threat-planes --turn-discount --co "$T4"
            --steps 32 --lam 0.99 --decide-cap --iterations 4
            --value-warmup 1 --state-every 2
            --init "$PARENT" --anchor "$ANCHOR" --anchor-kl 0.03)
    ARMS+=("smoke-ctl|--opponent jakeman --envs 8 --minibatch 64 --seed 7")
    ARMS+=("smoke-lg|--selfplay --frozen-init checkpoints/bc-net2-t4.pt --pool-bots jakeman --envs 8 --minibatch 64 --seed 7")
    ;;
  *)
    echo "unknown GRID '$GRID' (phase1|jakeman|jakeman2|mixed|mixjm|league|league2|batch|smoke)"; exit 1;;
esac

: "${RATE_GAMES:=200}"

mkdir -p logs
log() { echo "[$(date +%H:%M)] $*" | tee -a logs/grid.log; }

# Refuse to start without the files every arm needs. Without this, a box whose
# checkpoints were never copied in runs eight arms that each die in seconds
# and prints "grid complete" over an empty result.
# Everything an arm opens before it can start: the parent, the anchor, any
# pool members, and the two net panel members every rating uses.
NEEDS=("${PARENT:-}" "$ANCHOR" "${EXTRA[@]}"
       checkpoints/bc-powers-scaled2.pt checkpoints/ppo-adder3.pt)

# `LIST=1 GRID=league bash tools/grid.sh` prints that list and exits, so
# rental-setup.sh can tell you what to copy without scraping this file --
# which went stale the first time a grid needed more than a parent and an
# anchor, and would have again for the league's five pool members.
if [ -n "${LIST:-}" ]; then
  for need in "${NEEDS[@]}"; do [ -z "$need" ] || echo "$need"; done | sort -u
  exit 0
fi

# The value of a flag in an arm's command line, the last occurrence winning,
# as argparse reads it; the default is ppo.py's own.
flag_of() {
  local want="$1" value="$2" prev="" word; shift 2
  for word in "$@"; do [ "$prev" = "$want" ] && value="$word"; prev="$word"; done
  echo "$value"
}
# Orders an arm trains on, and the GPU memory it holds. Memory is fitted to
# the batch grid's measurement: minibatch 256 peaks at 1.5 GiB allocated and
# 1024 at 5.9, and nvidia-smi read 2.0 and ~6.3 GB a process with the CUDA
# context on top.
cost_of() {
  # shellcheck disable=SC2086
  set -- "${RECIPE[@]}" $1
  echo $(( $(flag_of --envs 32 "$@") * $(flag_of --steps 64 "$@") \
           * $(flag_of --iterations 200 "$@") ))
}
mem_of() {
  # shellcheck disable=SC2086
  set -- "${RECIPE[@]}" $1
  echo $(( 600 + 5700 * $(flag_of --minibatch 256 "$@") / 1024 ))
}

# Longest first, so the grid's tail is the short arms finishing in parallel
# rather than one long arm alone on an idle card. Stable, so arms of equal
# cost keep the case's order -- which interleaves treatment and control per
# seed, and a box lost early still leaves matched pairs.
ORDERED=()
while IFS=$'\t' read -r _ index; do ORDERED+=("${ARMS[$index]}"); done < <(
  for i in "${!ARMS[@]}"; do
    printf '%s\t%s\n' "$(cost_of "${ARMS[$i]#*|}")" "$i"
  done | sort -s -t$'\t' -k1,1nr)
ARMS=("${ORDERED[@]}")

# The CPU share, not the host: containers report the host's cores (see the
# OMP note above), and a lane is the env thread plus OMP_NUM_THREADS.
cores() {
  local quota period
  if [ -r /sys/fs/cgroup/cpu.max ]; then
    read -r quota period < /sys/fs/cgroup/cpu.max
    [ "$quota" != max ] && { echo $(( (quota + period - 1) / period )); return; }
  elif [ -r /sys/fs/cgroup/cpu/cpu.cfs_quota_us ]; then
    quota=$(cat /sys/fs/cgroup/cpu/cpu.cfs_quota_us)
    period=$(cat /sys/fs/cgroup/cpu/cpu.cfs_period_us)
    [ "$quota" -gt 0 ] && { echo $(( (quota + period - 1) / period )); return; }
  fi
  nproc
}
if [ "$CONCURRENT" = auto ]; then
  # Eight lanes on a 21-core share kept a 4090 at 100% with OMP at 2.
  CONCURRENT=$(( $(cores) * 2 / 5 ))
  [ "$CONCURRENT" -ge 1 ] || CONCURRENT=1
fi
# 90% of the card, leaving room for the rating processes that follow
# training inside each lane. 0 means no GPU memory limit.
GPU_MEM_MB="${GPU_MEM_MB:-$(nvidia-smi --query-gpu=memory.total \
  --format=csv,noheader,nounits 2>/dev/null | head -1 | awk '{print int($1*0.9)}')}"
GPU_MEM_MB="${GPU_MEM_MB:-0}"

if [ -n "${PLAN:-}" ]; then
  echo "grid $GRID: ${#ARMS[@]} arms, $CONCURRENT lanes, ${GPU_MEM_MB} MB of GPU"
  for arm in "${ARMS[@]}"; do
    name="${arm%%|*}"; flags="${arm#*|}"
    state=new
    [ -f "logs/trained-$name" ] && state=trained
    [ -f "logs/rated-$name" ] && state=rated
    printf '  %-14s %9s orders  %5s MB  %s\n' "$name" \
      "$(cost_of "$flags")" "$(mem_of "$flags")" "$state"
  done
  exit 0
fi

for need in "${NEEDS[@]}"; do
  [ -z "$need" ] || [ -f "$need" ] || { echo "missing $need -- copy it into checkpoints/ first"; exit 1; }
done

run_arm() {
  local name="$1" flags="$2"
  # What this arm is, exactly. A name two grids share must mean one run --
  # that is how a control is shared instead of re-run -- so the markers hold
  # the full command, and a mismatch refuses instead of reusing a result
  # from a different recipe.
  local recipe="${RECIPE[*]} $flags"
  local trained="logs/trained-$name" rated="logs/rated-$name"
  if [ -f "$rated" ]; then
    if [ "$(cat "$rated")" = "$recipe" ]; then
      log "skip  $name (rated earlier)"
    else
      log "FAILED $name: the name is already used by another recipe ($rated)"
    fi
    return
  fi
  if [ -f "$trained" ] && [ "$(cat "$trained")" != "$recipe" ]; then
    log "FAILED $name: the name is already used by another recipe ($trained)"
    return
  fi
  if [ ! -f "$trained" ]; then
    log "start $name"
    # `--resume` continues from the arm's state file if a lost box left one,
    # and starts fresh otherwise; the log appends so a resume keeps history.
    # shellcheck disable=SC2086
    if ! $PY python/ppo.py "${RECIPE[@]}" $flags --resume \
        --out "checkpoints/$name.pt" >> "logs/train-$name.log" 2>&1; then
      log "FAILED $name (see logs/train-$name.log)"
      return
    fi
    printf '%s' "$recipe" > "$trained"
  else
    log "rate  $name (trained earlier)"
  fi
  local ckpt="checkpoints/$name.pt"
  if [ "$RATE_CKPT" = last ] && [ -f "checkpoints/$name-last.pt" ]; then
    ckpt="checkpoints/$name-last.pt"
  fi
  local diag_games=$(( RATE_GAMES < 30 ? RATE_GAMES : 30 ))
  $PY python/panel.py --checkpoint "$ckpt" --raw-dir "logs/panelraw-$name" \
    --games "$RATE_GAMES" > "logs/panel-$name.txt" 2>&1
  $PY python/play_diag.py --checkpoint "$ckpt" \
    --co Adder --opponent greedy --games "$diag_games" \
    > "logs/diag-$name.txt" 2>&1
  log "done  $name  $(grep -m1 greedy "logs/panel-$name.txt" | tr -s ' ')"
  # The panel rates every arm as Adder so its columns stay comparable with
  # every entry on record. "Wins across the Tier-4 COs" is a different
  # claim, so each arm is also rated as each CO -- a mirror against greedy,
  # the rung's own opponent -- and read through play_diag per CO, because
  # a mixed arm whose indirect share does not move when it is Jake has
  # learned a generalist, not five players. Control arms get it too: an
  # Adder-only rung's score as Grimm is the baseline the mixed arms beat.
  # The mirror is against $RATE_OPP -- usually the rung's own opponent, but
  # `greedy` no longer separates anything (see the RATE_OPP note above).
  #
  # When $RATE_OPP is a panel bot, the panel already played it as Adder with
  # these exact settings and seeds -- identical games, so it is reused.
  for co in ${T4//,/ }; do
    if [ "$co" = Adder ] && [ -s "logs/panelraw-$name/$RATE_OPP.txt" ]; then
      cp "logs/panelraw-$name/$RATE_OPP.txt" "logs/co-$name-$co.txt"
    else
      $PY python/evaluate.py --checkpoint "$ckpt" \
        --opponent "$RATE_OPP" --co "$co" --games "$RATE_GAMES" --temperature 1.0 --decide-cap \
        > "logs/co-$name-$co.txt" 2>&1
    fi
    $PY python/play_diag.py --checkpoint "$ckpt" \
      --co "$co" --opponent "$RATE_OPP" --games "$diag_games" \
      > "logs/diag-$name-$co.txt" 2>&1
  done
  # Against the weights it grew from, head to head. The per-CO column is the
  # rung's own opponent and cannot tell a stronger player from a specialist:
  # two batch arms gained twenty points a CO on JakeMan and lost to their
  # parent 36.5% and 42.5%, while the third beat it 61.0%
  # (log/2026-10-16-the-bigger-batch-climbs-and-can-specialise.md).
  local vs_parent=""
  if [ -n "${PARENT:-}" ] && [ -f "$PARENT" ]; then
    $PY python/evaluate.py --checkpoint "$ckpt" --versus "$PARENT"       --co Adder --games "$RATE_GAMES" --temperature 1.0 --decide-cap       > "logs/h2h-$name-vs-parent.txt" 2>&1
    vs_parent="parent $(grep -m1 '^ *score' "logs/h2h-$name-vs-parent.txt" | awk '{print $2}')"
  fi
  log "rated $name  $(for co in ${T4//,/ }; do printf '%s %s  ' "$co" "$(grep -m1 '^ *score' "logs/co-$name-$co.txt" | awk '{print $2}')"; done)$vs_parent"
  printf '%s' "$recipe" > "$rated"
}

log "grid: $GRID, ${#ARMS[@]} arms, up to $CONCURRENT lanes, ${GPU_MEM_MB} MB GPU, rating $RATE_CKPT"
# GPU memory held by the lanes still running, from each one's estimate.
declare -A LANE_MB=()
held_mb() {
  local pid total=0
  for pid in "${!LANE_MB[@]}"; do
    kill -0 "$pid" 2>/dev/null && total=$(( total + LANE_MB[$pid] ))
  done
  echo "$total"
}
for arm in "${ARMS[@]}"; do
  name="${arm%%|*}"; flags="${arm#*|}"
  need=$(mem_of "$flags")
  # A lane opens when one is free and the card has room -- or when nothing
  # runs at all, so an arm bigger than the budget still runs.
  while :; do
    lanes=$(jobs -rp | wc -l)
    [ "$lanes" -eq 0 ] && break
    if [ "$lanes" -lt "$CONCURRENT" ] && { [ "$GPU_MEM_MB" -eq 0 ] \
        || [ $(( $(held_mb) + need )) -le "$GPU_MEM_MB" ]; }; then break; fi
    wait -n
  done
  run_arm "$name" "$flags" &
  LANE_MB[$!]=$need
done
wait
log "grid complete"

echo
echo "=== panels ==="
for arm in "${ARMS[@]}"; do
  name="${arm%%|*}"
  echo "-- $name"; cat "logs/panel-$name.txt" 2>/dev/null | grep -E "%" || true
  for co in ${T4//,/ }; do
    printf '   as %-6s %s\n' "$co" "$(grep -m1 '^ *score' "logs/co-$name-$co.txt" 2>/dev/null | tr -s ' ')"
  done
done
echo
echo "Pull results home as one archive -- from the local machine:"
echo "  ssh -p <port> <user>@<host> 'cd $PWD && tar czf - logs checkpoints/*.pt' > results.tgz"
echo "or tar it here and download that through the box's file browser:"
echo "  tar czf $PWD/results.tgz -C $PWD logs checkpoints/*.pt"
