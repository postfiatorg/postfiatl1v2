# Storage-scaling evidence

Status: **PUBLIC TESTNET BLOCKED**

This directory owns implementation evidence for the
[locked storage scaling fix specification](../../docs/architecture/storage-scaling-fix-spec.md).

The earlier [e2-bounded-work.json](e2-bounded-work.json) packet is retained as
historical development evidence for the JSONL-head and fixed-bitmap prototype.
It is not release evidence: its elapsed values came from debug unit tests, and
the fixed bitmap was rejected as the selected primary store.

## Release campaign harness

`run_paired_campaign.py` is the only release campaign entry point. The active
profile selects transactional `redb` and runs only the evidence that can still
change its qualification decision, in this order:

1. transactional `redb` at height 50;
2. transactional `redb` at height 5,000; and
3. legacy JSON/JSONL at height 50 as the same-binary regression baseline.

Each row contains five independent 50-round windows. The superseded
three-lane/five-height campaign is not a release gate; its bounded-JSONL lane
and legacy runs above height 50 are optional diagnostics. The stopped
four-hour attempt is recorded in
[`campaign-stop-f3907ad5.json`](campaign-stop-f3907ad5.json) and is explicitly
not evidence eligible.

The backend selector is an authenticated node-local record named
`storage_backend_mode.json`. It is excluded from portable snapshots and every
consensus artifact. Missing configuration selects `transactional`; legacy
comparison mode requires both `--offline-confirmed` and
`--unsafe-comparison-mode` and is permitted only on disposable qualification
clones.

The runner creates one topology, validator-key set, deterministic wallet and
recipient, and canonical transactional seed snapshot. Height 50 is frozen as
both a portable authenticated snapshot and a content-hashed prepared six-node
fleet because the legacy comparison needs the portable form. Higher selected
heights are frozen only as content-hashed prepared fleets. Every selected
window restores a byte-for-byte file-content copy of its fleet at the canonical
database path recorded by the transactional generation pointer. The legacy
control imports the shared portable height-50 snapshot instead, because its
intentionally different backend must not inherit the selected redb generation.
Both height-50 lanes consume the same signed corpus. The source revision,
binary SHA-256, snapshot or prepared-fleet digest, signed transaction bytes,
host allocation, storage device, full-vote policy, and 900-second fail-closed
timeout are identical; only the authenticated backend mode changes. The
height-5,000 prepared fleet is built through selected transactional advance chunks; no
portable height-5,000 snapshot or legacy height-5,000 control is created.

Setup-only selected advances use one long-lived
`transport-peer-certified-batch-loop` process that owns validator-0 while the
other five validators remain resident. A separately built and hash-bound
`postfiat-storage-corpus-batches` helper converts each exact signed corpus entry
into one canonical one-transaction batch using the same `mempool_dag` builder
as the candidate source. The loop routes proposals to the deterministic leader,
records literal receipts and per-stage storage work, and advances all six nodes
without restarting a proposer process for every block. This optimization is
not performance evidence. All fifteen measured windows retain the original
one-round latency process and resource semantics.

The runner atomically checkpoints every advance chunk, frozen height input, and
completed window. `--resume` rehashes and refuses any changed source, candidate
binary, batch-builder binary, runner, topology, validator identities, snapshot
or prepared fleet, corpus, canonical batch, certificate, timeout, completed
report, resource stream, receipt, or result binding. A
partial unit is moved to an `interrupted/` quarantine before retry, not
overwritten. An exclusive campaign lock prevents concurrent resume. The release
profile has a four-hour aggregate wall-clock limit. Advances contain at most
1,500 rounds. Each
completed chunk freezes a content-hashed prepared fleet; only the initial
height-50 chunk also freezes a portable snapshot for the legacy comparison.
The next selected chunk restores a verified copy at the canonical redb path
instead of replay-importing or exporting ever-larger histories. Its signed
corpus is created on a byte-verified disposable canonical clone of the stopped
prepared fleet. The runner proves the frozen source is unchanged, binds the
scratch clone's before/after digests and expected sequence, discards the scratch,
and restores a pristine clone before measurement. Operators must
additionally wrap each unattended segment in the plan's two-hour hard timeout;
a completed unit is durable and a partial unit is quarantined.

The report derives the height-50 legacy baseline from raw observations,
recomputes p50/p95/p99/max/mean/standard deviation, publishes per-window
resource variance, and fits the selected height-50/height-5,000 stage model.
Mode-generic counters cover proposer construction, all five remote validator
reconstructions, the local finalized apply, and all five certified remote
applies. Each window preserves a redaction-safe sampler stream. Qualification
fails unless all six validators converge, every literal receipt is accepted
and final, every selected finalized height uses one durable database transaction
per validator, full-history work is zero, page work stays bounded, both 110%
latency gates pass, and the independent packet verifier recomputes the result.

Candidate and harness provenance are separate. `--expected-source-revision`
must name the source embedded in the qualification binary. The campaign also
records the clean evidence-runner checkout revision and hashes the paired,
selected, shared, and specification inputs. The setup-only batch builder embeds
the runner checkout revision, is built in release mode, and is independently
SHA-256 bound; it is not the candidate node binary. Packet assembly records its own
clean checkout revision instead of falsely claiming that a later harness commit
is the candidate binary source.

Build the current binary from the exact clean source under qualification. When
Zig is the available linker, use the repository wrappers:

```bash
export POSTFIAT_ZIG=/path/to/pinned/zig
export CC="$PWD/scripts/zig-cc"
export AR="$PWD/scripts/zig-ar"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$PWD/scripts/zig-cc"
cargo build --release --locked -p postfiat-node
```

Build the setup-only helper from the clean evidence-runner checkout with the
same pinned toolchain and linker environment:

```bash
cargo build --release --locked -p postfiat-bench \
  --bin postfiat-storage-corpus-batches
```

Then run from the clean selected checkout:

```bash
timeout --signal=INT --kill-after=120s 7200s \
python3 -u benchmarks/storage-scaling/run_paired_campaign.py \
  --node-bin target/release/postfiat-node \
  --batch-builder-bin target/release/postfiat-storage-corpus-batches \
  --expected-source-revision FULL_CANDIDATE_SOURCE_ID \
  --output-dir /explicit/disposable/storage-scaling-paired
```

Resume only the same bound output after a clean interruption:

```bash
timeout --signal=INT --kill-after=120s 7200s \
python3 -u benchmarks/storage-scaling/run_paired_campaign.py \
  --node-bin target/release/postfiat-node \
  --batch-builder-bin target/release/postfiat-storage-corpus-batches \
  --expected-source-revision FULL_CANDIDATE_SOURCE_ID \
  --output-dir /explicit/disposable/storage-scaling-paired \
  --resume
```

`run_paired_campaign.py --development-smoke` runs one round in the selected and
legacy lanes from one shared height-2 snapshot and signed corpus, then advances
the selected prepared fleet to height 3 and runs a second selected window with
no portable snapshot. That final row exercises disposable corpus generation,
source immutability, scratch discard, and the snapshot-free prepared-fleet
checkpoint. The
`--development-stop-after-units N` hook creates a controlled checkpoint stop so
interrupt/resume behavior can be exercised; neither mode can produce release
evidence. `run_campaign.py --development-smoke` remains the narrower
selected-store helper.

The paired run directory contains disposable private keys and must never be
published. Only the normalized latency reports and redaction-safe resource
sample streams copied by packet assembly may be published after independent
verification and redaction. Neither harness contacts or mutates the controlled
devnet.

## Existing-chain migration and activation workflow

The existing controlled chain cannot gain this feature by editing genesis.
Each validator must first stop a disposable clone, preserve an immutable copy
of the exact legacy data directory, and rebuild a new generation side by side:

```bash
postfiat-node storage-rebuild-transactional \
  --data-dir /disposable/legacy-working-clone \
  --output-dir /disposable/new-generation \
  --expected-tip TIP_SHA3_384 \
  --expected-state-root STATE_ROOT_SHA3_384 \
  --offline-confirmed

postfiat-node storage-rebuild-transactional \
  --data-dir /disposable/legacy-working-clone \
  --output-dir /disposable/new-generation \
  --expected-tip TIP_SHA3_384 \
  --expected-state-root STATE_ROOT_SHA3_384 \
  --verify-only \
  --offline-confirmed
```

The first command authenticates and replays legacy history, builds a separate
transactional generation, verifies its logical contents, writes a
checksum-bound migration manifest, writes the deterministic
`canonical-history.jsonl` audit export, and atomically publishes the generation
pointer. It refuses a non-empty output directory and reports required and
available disk. The second command independently rechecks the published
logical store, migration manifest, and canonical export against the
authenticated database. `--verify-only` opens both the source and target
read-only: it never creates a missing directory, integrity key, or database;
never recovers or removes a pending ordered-commit journal; never persists a
reconstructed chain tip, JSONL v1-head upgrade, or crash-suffix repair; and
never rewrites the manifest or canonical export. A pending source journal and
any state requiring repair fail closed so the operator can recover it through a
separate writable workflow before retrying verification. A missing, truncated,
or valid-but-foreign export likewise fails closed without changing the source,
database, or published generation. Neither command schedules activation.

Activation is a Foundation-governance workflow, not a Cobalt action. Run it
only on six disposable clones until the complete packet passes. The signing
sequence is intentionally split so an unsigned amendment cannot be confused
with a quorum-authorized one:

1. `storage-activation-template` freezes the current certified legacy tip, the
   fully verified migration packet root, and a future activation height. Its
   record contains a zero authorization placeholder.
2. `storage-activation-ratify` creates the exact unsigned Foundation amendment
   template whose kind binds the activation ID.
3. Each validator independently runs `governance-authorization-sign` over that
   frozen amendment. `governance-amendment-assemble` then verifies and combines
   the signed authorization files at the same proposal slot into the signed
   amendment.
4. `storage-activation-batch` injects that signed amendment ID into the frozen
   record, rechecks local migration readiness, and emits the governance batch
   that can be proposed through normal Consensus v2 finality.

A concrete artifact flow is:

```bash
postfiat-node storage-activation-template \
  --data-dir CLONE_DIR \
  --activation-height FUTURE_HEIGHT \
  --record-file activation-record.json

postfiat-node storage-activation-ratify \
  --data-dir CLONE_DIR \
  --record-file activation-record.json \
  --validators VALIDATOR_IDS_CSV \
  --support SUPPORTING_VALIDATOR_IDS_CSV \
  --amendment-file activation-amendment-unsigned.json

postfiat-node governance-authorization-sign \
  --data-dir CLONE_DIR \
  --amendment-file activation-amendment-unsigned.json \
  --validator VALIDATOR_ID \
  --validator-key-file VALIDATOR_KEY_FILE \
  --proposal-slot PROPOSAL_SLOT \
  --expires-at-height EXPIRY_HEIGHT \
  --authorization-file activation-authorization-VALIDATOR_ID.json

postfiat-node governance-amendment-assemble \
  --data-dir CLONE_DIR \
  --amendment-file activation-amendment-unsigned.json \
  --authorization-files AUTHORIZATION_FILES_CSV \
  --proposal-slot PROPOSAL_SLOT \
  --output activation-amendment-signed.json

postfiat-node storage-activation-batch \
  --data-dir CLONE_DIR \
  --record-file activation-record.json \
  --authorization-amendment-file activation-amendment-signed.json \
  --batch-file activation-batch.json
```

Before the activation height, cancellation uses the identical four-boundary
flow with `storage-cancellation-template`, `storage-cancellation-ratify`, the
same two generic signing/assembly commands, and
`storage-cancellation-batch`. Cancellation is rejected at or after activation.
After activation there is no chain rewind: an older software release is only a
valid rollback candidate if it understands the activated commitment version
and resumes from the same certified tip.

Run the complete sequence with six explicit, stopped, immutable source data
directories. The runner never discovers or contacts a fleet endpoint. It makes
separate backups and working clones, performs three rebuild/verify passes,
finalizes every transition through Consensus v2, refuses the incompatible v1
binary, holds one non-proposer validator behind at activation, catches it up
from the exact five-vote certificate, and rehashes every source and backup:

```bash
python3 benchmarks/storage-scaling/run_migration_rehearsal.py \
  --node-bin /CURRENT_CHECKOUT/target/release/postfiat-node \
  --incompatible-node-bin /V1_WORKTREE/target/release/postfiat-node \
  --incompatible-source-revision FULL_V1_COMMIT_ID \
  --source-data-dir /STOPPED/validator-0 \
  --source-data-dir /STOPPED/validator-1 \
  --source-data-dir /STOPPED/validator-2 \
  --source-data-dir /STOPPED/validator-3 \
  --source-data-dir /STOPPED/validator-4 \
  --source-data-dir /STOPPED/validator-5 \
  --validator-key-dir /ISOLATED/SPLIT-VALIDATOR-KEYS \
  --workload-key-file /ISOLATED/FUNDED-WORKLOAD-KEY.json \
  --workload-recipient FUNDED-TEST-RECIPIENT \
  --output-dir /explicit/disposable/storage-six-clone-migration \
  --expected-source-revision "$(git rev-parse HEAD)"
```

Evidence mode defaults to the exact controlled height-924 chain and requires a
clean checkout. `--development-smoke` permits an explicitly supplied local
fixture and dirty source, but its report is permanently ineligible for packet
publication. Raw output contains disposable key material; only
`six-clone-migration-report.json` may enter packet assembly after the offline
verifier accepts its binary identities, ten phases, restart receipts, clone
roots, backups, and unchanged Consensus/Cobalt boundaries.

## Offline rollback and tamper qualification

Build the current source and a distinct compatible ancestor as release
binaries. From a clean current checkout, the rollback harness starts six local
validators, finalizes height 2 with the current binary, resumes that exact tip
and finalizes height 3 with the older binary, then resumes height 3 and
finalizes height 4 with the current binary:

```bash
python3 benchmarks/storage-scaling/run_rollback_rehearsal.py \
  --node-bin /CURRENT_CHECKOUT/target/release/postfiat-node \
  --rollback-node-bin /OLDER_WORKTREE/target/release/postfiat-node \
  --output-dir /explicit/disposable/storage-compatible-rollback \
  --expected-source-revision "$(git rev-parse HEAD)" \
  --rollback-source-revision FULL_OLDER_COMMIT_ID
```

A dirty-checkout run is allowed only with `--development-smoke`; its report is
explicitly not evidence eligible. A clean `PASS` report is an input to the
closed tamper/crash matrix:

```bash
python3 benchmarks/storage-scaling/run_tamper_evidence.py \
  --output-dir /explicit/disposable/storage-tamper \
  --expected-source-revision "$(git rev-parse HEAD)" \
  --rollback-report \
    /explicit/disposable/storage-compatible-rollback/compatible-rollback-report.json
```

The tamper runner executes the frozen original 48-case E3 campaign and every
selected storage, node, crash, snapshot, migration, activation, catch-up, vote
refusal, and compatible-rollback owner test. Because the original E3 manifest
hash-binds an older source tree, the runner first verifies that frozen manifest
by its fixed SHA-256, preserves its exact live binding and case lists, and emits
a derived manifest that changes only the five audited source hashes to the
current revision. Both manifests' provenance, the derived manifest, and the
full independently verified E3 report are checksum-bound. The runner rejects a
zero-test filter and emits one checksum-bound receipt for every closed case.
Packet assembly must include three distinct release binaries so the offline
verifier can separately bind the current release, compatible rollback release,
and deliberately incompatible activation-fence probe. All three performance
lanes are bound to the current release binary above:

```bash
python3 benchmarks/storage-scaling/package_packet.py \
  --output-dir /explicit/disposable/storage-packet \
  --source-revision "$(git rev-parse HEAD)" \
  --captured-at YYYY-MM-DDTHH:MM:SSZ \
  --node-bin /CURRENT_CHECKOUT/target/release/postfiat-node \
  --batch-builder-bin /RUNNER_CHECKOUT/target/release/postfiat-storage-corpus-batches \
  --rollback-node-bin /OLDER_WORKTREE/target/release/postfiat-node \
  --incompatible-node-bin /V1_WORKTREE/target/release/postfiat-node \
  --state-distinction /PATH/state-distinction.json \
  --replay-report /PATH/replay-report.json \
  --performance-report /PATH/performance-report.json \
  --tamper-report /explicit/disposable/storage-tamper/tamper-report.json \
  --migration-report /PATH/migration-report.json
```

All paths above are operator procedures, not deployment authorization. Do not
probe, restart, or mutate the live fleet from this harness. Do not publish
validator keys, clone directories, authorization files, host paths, or raw
receipts.
