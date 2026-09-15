# Combined devnet release

Task Node: `task_1925021f97adf7488e21e22ff85f08c7`.

Execute the already-qualified [September 9 release decision](../governance/signing-fix-deploy-decision-20260909.md), preserving deployed A666, pfUSDC, pfETH, Arc and Cobalt functionality alongside main's accumulated fixes. This candidate requires deployment approval after qualification.

- [x] Pin main (`faff0e53533888bb229add117fcda3ed4094aed4`) and PR 39 (`f2e749a16b07be446cb11da1e1f523eb9cdabca7`); confirm PR 37 (`c87e7dc663ef7dafc2d17284abb5838a8ef152c1`) is included.
- [x] Merge in an isolated checkout and preserve newer regression checks through conflict resolution.
- [ ] Verify deployed source lineage and all seven consensus fixes.
- [ ] Pass focused integration regressions, then the final release suite.
- [ ] Reproduce the release binary from clean source trees.
- [ ] Replay the current chain and exercise service startup, finality and restart on disposable loopback clones.
- [ ] Verify the exact deployed binary and rehearse data-plus-binary rollback locally.
- [ ] Publish the candidate and concise qualification results for deployment approval.

Use the existing node CLI, [local service runner](https://github.com/postfiatorg/postfiatl1v2/blob/faff0e53533888bb229add117fcda3ed4094aed4/deployments/signing-fix-qualification-20260909/run_local_service_gate.py), [safe rollout interface](../../scripts/postfiat-safe-rollout), and release decision page. Fleet access during qualification is read-only. No live deployment, restart, configuration change or chain transaction belongs to this task.
