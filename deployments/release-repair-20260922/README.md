# Combined release tip qualification — September 22, 2026

**Requested repair follow-up: PASS. Nothing was deployed. CI verdicts remain unconfirmed.**
Build source: `1a0989ad6c35b7ea3eb6418f864958563233945a`; runtime build revision: `1a0989ad`.
The final packet commit records this source and adds evidence only.

The original qualification at `048d23df95929b1d544c8c13825e59844eb70e26`
passed reproducibility, history, rotation and rollback, but failed the inventory and Clippy gates.
This follow-up closes those two failures. It uses local disposable builds only, with no fleet or Task Node action.
The original Task Node task `task_5aa885411809aacca14a5ae2b2f47840` remains
**Rewarded**; its [lifecycle receipt](task-node.json) is unchanged.
The protected release checkout was not accessed.

Executable SHA-256, build 1: `e7bb1afa17b4c6322ac8eadabdab570595ad778a5173a5516c09ba966ed9e4b1`.
Executable SHA-256, build 2: `e7bb1afa17b4c6322ac8eadabdab570595ad778a5173a5516c09ba966ed9e4b1`.
Reproducibility: **PASS**, byte comparison exit 0.
Separate clean source trees and separate copies of existing dependency caches were used;
these are not empty-cache builds. Both builds use the GCC Rust linker, identical path remaps,
and `SOURCE_DATE_EPOCH=1789514690`; neither executable has RPATH/RUNPATH.
The two superseded `048d23df` build records and hashes remain in [node-builds.json](node-builds.json).

History, rotation, rollback and software checks not rerun below remain evidence for
`048d23df`, using that revision's original build 1. No new replay or service run is claimed.

| Check | Result | Evidence |
|---|---|---|
| Two matching node executables | PASS | [Build identities and commands](node-builds.json) |
| Six original full-history checks, height 1020 | PASS | [History run](history-run.json) |
| Six saved V2 full-history checks, height 1021 | PASS | [History run](history-run.json) |
| Six freshly rotated full-history checks, height 1021 | PASS | [Fresh V2 history](fresh-v2-history-run.json) |
| Local governed rotation, both startup orders, convergence and restart | PASS | [Gate log](logs/governance-gate.stdout) |
| Pre-activation rollback: six old checkpoints/restarts plus new full replay | PASS | [Rollback log](logs/rollback-services.stdout) |
| Workspace check | PASS | [Log](logs/workspace-check.stdout) |
| Rust formatting | PASS at `1a0989ad` | [Receipt](receipts/fix-format.json) |
| proof-input-inventory | PASS at `85eee166` | [Reviewed hash and gate receipt](receipts/fix-proof-input-inventory.json) |
| fastpay-types | PASS | [Log](logs/fastpay-types.stdout) |
| fastpay-execution | PASS | [Log](logs/fastpay-execution.stdout) |
| node-fastpay | DEFERRED_TO_CI | [Log](logs/node-fastpay.stdout) |
| live-replay-supply | PASS | [Log](logs/live-replay-supply.stdout) |
| warm-latency | PASS | [Log](logs/warm-latency.stdout) |
| workspace-clippy | PASS at `1a0989ad` | [Receipt](receipts/fix-workspace-clippy.json), [compiler log](logs/fix-workspace-clippy.stderr) |
| node `navcoin_bridge` tests | PASS at `1a0989ad`: 14 passed | [Log](logs/fix-navcoin-bridge.stdout) |
| cobalt-handoff-tests | PASS | [Log](logs/cobalt-handoff-tests.stdout) |
| Full workspace test suite | LEFT TO CI | Not run locally |
| strict-docs | PASS after repairs | [Receipt](receipts/fix-strict-docs.json) |
| public-doc-links | PASS after repairs | [Receipt](receipts/fix-public-doc-links.json) |
| public-secret-scan | PASS, complete tracked-tree scan | [Receipt](receipts/fix-public-secret-scan.json), [log](logs/fix-public-secret-scan.stdout) |

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

## CI and deployment prerequisites

Required CI verdicts must pass on the pushed release tip, including full workspace tests
and the deferred node-fastpay coverage. No fresh CI success is claimed here.
The [September 21 deployment preparation](../combined-devnet-20260921/README.md)
pins an older executable and must be updated for this binary.
Its [key-holder handoff](../combined-devnet-20260921/SIGNING.md) still requires the
**existing trusted deployment publisher key** to sign the new manifest; no replacement
key may be generated. Deployment-day signed backup and operator preflight remain separate.
No deployment, manifest signing, backup export, or fleet action was performed.

Historical proof reproduction, retained-commitment size measurements and unrelated
earlier release obligations are not requalified by this repair follow-up.

## Closed failures and retained evidence

- `85eee16656231f31976e71b7be49534fffc6567f` regenerates one reviewed source pin:
  `crates/execution/src/nav_vault_asset_execution.rs`, changed by `043b9d69`.
  The old hash matches that repair's parent; the new hash matches its source bytes.
  The other 93 hashes, proof-system metadata and all frozen proof artifacts are unchanged.
  The gate passes with seven systems, 150 public fields and 94 source hashes.
- `1a0989ad6c35b7ea3eb6418f864958563233945a` removes only the needless `&` at
  `crates/node/src/market_bridge.rs:2474`. Workspace Clippy with `-D warnings`,
  all 14 selected bridge tests, and formatting pass.
- The full `scripts/public-secret-scan` run completes with exit 0 after staging
  the updated packet. This supersedes the original final scan timeout.

Original evidence remains byte-for-byte:
[inventory failure](logs/proof-input-inventory.stderr),
[Clippy failure](logs/workspace-clippy.stderr),
[secret-scan timeout receipt](receipts/public-secret-scan.json).
The original qualification is retained under `prior_qualification` in
[qualification.json](qualification.json); original software and publication records
remain under `superseded` in their respective JSON files.

## Resource limits and evidence retention

One Cargo process at a time, `CARGO_BUILD_JOBS=2`, two test workers, and a 20 GiB
address-space limit apply to all follow-up commands. Commands require 8 GiB available
at startup and stop below 4 GiB. Temporary and build files live on disk under
`/home/postfiatchad/.cache/qualify-fix-20260922`; the edit worktree is
`/tmp/qualify-fix-20260922`.
The existing September 18 disposable test cache was reused; release builds use
separate new copies of the September 22 caches. Executables remain outside Git.

The original 110-minute qualification and its overrun remain in the archived receipt.
This follow-up has a separate 40-minute limit. All three publication gates pass.
`SHA256SUMS` covers every packet file except itself.
