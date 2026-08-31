# Devnet storage live-canary rollback - 2026-08-30

- **Classification:** deployment-qualification failure; one-validator canary
- **Live impact:** validator-1 RPC unavailable during the failed canary
- **Final state:** rollback complete; all six validators healthy at height 924
- **Candidate disposition:** **DO NOT DEPLOY `10dd9f20`**

## Executive summary

Successor candidate `10dd9f20` passed the existing exact-height G6 clone
rehearsal, but the first live canary exposed a deployment topology that G6 did
not exercise. The live validator runs two separate long-lived processes against
one data directory:

- `transport-validator-serve`; and
- `rpc-serve`, which also owns the mempool-to-finality path.

Once the transactional generation pointer was published, the transport process
opened the `redb` database and retained its write handle. The RPC process then
failed at startup because `redb` would not grant a second process access to
the same writable database:

```text
storage_database_error: Database already open. Cannot acquire lock.
```

Starting the services in the opposite order would only move the failure to the
transport process. This is not a transient restart-order problem.

The rollout stopped after validator-1. The retained signed deployment was
restored. The rebuild had also upgraded three authenticated JSONL checkpoint
heads from v1 to v2 in the live source directory, so binary rollback alone was
not sufficient: the deployed old binary rejected those heads. Their underlying
logs still matched the exact stopped-clone backup, allowing the original heads
to be restored byte-for-byte.

A final six-host probe found every validator, RPC, and advisory shadow service
active on the original binary, with all six converged at height 924. No block,
governance action, storage activation, or Cobalt transition occurred. Z1 did
not start.

## Why the automation session stopped

The terminal session ended on a provider conversation-state rejection:

```text
thinking or redacted_thinking blocks ... cannot be modified
```

The same transcript warned that an uncached request was resending roughly 335k
input tokens. The provider error means replayed provider-owned reasoning blocks
no longer matched the original message history. It is a model-session transport
failure, not a node or consensus error. It occurred after the validator-1 live
state had changed, so a new session had to discover and recover the actual host
state rather than assume the interrupted command sequence had rolled back.

## Sequence

### G6 successor rehearsal

The registry-history continuation repair in `2c7aa36f` was frozen as successor
source `10dd9f20`. Its release binary SHA-256 was
`0cc664a3b2057a48547b0898487b979b89a0ba96ccad922402b5746b65ad4183`.

The existing
`benchmarks/storage-scaling/run_migration_rehearsal.py` workflow reported:

- exact height-924 inputs;
- six transactional rebuild and verify-only passes;
- certified continuation through height 925;
- storage activation, cancellation, restart, catch-up, and forward-recovery
  phases on isolated clones; and
- `status=PASS` with `evidence_eligible=true`.

That result fixed the previously observed registry-history failure. It did not
qualify the production service topology.

### Live canary

Only validator-1 was attempted.

1. The candidate binary and signed service artifacts were staged and
   hash-verified.
2. The first transactional generation was placed outside the validator data
   directory. The signed systemd unit uses `ProtectSystem=strict` and permits
   writes only under the validator data and log directories, so the transport
   process failed with a read-only-filesystem error.
3. The generation was rebuilt inside the permitted data directory. The
   transport service then started successfully.
4. The RPC service repeatedly failed because the transport process already held
   the transactional database lock.
5. The mandatory stop rule was applied. No second validator was attempted.

The initial path error was procedural and can be prevented by validating the
exact signed unit sandbox against the proposed generation path. The
multi-process database ownership failure is architectural.

## Why G6 passed

G6 exercises a different process topology from the deployed host.

### It starts only transport listeners

The shared `start_validator` helper in
`benchmarks/cobalt-activate-or-retire/run_consensus_v2_cobalt_integration.py`
starts one `transport-validator-serve` process per clone. It never starts
`rpc-serve`.

The G6 round helper in
`benchmarks/storage-scaling/run_migration_rehearsal.py` explicitly avoids a
second process for validator-0 because that process would contend for the
staged `redb` generation lock. The rehearsal therefore already encoded the
fact that two processes could not share the generation, but its final report
still described the candidate as evidence-eligible.

### It does not exercise systemd

The clone processes run directly from the Python harness. They do not use the
signed systemd units, `ProtectSystem=strict`, or `ReadWritePaths`. G6 could
not catch the invalid first generation location.

### Its rollback claim is not a deployment rollback

The report field `pre_activation_rollback=true` refers to cancelling a
scheduled storage activation while continuing with the candidate binary. It
does not restore the deployed binary, signed units, or pre-rebuild source
directory.

The mixed-version probe also uses source `13c969f8`, not the actually deployed
rollback revision `8cc7d15e`. It proves one older storage schema refuses the
new generation; it does not prove that the retained live release can resume
after the rebuild's source-side changes.

### Gate control was too permissive

The active storage milestone still had an open G5 height-915 input. The working
session carried forward the predecessor's G4 evidence and treated the old G6
runner's PASS as permission to start the successor canary. That decision was
not backed by a deployment-exact gate or a standalone receipt binding the new
candidate to each waived requirement. Even for a controlled devnet, an inferred
carry-over decision cannot substitute for testing the production process and
rollback topology.

Future canary authorization must identify the exact candidate, enumerate any
accepted open gates, and reference a passing deployment-exact rehearsal. The
failed session's execution decision conveys no authority for another attempt.

## Storage mechanics

### Cross-process lock

`NodeStore::transactional_store` caches a writable
`TransactionalStore` for the process lifetime. The cache is process-local.
The underlying `redb::Database` enforces exclusive database ownership.

Both long-lived services call node status during startup using a writable
`NodeStore`:

- transport: `crates/node/src/transport_runtime.rs`;
- RPC: `crates/node/src/rpc_cli.rs`.

The first process therefore retains the database. The second fails before
becoming ready. A read-only toggle is not automatically a valid fix because the
RPC service also accepts mempool submissions and drives local finality work; its
write responsibilities must be traced before changing ownership.

### Source-side JSONL upgrade

A non-verify rebuild opens the source with writable `NodeStore::try_new` and
runs block verification. Reading a v1 JSONL checkpoint head through this path
upgrades it to the v2, tip-bound format. On validator-1 this changed the heads
for receipts, blocks, and batch archive while leaving the underlying logs
unchanged.

The deployed `8cc7d15e` RPC does not understand the new head format and failed
closed during terminal mempool reconciliation. Exact old-binary rollback
therefore requires either:

- a source migration that is backward compatible;
- a complete, verified restoration set for every source-side mutation; or
- an architecture in which rebuild is genuinely non-mutating.

Calling the current rebuild “offline” does not mean its source directory is
read-only.

## Recovery

The rollback used the retained signed release rather than reconstructing new
units:

- old binary SHA-256:
  `d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf`;
- old transport and RPC unit hashes matched the signed deployment manifest;
- the candidate units, pointer, upgraded heads, and transactional databases
  were preserved in the private incident directory;
- the live generation pointer was removed;
- the exact original v1 JSONL heads were restored only after their underlying
  logs matched the stopped-clone backups; and
- the old signed transport and RPC services were restarted.

After recovery, every storage-owned file compared equal to the pre-rollout
validator-1 clone except `node_state.json:last_run_unix`, which changed as
expected when the service restarted.

The final authenticated probe ran from `2026-08-30T23:00:24Z` through
`23:00:39Z` and found:

- all six validators at height 924;
- one identical tip and state root;
- empty mempools;
- all validator, RPC, and advisory shadow services active;
- the original binary on every validator; and
- no live transactional generation pointer on any validator.

## Required repair and new gate

No configuration-only workaround is deployment-eligible. The owning
architecture must establish one authoritative writer for the transactional
database. Viable designs include a combined node process that owns transport
and RPC listeners, or a dedicated storage owner with authenticated IPC. The
choice must preserve deterministic commits, signer safety, RPC finality, crash
recovery, and bounded work.

Before another live attempt, a replacement gate must:

1. install the exact signed release tree and systemd units in an isolated host
   or namespace;
2. place the generation at the exact production path allowed by the unit;
3. start transport and RPC concurrently against one transactional generation;
4. prove both readiness files remain live through certified rounds and restart;
5. drive at least one real RPC mempool-to-finality round;
6. stop and restore the exact deployed `8cc7d15e` binary and units;
7. prove every source-side mutation is either backward compatible or restored
   from a verified backup;
8. restart both old services and prove the exact original tip and state;
9. repeat the exercise with both service start orders; and
10. emit a redaction-safe receipt whose rollback fields mean binary and data
    rollback, not activation cancellation.

Only after that gate and a repeated six-clone rehearsal may a new candidate be
considered for a separately authorized canary.

## Evidence

- [Canary rollback receipt](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/storage-scaling/devnet-rollout/canary-rollback-20260830.json)
- [Active rollout plan](../plans/active/devnet-storage-rollout-plan.md)
- [Current chain state](../status/chain-state-current.md)

Raw databases, clones, service logs, validator material, and host backups remain
private and must not be published.
