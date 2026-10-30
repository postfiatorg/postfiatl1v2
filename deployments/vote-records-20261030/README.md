# Vote-records release inputs — cut 2026-10-30, rollout at the operator's go

Release `vote-records-20261030` is `main` at `ae70d66d` (branch `release/vote-records-20261030`): the deployed
`index-v3-20261027` (source and main `d728f895`, executable `b5cf3ce4…`) plus the changes below
([plan](../../docs/plans/active/next-validator-release-plan-20261108.md)). Executable
`87cfe797af8c8c1c98e08b4794473bd25acb58ce0b7f57f0ea1b1f0fe98023c7` (two identical clean builds). Signed
manifest `ee1df5b3…`, publisher `pfc531e0…` (key on this server). Rollback release: `index-v3-20261027`
(`b5cf3ce4…`). Identities: [RELEASE-ID.txt](RELEASE-ID.txt).

## Status

**Deployed on all six validators on 2026-10-30** ([DEPLOY-SHEET.md](DEPLOY-SHEET.md),
[rollout record](observed/rollout-record.json)). CI on `ae70d66d` green in all three workflows (docs-build
`41032153113`, rust-ci `41032153158`, product-security-ci `41032153060`). Before state 1161; canary `9b68d2a1…`
and its head replay PASS; applied one validator at a time 10:31–11:14Z at 1162–1167; after checks PASS; the
proposer's vote record for 1168 names six, all `accepted`; after state 1168 / `a20e406e…` / `39771b74…`. This
release has no governance step and no index build.

## Scope since `d728f895`

`git log --first-parent --oneline d728f895..ae70d66d`: 101 commits; `git diff d728f895..ae70d66d -- crates/
Cargo.lock`: 94 files. `crates/storage` and `crates/ordering_fast` are unchanged.

| Change | What it does | In the executable |
|---|---|---|
| `5fc43961` (VR-01) | A vote that arrives after the quorum but inside the round deadline is recorded `accepted` (with `answered_after_quorum`), not `no_answer`. Consensus never reads the record. | yes |
| #175 (`72475b01`) | A refusal after the vote deadline stays `refused` with its class. | yes |
| #174 (`924a3fa6`) | The `late_after_deadline` column in the `vote_reliability` table. | no (Python) |
| `968da908`, `d7de5ec7`, `657ce706`, `ea2213d0` | W-1: the validator-owned private action admission store, round service and receipt read; only on `rpc-serve --allow-private-action-admission` (off by default and in the generated units; otherwise `rpc_method_not_allowed`). | yes, switched off |
| `ed89c916` | W-1: the three at-NAV pricing rules in consensus, only from the governed `asset_orchard_pricing_enforcement_activation_height` (absent; byte-identical before it, tested). | yes, inert |
| `899b26f8` | W-1: the governed egress policy registry; refuses nothing until a record or `asset_orchard_egress_policy_activation_height` exists (absent; byte-identical receipts, tested). | yes, inert |
| `ed89c916`, `899b26f8` (`crates/types`) | The two activation kinds join the Cobalt amendment whitelist; no such amendment is ratified. | yes, unused |
| `683a5b32`, `0c396fc8`, `93454729`, `556cac38` | Public finalized reads: `private_pool_snapshot`/`page`/`account_lines`/`receipts`/`market`, `asset_orchard_pricing_claim`, `asset_orchard_private_egress_binding`, `private_readiness`. | yes (read path) |
| `110997f1` | The node keeps the last 16 finalized snapshot points in `<data-dir>/private_pool_snapshot_points.json`, rewritten at every block commit; a failed write never blocks the commit (commit path, not consensus or state). | yes |
| `346c79e4` | Private actions contract types (`crates/types`) and fixtures. | types only |
| `e37d77fd`, `e5f8f5b8`, `14c8e43e` | Six-validator test modules and the gateway integration tests. | no (tests) |
| `548d4918`, `d23b14d1` | Asset-Orchard seeds and openings wiped on drop, redacted `Debug`, serde bytes proven identical; proof public-input inventory hashes updated. | yes |
| `6f7aae33` | The unused `k256` dependency removed from the node (`Cargo.lock` −2 lines). | yes |
| `ae70d66d` | The transport ack timeout fix (parallel node tests no longer clear each other's pooled vote streams or take the readiness test's RPC port). | tests |
| the rest | Tooling, contracts not built into the node (`crates/pft-token-contracts`, the Task Node validator binding tool under `tools/`), wallet-proxy, Python, docs. | no |

**Inert on the devnet.** The W-1 rules change a consensus or state result only after a governed activation
that is not ratified or scheduled; this cut proves them inert now (signed 1153 import: no W-1 amendment,
`private_readiness` not ready with reasons, private action methods `rpc_method_not_allowed`; local devnet block
applied by both executables with the same state root). Their activation on the devnet gets its own
qualification later (plan rule).

## Generated units

After the release-ID change the 33 generated files keep the deployed shape: RPC units keep
`--max-requests 100000` and `RestartSec=1`, the 12 environment files keep `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`,
and no unit passes `--allow-private-action-admission` ([packet](qualification/README.md#generated-units)).

## Contents

- [DEPLOY-SHEET.md](DEPLOY-SHEET.md): the release-day sequence, go points and rollback.
- `rootfs/etc/`: the generated files (12 units, 12 environment files, six runtime bindings, topology, both circuit metadata files).
- [deployment-manifest.signed.json](deployment-manifest.signed.json): the signed manifest (`ee1df5b3…`); [manifest-input.unsigned.json](manifest-input.unsigned.json): its reviewed inputs.
- `stage-report.template.json`, `validator-bindings.signing.template.json`: the generated stage files with the local prefix replaced by `@STAGE@`.
- [node-builds.json](node-builds.json): both builds, toolchain and arguments.
- [local-preflight.py](local-preflight.py), [observe-fleet.py](observe-fleet.py), [demo-preflight.py](demo-preflight.py), [rollback-one.sh](rollback-one.sh): adapted from `index-v3-20261027`; `rollback-one.sh` returns one validator to `index-v3-20261027`.
- [read-import-state.py](read-import-state.py): local read of a signed canary import: the three activation heights, the two W-1 activation kinds and both routes' `authority_epoch`.
- [inventory.txt](inventory.txt): the unchanged rollout inventory.
- [qualification/](qualification/README.md): the local qualification packet.
