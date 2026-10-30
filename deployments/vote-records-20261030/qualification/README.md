# Qualification packet — vote-records-20261030

Local checks only, 2026-10-30, on the work server. No validator host was contacted, no live RPC call was made,
no chain transaction was submitted and nothing was deployed. Tip: `main` `ae70d66d` on branch
`release/vote-records-20261030`. Tools: [tools/](tools/) (adapted from the `index-v3-20261027` packet; history
checks run six at a time and the two exact replays in parallel).

**Not done today: canary backup, fleet before-state, rollout** (release day, operator's go;
[DEPLOY-SHEET.md](../DEPLOY-SHEET.md)).

## CI at the tip

`gh run list --branch main --limit 6` at the cut (2026-10-30T09:33Z), all for `ae70d66d` (pushed 09:15:52Z):
docs-build `41032153113` **success**; rust-ci `41032153158` **in progress** (its `test` job is the full
workspace suite, which was not run here); product-security-ci `41032153060` **in progress**. The rollout starts
only when all three are green.

## Builds

Two clean worktrees of `ae70d66d`, built in parallel with the 2026-09-28 procedure (separate copies of the
release-profile cache, identical path remaps, GCC linker, `SOURCE_DATE_EPOCH=1789514690`,
`cargo build --release --locked -p postfiat-node --bin postfiat-node`): both produced
`87cfe797af8c8c1c98e08b4794473bd25acb58ce0b7f57f0ea1b1f0fe98023c7` (65,901,656 bytes); `cmp` exit 0; no
RPATH/RUNPATH, GCC linker, sources clean after the build ([node-builds.json](../node-builds.json)).

## History checks (new executable; rollback executable on the new import)

Signed archives on this server, imported with the new executable (trusted key
`cobalt-activation-8694b99d/snapshot-keys/snapshot-publisher.public.json`), then `verify-finalized-checkpoint` and
`verify-state` (full replay); every identity is compared with the backup record of that release
([history-run.json](history-run.json), [reports/](reports/)).

| Archive | Import (new) | Checkpoint (new) | Full replay (new) | Rollback executable on the new import | Identity (height / tip / root) |
|---|---|---|---|---|---|
| 1153, validator-1, 2026-10-27 (manifest `03e5c61f…`) | PASS | PASS | PASS | checkpoint PASS, full replay PASS | 1153 / `db8b87b6…` / `7141956e…` |
| 1131, validator-1, 2026-10-21 (manifest `df9b3847…`) | PASS | PASS | PASS | — | 1131 / `9b054f09…` / `a12e7c5f…` |
| 1123, validator-1, 2026-10-15 (manifest `d957141e…`) | PASS | PASS | PASS | — | 1123 / `bafcc6ef…` / `942d1fc2…` |

A copy of the 1153 export with one signature character flipped is refused by the new executable's import
(non-zero exit), as required.

## Exact replays (`benchmarks/storage-scaling/run_replay_evidence.py`, new executable)

- **Height 924, legacy source** (`--legacy-source-dir` on a scratch `cp -a` of
  `storage-rollout-10dd9f20/g6-rehearsal-20260830-v3/backups/validator-0`; original never modified):
  **PASS** (`verdict` PASS), all 14 recorded-manifest fields matched (archive, blocks, ordered
  batches, receipts, ordered-history accumulator, chain, height, tip, root), 924 / `ebeb0e1e…` / `0854bc47…`,
  no network contact; source tree `13ffe388…` before and after; receipt `61c47952…` ([replays/height-924/](replays/height-924/)).
- **Height 1153, newest signed export** (`--signed-archive-dir` on a scratch copy of the 2026-10-27 validator-1
  export, manifest `03e5c61f…`): **PASS** (`verdict` PASS, `exact_replay` and `full_replay_passed` true, no
  network contact), signed import, checkpoint and full replay; height, tip and root matched at 1153 /
  `db8b87b6…` / `7141956e…`; archive tree `a95fa231…` before and after; receipt `4754ebb0…`
  ([replays/height-1153/](replays/height-1153/)).

## Rotation and rollback rehearsal

- The six 2026-09-28 copies at height 1021 (post-rotation; `~/.cache/release-repair-20260928/working`, copied):
  full replay with the new executable PASS on all six at 1021 / `35ab6fbc…` / `b01a98e5…`.
- Six loopback services of the new executable on those copies, both start orders, startup and restart:
  `PASS_STARTUP_AND_RESTART_ONLY`, all six agree at 1021 ([receipt](new-build-service-receipt.json)). The governed
  rotation itself cannot be rerun (its 1020 originals are deleted), as in the previous packets.
- The rollback executable `b5cf3ce4…` on the six copies the new executable ran on: checkpoint PASS on all six and
  full replay of validator-0 PASS at 1021, then the verification core of `rollback-one.sh` (checkpoint, identity
  at 1021 / `35ab6fbc…` / `b01a98e5…`) PASS on all six; `validate-local-keys` not run (the archived copies have no
  faucet key file) ([rollback-run.json](rollback-run.json)); on the new executable's 1153 import: checkpoint and
  full replay PASS (table above). The generated `rollback-one.sh` carries the 12 `index-v3-20261027` unit hashes
  of the 2026-10-27 stage (6/6 validators match) and passes `bash -n`.
- **Step 4's question: does the old executable serve data the new one wrote?** Yes. On the four-validator
  loopback devnet below, after the new executable committed height 1 (validator-0, 1 and 2 wrote
  `private_pool_snapshot_points.json`; the proposer validator-1 wrote the fixed vote record), the old executable
  read the four directories (`status`; `verify-state` PASS at the new root `e4058cb7…`), then ran
  `transport-validator-serve` for validator-0 and validator-1 in round 2 (proposer validator-2, new) and both
  applied block 2 with the new executable's root `76895f4c…`. Afterwards both executables pass
  `verify-finalized-checkpoint`, `verify-state` and `status` on validator-0, 1 and 2 at 2 / `76895f4c…`
  ([after-round2-checks.json](devnet/after-round2-checks.json)). The old executable ignores the snapshot points
  file (validator-0 and 1 keep only the height-1 point; validator-2, new, holds 1 and 2) and does not read the
  vote records. Required step: none (sheet Rollback). (Before round 2, the old `verify-finalized-checkpoint`
  refused height 1 only because the devnet's consensus-v2 activation height is 2; at height 2 it passes.)

## This release's checks

**5a. Vote records through the executable** ([devnet/](devnet/), [tool](tools/devnet_round.py)). The fix's
tests drive in-process validator threads, so a scripted round was run: four validators (`init-consensus-v2`,
`topology-consensus-v2`, one `transport-validator-serve` process per validator with the unit's prewarm
environment), proposer validator-1 running `transport-peer-certified-batch-round` with the default flags
(`--timeout-ms 4000`, no `--quorum-early-full-propagation`, no `--local-apply-before-certified-send`).
validator-3 listened on 127.0.0.2 behind a loopback proxy that delays its connection by 1 s, so its vote came
after the quorum of three. Round exit 0 in 1.06 s; the proposer wrote
`block-vote-records/height-00000000000000000001.json` (quorum 3, deadline 4000 ms) naming all four:
validator-0, 1, 2 `accepted` with vote ids; **validator-3 `accepted` with its vote id and
`answered_after_quorum: true`** (the VR-01 case; the deployed executable recorded such votes `no_answer`). In
round 2 the record names all four again; validator-3, still at height 0 (without `--quorum-early-full-propagation` round 1
listed it in `skipped_certified_send_targets`), is `refused` with class `block_proposal_does_not_match_local_batch_and_state`, not `no_answer`.

**5b. Private path switched off** on a scratch copy of the signed 1153 import, new executable's `rpc-serve`
with the unit's flags and without `--allow-private-action-admission`
([private-off-report.json](private-path/private-off-report.json)):

- `private_action_submit` and `private_action_receipt`: `rpc_method_not_allowed`.
- `private_readiness` (`asset-orchard-v1`): `ready: false`, reasons
  `asset_orchard_pricing_enforcement_activation_absent` and `a666_pfusdc_route_absent`; admission switch `off`.
- `private_pool_snapshot` (`asset-orchard-v1`): answers at 1153 / `db8b87b6…` / `7141956e…`, complete, 175
  commitments, 163 nullifiers, 175 outputs, 135 accepted anchors, 6 assets.
- `status`: 1153 / `db8b87b6…` / `7141956e…`, build `ae70d66d`, as recorded. Activation heights
  (`read-import-state.py`): `recipient_format`, `rejected_tx_fee`, `require_memo` 1153 each (unchanged); no
  `asset_orchard_pricing_enforcement_activation_height` or `asset_orchard_egress_policy_activation_height`
  amendment.
- The 12 generated units pass no `--allow-private-action-admission`.

**5c. Snapshot points and identical state.** The signed import carries no validator signing keys, so no
certified block can be applied to it locally; the block was applied on the devnet of 5a instead. After height 1
each applying validator has `private_pool_snapshot_points.json` (schema
`postfiat.private_pool_snapshot_points.v1`, point at 1 / `6653fd69…` / `e4058cb7…`). The old executable's full
replay of that block log produces the same root `e4058cb7…`, and in round 2 the old executable (validator-0, 1)
and the new one (validator-2) applied the same block 2 to the same root `76895f4c…`: byte-identical state.
Not done: applying a block on a scratch copy of the 1153 import itself.

## Focused suites (release tip, debug)

[focused-suites.log](logs/focused-suites.log), [focused-suites-2.log](logs/focused-suites-2.log): `--lib
block_vote_records` 4 passed; `--bin postfiat-node transport_batch_payload_tests` 19 passed (includes the VR-01
tests `peer_round_records_a_vote_after_the_quorum_when_the_writer_runs_at_the_deadline`,
`vote_record_marks_an_answer_after_the_deadline_late_after_deadline`,
`vote_record_keeps_a_refusal_after_the_deadline_refused`); `--lib private_action_` 11 passed (admission store,
round service, receipt); rpc-serve switch tests `rpc_serve_exposes_private_actions_only_with_the_switch` and
`private_action_methods_exist_only_behind_the_admission_switch` passed; `--lib asset_orchard_pricing` 10 passed
(incl. `asset_orchard_pricing_plan_is_unchanged_before_the_effective_height`); `--lib asset_orchard_egress` 4 and
`asset_orchard_private_egress_binding` 3 passed; `--lib private_pool` 13 passed (incl.
`private_pool_tokens_outlive_new_blocks_until_the_ring_expires`); `--lib account_tx` 26 passed. The full workspace
suite is CI's (rust-ci `41032153158`); the release-only proof modules were not run.

## Manifest, units and preflight

- Signed manifest `ee1df5b3c71640df4fe04d95306ec6e6e8cf628f766f5ddacaed5e31ef9a2eec`, publisher
  `pfc531e00ffefecb8fabe40c6f47c69148df31e49c` (key on this server, used by path), valid 1793353262–1824889562,
  git revision `ae70d66d…`, binary `87cfe797…`, features `privacy,rpc,transport` ([stage log](logs/stage.log)).
  As in the previous packets, the stage script's per-validator `deployment-manifest-verify` loop needs the host's
  unit paths and is a host check; locally the signed gate is `local-preflight.py --require-signed`:
  `PASS_SIGNED_LOCAL_INPUTS_ONLY`, 33 generated files, 4 circuit artifacts, no network call
  ([local-preflight-signed.json](logs/local-preflight-signed.json)).

### Generated units

After the release-ID change the 33 generated files are byte-identical to `index-v3-20261027`'s: RPC units keep
`--max-requests 100000` and `RestartSec=1`; the 12 environment files keep `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`.
`systemd-analyze verify --root` over the stage (exit 0) reports nothing for the 12 units; its log has only the
host's own unresolvable dracut unit links ([log](logs/systemd-analyze-verify.log)).

## Gates

On the release tree ([gates/](gates/)): `cargo fmt --all -- --check` exit 0 (only the stable-channel
`make_backup` warnings); `scripts/test-proof-public-input-inventory` (in scope: `548d4918`/`d23b14d1` changed
`crates/privacy_orchard`) `proof_public_input_inventory=ok systems=7 public_fields=150 source_hashes=97`;
`.venv-docs/bin/mkdocs build --strict` exit 0; `scripts/public-doc-links` `ok files=572`;
`scripts/public-secret-scan` passed (tracked tree, with this packet staged).
