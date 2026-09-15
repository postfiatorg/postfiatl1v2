# RPC status cache repair — 2026-09-06

Status: deployed and verified on all six validators at `2026-09-06T20:04:03.087166+00:00`. All twelve transport/RPC processes run the new binary; served status and block data agree at height 992.

The P1 from the [storage/Cobalt/Task Node review](../review/storage-cobalt-tasknode-handoff-review-20260906.md) was an indefinitely stale `rpc-serve` status cache. Its one-second timer only limited checks of legacy JSON file metadata. Transactional commits could advance redb without changing those files, so a successful metadata check repeatedly renewed the old report. At 19:38 UTC, six RPC endpoints reported heights 989, 990, 991, 980, 981, and 988 while every endpoint served the same block 992.

`crates/node/src/rpc_cli.rs` now expires the status report itself after one second and reloads authoritative state. The refresh timestamp is taken before the read and published only after success. An unsuccessful refresh returns an error and cannot make the old report fresh again. Mempool caching and consensus behavior are unchanged.

`crates/node/src/main_parts/tests/rpc_serve_request_tests.rs` adds a regression that commits two transactional blocks, asserts that the legacy metadata is unchanged, expires the cached block-1 report, and requires block 2. It also verifies cache hits while fresh, repeated errors after a failed refresh, and recovery after the underlying read succeeds. The regression failed on the old cache and passes with the fix.

- [x] Main checkout: all 25 focused RPC tests pass.
- [x] Exact release source: all 25 focused RPC tests pass in the release profile.
- [x] Reconstruct the deployed V4 source at base `707e006fe2460048b1c7df0d4dc0a04d487f8542`, verify every recorded source-file hash, and apply only this P1 patch on top.
- [x] Verify all six current process binaries and service units against the V4 deployment; direct state reads converge at block 992.
- [x] Verify all six new signed deployment bindings with the existing rollout verifier. Service unit changes are limited to release paths.
- [x] Deploy the canary, then each remaining validator with local backup and rollback support.
- [x] Verify all twelve running process hashes, unchanged checkpoints, and fresh served RPC status across cache expiration.

Release: `rpc-status-cache-20260906`. Binary SHA-256: `740d2610f312a54fd2d1b9bec18a52c6d4390dc5994e6e6f7497a98b36e44b46`. The original V4 binary is retained. Source patches, source manifest, stage verification, and before/after regression logs are in `deployments/rpc-status-cache-20260906/`. Canonical implementation changes remain separate from the checkout's unrelated uncommitted work; the isolated release checkout is `postfiatl1v2-rpc-status-fix-20260906`.

The rollout uses the existing bounded devnet repair procedure from the V4 deployment: frozen current unit hashes, signed per-validator bindings, stopped same-host state backups with root-only access, atomic binary staging, sequential service replacement, and restoration of previous units on failure. Backups stay on their original hosts. No wallet transactions or new blocks are required for this repair.

The review's P2 findings remain open. This repair does not change Cobalt authority, Task Node admission policy, or storage retention.

Deployment and final read-only checks: [receipt](../../deployments/rpc-status-cache-20260906/deploy-receipt.json) and [fleet observations](../../deployments/rpc-status-cache-20260906/fleet-after.json). All six checkpoints remained unchanged; all 18 status samples reported height 992 across cache expiration.
