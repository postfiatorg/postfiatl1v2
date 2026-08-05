# A666 Decision Record — Live-Demo Control Truth

- **Status:** DECIDED
- **Date:** 2026-08-05
- **Authority:** principal directive via supervising chain, 2026-08-05.
- **Tracker context:** `docs/plans/A666-RECOVERY-EXECUTION-TRACKER-20260804.md`,
  2026-08-05 control-truth addendum.

## Decided control truths

1. **Route.** The target is the already-deployed literal A666 route
   `pftl-a666-ethereum-wA666-usdc-v1`. It must never be relabeled as
   `a651`.
2. **Objective.** The end state is a real full E2E loop using LIVE StakeHub
   funds. Anvil and synthetic assets are excluded from this objective.
3. **Principal and cap.** The principal is **10,000,000 atoms / 10.000000
   USDC**. The **530 USDC** cap remains unchanged and must never be raised or
   bypassed.
4. **Sequencing.** Implementation and qualification proceed now. Live fire
   occurs later only under per-leg HELD-packet approval. The end-state statement
   is not a per-leg fire command.

StakeHub remains a product. Its five services and its funds are preserved. This
record authorizes neither a service change nor a live-chain or fund action.

## Dirty-evidence classification

Commit `a037446` records the two post-v6 dirty evidence files as benign
runtime state:

- `environment-manifest.json` is preserved as the explicit RED R4-pass1
  attempt record. It is not silently restored to the earlier green-shaped
  manifest.
- `private-swap-dependencies.json` is a benign runtime refresh.

The classification is supported by valid JSON, runtime timestamps, the
controlled RED failure record, and refreshed private-swap process/height state.
It is not evidence of a completed R4 journey.

### Run-directory retention gap

The private-swap manifest records
`run-20260804T043154Z-869163`, but the run directory was absent under
`docs/evidence/a666-public-reserve-product-20260803/` when verified. The
runtime reference therefore has a retention gap. This record preserves that
limitation; it does not reconstruct or fabricate the missing directory.

## R3 artifact self-containment defect

`docs/evidence/a666-public-reserve-product-20260803/qualification/r3-repeatability-gate.json`
has no `commit` field. The verified R3 lineage commit is `5bfe466`
(`Pass gate R3: three consecutive clean lifecycle runs on the exact commit`).
The historical artifact is immutable and is not edited by this decision record.

## Ceremonies-70 provenance gap

The claim of **70 ceremonies** appears in
`docs/handoffs/A666-RECOVERY-AND-LIVE-DEMO-HANDOFF-20260805.md` and
`docs/reports/A666-STAKEHUB-PNL-REPORT-20260805.md`. The required search
found no `ceremonies` entry in
`/home/postfiat/repos/StakeHub-repeat-demo/data/` or in the A666 live-demo
evidence directory. Primary evidence for the count is therefore an evidence
gap; this record does not infer or create it.

## Scope and non-actions

- This record changes no credentials, services, configuration, funds, or
  live-chain state.
- No HELD packet, preflight report hash, or principal approval for an individual
  live leg is supplied here.
- The 40 tracker checkbox states remain unchanged.

## Sources

1. Principal directive via supervising chain, 2026-08-05.
2. Commit `a037446`.
3. Commit `5bfe466`.
4. `docs/plans/A666-RECOVERY-EXECUTION-TRACKER-20260804.md`.
5. `docs/handoffs/A666-RECOVERY-AND-LIVE-DEMO-HANDOFF-20260805.md`.
6. `docs/reports/A666-STAKEHUB-PNL-REPORT-20260805.md`.
