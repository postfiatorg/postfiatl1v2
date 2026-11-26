#!/bin/bash
# One Colab GPU session that stands in for the two Vast provers during a proving window.
#   colab-prover.sh up [deposits]   start a session, set it up, warm it, point the services at it
#   colab-prover.sh down            point the services away and stop the session
#   colab-prover.sh ensure          put back the local port and socket if the connection was lost
#   colab-prover.sh check           alert if the window is not open (the 17:05 UTC timer)
#   colab-prover.sh status
# The services keep their own commands (ssh/scp to root@ssh8.vast.ai): while a window is open an
# ssh_config stanza sends that name through one multiplexed connection to the Colab VM, because
# Colab allows one SSH connection per runtime. Removing the stanza is the rollback.
# -E: the ERR trap that `up` sets also covers the functions it calls (ensure, direct).
set -Eeuo pipefail
ROOT=$(cd "$(dirname "$0")" && pwd)
COLAB="$HOME/.local/bin/colab --auth=adc"
NAME=${COLAB_PROVER_SESSION:-pft-prover}
STANZA=$HOME/.ssh/config.d/colab-prover.conf
CM=$HOME/.ssh/cm-colab-prover
KNOWN=$HOME/.ssh/known_hosts.colab-prover
SOCK=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/pft-sol-prover/prover.sock
LOG=$ROOT/window.log
log() { echo "[$(date -u +%FT%TZ)] $*" | tee -a "$LOG"; }
alert() { "$HOME/.local/bin/b24-alert" "$*" || true; }
# Before the stanza exists, and for the first connection, talk to the VM directly.
direct() { ssh -o BatchMode=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile="$KNOWN" -o LogLevel=ERROR \
  -o ControlMaster=auto -o ControlPath="$CM" -o ControlPersist=yes -o ServerAliveInterval=20 \
  -o ProxyCommand="$COLAB ssh --proxy-mode -s $NAME" root@colab-prover "$@"; }
bounded_direct() { timeout --kill-after=2 "${COLAB_PROVER_STEP_SECONDS:-15}" ssh -o BatchMode=yes -o StrictHostKeyChecking=no \
  -o UserKnownHostsFile="$KNOWN" -o LogLevel=ERROR -o ControlMaster=auto -o ControlPath="$CM" -o ControlPersist=yes \
  -o ServerAliveInterval=20 -o ProxyCommand="$COLAB ssh --proxy-mode -s $NAME" root@colab-prover "$@"; }

# The local ends of the window: the withdrawal door's port and the deposit prover's socket. Both ride
# on the one connection; if it was lost they are put back. Safe to run at any time in a window.
# Each step runs whatever the others did (a failing door check does not keep the deposit socket from
# being put back); the steps that failed are named, and ensure fails if any did.
# Each remote command of a step has a whole-command deadline (COLAB_PROVER_STEP_SECONDS, 15 s). A step
# runs at most two of them (the deposit step also an 8 s socket probe): 30 + 30 + 38 s, inside the ensure
# unit's two-minute limit. A timeout is an ordinary failed step.
STEP_SECONDS=${COLAB_PROVER_STEP_SECONDS:-15}
bounded() { timeout --kill-after=2 "$STEP_SECONDS" "$@"; }
ensure_connection() {
  bounded ssh -O check -o ControlPath="$CM" root@colab-prover 2>/dev/null || bounded_direct true || return 1
}
ensure_door() {
  if ! ss -ltn | grep -q "127.0.0.1:20628 "; then
    bounded ssh -O forward -o ControlPath="$CM" -L 127.0.0.1:20628:127.0.0.1:2223 root@colab-prover || return 1
    log "withdrawal door forwarded"
  fi
  bounded ssh -F /dev/null -oIdentitiesOnly=yes -i "$ROOT/solexit/testkey" -oStrictHostKeyChecking=yes -oGlobalKnownHostsFile=/dev/null \
    -oUserKnownHostsFile="$ROOT/solexit/known_hosts.colab" -oBatchMode=yes -oConnectTimeout=8 -oHostKeyAlias='[ssh8.vast.ai]:20628' \
    -p 20628 root@127.0.0.1 pf-sol-exit-check || return 1
}
ensure_deposits() {
  [ "$(cat "$ROOT/window.mode" 2>/dev/null)" = deposits ] || return 0
  bounded systemctl --user stop pft-sol-prover-tunnel.service 2>/dev/null || true
  if ! python3 -c "import socket,sys;s=socket.socket(socket.AF_UNIX);s.settimeout(8);s.connect(sys.argv[1])" "$SOCK" 2>/dev/null; then
    mkdir -p -m 700 "$(dirname "$SOCK")" || return 1
    rm -f "$SOCK"
    bounded ssh -O forward -o ControlPath="$CM" -L "$SOCK:/workspace/sol-ingress/speed-i2/prover.sock" root@colab-prover || return 1
    log "deposit prover socket forwarded"
  fi
}
ensure() {
  [ -f "$STANZA" ] || return 0
  local failed=()
  ensure_connection || failed+=("connection")
  ensure_door || failed+=("withdrawal door")
  ensure_deposits || failed+=("deposit socket")
  [ ${#failed[@]} = 0 ] || { log "ensure failed: ${failed[*]}"; return 1; }
}

# The timer's ensure: after three failed runs in a row in an open window it alerts once, and once
# more when a run works again. The count of failed runs lives beside window.mode.
FAILS=$ROOT/window.ensure-failures
ALERTED=$ROOT/window.ensure-alerted
ensure_watch() {
  [ -f "$STANZA" ] || { rm -f "$FAILS" "$ALERTED"; return 0; }
  local before; before=$(cat "$FAILS" 2>/dev/null || echo 0)
  # A run the unit's limit cut short left its attempt counted: its alert is sent now.
  if [ "$before" -ge 3 ] && [ ! -e "$ALERTED" ]; then
    : > "$ALERTED"
    alert "Colab proving window has failed three checks in a row; deposits and withdrawals may be closed. See $LOG"
  fi
  echo $((before + 1)) > "$FAILS"  # this attempt counts until it succeeds
  if ensure; then
    if [ -e "$ALERTED" ]; then
      log "ensure recovered after $before failed runs"
      alert "Colab proving window recovered after $before failed checks. See $LOG"
    fi
    rm -f "$FAILS" "$ALERTED"
    return 0
  fi
  if [ $((before + 1)) -ge 3 ] && [ ! -e "$ALERTED" ]; then
    : > "$ALERTED"
    alert "Colab proving window has failed three checks in a row; deposits and withdrawals may be closed. See $LOG"
  fi
  return 1
}

# The open's failure handler: only the shell running `up` cleans up and alerts (a failure inside a
# command substitution reaches it once, through the failed assignment). It keeps the original
# status, does each step best-effort and always attempts the alert.
open_failed() {
  local status=$1
  [ "$BASHPID" = "${UP_SHELL:-}" ] || exit "$status"
  trap - ERR; set +e
  log "window failed to open" 2>/dev/null
  rm -f "$STANZA" "$ROOT/window.mode" "$ROOT/window.opened" 2>/dev/null
  timeout 180 $COLAB stop -s "$NAME" >/dev/null 2>&1
  alert "Colab proving window failed to open; deposits and withdrawals stay closed. See $LOG"
  exit "$status"
}

up() {
  local deposits=${1:-}
  UP_SHELL=$BASHPID
  trap 'open_failed $?' ERR
  # A window record from another day is stale: the services stop pointing at it before anything else.
  if [ "$(cat "$ROOT/window.opened" 2>/dev/null)" != "$(date -u +%F)" ]; then
    rm -f "$STANZA" "$ROOT/window.mode" "$ROOT/window.opened"
  fi
  if $COLAB sessions 2>/dev/null | grep -q "^\[$NAME\]"; then log "session $NAME already exists"; else
    local got=
    # G4 first: 48 cores and 96 GB of GPU memory (measured 2026-11-25). Deposit proving is limited by
    # CPU on the 12-core A100 shapes, and the deposit prover and a withdrawal prover fit together
    # only on a GPU with more than 60 GB.
    # Colab's supply varies: on 26 November a G4 came in 5 seconds at 23:40 UTC and nothing at all
    # came for 43 minutes from 02:26. Keep asking, every shape in turn, until the deadline.
    local deadline=$(( $(date +%s) + 9000 )) round=0
    [ -n "${COLAB_PROVER_UNTIL:-}" ] && deadline=$(date -u -d "today ${COLAB_PROVER_UNTIL}" +%s)
    while [ -z "$got" ] && [ "$(date +%s)" -lt "$deadline" ]; do
      round=$((round+1))
      for shape in "G4" "A100 --high-mem" "A100" "H100"; do
        if timeout 240 $COLAB new -s "$NAME" --gpu $shape 2>&1 | grep -q "Session READY"; then got=$shape; break; fi
      done
      [ -n "$got" ] || { [ $((round % 5)) = 1 ] && log "no GPU yet (round $round)"; sleep 45; }
    done
    if [ -z "$got" ]; then
      log "no GPU available from Colab before the deadline"
      rm -f "$STANZA" "$ROOT/window.mode" "$ROOT/window.opened"
      alert "Colab proving window did not open: no GPU before the deadline; deposits and withdrawals stay closed. See $LOG"
      exit 75
    fi
    log "session $NAME on $got"
  fi
  rm -f "$KNOWN"; ssh -O exit -o ControlPath="$CM" root@colab-prover 2>/dev/null || true
  tar -C "$ROOT" -cf - bundle provision.sh warm.sh fetch-circuits.py solexit/hostkey solexit/authorized_keys solexit/pf-sol-exit-prove | direct 'mkdir -p /content && tar -C /content -xf - && bash /content/provision.sh' 2>&1 | tail -3 | tee -a "$LOG"
  local vram; vram=$(direct 'nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits')
  # Under 60 GB the deposit prover (24.5 GB) leaves no room for a withdrawal prover (23 GB).
  if [ "$deposits" = deposits ] && [ "$vram" -lt 60000 ]; then log "GPU has ${vram} MiB: deposits stay closed this window"; deposits=; fi
  direct "bash /content/warm.sh $deposits" 2>&1 | tail -3 | tee -a "$LOG"
  if [ -n "${COLAB_PROVER_NO_SWITCH:-}" ]; then log "set up and warm; services not switched"; return; fi
  mkdir -p "$(dirname "$STANZA")"
  cat > "$STANZA" <<EOF
# Written by colab-prover.sh up; removed by down. Sends the Vast prover's name to the Colab session.
Host ssh8.vast.ai
    HostKeyAlias colab-prover
    UserKnownHostsFile $KNOWN
    StrictHostKeyChecking yes
    ProxyCommand $COLAB ssh --proxy-mode -s $NAME
    ControlMaster auto
    ControlPath $CM
    ControlPersist yes
    ServerAliveInterval 20
EOF
  chmod 600 "$STANZA"
  grep -q "^Include config.d/\*" "$HOME/.ssh/config" 2>/dev/null || { printf 'Include config.d/*\n\n' | cat - "$HOME/.ssh/config" 2>/dev/null > "$HOME/.ssh/config.new"; mv "$HOME/.ssh/config.new" "$HOME/.ssh/config"; chmod 600 "$HOME/.ssh/config"; }
  ssh -o BatchMode=yes -p 20628 root@ssh8.vast.ai 'test -x /workspace/b23-withdraw-20261116/proof-host && echo services-reach-colab' | tee -a "$LOG"
  echo "${deposits:-none}" > "$ROOT/window.mode"
  ensure
  date -u +%F > "$ROOT/window.opened"  # the dated record `check` asks for
  log "window open"
}

# The evening check after the opening time: a window that is not open is said the same evening,
# also when the open never ran (the relay host down at its start) or failed without a word.
check() {
  local mode opened live=no; mode=$(cat "$ROOT/window.mode" 2>/dev/null || true)
  opened=$(cat "$ROOT/window.opened" 2>/dev/null || true)
  timeout 60 $COLAB sessions 2>/dev/null | grep -q "^\[$NAME\]" && live=yes
  if [ -f "$STANZA" ] && { [ "$mode" = deposits ] || [ "$mode" = none ]; } && [ "$opened" = "$(date -u +%F)" ] && [ $live = yes ]; then
    log "check: window open (deposits: $mode)"; return 0
  fi
  log "check: window not open (window.mode: ${mode:-absent}, opened: ${opened:-never}, session alive: $live)"
  alert "Colab proving window is not open at its check time; deposits and withdrawals are closed. See $LOG"
  return 1
}

down() {
  # Every close action is attempted; each failure is named, and the close alerts once at the end.
  local failed=() f
  for f in "$ROOT/window.mode" "$ROOT/window.opened" "$FAILS" "$ALERTED"; do
    rm -f "$f" 2>/dev/null || failed+=("removing $f")
  done
  timeout 30 ssh -O exit -o ControlPath="$CM" root@colab-prover 2>/dev/null || true
  for f in "$STANZA" "$SOCK"; do
    rm -f "$f" 2>/dev/null || failed+=("removing $f")
  done
  timeout 180 $COLAB stop -s "$NAME" >>"$LOG" 2>&1 || failed+=("stopping the session $NAME")
  if ! $COLAB sessions >"$ROOT/window.sessions" 2>&1; then
    failed+=("listing the sessions")
  elif grep -q "^\[$NAME\]" "$ROOT/window.sessions"; then
    failed+=("the session $NAME is still listed")
  fi
  tail -2 "$ROOT/window.sessions" >>"$LOG" 2>/dev/null || true
  if [ ${#failed[@]} != 0 ]; then
    log "window close incomplete: ${failed[*]}" || true
    alert "Colab proving window did not close cleanly: ${failed[*]}. See $LOG"
    return 1
  fi
  log "window closed" || true
}

case ${1:-} in
  up) shift; up "$@";;
  down) down;;
  ensure) ensure_watch;;
  check) check;;
  status) $COLAB sessions 2>&1 | tail -3; $COLAB usage 2>&1 | head -3; ls -la "$STANZA" "$SOCK" 2>&1 | cut -c1-120;;
  *) sed -n 2,7p "$0"; exit 2;;
esac
