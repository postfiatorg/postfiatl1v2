#!/bin/bash
# Alert paths of colab-prover.sh, with no host contacted: the script runs from a scratch copy, with
# stand-ins for colab, ssh, ss and systemctl on PATH and the alert command appending to a file.
#   bash ops/colab-prover/test_alerts.sh
set -uo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
pass=0 fail=0
check() { if eval "$2"; then pass=$((pass+1)); echo "ok   $1"; else fail=$((fail+1)); echo "FAIL $1"; fi; }

setup() {  # a scratch copy of the launcher and its stand-ins
  T=$(mktemp -d); export HOME=$T/home XDG_RUNTIME_DIR=$T/run ALERTS=$T/alerts
  mkdir -p "$HOME/.local/bin" "$HOME/.ssh" "$XDG_RUNTIME_DIR" "$T/bin" "$T/root/bundle" "$T/root/solexit"
  cp "$HERE/colab-prover.sh" "$T/root/"; ROOT=$T/root; : > "$ALERTS"; echo "Host *" > "$HOME/.ssh/config"
  touch "$ROOT/provision.sh" "$ROOT/warm.sh" "$ROOT/fetch-circuits.py" "$ROOT/solexit/hostkey" \
        "$ROOT/solexit/authorized_keys" "$ROOT/solexit/pf-sol-exit-prove"
  printf '#!/bin/sh\necho "$*" >> "$ALERTS"\n' > "$HOME/.local/bin/b24-alert"
  # colab: no session; `new` gives a machine only when FAKE_MACHINE=1.
  cat > "$HOME/.local/bin/colab" <<'EOS'
#!/bin/sh
for a; do case $a in new) [ "${FAKE_MACHINE:-0}" = 1 ] && echo "Session READY"; exit 0;; sessions) exit 0;; stop) exit "${FAKE_STOP:-0}";; esac; done
EOS
  # ssh: control requests succeed; the remote commands answer as a healthy session, except the door
  # self-test, which fails when FAKE_DOOR=1.
  cat > "$T/bin/ssh" <<'EOS'
#!/bin/sh
for a; do [ "$a" = -O ] && exit 0; done
for a; do last=$a; done
case $last in
  *nvidia-smi*) echo 96000;;
  *provision.sh*) cat >/dev/null; echo PROVISIONED;;
  *warm.sh*) echo WARM;;
  *services-reach-colab*) echo services-reach-colab;;
  pf-sol-exit-check) exit "${FAKE_DOOR:-0}";;
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
open_window() { mkdir -p "$HOME/.ssh/config.d"; : > "$HOME/.ssh/config.d/colab-prover.conf"; echo "${1:-none}" > "$ROOT/window.mode"; }

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

echo "$pass passed, $fail failed"
[ "$fail" = 0 ]
