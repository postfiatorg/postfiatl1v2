# FastPay `train-a-r8` zero-`redb` hot path, live WAN verification, and docs sync

- **Operator:** Antigravity (`antigravity`)
- **Date:** 2026-11-28 UTC

## BLUF

Eliminated post-block FastPay v3 tail-latency spikes on the live 6-validator
`postfiat-wan-devnet-2` fleet (`train-a-r5` through `train-a-r8`), achieving
`0` hot-path `redb` opens and `0` cache misses across 19 live consensus blocks
(`1,370` server RPCs): direct 1-RTT `273.0 ms` p50 / `306.6 ms` p95 (down
`12.3x`), direct 2-RTT `563.8 ms` p50 / `596.0 ms` p95 (down `7.2x`), x402
`402 -> 200` `321.7 ms` p50 / `350.2 ms` p95 (down `4.3x`), co-located
`validator-2` `owned_sign_v3` `11.7 ms` p50 / `18.3 ms` p95 (down `19.3x`), and
`K = 8` concurrent `OwnedObjectPool` x402 burst `9.33 calls/s` (`32/32` OK).
The Rust fixes live on PR #328 (`perf/fastpay-latency-w1-w4`, deployed 6/6 as
`train-a-r8`, not yet merged to `main`), and public blog/docs updates are live
on `postfiat.org` (PRs #85, #86, plus follow-up docs sync).

## Current state

### Live fleet state (observed 2026-11-28 16:35 UTC)

- **Live probe performed:** Yes. After deploying `train-a-r8` 6/6 (`MANIFEST_OK`
  on every host), the 3-phase cross-block WAN benchmark crossed 19 live
  consensus blocks (`2016 -> 2035`) with 6/6 convergence, `0` unconfirmed
  certificates, `0` apply retries, and `0` full-history scans.
- **Chain / height:** `postfiat-wan-devnet-2`, height `2035` (`6/6` converged).
  - `state_root`: `79d788fbbe84213ef94f6dc13dc778d5390ebd458fb3a1c5a3f04e51a7d548909571fed96b1267ab68c594bb09414fd6`
  - `block_tip_hash`: `09a2f03138dfc06a1e508a85cbeb56f0927af711d8e98759c59c4e5f5f2298b2c7080b04c7e48331f7b95d441aca9111`
- **Deployed release:** `train-a-r8` (built from `perf/fastpay-latency-w1-w4`
  commit `1779951c8ed9d1b231e97f23ecf3b0b445aac9d4`, PR #328; **not on `main`
  yet**).
  - Node binary SHA-256: `d793d289a8444e26f77cbeee0fc7c9a3ccbcda64620e8333344997c82f8d4b12`
  - Deployment manifest SHA-256: `78d9227a825b8bf77fe65687dd6ff155adb80f120d7fb445ed242c06f7f1f83f`
- **Active FastPay committee note:** FastPay committee epoch 1 has `5-of-5`
  active signing keys (`validator-0..4`). `validator-5` rotated its consensus
  key at height `917` (`e85c2c259acdf11d -> f55c35faf47fa84b`) while epoch 1
  stayed pinned to the prior key, so `validator-5` operates as a non-voting
  holder returning unsigned `postfiat-fastpay-held-effect-v1` receipts until a
  governance epoch-2 rotation is executed.
- **Historical status pages:** [Current State](../status/chain-state-current.md)
  and [Master State](../status/master-state.md) still record the 2026-11-27
  `pq-r3` snapshot at height `1915`; they should be updated once PR #328 is
  merged to `main` and a `main`-lineage release is cut.

### What changed on `perf/fastpay-latency-w1-w4` (PR #328)

1. **`46e53948c` (`train-a-r5`):** Added atomic post-block
   `fastpay_ledger_snapshot.v1.json` sidecar + inode/tip-commitment freshness
   check (eliminating false misses caused by `redb` close-time compaction
   length changes) + background cache pre-warmer.
2. **`b566415c6` (`train-a-r6`):** Added commit-time WAL pruning
   (`prune_fastpay_speculative_wal_after_block_commit` inside
   `apply_transactional_ordered_commit_delta()` in
   `crates/node/src/storage_commit.rs` + `crates/node/src/fastpay_recovery_node.rs`),
   truncating anchored records out of `fastpay_speculative_effects_v2.wal` at
   block commit so post-block `owned_apply_v3` never opens `redb`.
3. **`db35d664e` (`train-a-r7`):** Eliminated a background pre-warmer race where
   off-lock `v2.wal` reads with `truncate_torn_tail = true` raced concurrent
   `owned_apply_v3` WAL appends (which had caused `UnknownInput` on `v0` during
   `r6`). Pre-warming now runs once per finalized tip, parses the snapshot
   off-lock, and synchronizes WAL replay + cache install under
   `lock_ordered_commit()` without overwriting newer in-block state. Also
   expanded `tools/x402-facilitator/chain-adapter.js` to report all
   per-validator errors on failure.
4. **`1779951c8` (`train-a-r8`):** Added `FASTPAY_V2_IDLE_READ_TIMEOUT_SECS = 300`
   in `crates/node/src/fastpay_rpc_v2.rs`, decoupling the persistent TLS 1.3
   idle read timeout from short per-request `--timeout-ms` (`30 s` on `v1..v4`
   vs `120 s` on `v0`). This eliminated the `~741–788 ms` (`3 x RTT`)
   post-block TLS reconnect spikes observed during `37–61 s` block-anchor
   pauses.

### Guardrails for future agents touching FastPay or storage

- **Never read or truncate `fastpay_speculative_effects_v2.wal` off-lock:**
  always synchronize WAL replay and cache installation under
  `lock_ordered_commit()`.
- **Keep `FASTPAY_V2_IDLE_READ_TIMEOUT_SECS` decoupled from `--timeout-ms`:**
  shrinking the idle timeout below block-anchor pauses reintroduces 3-RTT mTLS
  reconnect tails on `v1..v4`.
- **Keep `chain_tip_generation.v1`, `fastpay_ledger_snapshot.v1.json`, and
  `prune_fastpay_speculative_wal_after_block_commit` intact** in
  `apply_transactional_ordered_commit_delta()`; removing any of them reintroduces
  hot-path `redb` opens and `storage_writer_busy` contention.
- **Canonical FastPay v3 fee floors** (`crates/execution/src/owned_transfer_recovery.rs`):
  weighted on canonical unsigned order `signing_bytes_len`: `35 atoms` for
  1-in/1-out sweep (`4,582 B`), `1,043 atoms` for 1-in/2-out with change
  (`8,513 B`), `46 atoms` for 10-in/1-out merge (`5,590 B`).

### Public website & docs status (`postfiatorg/postfiatorg.github.io`)

- Split the combined announcement into two TIH-reviewed posts (live HTTP 200):
  - `/blog/post-quantum-messaging-fastpay-x402/` (TIH `86.67` avg)
  - `/blog/sub-second-post-quantum-fastpay-x402/` (TIH `85.17` avg)
- Updated `/docs/fastpay/`, `/agents/fastpay.md`, `/docs/message-protocol/`,
  and `/llms.txt` to match `train-a-r8` live benchmarks and zero-`redb` /
  `300 s` idle-timeout architecture.
- Archived raw `r8` evidence JSON at
  `/evidence/message-protocol/devnet2-wan-fastpay-x402-20261128-r8.json`.

## Next decision or action

1. **Review and merge PR #328** (`perf/fastpay-latency-w1-w4`) into `main` so
   `postfiatl1v2`'s MkDocs `/docs/l1/` updates deploy and the running fleet code
   joins `main` again.
2. **Cut a `main`-lineage validator release** (updating
   [Current State](../status/chain-state-current.md),
   [Master State](../status/master-state.md), and `ops/b24-live/known-releases.txt`
   if the B24 release pin needs the new build).
3. **Optional governance follow-up:** Enact a FastPay committee epoch-2 rotation
   to include `validator-5`'s rotated key (`f55c35faf47fa84b`) and restore
   `5-of-6` active signing tolerance (`f = 1` with 1 spare signer).

## References

- PR #328: `https://github.com/postfiatorg/postfiatl1v2/pull/328`
- Website PRs: `#85`, `#86` on `postfiatorg/postfiatorg.github.io`
- [FastPay For Agents](../integrate/fastpay-for-agents.md)
- [RPC Methods — FastPay RPC v2](../rpc/methods.md#fastpay-rpc-v2-direct-tls-13-binary-protobuf-transport)
- [Settlement Lanes](../architecture/settlement-lanes.md#fastpay-payment-lane)
- [State And Storage — FastPay hot-path sidecars](../architecture/state-and-storage.md#fastpay-hot-path-sidecars-and-commit-time-wal-pruning)
- `tools/x402-facilitator/README.md`
- Raw `r8` evidence on operator host: `/home/postfiat/var/train-a-r8/evidence/wan-fastpay-x402-r8-bench.json`
- Public `r8` evidence: `https://postfiat.org/evidence/message-protocol/devnet2-wan-fastpay-x402-20261128-r8.json`
