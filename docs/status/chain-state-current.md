# PostFiat L1 Current State

Updated: `2026-10-27T12:14:30Z` (release `index-v3-20261027` deployed to all six, applied at blocks 1154–1159; account_tx v3 index built on all six to 1159 and following commits; vote outcome records partly live, defect VR-01; tip 1160); latest fleet observation `2026-10-27T12:14Z`

Status: **canonical operational-state reference**

!!! note "2026-10-30: vote-records candidate cut, not deployed"

    On 2026-10-30 the next candidate, `vote-records-20261030` (`main` at `ae70d66d`, executable
    `87cfe797…`), which carries the VR-01 fix of the proposer's vote outcome records, the wallet app's private
    path switched off or ungoverned on the devnet, the Asset-Orchard secret wiping and the transport ack timeout
    fix, was cut and qualified locally; it is cut, not deployed
    ([release inputs](https://github.com/postfiatorg/postfiatl1v2/blob/release/vote-records-20261030/deployments/vote-records-20261030/README.md)).
    It has no governance step and no index build; the rollout starts only when CI on `ae70d66d` is green in
    all three workflows.

!!! success "2026-10-27: index-v3 release deployed to all six validators"

    Release `index-v3-20261027` runs on all six validators: source `d728f895` on branch
    `release/index-v3-20261027` (`main` `d728f895`), executable
    `b5cf3ce4bc046e6338beca6d5ead6fd0343426ddb01ac2f461c31e2775f9c8eb`, signed manifest `a61e290a…`
    (publisher `pfc531e0…`). It is `exchange-readiness-20261023` plus the account history index v3 that follows
    commits, the certified round's local apply and the proposer's vote outcome records; no consensus, execution or
    state-commitment change and no governance step.

    Before (`10:42Z`): six on `exchange-readiness-20261023` at 1153 / `db8b87b6…` / `7141956e…`, mempools empty;
    signed validator-1 canary backup `03e5c61f…` at 1153, re-imported and replayed, and its current-head exact
    replay through the new executable PASS (receipt `d3d06a9a…`). Rollout with `scripts/postfiat-safe-rollout
    apply-next` one validator at a time (validator-1, 0, 2, 3, 4, 5), 11:05–11:35Z, one 1-PFT faucet grant to the
    `testing` wallet after each at 1154–1159, each certified at view 0 with 5 of 6 votes and the same block, tip and
    root on all six; rollback was not needed. After checks: demo preflight READY (60 PASS), manifest verified and
    build `d728f895` on all six, activation heights still 1153, both routes `authority_epoch` 1, swap service
    committee epoch 2.

    `account-tx-index-build` ran on each host with only its RPC unit stopped (11:38–11:41Z, 7.1–9.8 s per host):
    v3 index usable on all six to 1159, and the next block (1160) extended it on all six (lag 0). The proposer's
    vote outcome records are partly live: validator-2's record for 1160 exists but lists validator-0 as
    `no_answer` although validator-0 accepted the votes, and validator-0 (1158) and validator-1 (1159) wrote no
    record. This is the known defect VR-01, accepted and planned for the next release; consensus never reads the
    record. After (`12:14Z`): six on `index-v3-20261027` at 1160 / `f65e6fac…` / `f06608d4…`, mempools empty.
    Records: [release README](https://github.com/postfiatorg/postfiatl1v2/blob/release/index-v3-20261027/deployments/index-v3-20261027/README.md),
    [deploy sheet](https://github.com/postfiatorg/postfiatl1v2/blob/release/index-v3-20261027/deployments/index-v3-20261027/DEPLOY-SHEET.md).

!!! success "2026-10-23: exchange-readiness release deployed to all six validators"

    Release `exchange-readiness-20261023` runs on all six validators: source `6b1a0585` on branch
    `release/exchange-readiness-20261023` (`main` `d9260e93`), executable
    `4085ac70c7bb3c21b46cc67be8cb1bbb3556bfea9f49fbf6b128cdac2adcbef9`, signed manifest `210544a8…`
    (publisher `pfc531e0…`). It is `store-handle-20261021` plus #138–#144, #49, their integration fixes
    and the account history index fix: exchange history (memo contents, shielded withdrawals),
    malformed recipients refused at RPC and mempool, and the governance-gated recipient-format,
    rejected-fee and require-memo rules. It replaced `store-handle-20261021` (`eea96ba6…`) with
    `scripts/postfiat-safe-rollout apply-next`, one validator at a time (validator-1 canary, then 0, 2,
    3, 4, 5), 07:10:57–07:37:03Z.

    Before (`06:31Z`): six on `store-handle-20261021` at height 1137, tip `18a4dc86…`, root
    `1277c76c…`, mempools empty. Signed canary backup from validator-1 at 1137: manifest `3257115c…`,
    re-imported, checkpoint verified, fully replayed to the same root, and the current-head exact replay
    (`run_replay_evidence.py --signed-archive-dir`) PASS. After (`07:40Z`): all 12 validator and RPC
    processes on `4085ac70…`, `deployment_manifest_verified=true` and build `6b1a0585` on every host,
    RPC units with `--max-requests 100000` and `RestartSec=1`, height 1143, tip `7fef75aa…`, root
    `8c3a32fd…`; `demo-preflight.py` READY (60 PASS). No divergence at any step.

    Grants used: six devnet faucet grants of 1 PFT from the faucet account to the `testing` wallet
    (StakeHub `pft faucet`), one after each apply, at heights 1138–1143 (proposers validator-4, 5, 0,
    1, 2, 3). Each certified at view 0 with quorum 5 of 6 and the same block, tip and root on all six
    (`blocks` RPC). The three governed activation heights were unset at the rollout; malformed recipients
    are refused at RPC and mempool by the binary itself. The heights are set by governance below.

    Account history index (sheet §7, 08:02–08:23Z): `account-tx-index-build` ran on all six, one host
    at a time (validator-1 first, then 0, 2, 3, 4, 5). Only the host's RPC unit was stopped, for 11–31 s;
    the validator units stayed active. The build took 7–15 s per host and produced a usable v2 index on
    every host: range 1–1143, 1,105 blocks, 970 rows. `account_tx` for `testing` answers from the index
    with today's grants. Six-way agreement at 1143 held before and after every host. Ordered commit does
    not extend the index, so after the next block reads fall back to the archive scan until it is rebuilt
    ([records](https://github.com/postfiatorg/postfiatl1v2/tree/release/exchange-readiness-20261023/deployments/exchange-readiness-20261023/observed/account-tx-index)).

    Governed activation (sheet §8 and §8b, 08:38–10:34Z): each of the three rules was set by one
    governance amendment with value **1153**, signed by the six validators with their key files.

    - `rejected_tx_fee_activation_height`: block 1144, amendment `e4745af7…`, batch `3c2a18d7…`.
    - `require_memo_activation_height`: block 1145, view 0, proposer validator-5, amendment `cdf4c13c…`.
    - `recipient_format_activation_height`: block 1146, view 0, proposer validator-0, amendment `a3771fed…`.
      It was submitted last and cannot be undone.

    Block 1144 needed a view change. The view-0 round stalled at 08:40Z: proposer validator-4 timed out
    reading validator-5's commit-phase reply, and a same-round retry was refused by the vote-safety rule.
    The six validators then signed timeout votes, and the timeout certificate (6 of 6, quorum 5) moved
    the round to view 1. There, proposer validator-5 re-proposed the locked block, `cee96283…`, root
    `34ede112…`. Validators 0–4 committed it at 09:34Z.

    Validator-5 itself did not apply the block, because the round's send to its own service was reset.
    Its round held the store lease, and the transport service could not open the store. That
    validator-5 send happens only without `--local-apply-before-certified-send`, which the 1145 and 1146
    rounds then used. At 09:46Z the same certified block was re-sent from validator-4, and validator-5
    applied it. A read-only health check found validator-5 normal, comparable to validator-4.

    Seven 1 PFT faucet grants to `testing` then brought the chain to 1153 (blocks 1147–1153), each
    certified identically on six. All three rules apply from block 1153.

    Probes at 1153:

    - recipient format: through validator-1's RPC, a 41-character recipient is refused with
      `invalid_recipient_address` and the canonical address passes;
    - fee claim and require-memo: read-only confirmations. The mempool dry-run refuses failing transfers,
      and no account sets the flag.

    After state (`10:30Z`): six on `4085ac70…` at 1153, tip `db8b87b6…`, root `7141956e…`, mempools
    empty; `demo-preflight.py` READY. Validator-1's account_tx index was rebuilt to cover 1–1153, with its
    RPC unit stopped for 11 s. Between 10:51 and 11:07Z validators 0, 2, 3, 4 and 5 were rebuilt the same
    way, so all six serve `account_tx` from a usable v2 index for 1–1153. Each RPC unit was stopped for
    11–27 s, with six-way agreement at 1153 before and after every host
    ([records](https://github.com/postfiatorg/postfiatl1v2/tree/release/exchange-readiness-20261023/deployments/exchange-readiness-20261023/observed/account-tx-index/after-activation)).
    Postponing a rule needs a later amendment of the same kind with a far height, and only works for
    the fee-claim and require-memo rules
    ([records](https://github.com/postfiatorg/postfiatl1v2/tree/release/exchange-readiness-20261023/deployments/exchange-readiness-20261023/observed/activation)).

    Rollback: the `store-handle-20261021` executable and release directory stay on every host;
    `rollback-one.sh` returns one validator to it with the data in place (before any index build on
    that host; after one, the old executable ignores the v2 index). No activation record may be
    submitted until all six run this release, and no validator may roll back after one is committed.
    Records: [deployment README](https://github.com/postfiatorg/postfiatl1v2/blob/release/exchange-readiness-20261023/deployments/exchange-readiness-20261023/README.md),
    [rollout record](https://github.com/postfiatorg/postfiatl1v2/blob/release/exchange-readiness-20261023/deployments/exchange-readiness-20261023/observed/rollout-record.json),
    [after checks](https://github.com/postfiatorg/postfiatl1v2/blob/release/exchange-readiness-20261023/deployments/exchange-readiness-20261023/observed/after-checks.json).

!!! success "2026-10-21: store-handle release deployed to all six validators"

    Release `store-handle-20261021` runs on all six validators: source
    `86706699` on branch `release/store-handle-20261021`, executable
    `eea96ba6761ed223047d7781663100d34efe5641d36031bd50874bd9c8b41f36`,
    signed manifest `8ce1da30…` (publisher `pfc531e0…`). It is
    `fastlane-cap-20261014` plus the STO-07 store-handle fix (`8644f0af`: the
    transactional store stays open for the life of a consensus operation), the
    offline `fastswap-committee-migrate` command (`2737f080`, run below) and
    the reviewed contributor merges. It replaced `fastlane-cap-20261014`
    (`b1b3dbb1…`) with `scripts/postfiat-safe-rollout apply-next`, one validator
    at a time (validator-1 canary, then 0, 2, 3, 4, 5), 11:15:17–11:38:16Z.

    Before (`10:57:52Z`): six on `fastlane-cap-20261014` at height 1131, tip
    `9b054f09…`, root `a12e7c5f…`, mempools empty. Signed canary backup from
    validator-1 at 1131: manifest `df9b3847…`, re-imported, checkpoint verified
    and fully replayed to the same root. After (`11:43:17Z`): all 12 validator
    and RPC processes on `eea96ba6…`, `deployment_manifest_verified=true` and
    build `86706699` on every host, RPC units with `--max-requests 100000` and
    `RestartSec=1`, height 1137, tip `18a4dc86…`, root `1277c76c…`;
    `demo-preflight.py` READY (60 PASS). No divergence at any step. The fix is
    visible in the transport event logs: every batch the new executable
    accepted reports `storage_work.database_opens=0`.

    Grants used: six devnet faucet grants of 1 PFT from the faucet account to
    the `testing` wallet (StakeHub `pft faucet`), one after each apply, at
    heights 1132–1137 (proposers validator-4, 5, 0, 1, 2, 3). Each certified at
    view 0 with quorum 5 of 6 and the same block, tip and root on all six
    (`blocks` RPC). Total: 6 PFT plus 192 atoms of fees. Nothing else moved:
    the A666 and BMNRC routes keep `authority_epoch` 1 and their hashes
    (`57da3837…`, `b2817e68…`); neither issuer update has been submitted, and
    the signing sheet's height-1131 preconditions no longer hold.

    Rollback: the `fastlane-cap-20261014` executable and release directory stay
    on every host. The per-validator path is `rollback-one.sh` (data in place,
    verified by that executable first); the signed validator-1 backup at 1131 is
    the fallback. Rollback was not needed. Records:
    [deployment README](https://github.com/postfiatorg/postfiatl1v2/blob/release/store-handle-20261021/deployments/store-handle-20261021/README.md),
    [rollout record](https://github.com/postfiatorg/postfiatl1v2/blob/release/store-handle-20261021/deployments/store-handle-20261021/observed/rollout-record.json).

    FastSwap committee migration (sheet §7), 12:07–12:27Z: this release's
    `fastswap-committee-migrate` moved each `fastswap-v1/` store from committee
    epoch 1 to the activated epoch 2, one host at a time (validator-5, then
    0–4; both units stopped, dry run, real run with a verified backup, units
    started). All six validators now run the swap service at committee epoch 2,
    root `72836827…`, `enabled: true`; `fastswap_checkpoint_status` answers.
    validator-5 was not refused: its store never applied the epoch-1 fence or
    anchor, but its replayed state equals the anchored drained state
    (`355fe608…`), as its report notes; its service, unavailable before, now
    opens. The fleet stayed at 1137 / `18a4dc86…` / `1277c76c…` with six-way
    agreement after every host; `demo-preflight.py` READY (60 PASS). The routes
    keep `authority_epoch` 1, so Ethereum checkpoint signing is unchanged.
    Backups: `/var/lib/postfiat/fastswap-v1-epoch2-backup-validator-N`. Records:
    [sheet §7](https://github.com/postfiatorg/postfiatl1v2/blob/release/store-handle-20261021/deployments/store-handle-20261021/DEPLOY-SHEET.md#7-fastswap-committee-migration-one-host-at-a-time--covered-by-the-operators-go-of-the-day),
    [per-host reports](https://github.com/postfiatorg/postfiatl1v2/tree/release/store-handle-20261021/deployments/store-handle-20261021/observed/committee-migration).

!!! success "2026-10-15: fastlane-cap release deployed; bridge committee epoch 2 active"

    Release `fastlane-cap-20261014` runs on all six validators: source
    `8bed9b90` on branch `release/fastlane-cap-20261014`, executable
    `b1b3dbb150d3596577df3bb3fe145103d90988cf71558c4d34a035fe4bb44405`,
    signed manifest `4f379c57…` (publisher `pfc531e0…`). It is
    `signer-rotation-20261007` plus the 256 KiB `fastlane_primary_json` RPC cap
    with per-operation finality receipt checks (`7f9d0d98`), the rustls 0.23.46
    advisory fix (#123) and the reviewed contributor merges. It replaced
    `signer-rotation-20261007` (`decaa411…`) with
    `scripts/postfiat-safe-rollout apply-next`, one validator at a time
    (validator-1 canary, then 0, 2, 3, 4, 5), 10:38:19–11:03:35Z.

    Before (`10:19:51Z`): six on `signer-rotation-20261007` at height 1123,
    tip `bafcc6ef…`, root `942d1fc2…`, mempools empty. Signed canary backup from
    validator-1 at 1123: manifest `d957141e…`, re-imported, checkpoint verified
    and fully replayed to the same root. After (`11:07:27Z`): all 12 validator
    and RPC processes on `b1b3dbb1…`, `deployment_manifest_verified=true` on
    every host, RPC units with `--max-requests 100000` and `RestartSec=1`,
    height 1129, tip `41154d2c…`, root `053fb35c…`; `demo-preflight.py` READY
    (60 PASS). No divergence at any step.

    Grants used: six devnet faucet grants of 1 PFT from the faucet account to
    the `testing` wallet (StakeHub `pft faucet`), one after each apply, at
    heights 1124–1129 (proposers validator-2, 3, 4, 5, 0, 1). Each certified at
    view 0 with quorum 5 of 6 and the same block, tip and root on all six
    (`blocks` RPC). Total: 6 PFT plus 192 atoms of fees. Nothing else moved.

    Rollback: the `signer-rotation-20261007` executable and release directory
    stay on every host. The per-validator path is `rollback-one.sh` (data in
    place, verified by that executable first); the signed validator-1 backup at
    1123 is the fallback. Rollback was not needed. The 70,944 B anchor passes
    the release's request validation offline; it was admitted live in sheet §7
    (below). Records:
    [deployment README](https://github.com/postfiatorg/postfiatl1v2/blob/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/README.md),
    [rollout record](https://github.com/postfiatorg/postfiatl1v2/blob/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/observed/rollout-record.json).

    Bridge half of the signer-group activation (sheet §7, 11:25–11:46Z), with a
    state read before and after each step. Validators 0–4 gave fresh, identical
    drained epoch-1 checkpoint votes (policy epoch 1 fenced, no previous
    checkpoint). The `AnchorCheckpoint` (71,005 B, tx `f34dd1f5…`) was admitted
    at height 1130 through validator-2 (block `4dafdfc8…`). `ActivateCommittee`
    epoch 2 (root `72836827…`, six members with validator-5's current key,
    quorum 5; 176,546 B, tx `2b2efa37…`), signed by validators 0–4 on their own
    hosts, was admitted at 1131 through validator-3 (block `9b054f09…`). The
    receipts are `fastlane_checkpoint_anchored` and `fastlane_control_applied` on
    all six, and the RPC reported both as success. After (`11:45:37Z`): height
    1131, tip `9b054f09…`, root `a12e7c5f…`, mempools empty, six-way agreement.
    The A666 and BMNRC routes stay on `authority_epoch` 1 (root `a2eebcba…`),
    so validators 0–4 still sign Ethereum checkpoints. The A666 update to
    `authority_epoch` 2 is prepared unsigned (sha256 `24aa2e28…`) and awaits
    the other lane's signature; BMNRC needs the same update from its issuer.
    `fastswap_capabilities` still reports each validator's local FastSwap
    committee (epoch 1, `enabled: false`); the local committee was not
    switched. Records:
    [sheet §7](https://github.com/postfiatorg/postfiatl1v2/blob/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/DEPLOY-SHEET.md#7-signer-group-activation-bridge-half),
    [activation record](https://github.com/postfiatorg/postfiatl1v2/blob/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/observed/bridge-activation/bridge-activation-record.json),
    [unsigned update](https://github.com/postfiatorg/postfiatl1v2/blob/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/observed/a666-bridge-policy-update.unsigned.json).

    On 2026-10-21 the next release, `store-handle-20261021` (`main` at `86706699`,
    executable `eea96ba6…`), which carries the STO-07 store-handle fix and the offline
    `fastswap-committee-migrate`, was cut, qualified locally and deployed (2026-10-21
    entry above; [release inputs](https://github.com/postfiatorg/postfiatl1v2/blob/release/store-handle-20261021/deployments/store-handle-20261021/README.md)).

!!! success "2026-10-12: FastSwap store converted; FastPay committee epoch 2 installed; bridge committee unchanged"

    The July `fastswap-v1` store (26 records, 177,899 B) was converted with
    `fastswap-store-migrate` from release `signer-rotation-20261007`, one
    host at a time (validator-5, then 0–4), 10:57–11:14Z: both units
    stopped, dry run, conversion with a verified backup, a second dry run
    `nothing-to-convert`, both units started. Every host rejoined within 22 s,
    and the fleet stayed at height 1121, tip `329bc2f1…`, root `aa462feb…`
    with six-way agreement after each host. `fastswap_checkpoint_status`
    answers on validators 0–4 with the same checkpoint (committee epoch 1,
    root `a2eebcba…`, `drain_ready`, no epoch-1 checkpoint anchored yet).
    Validator-5 still answers `fastswap_unavailable`: its rotated key is not
    its epoch-1 committee key, which is the signer-group activation, still to
    do ([sheet §7](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/DEPLOY-SHEET.md#7-fastswap-store-conversion-one-host-at-a-time)).

    The signer-group activation ran at 11:40–11:52Z. **FastPay:** all six
    validators signed the epoch-2 committee at proposal slot 1123, validator-5
    with its current key; it was installed at height 1123 (batch `66746fa2…`,
    `fastpay_recovery_committee_rotated` on all six). The committee has six members
    with current keys, quorum 5, and orders 10001–19990. Epoch 1 serves through
    10000, so payments are unchanged today. **Bridge:** the epoch-1 `StopPrepare`
    for FastSwap policy epoch 1 committed at height 1122. The final drained
    epoch-1 checkpoint was then refused before admission: `mempool_submit_fastlane_primary_finality`
    caps `fastlane_primary_json` at 64 KiB, and the five-vote `AnchorCheckpoint`
    is 70,944 B. `ActivateCommittee` and the issuer's route update were not run.
    The bridge committee stays at epoch 1 (root `a2eebcba…`), so Ethereum
    checkpoints still need validators 0–4, and new epoch-1 FastSwap prepares are
    fenced. The epoch-2 root is `72836827…`. After the activation the fleet is at
    height 1123, root `942d1fc2…`, with six-way agreement
    ([sheet §8](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/DEPLOY-SHEET.md#8-signer-group-activation)).

    On 2026-10-15 the next release, `fastlane-cap-20261014` (`main` at `8bed9b90`,
    executable `b1b3dbb1…`), which admits the anchor and `ActivateCommittee`, was cut,
    qualified locally and deployed (2026-10-15 entry above;
    [release inputs](https://github.com/postfiatorg/postfiatl1v2/blob/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/README.md)).

!!! success "2026-10-09: signer-rotation release deployed to all six validators"

    Release `signer-rotation-20261007` runs on all six validators: source
    `0fa55d0b` on branch `release/signer-rotation-20261007`, executable
    `decaa411376a125fb377a29037b6dd470afb2208a268d30a3ce8130f571b370f`,
    signed manifest `fa4649aa…` (publisher `pfc531e0…`). It is
    `combined-fastpay-20260928` plus what the signer-group change needs (the
    issuer-signed bridge policy update, the offline FastSwap store conversion,
    the committee builders), the `account_tx` fixes and the RPC unit change.
    It replaced `combined-fastpay-20260928` (`1f8b332d…`) with
    `scripts/postfiat-safe-rollout apply-next`, one validator at a time
    (validator-1 canary, then 0, 2, 3, 4, 5), 12:06:38–12:28:03Z.

    Before (`10:51:42Z`): six on `combined-fastpay-20260928` at height 1115,
    tip `28e1175b…`, root `a5287eaf…`, mempools empty. Signed canary backup from validator-1 at 1115: manifest
    `49b2b6cd…`, re-imported, checkpoint verified and fully replayed to the
    same root. After (`12:32:09Z`): all 12 validator and RPC processes on
    `decaa411…`, `deployment_manifest_verified=true` on every host, RPC units
    with `--max-requests 100000` and `RestartSec=1`, height 1121, tip
    `329bc2f1…`, root `aa462feb…`. No divergence at any step.

    Grants used: six devnet faucet grants of 1 PFT from the faucet account to
    the `testing` wallet (StakeHub `pft faucet`), one after each apply, at
    heights 1116–1121 (proposers validator-0, 1, 2, 3, 4, 5). Each certified at
    view 0 with quorum 5 of 6 and the same block, tip and root on all six
    (`blocks` RPC). Total: 6 PFT plus 192 atoms of fees. Nothing else moved.

    Rollback: the `combined-fastpay-20260928` executable and release
    directory stay on every host. The per-validator path is `rollback-one.sh`
    (data in place, verified by that executable first); the signed validator-1
    backup at 1115 is the fallback. Rollback was not needed. Not done yet: the
    FastSwap store conversion and the signer-group activation (sheet §7–§8).
    Records:
    [deployment README](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/README.md),
    [rollout record](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/observed/rollout-record.json).

!!! success "2026-09-28: FastPay effect-anchoring release deployed to all six validators"

    Release `combined-fastpay-20260928` runs on all six validators. It is the
    deployed September 25 code plus the FastPay effect-anchoring fix, which
    lets a registered validator that cannot sign hold certificate-verified
    FastPay effects so that its own proposal anchors them. Source: `c93b2137` on
    branch `release/combined-fastpay-20260928`, qualified PASS including the
    full workspace suite (84 groups, 1,487 passed, 0 failed). Executable
    `1f8b332d9f482cdcf6ccf5cc15307ebd5d9bf0058b7a6db80a7132690d97e24a`,
    signed manifest `d2fdb687…` (publisher `pfc531e0…`). It replaced
    `combined-fastpay-20260925` (`d66cecc3…`) with
    `scripts/postfiat-safe-rollout apply-next`, one validator at a time
    (validator-1 canary, then 0, 2, 3, 4, 5), 07:40–08:05Z.

    Before: six on `combined-fastpay-20260925` at height 1050, tip
    `03a24230…`, root `13d9e652…`. After (`08:09:24Z`): six on the new
    release, all 12 validator and RPC processes on `1f8b332d…`, manifest
    verified on every host, height 1056, tip `30ebdccd…`, root `22576546…`.
    There was no divergence at any step.

    Grants used: six devnet faucet grants of 1 PFT from the faucet account to
    the `testing` wallet (StakeHub `pft faucet`), one per applied validator, at
    heights 1051–1056. All six certified at view 0. Total: 6 PFT plus 192 atoms
    of fees. Nothing else moved.

    Live fix check (08:28–08:38Z) **passed**: after a 0.001 PFT FastPay
    payment signed by validators 0–4, height 1061 certified at view 0 with
    validator-5 as proposer and the payment's effect anchored, where 1043
    needed view 1 before the fix, and all six agree at 1062 (tip `9d08fd3e…`,
    root `6324f86e…`).

    Rollback: the `combined-fastpay-20260925` executable and release
    directory stay on every host. The per-validator path is `rollback-one.sh`
    (data in place, verified by that executable first). The signed
    validator-1 backup at 1050 is the fallback. Rollback was not needed.
    Records:
    [deployment README](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-fastpay-20260928/deployments/combined-fastpay-20260928/README.md).

### Why `status` reports `deployment_manifest_verified=false`

On the deployed release `c93b2137` the field is always `false`, and this does
not contradict the rollout. `status` in `crates/node/src/lifecycle_queries.rs`
fills it from `deployment_runtime_identity_from_env`, which hashes the current
manifest file and checks the validator binding and the four runtime artifact
hashes (any mismatch makes `status` fail). In that build it never checks the
publisher signature or the validity window, so the field has been `false` since
`7095b393` (2026-09-16, finding SRV-03).

The signature check is `postfiat-node deployment-manifest-verify`. Every
validator and RPC unit runs it as `ExecStartPre`, so a unit starts only if it
passes, and `observe-fleet.py` re-runs it over SSH. That is what "manifest
verified on every host" above means. The rollout evidence is the active units
together with `deployment_manifest_sha256` and `deployment_runtime_artifacts`.
Reproduced read-only at 09:51:50–09:51:55Z: all six validators at height 1062
report `false`, manifest `d2fdb687…` and binary `1f8b332d…`.

**Repaired on main** in the commit *Report the deployment manifest signature
check in the validator status*. When `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD` is
set, `deployment-manifest-verify` writes a small record (manifest SHA-256,
deployment ID, publisher, verification time, validity window) to that path
after the check passes. On failure, it deletes the record. `status` then
reports `true` only if the record names the current manifest bytes and the
signed window still covers the current time. A missing, stale or malformed
record keeps the field `false` and does not make `status` fail. Generated
release env files set the record path under `<data-dir>/readiness/`. Deployed
with `signer-rotation-20261007` on 2026-10-09: all six report `true`.

!!! success "2026-09-25: merged combined and FastPay release deployed to all six validators"

    Release `combined-fastpay-20260925` runs on all six validators: source
    `f60e9639` on branch `release/combined-fastpay-20260925` (the merge of
    the deployed FastPay line r4 into the qualified combined line), executable
    `d66cecc36426ce05ced8730b2439a27285c6b404688acd13dc23594b884eabd6`, signed
    manifest `7a682ffe…` (publisher `pfc531e0…`). It replaced
    `fastpay-committee-20260925-r4` (`44b6794f…`) with
    `scripts/postfiat-safe-rollout apply-next`, one validator at a time
    (validator-1 canary, then 0, 2, 3, 4, 5), 13:37–14:14Z.

    Before: six on r4 at height 1044, tip `136985ca…`, root `8f22d40f…`.
    After (`14:18:53Z`): six on the new release, all 12 validator and RPC
    processes on `d66cecc3…`, manifest verified on every host, height 1050,
    tip `03a24230…`, root `13d9e652…`, no divergence at any step.

    The chain makes blocks only for transactions, so each check used one
    devnet faucet grant of 1 PFT from the faucet account to the `testing`
    wallet (StakeHub `pft faucet`): heights 1045–1050, one per applied
    validator, 6 PFT plus 192 atoms of fees in total. Nothing else moved.

    Rollback: the r4 executable and r4 units stay on every host; the
    per-validator path is `rollback-one.sh` (data in place, verified by r4
    first), and the signed validator-1 backup at 1044 is the fallback. It was
    not needed. The FastPay stall trigger is unchanged: validator-5 holds no
    FastPay effects, so a FastPay payment followed by validator-5's turn to
    propose still needs a view change. StakeHub's config still names the r4
    executable for its CLI calls. Records:
    [deployment README](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-fastpay-20260925/deployments/combined-fastpay-20260925/README.md).

!!! note "2026-09-23: the September 11 fleet-wide service restart was Ubuntu's unattended security upgrade"

    Read-only checks on all six hosts on 2026-09-23 (`/var/log/apt/history.log`,
    `/var/log/unattended-upgrades/unattended-upgrades.log` and
    `unattended-upgrades-dpkg.log`, and the systemd journal) found the cause of
    the September 11 `06:01`–`06:47Z` rolling restart. `apt-daily-upgrade.timer`
    started `unattended-upgrade` on validator-2 at `06:01:04Z`, validator-0
    `06:15:29Z`, validator-4 `06:24:38Z`, validator-5 `06:25:47Z`, validator-1
    `06:26:17Z`, and validator-3 `06:46:15Z`. Each run upgraded `libc6`
    `2.39-0ubuntu8.8` → `2.39-0ubuntu8.9` (with `libc-bin`, `locales` and the
    other glibc packages) and `python3.12` `3.12.3-1ubuntu0.15` →
    `3.12.3-1ubuntu0.17`; validator-1 also took `wireless-regdb`. `needrestart`
    (installed on all six hosts with its default configuration) then logged
    `Restarting services...` and restarted every service linked against the
    upgraded C library, including the validator, RPC, Cobalt shadow and NAVCoin
    Ethereum RPC services, which is why the four PostFiat services on each host
    entered active seconds after each upgrade ended (validator-2 `06:01:47`–`50Z`,
    validator-0 `06:16:06`–`08Z`, validator-4 `06:25:15`–`17Z`, validator-5
    `06:26:15`–`18Z`, validator-1 `06:26:48`–`50Z`, validator-3 `06:46:42`–`44Z`,
    re-read on 2026-09-23). No person, deployment tool or session acted; the
    "actor unknown" statements below are superseded. The same timer runs daily on
    each host at a randomized time between roughly `06:00` and `07:00Z`: on
    2026-09-22 (validator-0, `libexpat1`/`rsyslog`/`glib` upgrades, `06:54:50Z`)
    and 2026-09-23 (validator-1, `libxml2`/`glib`/`sudo` upgrades, `06:48:49Z`)
    it restarted only the NAVCoin Ethereum RPC proxy services; the validator,
    RPC and Cobalt shadow services on all six hosts still show the September 11
    start times. On 2026-09-23 at `09:13Z` the operator installed
    `/etc/needrestart/conf.d/50-postfiat-manual-restart.conf` on all six hosts
    (identical file, SHA-256 `305836ca…`), which sets `override_rc` to `0` for
    every `postfiat-*` and `navcoin-*` unit: security upgrades keep installing,
    and `needrestart` now lists those services as needing a restart instead of
    restarting them, so they are restarted only in a planned window. Before the
    install the file was tested on a temporary copy (it matches the validator,
    RPC, Cobalt shadow and NAVCoin RPC proxy units and not `cron`); after it,
    `needrestart -r l -b` exits 0 with no error, and all PostFiat services are
    active with unchanged start times, so nothing was restarted. Rollback:
    delete that file on each host.

!!! warning "2026-09-14: fresh read-only fleet observation, not repair"

    The RPC capture ran from `2026-09-14T11:30:24Z` through `11:32:29Z`.
    All six validators answered `status`, `server_info`, and `mempool_status`
    and agreed at height 1020, tip `9d02b8ee…b1768feb`, state root
    `587c6526…d39bead6`, with empty mempools. Validator-2's first read timed
    out; its three reads succeeded on retry. The chain is unchanged since the
    September 11 observation. All 12 validator/RPC processes are active on
    release `a666-source-route-20260907`, binary `57b0f4d1…634eec83`.

    The September 11 validator-0 RPC replacement was part of an out-of-lane
    rolling stop/start of the validator, RPC, Cobalt shadow, and NAVCoin
    Ethereum archive RPC services on all six hosts. Current systemd service
    entry times are validator-2 `06:01:47`–`06:01:50Z`, validator-0
    `06:16:06`–`06:16:08Z`, validator-4 `06:25:15`–`06:25:17Z`,
    validator-5 `06:26:15`–`06:26:32Z`, validator-1 `06:26:48`–`06:27:01Z`,
    and validator-3 `06:46:42`–`06:46:55Z`. Validator-5's archive RPC entered
    active at `06:26:32Z`, later than the `06:26:18Z` end of the operator's
    earlier summary window; its validator and RPC entered active by `06:26:18Z`.
    The operator's September 14 read-only checks found no host reboot, no
    accepted SSH login on validators 0, 2, or 3 in the `05:45`–`07:00Z`
    journals, and no action from this server in that window. The actor is
    unknown. This was a fleet-wide service restart, not a host reboot or a
    deployment: the September 11 post-restart identity check and this capture
    found the same validator/RPC binary on all 12 processes.

    At the operator's `11:15`–`11:22Z` checks, validator-0 loopback status
    answered immediately (response id `remote-5653`), and the
    `127.0.0.1:27650` listener had an empty accept backlog. The event log's
    last write then was `04:54Z`; its last 200 events were 177
    `vault_bridge_route` and 23 `status`. Validator-0 RPC is still answering.
    Its fresh event-log tail at `2026-09-14T11:31:36Z` ends at completed
    request index 5,655 of 10,000
    (56.55%). Since process start at `2026-09-11T06:16:08Z`, that averages
    about 73.2 accepted connections per hour; at that whole-process rate the
    remaining 4,345 requests reach the limit around `2026-09-16T22:53Z`.
    The earlier `2026-09-13T00:21Z` estimate did not occur as traffic fell.
    The event log has request indices but no event timestamps or index from
    24 hours before this capture, so a last-24-hour rate and corresponding
    exhaustion time cannot be calculated from the permitted reads. These are
    point-in-time observations and conditional straight-line estimates. No
    fleet or chain write was performed.

!!! warning "2026-09-11: fresh read-only fleet observation, not repair"

    The capture ran from `2026-09-11T10:33:54Z` through `10:34:57Z`. All six
    validators answered `status`, `server_info`, and `mempool_status` and
    agreed at height 1020, tip `9d02b8ee…b1768feb`, state root
    `587c6526…d39bead6`, with empty mempools. The chain has not advanced past
    the September 10 height; the delta is zero blocks.

    Validator-0's RPC answered. Its current process started at
    `2026-09-11T06:16:08Z`, not `2026-09-09T20:19:37Z`, and its latest
    completed accepted-connection request index was 1,025 of 10,000 (10.25%).
    Across its actual 4 hours 18 minutes 49 seconds of runtime, that is roughly
    238 accepted connections per hour. A straight-line estimate leaves about
    37 hours 46 minutes to the limit, around `2026-09-13T00:21Z`, if the rate
    and process lifetime remain unchanged. The observation does not establish
    why the September 9 process was replaced.

    Read-only process identity checks found all 12 validator/RPC processes
    active and running release `a666-source-route-20260907`, binary
    `57b0f4d1…634eec83`. Source repairs `83488d91` and `15af691d` remain
    undeployed. No restart, deployment, configuration change, host write, or
    chain write was performed.

!!! warning "2026-09-10: validator-0 RPC diagnosis, not repair"

    Read-only diagnosis identified why validator-0 timed out on September 9.
    Its deployed RPC accepts at most 10,000 connections. The event log for the
    affected process ends at request index 10,000, while one long-lived
    keep-alive connection had not returned an event. The deployed
    [serve loop](https://github.com/postfiatorg/postfiatl1v2/blob/634e3625b48aa75cb6b9fc2225aa5ce83584bb3c/crates/node/src/rpc_cli.rs#L690-L820)
    then stops accepting and waits for every active connection,
    leaving systemd `active` while new clients wait in the unserviced socket
    backlog. Validator-0 alone receives repeated wallet bridge-readiness
    traffic through a long-lived authenticated SSH tunnel; the other five RPC
    processes had accepted only 121–124 connections when validator-0's current
    process had accepted 3,404.

    A separate session restarted the RPC at `2026-09-09T20:19:37Z`, after the
    failed observation and before this diagnosis. This campaign did not
    restart or change anything. The replacement answered `status`,
    `server_info`, and `mempool_status` from `2026-09-10T10:24:20Z` through
    `10:24:31Z`: height 1020, tip `9d02b8ee…b1768feb`, state root
    `587c6526…d39bead6`, empty mempool, and the same pre-fix binary. Current
    service availability is restored but the finite-request/keep-alive defect
    remains and can recur; no repair or fleet mutation was performed.

!!! warning "2026-09-09: fresh read-only observation has five-of-six RPC agreement"

    The capture ran from `2026-09-09T09:24:54Z` through `09:27:00Z`.
    Validators 1–5 answered `status`, `server_info`, and `mempool_status` and
    agreed at height 1020, tip `9d02b8ee…b1768feb`, state root
    `587c6526…d39bead6`, with empty mempools. Validator-0's three RPC reads
    each timed out after eight seconds, so its height, root, and mempool are
    unknown for this observation; its host remained reachable and both node
    services were active. Read-only process-identity checks found all 12
    validator/RPC processes running release `a666-source-route-20260907`,
    binary `57b0f4d1…634eec83`. No corrective action or fleet/chain write was
    performed.

!!! success "2026-09-06: RPC status cache repair deployed and verified at height 992"

    All six validators run release `rpc-status-cache-20260906`, binary
    `740d2610…4b46`. The status report now expires after one second and
    refreshes from authoritative state even when legacy JSON files are
    unchanged. All twelve transport/RPC process hashes matched; all 18 served
    status samples across cache expiration agreed with the same block 992,
    tip `066ae1e8…dac111d`, state root `beb33019…f52a8e8`.
    Transactional storage remains active. No new blocks or wallet transactions
    were created by the repair. See the
    [repair handoff](../handoffs/2026-09-06___codex__rpc_status_cache_fix.md)
    and `deployments/rpc-status-cache-20260906/deploy-receipt.json`.
    The August observations below describe earlier releases and checkpoints.

!!! success "2026-08-31: transactional storage DEPLOYED AND ACTIVE at height 931"

    Release `storage-lease-af9b83c3` (writer lease `f0013c29` + storage
    rework, binary `383f4325…141a7a`) is live on all six validators after
    passing the deployment-exact gate on real-host clones. The chain migrated
    and activated transactional `redb` storage at height 930 and finalized
    block 931 on it: all six converged at tip `8e3639ee…665c4b`, commitment
    `postfiat.replicated_state.v2`, zero full-history scans. Z1 observation
    started `2026-08-31T04:29:41Z`. Receipts:
    `deployments/storage-lease-20260831/deploy-receipt.json` and
    `benchmarks/storage-scaling/deployment-exact-gate/gate-926-receipt.json`.
    Height-926 details below are historical.

!!! success "2026-08-31 (earlier): chain unwedged and live at height 926"

    The height-924 wedge was the superseded validator-registry reapplication
    defect in the **deployed** `8cc7d15e` lineage, not only in the storage
    candidate. Minimal release `registry-fix-291d1eb1`
    (`8cc7d15e` + backported `2c7aa36f`, binary
    `6b07a8c3…8ef7de6`, no storage changes) was signed and rolled to all six
    validators. Certified transfer rounds then committed blocks 925 and 926;
    all six validators converged at tip `9e738e87…c2098b`, state root
    `687b45d5…ac4ad5`. Three operational faults were also repaired
    (validator-0 RPC accept-queue wedge, validator-1's unrestored
    `ordered_batches` JSONL head, root-owned certified-send outbox jobs).
    Source repair `353156c3` for the block-924 finalized-checkpoint snapshot
    export defect is an ancestor of deployed base `707e006f`. No post-repair
    fleet-wide signed export receipt is committed, so live snapshot usability
    remains unverified even though the source defect is no longer open.
    Inventory row STO-05 was closed on 2026-10-02 (`830dc9c7`) on a single-host,
    post-repair receipt: the 2026-09-28 signed export from validator-1 at height
    1050 (manifest `a40d6a99…`, replayed to root `13d9e652…`)
    ([STO-05](../review/defect-inventory-20260910.md#live-environment-check-20261002)). See the
    [registry-continuation wedge postmortem](../postmortems/devnet-registry-continuation-wedge-2026-08-31.md)
    and `deployments/registry-fix-20260831/deploy-receipt.json`. Sections
    below describing the height-924 state are historical.

This page separates the last observed controlled-devnet state, deployed runtime
lineage, repository state, and adversarial campaign. These are different planes.
A repository commit is not deployed unless a fleet receipt binds its source and
binary to the running services.

!!! warning "Point-in-time evidence"

    The latest recorded fleet state is from the 2026-10-21 store-handle
    rollout and FastSwap committee migration (entries at the top of this
    page): all six validators at height 1137, tip `18a4dc86…`, root
    `1277c76c…`, with six-way agreement after every host and
    `demo-preflight.py` READY (60 PASS). The latest fleet observation is
    `2026-10-21T12:30:55Z`. The September and August observations below remain
    historical evidence. Every observation is point-in-time evidence, not a
    real-time query now.

## Operational summary

| Plane | Recorded state | Exact identifier | Observed or updated at | Evidence and freshness |
| --- | --- | --- | --- | --- |
| Running devnet, latest recorded state | All six validators agree at height 1160. Blocks 1154–1159 are the index-v3 rollout's faucet grants (one after each apply); 1160 is the vote-record check grant. `demo-preflight.py` reported READY. | Chain `postfiat-wan-devnet-2`; genesis `ce22ca8c…e90a9`; tip `f65e6fac…`; root `f06608d4…` on all six. | `2026-10-27T12:14Z` | 2026-10-27 entry above; [after state](https://github.com/postfiatorg/postfiatl1v2/blob/release/index-v3-20261027/deployments/index-v3-20261027/observed/after.json). Point in time. |
| Deployed runtime | Release `index-v3-20261027` on all six validators: all 12 validator and RPC processes on one executable, `deployment_manifest_verified=true` and build `d728f895` on every host, RPC units with `--max-requests 100000` and `RestartSec=1`. Rolled out one validator at a time (validator-1 canary, then 0, 2, 3, 4, 5); rollback was not needed. Governed activation heights `recipient_format`, `rejected_tx_fee` and `require_memo` all 1153 (unchanged); `account_tx` v3 index built on all six (range 1–1159) and following commits. Vote outcome records partly live (VR-01). | Source `d728f895`; executable `b5cf3ce4…`; signed manifest `a61e290a…`; rollback release `exchange-readiness-20261023` (`4085ac70…`). | `2026-10-27T12:14Z` | 2026-10-27 entry above. |
| Storage | Transactional `redb` storage has been the live store since height 930 (2026-08-31). This release carries the STO-07 store-handle fix (`8644f0af`); every batch the new executable accepted reports `storage_work.database_opens=0`. Storage is `OFFLINE QUALIFIED` (2026-10-23, packet-r3 `357f06ee…`); that is neither `CLONE QUALIFIED` nor a deployment decision. | Activation height 930; deployed binary `eea96ba6…` | 2026-10-21 deployment; 2026-10-23 storage note | [Storage-scaling implementation boundary](#storage-scaling-implementation-boundary) and the [storage milestone](../plans/completed/storage-scaling-milestone.md). The August `10dd9f20` canary and its rollback are in the [canary rollback report](../postmortems/devnet-storage-live-canary-rollback-2026-08-30.md). |
| Swap and bridge committees | FastSwap committee epoch 2 on all six (`enabled: true`), migrated 2026-10-21. Bridge committee epoch 2 activated at height 1131 on 2026-10-15. The A666 and BMNRC routes stay on `authority_epoch` 1, so validators 0–4 still sign Ethereum checkpoints; neither issuer route update has been submitted. FastPay committee epoch 2 was installed at height 1123 for orders 10001–19990. | Epoch-2 committee root `72836827…`; route authority root `a2eebcba…` | 2026-10-21 | 2026-10-21, 2026-10-15 and 2026-10-12 entries above. |
| Validator-trust authority | Cobalt remains active for validator-registry and trust-graph ratification. The final signed drill rollback committed at 922, return to Cobalt at 923, and legitimate validator-5 rotation at 924. Consensus v2 remains block finality. | Registry root `08a451e0…2b9b1d`; trust root `89f18aef…08f0307`; ratification anchor sequence 2, ID `5eada38d…c21153c8`. | Accepted history through height 924; fleet-audited through `2026-08-30T23:00:39Z`. | The recovery probe found authority mode 1 and identical registry/trust roots on all six. |
| Repository | The deployed source is `d728f895` on `release/index-v3-20261027`; it equals `main` `d728f895`. A later `main` commit is not deployed until a fleet receipt binds its source and binary to the running services. | Deployed source `d728f895`; use `git rev-parse HEAD` for the checkout. | 2026-10-27 | Release records above. |
| Adversarial campaign | E1–E6 passed their locked gates. The consolidated decision is `KEEP_ACTIVE` for Cobalt's bounded controlled-devnet validator-trust role. | E1 `495a59a2…4dfcd90`; E2 `8742d960…d7cba3`; E3 `9302b355…40b600`; E4 `93ba3db0…c14508`; E5 `0695284a…f3b3db`; E6 `ee6848f5…121be0b`. | Completed 2026-08-26 | [Completed milestone](../plans/completed/cobalt-adversarial-verification-milestone.md) and [results](../governance/cobalt-adversarial-verification-results.md). |
| Operator boundary | The campaign proves protocol capability, not operator decentralization. Current proposals and authorizations originate from Foundation-administered validators. | E6 decision: independent-operator proposal path remains a mandatory follow-on milestone. | 2026-08-26 | No independent operators were recruited and no mainnet authority was granted. |

Earlier summary values (the 2026-08-30 to 2026-09-14 observations, the
validator-0 RPC diagnosis and the August deployed binaries) are kept in
[Last observed devnet values](#last-observed-devnet-values) and the dated
entries above.

## Baseline latency versus chain height

E4's first 50 baseline rounds reproduce the activation run's
`consensus_round_ms` p95: 1,664 ms versus 1,660 ms. Latency then rises almost
linearly with height to about 14.9 s at round 500, with correlation approximately
0.9998. In the deployed lineage measured by E4, chain-height amplification came
from full-prefix JSONL verification and full ordered-history proposal rebuilds.
The repository now contains an undeployed transactional `redb` candidate. Its
height-915 replay, tamper/crash matrix, and a development-only height-501
six-clone workflow pass. On 2026-08-30, all six exact height-924 transactional
rebuilds and independent verifies passed, but the first certified continuation
round failed on superseded validator-registry history reapplication. That
candidate is not clone-qualified. Successor `10dd9f20` repaired the history
fault and passed the existing runner, but the validator-1 canary proved the
runner was not deployment-exact: it never co-started transport and RPC against
one transactional database or exercised the signed systemd sandbox. The
successor is not deployment-qualified. A partial three-binary performance
run through legacy height 100 was stopped after its selected verifier was found
to mutate inspected state; source `785806bd` corrects that verifier and passes
whole-directory mutation sentinels. The existing three-binary,
lane-native-snapshot harness still conflicts with the locked research
specification and cannot close the performance gate. The 5% gate remains valid as a
paired, same-length A/B comparison; the absolute latency numbers are not a
finality SLA and public testnet remains blocked.

## Storage-scaling implementation boundary

The committed implementation selects transactional `redb` storage for the
finality path while retaining authenticated bounded JSONL heads for legacy
import and audit. It includes the fixed-size ordered-history accumulator,
atomic per-height commit, replay/rebuild tooling, the closed 69-case
tamper/crash matrix, exact height-915 replay, and local paired-binary rollback
evidence. The live canary invalidated any broader rollback claim: the writable
rebuild upgrades legacy JSONL heads in the source directory, and the exact
deployed `8cc7d15e` binary rejects those v2 heads. Its offline verifier opens
source and target read-only and
refuses every repair-required state without durable mutation. A local
height-501 development rehearsal also completed the ten-phase six-clone
workflow. The later exact height-924 rehearsal stopped before its first new
height: the candidate treated a superseded registry update as due again and
failed its previous-root check. Successor `10dd9f20` fixed that defect, but the
old G6 service model started only transport processes. On the live host,
transport retained the transactional database lock and RPC could not start.
The canary was rolled back, and no current candidate is deployment-eligible.
Nothing in this section proves deployment. See the
[active milestone](../plans/completed/storage-scaling-milestone.md) and
[development evidence](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/storage-scaling).

**2026-10-22, storage qualification packet:** not assembled, so storage is not offline qualified (state SELECTED). Both exact replays passed with the deployed `store-handle-20261021` binary (`eea96ba6…`, source `86706699`): height 1123 from the signed canary archive, and height 924 from the custody copy. The 2026-10-20 G1, G2 and G4 evidence, however, is on `8644f0af` (binary `d1799edb…`). The packet verifier accepts one source and binary for all of it, still requires the lost height-915 archive, and needs a six-clone migration report for that candidate, which does not exist. The gap list is in [storage G5](../plans/completed/storage-scaling-milestone.md#g5-offline-qualification-packet). The same day, G1, G2 and G4 were re-run on the deployed binary `eea96ba6…` and PASS (consensus ratio 1.048, wallet 1.064; [attempt 4](../plans/active/g4-regression-deployed-release-20261019.md#result-2026-10-22-attempt-4-deployed-binary)), so that evidence and the replays now share one source and binary.

**2026-10-23, storage qualification packet:** assembled, and the offline verifier passes. Storage is still not offline qualified (state SELECTED). The six-clone migration rehearsal passed on the deployed binary `eea96ba6…` (source `86706699`, work server only). It used scratch copies of the six height-924 directories and ran from 924 through activation at 932 to 934. All six converged, and the originals' tree hashes were unchanged. The packet binds that rehearsal, G1/G2/G4 of attempt 4 and the 924 and 1123 replay receipts. After the reviewer's changes (`4d8d2a72`), packet-r3 (checksum manifest `357f06ee…`, verifier output `e4fba47e…`, work server) verifies offline, so storage is **OFFLINE QUALIFIED** as of 2026-10-23. This supersedes the SELECTED state and open boxes stated in this paragraph, and it is neither `CLONE QUALIFIED` nor a deployment decision. Five storage-milestone boxes remain before the label; they are listed in [storage G5](../plans/completed/storage-scaling-milestone.md#g5-offline-qualification-packet).

## Last observed devnet values

### Fresh all-six fleet observation — 2026-09-14

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| RPC capture window | `2026-09-14T11:30:24Z`–`2026-09-14T11:32:29Z` |
| Agreement | All six agree on chain, genesis, height, tip, and state root; every mempool is empty. Validator-2 answered on retry after one timeout. |
| Height | 1020 on all six; no advance past September 11 (`+0`) |
| Mempool | 0 pending on every validator |
| Tip hash | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` |
| State root | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` |
| Running release | `a666-source-route-20260907` on all 12 validator/RPC processes |
| Running binary | SHA-256 `57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83` on all 12 processes |
| Validator-0 RPC budget | Answering; PID 3611291 started `2026-09-11T06:16:08Z`; latest completed event index 5,655 of 10,000 (56.55%) at `2026-09-14T11:31:36Z`. Whole-process mean ~73.2/hour; conditional limit ~`2026-09-16T22:53Z`. Last-24-hour rate and limit estimate unavailable because events have no timestamps. |
| September 11 service restart | All four PostFiat services on each of the six hosts entered active that morning. Origin unknown; operator checks found no host reboot, and the same validator/RPC binary remained in service. Validator-5 archive RPC entered active at `06:26:32Z`, outside the earlier reported `06:26:15`–`06:26:18Z` window. |

| Validator | RPC status, server info, mempool | Observed ledger | Running identity | RPC captured |
| --- | --- | --- | --- | --- |
| validator-0 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-14T11:30:24Z`–`11:30:31Z` |
| validator-1 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-14T11:30:24Z`–`11:30:31Z` |
| validator-2 | Running; mempool 0; one prior timeout | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-14T11:32:18Z`–`11:32:29Z` |
| validator-3 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-14T11:30:24Z`–`11:30:32Z` |
| validator-4 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-14T11:30:24Z`–`11:30:31Z` |
| validator-5 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-14T11:30:24Z`–`11:30:34Z` |

Since the validator-0 RPC process started, 5,655 completed event indices in
77 hours 15 minutes 28 seconds average approximately 73.2 accepted connections
per hour. At that rate, 4,345 remaining connections imply about 59 hours
22 minutes to the limit, around `2026-09-16T22:53Z`. The September 11
projection of `2026-09-13T00:21Z` did not occur as traffic fell. The event
log does not timestamp individual requests, and no September 13 index is
available from the permitted reads. A last-24-hour rate or corresponding
straight-line limit time would be guesswork. These estimates assume unchanged
traffic and process lifetime.

The observation used authenticated SSH forwarding to each loopback RPC and
only `status`, `server_info`, and `mempool_status` calls, plus read-only
systemd, `/proc` executable SHA-256, and validator-0 event-log tail checks.
The earlier operator uptime and journal checks are cited above, not repeated.
No restart, deployment, configuration change, host write, or chain write was
performed.

### Fresh all-six fleet observation — 2026-09-11

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Observation window | `2026-09-11T10:33:54Z`–`2026-09-11T10:34:57Z` |
| Agreement | All six agree on chain, genesis, height, tip, and state root; every mempool is empty. |
| Height | 1020 on all six; no advance past the September 10 height (`+0`) |
| Mempool | 0 pending on every validator |
| Tip hash | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` |
| State root | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` |
| Running release | `a666-source-route-20260907` on all 12 validator/RPC processes |
| Running binary | SHA-256 `57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83` on all 12 processes |
| Validator-0 RPC budget | Answering; current process PID 3611291 started `2026-09-11T06:16:08Z`; latest completed accepted-connection request index 1,025 of 10,000 (10.25%) |
| Repository/fleet boundary | Source repairs `83488d91` and `15af691d` are not deployed on any observed process. |

| Validator | RPC status, server info, mempool | Observed ledger | Running identity | RPC captured |
| --- | --- | --- | --- | --- |
| validator-0 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-11T10:33:54Z`–`10:34:01Z` |
| validator-1 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-11T10:34:01Z`–`10:34:08Z` |
| validator-2 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-11T10:34:08Z`–`10:34:18Z` |
| validator-3 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-11T10:34:18Z`–`10:34:26Z` |
| validator-4 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-11T10:34:26Z`–`10:34:32Z` |
| validator-5 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-11T10:34:32Z`–`10:34:42Z` |

The requested rate from the September 9 process start cannot describe the
current process because its accept counter reset with the later September 11
start. From the current start through the accept-count check, 1,025 connections
in 4 hours 18 minutes 49 seconds is approximately 238 per hour. At that rate,
the remaining 8,975 connections represent roughly 37 hours 46 minutes, with a
straight-line limit time around `2026-09-13T00:21Z`. This is a rough utilization
projection, not a prediction that the process will remain active or exhaust.

The observation used authenticated SSH forwarding to each loopback-bound RPC,
exactly one `status`, `server_info`, and `mempool_status` call per validator,
and read-only service/process identity and validator-0 event-index checks. No
restart, deployment, configuration change, host write, or chain write was
performed.

### Validator-0 read-only diagnosis — 2026-09-10 { #validator-0-rpc-diagnosis-20260910 }

This was a diagnosis of the failed September 9 endpoint, not a new all-fleet
observation. The replacement RPC process answered all three documented reads:
height 1020, tip `9d02b8ee…b1768feb`, state root
`587c6526…d39bead6`, and zero pending transactions. Its release remains
`a666-source-route-20260907`, binary SHA-256
`57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`.
That agrees with the September 9 values from validators 1–5, but the captures
were not simultaneous and do not establish a new all-six observation.

The affected process ran from `2026-09-07T01:10:26Z` until a separate session
restarted it at `2026-09-09T20:19:37Z`. Its event sequence reached the unit's
`--max-requests 10000` ceiling. One connection remained active, so the process
stayed alive in its post-accept drain instead of exiting; new requests were no
longer accepted. Host telemetry around the failed `09:24`–`09:27` capture
showed 94.56% CPU idle, no blocked tasks, 0.14% I/O wait, and ample available
memory, so host resource exhaustion does not explain the timeouts.

The replacement process still has a persistent keep-alive client and the same
10,000-connection budget. At the diagnostic capture it reported 3,404 accepted
connections and a peak of three active connections; the five peers reported
121–124 accepted connections and peaks of two or three. This is a reproduced
service-lifecycle defect and a recurrence risk, not evidence of a live repair.
No service, host, configuration, or chain state was changed by this diagnosis.

### Historical partial fleet observation — 2026-09-09

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Observation window | `2026-09-09T09:24:54Z`–`2026-09-09T09:27:00Z` |
| Agreement | Validators 1–5 agree; validator-0 RPC status is unknown after timeouts. Full-six agreement is not established by this observation. |
| Height | 1020 on validators 1–5; validator-0 unknown |
| Mempool | 0 pending on validators 1–5; validator-0 unknown |
| Tip hash | `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb` on validators 1–5 |
| State root | `587c6526a2549c97458b371f42e849c49274a1f522e7dcb841b74cd74bdb3d6747c2e6ca646c08ac51733796d39bead6` on validators 1–5 |
| Running release | `a666-source-route-20260907` on all 12 validator/RPC processes |
| Running binary | SHA-256 `57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83` on all 12 processes |
| Repository/fleet boundary | `origin/main` signing fix `bbb291ce` is not deployed on any validator. |

| Validator | RPC status, server info, mempool | Observed ledger | Running identity | Identity captured |
| --- | --- | --- | --- | --- |
| validator-0 | Timed out after 8 seconds on each read; host reachable; validator and RPC services active/running | Height, tip, root, and mempool unknown | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:45Z` |
| validator-1 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:47Z` |
| validator-2 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:52Z` |
| validator-3 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:55Z` |
| validator-4 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:26:56Z` |
| validator-5 | Running; mempool 0 | Height 1020; `9d02b8ee…b1768feb`; `587c6526…d39bead6` | `a666-source-route-20260907`; `57b0f4d1…634eec83` | `2026-09-09T09:27:00Z` |

### Drift from the 2026-09-07 machine handoff

The five validators with readable RPC status advanced from height 1005 to 1020
(+15 blocks), and their state root changed from `6ed69ca9…ced065f9` to
`587c6526…d39bead6`. They agree with one another. Validator-0's current ledger
state could not be compared because its RPC timed out. All six hosts still run
the same release and byte-identical binary recorded on September 7. Because the
running SHA-256 remains `57b0f4d1…634eec83`, none runs a build containing the
later signing fix `bbb291ce`.

The observation used authenticated SSH forwarding to each loopback-bound RPC
and the repository's read-only `status`/`server_info`/`mempool_status` checks.
Release identity came from read-only systemd `MainPID`, `/proc/<pid>/exe`, and
SHA-256 inspection. No restart, deployment, configuration change, host write,
or chain write was performed.

### Historical 2026-08-30 all-six values

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Observation window | `2026-08-30T23:00:24Z`–`2026-08-30T23:00:39Z` |
| Validators | 6, all converged |
| Height | 924 |
| Mempool | 0 pending on every validator |
| Tip hash | `ebeb0e1ee27f30ba480255728832719d94eac1a89d762a7aa7019eae269008fac53098cf6495f477a241d63a7649fbef` |
| State root | `0854bc47f78996b2dcd279206cbdcc0b4858395c5937e0e0d56b3d645ca6b6a9d9c9578f5ac77bb14bea9dd1ee6f413e` |
| Registry root | `08a451e07aeaf9ada41a69e7c26dfd3fd86fce11c02f5567127c598b3cf775ac054b2add85295cc8c0d429bb6d2b9b1d` |
| Trust-graph root | `89f18aef2c5726ae43043407eb4d638ee8f3b6027e58ec3553296478602232cf3c2fc5d1dfebc4058d720b16508f0307` |
| Ratification anchor | Sequence 2; `5eada38d23c83709a44f2cfa7eb7897d9d4b1da906e6ef66fc5dfec7e64102edda2e82b33d71346c1d8f75ccc21153c8` |
| Validator-trust authority | Cobalt |
| Block finality | Consensus v2 |
| Validator registry updates | 2 |
| Latest update | Validator-5 key rotation `a6b806eb304ffc5d4c329fc179fa628745a2e609724cd67d654805ebfc4cc12bc4338ed5fa1f3bf301384ca8aaf8f18a` |

Cobalt ratifies validator-registry and trust-graph changes. A separate layer
decides which validators deserve trust. Current proposals originate from
Foundation-administered validators. Cobalt does not order blocks, replace
Consensus v2, or prove operator decentralization.

## Accepted live drill history

| Height | Accepted action | Proposal identity | Authorization identities | Result |
| ---: | --- | --- | --- | --- |
| 920 | Initial rollback to Foundation | validator-2 | validators 0–4 | Accepted; retained as remediation history. |
| 921 | Initial return to Cobalt | validator-3 | validators 0–4 | Accepted, but its trust binding did not match the protocol-native post-return graph; not used as the final gate. |
| 922 | Corrective rollback to Foundation | validator-4 | validators 0–4 | Accepted final-gate rollback. |
| 923 | Corrective return to Cobalt | validator-5 | validators 0–4 | Accepted final-gate return with the correct trust binding. |
| 924 | Legitimate validator-5 key rotation | validator-0 | validators 0–4 | Accepted; old/stolen validator-5 key was not an authorizer. |

All six validators reported the same height-920-through-924 history. Every block
had at least five Consensus v2 votes. The E5 packet preserves the first pair,
the correction, and the final state; it does not rewrite the remediation out of
the record.

## Evidence boundaries

The final E5 observation performed authenticated service, process/binary,
validator status, governance verification, registry-root, finality-history, and
shadow-status checks. Negative drills confirmed the durable governance and
registry file hashes were unchanged. The replacement validator private material
never entered the repository packet.

Verify the live drill packet:

```bash
python3 benchmarks/cobalt-adversarial-verification/e5/verify_packet.py
```

Verify the consolidated campaign and render the operator result:

```bash
python3 benchmarks/cobalt-adversarial-verification/packet/verify_packet.py
PYTHONPATH=python python3 -m postfiat_rpc.cobalt adversarial
```

These commands authenticate committed evidence. They do not query the fleet.

Dated handoffs and completed plans are historical snapshots. When their mutable
operational statements conflict with this page, this page owns the current
record; the observation time still limits every claim.

## 2026-09-28: disk cleanup on validator-1 and validator-0

Deleted only pre-rollout artifacts written by `python/postfiat_ops/safe_rollout.py`
under `/var/lib/postfiat/pre-rollout-snapshots`. No service was restarted and no
chain action was taken; both hosts still run `combined-fastpay-20260928`.

| Host | Free before | Free after | Deleted |
|---|---|---|---|
| validator-1 | 2.95 GB (97 %) | 9.92 GB (88 %) | 1 partial `.incoming` upload (56 MB); 69 exports and staged binaries of releases older than `fastpay-committee-20260925-r4` with no rollout state (6.92 GB) |
| validator-0 | 7.66 GB (90 %) | 7.72 GB (90 %) | 1 export, `atomic-swap-w6-395e28e5` (58 MB) |

No rotated logs existed. Left for the other lane: each host's live
`transport-validator-events.ndjson` (13.9 GB, never rotated), three
`/var/backups/postfiat` dumps (validator-1 10.6 GB, validator-0 7.6 GB), `gate931`
(7.7 GB and 7.1 GB, Arc gate 2026-09-02), `gate926` (5.5 GB and 4.9 GB; the
2026-08-31 storage-gate handoff lists its removal as owed), and entries of
unknown origin: validator-1 `.a666-checkpoint-verifier-experimental.incoming*`
(3 × 482 MB) and `a666-opening-export-proof-h348` (131 MB), validator-0
`fastpay-committee-20260925-r3-retained-payment` (229 MB). Kept: r4, both
combined-fastpay backups, and the cobalt and registry-fix artifacts referenced by
local rollout state. Inventory:
`deployments/combined-fastpay-20260925/observed/disk-inventory-20260928.json` on
`release/combined-fastpay-20260925` (`4d88956b`).

## 2026-09-29: validator event log rotation

Installed `/etc/logrotate.d/postfiat-validator-events` on all six validators:
`transport-validator-events.ndjson` now rotates at `size 2G` with `rotate 3`,
`compress`, `delaycompress`, `missingok`, `notifempty` and `copytruncate`, checked
by the daily `logrotate.timer`. The node opens the file once with `O_APPEND` and
never reopens it, so rotation copies then truncates. No service restarted; all
12 PIDs are unchanged. No chain action was taken and no file was removed.

A raw copy of the 13.87 GB backlog would not fit on validator-1 or validator-0.
So on validators 0, 1, 3, 4 and 5 the backlog was first streamed into
`transport-validator-events.ndjson.pre-rotation-20260929.zst` (4.18 GB,
sha256-verified), then the live file was truncated in place. Validator-2's file
was 40.7 MB, not 13.9 GB; the forced `logrotate` run copied it to `.1`.

| Host | Free before | Free after |
|---|---|---|
| validator-0 | 7.67 GB | 17.36 GB |
| validator-1 | 9.88 GB | 19.57 GB |
| validator-2 | 24.51 GB | 24.51 GB |
| validator-3 | 19.41 GB | 29.10 GB |
| validator-4 | 14.50 GB | 24.19 GB |
| validator-5 | 18.34 GB | 28.03 GB |

`copytruncate` loses events written while the file is being copied. For a 2 GiB
file that takes about 36 s, which is about 6 events at the long-run average of
2.2 KB/s. No events were lost today: every file had been idle since
2026-09-28 08:37Z. For the same reason, no new event had reached the fresh files
by 12:39Z. Rollback: delete the rule on each host; rotated files stay. Record:
`deployments/combined-fastpay-20260928/observed/event-log-rotation-20260929.json`
on `release/combined-fastpay-20260928` (`4788ef1f`).

On 2026-09-30 one 1 PFT faucet grant made height 1063 (proposer validator-1,
view 0, identical on all six), and the fresh transport event files grew by about
50 KB on validators 0 and 2–5 but not on validator-1, whose proposer round runs
in a separate process and sends it no connection (fd, journal and stderr clean),
and a second grant at height 1064 (proposer validator-2, view 0, identical block
and root on all six) grew validator-1's file from 0 to 50,494 bytes (`7869a707`);
`/etc/logrotate.d/postfiat-rpc-events` now rotates `rpc-events.ndjson` at
`size 512M` with the same options (dry-run clean, no forced rotation), recorded in
the same file's `verified-20260930` section (`1448eb12`).

## 2026-10-01: fleet after the other lane's NAVCoin operator tests

Read-only observation at `2026-10-01T10:32:42Z`. This lane made no restart,
configuration change or transaction. Release unchanged: `combined-fastpay-20260928`,
executable `1f8b332d…`, source `c93b2137`.

**Agreement.** All six report height 1085, tip `b2c7f04e…`, root `b60665e9…`,
0 pending.

**Processes.** The six validator PIDs and the RPC PIDs of validators 1–4 are
unchanged since the 2026-09-28 rollout (`observed/after.json`). Two RPC processes
exited and were restarted by systemd:

| RPC unit | PID | Exit | Restart |
|---|---|---|---|
| validator-0 | 4003672 → 4162646 | 07:40:35Z | 07:40:40Z |
| validator-5 | 2891934 → 3034195 | 08:28:25Z | 08:28:31Z |

- Both run the same executable (`1f8b332d…`).
- The journal holds only systemd lines: "Deactivated successfully", which means
  exit 0 or a clean signal, then a scheduled restart (`Restart=always`,
  `RestartSec=5`, `NRestarts=1`).
- There is no "Stopping" line and no `systemctl` command, unlike the planned
  restarts on 2026-09-25 and 2026-09-28.
- SSH port forwards logged `connect_to 127.0.0.1 port … failed` for the RPC
  port at 07:40:35Z (validator-0, 27650) and 08:28:23–24Z (validator-5, 27655).
- Validator-0 had short root SSH sessions at 07:35:36–07:36:08Z and
  07:39:09–07:39:10Z. Validator-5 had none in that window.
- The service writes to `rpc-stdout.log` and `rpc-stderr.log`, not the
  journal; those files were not read. **Cause unknown.**
- In `c93b2137`, the only clean-exit path in `rpc-serve` is its accept budget.
  The unit passes `--max-requests 10000`
  (`crates/node/src/batch_snapshot.rs:2403`), and the accept loop stops at that
  count (`crates/node/src/rpc_serve_runtime.rs:105`,
  `crates/node/src/rpc_cli.rs:608`). The command then prints its report and
  returns 0 (`crates/node/src/main_parts/cli_dispatch_parts/group_03.rs:457`).
  `rpc-serve` installs no signal handler. This path is consistent with the
  journal but not verified.
- Main after `c93b2137` keeps the budget but shortens the gap for the next
  release: the generated unit passes `--max-requests 100000` with
  `RestartSec=1`, and `rpc-serve` finishes in-flight connections and writes
  its report (same `request_count` and `max_requests` fields) before closing
  the listener. The deployed `c93b2137` keeps the old behaviour until then.

**Blocks 1065–1085.** Blocks 1065–1078 are the other lane's operator tests (see its
[handoff](../handoffs/2026-10-01___nazgul__navcoin_create_and_swap_build_phase0_and_bmnrc_opening.md#what-this-lane-did-on-the-shared-fleet-please-read)).
Blocks 1079–1085 came after its 03:35Z update. All seven are on route
`pftl-a666-ethereum-wA666-usdc-v1`, were accepted with 22–23 atoms of fees, and
are signed by `pfab9b92…` (sequences 230–236), the account that signed 1065–1067,
1071, 1073–1075 and 1078. Block headers carry no time.

| Height | Proposer (view) | Kind |
|---|---|---|
| 1079 | validator-5 (0) | `pftl_uniswap_order_reserve` |
| 1080 | validator-1 (1, timeout certificate) | `pftl_uniswap_primary_subscribe_v2` |
| 1081 | validator-1 (0) | `pftl_uniswap_order_release` |
| 1082 | validator-2 (0) | `pftl_uniswap_primary_redeem` |
| 1083 | validator-3 (0) | `pftl_uniswap_order_reserve` |
| 1084 | validator-4 (0) | `pftl_uniswap_primary_subscribe_v2` |
| 1085 | validator-5 (0) | `pftl_uniswap_order_release` |

**Ethereum checkpoint signing.** Records are in
`/var/lib/postfiat/validator-N/ethereum-checkpoint-signing/`.

- Validators 0–4 each gained two records today: one at 01:54–01:56Z, and one at
  02:25Z (validator-0) or 02:49–02:51Z (validators 1–4). Their totals are 55, 55,
  55, 54 and 55.
- Validator-5 has 50 records, the newest from 2026-08-12. Its key is not in the
  bridge signer committee ([handoff](../handoffs/2026-10-01___nazgul__navcoin_create_and_swap_build_phase0_and_bmnrc_opening.md#what-this-lane-did-on-the-shared-fleet-please-read)).

**Event logs and disk.** No file has rotated since 2026-09-30.

| Host | `transport-validator-events.ndjson` | `rpc-events.ndjson` | Free on `/` |
|---|---|---|---|
| validator-0 | 5,347,754 B | 83.7 MB | 17.13 GB (78 %) |
| validator-1 | 5,192,299 B | 53.7 MB | 19.53 GB (75 %) |
| validator-2 | 959,832 B | 54.3 MB | 24.23 GB (69 %) |
| validator-3 | 5,242,406 B | 53.6 MB | 28.91 GB (63 %) |
| validator-4 | 5,242,682 B | 53.5 MB | 24.16 GB (69 %) |
| validator-5 | 5,242,767 B | 215.4 MB | 27.86 GB (64 %) |

Validator-2's transport file was last written at 04:31:38Z; the other five were
last written at 09:25:33Z. The proposer rule (a proposer's own file is not written)
does not account for this, because validator-2 proposed only 1082 of 1079–1085.
The cause is unknown; the event files were not read.

**Ethereum relays.** An archive read was run from validator-1: `eth_getCode` of USDC
at block 26094220.

| Port | Upstream | How it runs | Archive read |
|---|---|---|---|
| 28701 | `ethereum-rpc.publicnode.com` | Validator-1: unit `navcoin-ethereum-rpc-proxy-20260907.service`. Others: hand-started 2026-07-31 | HTTP 403, "Archive requests require a personal token" |
| 28702 | `eth.drpc.org` | Missing on validator-1. Others: hand-started 2026-08-08 | not tested |
| 28703 | `eth.drpc.org` | Unit `navcoin-ethereum-archive-rpc-20260907.service` on all six | HTTP 200, non-empty code |

- On validators 0 and 2–5, the hand-started 28701 and 28702 processes run
  `/tmp/a666-https-jsonrpc-loopback-proxy.py` inside root login-session scopes.
  That file no longer exists, so these relays do not survive a reboot.
- 28703 is the only relay that answers archive reads.
- At this observation, `scripts/a666-mainnet-return-import.sh` and
  `scripts/a666-mainnet-record-destination-consume.sh` defaulted to 28701. They
  now default to 28703
  ([PFTL tools](../navcoins/pftl-tools.md#ethereum-relays-on-the-validator-hosts)).

**Watchdog user units.** These are the other lane's units, run by root's user
manager. Linger was enabled on 2026-09-30 at 22:21Z (validator-0) and 22:22Z
(validator-3). They are left running.

- Validator-0 host: `navcoin-proof-watchdog@normal`, since 2026-09-30 22:21:40Z.
- Validator-3 host, `navcoin-proof-watchdog@` units:
    - `normal` and `fallback`, since 2026-09-30 22:22Z;
    - `fallbackd12`, since 02:10:49Z;
    - `fallbackd16`, since 04:34:07Z;
    - `fallbackd16c`, `d`, `f`, `g`, `h` and `j`, started 09:53:18–09:55:04Z.

**Full-history replay.** The other lane's replay of the ce22 archive at height
1033 is recorded under the
[block-1011 note](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/combined-fastpay-merge-20260925.md#why-the-lines-diverged).

## 2026-10-02: BMNRC created on the live network (blocks 1089–1101)

The other lane created BMNRC on 2026-10-01. Release `combined-fastpay-20260928`
(`c93b2137`) is unchanged. All rows were certified at view 0 and accepted. This
lane read the chain over read-only RPC on validator-1 (10:58–11:03Z) and sent no
transaction.

| Height | Kind | Signer | Key facts |
|---|---|---|---|
| 1089 | `asset_create` | `pf9bd795…` | "BMNR Carry NAVCoin", `BMNRC`, precision 6, asset `43b9ba50…` |
| 1090 | `nav_profile_register` | `pf9bd795…` | `sp1-nav-reserve-v1`, `groth16`, profile `4ff2afa5…` |
| 1091 | `nav_asset_register` | `pf9bd795…` | profile `4ff2afa5…`, unit `USD_1E8`, reserve operator `pf7143fe…` |
| 1099 | `nav_reserve_submit` | `pf7143fe…` | epoch 1, NAV 1.00000000, supply 640585238, net assets 64058523893, packet `ffd5c56b…` |
| 1100 | `nav_epoch_finalize` | `pf9bd795…` | epoch 1, packet `ffd5c56b…` |
| 1101 | `nav_mint_at_nav` | `pf9bd795…` | 640585238 atoms to `pf4b26fe…` |

- Blocks 1086–1088 and 1092–1098 are plain PFT transfers.
- At 1101: block `1b14c476…`, state root `e7122731…`. On the morning of
  2026-10-02 the six validators agreed at 1101.
- Two more blocks followed, both signed by `pf9bd795…` and certified at view 0:
  1102 `pftl_uniswap_route_init_v2` (route `pftl-bmnrc-ethereum-wBMNRC-usdc-v1`)
  and 1103 `pftl_uniswap_route_epoch_advance`.
- At 11:00Z all six validators report height 1103, tip `c9469f05…`, root
  `ad296a02…`, 0 pending.
- Morning preflight: **READY WITH ATTENTION**, relays only. RPC accept-budget
  restarts have now been seen on all six validators, twice on validator-0.
- Protocol enforcement check:
  [BMNRC protocol enforcement check, 2026-10-02](../review/bmnrc-protocol-enforcement-check-20261002.md).

## 2026-10-05: weekend state before the other lane's conference day

Release `combined-fastpay-20260928` (`c93b2137`) is unchanged; `main`, which now
includes the other lane's PR #55 (`b1d1928c`), is not deployed. On 2026-10-06
the next candidate, `signer-rotation-20261007` (`main` at `0fa55d0b`, executable
`decaa411…`), was cut and qualified locally; it is cut, not deployed
([release inputs](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/README.md)). This lane read
blocks 1104–1115 over read-only RPC on validator-1 (`blocks`, `tx`,
`batch_archive`). All twelve were certified at view 0 and accepted, with fees of
22–53 atoms. Block headers carry no time.

| Height | Proposer (view) | Kind | Route or asset | Signer |
|---|---|---|---|---|
| 1104 | validator-0 (0) | `pftl_uniswap_export_debit` | `pftl-bmnrc-ethereum-wBMNRC-usdc-v1` | `pf4b26fe…` |
| 1105 | validator-1 (0) | `pftl_uniswap_destination_consume` | `pftl-bmnrc-ethereum-wBMNRC-usdc-v1` | `pf9bd795…` |
| 1106 | validator-2 (0) | `transparent_transfer` | PFT | `pffcb93d…` |
| 1107 | validator-3 (0) | `pftl_uniswap_route_pause` (paused) | `pftl-a666-ethereum-wA666-usdc-v1` | `pfd0c86d…` |
| 1108 | validator-4 (0) | `nav_reserve_submit` (epoch 9) | A666 `521c6c63…` | `pfd0c86d…` |
| 1109 | validator-5 (0) | `nav_epoch_finalize` (epoch 9) | A666 `521c6c63…` | `pffcb93d…` |
| 1110 | validator-0 (0) | `pftl_uniswap_route_epoch_advance` (10 → 11) | `pftl-a666-ethereum-wA666-usdc-v1` | `pffcb93d…` |
| 1111 | validator-1 (0) | `pftl_uniswap_route_pause` (unpaused) | `pftl-a666-ethereum-wA666-usdc-v1` | `pfd0c86d…` |
| 1112 | validator-2 (0) | `vault_bridge_deposit_propose` | pfUSDC `02c46a36…` | `pf23d883…` |
| 1113 | validator-3 (0) | `vault_bridge_deposit_finalize` | pfUSDC `02c46a36…` | `pf23d883…` |
| 1114 | validator-4 (0) | `vault_bridge_deposit_claim` | pfUSDC `02c46a36…` | `pf23d883…` |
| 1115 | validator-5 (0) | `pftl_uniswap_return_import` | `pftl-bmnrc-ethereum-wBMNRC-usdc-v1` | `pf9bd795…` |

- 1104 exported 640,585,238 BMNRC atoms, the full supply minted at 1101.
  1115 imported 75,000,000 BMNRC atoms to `pff5e290…`; 1114 claimed 75,000,000
  pfUSDC atoms to the same account.
- Preflight `deployments/combined-fastpay-20260928/demo-preflight.py`
  (07:21:55Z): **READY WITH ATTENTION**, 55 PASS, 5 ATTENTION (the hand-started
  relays), 0 FAIL. All six report height 1115, tip `28e1175b…`, root
  `a5287eaf…`.
- Validators up 6 d 23 h with 0 restarts. RPC accept-budget restarts:
  validator-0 four, validator-5 two, validators 1–4 one each.
- Free disk on `/`: 16.8–28.6 GB across the six hosts.
- The other lane's `navcoin-proof-watchdog@` user units on the validator-3 host:
  14.
- This lane changed nothing on the hosts or the chain after 2026-10-02 and will
  change nothing before 2026-10-07.

## 2026-10-13: Ethereum relays run as systemd units

At 09:41–09:45Z the loopback relays `127.0.0.1:28701` (publicnode) and
`127.0.0.1:28702` (drpc) on validators 0, 2, 3, 4 and 5 were moved from
hand-started python3 processes, which ran from the deleted
`/tmp/a666-https-jsonrpc-loopback-proxy.py` and would not survive a reboot, to the
enabled units `navcoin-ethereum-rpc-28701.service` and
`navcoin-ethereum-rpc-28702.service` in `/etc/systemd/system` (`Restart=always`,
`/opt/postfiat/route-tools/navcoin-20260907/https-jsonrpc-loopback-proxy.py
--attempts 3`). Validator-1 gained only the 28702 unit. `eth_blockNumber` returned
HTTP 200 through every relay afterwards. Validator and RPC PIDs, the 28703 units
and the other lane's `navcoin-proof-watchdog@` user units did not change, and no
transaction was sent. The release preflight `demo-preflight.py` then reported
**READY**: 60 PASS, 0 ATTENTION, 0 FAIL, six-way agreement at height 1123, root
`942d1fc2…`
([record](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/observed/ethereum-relays-20261013.json)).
The 28703 unit on all six hosts and validator-1's 28701 unit are still transient
and do not survive a reboot.

## 2026-10-16: archive relay runs as a systemd unit

At 10:45–10:47Z the transient `systemd-run` relay units were replaced by enabled
units in `/etc/systemd/system`, modelled on `navcoin-ethereum-rpc-28702.service`
with the same command line: `127.0.0.1:28703` (drpc) on all six hosts moved from
`navcoin-ethereum-archive-rpc-20260907.service` to
`navcoin-ethereum-rpc-28703.service`, and validator-1's `127.0.0.1:28701`
(publicnode) moved from `navcoin-ethereum-rpc-proxy-20260907.service` to
`navcoin-ethereum-rpc-28701.service`. Each port was free for about one second
between stop and start; `eth_blockNumber` returned HTTP 200 through it afterwards.
Validator and RPC PIDs and the other lane's `navcoin-proof-watchdog@` user units
(validators 0 and 3, all paused or `pause_retry`) did not change, and no
transaction was sent. All relays on all six hosts now restart on boot. The
release preflight `demo-preflight.py` (10:46:48Z) reported **NOT READY**: 54
PASS, 0 ATTENTION, 6 FAIL, six-way agreement at height 1131, root `a12e7c5f…`.
Every relay row passed; the six FAIL rows are its `units` check, which still names
the removed transient unit `navcoin-ethereum-archive-rpc-20260907.service`
([record](https://github.com/postfiatorg/postfiatl1v2/blob/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/observed/ethereum-relays-20261016.json)).
That NOT READY was only the stale unit name in the scripts, not the fleet: with
`demo-preflight.py` and `observe-fleet.py` checking
`navcoin-ethereum-rpc-28703.service` (release `6b4fa4b0`), the preflight at
10:55:03Z reported **READY**: 60 PASS, 0 ATTENTION, 0 FAIL, six-way agreement at
height 1131, root `a12e7c5f…`
([preflight](https://github.com/postfiatorg/postfiatl1v2/blob/6b4fa4b0596fecbdad663d9cddf902ae7b56d8b7/deployments/fastlane-cap-20261014/observed/demo-preflight-20261016.json),
[fleet observation](https://github.com/postfiatorg/postfiatl1v2/blob/6b4fa4b0596fecbdad663d9cddf902ae7b56d8b7/deployments/fastlane-cap-20261014/observed/observe-fleet-20261016.json)
at 10:56:22Z, PASS).

## 2026-10-20: issuer signing window

The founder signs the two issuer bridge policy updates on his own host. The window is 2026-10-20 12:00–16:00 UTC, A666 first, then BMNRC. The files are the [A666 update](https://github.com/postfiatorg/postfiatl1v2/blob/30c6dcd647ae1efe5b52061fa0321e21946e7a0b/deployments/fastlane-cap-20261014/observed/a666-bridge-policy-update.unsigned.json) (`24aa2e28…`) and the [BMNRC update](https://github.com/postfiatorg/postfiatl1v2/blob/30c6dcd647ae1efe5b52061fa0321e21946e7a0b/deployments/fastlane-cap-20261014/observed/bmnrc-bridge-policy-update.unsigned.json) (`5179d6f7…`), both to `authority_epoch` 2, root `72836827…`. The steps are in the [signing sheet](https://github.com/postfiatorg/postfiatl1v2/blob/30c6dcd647ae1efe5b52061fa0321e21946e7a0b/deployments/fastlane-cap-20261014/ISSUER-SIGNING-SHEET-20261020.md). At 10:13Z no transaction had been sent and the chain was at height 1131.

At 10:25Z the other lane's `navcoin-proof-watchdog@` user units, which watched only loopback forks, were stopped and disabled on validator-0 (3) and validator-3 (14), with the validator and RPC PIDs unchanged and the state and config files kept ([record](https://github.com/postfiatorg/postfiatl1v2/blob/819918d2b0cd5798f365bd1c6341537f11b64ead/deployments/fastlane-cap-20261014/observed/watchdog-units-stopped-20261020.json)).

The STO-07 fix (`8644f0af`) is on main, and the G4 re-run on the work server qualifies it: **PASS**, report `d1e9b39c…0ac0`, consensus ratio 1.0261, wallet 1.0326, stage p95 back at the August level ([result](../plans/active/g4-regression-deployed-release-20261019.md#result-2026-10-20-attempt-3-after-the-fix)). The devnet still runs `fastlane-cap-20261014` (`8bed9b90`), which does not have the fix, at height 1131 (last observed). The fix ships with the [next release](../plans/active/next-validator-release-plan-20261101.md#candidates). The re-run made no validator-host contact and sent no transaction, and nothing changed on the validators.

## 2026-10-19: G4 regression campaign (work server only)

The devnet was not touched: there was no validator-host contact and no transaction. The G4
regression run on candidate `29824ed7` reported **PUBLIC TESTNET BLOCKED**
([result](../plans/active/g4-regression-deployed-release-20261019.md#result-2026-10-19),
report `94188f3c…`). Since `f0013c29`, the store handle lease reopens and closes the redb
database on most lookups. Round stages are several times slower than in August, and
certified sends grow with height
([STO-07](../review/defect-inventory-20260910.md)). The devnet runs this code (`8bed9b90`),
so the cost is real there too. The chain is idle at height 1131, though, and nothing changes
on the validators today. The fix and one G4 re-run are in the
[next release plan](../plans/active/next-validator-release-plan-20261101.md#candidates).
