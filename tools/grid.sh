#!/bin/bash
# The seed-group grid: N training arms in parallel on one big card, each
# panelled the moment it finishes. Two seeds of one recipe panelled up to 50
# points apart on a net member (docs/decisions.md), so the unit of experiment
# is the seed group -- this is that unit, made affordable.
#
#   bash tools/grid.sh                  # the default grid, see GRID below
#   GRID=jakeman2 bash tools/grid.sh    # a named grid
#   CONCURRENT=4 bash tools/grid.sh     # fewer lanes on a smaller card
#
# Each arm is  train -> panel -> play_diag -> per-CO ratings  as one chain;
# chains run CONCURRENT at a time (default 5: arms measured 4.4GB apiece, five
# fit in 24GB). Logs and results land in logs/, checkpoints in checkpoints/.
# Watch progress:  tail -f logs/grid.log
#
# Run it under nohup or tmux: an ssh drop on a box billing by the minute
# otherwise kills every arm at once.
set -uo pipefail
cd "$(dirname "$0")/.."

CONCURRENT="${CONCURRENT:-5}"
GRID="${GRID:-mixed}"
# The anchor is per grid: the three August grids leash to bc-net2, anything
# after the T4 re-clone leashes to bc-net2-t4, and a stale default here pulls
# a rung toward a clone that barely pops. Each case sets its own.
ANCHOR="${ANCHOR:-}"
PY="${PY:-python3}"
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
  *)
    echo "unknown GRID '$GRID' (phase1|jakeman|jakeman2|mixed)"; exit 1;;
esac

mkdir -p logs
log() { echo "[$(date +%H:%M)] $*" | tee -a logs/grid.log; }

run_arm() {
  local name="$1" flags="$2"
  log "start $name"
  # shellcheck disable=SC2086
  if $PY python/ppo.py "${RECIPE[@]}" $flags \
      --out "checkpoints/$name.pt" > "logs/train-$name.log" 2>&1; then
    $PY python/panel.py --checkpoint "checkpoints/$name.pt" \
      > "logs/panel-$name.txt" 2>&1
    $PY python/play_diag.py --checkpoint "checkpoints/$name.pt" \
      --co Adder --opponent greedy --games 30 \
      > "logs/diag-$name.txt" 2>&1
    log "done  $name  $(grep -m1 greedy "logs/panel-$name.txt" | tr -s ' ')"
    # The panel rates every arm as Adder so its columns stay comparable with
    # every entry on record. "Wins across the Tier-4 COs" is a different
    # claim, so each arm is also rated as each CO -- a mirror against greedy,
    # the rung's own opponent -- and read through play_diag per CO, because
    # a mixed arm whose indirect share does not move when it is Jake has
    # learned a generalist, not five players. Control arms get it too: an
    # Adder-only rung's score as Grimm is the baseline the mixed arms beat.
    for co in ${T4//,/ }; do
      $PY python/evaluate.py --checkpoint "checkpoints/$name.pt" \
        --opponent greedy --co "$co" --games 200 --temperature 1.0 --decide-cap \
        > "logs/co-$name-$co.txt" 2>&1
      $PY python/play_diag.py --checkpoint "checkpoints/$name.pt" \
        --co "$co" --opponent greedy --games 30 \
        > "logs/diag-$name-$co.txt" 2>&1
    done
    log "rated $name  $(for co in ${T4//,/ }; do printf '%s %s  ' "$co" "$(grep -m1 '^ *score' "logs/co-$name-$co.txt" | awk '{print $2}')"; done)"
  else
    log "FAILED $name (see logs/train-$name.log)"
  fi
}

log "grid: $GRID, ${#ARMS[@]} arms, $CONCURRENT concurrent"
for arm in "${ARMS[@]}"; do
  name="${arm%%|*}"; flags="${arm#*|}"
  while [ "$(jobs -rp | wc -l)" -ge "$CONCURRENT" ]; do wait -n; done
  run_arm "$name" "$flags" &
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
