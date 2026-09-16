# PR41 release blocker repairs

Task Node: `task_0747d04c6cffb93874ae122c17faeae6` (accepted). Work uses the isolated release branch and disposable local chain copies.

- [x] Trace all three failures against captured height-1020 history and original build records.
- [x] Lock the [compatibility specification](../specs/release-history-compatibility-20260916.md): 88.13/100 across 15 valid scores (five per model), after the required rewrite. SHA-256: `064980293a70c5d823ec8b300618077148934f4de809d46b2dfae35f8bd6a93a`. One malformed GLM result was retried with the 14 valid results retained. Existing locked specifications remain unchanged.
- [ ] Preserve FastPay v1 bytes; govern v2 committee and retained-certificate commitments. Owners: `crates/types/src/fastpay_recovery_types.rs`, `crates/execution/src/owned_transfer_recovery.rs`, `crates/node/src/state_commitment.rs`.
- [ ] Pass Orchard balances through normal archive replay and prove live/replay equality. Owner: `crates/node/src/execution_actions.rs`.
- [ ] Reproduce each deployed withdrawal proof identity from its own immutable source and enforce identity selection. Owners: `.github/workflows/arc-proof-identities.yml`, `tools/pfusdc-tier4-prover/`.
- [ ] Pass focused regressions, captured-chain replay, node reproducibility, six-node continuation/restart and exact-binary rollback; then run the final workspace gate.
- [ ] Expose working checks through the existing operator CLI and docs; publish a new qualification packet and update PR41.
- [ ] Complete Task Node evidence and verification; retire this milestone into completed plans.

Live deployment and governance activation remain separate operator actions.
