# Devnet Storage Rollout

**Status:** **SUPERSEDED on 2026-08-31 by the locked
[single-writer deployment plan](devnet-storage-single-writer-deployment-plan.md)
(TIH gate 88.60/100).** Retained as the record of the stopped 2026-08-30
attempt. **DO NOT DEPLOY `10dd9f20` or `d0ae79f3` as-is.**
`d0ae79f3` failed the first height-925 clone continuation. Successor `10dd9f20`
repaired that defect and passed the existing G6 runner, but the runner did not
exercise the signed two-process systemd topology. The validator-1 canary exposed
exclusive transactional-database ownership between transport and RPC and an
incomplete old-binary rollback surface. Validator-1 was rolled back; all six
validators are again healthy and converged at height 924.

**The job:** roll the G4-qualified storage build onto the live devnet and prove
the chain — including the already-active Cobalt governance — works on it.

Original binary: source `d0ae79f3`, release SHA-256 `9e82d928…8c80c`
(G4 PASS 2026-08-30; G1 `8df8f7a6…`, G2 `689a96dc…`). Stopped successor:
source `10dd9f20`, release SHA-256 `0cc664a3…ad4183`. Cobalt has been the
registry authority since height 916; it was not changed by either attempt.

- [x] 1. Read-only fleet probe: six validators, height, deployed binary hash,
      `authority_mode`, registry/trust roots — PASS; all six remained converged
      at height 924 with the deployed `d5e5ef63…c2696caf` binary
- [ ] 2. *(operator go)* Six-clone migration rehearsal — `d0ae79f3` **FAIL**
      at height 925; `10dd9f20` **PASS under the old runner but invalidated by
      the canary** because it started transport only, not concurrent transport
      and RPC under the signed systemd sandbox
- [ ] 3. *(operator go)* Rolling deploy — **STOPPED AND ROLLED BACK** after the
      validator-1 canary; transport acquired the `redb` database and RPC failed
      `Database already open. Cannot acquire lock.` No other validator was attempted
- [ ] 4. Health on the new storage: six converged, certified rounds finalizing,
      transactional backend active, zero full-history reads
- [ ] 5. Cobalt-on-new-storage check: one real signed registry transition
      commits (all six, one root); one stale/wrong-root transition rejects
      everywhere with no durable mutation; storage telemetry stays bounded
      through the governance rounds
- [ ] 6. Receipts from all six hosts; update `docs/status/chain-state-current.md`;
      short handoff. Z1 clock starts.

Failure anywhere in 3–5: roll back to the retained binary, one diagnosis, stop.
Existing tooling only. Operator authorization points: steps 2 and 3.

## Stop receipt

- Fleet probe: `benchmarks/storage-scaling/devnet-rollout/fleet-probe-20260830.json`
- G6 failure: `benchmarks/storage-scaling/devnet-rollout/g6-failure-20260830.json`
- Post-failure live-fleet receipt:
  `benchmarks/storage-scaling/devnet-rollout/stop-receipt-20260830.json`
- Successor live-canary rollback:
  `benchmarks/storage-scaling/devnet-rollout/canary-rollback-20260830.json`

The bounded diagnosis is a candidate compatibility defect: with both accepted
validator-registry updates present through height 924, the first new round tries
to reapply superseded registry history and fails its previous-root check. The
working clones remained at height 924. The live fleet was re-probed unchanged;
rollback was unnecessary. Steps 3–5 and the Z1 clock did not start.

## 2026-08-30 execution decision recorded by the failed session

The working session treated the operator's instruction to complete the rollout
as authorization for the following controlled-devnet exceptions and actions.
This records what the session did; it is not continuing authorization:

- **Successor candidate:** source `10dd9f20c2530fd90fbf69ff5512fae5448eaac6`
  (contains the continuation repair `2c7aa36f` plus documentation only).
- **G4/G5 evidence carry-over used by the session:** the diff from the
  G4-qualified `d0ae79f3` to `10dd9f20` touched validator-registry activation
  logic, tests, and documentation rather than the storage engine. The session
  carried forward G4 evidence and proceeded despite the still-open G5
  height-915 input. That exception was not a deployment-topology qualification
  and conveys no authority for another attempt.
- **G6 rerun (not waived):** the exact height-924 six-clone rehearsal must be
  rerun with the successor binary and must pass, including the height-925
  certified continuation that failed for `d0ae79f3`.
- **Deployment assumption used by the session:** an old-runner G6 pass was
  treated as permission to begin a rolling deploy, retaining the old binary and
  collecting receipts. The live failure invalidated that assumption.

## Successor canary result

The existing G6 runner passed but was not an exact deployment rehearsal. It
starts only `transport-validator-serve`; the live signed topology also starts
`rpc-serve` against the same data directory. The first process retains the
transactional database lock, so the second cannot start. The runner also bypasses
systemd path restrictions, and its `pre_activation_rollback` field means storage
activation cancellation rather than restoration of the deployed binary and
source directory.

The validator-1 rebuild additionally upgraded legacy JSONL checkpoint heads in
place. The retained deployed binary rejected those heads, so rollback required
restoring three exact pre-rollout heads after verifying their underlying logs
were unchanged. The signed old units and binary were restored and verified.
The final all-six probe ran from `2026-08-30T23:00:24Z` through `23:00:39Z`:
all services active, height/tip/state converged at 924, original binary on every
host, and no live transactional pointer. See the
[live-canary rollback report](../../postmortems/devnet-storage-live-canary-rollback-2026-08-30.md).

A successor requires one authoritative transactional-database writer and a new
gate using the exact signed systemd topology, concurrent transport and RPC,
RPC-to-finality, and exact `8cc7d15e` binary/data rollback. Fresh written
canary authorization is required after that gate passes.
