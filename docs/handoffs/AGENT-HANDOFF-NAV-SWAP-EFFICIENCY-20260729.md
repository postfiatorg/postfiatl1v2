# Agent Handoff: NAV Swap Efficiency and A666 Acceptance Continuation

**As of:** 2026-07-29 UTC
**Prior context:** review of
`docs/status/A666-PRIVATE-ROUNDTRIP-HANDOFF-20260728.md` plus deep
latency research on the private and transparent NAV swap round trips.
**All execution is stopped.** Nothing in this session touched PFTL,
Ethereum, validators, keys, or the stopped Phase 9 deposit.

## What this session produced

1. **Independent review of the round-trip handoff**, appended to the
   handoff doc itself under "Review comments (2026-07-28)":
   `docs/status/A666-PRIVATE-ROUNDTRIP-HANDOFF-20260728.md`.
   Every verifiable claim was cross-checked against evidence and
   matched. Verdict: accurate handoff, no fund-risk blockers.
2. **Latency research report** with a full stage-by-stage decomposition
   of the Phase 8 run and tiered recommendations:
   `docs/plans/NAV-SWAP-EFFICIENCY-RESEARCH-20260729.md`.

Both files are **written but uncommitted**. Review and commit them (no
secrets; text only).

## Key research findings (read the full report before acting)

- The SLO misses are **not finality-dominated**. The two dominant costs
  in the `2,688s` redemption leg are two **cold Halo2 proving-key
  builds** (~`560s` and ~`510s`) inside one-shot
  `asset-orchard-*-create` CLI invocations on validator-2. Hot prove is
  `5.8s`; cold `pk_build` is ~`341s` (see
  `docs/status/zk-prover-optimization-results.md`). The pinned-VK work
  fixed the verifier side only; `ASSET_ORCHARD_*_PROVING_KEY` in
  `crates/privacy_orchard/src/asset_orchard_circuit.rs` is a
  process-local `OnceLock` that dies with each process.
- Six-validator checkpoint votes are collected **serially over ssh**
  (~3.2 min per checkpoint round, two rounds).
- SP1 Groth16 wraps cost ~3 min each (two per round trip), fully
  serialized with everything else.
- Ethereum two-epoch finality (~13 min) is real but only ~`780s` of the
  `1,848s` issue leg; ~4 min of witness/proof work is serialized after
  it instead of overlapped. 3SF is not deployed; assume the floor
  holds.
- **Projection:** Tier 0 fixes alone (resident/prewarmed prover,
  parallel vote fan-out, overlap work into the finality window, ssh
  multiplexing) bring issue to ~`1,300-1,400s` and redemption to
  ~`800-1,000s` — both inside the `1,500s` SLO with no consensus,
  circuit, or gate change.

## Repository state

- `main` head: `0064bb3` (pushed). Working tree has:
  - modified: `docs/status/A666-PRIVATE-ROUNDTRIP-HANDOFF-20260728.md`
    (review section, this session);
  - new: `docs/plans/NAV-SWAP-EFFICIENCY-RESEARCH-20260729.md`
    (this session);
  - untracked (pre-existing, **do not delete or bulk-add**): Phase 9
    deposit evidence under
    `docs/evidence/a666-acceptance-20260728/phase-9-private-redeem-hands-off-verify/`
    (`deposit/`, `ingress/`, `pftl/`) and ~4.5 GB of deployment
    artifacts under `deployments/a666-mainnet-20260727/`.

## Live-state constraints (unchanged from the round-trip handoff)

- PFTL is frozen at height `410`, six validators converged, mempool
  empty, supply invariant valid.
- A live `1.005 USDC` Phase 9 deposit
  (`0x88f4c9ff...`, block `25,633,383`, vault `0xaaa78FdA...`) is
  recorded on Ethereum and **unresolved**. Never create a second issue
  attempt against it. Resume its exact lineage or use an audited
  recovery path — nothing else.
- Private note seeds/openings/spending keys live on validator-2 only,
  mode `0600`, and must never enter evidence or this repo.

## Recommended next actions, in order

1. **Commit this session's two documents** (review section + research
   report).
2. **Commit the Phase 9 deposit evidence** and archive the untracked
   `deployments/` artifacts out-of-band. This is the only real hazard:
   a working-tree wipe orphans a live deposit's lineage.
3. **Implement Tier 0 from the research report** (0.1 resident/
   prewarmed prover, 0.2 parallel vote fan-out, 0.3 finality-window
   overlap, 0.4 ssh multiplexing) plus automated stage timestamps
   (Tier 1.4). These are orchestration-only and keep the
   frozen-release requirement satisfiable.
4. **Resume the existing Phase 9 deposit lineage hands-off** under one
   frozen release. Do not start a fresh deposit.
5. If the rerun fails on timing alone, decide the gate question
   explicitly (research report Tier 2.2 vs 2.3) instead of rerunning
   unchanged.

## Traps for the next agent

- The orchestration scripts are pinned to this run's constants
  (amounts `1000000`/`999500`, holder address, asset IDs, egress policy
  hash, `log_index=1`). Bare `test`/`jq -e` under `set -e` fail with
  exit codes only — a mismatched resume dies silently.
- Script defaults read signing keys from `/home/postfiat/tmp/...`
  paths; confirm those files still exist before any run, and do not
  copy key material anywhere new.
- `run-manifest.json` pins `orchestration_commit: 3a5b970`, one
  evidence-only commit behind head. This is expected; do not "fix" it.
- The Phase 8 `FAIL / functional_pass: true` verdict is correct as
  recorded. Do not relabel it; a fresh hands-off run is the only path
  to a formal A8 pass.

## Primary references

- Round-trip status + review:
  `docs/status/A666-PRIVATE-ROUNDTRIP-HANDOFF-20260728.md`
- Latency research (this session):
  `docs/plans/NAV-SWAP-EFFICIENCY-RESEARCH-20260729.md`
- Acceptance spec:
  `docs/plans/A666-TRANSPARENT-PRIVATE-ISSUE-REDEEM-ACCEPTANCE-SPEC-20260728.md`
- Phase 8 verdict + defect ledger:
  `docs/evidence/a666-acceptance-20260728/phase-8-private-redeem-verify/`
- Prior perf precedent (verifier side):
  `docs/plans/private-egress-consensus-performance-plan.md`
- Prover benchmarks:
  `docs/status/zk-prover-optimization-results.md`
