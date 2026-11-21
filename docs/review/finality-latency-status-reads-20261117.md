# Why a transaction takes about 9 seconds to finalize, and a first fix

- **Date:** 2026-11-17 UTC
- **Branch:** `perf/finality-status-reuse-20261117`, based on `43ddc7d6` (the commit the live validators run)
- **Status:** deployed to all six validators on 2026-11-17 as release `b24-release5-20261117` (binary sha256 `69de5ce0…bcd18`, revision `ac91a1f4`). Measured results are in the last section.

## Measurements

All 60 transactions recorded in `b23-coord/evidence/**/finality.json` between heights 1302
and 1395 (13 to 16 November) show a certified round of 8 to 11 seconds and a submit call of
12 to 16 seconds on the proposer. The proposer's report for height 1395
(`/var/lib/postfiat/validator-3/finality-artifacts/rpc-finality-5071-b24-1794869260168598770/rpc-finality-round.json`):

| Stage | ms |
| --- | ---: |
| Setup and outbox resume | 571 |
| Build proposal | 903 |
| Local vote | 1,009 |
| Prepare votes, five peers in parallel | 3,557 |
| Precommit votes, five peers in parallel | 2,777 |
| Certificate assembly, other | 305 |
| Local apply (execution 20 ms, state root 45 ms) | 398 |
| Status read after apply | 442 |
| **Round** | **9,020** |
| **Submit call including the round** | **12,451** |

One prepare vote request, proposer side: 2,685 ms total, of which `status_ms` 1,648 and
waiting for the reply 996. The peer reports 487 ms of handling (vote creation 326 ms).

## Cause

`status()` (`crates/node/src/lifecycle_queries.rs`) reads and decodes the whole store:
genesis, node state, governance, mempool and the full ledger. On the live validators that is
about 440 ms when nothing else is running (`post_apply_status_ms`, `readiness_wait_ms`).
The validators are 2-vCPU, 4 GB machines with no free memory, so five of these in parallel
take about 1.6 seconds each.

The round calls it far more often than it needs to:

| Where | Calls per transaction | Used for |
| --- | ---: | --- |
| Proposer, each vote request (`transport_block_vote_request`) | 10 (5 peers, 2 phases) | node id and topology check only |
| Each peer, before reading each request (`handle_transport_validator_connection`) | 2 per peer | node id only, for vote requests |
| Proposer, round setup | 2 | parent and identity |
| Proposer, after apply | 1 | new tip |
| Submit wrapper (readiness, view, proposer check, mempool round, final state) | about 5 | parent, height, identity |

The ten proposer calls and the peers' calls sit directly inside the two vote phases. The
peers' call is the 500 ms gap between the proposer's 996 ms wait and the peer's reported
487 ms of handling: it happens before the peer's timer starts.

## Change on this branch

1. **Proposer.** `TransportBlockVoteRequestOptions` takes an optional `local_status`. The
   round passes the status it already read at setup to every prepare and precommit vote
   request. The standalone CLI command passes `None` and behaves as before.
2. **Peers.** The validator service reads the request first. For a block-vote request it
   reuses the first status it read (kept in `TransportValidatorServeSharedState`), since the
   vote path only uses `node_id` from it. It still runs ordered-commit crash recovery first
   (`recover_interrupted_ordered_commit`), which `status()` used to do as a side effect. Every
   other request kind reads a fresh status, now after the request arrives instead of before.

No message, vote, certificate or state format changes. A validator with the change and one
without it interoperate, so it can be rolled out one validator at a time.

## Tests

- `cargo test -p postfiat-node --bin postfiat-node transport`: 51 passed, including the
  consensus v2 failed-proposer tests for four and six validators.
- `cargo test -p postfiat-node --test b23_facility_offer_finality`: 6 passed.
- `atomic_swap_local_six_validator_tcp_finality_and_catch_up` (ignored by default) **fails
  with and without this change** at `atomic_swap_local_six.rs:5091`: it expects 6 votes and
  the round certifies with 5. That is an existing mismatch between the test and quorum-early
  vote collection, not caused by this branch.

There is no local measurement of the speed-up: local test ledgers are tiny, so `status()` is
cheap there. The numbers below are derived from the live report, not measured.

## Expected effect (estimate)

| Stage | Now | After |
| --- | ---: | ---: |
| Prepare votes | 3,557 | about 900 (bounded by the local vote and the peers' 487 ms) |
| Precommit votes | 2,777 | about 600 |
| Everything else | 2,686 | unchanged, perhaps lower with less CPU contention |
| **Round** | **9,020** | **about 4,200** |
| **Submit call** | **12,451** | **about 7,600** |

## What is left to reach about 3 seconds

1. **The submit wrapper.** About five more sequential full status reads around the round
   (roughly 2.2 seconds). They check parent, height and proposer, so reusing one read needs
   more care than this change did.
2. **Make `status()` cheap.** Most callers want identity and the chain tip, not the ledger.
   A light variant that skips the ledger, governance and mempool would fix every caller at
   once.
3. **Proposal build (903 ms) and local vote (1,009 ms).** Not investigated.
4. **Hardware.** 2 vCPUs and no free memory make every parallel step contend. Larger
   machines would help independently of code.
5. **Wrong-proposer retries.** A client that sends to the wrong validator loses 2 to 3
   seconds per miss. The proposer for a height is deterministic, so clients can go to it
   directly.

## Open question

Earlier measurements on this network were under a second. What changed has not been
established. The two vote phases of consensus v2 and ledger growth (status cost grows with
the ledger) are both candidates.

## Deployed and measured (2026-11-17, 14:28 to 15:22 UTC)

Rolled with the release 1 to 4 procedure (`ops/b23/rollout_one.sh`): validator-1 first, then
0, 2, 3, 4, 5, each followed by a test transfer accepted on all six with one state root.
Records: `b23-coord/evidence/ops/release5/`. Rollback units are saved on each host under
`/etc/postfiat/releases/b23-release4-20261113/rollback-units/`, and the release 4 binary is
still in place.

Before any host was touched, both binaries were run on a copy of validator-1's data
(key files excluded, copy deleted afterwards): `status` gave the same height and state root,
`verify-finalized-checkpoint` verified with both, and `verify-state` and `verify-blocks`
failed identically with both (`unsupported governance amendment kind
rejected_tx_fee_activation_height`), which is a gap in the running line, not in this change.

| Round | Proposer build | Round ms | Submit ms |
| --- | --- | ---: | ---: |
| 1422 | old | 10,670 | 14,699 |
| 1423 | new, peers old | 7,040 | 10,870 |
| 1424 to 1427 | old (four rounds) | 8,296 to 10,386 | 12,172 to 15,167 |
| 1428 | new, one peer old | 7,129 | 10,675 |
| 1429 | all new | 5,450 | 9,410 |
| 1430 | all new | 6,931 | 13,691 |
| 1431 | all new | 6,378 | 11,527 |

Average round: about 10.0 seconds before, 6.3 after. The estimate above (4.2) was too
optimistic. Round 1431 by stage: setup 658, proposal 944, prepare votes 1,855, precommit
votes 1,511, apply 417, status after apply 559. `status_ms` in every vote request is now 0
and the proposer's wait equals the peer's reported handling. What bounds the vote phases now
is the slowest peer (1,062 ms of handling plus 235 ms to connect on the farthest host).

The submit call did not improve as much as the round (about 14.3 to 11.5 seconds): the
wrapper's own status reads are untouched and one of the three took 6.8 seconds outside the
round.
