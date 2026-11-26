#!/usr/bin/env bash
# Install or remove the opt-in sandbox drop-in for a validator RPC or transport
# unit. Never restarts the service unless --restart is given.
#
#   install.sh [--dry-run] [--restart] install   rpc|transport UNIT
#   install.sh [--dry-run] [--restart] uninstall rpc|transport UNIT
#
# UNIT is the service name on the host, e.g. postfiat-rpc.service (example
# unit) or postfiat-validator-0-rpc.service / postfiat-validator-0.service
# (units written by batch_snapshot.rs). The drop-in lands in
# $SYSTEMD_UNIT_DIR/UNIT.d/50-postfiat-sandbox.conf (default /etc/systemd/system).
# After install the effective unit properties are read back with
# `systemctl show`; a unit that does not verify or does not show every
# directive is rolled back and the script exits 1.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UNIT_DIR="${SYSTEMD_UNIT_DIR:-/etc/systemd/system}"
SYSTEMCTL="${SYSTEMCTL:-systemctl}"
ANALYZE="${SYSTEMD_ANALYZE:-systemd-analyze}"
DROPIN_NAME=50-postfiat-sandbox.conf

usage() {
  sed -n '5,6p' "$0" | sed 's/^# *//' >&2
  exit 2
}

dry=0
restart=0
args=()
for arg in "$@"; do
  case "$arg" in
    --dry-run) dry=1 ;;
    --restart) restart=1 ;;
    -h | --help) usage ;;
    -*) echo "unknown option: $arg" >&2; usage ;;
    *) args+=("$arg") ;;
  esac
done
[ "${#args[@]}" -eq 3 ] || usage
mode="${args[0]}"
kind="${args[1]}"
unit="${args[2]}"
case "$mode" in install | uninstall) ;; *) usage ;; esac
case "$kind" in
  rpc) src="$HERE/postfiat-rpc-sandbox.conf" ;;
  transport) src="$HERE/postfiat-validator-transport-sandbox.conf" ;;
  *) usage ;;
esac
case "$unit" in
  *.service) ;;
  *) echo "UNIT must end in .service: $unit" >&2; exit 2 ;;
esac
case "$unit" in
  */* | .* | *..*) echo "refusing unit name: $unit" >&2; exit 2 ;;
esac
dir="$UNIT_DIR/$unit.d"
dest="$dir/$DROPIN_NAME"

run() {
  if [ "$dry" -eq 1 ]; then
    echo "dry-run: $*"
  else
    "$@"
  fi
}

# Expected `systemctl show` value of a drop-in line.
expected_value() {
  case "$2" in
    true) echo yes ;;
    false) echo no ;;
    EPERM) echo 1 ;;
    *) echo "$2" ;;
  esac
}

sorted_words() { tr ' ' '\n' <<<"$1" | sed '/^$/d' | sort | tr '\n' ' '; }

check_effective() {
  local key line value want bad=0
  while IFS= read -r line; do
    key="${line%%=*}"
    value="$("$SYSTEMCTL" show "$unit" -p "$key" --value)"
    case "$key" in
      RestrictAddressFamilies)
        echo "  $key=$value"
        [ "$(sorted_words "$value")" = "$(sorted_words "${line#*=}")" ] || { echo "  expected $line" >&2; bad=1; }
        ;;
      SystemCallFilter)
        # systemctl expands the group to syscall names; spot-check members.
        if grep -qw read <<<"$value" && ! grep -qw ptrace <<<"$value"; then
          echo "  $key=${line#*=} ($(wc -w <<<"$value") syscalls, ptrace not allowed)"
        else
          echo "  $key unexpected: ${value:0:80}" >&2
          bad=1
        fi
        ;;
      *)
        want="$(expected_value "$key" "${line#*=}")"
        echo "  $key=$value"
        [ "$value" = "$want" ] || { echo "  expected $key=$want" >&2; bad=1; }
        ;;
    esac
  done < <(grep -E '^[A-Za-z]+=' "$src")
  return "$bad"
}

restart_or_note() {
  if [ "$restart" -eq 1 ]; then
    run "$SYSTEMCTL" restart "$unit"
    [ "$dry" -eq 1 ] || "$SYSTEMCTL" is-active --quiet "$unit" || {
      echo "$unit is not active after restart; see: journalctl -u $unit" >&2
      exit 1
    }
  else
    echo "not restarted; takes effect at the next restart: $SYSTEMCTL restart $unit"
  fi
}

if [ "$mode" = uninstall ]; then
  if [ ! -e "$dest" ]; then
    echo "no drop-in at $dest; nothing to remove"
    exit 0
  fi
  run rm -f "$dest"
  run rmdir --ignore-fail-on-non-empty "$dir"
  run "$SYSTEMCTL" daemon-reload
  echo "removed $dest"
  restart_or_note
  exit 0
fi

load_state="$("$SYSTEMCTL" show "$unit" -p LoadState --value)"
[ "$load_state" = loaded ] || {
  echo "$unit is not a loaded unit (LoadState=$load_state)" >&2
  exit 1
}
run install -d -m 0755 "$dir"
run install -m 0644 "$src" "$dest"
run "$SYSTEMCTL" daemon-reload
if [ "$dry" -eq 1 ]; then
  echo "dry-run: would verify $unit and check: $(grep -oE '^[A-Za-z]+' "$src" | tr '\n' ' ')"
  restart_or_note
  exit 0
fi

rollback() {
  echo "rolling back: removing $dest" >&2
  rm -f "$dest"
  rmdir --ignore-fail-on-non-empty "$dir"
  "$SYSTEMCTL" daemon-reload
  exit 1
}
fragment="$("$SYSTEMCTL" show "$unit" -p FragmentPath --value)"
if [ -n "$fragment" ]; then
  "$ANALYZE" verify "$fragment" || rollback
fi
echo "installed $dest; effective properties:"
check_effective || rollback
restart_or_note
