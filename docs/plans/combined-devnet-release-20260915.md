# Combined devnet release

Task Node: `task_1925021f97adf7488e21e22ff85f08c7`.

Execute the already-qualified [September 9 release decision](../governance/signing-fix-deploy-decision-20260909.md), preserving deployed A666, pfUSDC, pfETH, Arc and Cobalt functionality alongside main's accumulated fixes. This candidate requires deployment approval after qualification.

- [x] Pin main (`faff0e53533888bb229add117fcda3ed4094aed4`) and PR 39 (`f2e749a16b07be446cb11da1e1f523eb9cdabca7`); confirm PR 37 (`c87e7dc663ef7dafc2d17284abb5838a8ef152c1`) is included.
- [x] Merge in an isolated checkout and preserve newer regression checks through conflict resolution.
- [x] Verify deployed source lineage and all seven consensus fixes; both exact PR tips are ancestors.
- [x] Complete the full workspace suite: 1,429 passed, zero failures, 39 ignored; the separately required warm latency test passes. Focused integration, Python, wallet and offline contract tests pass.
- [x] Reproduce the release binary from two clean source trees: SHA-256 `203995122290895a023dde4bf1cfb8cc4f05b22628822d7a32829ffd185fb314`.
- [x] Assess current-chain replay and local service gates: six-copy startup/restart PASS; full verification, replay and checkpoint import FAIL on the historical FastPay committee; new-block finality BLOCKED.
- [x] Verify the exact deployed binary and rehearse original-data restore locally: stored-tip readback PASS on six copies; full verification FAIL at block 1011 on the baseline supply check.
- [x] Publish [draft PR 41](https://github.com/postfiatorg/postfiatl1v2/pull/41) for integration review.
- [x] Complete the [qualification packet](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-release-20260915/README.md) with raw logs and explicit gate results. Deployment verdict: **HOLD**. Egress proof identity also fails; preserved source-history findings and unavailable testnet archive state remain unmet publication/test gates.

Use the existing node CLI, [local service runner](https://github.com/postfiatorg/postfiatl1v2/blob/faff0e53533888bb229add117fcda3ed4094aed4/deployments/signing-fix-qualification-20260909/run_local_service_gate.py), [safe rollout interface](../../scripts/postfiat-safe-rollout), and release decision page. Fleet access during qualification is read-only. No live deployment, restart, configuration change or chain transaction belongs to this task.
