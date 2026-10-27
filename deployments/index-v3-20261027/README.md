# Index-v3 release inputs — cut 2026-10-27, rollout at the operator's go

Release `index-v3-20261027` is `main` at `d728f895` (branch `release/index-v3-20261027`): the deployed
`exchange-readiness-20261023` (`6b1a0585`, main `d9260e93`, executable `4085ac70…`) plus the changes below
([plan](../../docs/plans/active/next-validator-release-plan-20261108.md)). Executable
`b5cf3ce4bc046e6338beca6d5ead6fd0343426ddb01ac2f461c31e2775f9c8eb` (two identical clean builds). Signed
manifest `a61e290a…`, publisher `pfc531e0…` (key on this server). Rollback release:
`exchange-readiness-20261023` (`4085ac70…`). Identities: [RELEASE-ID.txt](RELEASE-ID.txt).

## Status

**Cut, locally qualified, not deployed** (2026-10-27, [packet](qualification/README.md)).
Not done today: canary backup, fleet before-state, rollout, index build
([DEPLOY-SHEET.md](DEPLOY-SHEET.md) §0–§9, under the operator's go). This release has no governance step.

## Scope since `d9260e93`

`git diff d9260e93..d728f895 -- crates/ Cargo.lock`: 29 files in `crates/node`, 3 in `crates/consensus_cobalt`
(not called by consensus) and the new `crates/pft-token-contracts` (not built into the node). `crates/types`,
`crates/execution`, `crates/storage` and `Cargo.lock` are unchanged, so no consensus, execution or
state-commitment result changes; every change rides along with the standard qualification (plan rule).

| Change | What it does | In the executable |
|---|---|---|
| `0ac2bc9b`, `56603e04` | The `account_tx` index follows block commits: each commit appends its rows after the block is durable; never fails the commit. | yes |
| `8f2217d7` | Index schema v3: per-account chunks of at most 64 rows and metadata-only aggregate files; a commit writes only its rows. A v2 index is reported unsupported and not read until `account-tx-index-build` rebuilds it; reads use the archive scan until then. | yes |
| `d9463929` | `transport-peer-certified-batch-round` never sends the block to the proposer's own validator and applies it through the store it holds. | yes |
| `1432b8e0` | The proposer writes `block-vote-records/height-<h>.json` after its local apply (a new stored artifact; consensus does not read it; not in the signed snapshot). | yes |
| #161 (`43f309b2`) | `verify_bridge` answers over `rpc-serve` with no transfers. | yes |
| #149 (`5cc32792`) | The certified-asset-ops preflight. | yes |
| #153 (`c3f92c5f`) | `pftl-swapd` refuses best-effort requests it cannot serve. | no (separate binary) |
| `62c0d56a`, `2d4da27c` | Epoch-aware FastSwap bootstrap and wallet payload helpers. | no (helper binaries) |
| `1b317fc4` | `entry_profile_v1` rule module and its Python mirror; consensus does not call it. | linked, not called |
| `1b5daa6e` | Comment wording in the FastSwap bootstrap payload tool. | no |
| `ad5ba32f`, `62db8c0e` | The PFT ERC-20 token project, `crates/pft-token-contracts` (Foundry; not part of the Rust workspace build). | no |
| `202e5605` | `deny.toml` accepts RUSTSEC-2026-0387 until the k256 0.14 upgrade. | no |
| #95 and the rest | Wallet-proxy primed-route pin, storage packet tooling, bot fixes to scripts and wallets, docs. | no |

## Generated units

After the release-ID change the 33 generated files are byte-identical to the deployed
`exchange-readiness-20261023` files: RPC units keep `--max-requests 100000` and `RestartSec=1`, the 12
environment files keep `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD` ([packet](qualification/README.md#generated-units)).

## Contents

- [DEPLOY-SHEET.md](DEPLOY-SHEET.md): the release-day sequence, go points and rollback.
- `rootfs/etc/`: the generated files (12 units, 12 environment files, six runtime bindings, topology, both circuit metadata files).
- [deployment-manifest.signed.json](deployment-manifest.signed.json): the signed manifest (`a61e290a…`); [manifest-input.unsigned.json](manifest-input.unsigned.json): its reviewed inputs.
- `stage-report.template.json`, `validator-bindings.signing.template.json`: the generated stage files with the local prefix replaced by `@STAGE@`.
- [node-builds.json](node-builds.json): both builds, toolchain and arguments.
- [local-preflight.py](local-preflight.py), [observe-fleet.py](observe-fleet.py), [demo-preflight.py](demo-preflight.py), [rollback-one.sh](rollback-one.sh): adapted from `exchange-readiness-20261023`; `rollback-one.sh` returns one validator to `exchange-readiness-20261023`.
- [read-import-state.py](read-import-state.py): local read of a signed canary import: the three activation heights and both routes' `authority_epoch`.
- [inventory.txt](inventory.txt): the unchanged rollout inventory.
- [qualification/](qualification/README.md): the local qualification packet.
