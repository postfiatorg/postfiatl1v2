#!/bin/bash
# Forced command for the dedicated credential on the prover.
set -euo pipefail
umask 077
work=/workspace/b23-withdraw-20261116
lock=/workspace/b23-gpu/prover.lock
device=
deposit=
# Without this file the prover is the single-GPU host as before: device 0 and the paths above.
# On the four-GPU machine shared with the deposit prover it names the device, the staged tree
# and its SP1 home (RUNBOOK-LIVE.md, "One proving machine").
conf=/etc/pf-sol-exit-prove.conf
conf_owner=0
if [[ -e $conf ]]; then
    # The file chooses the device, the tree and the SP1 home that run: root's alone.
    if [[ $(stat -c %u "$conf") != "$conf_owner" || -n $(find "$conf" -maxdepth 0 -perm /022) ]]; then
        echo "pf-sol-exit-prove: $conf must be owned by root and writable by no one else" >&2
        exit 78
    fi
    # A last line without a newline is read too.
    while IFS='=' read -r key value || [[ -n $key ]]; do
        case "$key=$value" in
            =|'#'*) ;;
            EXIT_CUDA_DEVICE=[0-9]|EXIT_CUDA_DEVICE=[0-9][0-9]) device=$value;;
            DEPOSIT_CUDA_DEVICES=*) [[ $value =~ ^[0-9]{1,2}(,[0-9]{1,2})*$ ]] ||
                { echo "pf-sol-exit-prove: bad line in $conf: $key" >&2; exit 78; }
                deposit=$value;;
            EXIT_WORK_DIR=/*) work=$value;;
            EXIT_LOCK=/*) lock=$value;;
            EXIT_SP1_HOME=/*) export HOME=$value;;
            *) echo "pf-sol-exit-prove: bad line in $conf: $key" >&2; exit 78;;
        esac
    done < "$conf"
    # The deposit prover reads its devices from the same file; they must leave this one alone.
    if [[ ,$deposit, == *,${device:-0},* ]]; then
        echo "pf-sol-exit-prove: device ${device:-0} is also in DEPOSIT_CUDA_DEVICES=$deposit in $conf" >&2
        exit 78
    fi
fi
case "${SSH_ORIGINAL_COMMAND:-}" in
    pf-sol-exit-check) cd "$work"; test -x proof-host && test -s egress.elf; exit;;
    pf-sol-exit-checkpoint) kind=checkpoint;;
    pf-sol-exit-withdrawal) kind=withdrawal;;
    *) exit 64;;
esac
cd "$work"
mkdir -p jobs
job=$(mktemp -d jobs/sol-exit-worker.XXXXXXXX)
trap 'rm -rf -- "$job"' EXIT
# Receive at most 24 MiB before reserving the shared device. Only the settings file chooses the
# device: a PROOF_HOST_CUDA_DEVICE in the caller's environment is dropped.
timeout 30 head -c 25165825 > "$job/witness.json"
test "$(wc -c < "$job/witness.json")" -le 25165824
exec 9>"$lock"
flock -E 75 -n 9
# Holding the lock, no other proof of this prover runs. A GPU server on this device whose parent
# is gone (adopted by init: its proof host was killed by the timeout or for memory) is stale and
# stopped. One whose parent still lives belongs to a running prover: refuse, never share.
for pid in $(pgrep -x sp1-gpu-server || true); do
    dev=$(tr '\0' '\n' < "/proc/$pid/environ" 2>/dev/null | sed -n 's/^CUDA_VISIBLE_DEVICES=//p') || continue
    [[ $dev == "${device:-0}" ]] || continue
    parent=$(awk '{print $4}' "/proc/$pid/stat" 2>/dev/null) || continue
    if [[ $parent != 1 ]]; then
        echo "pf-sol-exit-prove: device ${device:-0} is used by GPU server $pid of live process $parent" >&2
        exit 75
    fi
    kill -9 "$pid" 2>/dev/null || true
    for _ in {1..50}; do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done
done
SP1_PROVER=cuda RAYON_NUM_THREADS=4 timeout 1500 env -u PROOF_HOST_CUDA_DEVICE ./proof-host ${device:+--cuda-device "$device"} "$kind" egress.elf "$job/witness.json" "$job/proof" prove > "$job/prove.log" 2>&1
for file in public-values.bin proof-report.json proof-calldata.bin; do
    test "$(wc -c < "$job/proof/$file")" -le 8192
    base64 -w0 "$job/proof/$file"
    echo
done
