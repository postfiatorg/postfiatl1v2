# FastPay committee authority repair

Specification: [locked repair specification](../../specs/fastpay-committee-local-authority-repair-20260925.md).
The specification bytes are locked, including their original proposed-status line.
SHA-256: `759ca8e44816b44e3737cb8cf7635b8d77d24273b4d1d6eac4e23809b8ab8a2f`.

- [x] Full TIH gate, `round-20260925T013542Z`: five scores each from
  `gpt-6-astra` (92.40), `anthropic/claude-fable-5.1` (80.80),
  `z-ai/glm-5.3` (89.60). Combined **87.60/100**; locked at first passing round.
- [x] Confirm six live hosts run the same deployed binary and registry; all at
  height 1031 and reject recovery capabilities with the same committee mismatch.
- [x] Replace global registry equality with local signer authorization and verify
  actual signatures before durable mutation in `crates/node/src/fastpay_recovery_node.rs`.
- [x] Exercise six-member key rotation, negative cases and certified convergence
  in `crates/node/src/tests/fastpay_payment_safety.rs`; run affected client tests.
- [ ] Build minimal repair from the archived deployed source (`03e1138e`,
  base `707e006f` plus all 26 archived source-file hashes); qualify isolated
  current-state snapshots and preserve signed deployment/rollback bindings.
- [ ] Roll all six hosts and verify capabilities, Python CLI payment, wallet UI
  result and certified convergence; publish exact evidence and operator guidance.

Task Node's skill/integration is unavailable in this session. No generated task,
acceptance, submitted evidence or reward is claimed. User-authorized repair work
continues, with the lifecycle gap recorded here rather than invented receipts.

Operational limit: five eligible FastPay signers remain; one further signer outage
prevents new FastPay certificates until a governed committee rotation.

Restore prerequisite: [checkpoint specification](../../specs/fastpay-checkpoint-restore-prerequisite-20260925.md).
First full TIH mean 83.53; direct OpenRouter `openai/gpt-5.6-sol-pro` rewrite,
then `round-20260925T020822Z`: five each, GPT 91.60, Fable 84.80, GLM 89.80,
combined **88.73**. Locked SHA-256
`9c2af1e7689be5a8becc8afa1ce45d107a8166080e54425608fee6e5752c9b33`.

- [x] Reproduce live snapshot import failure at block 1011; services unchanged.
- [x] Preserve checkpoint verification during transactional snapshot restore in
  `batch_snapshot.rs` and `storage_migration.rs`, with a sealed fresh-import
  capability. Standalone full-history verification retains its existing ordering.
- [ ] Test activated storage, ordinary replay rejection, and tampering with updated
  unsigned manifests; verify the signed backup before any restart.

Validation: 20 node FastPay tests, 3 execution FastPay tests, 67 Python wallet
tests plus 8 subtests, and 4 wallet-proxy test scripts passed. The first negative-key
fixture was corrected to inject invalid bytes directly because the normal key
writer correctly rejects mismatched key pairs. No production relaxation was needed.

Checkpoint validation: 16 snapshot/deployment and 14 storage-activation tests
passed. The activated-storage regression fails on the unpatched code at historical
replay, then passes with the restore repair. Six tampered snapshots with updated
unsigned file hashes are rejected. Full-history import still rejects the anomaly.
