#!/bin/bash
# The seed-group grid: N training arms in parallel on one big card, each
# panelled the moment it finishes. Two seeds of one recipe panelled up to 50
# points apart on a net member (docs/decisions.md), so the unit of experiment
# is the seed group -- this is that unit, made affordable.
#
#   bash tools/grid.sh                # the default phase-1 grid
#   CONCURRENT=4 bash tools/grid.sh   # fewer lanes on a smaller card
#
# Each arm is  train -> panel -> play_diag  as one chain; chains run
# CONCURRENT at a time (default 5: arms measured 4.4GB apiece, five fit in
# 24GB). Logs and results land in logs/, checkpoints in checkpoints/.
# Watch progress:  tail -f logs/grid.log
set -uo pipefail
cd "$(dirname "$0")/.."

CONCURRENT="${CONCURRENT:-5}"
INIT="${INIT:-checkpoints/bc-net2.pt}"
RECIPE=(--threat-planes --opponent greedy --co Adder --turn-discount
        --steps 256 --lam 0.99 --decide-cap --iterations 200 --init "$INIT")
PY="${PY:-python3}"

# name : extra flags -- the phase-1 grid. Anchor weights bracket the sweep's
# two candidates; seeds give each recipe a group instead of a lottery ticket.
ARMS=()
for seed in 7 43 101 202; do
  ARMS+=("n2-a001-s$seed|--anchor $INIT --anchor-kl 0.01 --seed $seed")
  ARMS+=("n2-a003-s$seed|--anchor $INIT --anchor-kl 0.03 --seed $seed")
done
ARMS+=("n2-plain-s7|--seed 7")
ARMS+=("n2-plain-s43|--seed 43")

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
  else
    log "FAILED $name (see logs/train-$name.log)"
  fi
}

log "grid: ${#ARMS[@]} arms, $CONCURRENT concurrent, init $INIT"
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
done
echo
echo "Pull results home with:"
echo "  scp '<this-box>:awbw-ai/checkpoints/n2-*.pt' checkpoints/"
echo "  scp '<this-box>:awbw-ai/logs/*' <somewhere-local>/"
