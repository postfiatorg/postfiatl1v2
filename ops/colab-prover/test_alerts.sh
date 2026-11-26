#!/bin/bash
# Alert paths of colab-prover.sh, with no host contacted: the script runs from a scratch copy, with
# stand-ins for colab, ssh, ss and systemctl on PATH and the alert command appending to a file.
#   bash ops/colab-prover/test_alerts.sh
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
pass=0 fail=0
check() { if eval "$2"; then pass=$((pass+1)); echo "ok   $1"; else fail=$((fail+1)); echo "FAIL $1"; fi; }

setup() {  # a scratch copy of the launcher and its stand-ins
  T=$(mktemp -d); export T HOME=$T/home XDG_RUNTIME_DIR=$T/run ALERTS=$T/alerts
  mkdir -p "$HOME/.local/bin" "$HOME/.ssh" "$XDG_RUNTIME_DIR" "$T/bin" "$T/root/bundle" "$T/root/solexit"
  cp "$HERE/colab-prover.sh" "$T/root/"; ROOT=$T/root; : > "$ALERTS"; echo "Host *" > "$HOME/.ssh/config"
  touch "$ROOT/provision.sh" "$ROOT/warm.sh" "$ROOT/fetch-circuits.py" "$ROOT/solexit/hostkey" \
        "$ROOT/solexit/authorized_keys" "$ROOT/solexit/pf-sol-exit-prove"
  printf '#!/bin/sh\necho "$*" >> "$ALERTS"\n' > "$HOME/.local/bin/b24-alert"
  # colab: no session; `new` gives a machine only when FAKE_MACHINE=1.
  cat > "$HOME/.local/bin/colab" <<'EOS'
#!/bin/sh
for a; do case $a in
  new) [ "${FAKE_MACHINE:-0}" = 1 ] && { touch "$T/session"; echo "Session READY"; }; exit 0;;
  sessions) [ -e "$T/session" ] && echo "[pft-prover] G4"; exit 0;;
  stop) echo stop >> "$T/stops"; [ "${FAKE_STOP:-0}" = 0 ] && rm -f "$T/session"; exit "${FAKE_STOP:-0}";;
esac; done
EOS
  # ssh: control requests succeed; the remote commands answer as a healthy session, except the door
  # self-test, which fails when FAKE_DOOR=1.
  cat > "$T/bin/ssh" <<'EOS'
#!/bin/sh
for a; do [ "$a" = -O ] && exit 0; done
for a; do last=$a; done
case $last in
  *nvidia-smi*) [ -n "${FAKE_VRAM_FAIL:-}" ] && exit 23; echo 96000;;
  *provision.sh*) cat >/dev/null; echo PROVISIONED;;
  *warm.sh*) echo WARM;;
  *services-reach-colab*) echo services-reach-colab;;
  pf-sol-exit-check) [ "${FAKE_DOOR:-0}" = hang ] && exec sleep 30; exit "${FAKE_DOOR:-0}";;
esac
exit 0
EOS
  printf '#!/bin/sh\necho "LISTEN 0 128 127.0.0.1:20628 0.0.0.0:*"\n' > "$T/bin/ss"
  printf '#!/bin/sh\nexit 0\n' > "$T/bin/systemctl"
  chmod +x "$HOME/.local/bin/"* "$T/bin/"*
  export PATH=$T/bin:$PATH
}
alerts() { grep -c . "$ALERTS" || true; }
run() { bash "$ROOT/colab-prover.sh" "$@" >>"$T/out" 2>&1; }
open_window() {  # a window as a successful open leaves it today: stanza, mode, dated record and a live session
  mkdir -p "$HOME/.ssh/config.d"; : > "$HOME/.ssh/config.d/colab-prover.conf"; echo "${1:-none}" > "$ROOT/window.mode"
  date -u +%F > "$ROOT/window.opened"; touch "$T/session"
}

setup; COLAB_PROVER_UNTIL=00:00 run up deposits; rc=$?
check "no machine before the limit: exit 75" '[ $rc = 75 ]'
check "no machine before the limit: one alert" '[ "$(alerts)" = 1 ]'

setup; FAKE_MACHINE=1 FAKE_DOOR=1 run up; rc=$?
check "a failure inside ensure during up: up fails" '[ $rc != 0 ]'
check "a failure inside ensure during up: one alert" '[ "$(alerts)" = 1 ]'
check "a failure inside ensure during up: services not left pointed at the session" '[ ! -e "$HOME/.ssh/config.d/colab-prover.conf" ]'

setup; FAKE_MACHINE=1 run up; rc=$?
check "a healthy up opens the window without an alert" '[ $rc = 0 ] && [ "$(alerts)" = 0 ] && [ -f "$ROOT/window.mode" ]'

setup; open_window
for n in 1 2 3; do FAKE_DOOR=1 run ensure; done
check "three failing ensure runs: one alert" '[ "$(alerts)" = 1 ]'
FAKE_DOOR=1 run ensure
check "a fourth failing ensure run: no further alert" '[ "$(alerts)" = 1 ]'
run ensure; run ensure
check "ensure working again: one recovered line" '[ "$(alerts)" = 2 ] && grep -q recovered "$ALERTS"'

setup; open_window deposits; FAKE_DOOR=1 run ensure
check "a failing door check still puts the deposit socket back" 'grep -q "deposit prover socket" "$ROOT/window.log"'

setup; run check
check "17:05 check without window.mode: alert" '[ "$(alerts)" = 1 ]'
setup; open_window closed; run check
check "17:05 check with window.mode saying closed: alert" '[ "$(alerts)" = 1 ]'
setup; open_window none; run check
check "17:05 check with the window open: silent" '[ "$(alerts)" = 0 ]'

setup; open_window; FAKE_STOP=1 run down; rc=$?
check "down whose session stop failed: not 'window closed', says what failed" \
  '[ $rc != 0 ] && ! grep -q "window closed" "$ROOT/window.log" && grep -q "stopping the session" "$ROOT/window.log"'

# A01: a step that hangs is cut at its own deadline, the other steps still run, and the count is kept.
setup; open_window deposits
for n in 1 2 3; do FAKE_DOOR=hang COLAB_PROVER_STEP_SECONDS=1 timeout 20 bash "$ROOT/colab-prover.sh" ensure >>"$T/out" 2>&1; done
check "a hanging door check: three runs alert once" '[ "$(alerts)" = 1 ]'
check "a hanging door check: the deposit socket is still put back" 'grep -q "deposit prover socket" "$ROOT/window.log"'
setup; open_window none
FAKE_DOOR=hang COLAB_PROVER_STEP_SECONDS=30 timeout 2 bash "$ROOT/colab-prover.sh" ensure >>"$T/out" 2>&1
check "a run cut short by the unit's limit still records its failed attempt" '[ "$(cat "$ROOT/window.ensure-failures" 2>/dev/null)" = 1 ]'

# A02: only today's successful open, with its session alive, passes the check.
setup; open_window none; echo 2000-01-01 > "$ROOT/window.opened"; run check
check "check with yesterday's record: alert" '[ "$(alerts)" = 1 ]'
setup; open_window none; rm -f "$T/session"; run check
check "check with no live session: alert" '[ "$(alerts)" = 1 ]'
setup; open_window deposits; run check
check "check with today's deposits window: silent" '[ "$(alerts)" = 0 ]'
setup; open_window none; rm -f "$T/session"; echo 2000-01-01 > "$ROOT/window.opened"; COLAB_PROVER_UNTIL=00:00 run up
check "no machine: stale open state is removed" '[ ! -e "$ROOT/window.mode" ] && [ ! -e "$HOME/.ssh/config.d/colab-prover.conf" ]'

# A03: a logging or cleanup error does not skip the alert or the rest of the close.
setup; mkdir -p "$ROOT/window.log"; FAKE_MACHINE=1 FAKE_DOOR=1 run up
check "a failed open with an unwritable log still alerts once" '[ "$(alerts)" = 1 ]'
setup; open_window none; rm -f "$ROOT/window.mode"; mkdir "$ROOT/window.mode"; : > "$ROOT/window.mode/x"; run down; rc=$?
check "down with a removal error still stops the session and alerts" '[ $rc != 0 ] && grep -q stop "$T/stops" && [ "$(alerts)" = 1 ] && [ ! -e "$HOME/.ssh/config.d/colab-prover.conf" ]'

# A04: a failure inside a command substitution is handled once.
setup; FAKE_MACHINE=1 FAKE_VRAM_FAIL=1 run up
check "a failed GPU query alerts once and stops the session once" '[ "$(alerts)" = 1 ] && [ "$(grep -c stop "$T/stops")" = 1 ]'

echo "$pass passed, $fail failed"
[ "$fail" = 0 ]
