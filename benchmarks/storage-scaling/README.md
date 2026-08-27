# Storage-scaling evidence

Status: **PUBLIC TESTNET BLOCKED**

This directory owns implementation evidence for the
[locked storage scaling fix specification](../../docs/architecture/storage-scaling-fix-spec.md).

The earlier [e2-bounded-work.json](e2-bounded-work.json) packet is retained as
historical development evidence for the JSONL-head and fixed-bitmap prototype.
It is not release evidence: its elapsed values came from debug unit tests, and
the fixed bitmap was rejected as the selected primary store.

## Release campaign harness

`run_paired_campaign.py` is the only release campaign entry point. It runs the
closed three-lane comparison in this order:

1. deployed-style full-prefix JSON/JSONL from source
   `8cc7d15edc58b5f5a0b745143fef2d45203465ff`;
2. authenticated JSONL v2 heads plus the fixed-slot index candidate from source
   `dfd0b9f11108b0b773d1e02bebae71685864228e`; and
3. the selected transactional `redb` store from the clean source under
   qualification.

The retired storage behaviors are not selectable modes in the current node, so
each is measured with the exact release binary that owned it. The report and
packet explicitly disclose that boundary and bind all three source revisions
and binary hashes. The lanes use the same validator keys, deterministic wallet
and recipient, semantic transfer input, six-validator loopback topology shape,
full-vote policy, CPU affinity, host, filesystem device, resource sampler, and
height/window cardinality. All lanes use the same 900-second fail-closed
request/server timeout so the known slow legacy curve can still be measured at
height 5,000; timeout time is not reported as latency. Snapshots are
authenticated and identical across the five windows for a lane and height, but
they are lane-native rather than
byte-identical across retired storage formats. ML-DSA signature randomness also
means independently executed blocks are not byte-identical across lanes.

At starting heights 50, 100, 500, 1,000, and 5,000, the harness runs five
independent 50-round windows. It derives the height-50 legacy baseline from the
raw legacy lane, recomputes p50/p95/p99/max/mean/standard deviation, publishes
per-window resource variance, and fits constant, logarithmic, and linear models
with raw observations, predictions, and residuals for every material stage.
Each window also preserves a redaction-safe, checksum-bound sampler stream; the
runner fails unless every foreground benchmark process appears in at least two
samples, and the packet verifier independently reconstructs CPU, RSS, disk,
process I/O, host load, memory, and network totals from that stream. The
selected lane fails unless all six validators converge, every literal
receipt is accepted and final, every finalized height uses one durable database
transaction per validator, full-history work is zero, page work stays bounded,
the two 110% latency gates pass, and no material stage retains a positive
linear relationship after the repeated-window variance allowance.

Build all three binaries with the pinned toolchain in their exact clean
worktrees. When Zig is the available linker, use the repository wrappers:

```bash
export POSTFIAT_ZIG=/path/to/pinned/zig
export CC="$PWD/scripts/zig-cc"
export AR="$PWD/scripts/zig-ar"
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$PWD/scripts/zig-cc"
cargo build --release --locked -p postfiat-node
```

Then run from the clean selected checkout:

```bash
python3 benchmarks/storage-scaling/run_paired_campaign.py \
  --legacy-node-bin /LEGACY_WORKTREE/target/release/postfiat-node \
  --legacy-source-revision 8cc7d15edc58b5f5a0b745143fef2d45203465ff \
  --bounded-node-bin /BOUNDED_WORKTREE/target/release/postfiat-node \
  --bounded-source-revision dfd0b9f11108b0b773d1e02bebae71685864228e \
  --node-bin target/release/postfiat-node \
  --expected-source-revision "$(git rev-parse HEAD)" \
  --output-dir /explicit/disposable/storage-scaling-paired
```

`run_campaign.py --development-smoke` is only the one-round selected-store
helper used while changing the harness. It cannot produce release evidence.
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
authenticated database. A missing, truncated, or valid-but-foreign export fails
closed without changing the database or published generation. Neither command
schedules activation.

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
Packet assembly must include five distinct release binaries so the offline
verifier can separately bind the current release, compatible rollback release,
deliberately incompatible activation-fence probe, frozen legacy performance
lane, and frozen bounded-JSONL performance lane:

```bash
python3 benchmarks/storage-scaling/package_packet.py \
  --output-dir /explicit/disposable/storage-packet \
  --source-revision "$(git rev-parse HEAD)" \
  --captured-at YYYY-MM-DDTHH:MM:SSZ \
  --node-bin /CURRENT_CHECKOUT/target/release/postfiat-node \
  --rollback-node-bin /OLDER_WORKTREE/target/release/postfiat-node \
  --incompatible-node-bin /V1_WORKTREE/target/release/postfiat-node \
  --legacy-performance-node-bin /LEGACY_WORKTREE/target/release/postfiat-node \
  --bounded-performance-node-bin /BOUNDED_WORKTREE/target/release/postfiat-node \
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
