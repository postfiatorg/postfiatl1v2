# PR41 release blocker repairs

Task Node: `task_0747d04c6cffb93874ae122c17faeae6` (accepted). Work uses the isolated release branch and disposable local chain copies.

- [x] Trace all three failures against captured height-1020 history and original build records.
- [x] Lock the [compatibility specification](../specs/release-history-compatibility-20260916.md): 88.13/100 across 15 valid scores (five per model), after the required rewrite. SHA-256: `064980293a70c5d823ec8b300618077148934f4de809d46b2dfae35f8bd6a93a`. One malformed GLM result was retried with the 14 valid results retained. Existing locked specifications remain unchanged.
- [x] Preserve FastPay v1 bytes; govern v2 committee and retained-certificate commitments. Owners: `crates/types/src/fastpay_recovery_types.rs`, `crates/execution/src/owned_transfer_recovery.rs`, `crates/node/src/state_commitment.rs`.
- [x] Pass Orchard balances through normal archive replay and prove live/replay equality. Owner: `crates/node/src/execution_actions.rs`.
- [x] Reproduce each deployed withdrawal proof identity from its own immutable source and enforce identity selection. Owners: `.github/workflows/arc-proof-identities.yml`, `tools/pfusdc-tier4-prover/`.
- [x] Pass focused regressions, captured-chain replay, node reproducibility, six-node continuation/restart and exact-binary rollback; then run the final workspace gate.
- [x] Expose working checks through the existing operator CLI and docs. Qualification packet: `deployments/release-repair-20260916/`; publication and PR41 update accompany this evidence change.
- [ ] Complete Task Node evidence and verification; retire this milestone into completed plans.

Evidence at source `1c435f4f`: all six saved nodes passed full replay through the local V2 installation at height 1021; both retained withdrawal identities passed [CI reproduction](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35044347138). The final workspace gate passed (1,433 passed, zero failed, 39 ignored), as did isolated latency, check and Clippy. Exact-old-binary checkpoint/restart passed on six restored copies; final-source full replay of restored validator-0 passed at height 1020. Results, raw logs and exact commands are in `deployments/release-repair-20260916/`.

Live deployment and governance activation remain separate operator actions.
