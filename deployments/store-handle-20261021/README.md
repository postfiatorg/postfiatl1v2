# Store-handle release inputs — cut 2026-10-21, rollout at the operator's go

Release `store-handle-20261021` is `main` at `86706699` (branch
`release/store-handle-20261021`): the deployed `fastlane-cap-20261014`
(`8bed9b90`, executable `b1b3dbb1…`) plus the changes below. It carries the STO-07
store-handle fix and the offline `fastswap-committee-migrate` command that moves each
validator's FastSwap store to the bridge committee activated at height 1131
([plan](../../docs/plans/active/next-validator-release-plan-20261101.md)).
Executable `eea96ba6761ed223047d7781663100d34efe5641d36031bd50874bd9c8b41f36`
(two identical clean builds). Signed manifest `8ce1da30…`, publisher `pfc531e0…` (key on this server).
Rollback release: `fastlane-cap-20261014` (`b1b3dbb1…`). Identities:
[RELEASE-ID.txt](RELEASE-ID.txt).

## Status

**Deployed** (`status=DEPLOYED`, 2026-10-21 11:43:17Z). All six validators run
`eea96ba6…` with the signed manifest `8ce1da30…`, at height 1137 after one
certified faucet grant per validator (1132–1137). Before state, signed canary
backup, applies, grants and after state: [observed/](observed/)
([rollout record](observed/rollout-record.json)); after checks in the
[qualification packet](qualification/README.md#after-rollout).

FastSwap committee migration done on all six (2026-10-21 12:07–12:30Z): each store and
service at committee epoch 2, root `72836827…`, enabled ([DEPLOY-SHEET.md](DEPLOY-SHEET.md) §7).

## Scope since `8bed9b90`

`git log 8bed9b90..86706699 -- crates/` changes two crates, `crates/node` and
`crates/storage`, in 26 files, each touched by one of the changes below. None changes a
consensus or execution result: no file in `crates/execution`, `crates/types`,
`crates/consensus` or `Cargo.lock` changed.

| Commit | Change |
|---|---|
| `8644f0af` | STO-07: the transactional store stays open for the life of a consensus operation (request, round, `status`) instead of reopening per lookup; disk format, pointer file, consensus and execution unchanged ([G4 re-run PASS](../../docs/plans/active/g4-regression-deployed-release-20261019.md#result-2026-10-20-attempt-3-after-the-fix)). |
| `2737f080` | Offline `postfiat-node fastswap-committee-migrate`: moves a stopped validator's `fastswap-v1/` store to the activated committee epoch ([runbook](../../docs/runbooks/fastpay-committee-recovery.md#moving-the-fastswap-store-to-the-activated-committee-epoch)). |
| `38d8308b` (PR #134, `4559ca36`, `6b80e5a9`) | A migrated FastSwap store keeps opening at epoch 2: open checks the migrated `base-state.json` and its own policies against the anchored drain checkpoint. |
| `8dcd29f7` (PR #136, `05d3c720`) | `operator-attestation-create`/`-verify` are routed to their handlers. |
| `0a4d6967` (PR #135, `1274f67c`) | `pftl_swapctl` creates a missing record directory as 0700. |
| `b18e4e02` (PR #131, `4bc5dca3`) | Test only: unique `cobalt_handoff` fixture root per process. |
| `8acb5f4c`, `8f9e0297`, `4433ce5a`, `f6c22caa`, `0cbe0c54`, `e855ad80` (PRs #129, #130, #132, #133, #137) | Python client, wallet-proxy and docs only; not in the executable. |

## Generated units

After the release-ID change, the 33 generated files are byte-identical to the
deployed `fastlane-cap-20261014` files (`diff -r`, no difference): the RPC units
keep `--max-requests 100000` and `RestartSec=1`, the 12 environment files keep
`POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`. `systemd-analyze verify` reports nothing for
the 12 units ([log](qualification/logs/systemd-analyze-verify.log)).

## Contents

- [DEPLOY-SHEET.md](DEPLOY-SHEET.md): the release-day sequence, go points and rollback.
- `rootfs/etc/`: the generated files (12 units, 12 environment files, six
  runtime bindings, topology, both circuit metadata files).
- [deployment-manifest.signed.json](deployment-manifest.signed.json): the signed
  manifest (`8ce1da30…`); [manifest-input.unsigned.json](manifest-input.unsigned.json): its reviewed inputs.
- `stage-report.template.json`, `validator-bindings.signing.template.json`: the
  generated stage files with the local prefix replaced by `@STAGE@`.
- [node-builds.json](node-builds.json): both builds, toolchain and arguments.
- [local-preflight.py](local-preflight.py), [observe-fleet.py](observe-fleet.py),
  [demo-preflight.py](demo-preflight.py), [rollback-one.sh](rollback-one.sh):
  adapted from `fastlane-cap-20261014` (branch `release/fastlane-cap-20261014`);
  `rollback-one.sh` returns one validator to `fastlane-cap-20261014`.
- [observed/bridge-activation/](observed/bridge-activation/): the 2026-10-15
  `AnchorCheckpoint` and `ActivateCommittee` files the §7 migration reads, copied
  unchanged from the `fastlane-cap-20261014` packet (identical to the test fixtures in
  `crates/node/tests/fixtures/fastswap-committee-migration/`).
- [inventory.txt](inventory.txt): the unchanged rollout inventory.
- [qualification/](qualification/README.md): the local qualification packet.

Not in Git: private keys and the signed stage under
`~/.postfiat/deployments/store-handle-20261021/` on the signing workstation.
No key material is in this directory.
