#!/usr/bin/env bash
# Rebuild the witness, produce a pinned Groth16 proof and crash/restart the stub job.
set -euo pipefail
repo=$(cd "$(dirname "$0")/../.." && pwd)
cd "$repo"
run=$(realpath -m "${1:-target/sol-relay-gate}")
source_root=${2:-"$run/source"}
if [[ ! -s "$source_root/signature" ]]; then
  echo "Supply a captured stub fixture: gate.sh RUN_DIRECTORY CAPTURE_DIRECTORY" >&2
  exit 2
fi
mkdir -p "$run"
build_target=${CARGO_TARGET_DIR:-}
host=${SOL_RELAY_HOST:-"$repo/tools/pfusdc-sol-ingress-host/target/release/pfusdc-sol-ingress-host"}
if [[ ! -x "$host" ]]; then
  host_target=${build_target:-"$repo/tools/pfusdc-sol-ingress-host/target"}
  if [[ -n ${SOL_RELAY_HOST:-} ]]; then
    echo "Configured SOL_RELAY_HOST is not executable: $host" >&2
    exit 1
  fi
  host_target=$(realpath -m "$host_target")
  (cd tools/pfusdc-sol-ingress-host && cargo build --target-dir "$host_target" --locked --release --features cuda)
  host="$host_target/release/pfusdc-sol-ingress-host"
fi
capture=${SOL_CAPTURE_BIN:-"$repo/tools/pfusdc-sol-capture/target/release/pfusdc-sol-capture"}
if [[ ! -x "$capture" ]]; then
  if [[ -n ${SOL_CAPTURE_BIN:-} ]]; then
    echo "Configured SOL_CAPTURE_BIN is not executable: $capture" >&2
    exit 1
  fi
  capture_target=$(realpath -m "${build_target:-"$repo/tools/pfusdc-sol-capture/target"}")
  if [[ -z ${LIBCLANG_PATH:-} && -d /home/postfiat/.local/share/libclang-18 ]]; then
    LIBCLANG_PATH=$(dirname "$(find /home/postfiat/.local/share/libclang-18 -name libclang.so -print -quit)")
    export LIBCLANG_PATH
    export LD_LIBRARY_PATH="$LIBCLANG_PATH${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    export BINDGEN_EXTRA_CLANG_ARGS="-I$(gcc -print-file-name=include)"
  fi
  (cd tools/pfusdc-sol-capture && cargo +1.99.1 rustc --target-dir "$capture_target" --locked --release -- -C link-arg=-lstdc++)
  capture="$capture_target/release/pfusdc-sol-capture"
fi
tools/pfusdc-sol-relay/relay.py fixture-config --directory "$run" --capture-root "$source_root" \
    --host "$host" --capture-binary "$capture" --prover "${SOL_RELAY_PROVER:-auto}"
exec tools/pfusdc-sol-relay/gate.py "$run/config.json"
