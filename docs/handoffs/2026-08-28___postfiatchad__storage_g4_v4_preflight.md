# Storage G4 v4 clean preflight

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-28 UTC

## BLUF

The v3 G4 campaign reached height 5,000 but failed closed because its resource
sampler missed a foreground observation; that was an evidence-harness failure,
not a storage-candidate result. Clean v4 runner `03123ca0` fixes deterministic
sampler startup and bounded large-fleet restoration. Its real six-validator
smoke and 19 GB height-5,000 restore preflight pass. G4 still has **not** passed:
the next bounded action in the
[active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
is exactly one clean v4 qualification output with a four-hour aggregate budget.

## Current state

- Repository branch is `main`. At preflight inspection, local HEAD and
  `origin/main` were clean at `03123ca0`
  (`test(storage): make G4 sampling and fleet restore bounded`). The commit
  containing this handoff and the redaction-safe receipt is the documentation
  successor used for the clean campaign; the campaign checkpoint must bind its
  exact full revision rather than relying on this prose.
- The unchanged storage candidate is source
  `ae65844190f153cbdd49d1e5ac28ab96a19f7af4` and release node-binary SHA-256
  `891bfb42ea16af844fd72351ee38a90eaeb8f4302492a8fd64ce0f3db5dcbbf4`.
  The v4 work changes only the evidence harness and helper, not the candidate.
- The clean v4 helper embeds `03123ca0`, reports release profile, and has
  SHA-256
  `f8bb25f2df22a8337d571a404ae3d4799735978d162c086945d5ad95c6c1ca73`.
- The clean development smoke completed in 29.332 seconds. Report SHA-256 is
  `c7be9957e916bacb9d8697c6b52005c2665184ed1fa0feca74ae8712b0830710`;
  checkpoint SHA-256 is
  `a8b85cba83d1b495746e3310befde81e221b73bd313685d5d4c67b7b69380e9f`.
  It used the same candidate binary across both lanes, every measured foreground
  process had at least four samples, the selected scratch fleet mutated and
  restored to its exact starting digest, six validators converged, and no child
  process survived. This is development evidence, not a G4 qualification pass.
- The committed
  [height-5,000 preflight report](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/storage-scaling/g4c-height5000-preflight-03123ca0.json)
  has file SHA-256
  `84d5fa93c933e953b0efe1f98108bc7f2fdbb1748f1ac8396691b31a2db6e1fb`.
  It cloned 178,214 files totaling 19,040,307,767 bytes in 177.410 seconds,
  forced the incremental restore path across six 1,813,778,432-byte databases
  in 115.355 seconds, re-matched frozen fleet SHA-256
  `8a4618e7ea81df7d26c4547868d9941f712552fc5e8982c74bc8763909bccfeb`,
  preserved the source digest, removed its workspace, and exited `PASS`.
  The preflight was offline: it started no node or consensus process and made no
  network contact.
- V2 remains a preserved budget failure at height 3,050. V3 remains a preserved
  sampler failure after 13,987.118 seconds, despite completing its first 50
  high-height rounds. Neither output can be resumed or represented as a G4 pass.
- The last authenticated controlled-devnet observation remains the point-in-time
  height-924 capture from `2026-08-26T06:34:55Z`–`06:35:50Z`; see
  [Current State](../status/chain-state-current.md). The deployed node embeds
  `8cc7d15e` and has SHA-256 `d5e5ef63…2696caf`. Current repository and
  storage-candidate commits are merged but undeployed. This session performed no
  live probe, devnet query, data copy, service action, deployment, or mutation.
- No Task Node or additional agent was used. No campaign or validator process
  was alive after either preflight.

## Next decision or action

Commit and push this receipt, milestone update, and handoff. Rebuild only the
setup helper from that final clean revision, verify the candidate node hash is
unchanged, create one detached clean runner worktree at the same revision, and
start exactly one new v4 output from height 1. Cap the first unattended segment
at two hours. If it stops cleanly, verify the checkpoint and zero surviving
children, then resume only that same checksum-bound output for at most the
remaining two hours. Do not create a second v4 output and do not modify either
failed output.

If the final report and independent verifier pass every G4 requirement, bind the
existing redaction-safe G2 receipts and the new G4 material for the locally
available G5 packet. Do not claim `OFFLINE QUALIFIED` until the exact height-924
replay is performed from a separately authorized read-only copy and the complete
packet verifies. G6 remains deferred and separately authorization-bound.

## References

- [Active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Clean v4 implementation](https://github.com/postfiatorg/postfiatl1v2/tree/03123ca0/benchmarks/storage-scaling)
- [Storage scaling fix specification](../architecture/storage-scaling-fix-spec.md)
- [Locked storage scaling research specification](../architecture/storage-scaling-research-spec.md)
- [State and storage architecture](../architecture/state-and-storage.md)
- [Current State](../status/chain-state-current.md)
- [V3 remediation handoff, now historical](2026-08-28___postfiatchad__storage_g4_timeout_and_persistent_remediation.md)
