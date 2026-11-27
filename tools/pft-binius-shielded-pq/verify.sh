#!/usr/bin/env bash
# Replay the standalone PFSH02 lifecycle, settlement rules, and full epoch transition.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GO_BIN="${GO_BIN:-$(command -v go || echo "$HOME/go/pkg/mod/golang.org/toolchain@v0.0.1-go1.25.1.linux-amd64/bin/go")}"
PY_BIN="${PY_BIN:-$(if [ -x /home/postfiat/repos/pft-deep-ligero-pq/.venv/bin/python ]; then echo /home/postfiat/repos/pft-deep-ligero-pq/.venv/bin/python; else command -v python3; fi)}"
# Normalize relative targets before cargo invocations change directory.
export CARGO_TARGET_DIR="$(realpath -m "${CARGO_TARGET_DIR:-$ROOT/rust-prover/target}")"
export RUST_BIN="$CARGO_TARGET_DIR/release/pft-binius-shielded"
GO_VERIFY="$ROOT/go-verifier/binius-shielded-verifier"
cd "$ROOT"

sha256sum --check --quiet SHA256SUMS
echo "Fixture and replay checksums PASS"

# Run all stages on the same sampled CPU so the comparison has one affinity
# and later processes inherit it, including the Go runtime and Rayon.
cpu="${BENCH_CPU:-$("$PY_BIN" idle_cpu.py)}"
taskset -pc "$cpu" "$$"
export GOMAXPROCS=1 RAYON_NUM_THREADS=1
echo "Replay CPU $cpu"

echo "=== [1/7] Rust unit tests, including signed mint controls and bulk tree equivalence ==="
(cd rust-prover && cargo test --locked --release --lib)

echo "=== [2/7] Rust lifecycle settlement ==="
(cd rust-prover && cargo build --locked --release)
measure_lifecycle() {
    local verifier="$1" prefix="$2" scenario result
    echo "Lifecycle CPU $cpu ($verifier)"
    for scenario in mint hop1 dummy_1in hop2; do
        result="$(taskset -c "$cpu" env GOMAXPROCS=1 RAYON_NUM_THREADS=1 "$verifier" verify \
            "${prefix}tx" "payments/tx_${scenario}.bin" "${prefix}ledger" "payments/ledger_${scenario}.json")"
        echo "$result"
        awk '$(NF) == "ms" && $(NF-1) < 50 {pass=1} END {exit !pass}' <<< "$result"
    done
}
measure_lifecycle "$RUST_BIN" --

echo "=== [3/7] Rust adversarial suite ==="
"$RUST_BIN" adversarial --tx payments/tx_hop1.bin --ledger payments/ledger_hop1.json

echo "=== [4/7] Go unit and authorization tests, lifecycle settlement, full epoch rollover ==="
(cd go-verifier
    "$GO_BIN" test -count=1 -v ./...
    "$GO_BIN" build -o binius-shielded-verifier .
    "$GO_BIN" build -tags purego -o binius-shielded-verifier-purego .
    "$GO_BIN" build -o delivery-helper ./cmd/delivery
)
measure_lifecycle "$GO_VERIFY" -
run_dir="$(mktemp -d "$ROOT/.full-tree.XXXXXXXX")"
trap 'rm -rf -- "$run_dir"' EXIT
echo "Rust full tree CPU $cpu"
check_full_time() {
    local output="$1"
    echo "$output"
    awk -F'parse_verify(_apply)?=' 'NF == 2 && ($2 + 0) < 50 {pass=1} END {exit !pass}' <<< "$output"
}
rust_result="$(/usr/bin/time -f 'Rust peak RSS %M KiB, elapsed %e s' \
    taskset -c "$cpu" env RAYON_NUM_THREADS=1 "$RUST_BIN" full-tree --out "$run_dir")"
check_full_time "$rust_result"
echo "Go full tree CPU $cpu"
go_result="$(/usr/bin/time -f 'Go peak RSS %M KiB, elapsed %e s' \
    taskset -c "$cpu" env GOMAXPROCS=1 "$GO_VERIFY" full-tree \
    -boundary-mint "$run_dir/boundary-mint.bin" -tx "$run_dir/old-tree-spend.bin")"
check_full_time "$go_result"

echo "=== [5/7] Independent Go ML-KEM-1024/HMAC-SHA-384 offline scan ==="
go-verifier/delivery-helper -tx payments/tx_hop1.bin -wallet payments/bob_wallet.json

echo "=== [6/7] Exact-rational bounds and signed-mint wire ceiling ==="
"$PY_BIN" check_math.py >/dev/null
echo "check_math.py PASS"

echo "=== [7/7] Z3 and three-way Rust/Go/Python circuit equivalence ==="
"$PY_BIN" verify_circuit_equivalence.py >/dev/null
echo "verify_circuit_equivalence.py PASS"
sha256sum --check --quiet SHA256SUMS
echo "=== ALL 7/7 INDEPENDENT CHECKS PASSED ==="
