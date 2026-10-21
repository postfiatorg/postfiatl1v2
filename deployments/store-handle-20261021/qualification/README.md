# Local qualification — store-handle-20261021, 2026-10-21

**Local checks only.** Nothing ran on a validator host, no live RPC was called,
no transaction was sent and nothing was deployed. Source tip
`86706699f646405743385beabf609c6771a518a3` on `release/store-handle-20261021`.
Rollback executable: the deployed `fastlane-cap-20261014` (`b1b3dbb1…`, copied from
`~/.local/lib/postfiat/releases/fastlane-cap-20261014/` after its hash was checked).

**Not done today: canary backup, fleet before-state, rollout, migration** (release day,
[DEPLOY-SHEET.md](../DEPLOY-SHEET.md) §0–§8, under the operator's go).

## Results

| Check | Result | Evidence |
|---|---|---|
| CI on the tip (`gh run list --commit 86706699…`, read 10:25Z) | All green: `docs-build` 39433228893, `rust-ci` 39433228794 (`check`, `test`), `product-security-ci` 39433228932 (all seven jobs). `rust-ci` `test` runs `cargo test --workspace --locked`, so it is the full workspace suite at the cut commit (13:45–15:07Z, 2026-10-20). The release branch has no runs | [ci-main.json](ci-main.json), [ci-jobs.txt](ci-jobs.txt), [ci-release.json](ci-release.json) |
| Scope | `git log 8bed9b90..86706699 -- crates/`: `crates/node` and `crates/storage` only (26 files); no consensus or execution crate, no `Cargo.lock` change | [README](../README.md#scope-since-8bed9b90) |
| Two clean builds, separate worktrees and copied caches | PASS: both `eea96ba6761ed223047d7781663100d34efe5641d36031bd50874bd9c8b41f36`, `cmp` identical, GCC linker, no RPATH/RUNPATH, trees clean after build | [node-builds.json](../node-builds.json) |
| Canary 1123 (signed, validator-1, 2026-10-15, manifest `d957141e…`): signed import, new build | PASS; a copy with a changed signature is rejected (exit 1; its raw report says FAIL, the check expects the refusal) | [history-run.json](history-run.json) |
| Canary 1123, checkpoint verification and full-history replay, new build | PASS ×2: 1123, tip `bafcc6ef…`, root `942d1fc2…` = recorded backup identity | same |
| Canary 1123, rollback build (`b1b3dbb1…`) on the data the new build imported | PASS: checkpoint and full replay, same identity | same |
| Canary 1115 (signed, 2026-10-09, manifest `49b2b6cd…`): signed import, checkpoint and full replay, new build | PASS: 1115, tip `28e1175b…`, root `a5287eaf…` = recorded | same |
| Canary 1050 (signed, 2026-09-28): signed import, checkpoint and full replay, new build | PASS: 1050, tip `03a24230…`, root `13d9e652…` = recorded | same |
| The six 1020 originals | **Gone** (`~/.cache/combined-release-20260915` deleted). Used instead: the six 2026-09-28 copies (`~/.cache/release-repair-20260928/working`), already rotated at 1021 that day. Full-history replay, new build: ×6 at 1021, tip `35ab6fbc…`, root `b01a98e5…` = the 2026-09-28 rotation receipt (`release-repair-20260928/v2-service-receipt.json`); PASS ×6 against that receipt (the driver first compared them with the 1020 identity; `reports/original-validator-*.json` keep that raw FAIL, `history-run.json` carries the comparison with the receipt and a note) | [history-run.json](history-run.json) |
| Governed rotation on six disposable copies | **Not run**: it needs the 1020 originals; the 2026-09-28 copies already carry the rotation (`governance-authorization-sign`: slot 1021 does not match next height 1022) | [logs/governance-sign-0.stderr](logs/governance-sign-0.stderr) |
| Six loopback services of the new build on the six copies, both start orders, startup and restart | PASS: all six agree at 1021 (`35ab6fbc…`, `b01a98e5…`), mempools empty, after startup and after restart | [new-build-service-receipt.json](new-build-service-receipt.json) |
| Rollback: `fastlane-cap-20261014` on the data the new build's services ran on | PASS: checkpoint ×6 and full replay of validator-0, all at 1021 (`35ab6fbc…`, `b01a98e5…`); `rollback-one.sh` checks (checkpoint, identity) on all six. `validate-local-keys` not run: the archived copies have no faucet key file | [rollback-run.json](rollback-run.json), [rollback core](logs/rollback-core.stdout) |
| `deployment_manifest_verified` from the verified record | PASS: `false` without the variable and without the record, `true` after `deployment-manifest-verify` wrote it, `false` for a record with a changed manifest hash | [log](logs/verified-record.log) |
| Stage, signed manifest, local preflight with signed gate | PASS: manifest `8ce1da30…` signed by `pfc531e0…` (key on this server), 33 generated files, four circuit artifacts, `PASS_SIGNED_LOCAL_INPUTS_ONLY` for all six. As on 2026-10-15, `stage.sh`'s direct `deployment-manifest-verify` stops at validator-0 because the bindings name host paths; the preflight verifies through the rootfs | [local-preflight-signed.json](local-preflight-signed.json), [stage log](logs/stage.log) |
| Generated units vs `fastlane-cap-20261014` | Byte-identical after the release-ID change (`diff -r` empty): `--max-requests 100000`, `RestartSec=1`, `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD` kept | [diff](logs/stage-vs-20261014.diff) |
| `systemd-analyze verify` (systemd 259) on the 12 units | PASS: no finding for any unit (8 unrelated dracut lines from the copied host unit directory) | [log](logs/systemd-analyze-verify.log) |
| **Store-handle fix (STO-07, `8644f0af`)** | G4 re-run of 2026-10-20 **PASS** (`~/repos/postfiat-storage-g4-regression-20261020/campaign-report.json`, sha256 `d1e9b39c6498558da7c2bd3aa979ec2a71fd6cffd0a3c9abe376c513ebb0aac0`, re-hashed today, equal to the [campaign plan](../../../docs/plans/active/g4-regression-deployed-release-20261019.md#result-2026-10-20-attempt-3-after-the-fix)). Its three tests pass: `transactional_six_validator_round_opens_each_database_a_bounded_number_of_times`, `held_store_reuses_its_validated_pointer_binding_until_it_closes`, `rpc_reader_opens_the_database_between_two_operations` | [focused-suites.json](focused-suites.json) |
| **`fastswap-committee-migrate`**: its own suite | PASS: `fastswap_service::` 39 (all 15 `committee_migration_tests`, including `committee_migrate_reads_the_real_release_packet_files`: real epoch-2 root `72836827…`, anchor `355fe608…`, and the #134 tests) | same |
| **`fastswap-committee-migrate`**: release executable on scratch stores built as the 2026-10-16 tests build them (`node()`: drained, fenced, anchored epoch-1 store; epoch 2 activated in the ledger) | PASS. Snapshot store: dry run `would-migrate`, nothing written; real run `migrated`, `service open: committee epoch 2, root b0958edd…, enabled true`, 0600 files, backup with manifest; a second run is refused (store already at epoch 2). Validator-5 shape (base only, fence and anchor never applied): the same, with the note that the replayed state equals the anchored drained state. Runbook order with the backup directory pre-created by `install -d -m 700`: `would-migrate`, then `migrated`, five files backed up | [migration/](migration/) |
| `fastswap-committee-migrate` with the real 2026-10-15 files on a scratch store | Refused before any write, as it must be: `committee file is epoch 2 on another chain or not the epoch after the store's epoch 1` (the files are for `postfiat-wan-devnet-2` and the real drained state, which no local store holds); store unchanged, no backup. Files copied to [observed/bridge-activation/](../observed/bridge-activation/), identical to the test fixtures (`35acb2ff…`, `4497c495…`). The real run against a validator's store is §7 | [migration/migration-runs.json](migration/migration-runs.json) |
| Focused suites for this release's changes | PASS: storage `fastswap_store` 25, storage `transactional` 36 (both lease tests), node bin `fastswap_service::` 39, store-handle six-validator round 1, operator-attestation routing (#136) 1, `pftl_swapctl` (#135) 1 | [focused-suites.json](focused-suites.json) |
| Full workspace suite | Not run here (no Orchard boundary changed); CI `rust-ci` `test` on `86706699` (above) is the gate | [ci-jobs.txt](ci-jobs.txt) |

## After rollout

Live, read-only checks after the six applies on 2026-10-21 (sheet §6). Records:
[after-checks.json](../observed/after-checks.json), [after.json](../observed/after.json),
[rollout record](../observed/rollout-record.json).

| Check | Result |
|---|---|
| After state (11:43:17Z) | Six on `store-handle-20261021` at height 1137, tip `18a4dc869b52…`, root `1277c76c16af…`; mempools empty |
| Faucet grants | One 1 PFT grant after each apply, each certified at view 0 with the same block, tip and root on all six: heights 1132–1137 |
| `demo-preflight.py` (11:43:22Z) | READY: 60 PASS, 0 ATTENTION, 0 FAIL ([output](../observed/demo-preflight-after.json)) |
| Running executables | All 12 validator and RPC processes on `eea96ba6…` |
| `status` | `deployment_manifest_verified=true` and `build_git_revision` `86706699` on all six; manifest `8ce1da30…` |
| RPC units (`systemctl cat`) | `--max-requests 100000`, `Restart=always`, `RestartSec=1` on all six |
| Event logs | Each host's transport log gained one accepted-batch event for every grant height it did not propose |
| Store handle (STO-07) | Every batch the new executable accepted reports `storage_work.database_opens=0`; the `fastlane-cap-20261014` executable did not report the field |
| `fastswap-committee-migrate` | Listed in `postfiat-node --help` on all six hosts; not run (sheet §7) |
| Disk free `/` | validator-0 16.0 GB, validator-1 17.3 GB, validator-2 22.5 GB, validator-3 27.4 GB, validator-4 22.9 GB, validator-5 25.5 GB |

## Method

The fastlane-cap-20261014 method ([packet](https://github.com/postfiatorg/postfiatl1v2/blob/release/fastlane-cap-20261014/deployments/fastlane-cap-20261014/qualification/README.md)) with these differences:

- Same build command, `SOURCE_DATE_EPOCH=1789514690`, `/usr/bin/gcc` linker, remap scheme with this run's directories and copies of the 2026-09-28 release-profile cache; `CARGO_BUILD_JOBS=6`.
- Signed archives on this server: the 2026-10-15 canary at 1123 (`~/.postfiat/deployments/fastlane-cap-20261014/evidence/pre-rollout-backup/backup-signed`, the one `backup-reverify` re-imported), 1115 and 1050. The fleet is at 1131 (anchor and `ActivateCommittee`); the sheet's §4 backup closes that gap.
- The 1020 originals are deleted, so the governed rotation was replaced by six new-build services on the 2026-09-28 rotated copies (startup and restart, no block written). The rollback build then checked those copies.
- Scratch stores for the migration came from a temporary `#[ignore]` test calling `node()` in `fastswap_committee_migration_tests.rs`, run in the main checkout and removed afterwards ([tools/scratch_store_test.rs](tools/scratch_store_test.rs)); the release executable ran the command.
- Focused suites ran in the default test profile of the main checkout at `86706699`; the FastSwap service and migration tests live in the `postfiat-node` bin target.

Drivers are in [`tools/`](tools/); per-command records are in `reports/` and `logs/`.
Disposable copies stay under `~/.cache/release-store-handle-20261021`.
`SHA256SUMS` covers every packet file except itself.

## Task Node

Task `task_fbead19768bb3fac38e418e864dbf5e0` (request `req_d2ab7f17…`), generated by Task Node, inspected and accepted. Evidence is submitted after the push.
