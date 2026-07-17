# Research Request: Consensus Finality Latency Reduction

Status: active research request
Date: 2026-05-14
Audience: distributed systems, BFT consensus, blockchain performance, storage,
and post-quantum cryptography researchers
Priority: P0 for controlled testnet credibility

## One-Sentence Ask

Given the current PostFiat L1 codebase, our Cobalt-derived canonical validator
governance, our peer-certified/HotStuff-like ordering path, and our
post-quantum validator signature constraints, identify the fastest safe path to
reduce transparent transaction finality latency as far as possible without
weakening deterministic safety or quantum-resistance claims.

## Important: Assume No Repository Access

This request must be answerable without access to the PostFiat repository.
Repository paths and line numbers are included only so the PostFiat team can
trace the observations internally. Researchers should rely on the code excerpts,
pseudocode, measurements, constraints, and architecture notes in this document.

## Why This Request Exists

The current codebase has real transparent settlement, ML-DSA-signed wallets,
validator certificates, validator-registry governance, RPC finality proofs, and
5-validator evidence. However, the measured local finality path is not credible
for a controlled testnet that wants to be compared with XRP-like settlement
systems.

The latest 5-validator local benchmark shows:

| Metric | p50 | p95 | p99 | Max |
| --- | ---: | ---: | ---: | ---: |
| quote RPC | 18 ms | 20 ms | 22 ms | 22 ms |
| wallet sign | 88 ms | 109 ms | 141 ms | 141 ms |
| submit RPC | 66 ms | 68 ms | 71 ms | 71 ms |
| mempool batch | 53 ms | 59 ms | 61 ms | 61 ms |
| certified round | 8,988 ms | 15,407 ms | 15,909 ms | 15,909 ms |
| tx finality RPC | 1,138 ms | 2,023 ms | 2,192 ms | 2,192 ms |
| submit to certified | 9,182 ms | 15,606 ms | 16,089 ms | 16,089 ms |
| submit to finality | 10,687 ms | 18,039 ms | 18,698 ms | 18,698 ms |

Evidence:

- `reports/testnet-tx-finality-latency-benchmark/current-20260514T124520Z/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-tx-finality-latency-benchmark/current-20260514T124520Z/logs/iterations.jsonl`

The most alarming signal is linear growth with block height:

| Iteration | Height | Certified Round | Tx Finality RPC | Submit To Finality |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 2 | 2.73 s | 0.28 s | 3.51 s |
| 10 | 11 | 8.99 s | 1.14 s | 10.69 s |
| 20 | 21 | 15.91 s | 2.19 s | 18.70 s |

This strongly suggests hot-path O(height) behavior, serial network orchestration,
or both. We need researchers to tell us the best target architecture and the
shortest safe implementation path.

## Self-Contained Current Code State

This section summarizes the current behavior without requiring the repository.

### Benchmark Shape

The current latency benchmark is a shell harness. It defaults to 5 validators,
20 measured rounds, 5-second RPC timeouts, and 30-second process timeouts:

```sh
VALIDATORS="${VALIDATORS:-5}"
ROUNDS="${ROUNDS:-20}"
TIMEOUT_SECONDS="${TIMEOUT_SECONDS:-30}"
RPC_TIMEOUT_MS="${RPC_TIMEOUT_MS:-5000}"
POLL_MS="${POLL_MS:-10}"
SEND_RETRIES="${SEND_RETRIES:-1}"
```

For each measured round, the benchmark:

1. chooses the deterministic proposer for the next height;
2. waits for all validators to be at the current height;
3. starts short-lived validator service processes for every non-proposer;
4. asks RPC for a fee quote;
5. signs one wallet transfer;
6. submits the signed transfer by RPC;
7. turns exactly one pending transaction into a batch;
8. runs one peer-certified consensus round;
9. waits for all validators to reach the new height;
10. queries finality for the submitted transaction by RPC;
11. records timing samples.

The benchmark intentionally uses one transaction per block:

```sh
postfiat-node mempool-batch \
  --data-dir "$source_data_dir" \
  --batch-file "$batch_file" \
  --max-transactions 1
```

The consensus timing window wraps one `node-run-peer-certified` invocation:

```sh
consensus_start_ns="$(now_ns)"
scripts/node-run-peer-certified >"$loop_report" 2>"$loop_err"
consensus_end_ns="$(now_ns)"
```

The finality query requires the response to report that the block log was
verified:

```sh
jq -e '
  .ok == true
  and .result.schema == "postfiat-tx-finality-v1"
  and .result.confirmed == true
  and .result.receipt.accepted == true
  and .result.block_log_verified == true
' "$tx_response"
```

### Current Peer-Certified Round

The current measured consensus round is not a long-running production event
loop. It is a function that performs the full round as a direct sequence of
blocking steps.

Current behavior in pseudocode:

```text
peer_certified_round(options):
  create artifact directories
  topology = read_topology_file()
  local_status = status(local_data_dir)
  validate local status against topology

  proposal = propose_batch(local_data_dir, batch_file, proposal_key)
  ensure deterministic local proposer if required

  targets = all peers except local node

  local_vote = create_block_vote(local_data_dir, proposal, batch)

  vote_files = [local_vote_file]
  vote_requests = []
  for target in targets:
    request = transport_block_vote_request_with_retries(target, proposal, batch)
    validate returned vote belongs to target
    vote_files.push(target_vote_file)
    vote_requests.push(request)

  certificate = aggregate_block_certificate(vote_files)
  require certificate vote count >= quorum
  require all validator votes unless peer failures are allowed

  sends = []
  for target in targets:
    send = transport_batch_send_with_retries(target, batch, certificate)
    sends.push(send)

  local_receipts = apply_transport_batch(local_data_dir, batch, certificate)
  local_status_after = status(local_data_dir)
  validate local status against topology

  return report including vote request evidence, send evidence, local apply
```

Two critical sections are serial today. Vote collection is serial:

```rust
for target in &targets {
    let vote_file = vote_dir.join(format!("{target}.block_vote.json"));
    let request = match transport_block_vote_request_with_retries(
        options.data_dir.clone(),
        options.topology_file.clone(),
        target.clone(),
        Some(proposal.batch_kind.clone()),
        options.batch_file.clone(),
        proposal_file.clone(),
        vote_file.clone(),
        Some(proposal.block_height),
        options.timeout_ms,
        options.send_retries,
        options.retry_backoff_ms,
    ) {
        Ok(request) => request,
        Err(error) => { /* fail or record peer failure */ }
    };
    validate_vote_validator_matches_target(request, target)?;
    vote_files.push(vote_file);
    vote_requests.push(request);
}
```

Certified batch broadcast is also serial:

```rust
for target in targets {
    let send = match transport_batch_send_with_retries(
        options.data_dir.clone(),
        options.topology_file.clone(),
        target.clone(),
        Some(certification.batch_kind.clone()),
        options.batch_file.clone(),
        Some(certificate_file.clone()),
        options.timeout_ms,
        options.send_retries,
        options.retry_backoff_ms,
    ) {
        Ok(send) => send,
        Err(error) => { /* fail or record peer failure */ }
    };
    sends.push(send);
}
```

### Current Validator Service Shape

The validator service used in the benchmark is short-lived and connection-count
bounded. It is started per measured round for every non-proposer with
`--max-connections 2`, because each remote peer expects one vote request and
one certified-batch send.

Current behavior in pseudocode:

```text
transport_validator_serve(data_dir, topology, key_file, max_connections):
  create vote directory
  topology = read_topology_file()
  local_status = run_once(data_dir)
  validate local status against topology
  bind TCP listener

  for connection_index in 1..=max_connections:
    stream = listener.accept()
    line = read_transport_line(stream)
    if line is block vote request:
      validate proposal and batch
      create signed block vote
      write vote artifact
      reply with vote
    else if line is certified batch:
      validate certificate and batch
      apply batch
      reply with ack and state
    else:
      reject

  return service report
```

Key point: this is not yet a persistent validator with hot in-memory consensus
state, persistent peer connections, pipelined block processing, or an indexed
finality service.

### Current Finality Query Shape

The current transaction finality RPC path verifies the whole block log before
answering one transaction query.

Current behavior in pseudocode:

```text
tx_finality(data_dir, tx_id):
  validate tx_id format
  genesis = read_genesis()
  verification = verify_blocks(data_dir)

  receipts = read_receipts()
  matching_receipts = receipts where receipt.tx_id == tx_id
  require exactly one matching receipt

  blocks = read_blocks()
  scan every block and every receipt_id in each block
  require exactly one block links the receipt

  proof_id = hash(genesis, receipt, receipt_index, block)

  return:
    confirmed = true
    receipt
    receipt_index
    receipt_count
    block
    block_log_verified = verification.verified
```

Representative code shape:

```rust
pub fn tx_finality(options: TxFinalityQueryOptions) -> io::Result<TxFinalityReport> {
    validate_finality_tx_id(&options.tx_id)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let verification = verify_blocks(NodeOptions {
        data_dir: options.data_dir.clone(),
    })?;
    let receipt_log = store.read_receipts()?;
    let matching_receipts = receipt_log
        .iter()
        .filter(|receipt| receipt.tx_id == options.tx_id)
        .cloned()
        .collect::<Vec<_>>();
    let blocks = store.read_blocks()?;
    for block in &blocks.blocks {
        for (index, receipt_id) in block.receipt_ids.iter().enumerate() {
            if receipt_id == &options.tx_id {
                matches.push((block.clone(), index as u64));
            }
        }
    }
    /* build finality report */
}
```

### Current Full Block Verification Shape

The full verifier is audit-oriented. It reads aggregate files and walks the
entire retained block history.

Current behavior in pseudocode:

```text
verify_blocks(data_dir):
  genesis = read_genesis()
  blocks = read_blocks()
  ordered_batches = read_ordered_batches()
  archive = read_batch_archive()
  receipts = read_receipts()
  governance = read_governance()
  history_checkpoint = read_optional_history_checkpoint()
  live_validator_registry = read_validator_registry()
  certificate_validator_registry = read_replay_base_registry()

  build receipt count maps
  status = status(data_dir)
  parent_hash = checkpoint hash or "genesis"
  replay governance and validator registry state across heights

  for each block in blocks:
    verify expected height
    verify ordered batch id
    verify parent hash
    verify receipt count and receipt references
    activate validator-registry updates for height
    verify block certificate evidence
    recompute block hash
    find archived batch payload
    verify payload hash
    verify archived payload
    update governance replay state
    parent_hash = block_hash

  verify no orphan archive entries
  verify receipt counts match block references
  replay_state_root = verify_replayed_blocks(genesis, blocks, archive)
  compare replay/tip state root to current status
  return verified report
```

Representative code shape:

```rust
pub fn verify_blocks(options: NodeOptions) -> io::Result<BlockVerificationReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let blocks = store.read_blocks()?;
    let ordered_batches = store.read_ordered_batches()?;
    let archive = store.read_batch_archive()?;
    let receipts = store.read_receipts()?;
    let governance = store.read_governance()?;
    let status = status(options)?;

    for block in blocks.blocks.iter() {
        verify_block_certificate_evidence(...)?;
        let expected_hash = block_hash(...)?;
        verify_archived_payload(&genesis, block, archive_entry)?;
        update_governance_for_certificate_replay(...)?;
    }

    let replay_state_root = verify_replayed_blocks(&store, &genesis, &blocks, &archive)?;
    compare_replay_or_tip_root_to_status(replay_state_root, status)?;
    Ok(report)
}
```

### Current Commit Storage Shape

The current commit path writes aggregate JSON-like logs for receipts, ordered
batches, batch archive, and blocks:

```rust
fn write_ordered_commit(store: &NodeStore, commit: &OrderedCommitArtifacts) -> io::Result<()> {
    store.write_receipts(&commit.receipts)?;
    store.write_ordered_batches(&commit.ordered_batches)?;
    store.write_batch_archive(&commit.archive)?;
    store.write_blocks(&commit.blocks)
}
```

The research question is not whether this is acceptable for evidence capture.
It is whether this shape can ever support low-latency validators, and what
append/index/snapshot model should replace or supplement it.

## Current System Context

PostFiat L1 is intended to be a quantum-resistant, privacy-roadmapped,
XRP-like settlement chain with Cobalt-derived validator governance.

Important current design constraints:

- The controlled-testnet validator cohort is canonical-UNL style: a known
  validator set with governance-controlled admission, suspension, removal, and
  key rotation.
- The governance story is Cobalt-derived canonical validator governance, not
  full arbitrary non-uniform Cobalt trust graphs yet.
- Ordering/finality currently uses a peer-certified block path: a deterministic
  proposer produces a batch proposal, validators sign block votes, a quorum
  certificate is assembled, and certified batches are applied/broadcast.
- Transparent account and validator authentication use ML-DSA-style
  post-quantum signatures. BLS-style aggregation is not available as a
  production-safe assumption.
- Current quorum in the 5-validator benchmark is 4 of 5.
- We need deterministic state transitions, auditable finality proofs, bounded
  public RPC behavior, replayable evidence, and no hidden trusted sequencer.
- We can change engineering architecture aggressively. We should not "solve"
  latency by weakening finality, using non-PQ validator authentication, hiding
  safety assumptions, or dropping auditable certificates.

## Current Code Path Being Measured

This section gives internal traceability for the PostFiat team. Researchers
without repository access can rely on the self-contained code-state section
above.

The benchmark script is:

- `scripts/testnet-tx-finality-latency-benchmark`

Important script references:

- Lines 4-25: benchmark configuration via environment variables; default
  `VALIDATORS=5`, `ROUNDS=20`, `TIMEOUT_SECONDS=30`, `RPC_TIMEOUT_MS=5000`.
- Lines 246-274: starts per-round `transport-validator-serve` processes for
  non-proposer validators with `--max-connections 2`.
- Lines 357-361: creates a mempool batch with `--max-transactions 1`, so each
  measured transaction pays the whole consensus cost.
- Lines 366-382: measures `certified_round_ms` around
  `scripts/node-run-peer-certified`.
- Lines 421-454: runs the finality RPC query and requires
  `result.block_log_verified == true`.
- Lines 463-505: writes per-iteration latency samples.

The measured node path is primarily:

- `crates/node/src/main.rs:4372`:
  `transport_peer_certified_batch_round`.
- `crates/node/src/main.rs:4396-4401`: reads topology and local node status.
- `crates/node/src/main.rs:4403-4414`: builds/signs the block proposal.
- `crates/node/src/main.rs:4447-4457`: creates the local block vote.
- `crates/node/src/main.rs:4469-4512`: sends block-vote requests to peers in a
  serial loop.
- `crates/node/src/main.rs:4514-4522`: aggregates the block certificate.
- `crates/node/src/main.rs:4561-4585`: sends the certified batch to peers in a
  serial loop.
- `crates/node/src/main.rs:4587-4597`: locally applies the certified batch and
  reads status.

Validator service path:

- `crates/node/src/main.rs:3786`: `transport_validator_serve`.
- `crates/node/src/main.rs:3803-3807`: service startup calls `run_once` before
  binding and accepting requests.
- `crates/node/src/main.rs:3825+`: accepts a bounded number of connections
  rather than operating as a long-running peer process.

Block verification/finality path:

- `crates/node/src/lib.rs:9392`: `verify_blocks`.
- `crates/node/src/lib.rs:9395-9399`: reads blocks, ordered batches, batch
  archive, receipts, and governance.
- `crates/node/src/lib.rs:9473-9597`: walks all blocks and verifies certificate,
  payload, receipt, parent, and governance evidence.
- `crates/node/src/lib.rs:9623`: calls `verify_replayed_blocks`, which replays
  block execution.
- `crates/node/src/lib.rs:7657`: `tx_finality`.
- `crates/node/src/lib.rs:7661-7663`: `tx_finality` calls `verify_blocks`
  before answering a single transaction finality query.
- `crates/node/src/lib.rs:7664-7695`: finality query scans receipts and blocks
  to locate the transaction.

Storage/apply path:

- `crates/node/src/lib.rs:12751-12755`: `write_ordered_commit` rewrites
  receipts, ordered batches, archive, and blocks.

Local workspace note:

- A local uncommitted experiment added a `verify_block_log` flag to
  `BatchProposalOptions` and disabled full block-log verification for the
  peer-certified proposal hot path. `cargo check -p postfiat-node` passed, but
  a partial follow-up benchmark still showed certified-round latency growing
  badly. Researchers should treat this as evidence that proposal replay is not
  the only bottleneck.

## Working Diagnosis

We believe latency is high for several compounding reasons:

1. The validator path is shaped like an evidence/debug harness, not a
   production long-running consensus service.
2. Vote collection is serial across peers.
3. Certified batch broadcast is serial across peers.
4. Validator service processes are launched per measured round instead of
   being persistent, connected peers with hot state.
5. Finality RPC verifies and scans too much history for each transaction query.
6. Commit storage rewrites aggregate JSON logs instead of appending to indexed
   structures.
7. The benchmark uses one transaction per block, so every transaction absorbs
   full consensus overhead.
8. ML-DSA signatures and certificates are large enough that signature
   verification and payload movement need deliberate batching, indexing, and
   concurrency design.

We need this diagnosis challenged. If a different bottleneck is more likely,
identify it and explain how to prove it.

## Target Outcomes

For controlled testnet, we want:

- Local 5-validator transparent tx finality: p50 under 2 seconds, p95 under 4
  seconds.
- Multi-machine 5-validator transparent tx finality: p50 under 5 seconds, p95
  under 10 seconds.
- Latency must not grow linearly with block height.
- Finality RPC must be bounded and index-backed.
- Validator services must be long-running and production-shaped.
- Evidence must remain replayable and auditable.

If these targets are unrealistic under the current consensus design, explain
the theoretical and practical lower bounds and propose a more defensible target.

## Research Questions

Please answer these at PhD/distributed-systems depth, but with implementation
recommendations we can execute.

1. What is the minimum-round protocol we can safely run for a canonical
   validator-set, XRP-like settlement chain with deterministic finality?

2. Given canonical-UNL Cobalt-derived governance and 4-of-5 quorum, should the
   ordering path be:
   - current peer-certified proposal/vote/certify/apply;
   - HotStuff-family with pipelined phases;
   - Tendermint-style prevote/precommit;
   - XRP/RPCA-like ledger close with deterministic proposal convergence;
   - another protocol specialized for known-validator federated settlement?

3. Can Cobalt-derived governance remain the validator-set evolution layer while
   the ordering layer is optimized independently? If yes, what exact interfaces
   between governance epoch, validator registry root, and block certificate are
   required?

4. What is the best way to make vote collection optimistic and concurrent while
   preserving deterministic certificate artifacts?

5. Should the proposer wait for all validator votes in controlled testnet, or
   return once quorum is reached and record late votes separately? What are the
   safety, auditability, and liveness tradeoffs?

6. What certificate representation is optimal for ML-DSA validators?
   - full signatures in every block;
   - registry-root-bound compact votes;
   - separate certificate store with block header commitment;
   - Merkle commitment to vote set;
   - bitmap plus detached signatures;
   - another design?

7. Are there any credible production-ready post-quantum signature aggregation
   or multisignature techniques we should consider now, or should the protocol
   assume no aggregation and optimize around detached individual ML-DSA votes?

8. How should a low-latency validator service be structured in Rust?
   - sync TCP vs async Tokio;
   - persistent TCP vs QUIC;
   - actor model;
   - bounded queues and backpressure;
   - CPU-bound signature verification scheduling;
   - avoiding consensus nondeterminism.

9. What should be held in memory on the hot path?
   - current ledger state;
   - validator registry and epoch metadata;
   - current tip/header/certificate;
   - mempool;
   - tx receipt index;
   - certificate index.

10. What must be persisted synchronously before a validator can safely vote,
    certify, or report finality? What can be deferred, indexed asynchronously,
    or audited in the background?

11. How should finality RPC be designed so it is O(log n) or O(1) in the common
    case while still providing an auditable proof?

12. What retention/snapshot/checkpoint scheme lets validators avoid replaying
    full history while preserving safety for catch-up and audit nodes?

13. What batch-close policy should we use?
    - fixed close interval;
    - close on max tx count;
    - close on latency deadline;
    - adaptive close;
    - separate admission and consensus clocks.

14. Given one transaction per block is worst-case economics, what benchmark
    suite should replace the current one so it measures:
    - user-facing latency;
    - block close latency;
    - throughput;
    - p95/p99 under load;
    - catch-up behavior;
    - finality query latency?

15. What safety tests, simulation tests, Byzantine tests, and soak tests are
    required before we can claim the optimized path is credible?

## Requested Deliverable

Please produce a report with these sections:

1. **Executive Recommendation**
   - the protocol/runtime architecture we should implement;
   - whether the current peer-certified path should be evolved or replaced.

2. **Latency Critical Path**
   - exact expected critical path in message delays, disk fsyncs, CPU work, and
     signature operations;
   - theoretical lower bound and realistic local/WAN targets.

3. **Protocol Design**
   - ordering protocol;
   - quorum/certificate format;
   - governance epoch and registry-root binding;
   - proposer/leader rotation;
   - timeout/view-change behavior.

4. **Rust Runtime Design**
   - process model;
   - networking;
   - concurrency;
   - state ownership;
   - bounded queues;
   - persistence and crash recovery.

5. **Storage and Index Design**
   - hot state;
   - append-only logs;
   - receipt index;
   - certificate index;
   - snapshot/retention/archive roles.

6. **Implementation Plan**
   - first 24 hours;
   - first 72 hours;
   - first 2 weeks;
   - changes most likely to reduce latency immediately;
   - changes that are necessary but not on the critical path.

7. **Risks and Non-Negotiables**
   - safety risks;
   - liveness risks;
   - post-quantum crypto risks;
   - auditability risks;
   - where not to cut corners.

8. **Validation Plan**
   - metrics;
   - benchmarks;
   - simulation;
   - fault drills;
   - release gates.

## Constraints To Preserve

Do not recommend a solution that relies on:

- proof of work;
- economic staking/slashing as the primary safety mechanism;
- BLS aggregation as if it works for ML-DSA;
- trusted centralized sequencing;
- accepting probabilistic finality instead of deterministic finality;
- removing validator certificate evidence from finality proofs;
- weakening post-quantum validator authentication;
- unbounded mempool, RPC, or network queues;
- nondeterministic consensus inputs;
- hiding full verification in an unauditable service.

Researchers may recommend staged implementation if the final design is large,
but the staged plan must still drive directly toward the low-latency validator
architecture. The immediate goal is to turn the current correctness/evidence
harness into a credible controlled-testnet validator loop.
