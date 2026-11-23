# PFSH02 cryptographic and implementation review

**Historical evidence.** The original milestone measurements and artifact inventory tested L1 profile 1, including the public PFT payer for issued spends. Issued payments and exits now use profile 2.


The review target is the Rust verifier used in candidate block execution, its independent Go implementation, the PFT and governed pfUSDC settlement rules, and the owner-local wallet. The integration is restricted to the disposable rehearsal genesis. No production activation height has been selected. No independent review has been obtained.

The protocol and implementation at the start of the rehearsal are pinned to commit `f0a44680e6435fb87a7392f75677a86d3ba5d5fe` of `postfiatorg/postfiatl1v2`. The rehearsal record pins the test revision and executables. Use the earlier commit for the following specification sections and source counts. [Specification](../../security/peel-binius-shielded-pq-payments.md) · [Integration plan at the reviewed revision](https://github.com/postfiatorg/postfiatl1v2/blob/f0a44680e6435fb87a7392f75677a86d3ba5d5fe/docs/specs/pq-private-payments-integration.md) · [Rehearsal record](rehearsal-record.md)

## Claims to examine

| Claim | Argument in the pinned specification | Review boundary |
| --- | --- | --- |
| Two real or dummy inputs and two outputs conserve integer value and authenticate a depth-24 path for each real input. | §§1.2–1.4 and 2.1–2.3. Rules R0–R6 define acceptance. | Check full witness coverage, exact 66-bit addition, dummy gating, canonical fields, transcript ordering and dual-anchor migration. |
| Round-by-round knowledge soundness and statistical honest-verifier zero knowledge for the underlying 34-round IOP. | §3.3, Theorem 1A, Appendices A.1, A.2 and B.1. | Validate Peel(M), its small-domain cases, RS proximity hypotheses, masks/ranks, PoW round and batching losses. |
| Quantum zero knowledge after compilation. | §3.4, Theorem 1B, Appendices B.1–B.2. | Check capped PoW, multi-proof simulation, puncturing/reprogramming and exact oracle-query charges. |
| A straight-line extractor from a single fixed-range quantum random oracle. | §3.5, Theorem 2A, **Appendix A.3 is an unreviewed reduction**. | Verify the database transition bound in both directions, retroactive tree stability, readout loss and the reduction to an accepting witness. Published asymptotic BCS results do not supply these concrete constants. |
| Adaptive extraction after access to simulated proofs. | §3.5, Theorem 2B, **Appendix A.4 is a conjecture**. | Analyze the classical programming overlay under coherent queries, transcripts chosen adaptively, fresh statement-proof pairs and the reduction to the unmodified oracle. |
| Joint public/private conservation plus spend, statement and depositor authorization. | §§4.1–4.2, Theorems 3–4. §4.4 Table 3 itemizes assumption and query losses. | Reconcile serial uniqueness, account sequence, domain separation, pinned account key, issue identity, fee source, archive replay and exit policy with the games. |
| Mint and transfer privacy under active scanning and corruption. | §4.3, Theorem 5, and §4.4. | Assess IK-CCA recipient anonymity for ML-KEM-1024, KDF and AEAD hybrids, SHAKE joint-PRF, commitment hiding, multi-user loss and corruption restrictions. |
| Concrete sizes, checks and timing. | §§0 and 5, Appendix C. | Deterministic size bounds, differential tests and arithmetic checks support implementation claims. Hardware timings apply to the reported CPU and workload. |

At reference query budget (Q=2^{64}), Table 2 gives conditional extraction error about (2^{-170.98}) for one proof and simulation-extraction about (2^{-170.97}) under its stated proof budgets. At (Q=2^{96}) both reach about (2^{-92.25}). Table 3 includes separate unquantified assumptions. Its targeted 160-bit account-address second-preimage reference at (Q=2^{64}) is only about (2^{-32}). Evaluate each application claim with its own game, query model and public-key pinning assumptions.

Under the specification's reference budget of `Q = 2^64` quantum queries, `P = 2^32` simulated proofs and `L = 2^16` fresh accepted pairs, the quoted compiler advantage targets are `2^-170.98` for extraction, `2^-170.97` for simulation extractability and `2^-150.60` for zero knowledge. At `Q = 2^80` the extraction and simulation-extractability targets are `2^-137.55`. At `Q = 2^96` they are `2^-92.25`. These are conditional compiler terms. Use §4.4 Table 3 to add the distinct ledger, authorization and privacy assumptions for each application. The compiler models one 1,536-bit-output random oracle, instantiated by three SHA-512 calls, with a 512-bit internal transcript state. Verify that idealization, the projections, the 34-round schedule and the 20-bit final proof-of-work charge against §§1.1 and 3 and Appendices A–B. [Specification at the reviewed source revision](https://github.com/postfiatorg/postfiatl1v2/blob/f0a44680e6435fb87a7392f75677a86d3ba5d5fe/docs/security/peel-binius-shielded-pq-payments.md)

## Trace one payment

1. Start at `PqPayment::parts` and `Terms::{statement_context,signing_message}`. Decode the outer header and canonical inner body, then recompute the hash over the network, genesis, protocol, pool, asset, mode, epoch, public fee and account terms. Follow the inner header's commitments, ciphertexts, mint, fee and exit destination through the transcript.
2. Start with standalone **R0-Auth** in §1.3. The positive-mint suffix uses ML-DSA-65 with context `postfiat-l1-v2/tx/v1` over the domain, the SHA3-derived source address and the raw transaction context. For settlement, follow `verify_l1` and compare the Go L1 profile byte for byte. The L1 inner decoder accepts the settlement body. On deposits, the outer account signature binds the depositor key, sequence and terms to the chain-specific statement. Validate this profile transition separately from the standalone signature game. Issued payments and exits use profile 2 with an exact inner note fee at inclusion and no public payer. The proposed starting amount is 1 atom (0.000001 pfUSDC), equal in atoms to the native PFT minimum. Native internal payments retain their minimum fee. Inspect the distinct signature tags and fixed signature context.
3. Follow `sign_l1_shield` backwards to the local output-opening checks and forwards through the validator debit, spent serials, authenticated pair append and state root. The validator authenticates ciphertext bytes. It cannot verify that a recipient can decrypt an intended amount.
4. Trace key derivation and local scanning. An independently held 32-byte file wraps the encrypted wallet state with AES-256-GCM and record-specific associated data. The account signer, wallet seed, KEM secret, decrypted openings and local request socket need separate scrutiny. The wrapping key and backup must be available for recovery. Public fixture seeds and deterministic vector builders are test inputs.

## Source inventory

Counts are physical newline counts from the pinned Git blobs, including comments and tests. Paths in each table are relative to the checkout. The Go field assembly and portable fallback are both in scope.

### Rust verifier and prover

| File | Lines |
| --- | ---: |
| `crates/privacy_binius/Cargo.toml` | 20 |
| `crates/privacy_binius/examples/generate_l1_vectors.rs` | 98 |
| `crates/privacy_binius/examples/generate_rollover_vectors.rs` | 149 |
| `crates/privacy_binius/src/lib.rs` | 146 |
| `crates/privacy_binius/tests/l1_vectors.rs` | 233 |
| `tools/pft-binius-shielded-pq/rust-prover/Cargo.lock` | 525 |
| `tools/pft-binius-shielded-pq/rust-prover/Cargo.toml` | 35 |
| `tools/pft-binius-shielded-pq/rust-prover/src/binius_pcs.rs` | 2156 |
| `tools/pft-binius-shielded-pq/rust-prover/src/circuit.rs` | 1461 |
| `tools/pft-binius-shielded-pq/rust-prover/src/field.rs` | 1152 |
| `tools/pft-binius-shielded-pq/rust-prover/src/full_tree.rs` | 102 |
| `tools/pft-binius-shielded-pq/rust-prover/src/lib.rs` | 9 |
| `tools/pft-binius-shielded-pq/rust-prover/src/main.rs` | 852 |
| `tools/pft-binius-shielded-pq/rust-prover/src/mint_auth.rs` | 90 |
| `tools/pft-binius-shielded-pq/rust-prover/src/payment.rs` | 630 |
| `tools/pft-binius-shielded-pq/rust-prover/src/shake256.rs` | 352 |
| `tools/pft-binius-shielded-pq/rust-prover/src/wire.rs` | 216 |

### Independent Go verifier and scanner

| File | Lines |
| --- | ---: |
| `tools/pft-binius-shielded-pq/go-verifier/binius_pcs.go` | 1062 |
| `tools/pft-binius-shielded-pq/go-verifier/circuit.go` | 1020 |
| `tools/pft-binius-shielded-pq/go-verifier/cmd/delivery/main.go` | 138 |
| `tools/pft-binius-shielded-pq/go-verifier/cmd/delivery/wallet_test.go` | 79 |
| `tools/pft-binius-shielded-pq/go-verifier/field.go` | 509 |
| `tools/pft-binius-shielded-pq/go-verifier/field_pclmul_amd64.go` | 173 |
| `tools/pft-binius-shielded-pq/go-verifier/field_pclmul_amd64.s` | 81 |
| `tools/pft-binius-shielded-pq/go-verifier/field_pclmul_other.go` | 13 |
| `tools/pft-binius-shielded-pq/go-verifier/full_tree.go` | 59 |
| `tools/pft-binius-shielded-pq/go-verifier/go.mod` | 7 |
| `tools/pft-binius-shielded-pq/go-verifier/go.sum` | 4 |
| `tools/pft-binius-shielded-pq/go-verifier/l1_profile.go` | 88 |
| `tools/pft-binius-shielded-pq/go-verifier/l1_profile_test.go` | 52 |
| `tools/pft-binius-shielded-pq/go-verifier/l1_rollover_test.go` | 77 |
| `tools/pft-binius-shielded-pq/go-verifier/main.go` | 511 |
| `tools/pft-binius-shielded-pq/go-verifier/main_test.go` | 195 |
| `tools/pft-binius-shielded-pq/go-verifier/mint_auth.go` | 31 |
| `tools/pft-binius-shielded-pq/go-verifier/mint_auth_test.go` | 40 |

### Compiler, circuit and arithmetic checks

| File | Lines |
| --- | ---: |
| `tools/pft-binius-shielded-pq/check_math.py` | 545 |
| `tools/pft-binius-shielded-pq/verify_circuit_equivalence.py` | 403 |
| `tools/pft-binius-shielded-pq/verify.sh` | 79 |
| `tools/pft-binius-shielded-pq/SHA256SUMS` | 36 |
| `tools/pft-binius-shielded-pq/Dockerfile` | 22 |

### L1 domain, account, consensus and storage

| File | Lines |
| --- | ---: |
| `crates/types/src/pq_payment.rs` | 388 |
| `crates/types/src/pq_tree.rs` | 328 |
| `crates/types/src/core_chain.rs` | 978 |
| `crates/types/src/shielded_bridge_governance.rs` | 3131 |
| `crates/crypto_provider/src/lib.rs` | 602 |
| `crates/execution/src/entrypoints.rs` | 2404 |
| `crates/execution/src/issued_asset_ledger_helpers.rs` | 1065 |
| `crates/node/src/pq_payment_actions.rs` | 268 |
| `crates/node/src/pq_pool_state.rs` | 46 |
| `crates/node/src/consensus_artifacts.rs` | 5178 |
| `crates/node/src/execution_actions.rs` | 5407 |
| `crates/node/src/state_commitment.rs` | 5077 |
| `crates/node/src/block_replay_wallet.rs` | 3837 |
| `crates/node/src/batch_snapshot.rs` | 4080 |
| `crates/storage/src/pq_pool.rs` | 144 |

### Wallet, local signer and gateway boundary

| File | Lines |
| --- | ---: |
| `crates/privacy_binius_wallet/Cargo.toml` | 26 |
| `crates/privacy_binius_wallet/src/keys.rs` | 80 |
| `crates/privacy_binius_wallet/src/lib.rs` | 7 |
| `crates/privacy_binius_wallet/src/service.rs` | 101 |
| `crates/privacy_binius_wallet/src/storage.rs` | 110 |
| `crates/privacy_binius_wallet/src/tree.rs` | 171 |
| `crates/privacy_binius_wallet/src/wallet.rs` | 537 |
| `crates/privacy_binius_wallet/tests/delivery.rs` | 65 |
| `crates/privacy_binius_wallet/tests/keys.rs` | 30 |
| `crates/privacy_binius_wallet/vectors/make.py` | 34 |
| `python/postfiat_rpc/pq_browser.py` | 388 |
| `python/postfiat_rpc/pq_wallet.py` | 258 |
| `crates/types/src/pq_address.rs` | 45 |
| `crates/node/src/bin/pftl_pq_walletd.rs` | 147 |
| `wallet-web/src/components/PqPayments.jsx` | 186 |

## Reproduction

The source is available from the [repository](https://github.com/postfiatorg/postfiatl1v2). Use the revision table in the rehearsal record to select the verifier source and M8 tests. All verification vectors and the fresh-chain setup are in Git. The separate R64 check takes a retained validator-3 archive as input. Its row digest and the permitted public outputs are in the rehearsal record.

Use Linux x86-64 with Rust 1.95.0, Go 1.25.1, Python 3.12 with z3-solver 5.4.1.0 or the pinned container environment in the standalone tool, Node 22 and a Chromium installation available to Playwright. The host replay used Python's Z3 5.4.1. The wallet tests use disposable keys and loopback connections. Run from the pinned checkout with an absolute writable Cargo target. Set `GO_BIN` and `PY_BIN` to the local Go executable and Z3-enabled Python. Keep the package and module locks.

```bash
export CARGO_TARGET_DIR="$PWD/target"
export RAYON_NUM_THREADS=4
(cd tools/pft-binius-shielded-pq && BENCH_CPU=$("$PY_BIN" idle_cpu.py) ./verify.sh)
(cd tools/pft-binius-shielded-pq/go-verifier && "$GO_BIN" test -count=1 ./... && "$GO_BIN" test -count=1 -tags purego ./...)
cargo test --release --locked -p postfiat-privacy-binius
cargo test --release --locked -p postfiat-privacy-binius --features portable
cargo test --release --locked -p postfiat-privacy-binius-wallet
cargo test --release --locked -p postfiat-node --lib -- pq_payment_activation pq_payment_accounting pq_pool_state pq_snapshot_rebuild pq_reproves_reserve private_action_
cargo test --release --locked -p postfiat-node --bin postfiat-node -- pq_payment_local_chain_tests --test-threads=1
cargo test --release --locked -p postfiat-node --test private_actions_gateway
cargo test --release --locked -p postfiat-storage --lib -- pq_archive_
PYTHONPATH=python python3 -m unittest discover -s python/tests -p test_pq_wallet_cli.py
(cd wallet-web && npm ci)
node wallet-web/scripts/test-pq-payments.mjs
```

To reproduce the settlement timing, compile each node test target first. Run these next commands separately, with no other worker tests on the measured core. The release record retains both shared-host portable trials.

```bash
cargo test --release --locked -p postfiat-node --lib --no-run
core=$(cd tools/pft-binius-shielded-pq && "$PY_BIN" idle_cpu.py)
PQ_VALIDATOR_BENCH=1 RAYON_NUM_THREADS=1 taskset -c "$core" cargo test --release --locked -p postfiat-node --lib -- tests::pq_payment_accounting:: --nocapture --test-threads=1
cargo test --release --locked -p postfiat-node --lib --features postfiat-privacy-binius/portable --no-run
core=$(cd tools/pft-binius-shielded-pq && "$PY_BIN" idle_cpu.py)
PQ_VALIDATOR_BENCH=1 RAYON_NUM_THREADS=1 taskset -c "$core" cargo test --release --locked -p postfiat-node --lib --features postfiat-privacy-binius/portable -- tests::pq_payment_accounting:: --nocapture --test-threads=1
cargo test --workspace --release --locked --no-fail-fast
```

The seven-stage driver checks fixture digests, Rust proofs and settlement, rejection controls, independent Go settlement and rollover, recipient scanning, arithmetic bounds, and circuit equivalence. Preserve the first timing result and the core selection on any rerun. The rehearsal record gives the two-binary build, exact archive replay and measured resource results.

## Known limits and questions for reviewers

The L1 settlement profile permits one development genesis, native PFT and one governed source series. A small pool, public deposits/exits, gateway caller identity, validator peer addresses, receipt reads and traffic timing can identify users. A migration through a public Orchard exit is linkable. The standalone replay, validator regression tests and mathematical scripts are evidence within their respective models. Independent cryptographic and implementation review, broader hardware qualification and founder release authorization remain open.

1. Can a reviewer derive the A.3 transition bound with the exact constants, including exit/uncompute, for the implemented single-oracle transcript and PoW queries?
2. Does A.4's wrapper simulate a coherent oracle across adaptive programming, repeated statements and fresh proofs? Which loss survives without the conjecture?
3. Do the A.1 Peel/RS domain and degree conditions and A.2 round-by-round losses apply at every realized transcript? Does Appendix B preserve joint simulation under the declared proof budget?
4. Do the constrained Keccak rows, masking rows, field operations, canonical encodings and challenge schedule match in the Rust prover, Rust verifier and Go verifier across every supported arithmetic backend?
5. Does the account signature bind every economic choice and intended output across both asset modes, key registration/rotation, sequence changes, sponsors and network replay? Where does security reduce to a 160-bit address?
6. Can the composed privacy game justify the assumed joint PRF, key-in-middle envelope derivation, recipient anonymity under active scanning, AEAD integrity and corruption model at the stated scale?
7. Are state changes, serial ownership, per-issue liabilities, root retention, certificate read-back and pause semantics equivalent across admission, peer voting, crash recovery, snapshot import and historical replay?
8. Which secrets persist in memory, backups, socket requests, temporary files or browser/daemon boundary failures? Does an untrusted gateway learn more than transaction bytes, reads, caller identity and timing?
9. What compatibility, resource and hardware findings must be resolved before governance can safely schedule a live height? Return findings with source revision, affected claim, assumptions, evidence and severity.
