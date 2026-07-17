# Ambient P1 Remediation Status

Date: 2026-05-22

Status: active-code remediation substantially complete; raw P1 inventory not closed.

## Cleared In The Current Working Tree

| Area | Status | Evidence |
| --- | --- | --- |
| Bridge replay cache | Fixed | Bounded replay cache with historical transfer replay check retained. |
| Storage atomic writes | Fixed | Create-new temp files with collision retry replace predictable PID temp paths. |
| History archive import | Fixed | Import now consumes the verified bundle data instead of re-reading the path after verification. |
| Host parsing and JSON output | Fixed | Structural IPv4/IPv6 parsing and serde-backed status JSON. |
| Private-material file boundaries | Fixed for controlled local formats | Dev keys, wallet backups, validator keys, Orchard wallet/view keys enforce private read/write permissions; validator key payloads validate on read/write. |
| Python wallet temp/output permissions | Fixed | SDK-created temp work dirs use `0700`; wallet-adjacent JSON and secret temp files use `0600`. |
| Canonical debug/protocol hashes | Fixed for ordering/proofs/debug privacy | HotStuff IDs, proof statement hashes, and debug shielded IDs use explicit length-delimited encodings with golden vectors. |
| Debug shielded note positions | Fixed | `ShieldedState.next_note_position` preserves monotonic note positions across compaction/removal. |
| Orchard scan accounting | Fixed | New scan report APIs distinguish decrypted, non-matching, malformed, and total outputs. |
| Plaintext transport/RPC exposure guard | Partially fixed | Transport and `rpc-serve` listeners now require `POSTFIAT_ALLOW_PUBLIC_TRANSPORT_BIND=1` for public, wildcard, or DNS bind hosts; localhost/private binds remain allowed. |
| Canonical genesis/state root | Fixed | `genesis_hash` and `replicated_state_root` now use explicit length-delimited encodings with golden-vector tests. |
| Node atomic writes | Fixed | Node writes now route through the shared storage atomic writer; governance replay publication verifies through the same checked atomic path. |
| RPC child request spool | Fixed | `rpc-serve` child request JSON now uses a private non-predictable spool directory with cleanup coverage. |
| Debug proof release gate tests | Fixed | Proof tests now cover release-mode fail-closed behavior unless the controlled debug env override is explicit. |
| Cobalt example report-path hardening | Fixed | All Cobalt examples now use the shared `emit_example_report` helper with traversal rejection and `REPORT_ROOT` support. |

## Still Open

| Bucket | Status | Reason |
| --- | --- | --- |
| Encrypted-at-rest wallet/key formats | Follow-up | Current formats are explicitly private local controlled-testnet files, still plaintext on disk. |
| Authenticated public transport | Follow-up | Bind guards reduce accidental exposure, but mTLS/Noise or equivalent is still required before untrusted/public transport claims. |
| Operator scripts | Open | The script/workflow backlog is tracked in `docs/status/operator-script-hardening-burndown-2026-05-22.md`. |
| Generated artifacts | Open | Publication hygiene is tracked in `docs/status/generated-evidence-hygiene-burndown-2026-05-22.md`. |
| Raw-ID ledger follow-through | Created, not all fixed | `docs/status/ambient-finding-disposition-ledger.json` is now a shard manifest covering all 677 raw P1 rows; many are explicitly backlog/hygiene rows, not fixed rows. |
| Orchard authorizing hash public vector/spec | Partially covered | Existing tests cover determinism/domain separation; a published golden-vector spec is still needed before cross-implementation claims. |

## Verification

- `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`
- `cargo check -p postfiat-node -p postfiat-rpc-sdk -p postfiat-ordering-fast -p postfiat-proofs -p postfiat-privacy -p postfiat-privacy-orchard`
- `cargo clippy -p postfiat-node -p postfiat-rpc-sdk -p postfiat-ordering-fast -p postfiat-proofs -p postfiat-privacy -p postfiat-privacy-orchard --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace --all-targets`
- Focused tests for privacy, Orchard scan accounting, ordering/proof/privacy golden vectors, private-material permissions, transport bind classification, and wallet helper permissions.
- New focused tests/checks from the follow-up pass:
  - `cargo test -p postfiat-execution genesis_hash_has_stable_canonical_test_vector`
  - `cargo test -p postfiat-node replicated_state_root_commits_to_chain_domain`
  - `cargo test -p postfiat-storage atomic_write`
  - `cargo test -p postfiat-node rpc_serve_rejects_public_bind_without_explicit_override`
  - `cargo test -p postfiat-node rpc_serve_child_request_spool_uses_private_directory`
  - `cargo test -p postfiat-proofs debug_proof_gate`
  - `cargo test -p postfiat-consensus-cobalt example_report_path`
  - `cargo check -p postfiat-consensus-cobalt --examples`

## Safe Claim

The active-code P1 fixes needed for controlled-testnet hardening are substantially cleared in this working tree, with transport public exposure reduced to explicit opt-in.

## Unsafe Claim

Do not claim that all P1s are closed. The raw Ambient P1 inventory now has a
ledger, but script hardening, generated-artifact hygiene, encrypted key-file
format work, authenticated public transport, and Orchard public-vector/spec
work remain open unless their ledger rows are later changed to `fixed`.
