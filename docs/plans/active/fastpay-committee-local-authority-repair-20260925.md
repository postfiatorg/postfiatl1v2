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
- [x] Build minimal repair from the archived deployed source (`03e1138e`,
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
- [x] Test activated storage, ordinary replay rejection, and tampering with updated
  unsigned manifests; verify the signed backup before any restart.

Validation: 20 node FastPay tests, 3 execution FastPay tests, 67 Python wallet
tests plus 8 subtests, and 4 wallet-proxy test scripts passed. The first negative-key
fixture was corrected to inject invalid bytes directly because the normal key
writer correctly rejects mismatched key pairs. No production relaxation was needed.

Checkpoint validation: 16 snapshot/deployment and 14 storage-activation tests
passed. The activated-storage regression fails on the unpatched code at historical
replay, then passes with the restore repair. Six tampered snapshots with updated
unsigned file hashes are rejected. Full-history import still rejects the anomaly.

Release `0989bd846b9c486794eda070a5efbd7df0f088cf`, binary SHA-256
`ba21f996e2cbaa0e6140b3131839e527715b46c4a92bd2673b37aea95036a003`.
Signed backup verified at height 1031 with unchanged tip and state root; exact
committee, genesis and registry parity held across unsigned export, signed export
and fresh verified import. Basis: consensus-v2 finalized checkpoint, not full replay.

Live payment found a client compatibility regression: v3 SDK results nested
`created_objects` under `apply`, while the existing CLI consumes it at the top
level. Preserve both forms after authenticated-quorum verification in
`python/postfiat_rpc/wallet.py`; add result-contract assertions to the existing
transfer/unwrap tests. Reconcile the already-certified operation without re-sending.

Active-storage integration prerequisite:
[locked specification](../../specs/fastpay-transactional-read-view-repair-20260925.md),
SHA-256 `1e4a40a05a3cc3f103ba8f7b548c1a03cfa141c8a9c11b6dd961f5b335b4d7f1`.
First full gate `round-20260925T025433Z`: five each, GPT 92.20,
Fable 80.20, GLM 88.40; mean **86.93**, locked immediately.

- [x] Reproduce five acknowledgements with absent active-database output at height
  1033. Preserve the first certificate, locks and failed CLI evidence.
- [x] Restore eligible validators 0–4 to the original signed binary, preserving
  state and all-six convergence. Validator-5 retains r2 and cannot sign epoch 1.
- [x] Reconstruct pending effects for execution/query views; keep canonical
  checkpoint/history/database state separate. Audit proposal omission and restart.
- [x] Preserve verified output fields in the SDK and test the real v3 response.
- [ ] Qualify activated-storage fixtures and retained live certificate; sign a
  replacement, verify its backup and roll all six before resuming the same payment.

Implementation detail discovered after the gate: `fence.decided_at_height` is
normalized to the order's validity start, not the local apply height. Use optional
local journal tip metadata to bind new pending effects to their actual finalized
base and retain application order; do not change the signed fence or DB schema.
The existing single legacy live effect has validity start equal to current height.
Older omitted effects must remain recoverable without automatic resurrection.

Activated-storage qualification: 22 main-branch and 21 release-lineage FastPay
node tests passed. The negative control replacing the pending view with the old
canonical-only read fails at the missing output; restoring the implementation
passes. Tests cover two dependent transfers in reverse lexical lock order, missing
and forged evidence, future/wrong tip metadata, single legacy-journal recovery,
signed acknowledgement idempotence, fresh-process/snapshot restore, six-validator
certified anchoring, minority omission and unwrap at a later application height.
Canonical ledger and checkpoint roots remain unchanged before anchoring.
