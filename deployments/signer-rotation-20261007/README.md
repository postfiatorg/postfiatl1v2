# Signer-rotation release inputs — cut 2026-10-06, rollout 2026-10-09 at the earliest

Release `signer-rotation-20261007` is `main` at `0fa55d0b` (branch
`release/signer-rotation-20261007`): the deployed `combined-fastpay-20260928`
(`c93b2137`, executable `1f8b332d…`) plus the changes below. It carries what the
signer-group change needs: the issuer-signed bridge policy update and the
offline FastSwap store conversion ([plan](../../docs/plans/active/next-validator-release-plan-20261005.md)).
Executable `decaa411376a125fb377a29037b6dd470afb2208a268d30a3ce8130f571b370f`
(two identical clean builds). Signed manifest `fa4649aa…`, publisher `pfc531e0…`.
Rollback release: `combined-fastpay-20260928` (`1f8b332d…`). Identities:
[RELEASE-ID.txt](RELEASE-ID.txt).

## Status

**Deployed** (`status=DEPLOYED`, 2026-10-09 12:32:09Z). All six validators run
`decaa411…` with the signed manifest `fa4649aa…`, at height 1121 after one
certified faucet grant per validator (1116–1121). Before state, signed canary
backup, applies, grants and after state: [observed/](observed/)
([rollout record](observed/rollout-record.json)). The local qualification
passed on 2026-10-06 ([qualification packet](qualification/README.md)).

Not done yet: the FastSwap store conversion (§7) and the signer-group activation (§8).

## Scope since `c93b2137`

Every crate file that differs between `c93b2137` and `0fa55d0b` is touched by
one of these commits (`git diff --name-only c93b2137 0fa55d0b -- crates/`, 44 files).

| Commit | Change |
|---|---|
| `1bb15a78` | `status` reports `deployment_manifest_verified=true` from the record `deployment-manifest-verify` writes; the units set `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`. |
| `b1d1928c` (PR #55) | `account_tx` sets `truncated` only when rows are omitted. |
| `0609cb01` (PR #48) | One Apple-only storage line (no Linux change) plus smoke and monitor tooling. |
| `dc7d9bb6` (PR #51) | Devnet node helper scripts only; not in the binary. |
| `9e4d80be`, `8d1c9c20` | `pftl_uniswap_route_bridge_policy_update`: the issuer advances a route's bridge `authority_epoch` and `committee_root`. |
| `e3bb0dbf` | RPC unit: `--max-requests 100000`, `RestartSec=1`; the report is written before the listener closes. |
| `a683475a` | `fastpay-committee-prepare`: read-only builder of the six-member FastPay committee record. |
| `78c7b2e9` | Tests and record of the six-validator local dry run of the signer committee rotation. |
| `672b707c` | `fastswap-control-prepare`, `-vote-sign`, `-assemble`: per-validator `StopPrepare`/`ActivateCommittee` certificates. |
| `d9c42a79` | `fastswap-store-migrate`: offline conversion of the legacy FastSwap WAL to keyed checksums, with backup and restore on failure. |
| `cf65b81e` | Test only: deterministic Cobalt shadow catch-up test. |
| `69c598da` | `account_tx` archive scan and Python fallback scan return the newest rows and agree on `truncated`. |
| `d7212f34` | Tests for `StopPrepare` admission, legacy snapshot conversion and the FastPay epoch-2 boundary. |
| `0fa55d0b` | Wallet lock file only (`wallet-web/`); not in the binary. |

## Generated units

Compared with the deployed `combined-fastpay-20260928` units after the
release-ID change, the 33 generated files differ only in:

- six RPC units: `--max-requests 10000` → `100000`, `RestartSec=5` → `1`;
- twelve environment files: one added line
  `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD=/var/lib/postfiat/validator-N/readiness/{rpc,transport}.deployment-verified.json`.

`systemd-analyze verify` reports nothing for the 12 units
([log](qualification/logs/systemd-analyze-verify.log)).

## Contents

- [DEPLOY-SHEET.md](DEPLOY-SHEET.md): the release-day sequence, go points and rollback.
- `rootfs/etc/`: the 33 generated files (12 units, 12 environment files, six
  runtime bindings, topology, both circuit metadata files).
- [deployment-manifest.signed.json](deployment-manifest.signed.json): the signed
  manifest (`fa4649aa…`); [manifest-input.unsigned.json](manifest-input.unsigned.json): its reviewed inputs.
- `stage-report.template.json`, `validator-bindings.signing.template.json`: the
  generated stage files with the local prefix replaced by `@STAGE@`.
- [node-builds.json](node-builds.json): both builds, toolchain and arguments.
- [local-preflight.py](local-preflight.py), [observe-fleet.py](observe-fleet.py),
  [demo-preflight.py](demo-preflight.py), [rollback-one.sh](rollback-one.sh):
  adapted from `../combined-fastpay-20260928/`; `rollback-one.sh` returns one
  validator to `combined-fastpay-20260928`.
- [inventory.txt](inventory.txt): the unchanged rollout inventory.
- [qualification/](qualification/README.md): the local qualification packet.

Not in Git: private keys and the signed stage under
`~/.postfiat/deployments/signer-rotation-20261007/` on the signing workstation.
No key material is in this directory.
