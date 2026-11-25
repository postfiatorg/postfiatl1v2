#!/usr/bin/env bash
# Run on the dedicated four-GPU host from its speed2 installation.
set -euo pipefail
cd /workspace/sol-ingress/speed-i2
# The deposit prover takes CUDA devices 0 to gpus-1. On a machine shared with the withdrawal
# prover both read their devices from one root-owned settings file (RUNBOOK-LIVE.md, "One proving
# machine"): DEPOSIT_CUDA_DEVICES here, EXIT_CUDA_DEVICE (default 0) for the withdrawal. Without
# the file the machine is the deposit prover's alone: four devices, or SOL_DEPOSIT_GPUS.
conf=/etc/pf-sol-exit-prove.conf
conf_owner=0
refuse() { echo "prover-start: $*" >&2; exit 78; }
if [[ -e $conf ]]; then
    if [[ $(stat -c %u "$conf") != "$conf_owner" || -n $(find "$conf" -maxdepth 0 -perm /022) ]]; then
        refuse "$conf must be owned by root and writable by no one else"
    fi
    withdraw=0 deposit=
    while IFS='=' read -r key value || [[ -n $key ]]; do
        case $key in
            EXIT_CUDA_DEVICE) withdraw=$value ;;
            DEPOSIT_CUDA_DEVICES) deposit=$value ;;
        esac
    done < "$conf"
    # The same written form as the forced command accepts, so both compare like with like.
    [[ $withdraw =~ ^(0|[1-9][0-9]?)$ ]] || refuse "EXIT_CUDA_DEVICE=$withdraw in $conf: a device is 0 to 99, no leading zero"
    [[ -n $deposit ]] || refuse "$conf names no DEPOSIT_CUDA_DEVICES"
    gpus=$(( $(tr -cd , <<<"$deposit" | wc -c) + 1 ))
    # serve.py takes a count and uses devices 0 to count-1, so the list must be exactly those.
    [[ $deposit == "$(seq -s, 0 $(( gpus - 1 )))" ]] || refuse "DEPOSIT_CUDA_DEVICES=$deposit must list 0 to n-1"
    [[ ,$deposit, != *,$withdraw,* ]] || refuse "DEPOSIT_CUDA_DEVICES=$deposit includes $withdraw, also the withdrawal device"
    if [[ -n ${SOL_DEPOSIT_GPUS:-} && $SOL_DEPOSIT_GPUS != "$gpus" ]]; then
        refuse "SOL_DEPOSIT_GPUS=$SOL_DEPOSIT_GPUS contradicts DEPOSIT_CUDA_DEVICES=$deposit in $conf; unset it"
    fi
else
    gpus=${SOL_DEPOSIT_GPUS:-4}
fi
[[ $gpus =~ ^[1-9]$ ]] || refuse "bad device count $gpus"
export RAYON_NUM_THREADS=32
# Measured on a 19 MB mainnet block: 8 core workers per GPU cut leaves 36 s -> 28 s and the
# final proof 39 s -> 29 s; the final proof is still verified before it is returned.
export SP1_WORKER_NUM_CORE_WORKERS=8 SP1_WORKER_CORE_BUFFER_SIZE=8 SP1_WORKER_VERIFY_INTERMEDIATES=false
export SP1_GROTH16_CIRCUIT_PATH="$PWD/circuit-install"
# The SP1 installer uses this marker to recognize a completed artifact install.
(cd circuit-install/v6.1.0 && sha256sum -c ../../circuits.sha256 && touch .complete)
# Clear what a dead prover leaves behind, but only while no running prover holds prover.lock:
# its socket, which serve.py refuses to start over, and its proving processes, which outlive
# serve.py and keep the GPU memory, so the next prover runs out of memory on its first proof.
# A GPU server whose proof host is gone (its parent is init or any process other than proof-host
# or host-live) is stopped on any device, such as one left
# on device 3 when the count went from four to three. A server with a live parent is never
# stopped: on one of this prover's devices it refuses, naming the device and the process (the
# withdrawal prover's, when the settings file is missing); elsewhere it is left alone.
rc=0
flock -n -E 75 prover.lock bash -c 'pkill -9 -f "^$PWD/host-live serve "
    for _ in {1..50}; do pgrep -f "^$PWD/host-live serve " >/dev/null || break; sleep 0.1; done
    for pid in $(pgrep -x sp1-gpu-server); do
        dev=$(tr "\0" "\n" < "/proc/$pid/environ" | sed -n "s/^CUDA_VISIBLE_DEVICES=//p")
        [[ $dev =~ ^[0-9]+$ ]] || continue
        parent=$(awk "{print \$4}" "/proc/$pid/stat" 2>/dev/null) || continue
        owner=$(cat "/proc/$parent/comm" 2>/dev/null || echo unknown)
        # Only a proof host keeps a server: proof-host (the withdrawal prover, started by the forced
        # command) or host-live (a deposit prover, started by serve.py). Any other parent, init or a
        # subreaper such as tini or a user systemd, means its proof host is gone: a leftover.
        if [[ $parent == 1 || ( $owner != proof-host && $owner != host-live ) ]]; then kill -9 "$pid"; continue; fi
        if (( dev < $1 )); then
            echo "prover-start: device $dev is used by GPU server $pid of live process $parent ($owner); not stopped" >&2
            exit 78
        fi
    done
    rm -f prover.sock' _ "$gpus" || rc=$?
# 75: another prover holds the lock, as before nothing is cleared; anything else is a refusal.
[[ $rc == 0 || $rc == 75 ]] || exit "$rc"
exec python3 serve.py --lock prover.lock --prime prime.bin ./host-live elf "$PWD/prover.sock" "$gpus"
