# Improving FastPay Latency

**Date:** 2026-11-28  
**Status:** Design specification (`Phase 0` live on `postfiat-wan-devnet-2` at commit `b9b024327`; Workstreams `W0`–`W4` proposed)  
**Scope:** Single-owner FastPay v3 transfer/unwrap RPC hot path (`crates/node`, `crates/rpc_sdk`) and x402 settlement adapter (`tools/x402-facilitator`)  
**Related Specs:** [FastPay Payment Recovery v1](./fastpay-payment-recovery-v1.md), [FastPay Committee Local Authority Repair](./fastpay-committee-local-authority-repair-20260925.md), [Fast Validator Reads](./fast-validator-reads-20261124.md), [Latency Series II (FastPay)](https://postfiat.org/blog/postfiat-l1v2-fastpay-latency/)

> **Key terms:** **FastPay v3** is Post Fiat's consensusless owned-object payment lane: a client collects `q` validator ML-DSA-65 votes into an `OwnedTransferCertificateV3`, then applies that certificate to validators for asynchronous consensus checkpointing. **x402** is the HTTP `402 Payment Required` facilitator/client protocol settling paid API and MCP calls over FastPay v3. A **`committee_epoch`** pins the active validator public keys and quorum threshold `q`.

---

## Executive Summary

Post Fiat's Series II FastPay study measured 55 ms p50 co-located finality and 183 ms p50 cross-continent finality using ML-DSA-65 signatures. On live `postfiat-wan-devnet-2` (6 Rust validators across North America, Europe, and Asia; `committee_epoch = 1`, `q = 5`, height 1941), `Phase 0` removed a full-block history scan on `owned_apply_v3` and fixed `redb` timestamp false-misses in the in-process ledger cache—cutting co-located validator execution 101× (5,317 ms → 52.4 ms). Even so, live 3-continent FastPay v3 finality remains **1,488 ms p50** (6,494 ms observed max at N=15) and x402 settlement sits at **1,471 ms p50** (4,434 ms observed max at N=10).

Server-side validator logs show that post-quantum cryptography and Rust execution are not the bottleneck: `owned_sign_v3` takes **13.9–18.6 ms p50** on all five active signers, and co-located `validator-2` completes both `sign` and `apply` in 43.8 ms server-side (52.4 ms client wall clock). The remaining gap reflects five overlapping causes (not additive, because Step 2 includes both `apply` wire transit and disk `fsync`, and ~460 ms of the 2-round total is the 2 × 231 ms WAN ping floor):

1. **Synchronous 2-round settlement (`RC-1`):** `chain-adapter.js` waits for all 6 endpoints on `owned_sign_v3` and then synchronously waits for all 6 on `owned_apply_v3` before returning HTTP 200—adding ~894 ms (including assembly/verification) to x402 calls.
2. **TCP slow-start-after-idle on hex-JSON payloads (`RC-2`):** Both client and validators run Linux default `tcp_slow_start_after_idle = 1` (`initcwnd = 14,600 B`). Live echo measurements against `validator-0` (230.9 ms RTT) show 19.7 KB (`sign`) taking 460.2 ms (2× RTT) after 1.5 s idle and 53.1 KB (`apply`) taking 690.1 ms (3× RTT)—while both drop to 230.5–231.2 ms (1× RTT) when the congestion window is warm.
3. **`O(N^2)` speculative journal rewrites (`RC-3`):** Every `owned_apply_v3` parses, pretty-prints, and `fsync`-rewrites the full `fastpay_speculative_effects_v1.json` (1.61 MB at 27 effects; 1.89 MB at 32 effects), driving server `apply` p50 to 210–223 ms on slower European VPS disks.
4. **Zero straggler tolerance (`RC-4`):** `validator-5` (1.1 ms RTT) rotated its validator key while `committee_epoch = 1` still pins its pre-rotation key, leaving 5 active signers for `q = 5`. Any single validator stall hits the critical path directly.
5. **Suspected `redb` lease contention on tail outliers (`RC-5`):** Even on cache hits, FastPay RPCs acquire a `redb` lease to verify store metadata, creating a contention surface with 600–800 ms background `status` refreshes (hypothesis to be confirmed via `fastpay_redb_lease_wait_ms` instrumentation).

Five ordered workstreams (`W0`–`W4`) address these causes without weakening any FastPay v3 safety invariant, bringing modeled 3-continent `devnet-2` latency to **~245–270 ms p50** (1-RTT x402 / certificate finality) and **~500–550 ms p50** (full 2-round `sign + apply`).

---

## 1. Measured Baseline & Empirical Decomposition

### 1.1 Progression Across Releases

Baseline `devnet-2` runs used N=15 direct transfers and N=10 x402 calls after warmup. At those sample sizes, the reported p95 equals the single worst observed sample (`max`) and should be read as an observed tail outlier rather than a tight population quantile. Note also that moving from `train-a-r1` to `train-a-r3` combined `Phase 0` server fixes with switching the benchmark client from cold TCP connects to persistent keep-alive connections; the isolated server speedup is measured directly on co-located `validator-2` (5,317 ms → 52.4 ms, 101×).

| Benchmark / Release | Finality Boundary | Quorum & Transport | p50 | Observed Max (`p95`) | Min |
| --- | --- | --- | ---: | ---: | ---: |
| Series II (`fastpay-m1`, EU 3-val, n=25,000) | 1-RTT certificate (`owned_sign`) | 3-of-3, direct TCP | 55 ms | 58 ms *(p95)* | — |
| Series II (`fastpay-m1`, EU+NJ+SG, n=5,000) | 1-RTT certificate (`owned_sign`) | 4-of-6, direct TCP keep-alive | 183 ms | 186 ms *(p95)* | — |
| `devnet-2` `train-a-r1` direct (N=15) | 2-RTT (`sign` + `apply` acks) | 5-of-6 (5/5 active), SSH `-L` cold TCP | 6,796 ms | 9,158 ms | 5,714 ms |
| `devnet-2` `train-a-r1` x402 (N=10) | 2-RTT (`402 → sign → apply → 200`) | 5-of-6 (5/5 active), SSH `-L` cold TCP | 7,342 ms | 15,429 ms | 6,306 ms |
| `devnet-2` `train-a-r3` direct (`Phase 0`, N=15) | 2-RTT (`sign` + `apply` acks) | 5-of-6 (5/5 active), SSH `-L` keep-alive | 1,488 ms | 6,494 ms | 939 ms |
| `devnet-2` `train-a-r3` x402 (`Phase 0`, N=10) | 2-RTT (`402 → sign → apply → 200`) | 5-of-6 (5/5 active), SSH `-L` keep-alive | 1,471 ms | 4,434 ms | 1,179 ms |

Two small-sample artifacts appear in this table: (a) `train-a-r3` x402 p50 (1,471 ms, N=10) sits 17 ms below direct 2-RTT p50 (1,488 ms, N=15), well within transcontinental WAN jitter; and (b) `train-a-r3` vote-step max (5,697 ms) exceeded `train-a-r1` (2,673 ms) due to a single 5.69 s outlier on `validator-3` under the zero-straggler 5-of-5 committee.

### 1.2 Per-Validator Server vs. Client Wall-Clock Breakdown (`train-a-r3`, Height 1941)

Server `handler_ms` medians come from `/var/log/postfiat/validator-N/rpc-events.ndjson` (`handler_ms` wraps the full RPC handler including `lock_ordered_commit()` and `redb` lease acquisition); client timings come from `devnet2-wan-fastpay-x402-20261127.json` (N=15 direct transfers). `Implied WAN RTTs` is computed as `(client_p50 - server_p50) / ping_RTT`.

| Validator | Ping RTT | Server `sign` p50 | Client `sign` min / p50 | Implied `sign` RTTs | Server `apply` p50 | Client `apply` min / p50 | Implied `apply` RTTs |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `validator-2` (local) | 1.2 ms | 14.0 ms *(x402: 20.4 ms)* | 16.2 / 18.1 ms | — *(local +4.1 ms)* | 29.8 ms *(x402: 50.6 ms)* | 25.0 / 34.3 ms | — *(local +4.5 ms)* |
| `validator-5` (local, non-signer) | 1.1 ms | *refused* | — | — | 35.8 ms *(client)* | 19.6 / 35.8 ms | — |
| `validator-0` (Asia/Pacific) | 230.9 ms | 16.0 ms | 241.6 / 474.9 ms | 1.99× | 44.3 ms | 256.6 / 656.8 ms | 2.65× |
| `validator-3` (Asia/Pacific) | 229.4 ms | 13.9 ms | 254.3 / 494.8 ms | 2.10× | 82.1 ms | 493.1 / 717.6 ms | 2.77× |
| `validator-1` (Europe) | 190.7 ms | 18.6 ms | 276.0 / 473.7 ms | 2.39× | 222.6 ms | 398.1 / 624.3 ms | 2.11× |
| `validator-4` (Europe) | 190.9 ms | 14.3 ms | 346.1 / 535.8 ms | 2.73× | 209.6 ms | 540.8 / 626.4 ms | 2.18× |

Key readings from this decomposition:

1. On `validator-0`, warm-window minimums (`241.6 ms` sign + `256.6 ms` apply = `498.2 ms`) sit right on `2 × ping_RTT + server_compute`. At p50 (`474.9 + 656.8 = 1,131.7 ms`), network delay adds **+633.5 ms** despite only 60.3 ms combined server compute.
2. Live `apply` implied RTTs (2.11×–2.77×) are lower than a pure cold-start 3.0× prediction because `owned_apply_v3` immediately follows `owned_sign_v3` (19.7 KB) on the same persistent TCP connection: `cwnd` has just opened partially during `sign` (~15–20 MSS, or ~22–29 KB), allowing 53.1 KB to clear in ~2 flights on warm links (`validator-1`, `validator-4`) and ~2.7 flights on `validator-0`/`validator-3`.
3. Why `validator-1`/`validator-4` `sign` implied RTTs reached 2.39×–2.73× and why `validator-3` `min` apply stayed at 493.1 ms (~2× RTT) was not isolated with a packet capture during the run; `W0` directly tests whether eliminating post-idle slow-start collapses all five validators to ~1.0× RTT.

### 1.3 Empirical Payload-Size vs. TCP Slow-Start Isolation (`validator-0`)

We confirmed `net.ipv4.tcp_slow_start_after_idle = 1` on both the client host and `validator-0`, then sent exact-sized request frames over a persistent SSH pipe to `validator-0` (230.9 ms RTT; minimal 3-byte `"OK\n"` reply, isolating request-flight cwnd behavior—live RPC replies are ~7 KB, well inside one 14.6 KB initial flight) under two regimes: after a 1.5 s idle pause vs. in a 5-frame back-to-back warm burst:

| Request Frame Size (Matches) | After 1.5 s Idle (3 trials, median) | Warm Burst (5 trials, median) | Idle Penalty |
| --- | ---: | ---: | ---: |
| 1,024 B (baseline probe) | 231.0 ms (1.00× RTT) | 230.1 ms (1.00× RTT) | 0 ms |
| 7,600 B (proposed protobuf `owned_sign_v3`) | 230.6 ms (1.00× RTT) | 230.3 ms (1.00× RTT) | 0 ms |
| 16,700 B (proposed votes-only `owned_apply_v3_fast`) | 460.0 ms (2.00× RTT) | 230.4 ms (1.00× RTT) | +229.6 ms |
| 19,700 B (current hex-JSON `owned_sign_v3`) | 460.2 ms (2.00× RTT) | 230.5 ms (1.00× RTT) | +229.7 ms |
| 53,100 B (current hex-JSON `owned_apply_v3`) | 690.1 ms (3.00× RTT) | 231.2 ms (1.00× RTT) | +458.9 ms |

This measurement rules out SSH channel-window throttling (a 53.1 KB frame over the exact same SSH pipe finished in 231.2 ms = 1.00× RTT once warm) and isolates `tcp_slow_start_after_idle = 1` against `initcwnd = 14,600 B` as the cause of the 2×/3× RTT step function. It also proves that a 16.7 KB `owned_apply_v3_fast` payload still incurs a 2-RTT stall (460.0 ms) after idle unless `W0` disables `tcp_slow_start_after_idle`.

### 1.4 Measured Wire & Disk Payload Sizes (`validator-2`, Height 1941)

| Artifact | Current Encoding | Exact Size | Breakdown |
| --- | --- | ---: | --- |
| `owned_sign_v3` JSON-RPC request | Double-escaped JSON + hex | 19,702 B | `order_json` = 19,453 B (3 × 3,904 B hex ML-DSA-65 pubkeys + 6,618 B hex owner sig = 18,330 B, 94.2%; metadata = 1,123 B) + 249 B RPC envelope |
| `owned_apply_v3` JSON-RPC request | Double-escaped JSON + hex | 53,115 B | `cert_json` = 52,824 B (3 × 3,904 B hex pubkeys = 11,712 B + 6 × 6,618 B hex ML-DSA-65 sigs = 39,708 B; crypto hex total = 51,420 B, 97.3%; metadata = 1,404 B) + 291 B RPC envelope |
| Single speculative journal entry | Pretty-printed JSON | 59,220 B | Full `OwnedTransferCertificateV3` + `prior_objects` + indentation (57,998 B compact JSON) |
| `fastpay_speculative_effects_v1.json` | Full-file pretty JSON (27 effects) | 1,607,429 B (1.61 MB) | Rewritten and `fsync`ed in full on every `owned_apply_v3` (1.89 MB at 32 effects) |

---

## 2. Root Cause Analysis

- **RC-1 — Synchronous 2-Round Settlement & All-6 Barrier (`tools/x402-facilitator/chain-adapter.js:204-318`):** `settleFastPayAuthenticated` awaits `Promise.allSettled(endpoints.map(...))` for `owned_sign_v3` and then awaits a second `Promise.allSettled(...)` for `owned_apply_v3` before returning HTTP 200. Under the current degraded 5-of-5 signer set, quorum-first racing cannot shed stragglers until `W3` restores `validator-5`; therefore, the immediate latency cost of `RC-1` is the synchronous second round (~894 ms difference between `total_finality` 1,488 ms and `quorum_vote` 594 ms, comprising 798 ms `apply` + ~96 ms certificate assembly/verification and scheduling).
- **RC-2 — TCP Slow-Start-After-Idle on Oversized Hex-JSON Payloads:** Both `owned_sign_v3` (19.7 KB) and `owned_apply_v3` (53.1 KB) exceed Linux's 14.6 KB post-idle congestion window. Section 1.3 demonstrates +230 ms on `sign` after 1.5 s idle and +459 ms on `apply` from cold `initcwnd` (partially amortized to ~1–1.7 extra RTTs in live 2-round flows when `apply` immediately follows `sign`).
- **RC-3 — `O(N^2)` Full-File Speculative Journal Rewrites (`crates/node/src/fastpay_recovery_node.rs:414-431`):** `write_fastpay_speculative_journal` reads, validates, pretty-prints, and `fsync`-renames the full 1.61–1.89 MB file on every apply (~30 MB written per 32-effect window). On `validator-2`, server `apply` p50 rose from 29.8 ms during the first 15 transfers to 50.6 ms during the next 10 x402 calls as the journal grew; on slower European VPS disks (`validator-1`, `validator-4`), server `apply` p50 reached 210–223 ms.
- **RC-4 — Degraded 5-of-5 Signer Set (Zero Straggler Tolerance):** `validator-5` (1.1 ms RTT) rotated its validator key while `committee_epoch = 1` pins its old key ([local authority repair spec](./fastpay-committee-local-authority-repair-20260925.md)). Every `q = 5` payment waits for all 5 active signers (`validator-0..4`), leaving zero tolerance for a single validator stall.
- **RC-5 — `redb` Lease Contention Surface on Cache Hits (`crates/node/src/fastpay_recovery_node.rs:165-193`, *correlational hypothesis*):** Even on ledger cache hits, `owned_sign_v3` and `owned_apply_v3` call `store.transactional_store_lease()?` to read `transactional.meta()?`. Because `validator-1`/`validator-4` server `sign` p50 is only 14–19 ms, `redb` contention is **not** active at p50; however, periodic 600–800 ms `in_process_health_refresh` `status` calls and child-process reads share `redb`'s single-writer lock and are a plausible driver of multi-second max outliers (1,789 ms on local `validator-5` apply; 5,693 ms on `validator-3` sign). `W2` instruments `fastpay_redb_lease_wait_ms` before attributing tail latency exclusively to `RC-5`.

---

## 3. Non-Negotiable Safety, Recovery & Quorum Invariants

All proposed changes preserve [FastPay Payment Recovery v1](./fastpay-payment-recovery-v1.md):

1. **Quorum size `q = n - f` (`q = 5` for `n = 6`, `f = 1`) is preserved.** Two quorums of size `q = n - f` intersect in at least `2q - n = n - 2f` validators, of which at least `n - 3f` (here `6 - 3 = 3 >= 1`) are guaranteed honest; likewise any `q` certificate intersects any `n - f` recovery set in at least `n - 3f` honest validators. Although `q = 4` (used in Series II's pre-recovery prototype) satisfies `2q - n = 2 > f` for double-spend safety and would lower the 4th-fastest RTT on `devnet-2` from 229 ms to 191 ms, lowering `q` below `n - f` breaks recovery-set alignment with Cobalt BFT and is explicitly out of scope.
2. **Lock-before-vote:** An honest validator MUST durably persist `(object_id, version, registry_id) -> lock_id` before returning an `owned_sign_v3` vote. Importantly, `owned_locks.json` lives **outside `redb`** in the validator data directory, so lock-before-vote durability does not require opening `redb`.
3. **Persist-before-ack:** An honest validator MUST durably persist the complete verified `OwnedTransferCertificateV3` and speculative effect before signing a `FastPayApplyAckV1`.
4. **Trust-model boundary under 1-RTT x402 `'certificate'` finality:** Releasing HTTP 200 after verifying a 5-of-6 `OwnedTransferCertificateV3` (before remote `owned_apply_v3` finishes) cannot permit a double spend, **but shifts certificate-retention risk to the facilitator until the certificate reaches validators**:
   - *Facilitator crash-and-restart:* Closed by mandating `fsync` of the complete certificate into the facilitator's local **SQLite WAL (`PRAGMA synchronous=FULL`)** before returning HTTP 200, plus a startup replay worker.
   - *Permanent facilitator disk loss before remote apply:* If the facilitator's disk is permanently destroyed before any validator applies the certificate, ordered recovery cancels the lock after `recovery_closes_at_height` and returns funds to the payer. Therefore, under `'certificate'` mode: (a) **`coLocatedValidatorSyncApply: true` is REQUIRED by default** whenever a co-located validator is available (synchronous local apply takes 30–34 ms today, dropping to ~15–20 ms after `W2`, eliminating single-host disk-loss risk), and (b) `PAYMENT-RESPONSE` carries the compact `order_hash`, `certificate_digest`, and facilitator certificate retrieval URL (`GET /x402/certificates/{order_hash}`) rather than stuffing a 24–53 KB certificate into an HTTP header (avoiding 8–16 KB proxy/Node header limits). Operators without a co-located validator who refuse single-facilitator durability MUST configure `settlementFinalityMode: 'applied_quorum'`.
5. **Exact consensus anchoring equivalence:** Every effect persisted in `fastpay_speculative_effects_v2.wal` MUST produce the exact same `FastPayVersionFenceV1`, rollback inverse, and post-anchor `state_root` as `fastpay_speculative_effects_v1.json`, proven via dual-write shadow verification before cutover.

---

## 4. Proposed Solutions (Five Ordered Workstreams)

Workstreams are ordered to validate the highest-leverage assumptions with minimal code first (`W0` → `W1`), then eliminate disk/lock bottlenecks (`W2`), restore straggler tolerance (`W3`), and finally introduce the compact binary transport (`W4`).

### Workstream 0 (W0): `tcp_slow_start_after_idle = 0` & Live A/B Validation

**Target:** Facilitator and validator OS sysctl + `chain-adapter.js` startup connection priming  
**Purpose:** Immediately test the `RC-2` hypothesis on live `devnet-2` before writing binary RPC code.

1. Set `net.ipv4.tcp_slow_start_after_idle = 0` on facilitator and validator hosts (`/etc/sysctl.d/99-postfiat-fastpay.conf`).
2. Prime each persistent validator TCP connection once at facilitator startup (e.g., via one 64 KiB warmup exchange or the first real request) so `cwnd` opens past 53.1 KB and remains open across idle pauses. (Note: periodic heartbeats only keep `cwnd` warm when sent more frequently than one RTO (~250 ms), which wastes WAN traffic; disabling `tcp_slow_start_after_idle` solves it cleanly with zero idle chatter.)
3. Run an N >= 50 live A/B benchmark comparing `train-a-r3` before and after `W0` to measure exact p50/p95 reduction before proceeding to `W4`.
4. **Rollback:** `sysctl -w net.ipv4.tcp_slow_start_after_idle=1`.

### Workstream 1 (W1): Quorum-First 1-RTT Certificate Finality & Client Object Pool

**Target files:** `tools/x402-facilitator/chain-adapter.js`, `tools/x402-facilitator/facilitator.js`, `tools/x402-facilitator/mcp-client.js`

1. **Quorum-first vote racing:** Replace `await Promise.allSettled(...)` in `settleFastPayAuthenticated` with an incremental collector that verifies each vote on arrival and resolves immediately once `verifiedVotes.length === q`. (Under the degraded 5-of-5 signer set this avoids blocking on `validator-5`; once `W3` restores 6/6 signers, it sheds the slowest validator on every call.)
2. **Explicit finality modes (`settlementFinalityMode`):**
   - **`'certificate'` (default for x402 API calls):** Verify the `q`-vote `OwnedTransferCertificateV3`, `fsync` `{ orderHashHex, certDigestHex, lockId, certJson, expiresAtHeight, recoveryClosesAtHeight }` into the facilitator's local SQLite WAL (`PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;`, < 1 ms), synchronously apply to the co-located validator when `coLocatedValidatorSyncApply: true` (default when configured), and return HTTP 200 with compact `PAYMENT-RESPONSE` metadata + `GET /x402/certificates/{order_hash}` retrieval URI.
   - **Background apply delivery worker:** Fans out `owned_apply_v3` asynchronously to remaining validators with capped exponential backoff (`250 ms → 500 ms → 1 s → 2 s`, capped at `5 s`) until (a) `>= q` valid `FastPayApplyAckV1` receipts are stored, (b) the order is observed anchored on-chain, or (c) chain height exceeds `recovery_closes_at_height`. Emits a high-severity alert (`fastpay_unconfirmed_cert_expiring`) if any persisted certificate remains below `q` acks at `expires_at_height - 5`.
   - **`'applied_quorum'` (strict opt-in):** Awaits quorum-first `owned_sign_v3` followed by quorum-first `owned_apply_v3` before returning HTTP 200.
3. **Client-side `OwnedObjectPool` (`K = 4..8` pre-split objects):** In `X402HttpClient`, pre-split a funded object into `K` parallel owned objects sized at `>= max_single_call_budget` (e.g., 10× median tool price). Each x402 call checks out a ready object, allowing `K` concurrent 1-RTT calls without blocking on remote change-output apply. Returned change objects re-enter the pool once background apply confirms; if a single call exceeds the largest pooled object, the client falls back to an on-demand consolidation transfer.
4. **Rollback:** Set `X402_SETTLEMENT_FINALITY_MODE=applied_quorum` (runtime env toggle, zero schema change).

### Workstream 2 (W2): Append-Only Binary Speculative WAL & Lock-Free Tip Sidecar

**Target files:** `crates/node/src/fastpay_recovery_node.rs`, `crates/node/src/storage_commit.rs`

1. **Append-only framed speculative WAL (`fastpay_speculative_effects_v2.wal`):**
   - Replace full-file rewrites of `fastpay_speculative_effects_v1.json` (1.61–1.89 MB) with an append-only binary log of framed records: `[u32 magic_ver][u32 payload_len][u32 crc32c][bincode(FastPaySpeculativeEffectV1)]`.
   - Each `owned_apply_v3` appends one ~26 KB frame and calls `file.sync_data()` (`fdatasync`), replacing `O(N^2)` window writes (~30 MB) with `O(1)` 26 KB appends.
   - Maintain a rolling SHA-256 chain hash (`H_k = SHA256(H_{k-1} || frame_k)`) in memory so `FastPayLedgerCacheKey` validates journal state in `O(1)` without hashing a 1.6 MB file on every `owned_sign_v3`.
   - **Concurrency & crash safety:** Both `owned_apply_v3` appends and certified block compaction already serialize under `store.lock_ordered_commit()`, so compaction can never race with concurrent appends. On startup, replay valid frames and truncate any incomplete or CRC32C-invalid trailing frame. On first boot with `v1.json` present and `v2.wal` absent, convert once atomically via temp file + `fsync` + rename.
   - **Equivalence verification:** Include a shadow mode (`POSTFIAT_FASTPAY_WAL_SHADOW_VERIFY=1`, enabled in CI and on canary `validator-1`) that writes both `v1.json` and `v2.wal` and asserts identical `FastPayVersionFenceV1`, rollback inverse, and post-anchor `state_root`.
2. **Lock-free tip-generation sidecar (`chain_tip_generation.v1`):**
   - Inside `storage_commit.rs`—strictly inside the same `lock_ordered_commit()` mutex immediately after `redb` commit and before lock release—atomically rename a `< 256 B` sidecar containing `(generation, finalized_height, finalized_block_hash, finalized_state_root, ordered_history_accumulator, db_len, db_ino)`. Because `lock_ordered_commit()` serializes block commits against FastPay RPCs, no FastPay RPC can observe a partially committed tip; if the node crashes between `redb` commit and sidecar rename, startup reconciliation regenerates the sidecar from `redb` before opening RPC listeners.
   - On `owned_sign_v3` and `owned_apply_v3`, read the 256 B sidecar and `stat(db_path)` (`len`, `ino`). On a cache hit, serve directly from memory **without opening `redb`** (since `owned_locks.json` is already outside `redb`, durable lock-before-vote needs no `redb` lease). Fall back to `store.transactional_store_lease()` only on a cache miss or tip advance.
3. **Rollback:** Ship a `postfiat-node fastpay-wal-export-v1` subcommand that converts any unanchored `v2.wal` frames back into `fastpay_speculative_effects_v1.json` (or anchor one certified block to drain unanchored effects) before downgrading the binary.

### Workstream 3 (W3): Governed Committee Rotation (`committee_epoch = 2`)

**Target:** Live `postfiat-wan-devnet-2` governance + operator runbook

1. Execute a governed FastPay committee rotation (`committee_epoch = 1 → 2`) adding `validator-5`'s rotated ML-DSA-65 public key under the drain and `new_orders_through_height` fencing rules of [FastPay Payment Recovery v1](./fastpay-payment-recovery-v1.md).
2. Restoring 6-of-6 active signers gives `q = 5` genuine `f = 1` straggler tolerance so any single validator stall drops off the critical path.
3. **Pre-ratification safety check (no rollback):** Governed epoch transitions are forward-only; before ratifying, run an automated dry-run executing `sign_for_fastpay_committee` against the candidate `committee_epoch = 2` registry on all 6 validators (specifically proving `validator-5`'s private key matches its candidate public key).

### Workstream 4 (W4): Protobuf RPC v2, Votes-Only Fast Apply & Direct TLS 1.3 Transport

**Target files:** `crates/node/src/rpc_cli.rs`, `crates/node/src/fastpay_recovery_node.rs`, `crates/rpc_sdk/`, `tools/x402-facilitator/chain-adapter.js`

1. **Concrete codec & transport:**
   - **Codec:** Canonical **Protobuf v3** (`prost` in Rust, reusing the `pf.message.v1` stack) framed as `[u8 version=2][u32be frame_len][protobuf bytes]` (`postfiat-fastpay-rpc-v2`). Legacy JSON-RPC `v1` stays active on its existing port.
   - **Transport:** Direct **TLS 1.3 over persistent TCP** (`TCP_NODELAY = 1`, `tcp_slow_start_after_idle = 0`) on a dedicated FastPay listener port, removing SSH `-L` hops while keeping standard kernel TCP diagnostics.
2. **Wire-size reductions:**
   - **`owned_sign_v3` protobuf:** Raw ML-DSA-65 pubkeys (1,952 B) and signatures (3,309 B) plus a 1-bit `owner_is_change_recipient` flag omitting the duplicate change-output pubkey yield **`~7,563 B` (7.4 KiB)**—guaranteeing 1 WAN RTT even on a brand-new or post-idle socket (`<= 14.6 KB` `initcwnd`).
   - **`owned_apply_v3_fast` votes-only path:** Validators maintain a bounded in-memory LRU (`256` entries, ~2 MB RAM) mapping `lock_id -> SignedOwnedTransferOrderV3` populated during `owned_sign_v3`. `owned_apply_v3_fast` sends `(lock_id [48 B] + q raw ML-DSA-65 votes [16,545 B] + validator IDs)` = **`~16.7 KB`** (3.2× smaller than 53.1 KB hex JSON). Because 16.7 KB still exceeds 14.6 KB `initcwnd` after idle (Section 1.3), 1-RTT `owned_apply_v3_fast` explicitly relies on `W0` (`tcp_slow_start_after_idle = 0` on a warm connection). On LRU miss, the validator returns `OrderNotCached` and the client transparently falls back to the full binary certificate (~24.2 KB).
3. **Security & DoS boundary:**
   - **Authentication & rate limiting:** Mutual TLS (mTLS) with pinned client certs for facilitator/operator listeners; public wallet listeners enforce per-IP token-bucket rate limits, max concurrent connections, a hard `64 KiB` pre-allocation frame cap, and cheap `lock_id`/length checks before any ML-DSA-65 verification.
   - **Parser hardening & equivalence:** Add a `cargo-fuzz` target on the protobuf RPC decoder; reject unknown version bytes with `UnsupportedProtocolVersion`; deserialize both `v1` JSON and `v2` protobuf into identical Rust domain structs computing identical `lock_id` and `order_digest`.
4. **Rollback:** Start `rpc-serve` with `--disable-fastpay-rpc-v2` and point `chain-adapter.js` back to `v1` JSON-RPC; `W4` introduces no on-disk state changes.

---

## 5. Modeled End-to-End Latency Budget

The first two rows are measured live on `postfiat-wan-devnet-2`; subsequent rows are **modeled estimates** built from the additive formula `quorum_ping_floor + server_handler_p50 + client_crypto_overhead (~15 ms)` under each workstream's network-RTT multiplier:

- **Ping RTT floor on `devnet-2` (`q = 5`):** With 2 local (`1.2 ms`), 2 EU (`191 ms`), and 2 Asia (`230 ms`) validators, the 5th-fastest validator is `229–231 ms`, giving a **~230 ms** 1-RTT floor and **~460 ms** 2-RTT floor.
- **After `W0`:** Warm `cwnd` reduces `validator-0`/`validator-3` network transit from ~2.0×/2.7× RTT toward ~1.1×–1.4× RTT (~280–340 ms network + 16 ms server + 15 ms client = ~350–400 ms 1-RTT; adding EU 210–223 ms `apply` + ~230–300 ms network = ~800–950 ms 2-RTT).
- **After `W2`:** Replacing 1.6 MB pretty JSON rewrites with 26 KB `fdatasync` appends reduces EU `apply` server time from 210–223 ms toward local SSD/NVMe append latency (~15–25 ms), and removing hot-path `redb` opens removes lease-wait outliers.
- **After `W3` + `W4`:** 6/6 signers drop any single straggler, and 7.6 KB / 16.7 KB direct TLS 1.3 frames settle at 1.0× RTT (`230 ms + 14–18 ms server + 15 ms client = ~245–270 ms` 1-RTT; `460 ms + 32–45 ms server + 20 ms client = ~500–550 ms` 2-RTT).

| Stage | Co-located (`1.2 ms` RTT) | `devnet-2` 3-Continent p50 (1-RTT Cert / x402) | `devnet-2` 3-Continent p50 (2-RTT `sign + apply`) | `devnet-2` p95 (1-RTT / 2-RTT) |
| --- | ---: | ---: | ---: | ---: |
| `train-a-r1` *(measured, cold TCP)* | 5,317 ms | 1,126 ms *(vote step only)* | 6,796 ms (7,342 ms x402) | 2,673 ms / 9,158 ms *(max)* |
| **Current `train-a-r3` *(measured, `Phase 0`)*** | **52.4 ms** *(server: 43.8 ms)* | **594 ms** *(vote step only)* | **1,488 ms** (1,471 ms x402) | **5,697 ms** / **6,494 ms** *(max)* |
| `+ W0` (`tcp_slow_start_after_idle=0`, warm) | ~52 ms | ~350–400 ms *(vote step)* | ~800–950 ms | ~3–5 s *(5-of-5 + lease)* |
| `+ W1` (Quorum-first + 1-RTT x402 + SQLite WAL) | ~52 ms | ~350–400 ms *(x402 200 OK)* | ~800–950 ms | ~3–5 s *(5-of-5 + lease)* |
| `+ W2` (Append-only WAL + lock-free tip sidecar) | ~25–35 ms | ~260–310 ms | ~540–620 ms | ~500–900 ms / ~900–1,400 ms |
| `+ W3` (`committee_epoch = 2`, 6/6 signers) | ~25–35 ms | ~250–280 ms | ~520–580 ms | ~310–380 ms / ~620–740 ms |
| **`+ W4` (Protobuf RPC v2 + direct TLS 1.3)** | **~18–22 ms (1-RTT) / ~30–36 ms (2-RTT)** | **~245–270 ms** *(ping floor: 230 ms)* | **~500–550 ms** *(ping floor: 460 ms)* | **~290–350 ms** / **~580–680 ms** |

*Regional US+EU corridor estimate (at least 5 of 6 validators within `<= 80 ms` ping RTT, all workstreams shipped): ~95–110 ms p50 (1-RTT x402 / certificate finality) and ~190–215 ms p50 (2-RTT `sign + apply`).*

---

## 6. Observability, Risks & Acceptance Gates

### 6.1 Observability & Regression Detection

Add structured fields to `rpc-events.ndjson` and facilitator logs to validate `RC-5` and guard against regressions:

- **Validator:** `fastpay_handler_ms` (`owned_sign_v3` / `owned_apply_v3`), `fastpay_redb_lease_wait_ms`, `fastpay_wal_fdatasync_ms`, `fastpay_ledger_cache_hit` (`bool`), `fastpay_redb_opened` (`bool`), `fastpay_order_lru_hit` (`bool`).
- **Facilitator:** `x402_vote_quorum_ms`, `x402_apply_quorum_ms`, `x402_unconfirmed_certs_gauge`, `x402_apply_retry_total`.

### 6.2 Risks & Mitigations

| Risk | Failure Mode | Mitigation |
| --- | --- | --- |
| Facilitator crash or permanent disk loss after HTTP 200 (`W1`) | Certificate never reaches validators before `recovery_closes_at_height`; ordered recovery cancels lock | Local SQLite WAL `fsync` + startup replay worker handles crashes; `coLocatedValidatorSyncApply: true` (default when local validator exists) + `GET /x402/certificates/{order_hash}` retrieval handle permanent disk loss; `applied_quorum` available for strict callers |
| Torn WAL tail on power loss (`W2`) | Corrupt final frame blocks node restart or drops committed effects | `[magic_ver][len][crc32c]` framing; startup scanner truncates only an incomplete/invalid tail frame; `fdatasync` completes before signing `FastPayApplyAckV1` |
| Stale `chain_tip_generation.v1` sidecar (`W2`) | FastPay serves cached ledger from old height after external DB rebuild | Written atomically inside `lock_ordered_commit()` before lock release; startup reconciles from `redb`; cache verifies sidecar fields + `stat(db_path)` `(len, ino)` |
| Direct TLS / protobuf parser attack surface (`W4`) | Malformed frames cause OOM/panic or CPU DoS via ML-DSA-65 spam | mTLS or per-IP token bucket + 64 KiB pre-allocation cap + `cargo-fuzz` decoder harness + deterministic version rejection |
| Cross-codec equivocation (`W4`) | Conflicting orders submitted via `v1` JSON and `v2` protobuf | Identical Rust domain structs, `lock_id`, and `order_digest` checked against the same durable lock table |

### 6.3 Mandatory Acceptance Gates

All five acceptance gates must pass on live `postfiat-wan-devnet-2` before closing this specification:

1. **Adversarial & equivalence test suite:** Existing `fastpay_payment_safety` tests pass plus new tests for (a) torn `v2.wal` tail recovery, (b) `v1.json` vs. `v2.wal` dual-write shadow equivalence across applies and block anchors, (c) facilitator crash-and-restart replay before `recovery_closes_at_height`, (d) `v1`-JSON vs. `v2`-protobuf equivocation refusal, and (e) `owned_apply_v3_fast` LRU-miss fallback.
2. **Zero full-history scans & minimal hot-path `redb` opens:** After a 50-transfer + 25-x402 burst and subsequent certified block anchor, `storage.full_history_scans == 0` across all 6 validators and `fastpay_redb_opened == false` on `>= 98%` of warm FastPay RPCs.
3. **1-validator straggler test (`W3` active):** Injecting 2 s `netem` delay onto any one remote validator produces `< 10%` p50 impact and keeps `p95 <= 350 ms` (1-RTT) and `p95 <= 680 ms` (2-RTT); injecting 2 s delay onto one co-located validator keeps `p95 <= 350 ms` (1-RTT) and `p95 <= 680 ms` (2-RTT) because the remaining 5 signers still span the same 231 ms max RTT.
4. **Live 3-continent `devnet-2` latency targets (N >= 50 samples each):**
   - 1-RTT certificate / x402 `402 → 200`: `p50 <= 280 ms`, `p95 <= 350 ms`
   - 2-RTT full applied quorum (`sign + apply`): `p50 <= 560 ms`, `p95 <= 680 ms`
   - Co-located validator (1.2 ms RTT): server `owned_sign_v3 <= 18 ms`, `owned_apply_v3 <= 20 ms` (combined client `<= 42 ms`)
5. **6-of-6 consensus convergence:** Certified block anchoring immediately following the N >= 50 benchmark burst converges all 6/6 validators to one identical `state_root` and `block_tip_hash`.
