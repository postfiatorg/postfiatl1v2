# SCT-08 evidence campaign — 2026-10-23

**Status:** started 2026-10-23.

- Item 5 passes since the runner change of the same day.
- Item 4's native reliability input is defined (decision below). Its node
  side, the vote records, is a candidate of the next release, and until then
  the measure is "not available".
- Items 1–3 wait for their providers.
Items 1–3 cannot run here because their inputs do not exist yet. Nothing below
is live authority.

**Scope.** [Defect inventory](defect-inventory-20260910.md) row SCT-08 is a
bounded, read-only campaign that the operator authorized on 2026-10-12. It
completes the evidence named in the
[2026-09-06 review](https://github.com/postfiatorg/postfiatl1v2/blob/cfdeccd6897bfc8a80bfa2a95a6fb04f603c1cee/docs/review/storage-cobalt-tasknode-handoff-review-20260906.md).
That evidence is missing from the Task Node real-data shadow packet of
[2026-09-04](../governance/tasknode-unl-shadow-run-20260904.md).

**How it ran.** Work server only. There was no validator-host contact, no RPC
call to the controlled devnet, no transaction and no Task Node write.

- Binary: the deployed `exchange-readiness-20261023` executable `4085ac70…`
  (source `6b1a0585`). Repository at `d1ad40ba`.
- Inputs are three signed canary archives, each imported into scratch:

| Height | Signed manifest | Archive (work server) |
| ---: | --- | --- |
| 1123 | `d957141e…` | `fastlane-cap-20261014` pre-rollout backup |
| 1131 | `df9b3847…` | `store-handle-20261021` pre-rollout backup |
| 1137 | `3257115c…` | `exchange-readiness-20261023` pre-rollout backup |

- Each archive was imported with
  `snapshot-import-signed-finalized-checkpoint` and the snapshot-publisher key.
  The importer and `verify-finalized-checkpoint` exited 0, and each archive's
  tree hash was unchanged before and after.
- The 924 copies were not needed: these archives carry the full history from
  height 1.
- Outputs (not in git) are under `~/.cache/sct08-20261023/`, listed with
  hashes in `SHA256SUMS.txt` (`ab43c00a…`).

## The five items

| # | Item | What it means | Input that proves it | Acceptance |
| --- | --- | --- | --- | --- |
| 1 | Complete signed bindings | Each evaluated validator has a Task Node binding that ties its validator key to a ledger wallet, signed by both keys. | Binding records (public memo, transaction hash, challenge digest, both signatures), plus the native registry keys from a signed archive. | Every record passes `python -m postfiat_rpc.tasknode_unl verify-record`. The validator key in each record hashes to that validator's key in the native registry. |
| 2 | Publisher-attested work digests | Task Node publishes, per account and window, a signed digest of the work pointers used for accountability. | Signed work digests and the Task Node publishing-key record. | Every digest verifies against a published key, and its pointers reconcile with the ledger pointers (`tasknode_unl_work_digest`). |
| 3 | Exclusions | A published list of exchange and Foundation wallets whose funding does not count as an independent trust edge. | The published funding exclusion list and its publisher identity. | The list is present and well-formed, so edge extraction no longer holds on `missing_funding_exclusion_list`. |
| 4 | Admission facts | The remaining Admission Policy V1 facts per candidate. | Task Node, the operators and the L1 archives (see item 4). | Each fact has a named source, and the runner no longer holds on `missing_*` for it. |
| 5 | Native registry-root binding | The shadow packet names the six-validator native L1 registry root, not a content hash of another list. | The registry in a signed, finalized L1 archive, and the root in its block certificates. | The root is re-derived from the archive and carried by the certificates. The shadow input binds that root and the native validator set. |

The Admission Policy V1 facts of item 4 are: accountability, ρ (rho),
reliability, operator manifest signature, domain control or key-domain binding,
Cobalt linkedness and model classification.

## Results

| # | Verdict | Evidence (`~/.cache/sct08-20261023/`) |
| --- | --- | --- |
| 1 | **NOT RUNNABLE HERE** | `results/presence.json` (`a69a61f2…`) |
| 2 | **NOT RUNNABLE HERE** | `results/presence.json` |
| 3 | **NOT RUNNABLE HERE** | `results/presence.json` |
| 4 | **PARTIAL**: L1 facts derived. The reliability records are live on all six validators since 2026-10-30 (below); the fact over a window is not derived yet. The rest NOT RUNNABLE HERE | `results/native-evidence.json` (`a9010949…`); [1168 record](https://github.com/postfiatorg/postfiatl1v2/blob/release/vote-records-20261030/deployments/vote-records-20261030/observed/vote-record-1168.json) |
| 5 | **PASS** (2026-10-23, after the runner change). First run: PASS for the native root and its certification, FAIL for the shadow binding | `item5/shadow-report-1137.json` (`6b2059a4…`); first run `results/native-evidence.json`, `results/native-root-shadow/run.stderr` (`2c925153…`) |

The Task Node verifier suite passes on main (178 tests,
`results/tasknode-verifier-tests.txt`, `3fae1c84…`). Items 1–3 therefore have a
working verifier; the evidence itself is what is missing.

### Items 1–3: inputs do not exist yet

The retained 2026-09-04 packet holds:

- 0 binding records;
- 0 work digests;
- 0 publishing keys;
- a `null` funding exclusion list.

A search of the deployment records, the release caches and the repository's
JSON finds none of these outside schemas and test fixtures.
`stage/validator-bindings.signing.json` in the release packets binds services
to validator IDs and carries no key or wallet, so it cannot serve as item 1.

- **Item 1** needs each validator's operator to sign a binding with the
  validator key and the wallet key, and to publish it as a ledger memo
  (`tasknode_unl prepare-bind` / `finalize-bind`). Signing with validator
  keys is not read-only. For the six Foundation-run validators the provider is
  Post Fiat, under a separate authorization.
- **Item 2** needs Task Node to publish signed per-account work digests and its
  publishing-key record. Provider: the Task Node maintainers.
- **Item 3** needs the published exchange and Foundation funding exclusion list.
  Provider: Post Fiat.

### Item 4: admission facts

These facts were derived from the three signed L1 states for the six native
validators:

- **Native registry membership and keys: PASS.** All three heights list
  `validator-0`…`validator-5`, each with the SHA-256 of its registry key
  (in `native-evidence.json`). `verify-governance` verifies the state in
  `canonical` Cobalt mode with authority mode 1.
- **Reliability (`uptime_window_bps`): FAIL** as an L1-derived fact.
  - *Condition:* reliability for the native validators is taken from the
    finalized L1 record.
  - *Observed:* the only per-validator signal in the archives is the set of
    votes inside each block certificate. At 1137, over 214 blocks since 924,
    inclusion is 8037, 9859, 5607, 8691, 9859 and 8177 bps (validator-0 to
    validator-5). A certificate needs only five votes, and the
    `consensus-v2-qcs` directory is empty in all three states.
  - *Expected:* a native measure of availability that the 9,950 bps policy
    floor (`V1_MIN_RELIABILITY_BPS`) can apply to.
  - *Suggested change:* define `uptime_window_bps` against a native source that
    records each validator's availability per height, not certificate
    inclusion. Until then, keep this fact sourced from Task Node and holding
    when absent. Done as a definition in the decision below; the records it
    needs are a node change for the next release.

#### Decision: the native reliability input (2026-10-23)

1. **Measure.** For each validator, `uptime_window_bps` is its accepted votes
   divided by the finalized heights in the window, times 10,000. The window
   is a fixed run of heights ending at the evaluated archive's tip, and the
   votes are counted from that window's **vote records**.
2. **When it counts.** The measure counts only when a vote record exists for
   every height in the window. Otherwise it is **not available**: the runner
   holds on `missing_reliability`, and the 9,950 bps floor does not apply.
3. **No stand-in.** Certificate inclusion is never used as the reliability
   fact. A certificate carries only the votes needed for its quorum of five,
   so a validator missing from a certificate may still have voted.
   Certificate inclusion is reported only as a lower bound.
4. **What the node must record.** For every finalized height, the round's
   vote record lists each validator it asked for a vote. For each, it gives
   the outcome: an accepted vote with its vote ID, a refusal, or no answer by
   the round deadline. It includes votes that arrive after the quorum is
   formed. The record is kept with the block and exported in the signed
   snapshot, under a manifest hash, so an archive carries it.
5. **Which release.** The node change is a candidate of the release after
   `exchange-readiness-20261023`
   ([plan](../plans/active/next-validator-release-plan-20261108.md#candidates)).
   It adds a stored and exported artifact without changing a consensus or
   state result. It is qualified as the plan's rule requires, and gets its
   own qualification if review finds that it touches a state or consensus
   result.

No node code changed today. Until a release carries vote records, the
native reliability fact stays not available.

#### Deriving the fact from the vote records (2026-10-26)

Commit `1432b8e0` keeps the vote records (point 4; next-release candidate).
The fact is now derivable from them with a read-only command. Each validator
holds only the records of the heights it proposed, so a window needs all six
record directories: data directories, their `block-vote-records/` folders, or
one parent folder of them.

```bash
PYTHONPATH=python python3 -m postfiat_rpc.vote_reliability \
  --records DATA_DIR_0 --records DATA_DIR_1 ... --records DATA_DIR_5 \
  --from-height FROM --to-height TO [--validators validator-0,...] [--table]
```

Per validator it reports the heights it was asked, its accepted, refused (by
reason class) and unanswered votes, its answers after the quorum, and
`uptime_window_bps`: accepted votes × 10,000 ÷ heights in the window, rounded
down (point 1). A height without a record makes every validator **not
available**, and so does, for one validator, a record that does not list it
(point 2); the runner then holds on `missing_reliability`. Two records for one
height that disagree, such as two proposers, are refused and listed (exit 1).
JSON is the default output. Until a release carries the records on the six
validators, the fact stays not available.

#### Live vote records (2026-10-30)

Release `vote-records-20261030` (source `ae70d66d`, executable `87cfe797…`)
runs on all six validators since 11:14Z, applied at 1162–1167, with the VR-01
fix. Evidence: the release packet's `observed/` on branch
`release/vote-records-20261030`, commit `d4b344be`.

- [x] **The proposer writes the record live.** The 1168 record is validator-4's
  `/var/lib/postfiat/validator-4/block-vote-records/height-00000000000000001168.json`,
  SHA-256 `6618a82c…` ([check](https://github.com/postfiatorg/postfiatl1v2/blob/release/vote-records-20261030/deployments/vote-records-20261030/observed/vote-record-1168.json)). It has schema
  `postfiat.block_vote_record.v1`, block `a20e406e…`, certificate `bdf9576e…`,
  view 0, quorum 5 and a 60 s deadline. All six validators are `accepted`, each
  with its vote ID. validator-2 is `answered_after_quorum`, so the 5-of-6
  certificate omits it.
- [x] **VR-01 fixed live.** At 1160 a vote accepted after the quorum was
  recorded as `no_answer`. At 1168 the same case is `accepted` with its vote ID.
- [x] **Rollout heights.** The records for 1164–1167 (proposers validator-0 to
  validator-3) name all six, and every certificate voter is `accepted`. Their
  rounds still ran the previous executable `b5cf3ce4…`, which has VR-01.
- [x] **The 1166 `no_answer` is VR-01.** validator-2's record lists validator-3
  as `no_answer`, and the 5-of-6 certificate `29004ce5…` omits it. validator-3's
  event log shows two `block_vote_request` events from validator-2 for 1166,
  both `accepted`, then the certified batch, applied. Its vote file
  `1166.0.validator-3.block_vote.json` was written at 11:10:46Z. validator-3
  voted after the quorum, and the old round wrote it as `no_answer`.
- [ ] The fact over a window: needs every height's record from all six record
  directories, collected by the operator. The first window fully on the fixed
  build starts at 1168.
- [ ] The records in the signed snapshot: this needs a new snapshot version.
- **Cobalt linkedness: NOT RUNNABLE HERE.** The native trust state verifies,
  but linkedness is a property of a candidate's admission, and no candidate has
  a binding (item 1).
- **Accountability, ρ, operator manifest signature, domain control and model
  classification: NOT RUNNABLE HERE.** They need Task Node digests and wallet
  mappings (items 2 and 3), operator-signed manifests (no operator manifest
  exists on this server), and the model classification of the governance lane.

Re-running `python -m postfiat_rpc.tasknode_unl shadow-derive` on the retained
2026-09-04 input still gives **hold** for all three observed accounts. The
report differs from the committed one (`20e76784…` against `1998fafb…`): the
current runner adds `wallet_account_mapping_hold`, from the SCT-02 fix
`1267df6a`. That is a related finding:

- *Condition:* the committed shadow report is presented as byte-reproducible.
- *Observed:* the current runner produces a different report from the same
  input, with the same verdicts.
- *Expected:* the report states the runner revision it reproduces under.
- *Suggested change:* record beside the hash in the shadow-run note the
  repository revision it reproduces at (`3d0e5c01`, per the 2026-09-06
  review), or re-derive the report on the next real run.

### Item 5: native registry-root binding

**PASS: the native root exists and is certified.**

- The native registry root at 1123, 1131 and 1137 is `08a451e07aea…9b1d`
  (`validator-registry-root`). It equals the latest registry update's new root
  (`rotate_key validator-5`, activation 924).
- Every certificate from 925 to 1137 carries it (213 blocks). The activating
  block 924 carries the previous root `945768d5…`. No vote disagrees with its
  certificate's root.

**FAIL: the shadow packet does not bind it.**

- *Condition:* the shadow input's `baseline_list.registry_root` is set to the
  native root.
- *Observed:* the runner exits 2 with `invalid_lower_hex:
  baseline_list.registry_root`. `_require_hex` in
  `tasknode_unl_policy.py:273` accepts only 64 hex characters, while the
  native root is a 96-character SHA3-384 value. The 2026-09-04 packet uses the
  SHA-256 of the fork's 20-entry list (`b4edcf3d…`), and the runner checks only
  its syntax. Its `validator_ids` are the fork's keys, not `validator-0`…`5`.
- *Expected:* the shadow input names the native root and validator set from a
  signed, finalized archive. The runner re-derives that root, or checks it
  against the archive, before deriving.
- *Suggested change:* add a native-registry input to the shadow runner: the
  registry file, the signed-manifest hash and the height. Recompute the root
  with the node's registry-root function, accept the 96-character root, and
  hold when it differs. The fork's list stays a separate input of the trust
  graph.

**Re-run after the runner change (2026-10-23): PASS.**

The runner change (`python/postfiat_rpc/tasknode_unl_policy.py`):

- an optional `native_registry` input (`:90`);
- `native_registry_root` (`:282`), the node's `validator_registry_root` in
  Python, which equals the node's `08a451e0…` for the 1137 registry;
- `native_registry_from_document` (`:327`). It accepts 96-character roots for
  this kind only, recomputes the root from the ordered validator set, and
  refuses `native_registry_root_mismatch` or a certificate root that differs.
- `native_registry_from_archive` (`:398`). It reads the registry, the last
  registry update and the tip certificate from a signed export. Each file must
  match its hash in the signed manifest (`postfiat.snapshot.file.v1`).
- Admission packets and their registry deltas now bind the native root
  (`:1520`), and the report records it (`:1783`, `:2137`).
- The fork's 64-character content-hash baseline is unchanged; it still feeds
  the churn guard. Without the native input, the golden fixture output is
  byte-identical.
- CLI: `python -m postfiat_rpc.tasknode_unl native-registry-extract
  --archive-dir DIR` (`tasknode_unl.py:461`).

Tests are in `python/tests/test_tasknode_unl_native_registry.py`:

- 9 of 10 failed before the change and all 10 pass after. The tenth, the
  fork-path test, passes both times by design.
- The Task Node suite passes: 188 tests.

The re-run, with outputs under `~/.cache/sct08-20261023/item5/`:

- Extracting from the signed 1137 export (`3257115c…`) gives root
  `08a451e0…` from the registry update and from the tip certificate. The
  validator set is `validator-0`…`5`. The output is `native-registry-1137.json`
  (`157476ce…`). The 1131 export (`df9b3847…`) gives the same root
  (`45c4c7e7…`).
- Both archive trees were unchanged. The 1137 re-import's registry file is
  byte-identical to the export's.
- `shadow-derive` on the retained 2026-09-04 input plus the native input exits
  0. The report is `shadow-report-1137.json`, SHA-256 `6b2059a4…`, report hash
  `95b87398…`.
- All three observed accounts still hold. Every admission packet and registry
  delta now names `08a451e0…`.

## What remains and who provides it

| Item | Remaining | Provider |
| --- | --- | --- |
| 1 | Signed validator-to-wallet bindings for the six validators, published as ledger memos | Post Fiat as operator of the six validators, under a separate signing authorization |
| 2 | Signed per-account work digests and the publishing-key record | Task Node maintainers |
| 3 | The published exchange and Foundation funding exclusion list | Post Fiat |
| 4 | Reliability over a window from the live vote records (from 1168) and their signed-snapshot export; accountability, ρ and domain-control inputs from items 1–3; operator manifests; model classification | This lane and the operator (records and export); operators; the governance lane |
| 5 | Done 2026-10-23 (runner change and 1137 re-run). A real non-empty derivation still needs items 1–3. | — |

2026-10-29: PR #180 (`29589ab2`) added the [Task Node L1 validator binding](../runbooks/tasknode-l1-validator-binding.md) tool, in which the validator key alone signs a Task Node account challenge; it is not the dual-signed ledger-memo record item 1 needs, so item 1 stays open.

## Commands

```bash
# per archive (scratch data dir; the archive itself is read only)
postfiat-node snapshot-import-signed-finalized-checkpoint --data-dir SCRATCH \
  --snapshot-dir ARCHIVE --trusted-publisher-key-file snapshot-publisher.public.json \
  --node-id validator-1
postfiat-node verify-finalized-checkpoint --data-dir SCRATCH
postfiat-node verify-governance --data-dir SCRATCH
postfiat-node validator-registry-root --data-dir SCRATCH \
  --validators validator-0,validator-1,validator-2,validator-3,validator-4,validator-5
# certificate roots and vote inclusion
python3 ~/.cache/sct08-20261023/analyze.py
# shadow re-derivation and the native-root binding check
PYTHONPATH=python python3 -m postfiat_rpc.tasknode_unl shadow-derive \
  --input-dir INPUT --output OUT
```

The scripts are `native-root.sh` and `analyze.py` in the output directory.
