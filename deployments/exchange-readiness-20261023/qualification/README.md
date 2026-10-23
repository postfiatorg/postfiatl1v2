# Local qualification — exchange-readiness-20261023, 2026-10-23

**Local checks only.** Nothing ran on a validator host, no live RPC was called, no transaction was
sent and nothing was deployed. Source tip `6604c93e92c0bb5eac37837bb1669b987426469a` on
`release/exchange-readiness-20261023`. Rollback executable: the deployed `store-handle-20261021`
(`eea96ba6…`, copied from `~/.local/lib/postfiat/releases/store-handle-20261021/` after its hash was checked).

**Not done today: canary backup, fleet before-state, rollout, index build, activation**
(release day, [DEPLOY-SHEET.md](../DEPLOY-SHEET.md) §0–§9, under the operator's go).

**Re-qualified 2026-10-23 with the account history index fix** (`d9260e93` on main, merged into the
release branch as `6b1a0585`; executable `4085ac70…`, manifest `210544a8…`). The first cut's results
(source `6604c93e`, executable `59bcd5b5…`, manifest `be297bef…`, release commit `8b0ea1f8`) follow under
"Results (first cut)". Its index finding is fixed: see below.

## Re-qualification (`6b1a0585`)

Since `6604c93e`, `git diff 6604c93e..6b1a0585 -- crates/ Cargo.lock` is the fix only
(`crates/node/src/block_finality.rs`, two node test files). Also merged from main, none in the executable:
`9537af66` (storage milestone note), `3e956ff1` (this release's cut record), `ef7b406b` (a
`benchmarks/ai-governance` usage note). Records: [requal-6b1a0585/](requal-6b1a0585/).

**Cause.** A governance receipt's `tx_id` is its amendment id (`crates/node/src/execution_actions.rs:1182–1234`),
and a rejected governance operation may be retried under the same id in a later governance batch
(`crates/storage/src/lib.rs:2244`). The transactional store returns one receipt per linking block
(`receipts_in_block_order`, `crates/storage/src/transactional.rs:1124`), so on this devnet `219306e6…`
appears at heights 64 and 66 (both accepted; the known replay anomaly `WAN_DEVNET2_DEDUPLICATED_GOVERNANCE_RECEIPT`)
and `c9acb974…` at 940 and 941 (rejected, `vault_bridge_route_profile_rejected`) and 945 (accepted). No
non-governance id repeats in this history.

**Rule** (`receipt_by_tx_id`, `crates/node/src/block_finality.rs:972`; `tx`, `:40–90`): a repeated `tx_id` is
accepted only when every block linking it is a governance block and the receipt count equals the link count
(or is one, the legacy log's terminal receipt); `tx` then answers with the latest link and receipt. Governance
blocks have no `account_tx` rows, so no history row reads those receipts. Any other repeat is refused with the
same `receipt log contains duplicate tx_id` / `has multiple receipts` errors as before. Read path only: no
consensus, execution, state commitment or receipt log format change (the exact replays below reproduce the
recorded receipts root and state roots).

| Check | Result | Evidence |
|---|---|---|
| Fail-first tests | `repeated_governance_receipt_ids_build_the_index_and_answer_account_tx_and_tx` FAILED on `3e956ff1` code (the archive scan stops on the duplicate id) and passes with the fix; `repeated_ordinary_receipt_ids_are_still_rejected` passes before and after (ordinary and mixed governance/transparent repeats still refused by scan, build and `tx`). Focused node suites pass; `cargo fmt --check`, `cargo clippy -p postfiat-node --all-targets -D warnings` clean | [log](requal-6b1a0585/focused-suites.log) |
| Two clean builds of `6b1a0585` | PASS: both `4085ac70c7bb3c21b46cc67be8cb1bbb3556bfea9f49fbf6b128cdac2adcbef9`, `cmp` identical, GCC linker, no RPATH/RUNPATH, trees clean | [node-builds.json](../node-builds.json) |
| Stage, signed manifest, local preflight | PASS: manifest `210544a8…` signed by `pfc531e0…`, valid 1792734699–1824270999; `PASS_SIGNED_LOCAL_INPUTS_ONLY`. The 33 generated files are byte-identical to `8b0ea1f8`'s (only the manifest, its input and `node-builds.json` changed), so the rotation and rollback rehearsal of the first cut stands | [preflight](requal-6b1a0585/local-preflight-signed.json) |
| Canary 1131: signed import, tampered copy, checkpoint and full replay, new build; deployed `eea96ba6…` on the new build's import | PASS: signed import (42 s); a copy with a changed signature is rejected (exit 1); checkpoint and full replay at 1131, tip `9b054f09…`, root `a12e7c5f…` = recorded; the deployed build passes checkpoint and full replay on the new build's import. The new build's, the deployed build's and the first cut's full-replay reports agree in all 63 fields | [history-run.json](requal-6b1a0585/history-run.json) |
| Exact replay, height 924 (legacy source) | **PASS** with `4085ac70…`: 924, tip `ebeb0e1e…`, root `0854bc47…`; all 14 recorded manifest fields matched (972 receipts, receipts root `f2e27abe…`, ordered-history accumulator); verify-only trees unchanged; source tree `13ffe388…` before and after; the receipt equals the 2026-10-22 one in all 45 content fields | [replays/height-924/](requal-6b1a0585/replays/height-924/) |
| Exact replay, current head 1131 (signed archive) | **PASS** with `4085ac70…`: signed import, checkpoint and full replay at 1131, tip `9b054f09…`, root `a12e7c5f…` = the signed manifest `df9b3847…`; archive tree `be4e6ebb…` unchanged; receipt `f78946d5…` | [replays/height-1131/](requal-6b1a0585/replays/height-1131/) |
| Admission probe (loopback `rpc-serve`, fresh fixture) | PASS, unchanged: `testing` `pf6395ef…` admitted; 41 characters, `PF` prefix, `pf-exchange` → `invalid_recipient_address`; mixed-case checksum → `noncanonical_recipient_address` | [admission-result.json](requal-6b1a0585/admission-result.json) |
| Activation heights | PASS: none of the three set in the new build's 1131 import; both routes `authority_epoch` 1 | [import-state-1131.json](requal-6b1a0585/import-state-1131.json) |
| **Index build on the 1131 import, new executable** | **PASS**: `account-tx-index-build` 4.08 s for 1131 blocks (**3.61 s per 1,000 blocks**; 1,093 indexed blocks, 958 rows, 255 accounts and shards; a second build 3.99 s); `account-tx-index-status` both indexes usable, range 1–1131. `account_tx` (index used): `testing` 46 rows, faucet `pfcd4cc8…` 44 rows, each with the grants at 1124–1129 (1,000,000 atoms, accepted); the scan answers too (19 newest rows, truncated by its limit). `tx 219306e6…` → accepted, height 66, index 0; `tx c9acb974…` → accepted, height 945. With the first cut and the deployed build the same `tx` answers `has multiple receipts` | [index-fix.json](requal-6b1a0585/index-fix.json) |

## Results (first cut, `6604c93e`)

| Check | Result | Evidence |
|---|---|---|
| CI on the tip (`gh run list --commit 6604c93e…`, read 05:15Z) | `docs-build` 39903101066 success; `rust-ci` 39903101023 (`check`, `test` = full workspace suite) and `product-security-ci` 39903101051 **in progress** at the cut (not waited for). Last fully green combined source `7e7169b6`/`124a187b`: `rust-ci` 39864672912 incl. `test`, `product-security-ci` 39864672901, `docs` 39864672913. The release branch has no runs | [ci-main.json](ci-main.json) |
| Review follow-ups on main (`6604c93e`) | XR-02: `verify_mempool_amount_counts_issued_payment_v2_like_issued_payment` failed before the arm, passes after; XR-01: `orchard_deposit_funding_transfer_stays_on_plain_execute_transfer` passes; `cargo fmt --check` and `cargo clippy -p postfiat-node --all-targets -D warnings` clean | [review](../../../docs/review/exchange-readiness-merge-review-20261023.md) |
| Two clean builds, separate worktrees and copied caches | PASS: both `59bcd5b5d021b8765f89b404e2327dd6dc1f8daa2645a1e1d487fb50d80f206e`, `cmp` identical, GCC linker, no RPATH/RUNPATH, trees clean after build | [node-builds.json](../node-builds.json) |
| Canary 1131 (signed, validator-1, 2026-10-21, manifest `df9b3847…`): signed import, new build | PASS; a copy with a changed signature is rejected (exit 1; the raw report says FAIL, the check expects the refusal) | [history-run.json](history-run.json) |
| Canary 1131: checkpoint and full replay, new build | PASS ×2: 1131, tip `9b054f09…`, root `a12e7c5f…` = recorded backup identity | same |
| Canary 1131: rollback build `eea96ba6…` on the data the new build imported | PASS: checkpoint and full replay, same identity | same |
| Canaries 1123 (`d957141e…`), 1115 (`49b2b6cd…`), 1050 (`a40d6a99…`): signed import, checkpoint and full replay, new build | PASS ×3, each = its recorded backup identity: 1123 (tip `bafcc6ef…`, root `942d1fc2…`), 1115 (`28e1175b…`, `a5287eaf…`), 1050 (`03a24230…`, `13d9e652…`) | same |
| The six 2026-09-28 rotated copies at 1021 (scratch `cp -a` of `~/.cache/release-repair-20260928/working`) | PASS ×6: full replay at 1021, tip `35ab6fbc…`, root `b01a98e5…` = the 2026-09-28 rotation receipt | same |
| **Exact replay, height 924** (`run_replay_evidence.py --legacy-source-dir`, scratch `cp -a` of the g6 validator-0 backup, recorded manifest `storage-migration-manifest.json` `4119fb33…`) | **PASS**: 924, tip `ebeb0e1e…`, root `0854bc47…`; every recorded manifest field matched (block, receipt (972), archive and ordered-batch counts and roots, ordered-history accumulator `8d38f779…`, current state root); verify-only clone and target trees unchanged; source tree `13ffe388…` before and after. The receipt equals the 2026-10-22 deployed-binary receipt in all 45 content fields (receipts root `f2e27abe…` included); only binary identity differs | [replays/height-924/](replays/height-924/) |
| **Exact replay, current head** (`--signed-archive-dir`, scratch copy of the newest signed export on this server, the 1131 canary) | **PASS**: signed import, `verify-finalized-checkpoint` and full `verify-state` at 1131, tip `9b054f09…`, root `a12e7c5f…`, each equal to the signed manifest `df9b3847…`; archive tree `be4e6ebb…` unchanged; receipt `58223162…`. The new and the deployed executable's full replays of this import agree in all 63 report fields. The live 1137 head is replayed at sheet §4 | [replays/height-1131/](replays/height-1131/) |
| Six loopback services of the new build on the six copies, both start orders, startup and restart | PASS: all six agree at 1021 (`35ab6fbc…`, `b01a98e5…`), mempools empty, after startup and after restart | [new-build-service-receipt.json](new-build-service-receipt.json) |
| Rollback: `store-handle-20261021` on the data the new build's services ran on | PASS: checkpoint ×6 and full replay of validator-0 at 1021; `rollback-one.sh` checks (checkpoint, identity) on all six. `validate-local-keys` not run: the archived copies have no faucet key file | [rollback-run.json](rollback-run.json), [rollback core](logs/rollback-core.stdout) |
| Rollback after `account-tx-index-build` (index schema v2) | The old executable still reads the data dir (`status`, `verify-state` pass), reports the v2 index unusable (`unsupported account_tx index schema`) and answers `account_tx` from the scan; its own `account-tx-index-build` writes a usable v1 index; the new build then reports v1 unusable and rebuilds v2. Rollback after §7 is safe; rebuild the index with the old executable for indexed reads. On a fresh fixture chain, because devnet history fails the build (above) | [index-rollback.json](index-rollback.json) |
| Stage, signed manifest, local preflight with signed gate | PASS: manifest `be297bef…` signed by `pfc531e0…` (key on this server), valid 1792732134–1824268434; 33 generated files; `PASS_SIGNED_LOCAL_INPUTS_ONLY` for all six. As before, `stage.sh`'s direct `deployment-manifest-verify` stops at validator-0 because the bindings name host paths; the preflight verifies through the rootfs | [local-preflight-signed.json](local-preflight-signed.json), [stage log](logs/stage.log) |
| Generated units vs `store-handle-20261021` | Byte-identical after the release-ID change (33 files, 0 differences): `--max-requests 100000` (6 RPC units), `RestartSec=1`, `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD` (12 env files) kept | [diff](logs/stage-vs-20261021.diff) |
| `systemd-analyze verify` (systemd 259) on the 12 units | PASS: no finding for any unit (8 unrelated dracut lines from the copied host unit directory) | [log](logs/systemd-analyze-verify.log) |
| **Admission change** (loopback `rpc-serve` of the new build on a fresh `init` fixture, `postfiat-local-admission`) | PASS: `mempool_submit_signed_transfer` to the devnet `testing` wallet `pf6395ef…` (review table: pass) admitted; refused before the mempool: 41 characters and `pf-exchange` → `invalid_recipient_address`, `PF` prefix → `invalid_recipient_address`, valid mixed-case checksum → `noncanonical_recipient_address`. Only the canonical transfer is pending | [admission-result.json](admission-result.json) |
| **Activation heights inactive by default** | PASS: in the 1131 import (new build) no amendment of the three kinds exists, so `recipient_format`, `rejected_tx_fee`, `require_memo` are unset; both routes `authority_epoch` 1 ([read-import-state.py](../read-import-state.py)); tests `rejected_tx_fee_and_require_memo_activation_heights_are_independent_and_latest_wins`, `recipient_format_activation_is_absent_for_legacy_and_cannot_be_postponed` pass | [import-state-1131.json](import-state-1131.json) |
| **account-tx-index-build on a scratch copy of the signed 1131 import** | **FAIL (pre-existing)**: build exits 1 after 3.6 s with the duplicate governance `tx_id` (above); status stays "absent"; `account_tx` fails the same way; the deployed build fails identically. No timing per 1,000 blocks (no successful build on devnet data). Memo rows: the devnet has no `payment_v2` memo through 1131; covered by `payment_v2_memo_contents_flow_through_tx_scan_and_index`, `account_tx_index_rebuild_backfills_memos_and_shielded_withdrawals_over_a_v1_index` and `exchange_rpc_history` | [index-check.json](index-check.json) |
| Focused suites | PASS: node lib 14 (all `rejected_fee_require_memo`, `recipient_format_activation` incl. XR-01, `account_tx_exchange_history`, XR-02), node `exchange_rpc_history` 1, rpc_sdk `response_validation` 8, execution `recipient_format` 5, types activation defaults 2, reserve builders 8 + 14 (temporary keys) | [log](logs/focused-suites.log) |
| Full workspace suite | Not run here (no Orchard boundary changed); CI `rust-ci` `test` is the gate (above) | — |

## After rollout

Live, read-only checks after the six applies on 2026-10-23 (sheet §6). Records:
[after-checks.json](../observed/after-checks.json), [after.json](../observed/after.json),
[rollout record](../observed/rollout-record.json).

| Check | Result |
|---|---|
| After state (07:40Z) | Six on `exchange-readiness-20261023` at height 1143, tip `7fef75aaebc5…`, root `8c3a32fd0202…`; mempools empty |
| Applies | validator-1, 0, 2, 3, 4, 5, 07:10:57–07:37:03Z, one at a time; each restarted at the converged tip; no rollback |
| Faucet grants | One 1 PFT grant (`pfcd4cc8…` → `testing` `pf6395ef…`, 32 atoms fee) after each apply, each certified at view 0 with quorum 5 of 6 and the same block, tip and root on all six: 1138 (proposer validator-4), 1139 (5), 1140 (0), 1141 (1), 1142 (2), 1143 (3) |
| `demo-preflight.py` | READY: 60 PASS, 0 ATTENTION, 0 FAIL ([output](../observed/demo-preflight-after.json)) |
| Running executables | All 12 validator and RPC processes on `4085ac70…` |
| `status` | `deployment_manifest_verified=true`, build `6b1a0585`, manifest `210544a8…` on all six |
| RPC units (`systemctl cat`) | `--max-requests 100000`, `Restart=always`, `RestartSec=1`, ExecStart on this release on all six |
| Event logs | Each host's transport log gained one accepted-batch event for every grant height it did not propose |
| Activation heights | Inactive on all six: blocks 1138–1143 are transparent (no governance block since the signed 1137 canary, which has none of the three kinds) |
| `account_tx` index | Absent on all six (index build is §7); `account_tx` on validator-1 answers from the archive scan with the new executable (`testing`: grant rows through 1143) |
| Admission probe (validator-0 RPC, never admissible) | Malformed recipient → `invalid_recipient_address` at the destination check; canonical recipient passes it and is refused by the protocol check before any lock or round; mempool stays empty |
| Disk free `/` | validator-0 15.8 GB, validator-1 16.8 GB, validator-2 22.4 GB, validator-3 27.3 GB, validator-4 22.7 GB, validator-5 25.8 GB |
| Not run in this step | `account-tx-index-build` (§7), governance activation (§8) |

## Method

The store-handle-20261021 method ([packet](https://github.com/postfiatorg/postfiatl1v2/blob/release/store-handle-20261021/deployments/store-handle-20261021/qualification/README.md)) with these differences:

- Same build command, `SOURCE_DATE_EPOCH=1789514690`, `/usr/bin/gcc` linker, remap scheme with this run's directories and copies of the 2026-09-28 release-profile cache; `CARGO_BUILD_JOBS=6`.
- Signed archives: the 2026-10-21 canary at 1131 (newest; tamper and rollback checks), 1123, 1115 and 1050. The live 1137 head gets its own canary and exact replay at sheet §4.
- The exact replays ran from the clean build tree `source-1` at `6604c93e` with `target-1/release/postfiat-node` (the harness requires a `target/release` path; same hash as both candidates). Scratch copies only; the sources' tree hashes are unchanged. `local-paths.private.json` is not in the packet.
- The governed rotation still needs the deleted 1020 originals; six new-build services on the rotated 2026-09-28 copies stand in for it, as on 2026-10-21.
- Focused suites ran in the default test profile of the main checkout at `6604c93e`.

Drivers are in [`tools/`](tools/); per-command records are in `reports/` and `logs/`.
Disposable copies stay under `~/.cache/release-exchange-readiness-20261023`.
`SHA256SUMS` covers every packet file except itself.

## Task Node

First cut: task `task_660fb846a75c2cdbe8262156b20a3ecf` (rewarded). Re-qualification: task
`task_5bb9dfd7369cf5949e2acd7eeecefb45` (request `req_e60e9294…`), generated by Task Node, inspected and
accepted; evidence is submitted after the push.
