#!/bin/bash
# A grid on a rented Vast box, end to end: rent, upload, smoke, launch, watch,
# bring the results home, destroy. The box is the expensive part, and the
# batch grid's sat finished until someone noticed; this makes "results home,
# box gone" one step instead of a chore.
#
#   GRID=scale bash tools/rent.sh up     # rent, upload, smoke, launch
#   bash tools/rent.sh status            # box state, cost so far, grid.log tail
#   bash tools/rent.sh watch             # wait for the grid, pull, destroy
#   bash tools/rent.sh pull              # results home now; the box stays
#   bash tools/rent.sh down              # destroy the box
#
# Runs locally (Git Bash is fine) with ssh, jq and the vastai CLI
# (`pip install vastai`, `vastai set api-key`). One box at a time: its id and
# grid live in logs/rent-instance.
#
# On-demand by default. BID=1 rents interruptible at a margin over the
# cheapest bid, and on 2026-10-16 three of three never ran a grid: bid
# listings include machines already taken, and an on-demand renter preempts
# any bid -- two never booted, one died 30 minutes in. An outbid box pauses
# with its disk intact and the onstart hook relaunches the grid when it
# resumes (untested), so it is for long grids where the saving pays for that.
#
# `rent.sh box` is the box's side, started by `up` and by the onstart hook:
# it runs the grid, marks it complete, and if no one has pulled the results
# and destroyed the box GRACE_HOURS later, stops it -- stopped, not destroyed,
# because the results are on its disk.
set -uo pipefail
cd "$(dirname "$0")/.."

STATE=logs/rent-instance
REMOTE=/workspace/awbw-ai
IMAGE="${IMAGE:-pytorch/pytorch:2.5.1-cuda12.4-cudnn9-devel}"
DISK="${DISK:-60}"
# Big enough for a grid's eight lanes: the CPU share sets the lane count
# (grid.sh), host RAM holds the rollouts, and a 48GB card is the floor --
# Ampere or later, since the cheapest 48GB listings are Turing cards.
QUERY="${QUERY:-compute_cap>=800 gpu_ram>=46 num_gpus=1 cpu_cores_effective>=20 cpu_ram>=64 reliability>0.98 inet_down>=200 disk_space>=$DISK cuda_vers>=12.4}"
BID="${BID:-0}"
BID_MARGIN="${BID_MARGIN:-1.1}"
GRACE_HOURS="${GRACE_HOURS:-2}"
export PYTHONIOENCODING=utf-8

die() { echo "rent: $*" >&2; exit 1; }
id_of() { [ -f "$STATE" ] || die "no box ($STATE missing)"; awk '{print $1}' "$STATE"; }
grid_of() { awk '{print $2}' "$STATE"; }
info() { vastai show instance "$(id_of)" --raw 2>/dev/null; }
ssh_box() {
  local url host port
  url=$(vastai ssh-url "$(id_of)" 2>/dev/null) || die "no ssh url yet"
  url="${url#ssh://}"; host="${url%:*}"; port="${url##*:}"
  ssh -p "$port" -o StrictHostKeyChecking=accept-new \
      -o UserKnownHostsFile=logs/rent-known_hosts -o LogLevel=ERROR \
      -o ConnectTimeout=20 -o ServerAliveInterval=30 "$host" "$@"
}

up() {
  [ -n "${GRID:-}" ] || die "name the grid: GRID=<name> bash tools/rent.sh up"
  [ ! -f "$STATE" ] || die "a box is already up ($(cat "$STATE")); down it first"
  local needs
  needs=$(LIST=1 GRID="$GRID" bash tools/grid.sh) || die "grid.sh does not know GRID=$GRID"
  for f in $needs; do [ -f "$f" ] || die "missing $f"; done

  # OFFER=<id> takes a particular machine; otherwise the cheapest match.
  local offer price args=() type=on-demand
  [ "$BID" = 1 ] && type=bid
  read -r offer price < <(vastai search offers "$QUERY" -t "$type" -o dph --raw \
    | jq -r --arg want "${OFFER:-}" --arg bid "$BID" \
      'map(select($want == "" or (.id|tostring) == $want)) | first
       | "\(.id) \(if $bid == "1" then .min_bid else .dph_total*1000|round/1000 end)"')
  [ -n "$offer" ] && [ "$offer" != null ] || die "no offer matches: $QUERY ${OFFER:+(id $OFFER)}"
  if [ "$BID" = 1 ]; then
    price=$(awk -v p="${price:-0}" -v m="$BID_MARGIN" 'BEGIN{printf "%.3f", p*m}')
    [ -n "${BID_PRICE:-}" ] && price="$BID_PRICE"
    args+=(--bid_price "$price")
  fi
  # The onstart hook runs at every container start: a no-op before `up` has
  # launched anything, the relaunch after an interruption once it has.
  local onstart="cd $REMOTE 2>/dev/null && [ -f logs/launch ] && [ ! -f logs/complete ] && setsid nohup bash tools/rent.sh box >> logs/nohup.log 2>&1 < /dev/null &"
  echo "rent: offer $offer at \$${price}/hr ($([ "$BID" = 1 ] && echo interruptible || echo on-demand))"
  local id
  id=$(vastai create instance "$offer" --image "$IMAGE" --disk "$DISK" --ssh --direct \
       --label "awbw-$GRID" --onstart-cmd "$onstart" "${args[@]}" --raw \
       | jq -r '.new_contract') || die "create failed"
  [ -n "$id" ] && [ "$id" != null ] || die "create failed"
  echo "$id $GRID" > "$STATE"
  echo "rent: instance $id; waiting for it to boot"

  # An interruptible box outbid before it boots reads "loading" forever while
  # Vast intends it stopped; that is a lost bid, not a slow image pull.
  local tries=0
  until [ "$(info | jq -r '.actual_status')" = running ] && ssh_box true 2>/dev/null; do
    tries=$((tries + 1)); [ "$tries" -le 180 ] || die "not up after 30 minutes; check it or down it"
    [ "$tries" -ge 6 ] && [ "$(info | jq -r '.intended_status')" = stopped ] \
      && die "outbid before it booted (bid \$$price); down it and bid higher or elsewhere"
    sleep 10
  done

  echo "rent: uploading the working tree and $(echo $needs | wc -w) checkpoints"
  ssh_box "mkdir -p $REMOTE/checkpoints $REMOTE/logs" || die "ssh failed"
  git ls-files -z --cached --others --exclude-standard \
    | tar --null -T - -czf - | ssh_box "tar xzf - -C $REMOTE" || die "code upload failed"
  # shellcheck disable=SC2086
  tar czf - $needs | ssh_box "tar xzf - -C $REMOTE" || die "checkpoint upload failed"

  echo "rent: building on the box"
  ssh_box "cd $REMOTE && GRID=$GRID bash tools/rental-setup.sh" > logs/rent-setup.log 2>&1 \
    || die "setup failed, see logs/rent-setup.log (box still up)"

  echo "rent: smoke grid"
  ssh_box "cd $REMOTE && GRID=smoke bash tools/grid.sh" > logs/rent-smoke.log 2>&1
  if ! grep -q 'rated smoke-ctl' logs/rent-smoke.log || ! grep -q 'rated smoke-lg' logs/rent-smoke.log \
      || grep -q FAILED logs/rent-smoke.log; then
    die "smoke failed, see logs/rent-smoke.log (box still up)"
  fi
  # Smoke is not a result: clear it so the pull carries the grid alone.
  ssh_box "cd $REMOTE && rm -rf logs/*smoke* checkpoints/smoke* && : > logs/grid.log"

  ssh_box "cd $REMOTE && PLAN=1 GRID=$GRID bash tools/grid.sh"
  ssh_box "cd $REMOTE && echo $GRID > logs/launch && setsid nohup bash tools/rent.sh box > logs/nohup.log 2>&1 < /dev/null &"
  echo "rent: GRID=$GRID launched on $id. Watch with: bash tools/rent.sh watch"
}

box() {
  local grid; grid=$(cat logs/launch)
  [ -f logs/complete ] && exit 0
  # The onstart hook runs outside a login shell; the image keeps python here.
  export PATH="/opt/conda/bin:$PATH"
  GRID="$grid" bash tools/grid.sh
  touch logs/complete
  sleep "${GRACE_HOURS}h"
  # Still here, so nothing pulled and destroyed it. Stop billing for the GPU.
  if [ -z "${CONTAINER_API_KEY:-}" ] && [ -r /proc/1/environ ]; then
    eval "$(tr '\0' '\n' < /proc/1/environ | grep -E '^(CONTAINER_ID|CONTAINER_API_KEY)=' | sed 's/^/export /')"
  fi
  echo "[$(date +%H:%M)] grace over, stopping $CONTAINER_ID" >> logs/grid.log
  curl -s -X PUT "https://console.vast.ai/api/v0/instances/$CONTAINER_ID/" \
    -H "Authorization: Bearer $CONTAINER_API_KEY" -H 'Content-Type: application/json' \
    -d '{"state":"stopped"}' >> logs/grid.log
}

status() {
  local i; i=$(info) || die "vastai show instance failed"
  echo "$i" | jq -r '"instance \(.id) \(.gpu_name) \(.actual_status), $\(.dph_total|.*1000|round/1000)/hr, up \(((now - .start_date)/3600)*10|round/10) h, ~$\((now - .start_date)/3600*.dph_total*100|round/100) so far"'
  [ "$(echo "$i" | jq -r .actual_status)" = running ] || return 0
  ssh_box "cd $REMOTE && tail -n ${LINES_:-12} logs/grid.log; if [ -f logs/complete ]; then echo '(complete)'; fi"
}

pull() {
  local grid stamp dest tmp
  grid=$(grid_of); stamp=$(date +%F); dest="results-$grid-$stamp.tgz"
  echo "rent: pulling into $dest"
  # Logs, and the checkpoints the grid wrote (newer than its launch marker);
  # state files are for resuming on the box and stay there.
  ssh_box "cd $REMOTE && tar czf - logs \$(find checkpoints -name '*.pt' ! -name '*-state.pt' -newer logs/launch)" \
    > "$dest" || die "pull failed"
  # Listed in full before matching: `grep -q` quits at the first hit, tar
  # takes SIGPIPE, and under pipefail a good archive reads as a bad one.
  local listing; listing=$(tar tzf "$dest") || die "$dest is not a readable archive"
  grep -q '^logs/grid.log$' <<< "$listing" || die "$dest has no grid.log; not trusting it"
  tmp=$(mktemp -d)
  tar xzf "$dest" -C "$tmp"
  [ -d "$tmp/checkpoints" ] && cp "$tmp"/checkpoints/*.pt checkpoints/
  mv "$tmp/logs/grid.log" "logs/grid-rental-$grid-$stamp.log"
  rm -f "$tmp/logs/launch" "$tmp/logs/complete"
  cp -r "$tmp"/logs/. logs/
  rm -rf "$tmp"
  echo "rent: $(tar tzf "$dest" | grep -c '^checkpoints/.*\.pt$') checkpoints and the logs are home; grid log at logs/grid-rental-$grid-$stamp.log"
}

down() {
  local id; id=$(id_of)
  vastai destroy instance "$id" -y >/dev/null || die "destroy failed; the box may still bill"
  rm -f "$STATE"
  echo "rent: instance $id destroyed"
}

# Poll until the grid is complete, then pull and destroy. Prints each new
# grid.log line. Past DEADLINE_HOURS it pulls what exists and destroys, unless
# the box is paused -- then the results are only on its disk, so it says so
# and leaves it.
watch() {
  local interval="${INTERVAL:-600}" deadline=$(( $(date +%s) + ${DEADLINE_HOURS:-16} * 3600 ))
  local seen=0 st lines
  while :; do
    st=$(info | jq -r '.actual_status // "unknown"')
    if [ "$st" = running ]; then
      lines=$(ssh_box "cd $REMOTE && cat logs/grid.log" 2>/dev/null) || lines=""
      if [ -n "$lines" ]; then
        echo "$lines" | tail -n +$((seen + 1))
        seen=$(echo "$lines" | wc -l)
      fi
      if ssh_box "test -f $REMOTE/logs/complete" 2>/dev/null; then
        pull && down; return
      fi
    else
      echo "[$(date +%H:%M)] box is $st"
    fi
    if [ "$(date +%s)" -ge "$deadline" ]; then
      [ "$st" = running ] || die "deadline passed with the box $st; results are on its disk, not destroying"
      echo "rent: deadline passed, pulling what exists"
      pull && down; return 1
    fi
    sleep "$interval"
  done
}

case "${1:-}" in
  up|box|status|watch|pull|down) "$1" ;;
  *) echo "usage: [GRID=<name>] bash tools/rent.sh up|status|watch|pull|down"; exit 1 ;;
esac
