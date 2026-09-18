# Combined release tip qualification — September 18, 2026

**Local result: FAIL. Nothing was deployed.** Source tip: `d224c0be402dbd7bda4fa350ad3f349f009d0667`.
This packet follows the [September 16 operator procedure](../release-repair-20260916/README.md).
All database operations and service processes used new disposable local copies and six-peer loopback networking.
The captured databases are logical-history copies, not an atomic fleet backup.
No validator host, live service, live transaction, or Task Node was touched. No source repair was made.
The 80-minute checkpoint was pushed as `10b7b2030838adc4cec3dfefdec5a96cf3a2eeab` at 2026-09-18T12:00:18.654554+00:00. [Push receipt](checkpoint-push.json).

Executable SHA-256, build 1: `0ea47d5aa6f0ba558cbd341347e34097114fe8942cbfca34c161690b423608f0`.  
Executable SHA-256, build 2: `da51edea2b1423f9002290a5ab531e9659f4be5a9e62fbefaab70949ac28cb55`.  
Reproducibility: **FAIL**. Separate clean source trees and separate copies of existing dependency caches;
these are not claimed as builds from empty caches. All candidate history and service checks use build 1.

| Check | Result | Evidence |
|---|---|---|
| Two matching node executables | FAIL | [Build identities and commands](node-builds.json) |
| Six original full-history checks, height 1020 | PASS | [History run](history-run.json) |
| Six saved V2 full-history checks, height 1021 | PASS | [History run](history-run.json) |
| Six freshly rotated full-history checks, height 1021 | PASS | [Fresh V2 history](fresh-v2-history-run.json) |
| Local governed rotation, both startup orders, convergence and restart | PASS | [Gate log](logs/governance-gate.stdout) |
| Pre-activation rollback: six old checkpoints/restarts plus new full replay | PASS | [Rollback log](logs/rollback-services.stdout) |
| Workspace check | PASS | [Log](logs/workspace-check.stdout) |
| Rust formatting | PASS | [Log](logs/format.stdout) |
| proof-input-inventory | PASS | [Log](logs/proof-input-inventory.stdout) |
| fastpay-types | PASS | [Log](logs/fastpay-types.stdout) |
| fastpay-execution | PASS | [Log](logs/fastpay-execution.stdout) |
| node-fastpay | DEFERRED_TO_CI | [Log](logs/node-fastpay.stdout) |
| live-replay-supply | PASS | [Log](logs/live-replay-supply.stdout) |
| warm-latency | DEFERRED_TO_CI | [Log](logs/warm-latency.stdout) |
| workspace-clippy | FAIL | [Log](logs/workspace-clippy.stdout) |
| Full workspace test suite | CI PASS | [CI receipt](ci-source-rust-run.json) |
| strict-docs | PASS | [Log](logs/strict-docs.stdout) |
| public-doc-links | PASS | [Log](logs/public-doc-links.stdout) |
| public-secret-scan | PASS | [Log](logs/public-secret-scan.stdout) |

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

Full workspace tests remain CI’s responsibility on the pushed release branch. Additional uncompleted local gates: node-fastpay, warm-latency. No PASS is claimed for them. Exact-source [CI run](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35335763469) snapshot: test success, check failure. See [CI receipt](ci-source-rust-run.json).
Historical proof-reproduction, retained-commitment size measurements, and unrelated earlier release obligations
are not requalified here. Prior evidence is not attributed to this source tip.

## Failures

Both node build commands succeeded, but their unmodified executable SHA-256 hashes differ.
Clippy rejected `.err().expect()` at `crates/node/src/cobalt_handoff.rs:1466` (`clippy::err_expect`).
The source remains unchanged.

- **FAIL node-reproducibility**: [logs/node-reproducibility.stdout](logs/node-reproducibility.stdout); [stderr](logs/node-reproducibility.stderr)
- **FAIL workspace-clippy**: [logs/workspace-clippy.stdout](logs/workspace-clippy.stdout); [stderr](logs/workspace-clippy.stderr)

## Resource limits and evidence retention

One heavy stage at a time, one history copy at a time, two build/test workers, and a 20 GiB address-space limit
per verifier process. Replays require at least 8 GiB available at startup and stop below 4 GiB.
Temporary files and databases live on disk under `/home/postfiatchad/.cache/release-repair-20260918`; `/tmp` is used only for the source worktree.
Private keys, signed batches, databases and executables remain outside Git.
`SHA256SUMS` covers every packet file except the checksum manifest itself.
Packet updated: 2026-09-18T12:20:48.117984+00:00.
