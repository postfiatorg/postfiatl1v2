# FastLane-cap release inputs — cut 2026-10-15, rollout 2026-10-15 at the operator's go

Release `fastlane-cap-20261014` is `main` at `8bed9b90` (branch
`release/fastlane-cap-20261014`): the deployed `signer-rotation-20261007`
(`0fa55d0b`, executable `decaa411…`) plus the changes below. It carries what the
bridge half of the signer-group activation needs: an RPC size cap that admits the
five-vote `AnchorCheckpoint` (70,944 B) and the `ActivateCommittee` certificate
(~176 KB), and finality reports that match each operation's receipt codes
([plan](../../docs/plans/active/next-validator-release-plan-20261101.md)).
Executable `b1b3dbb150d3596577df3bb3fe145103d90988cf71558c4d34a035fe4bb44405`
(two identical clean builds). Signed manifest `4f379c57…`, publisher `pfc531e0…`.
Rollback release: `signer-rotation-20261007` (`decaa411…`). Identities:
[RELEASE-ID.txt](RELEASE-ID.txt).

## Status

**Cut, not deployed.** Local qualification passed on 2026-10-15
([qualification packet](qualification/README.md)). Not done today: canary backup,
fleet before-state, rollout, bridge activation ([DEPLOY-SHEET.md](DEPLOY-SHEET.md)).

## Scope since `0fa55d0b`

Every crate file that differs between `0fa55d0b` and `8bed9b90` is touched by one of
these changes (`git diff --name-only 0fa55d0b 8bed9b90 -- crates/ Cargo.lock`, 49 files).

| Commit | Change |
|---|---|
| `7f9d0d98` | `fastlane_primary_json` cap 64 KiB → 256 KiB (`MAX_RPC_FASTLANE_PRIMARY_JSON_BYTES`); finality reports check each operation's accepted receipt code; `--fastlane-primary-file`. |
| `97bb4d35` (PR #123) | `Cargo.lock` rustls 0.23.45 → 0.23.46 (RUSTSEC-2026-0346 to -0349). |
| `9e9d45d2` (PR #87) | rpc-serve refuses `owned_certificate` with both selectors. |
| `af346e00` (PR #89) | rpc-serve refuses wrong-typed `fastswap_objects` params. |
| `b993c0ed` (PR #90) | rpc-serve strict optional params for `fastswap_policy` and `fastswap_checkpoint_status`. |
| `38a10d54` (PR #91) | rpc-serve refuses wrong-typed `transfer_fee_quote` optional params. |
| `5270b8c1` (PR #92) | rpc-serve refuses wrong-typed proxy parent-wait params. |
| `926d8a46` (PR #93) | rpc-serve refuses root/parent-hash pins sent without the height. |
| `d51f6ae7` (PR #56) | rpc-serve makes relative `--data-dir`/`--spool-dir` absolute; a worker reply under another id is an `rpc_child_id_mismatch` error. |
| `a7a5a027` (PR #109) | rpc-serve fails closed on any other worker id mismatch. |
| `16c3c0ef` (PR #62) | Capped manifest, checksum and index reads; relative output preflight (SMG-05/06). |
| `df89feac` (PR #110) | Bounded index reads open with `O_NOFOLLOW` (SMG-05). |
| `a9bbf7c9` (PR #122) | Interrupted-prune recovery syncs both directories before the index entry is removed (SMG-07). |
| `a243aaf2` (PR #60) | Cobalt rehearsal: strict digests and attestation timestamps (CHO-06, SWP-05). |
| `0b49ac26` (PR #63) | Cobalt rehearsal: negative probes keep the approvals they test (CHO-05). |
| `14850a7e` (PR #61) | Identical timing replay at the 64-stage cap is a no-op; `latest_tx_id` follows the selection order (SWP-04, MPL-02). |
| `9157e7cd` (PR #84) | `FastPayRecoveryCapabilitiesV1::validate()` requires the exact quorum (`crates/types`); `owned_recovery_capabilities` in `rpc_sdk`. |
| `b39589b3`, `692779e7`, `92780e28`, `995f3046`, `fce1d39e`, `ab2792ae`, `3d8c4e9c`, `14d2c80f`, `417d32b2` (PRs #74–#77, #81–#83, #85, #80) | `rpc_sdk` request builders and validators for existing read methods. rpc-serve runs `validate_protocol` from this crate, so their params rules also apply there. |
| `12b83c86`, `ed42d646`, `0b3ec7bf`, `47874faf`, `3d8dd100`, `01652996` (PRs #78, #86, #79) | `postfiat-rpc-sdk` CLI only: validate before write, request-file binding, one method list. |
| `c377bfc5`, `9fe08cec` (PR #116) | `rpc_sdk` refuses mismatched nested asset and escrow result ids. |
| `8bed8159` (PR #106) | `rpc_sdk` FastSwap TCP connect bounded by the transport timeout. |
| `51144786` | `account_tx` archive-scan regression test; Python fallback only otherwise. |
| `2209cb49` (PR #58) | Test only: the example RPC unit matches the generator's accept budget. |
| `9d837bef` (PR #54) | Removes a dead `#[cfg(test)]` swap-pricing wrapper. Test only. |

## Generated units

After the release-ID change, the 33 generated files are byte-identical to the
deployed `signer-rotation-20261007` files (`diff -r`, no difference): the RPC units
keep `--max-requests 100000` and `RestartSec=1`, the 12 environment files keep
`POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`. `systemd-analyze verify` reports nothing for
the 12 units ([log](qualification/logs/systemd-analyze-verify.log)).

## Contents

- [DEPLOY-SHEET.md](DEPLOY-SHEET.md): the release-day sequence, go points and rollback.
- `rootfs/etc/`: the 33 generated files (12 units, 12 environment files, six
  runtime bindings, topology, both circuit metadata files).
- [deployment-manifest.signed.json](deployment-manifest.signed.json): the signed
  manifest (`4f379c57…`); [manifest-input.unsigned.json](manifest-input.unsigned.json): its reviewed inputs.
- `stage-report.template.json`, `validator-bindings.signing.template.json`: the
  generated stage files with the local prefix replaced by `@STAGE@`.
- [node-builds.json](node-builds.json): both builds, toolchain and arguments.
- [local-preflight.py](local-preflight.py), [observe-fleet.py](observe-fleet.py),
  [demo-preflight.py](demo-preflight.py), [rollback-one.sh](rollback-one.sh):
  adapted from `signer-rotation-20261007` (branch `release/signer-rotation-20261007`);
  `rollback-one.sh` returns one validator to `signer-rotation-20261007`.
- [inventory.txt](inventory.txt): the unchanged rollout inventory.
- [qualification/](qualification/README.md): the local qualification packet.

Not in Git: private keys and the signed stage under
`~/.postfiat/deployments/fastlane-cap-20261014/` on the signing workstation.
No key material is in this directory.
