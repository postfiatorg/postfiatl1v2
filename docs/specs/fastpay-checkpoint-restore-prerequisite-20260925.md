# Preserve the declared verification basis when restoring a finalized checkpoint

## 1. Objective

Implement the missing restore-path prerequisite for the FastPay repair.

An explicit finalized-checkpoint snapshot import must preserve the existing checkpoint trust model after transactional storage is activated. Checkpoint mode verifies consensus certification of the current state; it does not replay and validate every historical state transition.

The current failure occurs because:

- `crates/node/src/batch_snapshot.rs::import_snapshot_with_basis` invokes `rebuild_transactional_storage` unconditionally.
- `crates/node/src/storage_migration.rs::rebuild_transactional_storage` unconditionally calls `verify_blocks`.
- Full historical replay rejects archived block 1011 on a legacy NAV supply invariant, even though the exported height-1031 current state is certified by the existing consensus-v2 finalized-checkpoint verifier.

The implementation must separate deterministic transactional-storage reconstruction from the verification basis used to accept the reconstructed state.

## 2. Reported baseline and implementation status

The following is operational context, not evidence that the proposed patch is complete:

- The restore prerequisite described here has not yet been implemented.
- Live state remains at height 1031 on all six nodes.
- No validator has been restarted.
- No funds have moved.
- Reported existing test results are:
  - FastPay: 20 node, 3 execution, 67 Python, and 4 proxy tests passing.
  - Release branch: 19 node tests passing.
- Task Node process availability remains unresolved administrative tracking. It is not safety evidence, implementation acceptance, or a basis for claiming a reward.

The executor owns implementation, testing, evidence collection, deployment, and rollback operations. This specification makes no timing guarantee.

## 3. Verification models that must remain distinct

### 3.1 Full-history verification

Full-history verification reconstructs storage and calls the existing block verifier to replay historical execution.

It remains mandatory for:

- Ordinary full-history snapshot imports.
- The public standalone transactional-storage rebuild or migration command.
- Any existing import mode other than explicit finalized-checkpoint import.

A full-history replay failure must continue to be reported as a failure. This change must not weaken, bypass, or special-case the NAV supply invariant.

### 3.2 Finalized-checkpoint verification

Finalized-checkpoint mode verifies consensus certification of the current state rather than replaying every historical transition.

Its existing trust basis includes, as applicable to the current verifier:

- Exact current-state commitment.
- Chain and genesis binding.
- Retained tip identity.
- Authenticated consensus-v2 finality certificate or commit.
- Active registry and committee data.
- Receipt references and canonical receipt mapping.
- Ordered-history consistency.
- Expected root and expected tip.
- Snapshot file hashes and object/count checks.
- Logical equivalence and complete final-state checks.
- Database integrity and generation checks.

This trust mode already exists for checkpoint-only import before transactional-storage activation. Activating transactional storage must not silently replace it with full-history replay.

A successful finalized-checkpoint import proves that the reconstructed current state satisfies the selected finalized-checkpoint basis. It does not prove that every historical transaction was re-executed successfully.

The verifier version recorded for this path is a storage-format verifier identifier. It must not be described as proof of full execution replay. Acceptance evidence must explicitly identify the basis as finalized checkpoint.

## 4. Scope and safety constraints

The implementation must:

- Preserve archived deployed source data and its existing state encoding.
- Preserve snapshot file hashes, counts, logical-equivalence checks, root and tip checks, database integrity checks, and generation checks.
- Keep finalized-checkpoint verification mandatory for checkpoint imports.
- Run finalized-checkpoint verification only after the reconstructed generation required by the verifier is available.
- Complete finalized-checkpoint verification before the import is accepted or published.
- Restrict the new restore entrypoint to crate-internal use.
- Invoke it only for explicit finalized-checkpoint imports into fresh, disposable import directories.
- Preserve the current mainline behavior that publishes private staging only after complete success.
- Treat any failed destination left by an older deployed lineage as disposable and unaccepted.

The implementation must not:

- Weaken or remove the NAV supply invariant.
- Alter archived blocks, balances, keys, receipts, certificates, registry data, or state commitments.
- Substitute a newer execution protocol for archived execution.
- Add a general CLI flag that skips replay.
- Allow callers to select checkpoint reconstruction for arbitrary migrations.
- Call the new entrypoint on a running validator’s data directory.
- Treat generation construction as verification or acceptance.
- Treat manifest hashes as a substitute for checkpoint certification.
- Change the behavior of public standalone rebuilds or ordinary full-history imports.
- Publish, promote, or use a failed staging or destination directory as live data.

## 5. Code pointers and interface boundary

### 5.1 Existing integration points

- Snapshot import:
  - `crates/node/src/batch_snapshot.rs::import_snapshot_with_basis`
  - Current issue: this function invokes `rebuild_transactional_storage` without preserving the selected snapshot verification basis.

- Transactional-storage migration:
  - `crates/node/src/storage_migration.rs::rebuild_transactional_storage`
  - Current contract: public standalone rebuild with full historical replay.
  - Current behavior: unconditionally calls `verify_blocks`.
  - Required outcome: this public behavior remains unchanged.

### 5.2 Required internal interface

Add a crate-internal entrypoint with the following interface shape:

- `restore_transactional_checkpoint(options: StorageMigrationOptions)`
- Visibility: crate-internal only, such as `pub(crate)`.
- Caller: only the explicit finalized-checkpoint branch of `import_snapshot_with_basis`.
- Input location: a fresh, isolated, disposable import directory populated from manifest-verified snapshot files.
- Verification basis: an internal `FinalizedCheckpoint` basis that cannot be selected through the public standalone migration interface.
- Acceptance rule: return success only after reconstruction, integrity checks, and `verify_finalized_checkpoint` all succeed.

Do not expose the internal basis through a public CLI option or a generic caller-controlled “skip replay” Boolean.

### 5.3 Shared reconstruction boundary

Refactor only as needed so that both entrypoints can use the same deterministic reconstruction logic:

- `rebuild_transactional_storage(options)`:
  1. Reconstruct transactional storage.
  2. Run all existing integrity, count, root, tip, generation, receipt, and logical-equivalence checks.
  3. Run `verify_blocks`.
  4. Return success only if full historical replay succeeds.

- `restore_transactional_checkpoint(options)`:
  1. Reconstruct transactional storage through the same shared reconstruction.
  2. Run the same applicable integrity, count, root, tip, generation, receipt, and logical-equivalence checks.
  3. Do not call `verify_blocks` as the acceptance basis.
  4. After the reconstructed generation and ordered-history index are available, run `verify_finalized_checkpoint`.
  5. Return success only if finalized-checkpoint verification succeeds.

The internal implementation may represent this distinction with a private enum or equivalent private control flow, but the checkpoint basis must not become a public migration option.

## 6. Numbered implementation sequence

1. **Preserve the public full-replay contract.**
   - Leave `rebuild_transactional_storage` as the public standalone full-history path.
   - Ensure it still calls `verify_blocks`.
   - Ensure ordinary full-history imports still use this path.
   - Do not change its success criteria or reinterpret replay failures.

2. **Extract shared deterministic reconstruction.**
   - Separate generation construction and common integrity validation from final acceptance verification.
   - Retain all existing checks for snapshot files, counts, receipt mappings, complete state, logical equivalence, expected root, expected tip, database integrity, and generation consistency.
   - Do not name or report this construction step as “verified.”

3. **Add the crate-internal checkpoint restore entrypoint.**
   - Add `restore_transactional_checkpoint(options: StorageMigrationOptions)`.
   - Bind it internally to a `FinalizedCheckpoint` verification basis.
   - Prevent use from public CLI or standalone migration code.
   - Reject or structurally prevent use against live validator data.

4. **Route only explicit checkpoint imports to the new entrypoint.**
   - Update `crates/node/src/batch_snapshot.rs::import_snapshot_with_basis`.
   - When and only when the selected import basis is explicit finalized checkpoint, call `restore_transactional_checkpoint`.
   - Continue routing ordinary and full-history imports to `rebuild_transactional_storage`.

5. **Verify after reconstruction and before acceptance.**
   - Make the reconstructed generation and ordered-history index available.
   - Run `verify_finalized_checkpoint` against that reconstructed generation.
   - Keep all selected checkpoint checks mandatory, including certificate, chain/genesis, registry, finality, receipt references, expected root, and expected tip.
   - Return failure before publication if any check fails.

6. **Preserve atomic publication semantics.**
   - On mainline, keep the reconstructed database private in staging until every required check succeeds.
   - On any failure, do not publish or promote staging.
   - For the older deployed lineage, recognize that a failed import may leave a destination directory. Mark and handle it as disposable offline output; never treat its existence as acceptance.

7. **Record the actual verification basis.**
   - Acceptance evidence must state `FinalizedCheckpoint` or an equivalent explicit basis.
   - Record the storage-format verifier identifier separately.
   - Do not label checkpoint success as full-history replay success.
   - Record the known block-1011 historical replay failure separately from checkpoint acceptance.

8. **Keep the change bounded.**
   - Do not modify archived data or consensus commitments to make replay pass.
   - Do not change payment behavior, signature thresholds, keys, or the previously locked FastPay repair scope.
   - Do not add unrelated remediation for the historical anomaly as part of this prerequisite.

## 7. Required automated qualification

### 7.1 Finalized-checkpoint activation regression

Extend:

- `crates/node/src/tests/snapshot_deployment.rs`
- Test: `finalized_checkpoint_snapshot_accepts_certified_legacy_governance_anomaly_and_rejects_tampering`

Set:

- `storage_activation_height: Some(1)`

The test must demonstrate all of the following:

1. A certified snapshot containing the deliberately unreplayable legacy governance history restores successfully through explicit finalized-checkpoint mode after transactional-storage activation.
2. The same history remains rejected by ordinary full-history import.
3. The checkpoint path invokes finalized-checkpoint verification after the reconstructed generation is available.
4. No failed checkpoint import is reported as accepted or published.

### 7.2 Tampering and negative coverage

Preserve or add coverage proving rejection for:

- Missing certificate.
- Forged or modified finality.
- Tampered registry or committee data.
- Incorrect expected state root.
- Incorrect expected tip.
- Bad receipt references or receipt counts.
- Ordered-history inconsistency.
- Database-integrity failure.
- Generation inconsistency.
- Tampered current state.

For tampered state, finality, and registry cases:

1. Modify the snapshot content.
2. Update the snapshot’s unsigned file hashes or manifest so basic file-hash validation passes.
3. Confirm the import is still rejected by the mandatory finalized-checkpoint verification.

This establishes that manifest validation alone cannot substitute for consensus certification.

### 7.3 Standalone migration regression

Run the existing tests in:

- `crates/node/src/tests/replicated_state_activation.rs`

They must continue to prove that the original standalone migration performs full historical replay. No checkpoint-only exception may leak into that interface.

### 7.4 Test execution records

Use the repository’s established test runner for the affected targets. Record:

- Exact commands used.
- Source revision and candidate binary identity.
- Pass/fail result for each required test.
- Verification basis exercised by each test.
- Any generated failure artifacts and their cleanup disposition.

Do not claim tests that were not run.

## 8. Snapshot and signed-backup qualification

Before any validator restart:

1. Use the candidate binary to import the exact height-1031 snapshot through explicit finalized-checkpoint mode.
2. Confirm that all manifest and structural checks pass.
3. Confirm that transactional storage is reconstructed.
4. Confirm that `verify_finalized_checkpoint` runs against the reconstructed generation.
5. Confirm unchanged:
   - Height.
   - Retained tip identity.
   - State root.
   - Committee and active registry.
   - Required receipt and ordered-history relationships.
6. Complete the signed-backup round trip with the candidate binary.
7. Restore the signed backup into a fresh disposable directory.
8. Re-run the same root, tip, height, committee, registry, receipt, integrity, and checkpoint-certificate checks.
9. Record the verification basis as finalized checkpoint.
10. Record the historical block-1011 replay failure separately and do not describe the round trip as successful full-history replay.
11. Recheck all six live nodes before deployment and confirm no unexpected height, root, tip, committee, process, or data-directory changes.

A successful generation build without successful finalized-checkpoint verification is not a qualified backup.

## 9. Deployment sequence

Deployment remains governed by the existing safe-rollout tooling and the previously locked FastPay specification.

1. Do not restart any validator until the candidate’s signed-backup round trip is verified.
2. Retain the old signed release and existing rollback units.
3. Confirm the six-node live baseline immediately before rollout.
4. Restart validators individually using the existing safe-rollout tool.
5. After each restart, apply the existing health, certification, and convergence gates before proceeding.
6. Preserve:
   - Existing validator keys.
   - Five-signature quorum requirements.
   - Payment tests required by the locked FastPay specification.
   - All-six certified convergence.
7. Do not move funds as part of this prerequisite.
8. Do not continue rollout after any stop condition below.

## 10. Stop and rollback conditions

### 10.1 Stop before deployment if

- Any required automated test fails.
- Ordinary full-history import no longer rejects the legacy unreplayable history.
- The standalone migration no longer calls full historical verification.
- The checkpoint path can be selected through a public or generic migration interface.
- The new entrypoint can operate on live validator data.
- `verify_finalized_checkpoint` does not run after reconstruction.
- Staging can be published before checkpoint verification succeeds.
- Any tampered snapshot is accepted after its unsigned file hashes are updated.
- Height, tip, root, committee, registry, receipt relationships, counts, or integrity checks differ during qualification.
- The signed-backup round trip is incomplete or cannot be independently checked.
- The old signed release or rollback units are unavailable.
- Any of the six live nodes differs unexpectedly from the pre-rollout baseline.

### 10.2 Stop during rollout if

- A validator fails to restart cleanly.
- A restarted validator reports an unexpected height, tip, state root, committee, registry, or certification status.
- Five-signature quorum or required convergence is lost.
- Payment tests required by the locked FastPay specification fail.
- A failed staging or import destination is promoted or treated as accepted.
- Any unexpected write occurs to a non-target live data directory.
- The safe-rollout tool’s existing gate fails.

### 10.3 Rollback behavior

- Before publication, discard failed private staging directories.
- Treat failed destinations left by the older lineage as disposable offline directories; never promote or reuse them as accepted data.
- During deployment, stop further validator restarts immediately on a stop condition.
- Use the retained rollback units and old signed release according to the existing safe-rollout runbook.
- Preserve diagnostic artifacts separately without replacing live data with a failed import.
- Re-establish the required certified convergence before considering any renewed rollout.
- Do not rewrite archived history, alter balances, weaken invariants, or move funds as a rollback mechanism.

## 11. Definition-of-done checklist

- [ ] `rebuild_transactional_storage` remains the full-history public path and still calls `verify_blocks`.
- [ ] Ordinary full-history imports remain unchanged and reject the legacy replay anomaly.
- [ ] Crate-internal `restore_transactional_checkpoint(options: StorageMigrationOptions)` exists.
- [ ] Only explicit finalized-checkpoint snapshot import calls the internal entrypoint.
- [ ] No public skip-replay flag or equivalent interface exists.
- [ ] The internal entrypoint is limited to fresh disposable import directories.
- [ ] Shared reconstruction retains all file, count, receipt, state, root, tip, integrity, and generation checks.
- [ ] Generation construction is not reported as verification.
- [ ] `verify_finalized_checkpoint` runs after the reconstructed generation is available.
- [ ] Import cannot be accepted or published before checkpoint verification succeeds.
- [ ] Failed mainline staging remains unpublished.
- [ ] Failed older-lineage destinations are treated as disposable and unaccepted.
- [ ] The activated-storage checkpoint regression passes with `storage_activation_height: Some(1)`.
- [ ] The corresponding ordinary full-history import remains rejected.
- [ ] Updated-manifest tampering of state, finality, and registry remains rejected.
- [ ] Missing-certificate, expected-root, expected-tip, receipt, and integrity failures remain rejected.
- [ ] `replicated_state_activation.rs` continues to cover standalone full replay.
- [ ] Evidence explicitly records the finalized-checkpoint basis and storage-format verifier identifier.
- [ ] Checkpoint success is not described as successful historical replay.
- [ ] The exact height-1031 snapshot and signed-backup round trip pass with unchanged required commitments.
- [ ] All six live nodes are checked before deployment.
- [ ] No validator restart occurs before backup qualification.
- [ ] The old signed release and rollback units remain available.
- [ ] Rollout obeys the locked FastPay quorum, key, payment-test, and all-six convergence requirements.
