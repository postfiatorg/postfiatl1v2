# Consensus signing-fix release qualification

**Result:** `HOLD_NOT_DEPLOYMENT_QUALIFIED`  
**Date:** 2026-09-09 UTC  
**Scope:** Local server builds and disposable loopback clones only. No validator
host, service, fleet configuration, or live chain was touched.

Current `main` at `4153376b8097670ed1af0319affdf204ed85b2f5` produced
candidate binary SHA-256
`af4ccc3e3f7ca626b309de92e1172ccd60f0f0854f2731a6edd3dbbc9a947def`.
The focused signing tests and the archival six-clone service exercise pass.
Deployment qualification does not: the second clean build has a different
hash, current `main` is not a signing-only successor to the deployed A666
source lineage, no authorized current-height all-six clone is available on
this server, and the exact deployed rollback binary is not available locally.

The [decision sheet](../../docs/governance/signing-fix-deploy-decision-20260909.md)
therefore leans C: hold for a fuller campaign. It authorizes nothing.

## Evidence index

- [qualification-receipt.json](qualification-receipt.json) — aggregate gate
  result and blockers.
- [release-builds.json](release-builds.json) — locked build identities and the
  exact-hash reproducibility failure.
- [source-lineage.md](source-lineage.md) and
  [source-commit-range.txt](source-commit-range.txt) — deployed-source
  reconstruction and runtime classification.
- [deployed-source-verification.txt](deployed-source-verification.txt) —
  historical deployed manifest verification.
- [local-clone-gate.json](local-clone-gate.json) and
  [run_local_service_gate.py](run_local_service_gate.py) — signed archival
  snapshot and loopback-only service/finality/restart evidence.
- [consensus-test-results.txt](consensus-test-results.txt) — focused regression,
  consensus, transport/RPC, and Clippy summaries with raw-log hashes.
- [rollback-binary-check.txt](rollback-binary-check.txt) — bounded local search
  for the exact deployed binary.
- [text-improvement-score.json](text-improvement-score.json) — first compliant
  full Text Improvement Harness score for the decision sheet.
- [SHA256SUMS](SHA256SUMS) — hashes for the published record.

Private clone directories, signer material, raw service logs, build trees, and
Text Improvement Harness databases remain outside the repository. This record
does not turn local clone evidence into live authority.
