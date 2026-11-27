#!/bin/bash
# Solana deposits need a GPU with more than 60 GB next to the withdrawal prover, which on Colab
# means a G4, and Colab does not always have one (26 November 2026: none for over four hours).
# On such a day this rents one single-GPU machine from Vast for the rest of the window, sets up
# the deposit prover on it and forwards its socket; at the close the machine is destroyed.
# (The stopped four-GPU machine 62235352 is no fallback: its GPUs were rented out, "Required
# resources are currently unavailable", when this was first tried.)
#   vast-deposit-fallback.sh check   rent one if the Colab window has no deposits; give it back if it has
#   vast-deposit-fallback.sh stop    destroy it (run at the close of the window)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")" && pwd)
VAST=$HOME/.local/bin/vastai
LABEL=pft-deposit-fallback
IDF=$ROOT/vast-fallback.id
CM=$HOME/.ssh/cm-vast-deposit
SOCK=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/pft-sol-prover/prover.sock
LOG=$ROOT/window.log
QUERY='num_gpus=1 gpu_name=RTX_5090 cpu_cores_effective>=32 cpu_ram>=60 disk_space>=60 inet_down>=1000 reliability>0.98 verified=true rentable=true direct_port_count>=2 dph_total<1.2'
log() { echo "[$(date -u +%FT%TZ)] $*" | tee -a "$LOG"; }
alert() { "$HOME/.local/bin/b24-alert" "$1 See $LOG" || true; }
field() { $VAST show instances --raw | python3 -c "
import json,sys
i=next((i for i in json.load(sys.stdin) if i['id']==$1 and i.get('label')=='$LABEL'),None)
print('missing' if i is None else ' '.join(str(x) for x in $2))"; }
rsh() { ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile="$ROOT/vast-fallback.known" -o LogLevel=ERROR \
  -o ControlMaster=auto -o ControlPath="$CM" -o ControlPersist=yes -o ServerAliveInterval=20 -o ConnectTimeout=15 -p "$PORT" "root@$HOST" "$@"; }
alive() { python3 -c "import socket,sys;s=socket.socket(socket.AF_UNIX);s.settimeout(8);s.connect(sys.argv[1])" "$SOCK" 2>/dev/null; }

stop() {
  ssh -O exit -o ControlPath="$CM" root@vast 2>/dev/null || true
  # Every machine carrying this label was rented by this script; none is kept.
  for id in $($VAST show instances --raw | python3 -c "
import json,sys
print(' '.join(str(i['id']) for i in json.load(sys.stdin) if i.get('label')=='$LABEL'))"); do
    $VAST destroy instance "$id" -y >/dev/null && log "Vast deposit machine $id destroyed"
  done
  if [ -f "$IDF" ]; then rm -f "$IDF" "$SOCK" "$ROOT/vast-fallback.known"; fi
}

forward() {
  read -r HOST PORT < <(field "$(cat "$IDF")" "(i['public_ipaddr'], i['ports']['22/tcp'][0]['HostPort'])")
  alive && return 0
  mkdir -p -m 700 "$(dirname "$SOCK")"; rm -f "$SOCK"
  rsh true
  ssh -O forward -o ControlPath="$CM" -L "$SOCK:/workspace/sol-ingress/speed-i2/prover.sock" "root@$HOST"
  alive && log "deposit prover socket forwarded from Vast"
}

rent() {
  local offer id s=
  for offer in $($VAST search offers "$QUERY" -o dph_total --raw | python3 -c "
import json,sys
print(' '.join(str(o['id']) for o in json.load(sys.stdin)[:4]))"); do
    id=$($VAST create instance "$offer" --image nvidia/cuda:12.8.1-devel-ubuntu24.04 --disk 60 --ssh --direct --label "$LABEL" --raw | python3 -c "import json,sys;print(json.load(sys.stdin).get('new_contract',''))" || true)
    [ -n "$id" ] || continue
    echo "$id" > "$IDF"; log "rented Vast machine $id (offer $offer) for deposits"
    for _ in $(seq 32); do sleep 15; s=$(field "$id" "(i.get('actual_status'),)"); [ "$s" = running ] && break; done
    if [ "$s" = running ]; then
      read -r HOST PORT < <(field "$id" "(i['public_ipaddr'], i['ports']['22/tcp'][0]['HostPort'])")
      for _ in $(seq 20); do rsh true 2>/dev/null && break; sleep 6; done
      if tar -C "$ROOT" -cf - bundle/dep fetch-circuits.py vast-deposits-remote.sh | rsh 'mkdir -p /content && tar -C /content -xf - && bash /content/vast-deposits-remote.sh' 2>&1 | tail -3 | tee -a "$LOG" | grep -q DEPOSITS-WARM; then
        forward; return 0
      fi
    fi
    log "machine $id did not come up (state ${s:-none}); trying the next offer"
    ssh -O exit -o ControlPath="$CM" root@vast 2>/dev/null || true
    $VAST destroy instance "$id" -y >/dev/null || true; rm -f "$IDF"
  done
  alert "No Vast machine could be set up for deposits; Solana deposits stay closed."; return 1
}

check() {
  exec 8>"$ROOT/vast-fallback.lock"; flock -n 8 || exit 0
  if [ "$(cat "$ROOT/window.mode" 2>/dev/null)" = deposits ]; then
    [ -f "$IDF" ] && { log "Colab now serves deposits"; stop; }
    return 0
  fi
  if [ -f "$IDF" ] && [ "$(field "$(cat "$IDF")" "(i.get('actual_status'),)")" = running ]; then forward; return 0; fi
  [ -f "$IDF" ] && stop
  log "no deposit-capable Colab GPU: renting a Vast machine for deposits"
  rent
}

case ${1:-} in check) check;; stop) stop;; *) sed -n 2,10p "$0"; exit 2;; esac
