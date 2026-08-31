# Devnet storage canary rollback

- **Operator:** PostFiatChad (`postfiatchad`)
- **Date:** 2026-08-30 UTC

## BLUF

Successor storage candidate `10dd9f20` passed the existing G6 clone runner but
failed the first live validator canary because the runner did not reproduce the
deployed concurrent transport/RPC process topology. The rollout stopped after
validator-1 and exact binary-plus-data rollback completed. All six validators
were observed healthy and converged at height 924 through
`2026-08-30T23:00:39Z`. See the
[live-canary postmortem](../postmortems/devnet-storage-live-canary-rollback-2026-08-30.md)
and [Current State](../status/chain-state-current.md).

## Current state

- Candidate `10dd9f20`, binary `0cc664a3…ad4183`, is **DO NOT DEPLOY**.
- The G6 helper started only `transport-validator-serve`; production also
  starts `rpc-serve` against the same transactional generation.
- The first process retained the exclusive `redb` database handle, so the
  second process could not become ready. Service start order does not solve
  single-writer ownership.
- The writable rebuild also upgraded three authenticated JSONL heads in the
  source directory. The exact deployed `8cc7d15e` binary rejected those heads,
  so rollback required verified restoration of the original head files as well
  as the old signed units and binary.
- The authenticated recovery probe found all validator, RPC, and advisory
  shadow services active on the original binary, with one height-924 tip/state,
  empty mempools, and no live transactional pointer.
- No block, storage activation, governance action, Cobalt transition, or Z1
  observation period occurred.
- The session also carried predecessor evidence forward while G5 still had an
  open height-915 input. That execution decision conveys no future authority.
- Repository `main` is ahead of the deployed lineage; no successor code is
  installed on the fleet.

## Next decision or action

Repair the architecture around one authoritative transactional-database writer,
then replace G6 with a deployment-exact gate that installs the signed systemd
topology, co-starts transport and RPC in both orders, drives RPC-to-finality,
and restores the exact deployed binary, units, and every source-side mutation.
Repeat the six-clone gate and obtain fresh written canary authorization. Do not
resume the rollout from validator-2.

## References

- [Live-canary postmortem](../postmortems/devnet-storage-live-canary-rollback-2026-08-30.md)
- [Redaction-safe rollback receipt](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/storage-scaling/devnet-rollout/canary-rollback-20260830.json)
- [Active rollout plan](../plans/active/devnet-storage-rollout-plan.md)
- [Storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Current State](../status/chain-state-current.md)
