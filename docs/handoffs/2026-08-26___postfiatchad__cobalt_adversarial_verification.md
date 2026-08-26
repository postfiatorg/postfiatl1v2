# Cobalt adversarial verification continuation

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-26 UTC

## BLUF

The locked [Cobalt adversarial-verification milestone](../plans/active/cobalt-adversarial-verification-milestone.md) is still active. E1-E3 and the design-only E6 decision are complete. E4 is not complete: a required third clean 500+500 local finality-isolation run is currently executing after two documented harness remediations. E5 has not been executed live, the overall `KEEP_ACTIVE` gate remains open, and no adversarial-workflow change from this session is deployed.

Work is on `main`. The pushed pre-handoff work HEAD is `77c17cefafc284d811d4df875fa5f074c3155bc6`; the handoff commit containing this file is its only intended descendant. Commit `add07a7cce416daeaa61073085734937477f2b71` fixes the E4 cross-lane fork oracle, and `77c17cef` adds the fail-closed E4 packet verifier and packager. Preserve the uncommitted E3, E5, CLI, and browser work listed below.

The user explicitly asked that this continuation be done directly: do not use Task Node and do not spawn parallel agents.

## Current state

### Fresh controlled-devnet observation

An authenticated read-only probe ran across all six hosts from `2026-08-26T01:40:51Z` through `2026-08-26T01:41:04Z`. It invoked only service-state checks, process identity/hash reads, and `postfiat-node status`; it did not write files, restart services, or call a mutation RPC.

- All six validator and all six RPC services were `active`.
- Every validator reported chain `postfiat-wan-devnet-2`, status `running`, height 919, zero pending mempool transactions, tip `3a8a117af9ed40728717005d03edf032719a3ca3d696365415a2d5b0d9aeef1c509d06d54029e6c34660e29aab43d0fb`, and state root `ffa16323555800df7a4ff7cd336b9b151b0edfcf60954c207b704749133ff4b31ebd24444696d67e652f6e94510f7e60`.
- Every active validator process was `/opt/postfiat/releases/cobalt-verifier-92b63f5a/postfiat-node`, SHA-256 `c7cb0c25001a0bfe22eba32ce870f3739f9710471906e27c32797670ea9f6337`, with embedded build revision `92b63f5a`.
- This fresh probe did not re-run the governance auditor or inspect the six shadow units. The last authenticated authority evidence still records Cobalt validator-trust authority from height 916, the height-917 key rotation, registry root `945768d593497541f59961d1ba3920560cfde7bf5037e40eb89dd5466637f221709bff05b69d2d40a36d5cff8505c37e`, and trust root `9221316ae7f0f0e7e58d734700167f73f29aa1240377a8d61c637e7f36c5deb728203fcbb283c9f8f3398fc41d6b8b13`.

[Current State](../status/chain-state-current.md) is the canonical page but is now known stale in one important field: it says the active consensus runtime is `cobalt-activation-8694b99d` with hash `431f…`, which the fresh all-six process inspection disproves. Correct it to `cobalt-verifier-92b63f5a` / `c7cb…` and record the fresh capture window. Do not say current `main` is deployed.

### E4: active local run

The first frozen E4 run passed 500 baseline rounds but stopped before attack round 1 because its retry window was shorter than the deliberate validator-5 outage. Commit `451c2ad0` remediated that harness boundary without changing the corpus.

The next clean run completed all 500 baseline and 500 attack rounds. Both six-validator lanes independently converged at height 501, all lane checks passed, attack p95 was `+0.4433232431564571%` versus baseline inside the 5% budget, 45 governance runs exercised 900 proposals, 315 safe halts, and 315 view changes, and validator-5 restarted 12 times. Its top-level report failed only because the postprocessor required identical tips and state roots across two independent executions. That assertion is invalid: randomized ML-DSA transaction and consensus signatures change transaction IDs, certificates, block hashes, and roots between runs. No within-lane fork or durable divergence was observed. The failure receipt is `benchmarks/cobalt-adversarial-verification/e4/remediation/cross-lane-hash-comparator-failure.json`.

Commit `add07a7c` replaces that invalid oracle with:

- exact six-validator convergence inside each lane;
- equal final heights;
- a signed-message-independent comparison of workload configuration and per-round proposer, height, vote, receipt, finality, and round outcomes; and
- explicit non-gating disclosure of whether cross-lane hashes happen to match.

The unchanged mandatory rerun is active:

- OS PID: `1258574`
- disposable run root: `/home/postfiatchad/repos/cobalt-e4-rerun2.wRmS3W`
- source argument: `add07a7c`
- node binary SHA-256: `634f08368c174a288bfc42211dc52ef0725c7f6933acc816e4a9006606189a41`
- Cobalt simulation SHA-256: `6bef2df8a2ef18c11c774309713c878470f54819b98e15face09a9f9ffa62028`
- handoff-time progress at `2026-08-26T01:42:06Z`: baseline 295/500, attack 0/500

The process is attached to the existing Codex exec PTY, not tmux. Do not start a duplicate while PID `1258574` is alive. Poll it with:

```bash
pgrep -af 'run_consensus_v2_cobalt_integration.py.*cobalt-e4-rerun2.wRmS3W'
wc -l /home/postfiatchad/repos/cobalt-e4-rerun2.wRmS3W/output/baseline/iterations.jsonl
wc -l /home/postfiatchad/repos/cobalt-e4-rerun2.wRmS3W/output/attack/iterations.jsonl
```

The integrated report is written only at the end. If the process disappears, inspect `output/consensus-v2-cobalt-integration.json`. Do not infer success from 500+500 line counts. If no integrated report exists, treat the run as interrupted and inspect the logs before deciding whether a fresh clean rerun is needed.

The run root contains disposable private signing material and raw consensus artifacts. Never add the run root or its `private/`, `votes/`, or `artifacts/` trees to Git.

Commit `77c17cef` adds:

- `benchmarks/cobalt-adversarial-verification/package_e4_packet.py`, which refuses non-passing reports, copies only the bounded summary files, normalizes disposable paths, and rebinds evidence hashes; and
- `benchmarks/cobalt-adversarial-verification/e4/verify_packet.py`, which checks the frozen corpus, remediated source, both failure receipts, lane-local convergence, semantic workload equality, metrics, stress/rejection/resource receipts, checksums, and redaction.

The final E4 `README.md` does not exist yet because it must quote the actual third-run result. If the run passes, write that concise README first, run the packager, run the verifier, update the milestone E4 checkboxes, then commit and push the packet. If the run fails, preserve a redaction-safe receipt and leave E4 open; do not package or claim a pass.

### Uncommitted work that must be preserved

The worktree has 19 modified tracked files, about 1,811 insertions. These are intentional and were not included in the E4 commits because E5 remains gated on E4.

1. **E3 verifier correction**
   - `benchmarks/cobalt-adversarial-verification/e3/verify_packet.py`
   - `benchmarks/cobalt-adversarial-verification/e3/SHA256SUMS.txt`
   - The verifier now checks source files from frozen revision `5c9e543e` rather than mutable worktree HEAD. It passes with new packet root `9302b3555ab9091b2cae9b2d372d0548fe9f2fb1e67be43dfb3f63d89140b600`. The milestone still names the older root and must be updated when this correction is committed.

2. **E5 cross-root DABC and shadow-state remediation**
   - `crates/consensus_cobalt/src/dabc_registry.rs`
   - `crates/consensus_cobalt/src/internal_validation.rs`
   - `crates/consensus_cobalt/src/tests.rs`
   - `crates/node/Cargo.toml`
   - `crates/node/src/bin/postfiat_cobalt_shadow.rs`
   - `crates/node/src/cobalt_authority_certificate.rs`
   - `crates/node/src/cobalt_e5_live_drill.rs`
   - `crates/node/src/cobalt_handoff.rs`
   - `crates/node/src/cobalt_handoff_rehearsal.rs`
   - `crates/node/src/cobalt_shadow.rs`

   The identified production defect is that a post-rotation DABC ratification was validated against the current graph roots instead of the prior ratification/lineage anchor. The dirty changes add explicit ratification anchors, contiguous next-round derivation, existing-height-917 persisted-state migration, atomic peer/root persistence, cross-root regression tests, exact rollback/return preparation, and a nine-negative-case plus stolen-key live drill. This code has not yet passed its focused Rust tests or release build and is not deployed.

3. **CLI and read-only browser surface**
   - `python/postfiat_rpc/cobalt.py`
   - `python/postfiat_rpc/cobalt_ui.py`
   - `python/postfiat_rpc/cobalt_ui_assets/app.js`
   - `python/postfiat_rpc/cobalt_ui_assets/index.html`
   - `python/postfiat_rpc/cobalt_ui_assets/styles.css`
   - `python/tests/test_cobalt.py`
   - `python/tests/test_cobalt_ui.py`

   The dirty parser and UI consume the final adversarial packet, validate CLI/browser snapshots, expose no mutation route, and accurately label the present six-view trust graph as `uniform full overlap` rather than falsely calling it non-uniform. These changes have not yet had their final Python test run against a completed E5 packet.

Do not run `git checkout --`, `git reset --hard`, or stage all files blindly. Review and commit the E3 correction, E5 protocol/runtime work, and interface work as separate coherent changes.

### Verification completed in this continuation

- `python3 benchmarks/cobalt-activate-or-retire/test_consensus_v2_cobalt_integration.py` — `e4-final-state-comparator-tests-ok`.
- The comparator was checked against the completed second run: semantic workload equal despite distinct transaction/block identities.
- `python3 -m py_compile` passed for the E4 runner, comparator test, packager, and packet verifier.
- The E4 packager was tested against the failed second run and correctly refused it.
- The E4 path-normalization helper regression passed.
- `git diff --check` passed before this handoff.
- The dirty E3 verifier passed and printed packet root `9302b355…40b600`.

No focused Rust or Python E5 tests have run since the uncommitted E5 changes were assembled. Do not describe them as validated.

## Next decision or action

1. **Babysit E4 to a terminal report.** Do not touch E5 live state while E4 is open. If E4 passes, write the final E4 README, package and verify the evidence, update the milestone and canonical state, then commit and push. If it fails, keep `REMEDIATION_REQUIRED`, record the exact failure, and follow the locked remediation rule.
2. **Reconcile documentation immediately after E4.** Correct the active runtime identity and fresh probe time in [Current State](../status/chain-state-current.md); update the E3 packet root and E4 result in the active milestone. Preserve historical packets and dated handoff bodies as history.
3. **Validate the dirty E5 remediation off-chain.** Review the diff, run formatting and focused Cobalt/governance tests, exercise the exact persisted-height-917 migration and cross-root lineage regressions, then commit and push. Do not build deployment evidence from an uncommitted or failing tree.
4. **Only after that, prepare a signed rolling release and fresh preflight.** The currently deployed `92b63f5a` runtime does not contain the dirty E5 remediation. Any proposed heights 920/921/922 are provisional and must be re-derived from a fresh all-six state and authority probe before scheduling. Deploy one validator/RPC pair at a time, prove convergence after each, then migrate shadow anchors one at a time.
5. **Execute E5 exactly once the runtime is qualified.** The intended sequence is forward rollback to Foundation authority, separately authorized return to Cobalt, all named negative transitions, a certified stolen-key rejection, and legitimate validator-5 rotation. Preserve Consensus v2 finality and one accepted authority history throughout. Stop on any root, lineage, signer, service, or finality mismatch.
6. **Finish the shared CLI/browser packet, publication, and release gate.** The milestone is complete only after E5, authenticated interfaces, consolidated evidence, documentation, final verification, and the explicit `KEEP_ACTIVE`, `REMEDIATION_REQUIRED`, or `ROLLED_BACK` result.

Do not claim that offline E1-E4 evidence is a live probe, that E6 recruited independent operators, that Cobalt controls blocks, or that current Git HEAD is live. Cobalt ratifies validator-registry and trust-graph changes; Consensus v2 remains block finality; current proposal custody is still Foundation-administered.

## References

- [Current State](../status/chain-state-current.md)
- [Active adversarial-verification milestone](../plans/active/cobalt-adversarial-verification-milestone.md)
- [Locked adversarial-verification specification](../governance/cobalt-adversarial-verification-research-spec.md)
- [E4 campaign manifest](../../benchmarks/cobalt-adversarial-verification/e4/campaign-manifest.json)
- [Initial E4 failure receipt](../../benchmarks/cobalt-adversarial-verification/e4/remediation/initial-failure.json)
- [Cross-lane comparator failure receipt](../../benchmarks/cobalt-adversarial-verification/e4/remediation/cross-lane-hash-comparator-failure.json)
- [E4 runner](../../benchmarks/cobalt-activate-or-retire/run_consensus_v2_cobalt_integration.py)
- [E4 packet verifier](../../benchmarks/cobalt-adversarial-verification/e4/verify_packet.py)
- [E4 packet packager](../../benchmarks/cobalt-adversarial-verification/package_e4_packet.py)
- [E3 packet](../../benchmarks/cobalt-adversarial-verification/e3/README.md)
- [E6 decision packet](../../benchmarks/cobalt-adversarial-verification/e6/README.md)
- [Prior post-activation handoff](2026-08-25___dravlic__cobalt_post_activation_review.md)
