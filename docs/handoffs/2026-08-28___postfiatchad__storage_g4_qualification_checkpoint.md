# Storage G4 qualification checkpoint

> **Historical checkpoint:** this handoff was superseded after the original
> four-hour budget expired. Resume from the
> [G4 timeout and persistent-remediation handoff](2026-08-28___postfiatchad__storage_g4_timeout_and_persistent_remediation.md),
> not from the commands below.

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-28 UTC

## BLUF

The [active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
still blocks public testnet. G4A is complete on runner implementation
`f7b3d21d`, and the one authorized clean G4 campaign crossed the old
high-height snapshot failure and durably reached height 1,550 with zero
full-history reads. It was normally interrupted for this operator handoff, has
no final report, and is not qualification evidence yet. Resume this exact
checkpoint from the pinned `8768866a` worktree; do not start a new campaign or
reset its four-hour budget.

## Current state

- The working branch was clean `main` at
  `8768866aeb52e8e593e3a4055c9714ae642244a8` when the campaign started.
  Runner implementation is `f7b3d21d`; this handoff and the adjacent milestone
  update are documentation-only descendants and are not part of the runner
  checkout.
- The selected candidate remains source
  `ae65844190f153cbdd49d1e5ac28ab96a19f7af4` with release-binary SHA-256
  `891bfb42ea16af844fd72351ee38a90eaeb8f4302492a8fd64ce0f3db5dcbbf4`.
  Candidate source and binary did not change during G4A.
- G4A removed all selected high-height portable-snapshot import/export. High
  advances and windows use byte-verified prepared fleets; signed-corpus creation
  uses a disposable canonical clone whose starting digest must equal the frozen
  source, whose before/after hashes and sequence range are bound, and which is
  discarded before a pristine measurement clone is restored.
- Fifty-nine focused Python tests passed. The clean G4A six-validator smoke and
  stop/tamper/resume proof are recorded in the milestone. Documentation link,
  redaction, and site-build checks passed before the release campaign.
- Campaign output is
  `/home/postfiatchad/repos/postfiat-storage-g4-8768866a-ae658441-v1`.
  It contains private local seed/key material and generated node data; do not
  commit or copy it into a packet.
- Checkpoint
  `campaign-checkpoint.json` has SHA-256
  `79094159a091f306e2cf95b52cdf4dcba85bf6f9cecc0694abc9d0431a331889`.
  It started at `2026-08-27T23:54:36Z`, was interrupted at
  `2026-08-28T01:15:33Z`, records status `INTERRUPTED`, and has consumed
  4,857.029 of the original 14,400 seconds. Exactly 9,542.971 seconds remain.
- Complete durable work: height 1→50 advance; five selected height-50 windows;
  and snapshot-free height 50→1,550 advance. The latter's receipt SHA-256 is
  `ab11cae04a86b940c9117a5a379736f8e2c38c023bcda11b19f0626fc56e951b`.
  It reports final height 1,550, six-validator convergence, literal accepted
  receipts, zero full-history reads, and a passing backend-work gate.
- The durable height-1,550 prepared-fleet SHA-256 is
  `1edb40d3808f8b06f0f96ad8537e8eb0d6b12566ce46d12cf622cee138816648`.
  Both current and result snapshot fields are null. This crosses the exact
  boundary where the old runner failed while importing a 317 MiB
  `blocks.json`; no size cap was raised.
- The partial height 1,550→3,050 unit reached roughly round 243 before the normal
  interrupt. It is not evidence. `--resume` will verify every frozen binding,
  quarantine that partial unit, and rerun the whole 1,500-round unit.
- No campaign, benchmark, or local validator process survived the stop. There is
  no `campaign-report.json`, so G4 is not passed and no G5 packet may claim the
  run.
- A clean detached runner worktree already exists at
  `/home/postfiatchad/repos/postfiat-storage-runner-8768866a`. It is required
  because the checkpoint deliberately refuses any changed checkout revision;
  post-handoff `main` must not be used to resume it.
- No Task Node or subagent was used. No controlled-devnet query, copy, service
  action, deployment, or mutation occurred.
- The last observed fleet state remains the point-in-time height-924 observation
  from `2026-08-26T06:34:55Z`–`06:35:50Z`, with deployed source
  `8cc7d15e` and node binary SHA-256 `d5e5ef63…c2696caf`; see
  [Current State](../status/chain-state-current.md). This session made no fresh
  live claim.

## Next decision or action

Resume the same campaign only. First confirm the pinned worktree is clean, the
candidate binary hash is unchanged, and no campaign process is alive:

```bash
git -C /home/postfiatchad/repos/postfiat-storage-runner-8768866a status --porcelain
sha256sum /home/postfiatchad/repos/postfiatl1v2/target/release/postfiat-node
pgrep -af '[r]un_paired_campaign.py|[r]un_campaign.py|[p]ostfiat-node'
```

Then actively monitor this exact resume with the remaining aggregate hard cap:

```bash
cd /home/postfiatchad/repos/postfiat-storage-runner-8768866a
timeout --signal=INT --kill-after=120s 9540s \
  python3 -u benchmarks/storage-scaling/run_paired_campaign.py \
  --node-bin /home/postfiatchad/repos/postfiatl1v2/target/release/postfiat-node \
  --expected-source-revision ae65844190f153cbdd49d1e5ac28ab96a19f7af4 \
  --output-dir /home/postfiatchad/repos/postfiat-storage-g4-8768866a-ae658441-v1 \
  --resume
```

Do not pull, edit, or commit in the pinned worktree. Do not create a fresh
output. If the campaign completes `PASS`, independently inspect the final
report, close the G4 checkboxes, and assemble the locally available G2/G4
packet material. If the remaining budget expires or any selected-path gate
fails, record the real G4 failure and stop; do not grant another run
automatically. Exact height-924 replay still needs a separately authorized
read-only validator-directory copy, and G6 still needs separate authorization
for six stopped copies.

## References

- [Active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Storage-scaling evidence workflow](https://github.com/postfiatorg/postfiatl1v2/tree/f7b3d21d/benchmarks/storage-scaling)
- [Storage scaling fix specification](../architecture/storage-scaling-fix-spec.md)
- [Locked storage scaling research specification](../architecture/storage-scaling-research-spec.md)
- [State and storage architecture](../architecture/state-and-storage.md)
- [Independent storage candidate review](2026-08-27___dravlic__storage_candidate_review.md)
- [Current State](../status/chain-state-current.md)
