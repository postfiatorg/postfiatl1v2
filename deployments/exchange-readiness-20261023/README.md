# Exchange-readiness release inputs — cut 2026-10-23, rollout at the operator's go

Release `exchange-readiness-20261023` is `main` at `d9260e93` merged into the release branch as `6b1a0585`
(branch `release/exchange-readiness-20261023`; first cut `6604c93e`, re-qualified with the account history index fix): the deployed `store-handle-20261021`
(`86706699`, executable `eea96ba6…`) plus the changes below. It carries the other lane's
exchange-readiness pull requests (#138–#144, #49), their integration fixes and the two
review follow-ups ([review](../../docs/review/exchange-readiness-merge-review-20261023.md),
[handoff](../../docs/handoffs/2026-10-23___codex__chain_pr_merge_handoff.md),
[plan](../../docs/plans/active/next-validator-release-plan-20261101.md)).
Executable `4085ac70c7bb3c21b46cc67be8cb1bbb3556bfea9f49fbf6b128cdac2adcbef9`
(two identical clean builds). Signed manifest `210544a8…`, publisher `pfc531e0…` (key on this server).
Rollback release: `store-handle-20261021` (`eea96ba6…`). Identities: [RELEASE-ID.txt](RELEASE-ID.txt).

## Status

**Cut, re-qualified locally, not deployed** (2026-10-23, [packet](qualification/README.md)).
Not done today: canary backup, fleet before-state, rollout, index build, activation
([DEPLOY-SHEET.md](DEPLOY-SHEET.md) §0–§9, under the operator's go).

## Scope since `86706699`

`git diff 86706699..6b1a0585 -- crates/ Cargo.lock`: `crates/node`, `execution`, `types`,
`crypto_provider`, `consensus_cobalt`, `rpc_sdk`, `fastpay-prototype`, `fuzz_harness` and the root
`Cargo.lock`. Execution changes are governance-gated and inactive until a record sets their
heights (§8); the admission change applies from install.

| Change | What it does | In the executable |
|---|---|---|
| #142 (`920f7cc4`) | `account_tx`/`tx` return `payment_v2` memo contents, shielded-withdrawal credit rows and correct receipt positions in mixed blocks; index schema v2 (a v1 index is ignored until `account-tx-index-build`). Read path only. | yes |
| #143 (`59343085`) | Destination addresses must be `pf` + 40 lowercase hex: refused at RPC and mempool from install; refused in execution from `recipient_format_activation_height`; optional mixed-case display checksum (`address_checksummed`). | yes |
| #144 (`92866ba3`) | `tec_*` fee claim on authenticated effect failures from `rejected_tx_fee_activation_height`; require-memo account flag (`account_set`) and `issued_payment_v2` from `require_memo_activation_height`. | yes |
| #141 (`28594176`) | Asset-Orchard verifying-key fingerprints and their regeneration check. No executable code change. | no |
| #49 (`d01cb121`) | Reserve-proof successor candidate (`candidate_not_registered`, ELF `b64b7666…`); `program-identity.json`, `a666-successor-program-identity.json` and `elf/` unchanged. Tools and fixtures only. | no |
| `1ebc0faf`, `fa3d9b9b`, `7e7169b6` | Integration: recipient checks and fee claims compose; `IssuedPaymentV2` gets recipient validation; live and archive replay reconciled; reserve consumers repointed; RPC docs. | yes (`1ebc0faf`) |
| `6604c93e` | XR-02: `verify_mempool` counts pending `issued_payment_v2` amounts; XR-01: pin test and comments keep the Orchard deposit funding transfer on plain `execute_transfer`. | yes |
| `d9260e93` | The `account_tx` index build, archive scan and `tx` accept a governance receipt id repeated across governance blocks (a retried amendment keeps its id); ordinary repeats are still refused. Read path only (`crates/node/src/block_finality.rs`). | yes |
| `79d5f909` | Root `Cargo.lock`: rustls-webpki 0.103.16, webpki-roots 1.0.10 (RUSTSEC-2026-0381/-0382). | yes |
| #138, #139, #140 | Docs front door, Coinbase listing package, doc fixes. | no |
| `9d10133c`, `a70ce9f9`, `56be18af` | Wallet-proxy timeout cap, tool lock files, wallet-web lock. | no |
| `2ebc7b13`, `e8a962cd`, `325afbdb`, `5c331e60` | Storage replay evidence harness modes and packager. | no |

## Generated units

After the release-ID change the 33 generated files are byte-identical to the deployed
`store-handle-20261021` files: RPC units keep `--max-requests 100000` and `RestartSec=1`,
the 12 environment files keep `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`. `systemd-analyze verify`
reports nothing for the 12 units ([log](qualification/logs/systemd-analyze-verify.log)).

## Contents

- [DEPLOY-SHEET.md](DEPLOY-SHEET.md): the release-day sequence, go points and rollback.
- `rootfs/etc/`: the generated files (12 units, 12 environment files, six runtime bindings, topology, both circuit metadata files).
- [deployment-manifest.signed.json](deployment-manifest.signed.json): the signed manifest (`210544a8…`); [manifest-input.unsigned.json](manifest-input.unsigned.json): its reviewed inputs.
- `stage-report.template.json`, `validator-bindings.signing.template.json`: the generated stage files with the local prefix replaced by `@STAGE@`.
- [node-builds.json](node-builds.json): both builds, toolchain and arguments.
- [local-preflight.py](local-preflight.py), [observe-fleet.py](observe-fleet.py), [demo-preflight.py](demo-preflight.py), [rollback-one.sh](rollback-one.sh): adapted from `store-handle-20261021`; `rollback-one.sh` returns one validator to `store-handle-20261021`.
- [read-import-state.py](read-import-state.py): local read of a signed canary import: the three activation heights and both routes' `authority_epoch`.
- [inventory.txt](inventory.txt): the unchanged rollout inventory.
- [qualification/](qualification/README.md): the local qualification packet.

Not in Git: private keys and the signed stage under
`~/.postfiat/deployments/exchange-readiness-20261023/` on the signing workstation.
No key material is in this directory.
