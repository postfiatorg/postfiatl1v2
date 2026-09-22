# Combined release tip qualification — September 22, 2026

**Local result: FAIL. Nothing was deployed.** Source tip: `048d23df95929b1d544c8c13825e59844eb70e26`.
This packet follows the [September 16 operator procedure](../release-repair-20260916/README.md).
All database operations and service processes used new disposable local copies and six-peer loopback networking.
The captured databases are logical-history copies, not an atomic fleet backup.
No validator host, live service, or live transaction was touched. Nothing was deployed. No source repair was made.
Task Node task: `task_5aa885411809aacca14a5ae2b2f47840`; status: **Rewarded**. [Lifecycle receipt](task-node.json).
The 80-minute checkpoint was pushed as `a2b37ebc492579754cfcd611a5eafb1f86dc9cc4` at 2026-09-22T09:07:49.488301+00:00. [Push receipt](checkpoint-push.json).

Executable SHA-256, build 1: `3bffb105c9d5d9c3487b2b0e0758bd2ffdf557be5118b583d760d58006f5ffd1`.
Executable SHA-256, build 2: `3bffb105c9d5d9c3487b2b0e0758bd2ffdf557be5118b583d760d58006f5ffd1`.
Reproducibility: **PASS**. Separate clean source trees and separate copies of existing dependency caches;
these are not claimed as builds from empty caches. Both builds pin the GCC Rust linker and use identical remaps;
ELF dynamic sections are checked for RPATH/RUNPATH. All candidate history and service checks use build 1.
The requested September 22 reference packet did not exist at the fetched tip; this packet follows
September 18's successful build settings and September 16's operator procedure.

| Check | Result | Evidence |
|---|---|---|
| Two matching node executables | PASS | [Build identities and commands](node-builds.json) |
| Six original full-history checks, height 1020 | PASS | [History run](history-run.json) |
| Six saved V2 full-history checks, height 1021 | PASS | [History run](history-run.json) |
| Six freshly rotated full-history checks, height 1021 | PASS | [Fresh V2 history](fresh-v2-history-run.json) |
| Local governed rotation, both startup orders, convergence and restart | PASS | [Gate log](logs/governance-gate.stdout) |
| Pre-activation rollback: six old checkpoints/restarts plus new full replay | PASS | [Rollback log](logs/rollback-services.stdout) |
| Workspace check | PASS | [Log](logs/workspace-check.stdout) |
| Rust formatting | PASS | [Log](logs/format.stdout) |
| proof-input-inventory | FAIL | [Log](logs/proof-input-inventory.stdout) |
| fastpay-types | PASS | [Log](logs/fastpay-types.stdout) |
| fastpay-execution | PASS | [Log](logs/fastpay-execution.stdout) |
| node-fastpay | DEFERRED_TO_CI | [Log](logs/node-fastpay.stdout) |
| live-replay-supply | PASS | [Log](logs/live-replay-supply.stdout) |
| warm-latency | PASS | [Log](logs/warm-latency.stdout) |
| workspace-clippy | FAIL | [Log](logs/workspace-clippy.stdout) |
| cobalt-handoff-tests | PASS | [Log](logs/cobalt-handoff-tests.stdout) |
| Full workspace test suite | LEFT TO CI | Not run locally |
| strict-docs | PASS | [Log](logs/strict-docs.stdout) |
| public-doc-links | PASS | [Log](logs/public-doc-links.stdout) |
| public-secret-scan | TIMEOUT | [Log](logs/public-secret-scan.stdout) |

| Captured validator | Original height 1020 | Saved V2 height 1021 | Fresh V2 height 1021 |
|---|---|---|---|
| validator-0 | PASS | PASS | PASS |
| validator-1 | PASS | PASS | PASS |
| validator-2 | PASS | PASS | PASS |
| validator-3 | PASS | PASS | PASS |
| validator-4 | PASS | PASS | PASS |
| validator-5 | PASS | PASS | PASS |

Per-node verifier stdout and stderr are under `history/`; execution commands, timings,
memory settings, exits and identity comparisons are under `receipts/`.
[Machine-readable qualification](qualification.json) · [Exact commands and scope](verification-commands.md).

## Certified history and rollback scope

Original certified tip:
`9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb`;
root `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6`.

Saved V2 certified tip:
`5323a2a5243f7be4a1c47ec5e308361898d1662ef20df0dc8fa4349843ffeba45908f4d8050a9dca334133bf0b68804e`;
root `d19264ca4419df8826c7b1746b5042639b0d0dd493a0f34f8902b75811ede397ee1b241d59dacba42e69192a6aecca92`.

The fresh local rotation has its own certified tip/root, recorded in [its service receipt](v2-service-receipt.json).
It differs from the saved September 16 rotation. Fresh-copy replays require the new certified identity on all six copies;
saved-copy replays require the exact September 16 identity above. These are separate evidence sets.

The exact old executable hashes to
`57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`.
Rollback covers its certified-checkpoint verification and local service restart on six restored original copies,
paired with the new executable’s full replay of restored validator-0. The old executable’s known historical
full-replay bug is not represented as repaired. This is pre-activation rollback only.

## CI and incomplete work

Full workspace tests remain CI’s responsibility on the pushed release branch. Additional uncompleted local gates: node-fastpay. No PASS is claimed for them.
Historical proof-reproduction, retained-commitment size measurements, and unrelated earlier release obligations
are not requalified here. Prior evidence is not attributed to this source tip.

## Failures

The source remains unchanged; any failing check is retained below with raw logs.

Clippy reported `needless_borrows_for_generic_args` at `crates/node/src/market_bridge.rs:2474`; no source repair was made.

The proof-input inventory reported source-hash drift in `crates/execution/src/nav_vault_asset_execution.rs`. No source repair was made.

- **FAIL proof-input-inventory**: [logs/proof-input-inventory.stdout](logs/proof-input-inventory.stdout); [stderr](logs/proof-input-inventory.stderr)
- **TIMEOUT public-secret-scan**: [logs/public-secret-scan.stdout](logs/public-secret-scan.stdout); [stderr](logs/public-secret-scan.stderr)
- **FAIL workspace-clippy**: [logs/workspace-clippy.stdout](logs/workspace-clippy.stdout); [stderr](logs/workspace-clippy.stderr)

## Resource limits and evidence retention

One Cargo process at a time, one history copy at a time, two build/test workers, and a 20 GiB address-space limit
per verifier process. Replays require at least 8 GiB available at startup and stop below 4 GiB.
Temporary files and databases live on disk under `/home/postfiatchad/.cache/release-repair-20260922`; `/tmp` is used only for the source worktree.
Private keys, signed batches, databases and executables remain outside Git.
An extra publication whitespace check returned exit 2 for blank EOF lines in raw test logs
([driver log](logs/results-publication-driver.log)); those logs are preserved byte-for-byte.
The three requested publication gates passed. No source or test output was rewritten.
`SHA256SUMS` covers every packet file except the checksum manifest itself.
Packet updated: 2026-09-22T09:38:15.781843+00:00.
