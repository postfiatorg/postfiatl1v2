# Local qualification — fastlane-cap-20261014, 2026-10-15

**Local checks only.** Nothing ran on a validator host, no live RPC was called,
no transaction was sent and nothing was deployed. Source tip
`8bed9b9073f97d0c45d0babb9756636516a2e3f7` on `release/fastlane-cap-20261014`.
Rollback executable: the deployed `signer-rotation-20261007` (`decaa411…`, copied from
`~/.local/lib/postfiat/releases/signer-rotation-20261007/` after its hash was checked).

Not done today: canary backup, fleet before-state, rollout, bridge activation.

## Results

| Check | Result | Evidence |
|---|---|---|
| CI on the tip (`gh run list --branch main`, 10:04Z) | `docs-build` success; `product-security-ci` running with `rust-supply-chain` **success** (run 38713910871, started 09:34:16Z, after #123 merged at 09:13:15Z) and every other job success except `open-reserve-proof-kit` (running); `rust-ci` `check` success, `test` running (run 38713910738). No failed workflow on `8bed9b90`. Last finished `rust-ci` `test`: success on `cc297fa3` (09:13Z, contains #123). The release branch has no runs | [ci-main.json](ci-main.json), [ci-jobs.txt](ci-jobs.txt), [ci-release.json](ci-release.json) |
| Two clean builds, separate worktrees and copied caches | PASS: both `b1b3dbb150d3596577df3bb3fe145103d90988cf71558c4d34a035fe4bb44405`, `cmp` identical, GCC linker, no RPATH/RUNPATH, trees clean after build | [node-builds.json](../node-builds.json) |
| Canary 1115 (signed, validator-1, 2026-10-09, manifest `49b2b6cd…`): signed import, new build | PASS; a copy with a changed signature is rejected (exit 1) | [history-run.json](history-run.json) |
| Canary 1115, checkpoint verification and full-history replay, new build | PASS ×2: 1115, tip `28e1175b…`, root `a5287eaf…` = recorded backup identity | same |
| Canary 1115, rollback build (`decaa411…`) on the data the new build imported | PASS: checkpoint and full replay, same identity | same |
| Canary 1050 (signed, 2026-09-28): signed import, checkpoint and full replay, new build | PASS: 1050, tip `03a24230…`, root `13d9e652…` = recorded | same |
| Six original copies, full-history replay through block 1011, new build | PASS ×6: 1020, tip `9d02b8ee…`, root `587c6526…` = recorded (2026-09-16 receipt) | same |
| Governed rotation on six disposable copies, new build | PASS: both start orders, rotation accepted on all six at 1021, convergence and restart | [receipt](v2-service-receipt.json), [log](logs/governance-gate.stdout) |
| Fresh V2 copies, full-history replay, new build | PASS ×6: 1021, tip `7f730afd…`, root `8d2e654d…` = the rotation receipt | [history-run.json](history-run.json) |
| Rollback: `signer-rotation-20261007` on data written by the new build | PASS: checkpoint ×6 and full replay of validator-0 at 1021 (`7f730afd…`, `8d2e654d…`); `rollback-one.sh` checks (checkpoint, identity) on all six. `validate-local-keys` not run: the archived copies have no faucet key file | same, [rollback core](logs/rollback-core.log) |
| **Anchor size (the reason for this release)**: the real five-vote `AnchorCheckpoint` (70,944 B, SHA-256 `6dddedd5…`) and a 176,181 B `ActivateCommittee` | PASS. `postfiat-rpc-sdk validate-request --expect-kind mempool_submit_fastlane_primary`: both accepted, 262,145 B refused. rpc-serve (new build, read-only, loopback, disposable 1050 copy), method `mempool_submit_fastlane_primary_finality`: both pass the parameter check (answer `rpc_method_not_allowed` from the read-only server), 262,145 B refused with `must not exceed 262144 bytes`. The deployed build refuses all three with `must not exceed 65536 bytes`, the 2026-10-12 refusal | [anchor-check.json](anchor-check.json), [anchor-check/](anchor-check/) |
| `deployment_manifest_verified` from the verified record | PASS: `false` without the variable and without the record, `true` after `deployment-manifest-verify` wrote it, `false` for a record with a changed manifest hash | [log](logs/verified-record.log) |
| Stage, signed manifest, local preflight with signed gate | PASS: manifest `4f379c57…` signed by `pfc531e0…` (key on this server), 33 generated files, four circuit artifacts, `PASS_SIGNED_LOCAL_INPUTS_ONLY` for all six. As on 2026-10-06, `stage.sh`'s direct `deployment-manifest-verify` stops at validator-0 because the bindings name host paths; the preflight verifies through the rootfs | [local-preflight-signed.json](local-preflight-signed.json), [stage log](logs/stage.log) |
| Generated units vs `signer-rotation-20261007` | Byte-identical after the release-ID change (`diff -r` empty): `--max-requests 100000`, `RestartSec=1`, `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD` kept | [diff](logs/stage-vs-20261007.diff) |
| `systemd-analyze verify` (systemd 259) on the 12 units | PASS: no finding for any unit (8 unrelated dracut lines from the copied host unit directory) | [log](logs/systemd-analyze-verify.log) |
| Focused suites for this release's changes | PASS: `rpc_sdk` lib 191 (includes `fastlane_primary_json_cap_admits_signer_rotation_control_transactions`), types `fastpay_recovery` 10, node lib (FastSwap control signing, storage migration, Cobalt handoff and rehearsal, swap service, `latest_tx_id`) 43, node bin (rpc-serve request tests, child id, JSON storage, store migrate) 41, node bin `completed_index` 28 (2 ignored; both SMG-07 tests), execution `fastlane_primary` 6 | [focused-suites.json](focused-suites.json) |
| Previous release's focused suites (quick subset) | PASS: node lib `account_tx`, FastPay epoch, FastSwap legacy, Cobalt shadow 29; Python `account_tx` fallback 11 | same |
| Full workspace suite | Not run here (no Orchard boundary changed). CI `rust-ci` `test` on `8bed9b90` was still running at 10:04Z; read it before GO 0 | [ci-main.json](ci-main.json) |

## After rollout

Live, read-only checks after the six applies on 2026-10-15 (sheet §6). Records:
[after-checks.json](../observed/after-checks.json), [after.json](../observed/after.json),
[rollout record](../observed/rollout-record.json).

| Check | Result |
|---|---|
| After state (11:07:27Z) | Six on `fastlane-cap-20261014` at height 1129, tip `41154d2c4121…`, root `053fb35c15e5…`; mempools empty |
| Faucet grants | One 1 PFT grant after each apply, each certified at view 0 with the same block, tip and root on all six: heights 1124–1129 |
| `demo-preflight.py` (11:07:33Z) | READY: 60 PASS, 0 ATTENTION, 0 FAIL ([output](../observed/demo-preflight-after.json)) |
| Running executables | All 12 validator and RPC processes on `b1b3dbb1…` |
| `status` | `deployment_manifest_verified=true` on all six; manifest `4f379c57…` |
| RPC units (`systemctl cat`) | `--max-requests 100000`, `Restart=always`, `RestartSec=1` on all six |
| Event logs | Each host's transport log gained one accepted-batch event for every grant height it did not propose |
| Disk free `/` | validator-0 16.4 GB, validator-1 17.9 GB, validator-2 22.9 GB, validator-3 27.8 GB, validator-4 23.2 GB, validator-5 25.9 GB |
| Anchor size (the real 70,944 B `AnchorCheckpoint`) | Offline: `postfiat-rpc-sdk validate-request` (8bed9b90 build) accepts it. Live: not exercised. The live RPC units allow `mempool_submit_fastlane_primary_finality`, the method has no dry-run and rpc-serve dispatches right after its parameter check, so a valid request would be submitted. Left to §7.1 |

## Method

The signer-rotation-20261007 method ([packet](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/qualification/README.md)) with these differences:

- Same build command, `SOURCE_DATE_EPOCH=1789514690`, `/usr/bin/gcc` linker, remap scheme with this run's directories and copies of the 2026-09-28 release-profile cache; `CARGO_BUILD_JOBS=6`.
- The newest signed archive is the 2026-10-09 canary at 1115; the fleet is at 1123 (two blocks of the 2026-10-12 activation). The sheet's §4 backup closes that gap. The saved V2 copies of 2026-09-28 (validators 4 and 5) are no longer on this server; the 1115 canary replaces them.
- The `ActivateCommittee` input is shaped like `synthetic_activate_committee_json` in `crates/rpc_sdk/src/protocol_request_tests.rs` (synthetic keys and votes, the real checkpoint, the real epoch-2 root); its node-side parse is covered by that unit test. rpc-serve ran without `--allow-mempool-submit*` and without a finality topology, so nothing could be admitted or sent; it was killed after the requests.
- Focused suites ran in the default test profile of the main checkout at `8bed9b90`.

Drivers are in [`tools/`](tools/); per-command records are in `reports/` and `logs/`.
Disposable copies stay under `~/.cache/release-fastlane-cap-20261014`.
`SHA256SUMS` covers every packet file except itself.

## Task Node

Task `task_ec08d09a4c6244dba9608dca9f1c3564` (request `req_68e67401…`), generated by Task Node, inspected and accepted. Evidence is submitted after the push.
