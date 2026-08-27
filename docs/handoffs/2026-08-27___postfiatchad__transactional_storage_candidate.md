# Transactional storage candidate

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-27 UTC

## BLUF

Implemented the selected transactional storage candidate required by the
[locked storage-scaling fix specification](../architecture/storage-scaling-fix-spec.md)
in commit `cc9e32d71d697a6db97fac76744df26d320441b9`. The active finality path now
uses a typed `redb` store, one atomic transaction per finalized height,
bounded ordered-history lookups, versioned existing-chain activation and
cancellation records, side-by-side migration tooling, transactional
snapshot/pruning support, and offline evidence interfaces. This supersedes the
fixed-bitmap candidate described in the
[previous storage handoff](2026-08-26___postfiatchad__storage_scaling_implementation.md).
It is still an undeployed development candidate: exact height-915/924 replay,
the six-clone existing-chain rehearsal, the strict release performance
campaign, the final tamper campaign, and the checksum-bound release packet have
not passed. Public testnet remains blocked.

## Current state

### Repository and deployed-state separation

- Branch: `main`.
- Implementation source: `cc9e32d71d697a6db97fac76744df26d320441b9`.
- The commit containing this handoff and the milestone/current-state update is
  the pushed documentation descendant of that implementation commit. Use
  `git rev-parse HEAD` after pulling for its exact identity.
- The working tree was clean when the handoff commit was created.
- The implementation commit is not deployed. The last fleet-bound node source
  remains `8cc7d15edc58b5f5a0b745143fef2d45203465ff`, with node SHA-256
  `d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf`.
- No live probe, deployment, service restart, validator mutation, snapshot
  import, migration, or network experiment occurred in this implementation
  session. The last recorded fleet observation remains the point-in-time
  `2026-08-26T06:34:55Z`–`06:35:50Z` height-924 convergence recorded in
  [Current State](../status/chain-state-current.md); it is not a right-now
  claim.
- After the operator prohibition, no Task Node action or subagent work was used
  for this implementation. Do not resume either workflow unless the operator
  explicitly changes that instruction.

### Implemented candidate

The implementation commit adds:

- a typed authenticated `redb` boundary for metadata, blocks, block-hash
  lookup, receipts, batch archive, ordered ID/ordinal indexes, current state,
  history indexes, and FastPay anchor lookup;
- consistent point/range reads and one durable write transaction containing
  all finalized-height effects, with expected-parent and idempotent-retry
  checks;
- no active-path block/receipt/archive/ordered JSONL append after storage
  activation, while retaining authenticated legacy import and replay;
- the versioned fixed-size ordered-history accumulator across proposal,
  validator reconstruction, state commitment, commit, restart, and replay;
- full logical integrity verification, deterministic side-by-side rebuild,
  checksum-bound migration manifests, disk-capacity checks, atomic generation
  publication, and `--verify-only`;
- active transactional snapshot export/import, retained-history pruning, and
  archive/full-replay support;
- Foundation-governance storage activation and pre-activation cancellation
  records without expanding Cobalt's validator/trust-only authority;
- operator commands for migration plus the four-boundary activation and
  cancellation flow: freeze record, create unsigned amendment, collect and
  assemble validator signatures, then create the consensus batch;
- storage status fields and work counters for backend/generation, commitment,
  tip/root/count/accumulator, verification height, records/bytes/pages,
  transactions, durable commit time, and stable reason codes;
- release campaign, exact replay, tamper, packet assembly, independent Python
  verification, and loopback-only read-only browser tooling under
  `benchmarks/storage-scaling/` and `python/postfiat_rpc/storage_scaling.py`.

The operational sequence is documented in the
[storage-scaling evidence README](../../benchmarks/storage-scaling/README.md).
The [active milestone](../plans/active/storage-scaling-milestone.md) now marks
only the source-level boundaries that were actually implemented; release and
existing-chain evidence gates remain unchecked.

### Verification completed

The following completed successfully against the implementation worktree:

- `postfiat-storage`: 70 passed, zero failed, two intentionally manual scaling
  tests ignored; the separate process-level SIGKILL recovery test also passed;
- four focused transactional logical-tamper scans passed, including forged
  receipts/archive/state, conflicting indexes, deleted hash indexes, and
  padded/reordered/duplicated/omitted/modified history;
- `cargo check -p postfiat-node --locked` passed with the repository's Zig
  linker environment;
- Python storage packet verifier/browser tests: 6 passed;
- Python syntax compilation for campaign, replay, tamper, packet, and verifier
  modules passed;
- Rust formatting completed and `git diff --check` passed before commit.

A full `postfiat-node` run compiled and executed without an observed failure
through the portion shown in the terminal, but the operator instructed the
session to stop waiting and produce this handoff. The suite was interrupted
with exit 130, so it is **not** recorded as a passing gate. The prior full node
run before the last small reason-code and documentation changes passed 307
tests, but that older result is development context, not the final clean-checkout
qualification.

### Gates that are still open

- The exact 915-block quarantine archive and exact authenticated controlled
  history through height 924 have not been replayed by the new release runner.
- A clean committed release binary has not yet produced replay, tamper, or
  performance evidence for this implementation commit.
- The strict six-validator campaign has not run five 50-round windows at each
  required starting height 50, 100, 500, 1,000, and 5,000.
- The exact existing-chain activation, cancellation, migration, restart,
  catch-up, pre-activation rollback, and post-activation forward-recovery
  sequence has not run on six disposable height-924 clones. There is not yet a
  dedicated six-clone migration evidence runner; the production CLI and
  source-level tests exist.
- The tamper runner exists but has not produced a clean-commit release packet,
  and the full catch-up/software-rollback qualification remains part of the
  six-clone gate.
- No final checksum-bound storage packet exists, so the verifier/browser code
  passing its synthetic unit fixture does not satisfy the release packet gate.
- No deployment decision or fleet-bound receipt exists for this source.

## Next decision or action

Resume from the
[active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
and keep the public-testnet block in place.

1. Pull `main`, confirm `cc9e32d7` is an ancestor, and run the full locked node
   suite from a clean checkout using the Zig linker environment. Record a pass
   only if the uninterrupted command exits zero.
2. Build `target/release/postfiat-node` from the exact clean commit. Supply the
   immutable exact height-915 quarantine directory and exact height-924
   authenticated controlled-chain directory to
   `benchmarks/storage-scaling/run_replay_evidence.py`. The runner must remain
   offline and must preserve the source-tree digests.
3. Implement and run the missing six-clone migration evidence harness against
   the exact height-924 source, using disposable validator signing material.
   Exercise both a cancelled pre-activation lane and a completed activation
   lane, restart/catch-up/rollback, mixed-version refusal, backup verification,
   and all-six convergence. Do not use live validator keys or hosts.
4. Run `run_tamper_evidence.py` and the strict `run_campaign.py` qualification
   from that same clean source and release binary. Do not weaken the 110%
   latency ratios, zero-full-history counters, material-stage model, or
   resource publication requirements.
5. Assemble the packet only after every referenced report is a real PASS, then
   run `python -m postfiat_rpc.storage_scaling verify PACKET` offline. Update
   the milestone and handoff from those exact identities.
6. Treat any live probe, deployment, service restart, validator write, or
   migration as a separate decision requiring explicit operator authorization
   and fresh fleet-bound evidence.

## References

- [Current State](../status/chain-state-current.md)
- [Locked storage-scaling fix specification](../architecture/storage-scaling-fix-spec.md)
- [Active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Storage and state architecture](../architecture/state-and-storage.md)
- [Storage-scaling evidence and operator workflow](../../benchmarks/storage-scaling/README.md)
- Transactional store: `crates/storage/src/transactional.rs`
- Atomic node commit and activation: `crates/node/src/storage_commit.rs`
- Migration implementation: `crates/node/src/storage_migration.rs`
- Activation/cancellation CLI: `crates/node/src/storage_activation_cli.rs`
- Exact replay runner: `benchmarks/storage-scaling/run_replay_evidence.py`
- Release campaign: `benchmarks/storage-scaling/run_campaign.py`
- Tamper runner: `benchmarks/storage-scaling/run_tamper_evidence.py`
- Packet verifier/browser: `python/postfiat_rpc/storage_scaling.py`
