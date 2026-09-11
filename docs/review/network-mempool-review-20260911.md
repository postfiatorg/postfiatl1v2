# Network and mempool admission review — 2026-09-11

This review covers `crates/network`, `crates/mempool_dag`, and the
non-signing paths in `crates/ordering_fast`. It is the A4 surface of the
[burn 3 campaign](qa-campaign-20260911-burn3-brief.md). The pass also traced
those crates through the node transport and RPC entry points needed to
establish hostile-peer reachability; signing and consensus-v2 signature
verification were not reviewed.

The review traced frame-size enforcement, transport connection handling,
topology membership and envelope authentication, RPC mempool rate admission,
batch transaction and payload limits, deterministic reference ordering, and
legacy admission-receipt aggregation. The pre-repair focused baseline was
green:

- `postfiat-network`: 9 tests passed.
- `postfiat-mempool-dag`: 15 tests passed.
- `postfiat-ordering-fast`: 32 tests passed.

## Findings

### 1. P1 — the validator service creates one unbounded worker per pre-auth connection

**Source:** `crates/node/src/transport_runtime.rs:895,933-978` and
`systemd/postfiat-validator-transport.service.example:12`.

The long-running validator listener allocates its worker-handle vector to the
operator-supplied `max_connections` and immediately spawns one operating-system
thread for every accepted TCP connection. No in-flight semaphore or peer
admission runs before the spawn. The shipped service example permits 10,000
connections with a 30-second read timeout, while the sibling block-vote
listener already limits in-flight workers to 16.

Concrete failure scenario: an unauthenticated host that can reach the
controlled transport bind opens thousands of sockets without sending a frame.
Each socket receives a thread and stack before any topology or ML-DSA check.
The process can exhaust thread, virtual-memory, or file-descriptor resources
well before the 10,000-connection lifetime budget is reached, taking validator
transport out of service.

The repair must bound simultaneously executing validator connection workers
independently of the lifetime connection budget and preserve shutdown cleanup
and per-connection timeouts.

### 2. P1 — one unauthenticated persistent connection can grow memory and logs without bound

**Source:** `crates/node/src/transport_runtime.rs:1066-1322,1328-1372` and
`crates/node/src/transport_cli.rs:554-584`.

A validator worker reads request frames in an unbounded `loop`. Every
successful response and every rejection is cloned into an append-only shared
`Vec`; when an invalid request is rejected, the connection remains open and
the loop reads another request. An optional event log also appends every
rejection. Neither the number of requests on one connection nor the retained
summary count has a cap.

Concrete failure scenario: one unauthenticated client sends a stream of
newline-terminated JSON objects with an unsupported schema. Each frame is
small and arrives before the read timeout. The service performs a fresh status
read, writes a rejection response and optional log event, retains the full
rejection (including a status snapshot), and returns to the same socket.
Memory and log growth continue without consuming another `max_connections`
slot, eventually exhausting the process or filesystem.

The repair must evict a connection after a rejected request, cap requests per
authenticated persistent connection, and retain only a bounded number of
in-memory report summaries while keeping exact saturating counters.

### 3. P2 — the standalone batch service retains unlimited unauthenticated rejections

**Source:** `crates/node/src/transport_runtime.rs:270-340`.

The standalone `transport-batch-serve` command terminates after
`max_batches` successful batches, but rejected connections do not consume
that budget. Every rejection is retained in a report vector, and connections
are handled serially.

Concrete failure scenario: before any valid sender arrives, an
unauthenticated client repeatedly connects and sends a malformed or unsigned
batch envelope. Each failure appends a status-bearing rejection and starts
another accept iteration. The report grows without bound; a client that sends
no newline can additionally hold the only handler until the full read timeout.

This command is not referenced by the maintained service units, so the impact
is lower than findings 1 and 2. The repair must give rejected attempts a bound
derived from the requested successful-batch budget and return a fail-closed
error when that bound is exhausted.

## P3 observations

### 4. P3 — the legacy validator-set type can be deserialized with a false quorum

**Source:** `crates/ordering_fast/src/lib.rs:54-72,357-408,1434-1447`.

`ValidatorSet::try_new` canonicalizes validators and computes the BFT quorum,
but the type exposes public fields and derives `Deserialize`. Legacy
certificate routines trust the supplied `quorum` and sorted membership
without revalidating that the value came from `try_new`.

Concrete failure scenario: a caller deserializes four validators with
`quorum: 1`, constructs one hash-only legacy vote, and obtains a
`QuorumCertificate` from `certify_proposal`. Current node production paths
construct local sets with `try_new`, and consensus-v2 certificates are
separately signature verified, so no unauthenticated production path to the
malformed set was found. This unreachable legacy API invariant is recorded
under the P3-only rule and is not repaired in this campaign.

## Areas with no findings

- Wire reads are bounded to 4 MiB, require a newline, reject invalid UTF-8, and
  run under read and write timeouts.
- Maintained transport envelopes bind route, topology, payload length, payload
  hash, message ID, and an ML-DSA identity from the committed validator
  registry before batch application, proposal reconstruction, or voting.
- Public RPC mempool mutation is opt-in and applies bounded active
  connections, bounded request frames, and both per-peer and global
  submission-rate limits before mutation dispatch.
- Mempool batch references validate each transaction family, reject more than
  1,024 transactions, reject canonical payloads above 1 MiB, bind the chain
  domain, and recompute the batch ID during verification.
- Deterministic batch-reference ordering removes exact duplicates. The node
  currently orders one already verified reference at a time.
- Admission-receipt aggregation verifies committee membership, target and age
  binding, ML-DSA signatures, distinct validators, canonical order, and the
  conservative maximum bucket.

## Repair status

Findings 1 through 3 require source repairs and focused regressions. Finding 4
remains recorded under the P3 rule. None of these repairs changes a consensus
rule, state-transition result, or on-disk storage format.
