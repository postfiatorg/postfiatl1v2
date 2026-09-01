# L1v2 Public Testnet Path

**Status:** Active — sequencing document; authorizes no deployment, devnet mutation, or launch
**Date:** 2026-08-30
**Task:** Task Node `task_510e7605cb2dff0dfd672b397d26f2a6`
**Basis:** [Storage scaling milestone](storage-scaling-milestone.md), [G4 first-pass handoff](../../handoffs/2026-08-30___postfiatchad__g4_first_pass.md), [Cobalt further evaluation](https://postfiat.org/posts/cobalt-further-evaluation/), Dynamic UNL roadmap (`dynamic-unl-scoring/docs/CurrentRoadmap.md`)

Each lettered gate that changes chain, fleet, or authority state requires its
own operator decision; this plan orders the work and names the decision
points, nothing more.

## Gate Zero — operator preconditions (recorded 2026-08-30)

**No community-facing step — genesis outreach (C3 execution), ratification
clients on operator machines (C4 deployment), any Phase D gate, or launch —
may start until all three are true.** Internal work (A, B, C1, C2, E, and
design-only portions of C3/C4) proceeds normally. Bars below are proposed
operationalizations; the operator may tighten or restate them.

- [ ] Z1 **Cobalt is live**: Cobalt-ratified transitions are the sole
      registry-change path on the running devnet with the qualified storage
      lineage (post-B3), and have processed real transitions — including at
      least one validator change — over a sustained window with zero
      overrides
- [ ] Z2 **AI-governance decision made and live**: the C2 shadow evaluation
      is complete, the model-authority decision (keep or demote) is recorded,
      and the decided configuration is what actually runs the fork's weekly
      production rounds
- [ ] Z3 **NAVCoin swaps work end to end**: full round trips
      (deposit → mint → swap → redemption) succeed repeatedly on the
      qualified lineage over a sustained window **without requiring any
      consensus upgrade during that window** — consensus-surface churn on
      NAVCoin paths must have stopped before strangers run the binary

## A — Finish offline storage qualification (G3 → G5)

- [x] G4 scaling PASS at candidate `d0ae79f3`, binary `9e82d928…8c80c`, report `e2cff9cd…999f` ([handoff](../../handoffs/2026-08-30___postfiatchad__g4_first_pass.md))
- [ ] A1 *(operator, external)* Re-supply the height-915 quarantine archive to the qualification host; record its source-tree SHA-256 against the prior receipt (`postfiat-storage-g3-ae658441/replay-915/receipts/height-915.json`)
- [ ] A2 *(operator, external)* Name the height-924 custodian and authorize one read-only copy of a quiescent validator directory (`docs/status/chain-state-current.md` identities)
- [ ] A3 Run both exact replays against the frozen binary with `benchmarks/storage-scaling/run_replay_evidence.py`; bind receipts
- [ ] A4 Assemble the redaction-safe packet with `benchmarks/storage-scaling/package_packet.py`; run the offline verifier; milestone state becomes **OFFLINE QUALIFIED**
- [ ] A5 Retire [storage-scaling-milestone.md](storage-scaling-milestone.md) into `docs/plans/completed/` and refresh `docs/architecture/state-and-storage.md` concisely

## B — Pre-deployment rehearsal and deployment decision (G6)

- [x] B1 *(operator)* Authorized and captured six distinct stopped validator-directory copies from the controlled devnet fleet (2026-08-30)
- [ ] B2 Run the six-clone migration rehearsal (`benchmarks/storage-scaling/run_migration_rehearsal.py`): **FAIL 2026-08-30** after all six exact height-924 rebuild/verify passes; the first height-925 round rejected superseded validator-registry history reapplication
- [ ] B3 *(operator decision)* Deploy a qualified lineage to the controlled devnet with pinned source/binary/data/activation/rollback identities; **blocked — do not deploy `d0ae79f3`**; verify any successor with a fleet receipt (`docs/status/chain-state-current.md` update)

## C — Validator story: fork community feeds the l1v2 registry

The PFT Ledger fork (51+ community validators, Dynamic UNL live) supplies the
operator population; l1v2 supplies the registry-as-protocol-state destination.

- [ ] C1 Complete Dynamic UNL governance verification G.6/G.7 and Evidence Transparency E.1 on the fork (`dynamic-unl-scoring/docs/CurrentRoadmap.md`)
- [x] C2 Run the deterministic sub-scorer shadow evaluation against frozen rounds 12–19 (per `dynamic-unl-scoring/docs/DeterministicFinalScore.md` method): complete 2026-09-01, all eight rounds — 2 cutoff flips (both stricter), UNL overlap 19–20/20 ([results note](../../governance/dunl-subscorer-shadow-eval-20260901.md), `benchmarks/ai-governance/dunl-subscorer-shadow-20260901/`); the *(operator decision)* keep or demote model authority remains open and is tracked by Z2
- [x] C3 Design the l1v2 genesis-registry proposal path: scored fork operators → proposed registry + template trust graph → Cobalt-checked ratification (Dynamic UNL Phase 3A/3B; `crates/consensus_cobalt/`, `docs/architecture/overview.md`): designed in [genesis-registry-proposal-path.md](../../architecture/genesis-registry-proposal-path.md) (2026-09-01, harness 88.93) — source artifacts, ML-DSA identity binding, proposed-registry schema and content hash, Cobalt-checked path with DGA bounds, verification story, `SHADOW_ONLY` boundaries, and work sequence; design only, execution stays Gate Zero-blocked
- [ ] C4 Extend the validator sidecar into the l1v2 ratification client (commit-reveal signature after deterministic round replay; `validator-scoring-sidecar`)

## D — Public-testnet eligibility gates

- [x] D1 Inventory the non-storage release gates: `SECURITY.md`, `docs/release-process.md`, open security items (`docs/security/`): inventoried in [`docs/status/release-gate-inventory.md`](../../status/release-gate-inventory.md) (21 gates: 4 DONE, 16 OPEN, 1 UNKNOWN)
- [x] D2 Public operator runbook: join, key custody (ML-DSA), sidecar, monitoring (`docs/runbooks/`, fork operator docs as template): published [`docs/runbooks/public-operator-runbook.md`](../../runbooks/public-operator-runbook.md) (2026-09-01) — seven operator journeys, commands verified against the current tree; steps an outsider cannot yet perform carry boxed gap notes naming the blocking inventory rows (3 custody, 6–8/10 per-release, 17 launch authority, 18 ops readiness) and milestone items (C3/C4, Gate Zero)
- [x] D3 Topology/independence thresholds for launch, reusing the fork's strict-gate machinery (placement preflight, concentration caps): proposed in [launch-topology-thresholds.md](../../architecture/launch-topology-thresholds.md) (2026-09-01) — concrete numbers for all five dimensions with cited fork rules, existing vs `new:` preflights named (placement preflight and L3 independence verifier are `new:`); every threshold awaits the operator's confirmation
- [ ] D4 *(operator decision)* Public-testnet launch — explicitly outside this plan's authority

## E — Mandate deliverables

- [x] E1 Python CLI: `testnet-path` status tool reading gate states from this document and the milestone registry; human-runnable (`python/postfiat_rpc/testnet_path.py`, tests `python/tests/test_testnet_path_status.py`; run `PYTHONPATH=python python3 -m postfiat_rpc.testnet_path`, with `--json` and `--markdown`)
- [x] E2 User-facing interface consuming the CLI output (docs/status page): generated [`docs/status/testnet-path.md`](../../status/testnet-path.md) via `--markdown`, listed in the mkdocs nav next to the current-state page
- [ ] E3 On completion, retire this document into `docs/plans/completed/` and refresh documentation

## Order and dependencies

A1/A2 are independent external inputs; A3–A5 follow either. B needs A
complete. C1/C2 and design work run in parallel with A and B. **Everything
that touches community operators is blocked by Gate Zero (Z1–Z3).** D needs
B, C, and Gate Zero. E1/E2 can start immediately.
