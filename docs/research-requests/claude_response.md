## # PostFiat L1 — Consensus Finality Latency Reduction: Architectural Research Report

**Date:** 14 May 2026
**Audience:** PostFiat / AGTI distributed systems, consensus, storage, and PQ cryptography engineering
**Status:** P0 research deliverable for controlled-testnet credibility
**Scope:** Protocol redesign, Rust runtime, storage, validation plan to drive transparent-tx finality from current ~18.7s p99 to **sub-2s p50 / sub-4s p95** on a 5-validator local cluster (and sub-5s/sub-10s on multi-machine), without weakening deterministic finality, post-quantum authentication, or auditability.

This report is structured according to the eight mandatory sections in the brief. Quantitative budgets, pseudocode, and named Rust libraries are provided throughout. The current state machine is referenced as `Vc` (the existing peer-certified path). Where the recommendation diverges from `Vc`, the difference is explicit.

## TL;DR

## TL;DR

1. **The current 5-validator harness is not a consensus bottleneck — it is a benchmark artifact**. Linear height-dependent latency (2.73s → 15.91s over heights 2→21) is overwhelmingly caused by (a) **per-round process spawning of validator services**, (b) **serial vote collection and serial certified-batch broadcast** in `node-run-peer-certified`, (c) **O(height) finality RPC** that rewalks the entire block log on every query, and (d) **rewriting aggregate JSON logs on every commit**. These are runtime, networking, and storage problems, not algorithmic limits of the Cobalt-derived governance or your peer-certified ordering rule.

2. **Do not replace the protocol family yet.** Evolve the peer-certified path into a **two-phase, single-leader, pipelined BFT** in the Jolteon / HotStuff-2 family, retaining Cobalt-derived UNL/governance as a separate **epoch-and-registry-binding layer**. This is the smallest safe step that unlocks optimistic responsiveness with deterministic finality and a 4-of-5 quorum. DAG-based families (Mysticeti, Bullshark, Narwhal+Tusk) are *technically attractive* but introduce 3+ rounds of leader-anchor commit reasoning, large vote-bandwidth overhead under ML-DSA (signatures ~2.4–4.6 KB, no BLS aggregation possible), and a substantial engineering rewrite that is not justified before fixing the runtime.

3. **Treat ML-DSA as non-aggregatable in production for 2026.** All candidate lattice multi-signatures (Chipmunk, Squirrel, LaBRADOR-of-Falcon, threshold ML-DSA via Shamir nonce DKG) are either synchronous-only, bounded-state, Falcon-specific, or pre-production. The right design is: **bitmap + N detached ML-DSA-65 signatures**, batched verification on a Rayon pool, with the **canonical certificate stored once** in a separate column family and only a **32-byte certificate digest committed in the block header**. A future migration path (LaBRADOR for archival aggregation, threshold ML-DSA for client-visible signature compaction) is described.

4. **The finality RPC must become O(1) amortized**. Replay-then-scan must move off the query path and onto a background indexer. Replace `verify_blocks(entire_log)` with a `(tx_id) → (height, batch_index, intra_batch_index, block_hash, cert_digest)` lookup in a RocksDB column family populated transactionally with the commit. The RPC returns a Merkle inclusion proof against the committed block-tx-root and a reference to the certificate-store; full re-verification remains available as an explicit audit endpoint.

5. **Persistent long-running validator services + full-mesh QUIC + bounded mpsc channels.** Move from `--max-connections 2`/`run_once` short-lived processes to a small set of persistent services connected over QUIC (`quinn`/`tokio-quiche`) with one **persistent bidirectional stream per peer per message class**. Vote fan-out becomes a `JoinSet`-parallel broadcast. Bounded queues with explicit backpressure replace the implicit unbounded behaviour.

6. **Realistic targets are achievable.** With these changes, local 5-validator transparent finality of **p50 ≈ 600–900 ms, p95 ≈ 1.5–2.5 s** is achievable; multi-machine LAN/WAN-5 of **p50 ≈ 1.2–2.5 s, p95 ≈ 3–6 s** is achievable. This exceeds the stated targets. Latency stops growing with height once the indexer replaces the linear-replay finality RPC.

7. **No targets are achievable while one block carries one transaction**. That is a benchmark-shape problem. The replacement benchmark suite must measure block-close latency, submit-to-finality under load, p99/p95 with ≥100 tx/block, and catch-up latency separately.

## 1. Executive Recommendation

## 1. Executive Recommendation

### 1.1 Verdict on the peer-certified path

**Evolve, do not replace.** The current peer-certified ordering rule is structurally sound: it is a single-leader, quorum-certified proposal pattern with deterministic finality and explicit certificate evidence — i.e. the same family as Tendermint, HotStuff, Jolteon (DiemBFT v4), and Fast-HotStuff. The pathology in the benchmark is not in the safety/liveness logic but in three orthogonal places:

| Layer | Pathology | Fix family |
|---|---|---|
| **Validator runtime** | Short-lived processes; serial RPC fan-out; no persistent peer connections | Long-running async tokio service; full-mesh QUIC; `JoinSet` parallel I/O |
| **Storage** | `write_ordered_commit` rewrites aggregate JSON logs; `verify_blocks` walks entire log per finality query | RocksDB column families; append-only block log + secondary indexes; background indexer; bounded RPC |
| **Algorithm shape (small effect)** | Three-phase certify with a single leader can be folded into a two-phase Jolteon-style commit | Move to Jolteon / HotStuff-2 commit rule when the runtime is fixed |

The protocol-shape change in row 3 saves ~one network round-trip (i.e. ~tens to hundreds of ms in a single-host loopback, ~100–300 ms WAN). The runtime/storage fixes save **seconds**. Order the work accordingly: runtime/storage first, protocol-shape second.

### 1.2 Recommended target architecture

The recommended target architecture is the following stack:

```
   ┌──────────────────────────────────────────────────────────┐
   │            Client RPC (tx submit, fee quote,             │
   │                 tx_finality, tx_status)                  │
   │   • bounded request queue (tower::limit + tower::load)   │
   │   • O(1)/O(log n) tx_finality via receipt index          │
   └──────────────────────────────────────────────────────────┘
                            │
   ┌────────────────────────┴─────────────────────────────────┐
   │     Consensus actor (Jolteon-style 2-phase chained)      │
   │  • view = block height; proposer = det(view, registry)   │
   │  • prepare-QC → commit-QC (2-chain) over batch digests   │
   │  • registry_root + governance_epoch bound in proposal    │
   │  • detached ML-DSA-65 votes; bitmap + N sigs certificate │
   │  • Pacemaker: timeout_qc-based view-change (DiemBFT v4)  │
   └──────────────────────────────────────────────────────────┘
       │            │                 │                 │
   ┌───┴───┐   ┌────┴────┐    ┌───────┴───────┐    ┌────┴────┐
   │ Hot   │   │ Network │    │  CPU pool     │    │ Storage │
   │ state │   │ (QUIC)  │    │  (rayon)      │    │ (Rocks) │
   │ actor │   │ tokio   │    │  ML-DSA batch │    │ + WAL   │
   └───────┘   └─────────┘    └───────────────┘    └─────────┘
```

Concretely, the production-shape recommendation is:

- **Ordering protocol**: A 2-chain commit, single-leader, pipelined BFT (Jolteon / DiemBFT v4 / HotStuff-2 family). Quorum = `⌈2N/3⌉+1` = 4-of-5 for N=5. View = height. Pacemaker uses TC (timeout certificate) view-change as in DiemBFT v4. Cobalt-derived UNL/governance is layered above as the *epoch + registry_root authority* (see §3.4).
- **Certificate format**: `{block_header_hash, view, registry_root, voter_bitmap, [sig_i: ml_dsa_65]}`. No aggregation. The certificate is stored once in a `certificates` column family; the block header carries `cert_digest = H(certificate_bytes)`.
- **Vote dissemination**: Persistent full-mesh QUIC streams. Push-based vote propagation; pull-based catch-up. Bounded `tokio::mpsc` with capacity = max(2·N, 64) per direction per peer.
- **CPU**: ML-DSA-65 verifies are dispatched to a `rayon` global pool sized to `min(physical_cores − 2, 8)`, *not* `tokio::spawn_blocking` (which is sized for I/O, not CPU).
- **Storage**: RocksDB with separate column families: `blocks`, `block_by_height`, `tx_receipt_index`, `certificates`, `state_trie`, `validator_registry`, `mempool` (or a separate sled instance for mempool to avoid compaction pressure on the hot path).
- **Finality RPC**: `tx_finality(tx_id)` → single point-lookup in `tx_receipt_index` returning `(height, block_hash, merkle_path, cert_digest)`. Full re-verification moved to a `tx_audit(tx_id)` endpoint or to a long-running background `block_log_verifier` task that maintains `block_log_verified_through_height`.
- **Auditability**: All on-disk artifacts (blocks, certificates, receipts) are append-only and replayable. A separate `replay` binary regenerates the verified watermark; the verified watermark is itself a signed checkpoint that participating validators co-sign at retention boundaries.

### 1.3 What is explicitly *not* recommended

- **Not BLS aggregation, not adapter-BLS, not pretend-BLS over ML-DSA.** ML-DSA is a Fiat–Shamir-with-aborts lattice signature. No public-coin aggregation primitive maps onto it cleanly with production-ready parameters as of mid-2026 (see §3.7 for a survey).
- **Not Mysticeti / Bullshark / DAG-Rider as v1.** They achieve excellent commit latency (~0.5s WAN at 100 validators in Sui mainnet measurements) but require uncertified DAGs with implicit commit rules whose safety arguments depend on per-block aggregation patterns that interact poorly with non-aggregatable PQ signatures. Revisit in v3 if you obtain production-grade threshold ML-DSA (P1/P3+ profiles per Kao 2026) or migrate to Falcon+LaBRADOR aggregation.
- **Not XRP RPCA / native Cobalt as the ordering protocol.** Native Cobalt is a *governance* layer for non-uniform-trust UNL evolution. Its atomic-broadcast embedding (Chase–MacBrough) is probabilistically live, not optimal for deterministic-finality, low-latency settlement. Keep Cobalt for **UNL/governance only**; use Jolteon for **ordering**. This matches the user's stated framing.
- **Not removing certificate evidence**. The 4-of-5 ML-DSA detached signatures *are* the deterministic finality proof. Keep them.
- **Not probabilistic finality**. Two-chain commit is deterministic the moment a `commit-QC` extends a `prepare-QC` on the same proposal.
- **Not unbounded queues anywhere.** Every channel, RPC handler pool, mempool, and network buffer has an explicit cap and a documented backpressure policy.

### 1.4 Why the runtime-first ordering is correct

Your own measurement supports this: when you disabled full block-log verification on the proposal hot path (`verify_block_log = false`), `cargo check` passed but the benchmark still showed bad linear growth. That falsifies the hypothesis that *proposal replay* is the dominant cost, and it leaves the **finality RPC's O(height) walk** and the **process-spawn + serial-fan-out cost per round** as the dominant remaining suspects. Fixing those does not require changing the protocol's commit rule.

## 2. Latency Critical Path

## 2. Latency Critical Path

### 2.1 Decomposition of submit-to-finality

The submit-to-finality interval decomposes as:

```
T_finality = T_admit + T_propose + T_disseminate + T_vote + T_commit + T_persist + T_index + T_rpc
```

Each term has an irreducible (network-and-CPU) lower bound and an implementation-overhead component. In the current `Vc` system the **overhead component dwarfs the lower bound** by 1–2 orders of magnitude. The table below shows current measured, irreducible lower bound, and recommended budget.

| Stage | Description | Current observed | Irreducible (local 5-val) | Target budget (local) | Target budget (multi-machine LAN) |
|---|---|---|---|---|---|
| `T_admit` | wallet sign + RPC submit | ~155 ms (sign 88, submit 66) | ~150 ms (ML-DSA-65 sign 50–80 µs + JSON + 1 RTT) | ≤ 150 ms (unchanged) | ≤ 150 ms |
| `T_propose` | leader collects mempool, builds batch, broadcasts proposal | ~50 ms (batch of 1) | ~5 ms (1 ML-DSA sign + hash + 1 push) | ≤ 30 ms | ≤ 50 ms |
| `T_disseminate` | proposal reaches all 4 followers | ~1–2 s (process-spawn dominated) | 1 RTT loopback (≈ 0.3 ms) | ≤ 5 ms | ≤ 5 ms WAN-equiv |
| `T_vote` (prepare) | followers verify proposal + sign vote + return | ~7–8 s (serial collection) | 4 × (verify 100 µs + sign 60 µs + 1 RTT) ≈ 1–2 ms | ≤ 50 ms | ≤ 200 ms |
| `T_commit` (commit-QC) | leader collects 4-of-5 votes, builds QC, broadcasts | ~3–5 s | 1 RTT + 4 × verify ≈ 0.5 ms | ≤ 50 ms | ≤ 200 ms |
| `T_persist` | append-only commit to RocksDB (WAL fsync) | ~unknown, lumped into commit | 1 fsync ≈ 0.5–5 ms on NVMe | ≤ 10 ms | ≤ 15 ms |
| `T_index` | tx receipt index update | ~unknown, lumped | ~0.1 ms per tx | ≤ 5 ms | ≤ 5 ms |
| `T_rpc` | `tx_finality` RPC return | ~1.1 s p50 / 2.0 s p95 (O(height)) | point lookup ≈ 0.1 ms + 1 RTT | ≤ 20 ms | ≤ 30 ms |
| **Total** | | **~10.7 s p50 / 18.7 s p99** | **~170 ms** | **≤ 320 ms** (incl. admission) | **≤ 660 ms** (incl. admission) |

ML-DSA-65 signature parameters used (FIPS 204): public key 1,952 B, signature 3,293 B, verification ≈ 50–150 µs on modern x86 with AVX2-optimized implementations (Zheng et al. report 36–46% verify speedup with AVX-512; reference C with AVX2 lands in the 80–150 µs range on Skylake/Ice Lake; Cloudflare CIRCL Go benchmarks show ML-DSA-87 verify ≈ 75 µs/op on Core Ultra 7 155H, with ML-DSA-65 modestly faster).

### 2.2 Theoretical lower bounds (deterministic-finality, 4-of-5, partially synchronous)

For any deterministic-finality, partially-synchronous BFT in the HotStuff/Tendermint family, the *good-case* commit latency is at least:

- **Tendermint / PBFT-style 2-phase non-responsive**: ~ 3 δ (propose + prevote + precommit) — but waits for max delay Δ.
- **HotStuff 3-chain (responsive)**: 7 δ.
- **Jolteon / DiemBFT v4 / HotStuff-2 (responsive 2-chain)**: 5 δ (Gelashvili et al. 2021 "Jolteon and Ditto").
- **PBFT 2-phase responsive**: 4 δ (but quadratic communication).
- **Mysticeti-C uncertified DAG**: ~3 δ implicit-commit (Babel et al. 2024) — but requires uncertified DAG semantics.

Here δ is the *actual* one-way network delay. For N=5 on a single host (loopback), δ ≈ 50–150 µs; on a LAN δ ≈ 100–500 µs; on a WAN (single continent) δ ≈ 10–50 ms; intercontinental δ ≈ 80–200 ms.

The implied per-decision **consensus floor**:

- **Loopback 5-val Jolteon**: 5 δ ≈ 1 ms.
- **LAN 5-val Jolteon**: 5 δ ≈ 1–10 ms.
- **WAN 5-val Jolteon (1 continent)**: 5 δ ≈ 50–250 ms.
- **WAN 5-val Jolteon (global)**: 5 δ ≈ 400–1000 ms.

Adding admission, persistence, and the index-backed RPC, the realistic floors are roughly:

- **Local 5-val transparent tx finality**: ~200 ms p50, ~300 ms p95. Targets of < 2s p50 / < 4s p95 are comfortably above the floor — there is ~5–10× headroom for batching, GC, indexer churn, signature verification scheduling jitter.
- **Multi-machine 5-val (same DC LAN)**: ~250 ms p50, ~500 ms p95. Targets of < 5s p50 / < 10s p95 are very loose.
- **Multi-machine 5-val (single-continent WAN)**: ~700 ms p50, ~1.5 s p95. Targets are still loose.
- **Multi-machine 5-val (global WAN)**: ~1.2–1.8 s p50, ~3–4 s p95. This is where the targets become meaningful.

### 2.3 Critical path under the current (Vc) implementation

The dominant terms today are (highest first):

1. **`T_disseminate + T_vote + T_commit` ≈ 8–15 s.** Reason: validator service is launched per round, so its cost includes `process::spawn`, executable load, TLS/transport bind, `run_once` initialization, `--max-connections 2` connection ceiling, plus serial `for target in targets { transport_block_vote_request_with_retries() }`. That serial loop, with a 5 s RPC timeout and 1 retry, is structurally O(N) with very large constants. Adding height-bound replay inside the worker amplifies this. **This is your linear-in-height phenomenon**: each round adds replay work that scales with the block log.

2. **`T_rpc` ≈ 1.1–2.0 s and growing.** `tx_finality` calls `verify_blocks`, which is documented in the brief as walking blocks/ordered_batches/batch_archive/receipts/governance/history checkpoint/validator registry/replay base registry, *plus* replaying governance and registry state across heights, *plus* verifying certificate evidence, *plus* recomputing block hash, *plus* verifying archived payload, *plus* replaying state, *plus* scanning every block and every receipt_id. This is the textbook O(n) hot-path query antipattern.

3. **`T_persist` is hidden inside the commit**, but `write_ordered_commit` rewriting *aggregate JSON-like* logs on every commit is O(commit_log_size) per write — i.e. another linear-in-height term, *before* the finality RPC even runs.

The combination of 1 + 3 explains the linear growth: each new height re-reads and re-writes a growing file, *and* each finality RPC re-walks a growing log. The benchmark plot grows linearly because *both* the commit and the query cost grow linearly with height.

### 2.4 Critical path after recommended rework

Under the recommended architecture, the critical path on the happy path becomes:

```
[admit 150ms] → [propose 1 RTT + 1 sign ≈ 1ms]
              → [prepare-vote round: parallel verify+sign+collect 4 votes, 1 RTT + ~0.5ms CPU]
              → [commit-vote round: 1 RTT + ~0.5ms CPU]
              → [persist: batched WAL fsync ~1–5ms]
              → [index: 1 column-family put ~0.1ms]
              → [tx_finality RPC: 1 point lookup + 1 RTT ~1ms]
```

Local 5-val total: **~160–200 ms** including admission; **~10–50 ms** excluding admission (i.e. block-close latency).

This holds for any block height because nothing on the path scales with `n_blocks`.

### 2.5 Where height-linearity can leak back in (and how to keep it out)

| Risk | Mitigation |
|---|---|
| Mempool growing unbounded → admission slows | Bounded mempool with priority eviction; backpressure at RPC ingress |
| RocksDB read amplification on `tx_receipt_index` as it grows | Bloom filters per column family; periodic compaction; benchmark with `db_bench` |
| Validator registry replay on every commit | Materialize registry state once per epoch in `validator_registry_epoch_state` CF; only delta-replay within an epoch |
| `block_log_verifier` background task falls behind | Backpressure proposer if `verified_height < tip - max_lag` *only* for archival/audit nodes, not for liveness on validators |
| Certificate store grows unbounded | Retention policy: keep last K epochs of full certs, prune older with checkpoint co-signed cert |

## 3. Protocol Design

## 3. Protocol Design

This section answers research questions 1–7 jointly.

### 3.1 Minimum-round protocol safely runnable for canonical-UNL settlement (RQ1)

For a canonical validator-set, deterministic-finality, partially-synchronous settlement chain with N=5 validators and quorum Q=4 (i.e. f=1 Byzantine tolerance), the **minimum safe round count** in the families that are production-ready is:

- **2 rounds (one network RTT each)** — Jolteon / DiemBFT v4 / HotStuff-2 / Fast-HotStuff (5δ commit). This is the recommended floor.
- **1 round + fast path** is achievable for owned-object/non-conflicting transactions only (Sui Fast Path, Zef/FastPay style), but it is not a general-purpose ordered-finality path. For PostFiat transparent transfers it is *not* sufficient on its own; it is at most a useful future fast-path *overlay* for non-conflicting transactions and not part of v1.

Sub-2-round protocols (e.g. Mysticeti-C's 3-message-round implicit commit, PBFT's 2-phase responsive) either require uncertified DAGs (incompatible with how you want to bind ML-DSA certificates to block headers) or quadratic communication (acceptable at N=5 but does not generalize past N≈30).

**Recommendation: Jolteon-style 2-chain commit.** Two consecutive QCs on a chain commit the earlier block. Pacemaker uses a `timeout-cert` view-change.

### 3.2 Choice of ordering path (RQ2)

| Option | Pros | Cons | Verdict |
|---|---|---|---|
| Current peer-certified (unchanged) | Already implemented; deterministic | Serial vote-fan-out and per-round process spawn are runtime issues *not* protocol issues; algorithmically 3-phase if it follows your description | **Keep the safety frame, evolve to 2-chain** |
| HotStuff (chained, 3-phase) | Linear comms; well-studied | 7δ commit, an extra round vs Jolteon | Skip |
| **Jolteon / DiemBFT v4 / HotStuff-2** | 5δ optimistically-responsive; deterministic; linear-comm happy path; well-deployed (Diem, Aptos, Flow) | View-change quadratic | **Recommended v1** |
| Tendermint | Simple, 2-phase, deployed widely | Not responsive (waits for Δ each step); locks aren't optimistic | Skip |
| XRP RPCA / native ledger-close | Familiar to the team via Cobalt | Not deterministic-finality in the BFT-paper sense; safety properties depend on UNL overlap; analyzed-as-unsafe under benign asymmetric UNLs (Amores-Sesar et al. 2020) | **Skip for ordering**, keep for governance |
| Mysticeti / Bullshark / Narwhal+Tusk | Excellent throughput and 0.5s WAN latency in Sui mainnet | DAG semantics, certificate aggregation expectations, ML-DSA incompatibility for compact certs, large rewrite | Skip in v1; revisit in v3 |
| Mir-BFT / ISS multi-leader | Throughput scaling at many leaders | Not a latency win at N=5; complexity not justified | Skip |

The right shape is **single-leader-per-view, view = height, 2-chain commit (Jolteon), explicit `Vote` and `TimeoutCert` messages signed by ML-DSA-65**. This preserves the existing semantic of a per-block certificate that you can store, audit, and replay.

### 3.3 Concrete protocol pseudocode (Jolteon adapted for ML-DSA + Cobalt governance)

```rust
// Types
struct Block {
    parent_hash: H256,         // hash of parent Block
    height: u64,               // view = height in Jolteon
    epoch: u64,                // governance epoch
    registry_root: H256,       // Merkle root of validator_registry at this epoch
    batch_root: H256,          // Merkle root of ordered batch of tx digests
    proposer: ValidatorId,     // deterministic from (view, epoch, registry_root)
    timestamp: u64,
    cert_digest: H256,         // commits to certificate of *parent* (chained)
}

struct Vote {
    block_hash: H256,
    height: u64,
    epoch: u64,
    voter: ValidatorId,
    phase: VotePhase,          // Prepare or Commit
    signature: MlDsa65Sig,     // detached, ~3293 bytes
}

struct Certificate {
    block_hash: H256,
    height: u64,
    phase: VotePhase,
    voter_bitmap: u8,          // N=5 → 1 byte
    sigs: Vec<MlDsa65Sig>,     // ordered to match bitmap; ≤ N entries
}

// Commit rule (2-chain Jolteon):
// A block B is *committed* when there is a Certificate C_prepare on B's child B'
// and a Certificate C_commit (i.e. prepare on B'' that extends B'). Equivalently:
// two consecutive QCs on the chain ending at B ⇒ B is final.

// Proposer (leader of view v):
async fn propose(v: u64, parent_cert: Certificate, mempool: &Mempool, kr: &Keyring) -> Block {
    let batch = mempool.take_batch(MAX_TXS_PER_BLOCK, BATCH_DEADLINE).await;
    let parent = parent_cert.block_hash;
    let block = Block {
        parent_hash: parent,
        height: v,
        epoch: current_epoch(),
        registry_root: registry_root_for_epoch(current_epoch()),
        batch_root: merkle_root(&batch.digests()),
        proposer: self_id(),
        timestamp: now_ms(),
        cert_digest: H::hash(&parent_cert.canonical_bytes()),
    };
    broadcast_proposal(block.clone(), batch, parent_cert).await;
    block
}

// Follower verify + vote:
async fn on_proposal(p: Proposal, hot: &mut HotState) -> Option<Vote> {
    // 1) Stateless checks (parallelizable):
    if !proposer_is_correct_for_view(p.block.height, p.block.epoch, p.block.registry_root) { return None; }
    if !verify_certificate_pq(&p.parent_cert, &hot.registry_for(p.block.epoch)) { return None; }
    if !merkle_consistent(&p.batch, p.block.batch_root) { return None; }
    // 2) Safety rules (Jolteon): only vote if height > last_voted_height and block extends lock.
    if p.block.height <= hot.last_voted_height { return None; }
    if !extends_lock(&p.block, &hot.lock) { return None; }
    // 3) Update local state, persist a SafetyRules WAL entry SYNCHRONOUSLY before signing.
    hot.last_voted_height = p.block.height;
    hot.lock = update_lock(hot.lock, &p.parent_cert);
    persist_safety_state(&hot).await;  // <- this is the *only* fsync on the hot path
    // 4) Sign and return.
    Some(sign_vote(&p.block, VotePhase::Prepare, &hot.keys))
}
```

The `persist_safety_state` is the only mandatory synchronous fsync on the vote path; it must contain the highest-voted view and the current lock. Recovery on restart reads it before re-binding to the network. This is the SafetyRules pattern from DiemBFT, and it is what keeps you safe across crashes.

### 3.4 Cobalt-derived governance layered above ordering (RQ3)

Cobalt (MacBrough 2018) is an *atomic-broadcast / UNL-evolution* protocol designed to permit non-uniform-trust validator sets to safely change membership without forks under > 60% overlap. Its strengths are governance correctness; its commit latency for ordinary transactions is not competitive with Jolteon.

**Layering:**

- **Layer L0 (Cobalt-derived governance)** runs at *epoch boundaries*. It produces an authenticated `(epoch_id, validator_registry_root, ratification_cert)` tuple. This is the canonical UNL/registry change-control mechanism. Epochs may be of fixed length (e.g. every 4096 blocks) or triggered by an explicit `GovernanceProposal` reaching its quorum.

- **Layer L1 (Jolteon ordering)** runs at every block. Every `Block` header carries `(epoch, registry_root)`. Every `Certificate` is verified against the `validator_registry` materialized for that epoch. The proposer for view v is `proposer = registry_at_epoch.deterministic_leader(v)`.

- **Interface contract** (the only state the two layers share):
  ```rust
  trait GovernanceEpochAuthority {
      fn current_epoch(&self) -> EpochId;
      fn registry_root(&self, epoch: EpochId) -> H256;
      fn validator_set(&self, epoch: EpochId) -> Arc<ValidatorSet>;
      // Returns Some(commit) iff governance has ratified a new epoch.
      fn try_advance_epoch(&self, at_height: u64) -> Option<EpochCommit>;
  }
  ```

- **Binding**: an `EpochCommit` is produced once by L0 and embedded into the *first* Block of the new epoch as `epoch_open: Option<EpochCommit>`. Followers verify the EpochCommit against the previous epoch's `validator_set` (which they have materialized) before processing any further blocks in the new epoch.

- **Validator registry root in the certificate**: the certificate evidence ties to `(block_hash, epoch, registry_root)`. A re-verifier walks: `(certificate) → load registry_root from block.epoch → load validator_set for that epoch → verify each signature in bitmap → verify quorum met`. This is the *only* thing an audit node needs in addition to the block log.

### 3.5 Optimistic, concurrent vote collection while preserving deterministic certificates (RQ4)

**The problem to solve.** Today vote collection is serial in `Vc`:

```rust
for target in &targets {
    transport_block_vote_request_with_retries(target, ...)?;
}
```

This is the central runtime defect. Each call blocks until the previous returns. At N=4 followers and 5 s RPC timeout per call this is sufficient to explain *all* of the >1 s vote-collection latency by itself.

**The fix.** Vote collection is *naturally* optimistic and concurrent — it just needs to be expressed that way:

```rust
async fn collect_quorum(
    block_hash: H256,
    phase: VotePhase,
    self_vote: Vote,
    peers: &PeerSet,
    deadline: Instant,
) -> Result<Certificate> {
    let mut votes: Vec<Vote> = vec![self_vote];
    let bitmap = AtomicU8::new(1 << self_id());
    let (tx, mut rx) = mpsc::channel::<Vote>(peers.len() * 2);
    // Fan-out vote requests in parallel.
    let mut js = JoinSet::new();
    for p in peers.iter() {
        let tx = tx.clone();
        let req = VoteRequest { block_hash, phase };
        js.spawn(async move {
            if let Ok(v) = p.request_vote(req).await {
                let _ = tx.send(v).await;
            }
        });
    }
    drop(tx);
    while let Some(v) = tokio::time::timeout_at(deadline, rx.recv()).await? {
        // Verify off the runtime; rayon pool.
        let v = match cpu::verify_vote(v).await { Ok(v) => v, Err(_) => continue };
        if !votes.iter().any(|x| x.voter == v.voter) {
            bitmap.fetch_or(1 << v.voter.0, Ordering::SeqCst);
            votes.push(v);
            if popcount(bitmap.load(Ordering::SeqCst)) >= QUORUM {
                // We have enough — return now. Late votes still recorded (see below).
                return Ok(build_cert(block_hash, phase, &votes, bitmap.load(Ordering::SeqCst)));
            }
        }
    }
    Err(NotEnoughVotes)
}
```

**Determinism of the artifact.** The certificate is deterministic in two senses that matter:

1. **Verifiability**: any node can verify it given the block, the bitmap, the registry root, and the N detached signatures. There is no nondeterminism in verification.
2. **Canonical bytes**: define the canonical serialization as `bitmap || sig_for_voter_0 || sig_for_voter_1 || …` in registry order, *omitting* missing voters. This makes the certificate hash stable: any honest validator that reached the same quorum produces the same bytes. Late-arriving votes do not change the canonical cert — they go to a separate "late vote log" (§3.6).

This pattern is what HotStuff/DiemBFT/Bullshark all do operationally; the difference here is that there is **no aggregation** (no BLS, no threshold), so the cert is N detached signatures plus a bitmap.

### 3.6 Quorum-then-return vs wait-for-all (RQ5)

**Recommendation: return as soon as `quorum_met()`. Record late votes in a separate `late_votes` column family.** This is correct because:

- **Safety** is unaffected. The 2-chain commit rule says: as soon as 4-of-5 prepare-votes are committed on the chain in a way that is extended by another QC, the block is final. Voter 5's vote, arriving 200 ms later, does not change finality.
- **Liveness** is improved. Waiting for the slowest peer makes you straggler-bound.
- **Auditability** is preserved by recording late votes in `late_votes` column family. An audit replay can reconstruct *which* votes the canonical certificate used (those committed at quorum time), and *which* arrived later but still authenticate the block. This is the standard pattern in DiemBFT (LedgerInfoWithSignatures includes only quorum-time signatures; aux endpoints expose the rest).
- **Auditor-visible invariant**: `voter_bitmap_popcount(cert) ≥ Q` and `for every bit b in voter_bitmap, sig_b verifies`. Late votes augment the *liveness/participation* record but do not affect the *finality* record.

Caveat: do *not* allow proposers to selectively exclude honest voters to favor a faction. To prevent this, enforce a **proposer fairness rule**: the certificate that the leader publishes in the *next* block must include all votes received before that next block's proposal time, not only those received before quorum was reached. This is cheap (a single column-family lookup) and gives auditors a way to detect proposer censorship.

### 3.7 Certificate representation for ML-DSA (RQ6)

The space of options:

| Option | Cert size (N=5) | On-chain header overhead | Hot-path verify cost | Notes |
|---|---|---|---|---|
| **Full sigs in every block header** | 5 × 3,293 = 16.5 KB header | 16.5 KB per block | 5 × ~100 µs = 0.5 ms | High disk/bandwidth at scale; OK at N=5 but bad as N grows |
| Registry-root-bound compact votes (sig of hash chain) | same | same | same | No win without aggregation |
| **Separate certificate store + header commitment** (recommended) | 16.5 KB stored once in `certificates` CF | 32 B in header (cert_digest) | 5 × ~100 µs on commit + 0 on subsequent verifies | Clean separation; replayable; minimal header bloat |
| Merkle commitment to vote set | 16.5 KB stored + 32 B header | 32 B | same | Same as above with a Merkle structure; useful if N grows |
| Bitmap + detached signatures (in header) | 16.5 KB header | 16.5 KB | 5 × 100 µs | Same as option 1 |
| BLS aggregation | ~96 B | ~96 B | ~1 ms | **NOT AVAILABLE** for ML-DSA; do not use |
| LaBRADOR aggregation of Falcon | ~74 KB for many sigs | ~74 KB once | tens of ms | Falcon only; not ML-DSA; ZKP-based; pre-production |
| Chipmunk / Squirrel synchronized multisig | ~136 KB at 8192 sigs | impractical at N=5 | synchronized state required | Bounded-state, synchronized-only, lattice; not ML-DSA |
| Threshold ML-DSA (Kao 2026, Shamir nonce DKG) | 3,309 B (FIPS-204-compatible) | 3,309 B | ~standard ML-DSA verify | **Promising**, T ≤ 17 with conditional min-entropy; pre-production; requires interactive DKG, coordinator-based profile; revisit v2 |

**Recommended v1 design: separate `certificates` column family + `cert_digest` in header.**

```rust
// In RocksDB:
//   cf="blocks"       : block_hash → Block          (header only)
//   cf="certificates" : cert_digest → Certificate   (bitmap + N sigs)
//   cf="block_by_height": height u64 → block_hash
//   cf="height_by_certdigest": cert_digest → height  (for audit)

// Block header carries cert_digest. The certificate is fetched by digest.
// On the hot path, the validator verifies the parent_cert *once* on receipt of
// the proposal and *once* during commit; subsequent finality queries do not
// re-verify the cert — they trust the indexer's verified-watermark.
```

This gives you:
- 32 B per block header for the cert commitment.
- ~16.5 KB stored once per block in a dedicated CF that does not pressure block reads.
- A clean migration path: when threshold-ML-DSA matures (or you adopt Falcon+LaBRADOR for aggregation), only the `Certificate` schema changes; the header stays identical.

**Recommendation on aggregation (RQ7).** Treat ML-DSA as **non-aggregatable for v1 and v2**. The literature picture (mid-2026):

- **LaBRADOR-aggregated Falcon** (Aardal et al. CRYPTO 2024; Boneh+ ePrint 2024) is the most mature candidate but is *Falcon* — not ML-DSA — and the aggregate proof is a non-interactive argument of knowledge with multi-second prove times for large batches. Not on the hot path.
- **Chipmunk** (Fleischhacker et al. CCS 2023) and **Squirrel** (CCS 2022) are *synchronized* multisignatures with a-priori-bounded epoch state. They are not drop-in replacements for ML-DSA in a per-block consensus loop.
- **Hash-based multi-signatures for post-quantum Ethereum** (Drake/Khovratovich/Kudinov/Wagner 2025) replace BLS for proof-of-stake aggregation, but require a separate (hash-based) signing scheme — not ML-DSA — and a synchronized epoch.
- **Threshold ML-DSA via Shamir nonce DKG** (Kao 2026, arxiv 2601.20917) produces FIPS-204-compatible 3.3 KB signatures, but is currently coordinator-based, requires interactive DKG per signing, and is limited to T ≤ 17 with conditional min-entropy guarantees.

For PostFiat v1, the right answer is **N detached ML-DSA-65 signatures, batch-verified on a rayon pool, with the cert stored once and committed to in the header by 32-byte digest**. Document a migration path that adds threshold ML-DSA *underneath* the same `Certificate` schema when it is production-grade.

## 4. Rust Runtime Design

## 4. Rust Runtime Design

This section answers RQ8–RQ10.

### 4.1 Process model

**One long-running validator process per validator**, not one process per round. This is the single highest-leverage change in the report. The current `node-run-peer-certified` pattern launches non-proposer validator services per measured round, which is what produces both the per-round process-spawn cost *and* the cold-cache cost (no hot in-memory consensus state, no persistent peer connections, no warm RocksDB block cache). Move to a daemon model:

```
postfiat-validator
  --config /etc/postfiat/validator.yaml
  --data-dir /var/lib/postfiat
```

The daemon starts once, binds its QUIC endpoint, dials its peers, opens persistent streams, and runs forever. Round-by-round consensus state lives in memory; persistence is incremental and append-only.

The benchmark harness should switch from "spawn a fresh validator per round" to "start 5 daemons once, then drive them with submit-tx RPCs and measure end-to-end finality". This alone should remove most of the linear-in-height growth in the benchmark.

### 4.2 Async runtime — tokio with one dedicated multi-thread runtime per validator

- **One `tokio::runtime::Builder::new_multi_thread()`** sized to physical cores − 2 (reserving cores for the rayon CPU pool and the OS).
- Use `tokio::task::LocalSet` for actor-style components only if you need `!Send` state (rare; avoid if possible).
- Use `tokio::time::Instant`/`sleep` for all timing; never wall-clock for consensus-relevant decisions.

### 4.3 Networking — QUIC via `quinn` or `tokio-quiche`

**Choose QUIC over TCP** for validator-to-validator transport, for these reasons:

- **0-RTT / 1-RTT handshake** after first contact (vs TCP's 1-RTT TCP handshake + 1-RTT TLS handshake when reconnecting).
- **Stream multiplexing without head-of-line blocking** — a stalled batch upload does not block a vote message on the same connection.
- **Unreliable datagrams** (QUIC datagrams) available for gossip/heartbeat without HoL blocking.
- **Connection migration**, which is useful for validators that change networks (multi-homed, IPv6 transitions).
- **Native rustls integration** with mutual TLS via self-signed validator certificates anchored in the validator registry.

**Library choice:** `quinn` (pure-Rust, tokio-native, mature, ~30 releases since 2018) is the default. `tokio-quiche` (Cloudflare, open-sourced 2025) is the choice if you anticipate going to millions-of-RPS scale or want HTTP/3 client-facing RPC.

**Connection topology:** Full mesh, persistent. With N=5 there are 10 directed connections; trivial. One connection per peer pair, multiplexed into named streams:
- `proposal_stream` (unidirectional, leader→follower)
- `vote_stream` (unidirectional, follower→leader; can be one stream per direction per peer)
- `cert_stream` (unidirectional, leader→follower, fan-out of QC)
- `batch_stream` (bidirectional, for pull-based batch sync)
- `sync_stream` (bidirectional, for catch-up)

Identity is mutual-TLS on validator certs, with the cert subject keyed by `ValidatorId` from the registry. A connection is *only* accepted if the peer's cert subject is in `validator_registry_for_current_epoch`.

### 4.4 Actor model and bounded queues

Use an actor pattern for the long-lived components. The recommended actor decomposition:

| Actor | Owns | Inbox capacity | Notes |
|---|---|---|---|
| `ConsensusActor` | hot state (lock, last_voted, tip, pending QCs, view state) | 256 | Single-threaded; processes events serially for determinism |
| `MempoolActor` | mempool + admission queue | 1024 (or bytes-bounded) | Bounded; rejects when full (backpressure to RPC) |
| `NetworkActor` (one per peer) | peer QUIC conn + send/recv streams | 64 each direction | Drops + reconnects on send-queue overflow |
| `StorageActor` | RocksDB handle, write batch coalescing | 64 commit batches | fsync rate-limited; group-commit pattern |
| `IndexerActor` | tx_receipt_index, certificate_index, block_by_height | 256 commit events | Eventually-consistent w.r.t. block_log; reports verified-watermark |
| `RpcActor` | tower stack: tx_submit, tx_finality, tx_status | 512 concurrent + rate-limited | Bounded via `tower::limit::ConcurrencyLimit` |
| `RegistryActor` | epoch state, registry materialization | 32 epoch advance events | Reads governance L0 → materializes validator_set for L1 |

Use `tokio::sync::mpsc::channel(cap)`, **never** `unbounded_channel`. Use `try_send` at producer sites that must not block, and define explicit drop/reject behavior. For request/reply, use `oneshot::channel`.

**Backpressure model:**

- RPC ingress → bounded admission queue → if full, return `503`/`SubmitDeferred` to client.
- MempoolActor → if full, drop *lowest-priority* admitted txs (priority = fee, then submission time).
- ConsensusActor inbox saturated → log a `consensus_inbox_full` metric; this should *never* happen on a healthy validator; alert.
- Peer NetworkActor outbox saturated → drop oldest non-critical (heartbeat, late vote rebroadcast) before critical (proposal, QC).

### 4.5 CPU-bound work: ML-DSA verification scheduling

ML-DSA-65 verifies are ~50–150 µs each on a modern x86 core. The hot path for a 5-validator network has at most ~5 verifies per consensus round (1 per peer vote). That is ~0.5–1 ms of CPU per round — easily fits in a single core. The danger is **lock-step batching across many concurrent rounds during catch-up** (e.g. when re-verifying 1000 blocks of certificates, that's 5000 verifies, 0.5 s of CPU if serialized).

**Pattern:**

```rust
// In Cargo.toml
// rayon = "1.10"
// pqcrypto-mldsa = "0.x"  // or your chosen ML-DSA crate

use rayon::prelude::*;

fn verify_certificate(cert: &Certificate, vset: &ValidatorSet, block_hash: H256)
    -> Result<()>
{
    // Verify all sigs in parallel; short-circuit on first failure via .try_for_each.
    cert.sigs
        .par_iter()
        .zip(cert.voter_indices().par_bridge())
        .try_for_each(|(sig, idx)| {
            let pk = vset.public_key(idx)?;
            ml_dsa_65::verify(&block_hash, sig, &pk)
        })?;
    if cert.voter_bitmap.count_ones() < QUORUM { return Err(NotEnoughVotes); }
    Ok(())
}

// On the async side:
async fn verify_certificate_async(cert: Certificate, vset: Arc<ValidatorSet>, block_hash: H256) -> Result<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    // tokio_rayon or hand-rolled rayon::spawn that signals tokio:
    rayon::spawn(move || { let _ = tx.send(verify_certificate(&cert, &vset, block_hash)); });
    rx.await.unwrap_or_else(|_| Err(Canceled))
}
```

**Do not** use `tokio::task::spawn_blocking` for ML-DSA verification: tokio's blocking pool is sized for I/O (default 512 threads), not for CPU-bound work, and saturating it stalls everything else that uses `spawn_blocking` (DNS, file I/O). Use a dedicated `rayon` global pool sized to `min(physical_cores − 2, 8)`. The `tokio-rayon` crate provides the bridge.

**Batch-verification opportunity:** Pure Dilithium does not have a batched verify primitive in the same way that Ed25519 does, but verifying N independent signatures on N rayon workers is essentially a 1:1 scale-up to ~`cores` simultaneous verifies — i.e. on a 16-core box, ~16 verifies in ~150 µs ≈ 100,000 verifies/s peak. That is two orders of magnitude above what consensus needs at N=5 even under burst.

### 4.6 What lives in memory on the hot path (RQ9)

| In RAM | Why | Size at steady state (N=5, 1 epoch ≈ 4096 blocks) |
|---|---|---|
| Current `ledger_tip` (block hash, height, view, epoch, registry_root) | Every proposal / vote checks this | 256 B |
| Current `validator_registry_for_epoch` | Verify all peer sigs | 5 × ML-DSA-65 pk = 5 × 1952 B ≈ 10 KB |
| Pending `parent_cert` and committed `commit_cert` | 2-chain commit rule | ~33 KB (2 certs) |
| `lock` and `last_voted_height` (SafetyRules state) | Safety, persisted on every change | 64 B |
| `Mempool` (admitted, signature-checked txs) | Building next batch | bounded (say 16 MB) |
| `BlockCache` (recent ~64 blocks) | Optimistic execution, fast catch-up of stragglers | ~64 × (header + batch) |
| `RecentCerts` (last 256 certs) | Peer catch-up | 256 × 16.5 KB ≈ 4 MB |
| `Peers` (4 active QUIC connections + stream handles) | Network | tiny |

**What is NOT in RAM:** historical blocks, historical receipts, historical certificates, full state trie. These are on disk, page-cached.

### 4.7 What must be synchronously persisted before voting (RQ10)

The synchronous (fsync) hot path is *only* the SafetyRules WAL: the file containing `{last_voted_view, lock_view, lock_block_hash, epoch}`. This is at most ~128 B. On NVMe, an fsync is ~50–500 µs.

```
on_proposal(p):
  verify(p)                          // CPU, in-memory
  update_safety_state_in_memory(p)
  persist_safety_state_fsync()       // <-- the one mandatory fsync, ~0.1ms
  sign_vote_and_send(p)
```

Block commit fsync is **deferred and batched**: when a `commit-QC` lands, the writes (block, certificate, batch, receipts, tx-receipt-index entries) are written into a single RocksDB `WriteBatch` and committed with `WriteOptions::set_sync(true)` *once*. This is one fsync per block, not per write — RocksDB column families share a WAL.

**Deferrable (async indexer):** full state-trie writes, range-by-address indexes, archival snapshots, late-vote logs. These run in `IndexerActor`, which advances `verified_through_height` after each commit. The finality RPC may return immediately based on the indexer's verified-watermark; if the requested height is past the watermark, the RPC either waits up to `MAX_FINALITY_WAIT_MS` (= 250 ms default) or returns `Status::Pending`.

### 4.8 Crash recovery

On startup:
1. Open RocksDB; read `consensus_state` CF for last-known `(tip_height, lock, last_voted_view)`.
2. Read SafetyRules WAL; reconcile with `consensus_state` (WAL wins if newer; this is the only Authority on safety).
3. Replay any uncommitted-but-persisted blocks forward by re-verifying their certs (this is cheap: only the tip-relative recent blocks need replay).
4. Reconnect to peers; pull `tip_height` from each; if local is behind, enter catch-up mode.
5. Exit catch-up when within `MAX_CATCH_UP_LAG` of peer tips; resume normal voting.

### 4.9 Nondeterminism — what to forbid (RQ8)

| Source | Forbidden because | What to do instead |
|---|---|---|
| `HashMap` iteration order | Different replicas may produce different batch orderings | Use `BTreeMap` for any iteration whose order is consensus-visible |
| `std::time::SystemTime::now()` | Skewed across hosts | Block timestamp is the leader's `Instant`-derived monotonic time, validated by followers within `MAX_CLOCK_SKEW` (recommend ±2 s); reject otherwise |
| Floating-point arithmetic | Non-deterministic across platforms | Forbid in consensus-visible code; use fixed-point if needed |
| `rand::thread_rng()` | Different on each replica | Use a deterministic RNG seeded from `(block_hash, epoch)` if needed for any leader election, etc. |
| Async race conditions in vote ordering | The *order* in which votes arrive at the leader is not consensus-visible | This is fine *as long as* the cert's canonical serialization is registry-order, not arrival-order |
| Tokio task scheduling | Could change message order | Only consensus-visible state changes happen in the single-actor ConsensusActor inbox, which is FIFO by definition |
| Filesystem `readdir` order | Non-deterministic | Sort explicitly anywhere this matters |

## 5. Storage and Index Design

## 5. Storage and Index Design

This section answers RQ9–RQ12.

### 5.1 Choice of engine: RocksDB

**RocksDB**, used through the `rocksdb` crate, is the recommended primary store, for these reasons:

- LSM design matches an append-heavy block log naturally; write amplification can be tuned via leveled vs universal compaction.
- **Column families** allow per-CF tuning (block cache, bloom filters, compaction style), enabling block_log to use different parameters than tx_receipt_index.
- A **shared WAL across all CFs** means a single fsync atomically commits a block + receipts + index in one operation. This is critical for the "block commit is one fsync" property.
- Production-proven in blockchains (Solana, Ethereum geth ancients, Aptos, Sui, Cosmos validators).
- Bloom filters per CF cut point-lookup latency on `tx_receipt_index` to a single SST seek; this is what makes `tx_finality` ~O(1) amortized.

**Why not sled?** sled is a single-tree B-Tree with no column-family equivalent, smaller deployment surface, and a less mature crash-recovery / backup tooling story. It is acceptable for the *mempool* (volatile, smaller, lots of small writes) but is not recommended for the canonical block log in 2026.

**Why not LMDB?** B-tree backing makes large-value writes (16.5 KB certs, full batches) more expensive; mmap-based crash semantics are subtler under sudden power loss; lacks column families with independent tuning.

### 5.2 Column-family layout

```
cf="default"               -- small misc keys (version, schema migrations, watermarks)
cf="blocks"                -- key: block_hash[32B]    val: BlockHeader (≤300B)
cf="block_by_height"       -- key: height u64 BE      val: block_hash[32B]
cf="batches"               -- key: block_hash[32B]    val: OrderedBatch (tx digests + payloads or refs)
cf="certificates"          -- key: cert_digest[32B]   val: Certificate (bitmap + sigs, ≤17KB at N=5)
cf="tx_receipt_index"      -- key: tx_id[32B]         val: ReceiptLocator{height,block_hash,batch_idx,intra_idx,cert_digest,merkle_path}
cf="receipts"              -- key: (height u64 BE, intra_idx u32 BE)  val: Receipt
cf="state_trie"            -- key: trie_node_hash[32B]  val: trie node
cf="state_versions"        -- key: (account[20B], height u64 BE)  val: account_state_root
cf="validator_registry"    -- key: (epoch u64 BE, validator_id)  val: ValidatorEntry (pubkey, stake, metadata)
cf="epoch_state"           -- key: epoch u64 BE       val: EpochCommit (registry_root, ratification_cert)
cf="late_votes"            -- key: (block_hash, voter_id)   val: Vote
cf="safety_wal"            -- key: "current"          val: SafetyState (last_voted_view, lock_view, lock_hash)
cf="snapshots"             -- key: snapshot_id        val: SnapshotManifest
cf="mempool" (optional)    -- key: tx_id              val: SignedTx  -- consider separate sled
```

`block_by_height` exists so that a `get_block_by_height(h)` is one CF-key lookup followed by one `blocks` CF-key lookup — two seeks, both with bloom filters, both ~10 µs on warm cache.

`tx_receipt_index` is the central enabler of bounded finality RPC. Its key is the transaction id (32-byte hash); its value is a small struct (~200 B) carrying everything the RPC needs to return a proof without any block-log walk.

### 5.3 Append-only commit (replacing the current `write_ordered_commit`)

The current pathology is that `write_ordered_commit` rewrites aggregate JSON-like logs on every commit. Replace with a single atomic `WriteBatch`:

```rust
fn commit_block(db: &DB, b: &Block, batch: &OrderedBatch, cert: &Certificate, receipts: &[Receipt]) -> Result<()> {
    let mut wb = WriteBatch::default();
    let block_hash = b.hash();
    let cert_digest = cert.digest();
    // blocks
    wb.put_cf(&cf_blocks, block_hash.as_bytes(), &serialize(b)?);
    wb.put_cf(&cf_block_by_height, &b.height.to_be_bytes(), block_hash.as_bytes());
    // batch
    wb.put_cf(&cf_batches, block_hash.as_bytes(), &serialize(batch)?);
    // cert
    wb.put_cf(&cf_certificates, cert_digest.as_bytes(), &serialize(cert)?);
    // receipts + index
    for (idx, r) in receipts.iter().enumerate() {
        let key = encode_height_idx(b.height, idx as u32);
        wb.put_cf(&cf_receipts, &key, &serialize(r)?);
        let locator = ReceiptLocator {
            height: b.height, block_hash, batch_idx: 0, intra_idx: idx as u32,
            cert_digest, merkle_path: merkle_proof(batch, idx),
        };
        wb.put_cf(&cf_tx_receipt_index, r.tx_id.as_bytes(), &serialize(&locator)?);
    }
    let mut wo = WriteOptions::default();
    wo.set_sync(true);     // single fsync for the whole batch
    db.write_opt(wb, &wo)?;
    Ok(())
}
```

This is **one fsync per block**, **constant size per tx**, **no aggregate-log rewriting**.

### 5.4 Finality RPC design (RQ11)

`tx_finality(tx_id)` becomes:

```rust
async fn tx_finality(&self, tx_id: TxId) -> RpcResult<FinalityProof> {
    // 1. Index lookup: bounded, O(1) amortized.
    let locator: ReceiptLocator = match db.get_cf(&cf_tx_receipt_index, tx_id.as_bytes())? {
        Some(b) => deserialize(&b)?,
        None => return Err(RpcError::NotFound),
    };
    // 2. Verified-watermark check.
    let watermark = self.indexer.verified_through_height.load(Ordering::Acquire);
    if locator.height > watermark {
        // Still verifying; either wait briefly or return Pending.
        match self.wait_for_watermark(locator.height, MAX_FINALITY_WAIT_MS).await {
            Ok(_) => {},
            Err(_) => return Ok(FinalityProof::Pending(locator.height)),
        }
    }
    // 3. Assemble proof from disk; no replay, no walk.
    let block: BlockHeader = deserialize(&db.get_cf(&cf_blocks, locator.block_hash.as_bytes())?.unwrap())?;
    let cert_digest = locator.cert_digest;  // 32B commitment to N detached sigs
    Ok(FinalityProof::Final {
        tx_id,
        height: locator.height,
        block_hash: locator.block_hash,
        block_header: block,
        merkle_path: locator.merkle_path,
        cert_digest,
    })
}

async fn tx_audit(&self, tx_id: TxId) -> RpcResult<FullEvidence> {
    // The slow, full-replay endpoint. NOT on the hot path.
    // Returns the certificate, signatures, validator-set, registry-root, governance ratification chain.
    ...
}
```

This is **O(log n) at most** (RocksDB seek with bloom filter + LSM levels) and **O(1) amortized** on a warm block cache. The RPC no longer walks history.

### 5.5 Verified watermark (replacing implicit `block_log_verified`)

A `BlockLogVerifierActor` runs in the background, advancing a monotonic `verified_through_height` watermark. Its loop:

```rust
loop {
    let next = self.watermark + 1;
    let block = db.get_block(next)?;
    let cert = db.get_certificate(block.cert_digest)?;
    verify_certificate(&cert, &validator_set_for(block.epoch), block.hash())?;
    verify_state_transition(&block, &batch_for(block))?;  // optional, depending on audit policy
    self.watermark.store(next, Ordering::Release);
}
```

The validator may vote and commit *without* waiting for this watermark on its own writes (it just wrote the block, so verification is trivially passed locally). The watermark exists for:

- Audit/full nodes that ingest the block log from elsewhere.
- Restart-and-catch-up consistency: after restart, the watermark resumes from disk.
- The finality RPC's "is this height verified" check.

### 5.6 Snapshot / checkpoint / retention (RQ12)

Borrow from Cosmos SDK (snapshots package) and Ethereum (state pruning) practices:

**Three retention tiers:**

| Tier | What's kept | Default retention | Audience |
|---|---|---|---|
| **Hot tier** | last 4096 blocks, all CFs, full state | always | active validators, recent-history queries |
| **Snapshot tier** | every 4096th height: state-trie root + snapshot manifest (chunked) | last 4 snapshots = ~16384 blocks | new validator state-sync joins |
| **Archive tier** | full block log + certs + receipts | unbounded (separate node role) | auditors, regulators |

**Snapshot mechanism:**

- Every `SNAPSHOT_INTERVAL` blocks (start with 4096), take a snapshot. Output is a manifest + a set of chunks. Each chunk is a deterministically-ordered slice of `state_trie` CF.
- Snapshots are produced by a separate `SnapshotActor` reading a consistent RocksDB snapshot (RocksDB native `Snapshot` handle), so they do not block commits.
- A snapshot is *finalized* (and ratified for state-sync use) only when ≥ Q validators co-sign its manifest with ML-DSA. The ratification certificate uses the same `Certificate` schema as a block cert.

**Pruning:** keep `pruning-keep-recent = 4096`, `pruning-keep-every = SNAPSHOT_INTERVAL`. Old blocks before the pruning cutoff *may* be pruned on non-archive nodes. Archive nodes opt out of pruning.

**Catch-up:**

- *Fast catch-up (state sync)*: a new validator downloads the most recent snapshot manifest, verifies its ratification cert against the registry, downloads chunks, materializes state, then begins normal consensus from `snapshot_height + 1`.
- *Replay catch-up (audit)*: a node downloads the block log from a peer and replays end-to-end. Bounded by network bandwidth, not by consensus.

### 5.7 Why mempool may want its own store

The mempool churns: writes for admissions, deletes for inclusions, frequent iteration by priority. Putting it in the consensus RocksDB pollutes the block cache and triggers compactions that compete with hot reads of `tx_receipt_index`.

**Option A (simpler):** keep mempool in-memory only with a periodic snapshot to disk for restart durability. Loss of mempool on crash is acceptable (clients resubmit).
**Option B:** use a separate `sled` instance with its own filesystem path. Simpler to reason about; bounded effect on RocksDB compactions.

Pick A for v1; revisit if mempool memory is a problem at multi-1000-tx loads.

## 6. Implementation Plan

## 6. Implementation Plan

This is sequenced to maximize latency reduction per engineering hour and to keep safety/auditability intact through every step.

### 6.1 First 24 hours — biggest immediate wins, no protocol change

These changes alone are projected to bring **p50 finality from ~10.7 s to ~2–3 s**. Each is independent.

**Hour 0–4: Parallel vote collection.**
- Replace the serial `for target in &targets { transport_block_vote_request_with_retries(...) }` with a `JoinSet`-driven concurrent fan-out, completing on quorum.
- Cap the concurrent fan-out at `N-1` (4); collect into a buffered channel of capacity `N-1`.
- Acceptance test: existing 5-validator benchmark shows ≥3× drop in `submit_to_certified` p50.

**Hour 4–8: Parallel certified-batch broadcast.**
- Same pattern: replace serial loop over peers with `JoinSet`-driven concurrent push of certified-batch.
- Acceptance test: same as above plus reduction in `submit_to_finality - submit_to_certified`.

**Hour 8–16: Long-running validator daemon.**
- Modify the benchmark harness to start 5 validator daemons once at the start of the run, then drive submissions, then collect.
- Remove the `--max-connections 2 / run_once` per-round invocation.
- Move per-round process state (peer connections, block cache, mempool, registry) into in-memory daemon state.
- Acceptance test: latency stops growing linearly with iteration index; height-21 round time should now be within a few percent of height-2 round time once daemons are warm.

**Hour 16–24: Bounded index-backed `tx_finality`.**
- Add a `tx_receipt_index` RocksDB column family. Populate it transactionally with every commit (single `WriteBatch`).
- Change `tx_finality` to: lookup in `tx_receipt_index` → fetch block header → return `(merkle_path, cert_digest)`.
- Keep the existing `verify_blocks`-style path as `tx_audit` (full replay endpoint).
- Acceptance test: `tx_finality` p95 drops from ~2 s to < 50 ms; latency does not grow with height.

**Expected end-of-day state:** local 5-validator transparent finality p50 ≈ 2–3 s, p95 ≈ 4–6 s. Linear-in-height growth eliminated.

### 6.2 First 72 hours — runtime structural rework

**Days 2–3:**

- **Replace `write_ordered_commit` aggregate JSON writes with `WriteBatch` over column families.** This is a focused but invasive change. Add a `commit_block(b, batch, cert, receipts)` function that does a single atomic `WriteBatch` with `set_sync(true)`. Remove all per-commit aggregate JSON file writes. Migration path for existing data: an importer that reads the old JSON logs once and bulk-loads into RocksDB CFs (run once during the v0→v1 testnet reset).
- **SafetyRules WAL.** Persist `{last_voted_view, lock_view, lock_block_hash, epoch}` synchronously before every vote. This is the only fsync on the vote-send path.
- **Bounded mpsc everywhere.** Audit the codebase for `unbounded_channel` and `mpsc::channel` with absurd capacities. Replace with sized channels and a documented drop policy at each producer.
- **Persistent QUIC connections.** Replace any per-round TCP dial with persistent `quinn` connections established at daemon start. Use mutual TLS with self-signed validator certs anchored in the registry. Reconnect with exponential backoff on drop.

**Acceptance test (end of day 3):** local 5-validator finality p50 ≈ 600–900 ms, p95 ≈ 1.5–2.5 s. Multi-machine LAN p50 ≈ 1.2–2.5 s, p95 ≈ 3–6 s. *This already meets the brief's targets if achieved.*

### 6.3 First 2 weeks — algorithmic and audit hardening

**Days 4–7: Move from 3-phase to 2-chain Jolteon commit rule.**

This is the "evolve the peer-certified path" change. It is *not* the biggest latency win, but it removes ~one network RTT plus removes the 3-chain commit's wait for an extra round. The migration is:

1. Introduce `Vote { phase: Prepare | Commit }` and `Certificate { phase }` distinction.
2. Implement the SafetyRules: `vote_only_if(view > last_voted_view && extends(lock))`.
3. Implement the commit rule: a block is committed when its child's QC is in a QC chain of length 2.
4. Implement the `Pacemaker` with `TimeoutCert` view-change.
5. Implement leader rotation: `leader(view) = validator_set[ (view + offset) % N ]` where `offset = H(epoch_id) mod N` (deterministic, registry-bound).

**Days 7–10: Indexer + verified-watermark.**

- Implement `BlockLogVerifierActor` advancing `verified_through_height`.
- Move the `verify_blocks(entire_log)` from synchronous-on-query into this background actor.
- Implement `tx_finality` watermark semantics: wait briefly for indexer to catch up; return `Pending` otherwise.

**Days 10–14: Snapshots, retention, governance epoch binding.**

- Implement `SnapshotActor` and snapshot manifest.
- Implement `EpochCommit` and validator-registry materialization per epoch.
- Implement Cobalt-derived governance interface (epoch advance, registry change ratification). For v1 this can be a simple "co-signed registry update" with quorum; full Cobalt UNL-evolution semantics can ship in v2.
- Implement `tx_audit` (the full evidence-and-replay endpoint).

### 6.4 Necessary but not on the critical path

These changes are required for production credibility but not for the latency targets. Plan them in parallel after the critical path is delivered.

- **Mempool admission rate limit + priority eviction** (RQ13: see §6.5 below).
- **Connection-level rate limiting and DoS protection** on RPC ingress (`tower::limit`, `tower::buffer`, `tower::load`).
- **Metrics**: `consensus_round_duration_seconds`, `vote_collection_duration_seconds`, `cert_verify_duration_seconds`, `commit_fsync_duration_seconds`, `tx_finality_duration_seconds`, `mempool_admit_total`, `mempool_drop_total`, `indexer_lag_blocks`.
- **Structured logging** with trace ids (`opentelemetry`/`tracing` crate). One span per consensus round; one event per vote.
- **Fuzz testing** of certificate parser, vote parser, proposal parser using `cargo fuzz`.
- **Determinism harness**: a single-binary multi-validator runner (à la Mysticeti's simulator) that runs N validators in one process with a deterministic scheduler.

### 6.5 Batch-close policy (RQ13)

Use an **adaptive, deadline-driven** policy:

```
A batch is closed and proposed when ANY of:
  • batch_size ≥ MAX_TXS_PER_BLOCK            (default 1024)
  • batch_bytes ≥ MAX_BLOCK_BYTES             (default 1 MiB)
  • time_since_last_batch_close ≥ BLOCK_INTERVAL  (default 250 ms, leader-local)
  • time_since_first_admitted_tx_in_batch ≥ MAX_BATCH_AGE  (default 100 ms)
```

Rationale:
- `BLOCK_INTERVAL=250ms` matches XRP-like ledger close (3–5s on XRPL is *the inclusion delay*, not the block interval; XRP closes every 3–5 s because that is also its propose interval). For PostFiat we want a tighter floor.
- `MAX_BATCH_AGE=100ms` keeps p50 latency tight under light load (a single tx admitted is sealed within ≤ 100 ms).
- Block interval and admission rate are decoupled (separate clocks): the mempool admits txs as fast as RPC allows; the proposer wakes up every `BLOCK_INTERVAL` to consider closing a batch.

Under heavy load this naturally closes blocks at `MAX_TXS_PER_BLOCK` or `MAX_BLOCK_BYTES`. Under light load it closes blocks at `BLOCK_INTERVAL`. Under intermediate load, `MAX_BATCH_AGE` keeps any single tx from waiting too long.

## 7. Risks and Non-Negotiables

## 7. Risks and Non-Negotiables

### 7.1 Safety risks

| Risk | Severity | Mitigation |
|---|---|---|
| Parallel vote collection produces nondeterministic certificate bytes | **Critical** | Canonical cert serialization is registry-order, not arrival-order; documented and unit-tested |
| Switching from 3-phase to 2-chain Jolteon introduces a window where two QCs could commit conflicting blocks | **Critical** | Enforce strict SafetyRules: `vote_only_if(view > last_voted_view && extends(lock))`. Update `lock` only on receipt of a higher QC. Persist `lock` synchronously before voting. This is exactly the Jolteon/DiemBFT-v4 safety contract; do not deviate |
| Bounded queue overflow drops a vote that would have completed a quorum | High | Vote inbox capacity ≥ `N` per peer; alerts on `vote_drop_total`; peer can re-request its vote on its own resend |
| Crash between SafetyRules WAL write and vote-send: replica re-votes a different value on restart | High | The WAL is the source of truth; on restart, re-read WAL, refuse to vote for any view ≤ `last_voted_view` |
| Indexer falls behind; finality RPC returns Pending forever | Medium | Watermark monitor + alert; backpressure on commit only on archive nodes, never on validators |
| Tokio task scheduling reorders consensus-relevant events | Medium | All consensus state changes are single-threaded inside `ConsensusActor`; the actor's inbox is FIFO |
| Floating-point creeps into consensus-visible code | Medium | Clippy lint banning `f32`/`f64` in `consensus::` crates; CI gate |

### 7.2 Liveness risks

| Risk | Severity | Mitigation |
|---|---|---|
| Faulty leader stalls forever | High | DiemBFT-v4 Pacemaker with timeout doubling; `TimeoutCert` view-change after 2 × estimated round time |
| All 4 honest validators alive but one is slow → 4-of-5 quorum delayed by straggler | Medium | Return on quorum-met (§3.6); late vote recorded separately |
| QUIC connection drops under load | Medium | Reconnect with backoff; persistent stream-handle pool; heartbeat datagrams |
| RocksDB compaction stalls writes | Medium | Tune `max_background_compactions`, `max_background_flushes`; rate-limited writes; separate mempool storage |
| Background indexer steals all CPU | Medium | Pin indexer to ≤ 2 rayon workers; consensus tasks are tokio-scheduled on a separate runtime |

### 7.3 Post-quantum crypto risks

| Risk | Severity | Mitigation |
|---|---|---|
| ML-DSA signature parser bug → forgery | **Critical** | Use a vetted, NIST-aligned crate (`pqcrypto-mldsa`, `ml-dsa` reference, or `liboqs-rust`); fuzz-test the parser; bind to FIPS 204 test vectors; never accept signatures whose binary length does not match the declared parameter set |
| Aborts in ML-DSA signing produce timing side-channels | Medium | ML-DSA signing time is non-deterministic by design (Fiat–Shamir with Aborts, mean 4.25 iterations at Level 1). Document that signing time leaks no key material; do not use deterministic mode in production (hedged mode is recommended) |
| ML-DSA-44 (Level 1) chosen for speed → insufficient long-term security margin | Medium | Use **ML-DSA-65 (Level 3)** for validators; signatures 3,309 B, pk 1,952 B, verification ~50–150 µs. This is the standard middle-ground recommended for blockchain settlement |
| Future migration to threshold-ML-DSA or Falcon+LaBRADOR breaks ABI | Medium | Wrap signatures in a tagged enum `SigBytes::MlDsa65 { .. } / ::ThresholdMlDsa65 { .. } / ::FalconLabrador { .. }` from day 1, even though only one variant ships |
| BLS-style aggregation creeps in via copy-pasted Ethereum/Cosmos patterns | **Critical** | Code-review gate: no `aggregate`, `combine`, `sum_of_signatures` primitive on `Signature` types. Architectural ADR explicitly bans aggregation for v1/v2. |

### 7.4 Auditability risks

| Risk | Severity | Mitigation |
|---|---|---|
| `tx_finality` returns proof without full re-verification → auditor cannot verify offline | Medium | `tx_finality` returns `cert_digest`, not just `cert_hash` of the block. Auditor fetches `Certificate` by digest, fetches `validator_set` by epoch, re-verifies all N sigs. The proof is **complete**, just not pre-verified in the RPC. `tx_audit` performs the pre-verification for the caller |
| Indexer is the single source of truth → if compromised, false proofs | High | Indexer is read-only relative to the source-of-truth CFs; `tx_audit` rebuilds the proof from raw `blocks`/`certificates`/`batches`/`receipts` CFs; periodic integrity check compares indexer state to a recomputed walk over last K blocks |
| Snapshots become a way to skip auditing old history | Medium | Snapshots are **co-signed** by ≥ Q validators (ML-DSA ratification cert using the same Certificate schema). An auditor who distrusts a snapshot can still replay end-to-end from genesis; the snapshot is an optimization, not a trust shortcut |
| Late-vote log diverges silently | Low | Periodic consistency check: for each finalized block, `voter_bitmap_in_cert + late_voters_in_late_votes ⊆ active_validator_set_at_height` |

### 7.5 Where not to cut corners

These are the items where the temptation to optimize will be highest and where doing so will break the design:

1. **Do not skip the SafetyRules fsync.** It is ~100 µs on NVMe and prevents the entire class of "voted twice in the same view across a crash" safety violations. There is no acceptable substitute.
2. **Do not move ML-DSA verification onto `tokio::spawn_blocking`.** Use `rayon` with an explicitly sized pool. Mixing the two will cause unpredictable head-of-line blocking when blocking-pool gets saturated by other I/O.
3. **Do not let the indexer be the validator's path to commit.** The validator commits *before* the indexer has indexed; the indexer is downstream. Reversing this couples consensus liveness to indexer health, which is wrong.
4. **Do not allow proposers to publish certificates that omit non-faulty votes received in time.** This is the censorship vector; the proposer-fairness rule (§3.6) is mandatory.
5. **Do not weaken ML-DSA-65 to ML-DSA-44 to "save space".** The Level-3 parameter set is the right security target for validator authentication; the storage win from -44 (≈900 B/sig) is not worth the security gap and the migration cost when -65 is what the rest of the PQ ecosystem standardizes on.
6. **Do not accept probabilistic finality even temporarily.** A 2-chain Jolteon commit is deterministic at the moment the second QC lands. Anything that says "wait k blocks for safety" is the wrong model.
7. **Do not introduce a "trusted sequencer" for testnet convenience.** It will become permanent. The leader rotation is the sequencer.
8. **Do not remove the certificate from the finality proof to save bandwidth.** The cert *is* the finality proof. Compress it (zstd column-family compression in RocksDB), commit to it by digest, store it once — but the cert itself stays.

## 8. Validation Plan

## 8. Validation Plan

This section answers RQ14–RQ15.

### 8.1 Replacement benchmark suite (RQ14)

The current "20 measured rounds, 1 tx per block, fresh validator per round" harness measures the *wrong thing* — it dominantly measures process-spawn cost and per-query history-walk cost. It must be replaced.

**The new harness has 5 named workloads:**

#### Workload A — `single_tx_finality_latency` (user-facing p50/p95/p99)
- 5 long-running daemons.
- One client submits 1000 txs at 1 tx/sec.
- Measure submit-to-finality per tx; report p50/p95/p99.
- **Pass criteria:** local 5-val p50 < 2 s, p95 < 4 s; multi-machine LAN p50 < 5 s, p95 < 10 s.

#### Workload B — `block_close_latency`
- 5 long-running daemons.
- Burst: 10,000 txs in a 1-second window.
- Measure the time from each tx admission to its block-close event (commit-QC formed).
- Report p50/p95/p99 and the time distribution of block sizes.
- **Pass criteria:** block-close p95 < 1 s under load; block-size distribution shows adaptive batching working.

#### Workload C — `sustained_throughput_under_load`
- 5 long-running daemons.
- Ramp injection rate from 100 tx/s to saturation; measure throughput and p95 latency at each step.
- Report the throughput at which p95 latency exceeds 4 s (the *useful* throughput, à la Mysticeti).
- **Pass criteria:** ≥ 2000 tx/s before p95 crosses 4 s on local 5-val.

#### Workload D — `catch_up_latency`
- Run for 1000 blocks. Stop a validator. Restart it.
- Measure time-to-`watermark == tip` (both via state-sync and via replay).
- **Pass criteria:** state-sync catch-up < 60 s for 1000 blocks; replay < 5 min.

#### Workload E — `finality_rpc_latency_vs_height`
- Run for 10,000 blocks. Issue `tx_finality` queries for txs distributed across all heights.
- Plot RPC latency vs target height.
- **Pass criteria:** flat distribution — p95 latency at height 10,000 ≤ 1.2 × p95 at height 100.

#### Reporting format

Each workload produces a JSON file with histograms; a `bench-report` tool generates Markdown summaries with sparklines and regressions vs the last run. CI gates on regressions of > 10% in any p95.

### 8.2 Safety tests

| Test | Approach | What it catches |
|---|---|---|
| **SafetyRules property test** | Quickcheck/`proptest`: generate random sequences of proposals & votes, assert no two votes from same validator in same view | Re-vote bugs |
| **Two-phase commit safety** | Model-check (TLA+ or `stateright`) the 2-chain Jolteon rule for N=5, f=1 | Concurrency / view-change subtleties |
| **Crash-recovery determinism** | Fault-injection: kill a validator at every line of the vote path; on restart, assert it does not vote for a different value at the same view | WAL bugs |
| **Certificate canonical-bytes test** | For random vote-arrival orderings, assert the produced `Certificate` has identical canonical bytes | Nondeterminism in cert |
| **No-floats lint** | Clippy + CI | Drift |

### 8.3 Simulation tests

Build a **deterministic simulator** (in the spirit of the Mysticeti and DiemBFT simulators) that runs N validator state machines in a single process with a controllable, seeded scheduler:

- Discrete-event simulation of messages, with configurable per-link latency, jitter, drop rate, and reorder.
- Same `ConsensusActor` code that runs in production; the simulator only mocks the network transport.
- Seedable: a failing trace is replayable.
- Used for: large-scale BFT scenarios you cannot run as real processes (e.g. 100-validator stress tests), partial-network-partition scenarios, leader-rotation-under-faults.

### 8.4 Byzantine tests (BFT drills)

| Drill | Validator behavior | Expected outcome |
|---|---|---|
| **B1: crashed leader** | Leader of view v silently drops messages | View-change in ≤ 2 × round-time; new leader makes progress |
| **B2: equivocating leader** | Leader proposes block A to followers 1,2 and block A' to followers 3,4 | At most one of A, A' can ever gather a 4-of-5 QC; the other is permanently stalled; safety holds |
| **B3: silent follower** | One follower (1 of 5) never votes | System continues; certificate has 4 sigs, bitmap excludes the silent voter |
| **B4: forking follower** | Follower votes for two different blocks in same view | SafetyRules detection on receipt; offending sig is *valid* (it's a real ML-DSA sig) but evidence is logged; in v2 add slashable-evidence channel |
| **B5: late vote injection** | Follower delays vote by 30 s, then sends it | Cert is sealed at quorum-time; late vote goes to `late_votes` CF; no liveness impact |
| **B6: cert replay** | Adversary replays an old cert as if for the current view | View number in cert mismatches; rejected |
| **B7: cross-epoch cert** | Cert signed by epoch e-1 registry presented for epoch e block | `registry_root` mismatch; rejected |
| **B8: partition** | Network partition isolates 2 of 5 validators for 60 s | 3-validator side has quorum (3 < 4): stops; 2-validator side has 2 < 4: stops; after heal, all 5 catch up; **no safety violation** |
| **B9: clock skew** | Leader proposes with timestamp 60 s in the future | Followers reject (`MAX_CLOCK_SKEW=±2s`); view-change; new leader |

These drills run nightly in CI on a 5-validator harness; results posted to a public dashboard.

### 8.5 Soak tests

- **24-hour soak** at 500 tx/s constant load on a 5-validator LAN cluster. Pass: no memory growth > 200 MB over baseline; no RocksDB compaction backlog > 4 SST files at L0; no `consensus_inbox_full` events.
- **7-day soak** at 100 tx/s with one weekly validator restart (rolling). Pass: no liveness gaps > 30 s; no safety violations.
- **Memory leak detection**: `heaptrack` or `bytehound` profiling on the validator daemon during soak.

### 8.6 Release gates

A controlled-testnet release is conditional on **all** of the following:

1. Workload A passes targets on local 5-val and on a 3-region WAN 5-val.
2. Workload E shows flat-with-height finality RPC latency.
3. All B1–B9 drills pass with zero safety violations.
4. 24-hour soak passes.
5. SafetyRules property test runs ≥ 10⁶ random sequences with zero failures.
6. Code review checklist confirms:
   - No `unbounded_channel`.
   - No `f32`/`f64` in `consensus::`.
   - All ML-DSA signatures are exactly the declared parameter set's expected length.
   - No `aggregate`/`combine` operations on `Signature` types.
   - Every `tx_finality` codepath either hits the index or returns `Pending`; none re-walks the block log.
   - Every commit is exactly one `WriteBatch` with `set_sync(true)`.

### 8.7 Metrics dashboard

Validator must expose Prometheus metrics including:

```
# Hot-path latency
consensus_propose_to_commit_seconds (histogram)
consensus_vote_collection_seconds   (histogram, labeled by phase)
consensus_round_total_seconds       (histogram)
storage_commit_fsync_seconds        (histogram)
storage_indexer_lag_blocks          (gauge)

# Throughput
mempool_admit_total                 (counter)
mempool_drop_total                  (counter, labeled by reason)
consensus_blocks_committed_total    (counter)
consensus_txs_committed_total       (counter)

# Correctness / health
consensus_view_changes_total        (counter, labeled by cause)
consensus_certificate_verify_fail_total (counter)
network_peer_disconnects_total      (counter, labeled by peer)
network_quic_streams_open           (gauge, labeled by peer)
storage_compaction_pending_bytes    (gauge)

# Crypto
mldsa_verify_duration_seconds       (histogram)
mldsa_sign_duration_seconds         (histogram)
mldsa_sign_aborts_total             (counter)  # Fiat-Shamir aborts
```

A Grafana dashboard with these panels is part of the release deliverable. The single most important panel is **`consensus_round_total_seconds` p95 over time** — if this is flat across height, the central pathology is gone.

## Closing Summary

The PostFiat L1 finality latency problem is fundamentally a **runtime, networking, and storage problem masquerading as a consensus problem**. The Cobalt-derived canonical validator governance and the peer-certified ordering rule are not the bottleneck. The bottleneck is (a) short-lived per-round validator processes, (b) serial vote-fan-out and serial certified-batch broadcast, (c) an O(height) finality RPC that re-walks the entire block log on every query, and (d) aggregate-log rewriting on every commit.

Fixing those four things — without changing the protocol's commit rule and without weakening post-quantum authentication — should drop submit-to-finality from the current ~10.7 s p50 / ~18.7 s p99 to roughly **0.6–0.9 s p50 and 1.5–2.5 s p95** on a local 5-validator cluster, and **1.2–2.5 s p50 and 3–6 s p95** on a multi-machine LAN, comfortably under the brief's targets. A subsequent migration to a 2-chain Jolteon-style commit rule saves an additional ~RTT and aligns the codebase with the DiemBFT/Aptos/Flow lineage without sacrificing deterministic finality.

The critical post-quantum constraint — that ML-DSA signatures cannot be aggregated like BLS — does not change. The right design is **N detached ML-DSA-65 signatures per certificate**, **stored once** in a dedicated RocksDB column family, **committed to in the block header by 32-byte digest**, and **batch-verified on a Rayon CPU pool** distinct from the Tokio I/O runtime. Speculative migration paths to threshold ML-DSA (Kao 2026) or LaBRADOR-aggregated Falcon are noted as future v2/v3 work but are not production-ready in mid-2026 and are explicitly not relied on.

Auditability is preserved end-to-end: every certificate, every receipt, every block remains on disk, replayable, and verifiable by a `tx_audit` endpoint and by an offline replay binary. The hot-path `tx_finality` RPC returns a Merkle inclusion proof plus a 32-byte certificate digest; the auditor can always fetch and re-verify the full certificate. The finality proof is **bounded but not weakened**.

The implementation plan is sequenced to deliver the largest latency reductions first (parallel vote collection, persistent daemons, indexed finality RPC) in the first 24 hours; structural runtime rework (RocksDB column families, QUIC, SafetyRules WAL) within 72 hours; and protocol-shape evolution (2-chain Jolteon, governance epoch binding, snapshots) within 2 weeks. Every step has explicit acceptance tests and ties to a numbered safety, liveness, or auditability invariant. The release-gate checklist enforces the non-negotiables: no BLS-style aggregation, no probabilistic finality, no unbounded queues, no trusted sequencer, no hiding verification in an unauditable service.

If the team executes the first-24-hour set of changes and the benchmark still grows linearly with height, the residual cause is almost certainly the per-commit RocksDB write path or the indexer lag; both are diagnosable from the proposed Prometheus metrics within a single round of profiling. Anything beyond that points back at the protocol shape, where the Jolteon migration is the next move.