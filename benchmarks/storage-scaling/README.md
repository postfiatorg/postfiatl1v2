# Storage-scaling evidence

Status: **PUBLIC TESTNET BLOCKED**

This directory owns implementation evidence for the
[locked storage scaling fix specification](../../docs/architecture/storage-scaling-fix-spec.md).

The earlier [e2-bounded-work.json](e2-bounded-work.json) packet is retained as
historical development evidence for the JSONL-head and fixed-bitmap prototype.
It is not release evidence: its elapsed values came from debug unit tests, and
the fixed bitmap was rejected as the selected primary store.

## Release campaign harness

`run_campaign.py` creates a fresh six-validator Consensus v2 chain with the
transactional storage commitment active at height 1. It advances authenticated
snapshots to starting heights 50, 100, 500, 1,000, and 5,000, then runs five
independent 50-round full-vote windows from each height. Every measured window
uses fresh nodes restored from the same height-bound snapshot.

The runner fails closed unless:

- the checkout is clean and exactly matches the requested source revision;
- all six validators converge before and after every window;
- every receipt is accepted and finalized;
- each validator performs exactly one durable database commit per height;
- in-process telemetry reports zero full-history scans, records, and bytes
  during measured rounds; and
- logical page work remains within the locked per-round bound.

It records latency, CPU, RSS, disk growth, process I/O, network I/O, logical
page work, database records/bytes, transaction count, and fsync count/time.
The historical height-50 comparator is recomputed from the first 50 rounds of
the checksum-bound Cobalt E4 baseline packet.

Build and run only from a clean committed checkout:

```bash
CC=/home/postfiatchad/.local/bin/zig-cc \
CFLAGS='--target=x86_64-linux-gnu' \
AR=/home/postfiatchad/.local/zig-0.17.0-dev.1857/zig \
ARFLAGS='ar' \
RUSTFLAGS='-C linker=/home/postfiatchad/.local/bin/zig-cc' \
cargo build --release -p postfiat-node

python3 benchmarks/storage-scaling/run_campaign.py \
  --node-bin target/release/postfiat-node \
  --output-dir /explicit/disposable/storage-scaling-run \
  --expected-source-revision "$(git rev-parse HEAD)"
```

The run directory contains disposable private keys and must never be published.
Only an explicitly selected, redaction-checked, checksum-bound packet is
publishable. The harness never contacts or mutates the controlled devnet.

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
checksum-bound migration manifest, and atomically publishes the generation
pointer. It refuses a non-empty output directory and reports required and
available disk. The second command independently rechecks the published
logical store and manifest. Neither command schedules activation.

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

All paths above are operator procedures, not deployment authorization. Do not
probe, restart, or mutate the live fleet from this harness. Do not publish
validator keys, clone directories, authorization files, host paths, or raw
receipts.
