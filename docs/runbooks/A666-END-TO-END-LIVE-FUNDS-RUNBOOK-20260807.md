> **REVIEW-ONLY RUNBOOK — NO COMMANDS HAVE BEEN EXECUTED FROM THIS DOCUMENT. Every command below requires operator review and explicit execution authorization. STOP-no-retry discipline applies to every live mutation.**

# A666 End-to-End Live Funds Runbook

## 1. Architecture

PFTL1V2 is the workflow and economic source of truth: it records operations, receipts, supply caps, and finality. Ethereum mainnet is the EVM and Uniswap source of truth for vault deposits, wA666/USDC swaps, and bridge-out. StakeHub is custody, signing, and funds only. It holds keys and signs leaf requests; it makes no workflow decisions and has no control-plane authority. No StakeHub control-plane calls appear anywhere in this runbook.

## 2. Current verified state (h778 checkpoint)

Unit convention: every PFTL and EVM token quantity in this runbook is stated in 6-decimal atomic units (atoms) unless explicitly suffixed with a whole-token unit. Acceptance reads compare against atom values exactly.

- PFTL propose is h777 and finalize is h778. Finalized deposit record `e54713583c1bb46e908e8f01f1c996966dc5c82281365c91092a27ae0852d02b`, transaction `ca562096…`, status `finalized`, expires at h1776.
- Fleet is 6/6 at h778 with state root `b287451679a9d4d9`. The claim has not been submitted. No mutations have occurred since h778.
- Ethereum deposit is mined: `0x016f9c5f9b99fc951cea7c539f7f791c2d753d35793296b2e284f96512575924`, block 25698310, 10,000,000 USDC atoms (10.000000 USDC) to vault `0xaaa78fda7062efce769e95cd72fc55e507bc8183`.
- Wallet USDC is 74,161,443 atoms (74.161443 USDC) and nonce is 304.
- Holder `pfab9b9228942e5c529633a13aa271d5297bec6353` has pfUSDC balance 1,358,493 atoms (1.358493 pfUSDC).
- Mainnet route backing is deposits_verified 422,210,781 atoms (422.210781 pfUSDC) minus claims_minted 412,210,781 atoms (412.210781 pfUSDC), leaving exactly 10,000,000 atoms (10.000000 pfUSDC) finalized unclaimed backing.
- NAV cap is 287,859,297 atoms (287.859297 pfUSDC). The target is 297,859,297 atoms (297.859297 pfUSDC) at epoch 45.

## 3. Authoritative artifact hashes

| Artifact | SHA-256 / location |
|---|---|
| Fleet release commit | `540b2c1c739affd0f33da0be9fd5f9a92c3c8673`, `/home/postfiat/repos/a666-orchard-fix-2246d25` |
| Feature lineage, not for fleet | `16621fa94a4a29c637574180800777fa4ed0e1b5` |
| Fleet orchard-fix binary | `25e607595e581e7d435c6282f2db95aa473cf61877a7d75cea73505ea697c7f4`, `/tmp/fire-20260806-bin/postfiat-node-2246d25-orchardfix` |
| Finality submitter | `a29f19e9b67cabc43ed2a9140efdf1aa139f92259881a2311bf9a04428cfe315`, `scripts/native_rpc_finality_submit.py` |
| Binding | `df8d2e35bd62a74b294a1cfbf40423283fbb670b236d1b5e40094a5149e6b901`, `binding-S1.json` |
| Values | `2a0d9574c682180a54ccb1f5a158c4befd4b2a9f3f4ae7fc354033401eaa8cf2`, `values-S1.json` |
| Resolution rules | `04c6f1c69f27194ef24f774dd52b1b6247fdaf5a515d65b86acb6be3bc7b1ecd`, `resolution-rules.md` |
| Authorization | `90709e21d62fc226e9d0b533ff1f4f9bf01bc751645c2f4584b0e2b08901a91d`, `authorization-native-fire-20260806.json` |

Packet SHA-256 values, computed from `packets-S1/*.json`:

| Packet | SHA-256 |
|---|---|
| native-leg0-proxy-verify.json | `2df8068c0b9e6db360eb828959314467bfa1d34763f7eda8f1a11339c2bb8ced` |
| native-leg1-bridge-in.json | `781196d440a24486200cd39f9c5d7897a15839889cdc5f02fe45b7bdbb4177c9` |
| native-leg2a-order-reserve.json | `fc356139e9950bd6ff3056a03bdc3351271ec6c24a2c60e4676928b886671714` |
| native-leg2b-primary-subscribe.json | `ef88d977d02c874df4bcbca1879ab595265085cd26e40a7fce8b0ad20163bf42` |
| native-leg3a-export-debit.json | `b3271f2fc7da6c91a67744f736d3e3ef926a40beabd260a9929df6ba2e76bf3a` |
| native-leg3b-accept-mint.json | `14b0a36cd1e4768608fb4a0a97d3d9fc25cc7722be2a74bdb0c44715d15f848b` |
| native-leg3b0-signer-funding.json | `a3447f98eb15d255df3622cb1912ffdbbca7c719b8316b6c9d260617e14de064` |
| native-leg3c-approve-wa666-permit2.json | `af67f1b1d89a25f790508e77b15321c019bf7899cdc3e2b7f936960ae48f7c1a` |
| native-leg3d-permit2-wa666-router.json | `e80e8092958824ba53074a5107a117acad55491010be2438d9c4069d0a1f85a3` |
| native-leg3e-swap-wa666-usdc.json | `f171c982c1a37a436ff9989809cf53d23b84d45a2a7d1095e5fcdd41f6c8e50f` |
| native-leg3f-approve-usdc-permit2.json | `cc99af4f4eea906aaba05d0eb0ea67eb82ac5740404402d40898c8a5f1c81cbe` |
| native-leg3g-permit2-usdc-router.json | `3251954991274136d678ed946a3c7fd431d30a80c5dc65a80dfcd2d53595586c` |
| native-leg3h-swap-usdc-wa666.json | `4c9a598f27dbd3818f8afb9dd43a5c95d666689d827b385bea270caf213656d9` |
| native-leg4-return-burn-import.json | `46ac60b2d0fca313fa333425a8788697adad2abb94c2800e66bbe77664391c9c` |
| native-leg5a-primary-redeem.json | `c3affa77469799f2094346927b3933a44b97efded4aab027e60ed271ddb343e9` |
| native-leg5b-bridge-out.json | `5dc9fa5425c86d9904dfda3b4ebbd50291eba4af19464644bcb254a023d44185` |

## 4. Prerequisites and read-only preflight

Credential locations only, never values:

- Deployment publisher: `/home/postfiat/.postfiat/deployment-bfinal-1a8c0cb6.private.json`
- Snapshot publisher: `/home/postfiat/.postfiat/recovery-v3-snapshot-publisher.private.json`
- Fleet inventory: `/home/postfiat/repos/wan-vultr-all-fleet.txt`
- Vultr key: `/home/postfiat/repos/vultr.txt`
- Issuer and holder key locations: `/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/{pfusdc-issuer-key.json,holder-key.json}`

Fleet hosts: v0 `64.176.220.75`, v1 `95.179.184.122`, v2 `66.42.48.39`, v3 `149.28.63.106`, v4 `95.179.179.206`, v5 `45.32.110.170`. Forwarded RPC ports are v0 `39660`, v1..v5 `39651..39655`; remote ports are `27650..27655`. Rollout tunnels use free ports `39661..39666`. Preflight is read-only status and balance reads plus packet SHA-256 verification.

## 5. Stage 0: orchard-fix rolling upgrade

The bound S-UPGRADE commands are in `packets-S1/native-leg1-bridge-in.json`: cmd00-cmd04, `rollout_commands` preflight/backup/apply-next for all six validators, and `post_rollout_gates`. Resolve them through the packet resolution rules. Do not re-author commands.

Post-gates: each validator's remote `/opt/postfiat/releases/pnok-private-fix-2246d25-orchard1/postfiat-node` hashes to `25e60759…`, units are active, all six report the same advancing height and state root, and height advances after observation. This is a separate HELD packet boundary.
## 6. Stage 1: finalized deposit claim

Packet: `native-leg1-bridge-in.json`, S-CLAIM. Resolve the bound claim command and its S-UPGRADE dependencies through `resolution-rules.md`; do not copy unresolved fields into a command. Objective: claim exactly 10,000,000 pfUSDC atoms (10.000000 pfUSDC) to holder `pfab9b9228942e5c529633a13aa271d5297bec6353`. HELD boundary: claim is its own packet and must finish before any later packet is opened. STOP-no-retry on any mismatch, unresolved field, replay, or finality failure. Acceptance: holder balance becomes 11,358,493 atoms (11.358493 pfUSDC), cap becomes 297,859,297 atoms (297.859297 pfUSDC) at epoch 45, route claims_minted becomes 422,210,781 atoms (422.210781 pfUSDC), global supply equals cap, and all six validators report identical height and root. Evidence follows the packet's resolved `native-v1/leg1/` artifact convention.

## 7. Stage 2: pfUSDC to A666 subscription

Packets: `native-leg2a-order-reserve.json`, `native-leg2b-primary-subscribe.json`. Resolve each certified-ops command and submit as separate held packets, with the reservation receipt chaining into subscribe. Objective: debit exactly 10,000,000 pfUSDC atoms (10.000000 pfUSDC) and mint exactly 11,027,135 A666 atoms (11.027135 A666) to the bound subscriber. HELD boundary: order-reserve finality is required before subscribe. STOP-no-retry on stale epoch, packet hash, reservation, amount, signer, or replay. Acceptance: each PFTL receipt is accepted and finalized, certificate quorum passes, and six validators converge on height/root. Evidence uses `native-v1/leg2a/` and `native-v1/leg2b/` resolver-selected artifact directories.

## 8. Stage 3: export to wA666

Packets: `native-leg3a-export-debit.json`, `native-leg3b-accept-mint.json`, and `native-leg3b0-signer-funding.json`. Resolve the export certified operation, receipt-witness command, accept-mint leaf, and signer-funding held packet from packet rules. Objective: debit the PFTL A666 balance and import the exact receipt-chained amount to Ethereum. HELD boundary: signer funding is separate and must be finalized before any constrained signing leaf. STOP-no-retry on any unresolved receipt, wrong packet digest, signer mismatch, or replay. Acceptance: PFTL operations are accepted/finalized with certificate quorum and six-way convergence; EVM receipts are mined with status 1 and exact balance deltas. Evidence uses `native-v1/leg3a/`, `leg3b/`, and `leg3b0/` resolver-selected directories.

## 9. Stage 4: forward wA666 to USDC

Packets: `native-leg3c-approve-wa666-permit2.json`, `native-leg3d-permit2-wa666-router.json`, `native-leg3e-swap-wa666-usdc.json`. Resolve each EVM leaf command from the packet. Objective: approve Permit2, authorize the router, and swap the exact receipt-chained wA666 amount forward. HELD boundary: each EVM mutation is independently held; no next command follows a failed or ambiguous receipt. STOP-no-retry on status other than 1, wrong recipient/value/calldata, allowance mismatch, or cap breach. Acceptance: every receipt is mined status 1, expected allowance is present, and USDC delta equals the packet's resolved quote and minimum-output rule. Evidence uses `native-v1/leg3c/`, `leg3d/`, and `leg3e/`.

## 10. Stage 5: reverse USDC to wA666

Packets: `native-leg3f-approve-usdc-permit2.json`, `native-leg3g-permit2-usdc-router.json`, `native-leg3h-swap-usdc-wa666.json`. Resolve the packet-bound approvals, router call, and receipt-chained amount. Objective: restore the wA666 position through the bound Uniswap pool. HELD boundary: each EVM transaction is a distinct held mutation. STOP-no-retry on receipt failure, wrong calldata, output below minimum, or fee projection above the cap. Acceptance: mined status 1 and exact USDC/wA666 balance deltas satisfy packet minimums. Evidence uses `native-v1/leg3f/`, `leg3g/`, and `leg3h/`.

## 11. Stage 6: return import wA666 to PFTL

Packet: `native-leg4-return-burn-import.json`. This is two phases: the resolved constrained-signer burn, then certified `pftl_uniswap_return_import` with the phase-1 burn transaction hash and actual leg3h output amount. HELD boundary: phase 1 must finalize before phase 2 is rendered. STOP-no-retry on any burn receipt mismatch, missing phase report, wrong amount, wrong recipient, or replay. Acceptance: EVM burn status is 1 with exact sender/recipient/value, then PFTL import is accepted/finalized with certificate quorum and six-way convergence. Evidence uses `native-v1/leg4/`.

## 12. Stage 7: A666 redemption to pfUSDC

Packet: `native-leg5a-primary-redeem.json`. Resolve the primary redeem certified operation from the packet, using the actual leg4 imported A666 delta, route epoch 6, and the packet-pinned E5 pricing reserve packet. HELD boundary: redemption is separate from external bridge-out. STOP-no-retry on wrong subscriber, stale NAV packet, amount mismatch, replay, or finality failure. Acceptance: PFTL receipt accepted/finalized, certificate quorum passes, six validators converge, and holder pfUSDC delta equals the resolved payout. Evidence uses `native-v1/leg5a/`.

## 13. Stage 8: external USDC bridge-out

Packet: `native-leg5b-bridge-out.json`. Resolve the three sequential stages: burn-to-redeem, EVM withdrawal, and PFTL settlement. Objective: return the resolved pfUSDC payout as external USDC to the bound Ethereum destination. HELD boundary: all three phases are one held packet but each receipt gate is mandatory. STOP-no-retry on challenge/report mismatch, non-final EVM receipt, verifier failure, wrong destination, or settlement divergence. Acceptance: PFTL burn and settle receipts are accepted/finalized with six-way convergence; EVM withdrawal is mined status 1 with exact USDC delta to wallet. Evidence uses `native-v1/leg5b/`.

## 14. Conservation and caps

Protected baseline: 103,000,000 wA666 atoms (103.000000 wA666), isolated and never touched. Wallet USDC baseline was 84,161,443 atoms (84.161443 USDC); after the 10,000,000-atom deposit (10.000000 USDC) it is 74,161,443 atoms (74.161443 USDC). The 530 USDC cap is cumulative campaign-wide: prior spend 501.024845 + mined 10-USDC leg-1 principal 10.000000 = 511.024845 <= 530.000000, leaving 18.975155 headroom before fresh gas. Every subsequent gas quote updates prior_actual and must keep projected cumulative spend <= 530.000000. GPU cap is $10 total, approximately $4.55 already spent; no further GPU is authorized because the reserve-packet proving path is abandoned.

Arithmetic:

- Claim: 1,358,493 atoms (1.358493 pfUSDC) + 10,000,000 atoms (10.000000 pfUSDC) = 11,358,493 atoms (11.358493 pfUSDC).
- NAV cap: 287,859,297 atoms (287.859297 pfUSDC) + 10,000,000 atoms (10.000000 pfUSDC) = 297,859,297 atoms (297.859297 pfUSDC).
- Route backing: 422,210,781 atoms (422.210781 pfUSDC) - 412,210,781 atoms (412.210781 pfUSDC) = 10,000,000 atoms (10.000000 pfUSDC).

| Leg | Pre-state | Debit | Credit | Fee | Post-state | Evidence |
|---|---:|---:|---:|---:|---:|---|
| 1 claim | | 10,000,000 USDC atoms (10.000000 USDC) | 10,000,000 pfUSDC atoms (10.000000 pfUSDC) | | | |
| 2 subscription | | 10,000,000 pfUSDC atoms (10.000000 pfUSDC) | 11,027,135 A666 atoms (11.027135 A666) | | | |
| 3 export | | 11,027,135 A666 atoms (11.027135 A666) | EVM A666 | | | |
| 4 forward swap | | A666 | USDC | gas | | |
| 5 reverse swap | | USDC | A666 | gas | | |
| 6 return import | | A666 | PFTL A666 | | | |
| 7 redeem | | A666 | pfUSDC | | | |
| 8 bridge-out | | pfUSDC | external USDC | gas | | |

## 15. Idempotency and recovery

Before every batch-only submission, refresh a read-only clone with `rsync -a --delete --exclude validator_keys.json` from validator-1 into the designated clone directory, then enforce the packet height gate. `--resume` may read only packet artifacts and verified receipts. `round_ok` is audit-only; `finality.json` and accepted receipt evidence are authoritative. STOP-no-retry means halt, preserve every artifact, and escalate. Never retry a failed live mutation without a new held packet. Verify an existing receipt before considering any resubmission.

## 16. Rollback boundaries

The safe-rollout pre-rollout backup is the only full-fleet rollback point. After the PFTL claim there is no rollback, only forward recovery through a newly authorized packet. Ethereum transactions are irreversible once mined.

## 17. Final acceptance checklist

- [ ] Stage 0 orchard-fix SHA, active units, advancing height, and 6/6 convergence verified.
- [ ] Claim holder balance is 11,358,493 atoms (11.358493 pfUSDC); cap is 297,859,297 atoms (297.859297 pfUSDC) at epoch 45.
- [ ] Every PFTL receipt is accepted/finalized with certificate quorum and six-way convergence.
- [ ] Every EVM receipt is mined status 1 with exact balance deltas.
- [ ] Subscription mints exactly 11,027,135 A666 atoms (11.027135 A666).
- [ ] Forward and reverse swap minimum-output rules pass.
- [ ] Return import and redemption deltas reconcile.
- [ ] External bridge-out destination and USDC delta reconcile.
- [ ] Conservation table is complete; cumulative spend arithmetic holds: 501.024845 + 10.000000 = 511.024845 <= 530.000000, remaining headroom 18.975155, and every gas quote kept projected cumulative spend <= 530.000000.
- [ ] No packet hash, route, epoch, signer, recipient, or amount mismatch occurred.
- [ ] All evidence paths are populated and replay-safe.

## 18. Open limitations and deferred fixes

The same orchard-undercount pattern remains deferred at `crates/execution/src/nav_vault_asset_execution.rs:2949` (mint-from-receipts) and `:7506` (reserve-packet bound). Overflow and missing-asset unit tests remain deferred. Residual legacy mempool call sites fail closed only. A full `verify-blocks` pass is unavailable on current binaries because of pre-existing block-483 receipt-ID drift from ancient pre-2246d257 history. The acceptance gate is byte-identical block-483 evidence, state verification, regression coverage, and the live claim proof.

## 19. Appendix: path classification

| Path | Classification |
|---|---|
| `/home/postfiat/repos/a666-eth-fast-lane-combined-20260724` | [EXISTS] |
| `/home/postfiat/repos/a666-orchard-fix-2246d25` | [EXISTS] |
| `/tmp/fire-20260806-bin/postfiat-node-2246d25-orchardfix` | [EXISTS] |
| `docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/fire-20260806/` | [EXISTS] |
| `/home/postfiat/.postfiat/deployment-bfinal-1a8c0cb6.private.json` | [EXISTS] |
| `/home/postfiat/.postfiat/recovery-v3-snapshot-publisher.private.json` | [EXISTS] |
| `/home/postfiat/repos/wan-vultr-all-fleet.txt` | [EXISTS] |
| `/home/postfiat/repos/vultr.txt` | [EXISTS] |
| `/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/pfusdc-issuer-key.json` | [EXISTS] |
| `/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/holder-key.json` | [EXISTS] |
| `/run/user/1000/postfiat-constrained-signer/a666-signer.sock` | [EXISTS] |
| `native-v1/leg*/` artifact directories | [FIRE-TIME GENERATED] |
| `/tmp/krimp-exec-fire20260806/` | [EXISTS] |
