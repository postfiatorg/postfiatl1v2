# Combined release tip qualification — September 18, 2026

**Local repair result: PASS for both targeted failures. Nothing was deployed.**
Release executable source: `03e422a722eba5bc37e9b3ea71ecae81c02d6f45`.
This packet follows the [September 16 operator procedure](../release-repair-20260916/README.md).

History, governed continuation, restart and rollback checks ran on the earlier
`d224c0be402dbd7bda4fa350ad3f349f009d0667` build 1, SHA-256
`0ea47d5aa6f0ba558cbd341347e34097114fe8942cbfca34c161690b423608f0`.
Those database operations and services used disposable local copies and six-peer loopback networking;
the captures are logical-history copies, not an atomic fleet backup. They were not rerun for this repair.
The only Rust source change is the Clippy test assertion; production Rust source is unchanged.
No validator host, live service, live transaction, or Task Node was touched.
The 80-minute checkpoint was pushed as `10b7b2030838adc4cec3dfefdec5a96cf3a2eeab` at 2026-09-18T12:00:18.654554+00:00. [Push receipt](checkpoint-push.json).

Executable SHA-256, build 1: `051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d`.

Executable SHA-256, build 2: `051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d`.

Reproducibility: **PASS**, `cmp` exit 0; both unmodified executables are 62,014,976 bytes.
Separate clean source trees use separate copies of existing dependency caches;
these are not claimed as builds from empty caches. Both builds pin `/usr/bin/gcc` as the Rust linker,
use identical remap arguments and contain no ELF RPATH/RUNPATH. The failed attempt is retained
as `SUPERSEDED` in [node-builds.json](node-builds.json).

| Check | Result | Evidence |
|---|---|---|
| Two clean release builds | PASS at `03e422a7`, SHA-256 `051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d` | [Build identities and commands](node-builds.json) |
| Six original full-history checks, height 1020 | PASS | [History run](history-run.json) |
| Six saved V2 full-history checks, height 1021 | PASS | [History run](history-run.json) |
| Six freshly rotated full-history checks, height 1021 | PASS | [Fresh V2 history](fresh-v2-history-run.json) |
| Local governed rotation, both startup orders, convergence and restart | PASS | [Gate log](logs/governance-gate.stdout) |
| Pre-activation rollback: six old checkpoints/restarts plus new full replay | PASS | [Rollback log](logs/rollback-services.stdout) |
| Workspace check | PASS | [Log](logs/workspace-check.stdout) |
| Rust formatting | PASS at `03e422a7` | [Receipt](receipts/fix-format.json) |
| proof-input-inventory | PASS | [Log](logs/proof-input-inventory.stdout) |
| fastpay-types | PASS | [Log](logs/fastpay-types.stdout) |
| fastpay-execution | PASS | [Log](logs/fastpay-execution.stdout) |
| node-fastpay | DEFERRED_TO_CI | [Log](logs/node-fastpay.stdout) |
| live-replay-supply | PASS | [Log](logs/live-replay-supply.stdout) |
| warm-latency | DEFERRED_TO_CI | [Log](logs/warm-latency.stdout) |
| workspace-clippy | PASS at `03e422a7` | [Log](logs/fix-workspace-clippy.stderr) |
| Cobalt handoff tests | PASS, 13 passed | [Log](logs/fix-cobalt-handoff-tests.stdout) |
| Full workspace test suite | CI PASS at earlier `d224c0be` | [CI receipt](ci-source-rust-run.json) |
| strict-docs | PASS | [Log](logs/fix-strict-docs.stderr) |
| public-doc-links | PASS | [Log](logs/fix-public-doc-links.stdout) |
| public-secret-scan | PASS | [Log](logs/fix-public-secret-scan.stdout) |

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

Full workspace tests remain CI’s responsibility on the pushed release branch. Additional uncompleted local gates: node-fastpay, warm-latency. No PASS is claimed for them. The existing [CI run](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35335763469) covers `d224c0be`, with test success and check failure; it is not CI evidence for the Clippy fix. See [CI receipt](ci-source-rust-run.json). Fresh release-branch CI and the [earlier packet’s separate release obligations](../release-repair-20260916/README.md) remain before deployment; this local repair does not authorize deployment.
Historical proof-reproduction, retained-commitment size measurements, and unrelated earlier release obligations
are not requalified here. Prior evidence is not attributed to this source tip.

## Failures

The initial build attempt failed reproducibility: byte 8703 (zero-based offset `0x21fe`)
falls in ELF `.dynstr` (`0x19c4`–`0x22fd`), in the `DT_RUNPATH` string.
It contains `build-target-1` versus `build-target-2`; six more bytes differ in
`rustcEjttPN/raw-dylibs` versus `rustc1yK2Ze/raw-dylibs`, and byte 8804 differs
in the second target path. These eight bytes are the complete executable difference.
The ambient `cc` is a Zig wrapper that adds library search directories to RUNPATH;
Rust source/target remapping does not rewrite those linker-created strings.
Today's invocation omitted the explicit `/usr/bin/gcc` Rust linker selection in the
[original build provenance](../combined-release-20260915/build-provenance.md),
which the September 16 builds reused. Those earlier binaries have no RPATH/RUNPATH.
The repair restores that linker selection and uses identical remap arguments in both builds.

Clippy rejected `.err().expect()` at `crates/node/src/cobalt_handoff.rs:1466`
(`clippy::err_expect`). Commit `03e422a722eba5bc37e9b3ea71ecae81c02d6f45`
replaces it with `.expect_err(...)` in test code only. Workspace Clippy with
`-D warnings`, all 13 `cobalt_handoff::tests`, and formatting pass.
The failed build diagnosis is retained in [binary-diagnosis.json](binary-diagnosis.json)
and the `failed-build-*-sections`, `-dynamic`, and `-notes` logs.

Both failures are closed. Original failure evidence remains available:

- **SUPERSEDED node-reproducibility FAIL**: [log](logs/node-reproducibility.stdout); [stderr](logs/node-reproducibility.stderr). New [comparison receipt](receipts/fix-node-reproducibility.json): PASS.
- **SUPERSEDED workspace-clippy FAIL**: [log](logs/workspace-clippy.stdout); [stderr](logs/workspace-clippy.stderr). New [Clippy receipt](receipts/fix-workspace-clippy.json): PASS.

## Resource limits and evidence retention

One Cargo process at a time, one history copy at a time, two build/test workers, and a 20 GiB address-space limit
per process. Replays required at least 8 GiB available at startup and stopped below 4 GiB.
Original temporary files and databases remain under `/home/postfiatchad/.cache/release-repair-20260918`.
Repair builds, binaries and disk-backed TMPDIR are under `/home/postfiatchad/.cache/qualify-fix-20260918`;
`/tmp/qualify-fix-20260918` holds only the repair worktree. The repair has a separate 60-minute time box.
Private keys, signed batches, databases and executables remain outside Git.
`SHA256SUMS` covers every packet file except the checksum manifest itself.
Packet updated: 2026-09-18T12:53:38Z.
