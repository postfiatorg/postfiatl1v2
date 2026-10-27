# Qualification packet — index-v3-20261027

Local checks only, 2026-10-27, on the work server. No validator host was contacted, no live RPC call was made,
no chain transaction was submitted and nothing was deployed. Tip: `main` `d728f895` on branch
`release/index-v3-20261027`. Tools: [tools/](tools/) (adapted from the `exchange-readiness-20261023` packet;
the runner launches commands through `prlimit` instead of a `preexec_fn`, which deadlocked with concurrent
threads on the first start, and history checks run four at a time).

**Not done today:** canary backup, fleet before-state, rollout, index build (release day, operator's go;
[DEPLOY-SHEET.md](../DEPLOY-SHEET.md)). Also not done: a two-process local devnet round through the built
executable (§5b below).

## CI at the tip

`gh run list --branch main --limit 12`, all for `d728f895` (2026-10-26T14:02Z): docs-build `40335966553`
success; rust-ci `40335966336` success (its `test` job is the full workspace suite, which was not rerun here);
product-security-ci `40335966335` success.

## Builds

Two clean worktrees of `d728f895`, built in parallel with the 2026-09-28 procedure (separate copies of the
release-profile cache, identical path remaps, GCC linker, `SOURCE_DATE_EPOCH=1789514690`,
`cargo build --release --locked -p postfiat-node --bin postfiat-node`): both produced
`b5cf3ce4bc046e6338beca6d5ead6fd0343426ddb01ac2f461c31e2775f9c8eb`; `cmp` exit 0; no RPATH/RUNPATH, GCC
linker, sources clean after the build ([node-builds.json](../node-builds.json)).

## History checks (new executable, rollback executable on the new imports)

Signed archives on this server, imported with the new executable (trusted key
`cobalt-activation-8694b99d/snapshot-keys/snapshot-publisher.public.json`), then `verify-finalized-checkpoint` and
`verify-state` (full replay); every identity is compared with the backup record of that release
([history-run.json](history-run.json), [reports/](reports/)).

| Archive | Import (new) | Checkpoint (new) | Full replay (new) | Rollback executable on the new import | Identity (height / tip / root) |
|---|---|---|---|---|---|
| 1137, validator-1, 2026-10-23 (manifest `3257115c…`) | PASS | PASS | PASS | checkpoint PASS, full replay PASS | 1137 / `18a4dc86…` / `1277c76c…` |
| 1131, validator-1, 2026-10-21 (manifest `df9b3847…`) | PASS | PASS | PASS | — | 1131 / `9b054f09…` / `a12e7c5f…` |
| 1123, validator-1, 2026-10-15 (manifest `d957141e…`) | PASS | PASS | PASS | — | 1123 / `bafcc6ef…` / `942d1fc2…` |

A copy of the 1137 export with one signature character flipped is refused by the new executable's import
(non-zero exit), as required.

## Exact replays (`benchmarks/storage-scaling/run_replay_evidence.py`, new executable)

- **Height 924, legacy source** (`--legacy-source-dir` on a scratch `cp -a` of
  `storage-rollout-10dd9f20/g6-rehearsal-20260830-v3/backups/validator-0`; original never modified, tree hash
  `13ffe388…` before and after): **PASS**, all 14 recorded-manifest fields matched (archive, blocks, ordered
  batches, receipts, ordered-history accumulator, chain, height, tip, root), 924 / `ebeb0e1e…` / `0854bc47…`,
  no network contact; receipt `0c8e8de5…` ([replays/height-924/](replays/height-924/)).
- **Height 1137, newest signed export** (`--signed-archive-dir` on a scratch copy of the 2026-10-23 validator-1
  export, manifest `3257115c…`): **PASS**, signed import, checkpoint and full replay, height, tip and root matched
  at 1137 / `18a4dc86…` / `1277c76c…`; archive tree unchanged; receipt `7b95bea8…`
  ([replays/height-1137/](replays/height-1137/)).

## Rotation and rollback rehearsal

- The six 2026-09-28 copies at height 1021 (post-rotation; `~/.cache/release-repair-20260928/working`, copied):
  full replay with the new executable PASS on all six at 1021 / `35ab6fbc…` / `b01a98e5…`.
- Six loopback services of the new executable on those copies, both start orders, startup and restart:
  `PASS_STARTUP_AND_RESTART_ONLY`, all six agree at 1021 ([receipt](new-build-service-receipt.json)). The governed
  rotation itself cannot be rerun (its 1020 originals are deleted), as in the previous packets.
- The rollback executable `4085ac70…` on the six copies the new executable ran on: checkpoint PASS on all six and full replay of validator-0 PASS at 1021, then the verification core of `rollback-one.sh` (checkpoint, identity at 1021 / `35ab6fbc…` / `b01a98e5…`) PASS on all six; `validate-local-keys` not run (the archived copies have no faucet key file)
  ([rollback-run.json](rollback-run.json)); and on the new executable's 1137 import: checkpoint and full replay
  PASS (table above).
- Units: the 33 generated files keep `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`, `--max-requests 100000` and
  `RestartSec=1` (below).

## This release's checks

**5a. Index v2 → v3** on a scratch import of the signed 1137 archive
([index-v3-check.json](index-check/index-v3-check.json)). The deployed executable (`4085ac70…`) imported it and
built the v2 index (4.84 s; 1,099 indexed blocks, 964 rows, 255 accounts), as the hosts carry today.

- The new executable reports both indexes unsupported: ``unsupported account_tx index schema `postfiat-account-tx-index-v2` ``
  and ``unsupported account_tx disk index schema `postfiat-account-tx-disk-index-v2` ``.
- `account_tx` for the 20 addresses with the most rows answers from the bounded archive scan (`index_used: false`,
  newest 100 blocks, `truncated: true`); its rows equal the v2 index rows of those blocks for 20/20.
- `account-tx-index-build` rebuilt it as v3: **6.83 s for 1,137 blocks, 6.01 s per 1,000 blocks**; 964 rows,
  255 accounts and account directories; both indexes usable, schema `postfiat-account-tx-disk-index-v3`, lag 0.
- After the rebuild the 20 `account_tx` answers (`--limit 100`) are **byte-identical, 20/20**, to the answers
  the deployed executable gave from its v2 index.

**Rollback after v3 (step 4's question).** The deployed executable does not fail on `account_tx`: it ignores the
v3 files and answers from its archive scan (20/20 rows equal within the scanned window). Its
`account-tx-index-status` fails: `failed to parse account_tx index …: missing field accounts`. Its own
`account-tx-index-build` then rewrites a usable v2 index in 4.29 s, and its answers are byte-identical to the
pre-upgrade answers, 20/20. The sheet's Rollback section states the required step.

**5b. Proposer vote record and local apply.** Covered in-process by the binary target's tests, not by a
two-process devnet: `peer_round_keeps_a_vote_outcome_record_for_every_requested_validator` (ten validators with
one refusing, one silent and one late; the proposer writes `block-vote-records/height-<h>.json` naming every
requested validator) and `default_peer_round_applies_locally_without_sending_to_its_own_validator` (default
flags; the block is applied locally), both in `transport_batch_payload_tests`, 16 passed. **Not done:** a local
two-validator devnet round through the built executable; the first executable-level evidence is the sheet's §8
check on the release day.

**5c. Activation heights.** The built executable's `status` read has no activation fields, and the newest signed
archive on this server (1137) predates the activation blocks 1144–1146, so no local import carries them.
`crates/types` and `crates/execution` are unchanged since the deployed `d9260e93`
(`git diff --stat d9260e93 d728f895 -- crates/types crates/execution` is empty), so the governance kinds and
their rules are unchanged; the live read is in the sheet (§3 from the §4 import, §9).

## Focused suites (release tip, debug)

[logs/focused-suites.log](logs/focused-suites.log): `--lib account_tx` 26 passed; `--bin postfiat-node
transport_protocol` 32 passed (`--lib transport_protocol` has no tests); `--bin postfiat-node
transport_batch_payload_tests` 16 passed; `--lib block_vote_records` 4 passed; `--test fastpay_default_rpc`
4 passed. The full workspace suite is CI's (rust-ci `40335966336`).

## Manifest, units and preflight

- Signed manifest `a61e290a6045e71b38c5451df33ddc08c16505d9d3453f98a0131466d662b007`, publisher
  `pfc531e00ffefecb8fabe40c6f47c69148df31e49c` (key on this server, used by path), valid 1793095187–1824631487,
  git revision `d728f895…`, binary `b5cf3ce4…` ([stage log](logs/stage.log)). The stage script's per-validator
  `deployment-manifest-verify` loop needs the host's unit paths and is a host check; locally the signed gate is
  `local-preflight.py --require-signed`: `PASS_SIGNED_LOCAL_INPUTS_ONLY`, 33 generated files, 4 circuit
  artifacts, no network call ([local-preflight-signed.json](logs/local-preflight-signed.json)).

### Generated units

After the release-ID change the 33 generated files are byte-identical to `exchange-readiness-20261023`'s: RPC
units keep `--max-requests 100000` and `RestartSec=1`; the 12 environment files keep
`POSTFIAT_DEPLOYMENT_VERIFIED_RECORD`. `systemd-analyze verify --root` over the stage reports nothing for the 12
units; its log has only the host's own unresolvable system-unit symlinks
([log](logs/systemd-analyze-verify.log)).

## Gates

On the release tree ([gates/](gates/)): `cargo fmt --all -- --check` exit 0 (only the stable-channel
`make_backup` warnings); `scripts/test-proof-public-input-inventory`
`proof_public_input_inventory=ok systems=7 public_fields=150 source_hashes=97`; `.venv-docs/bin/mkdocs build
--strict` exit 0; `scripts/public-doc-links` `ok files=561`; `scripts/public-secret-scan` passed (tracked tree,
with this packet staged).
