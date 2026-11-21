#!/usr/bin/env bash
# Self-contained 7-step independent verification & replay script for Peel-Binius-Shielded.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GO_BIN="${GO_BIN:-$(command -v go || echo "$HOME/go/pkg/mod/golang.org/toolchain@v0.0.1-go1.25.1.linux-amd64/bin/go")}"
PY_BIN="${PY_BIN:-$(if [ -x /home/postfiat/repos/pft-deep-ligero-pq/.venv/bin/python ]; then echo /home/postfiat/repos/pft-deep-ligero-pq/.venv/bin/python; else command -v python3; fi)}"

echo "=== [1/7] Rust unit tests (FIPS-202 SHAKE256, Depth-24 Merkle, E384 PCLMULQDQ, Streaming ANTT, 56-perm Circuit, Binius PCS) ==="
(cd "$ROOT/rust-prover" && cargo test --release --lib)

echo "=== [2/7] Rust release binary: verify all 4 end-to-end Depth-24 lifecycle transactions (ZK-Mint, Hop-1 2-in, Dummy 1-in, Hop-2 + Unshield) ==="
(cd "$ROOT/rust-prover" && cargo build --release)
"$ROOT/rust-prover/target/release/pft-binius-shielded" verify --tx "$ROOT/payments/tx_mint.bin" --ledger "$ROOT/payments/ledger_mint.json"
"$ROOT/rust-prover/target/release/pft-binius-shielded" verify --tx "$ROOT/payments/tx_hop1.bin" --ledger "$ROOT/payments/ledger_hop1.json"
"$ROOT/rust-prover/target/release/pft-binius-shielded" verify --tx "$ROOT/payments/tx_dummy_1in.bin" --ledger "$ROOT/payments/ledger_dummy_1in.json"
"$ROOT/rust-prover/target/release/pft-binius-shielded" verify --tx "$ROOT/payments/tx_hop2.bin" --ledger "$ROOT/payments/ledger_hop2.json"

echo "=== [3/7] Rust adversarial suite (35 named controls incl. R4 zero-coin, R6 capacity, R5 mint/unshield, live dummy-inflation & forged-Merkle ZKP attacks + 150 fuzz) ==="
"$ROOT/rust-prover/target/release/pft-binius-shielded" adversarial --tx "$ROOT/payments/tx_hop1.bin" --ledger "$ROOT/payments/ledger_hop1.json"

echo "=== [4/7] Independent Go 1.25.1 stdlib verifier (ZERO third-party deps): 10k PCLMULQDQ + 3D-Karatsuba + 35 adversarial + all 4 lifecycle scenarios ==="
(cd "$ROOT/go-verifier" && "$GO_BIN" test -v ./... && "$GO_BIN" build -o binius-shielded-verifier . && "$GO_BIN" build -tags purego -o binius-shielded-verifier-purego . && "$GO_BIN" build -o delivery-helper ./cmd/delivery)
GOMAXPROCS=1 "$ROOT/go-verifier/binius-shielded-verifier" verify -tx "$ROOT/payments/tx_mint.bin" -ledger "$ROOT/payments/ledger_mint.json"
GOMAXPROCS=1 "$ROOT/go-verifier/binius-shielded-verifier" verify -tx "$ROOT/payments/tx_hop1.bin" -ledger "$ROOT/payments/ledger_hop1.json"
GOMAXPROCS=1 "$ROOT/go-verifier/binius-shielded-verifier" verify -tx "$ROOT/payments/tx_dummy_1in.bin" -ledger "$ROOT/payments/ledger_dummy_1in.json"
GOMAXPROCS=1 "$ROOT/go-verifier/binius-shielded-verifier" verify -tx "$ROOT/payments/tx_hop2.bin" -ledger "$ROOT/payments/ledger_hop2.json"

echo "=== [5/7] Independent Go 1.25.1 stdlib FIPS-203 ML-KEM-1024 offline stealth scan ==="
"$ROOT/go-verifier/delivery-helper" -tx "$ROOT/payments/tx_hop1.bin" -wallet "$ROOT/payments/bob_wallet.json"

echo "=== [6/7] Mechanized exact-rational bounds (tower irreducibility, 58x76 G1 + 17x32 SC2 Jacobians, 512b-state DFMS22 QROM, wire) ==="
"$PY_BIN" "$ROOT/check_math.py" >/dev/null && echo "check_math.py PASS"

echo "=== [7/7] Mechanized Z3 UNSAT proofs (66b mint/unshield conservation, Merkle mux, is_dummy gating, Chi-Iota) + 1,600-basis L_TRP + 56-perm DAG + 3-way Rust==Go==Python IR ==="
"$PY_BIN" "$ROOT/verify_circuit_equivalence.py" >/dev/null && echo "verify_circuit_equivalence.py PASS"

echo "=== ALL 7/7 INDEPENDENT CHECKS PASSED ==="
