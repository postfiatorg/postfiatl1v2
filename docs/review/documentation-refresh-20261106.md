# Documentation refresh — 2026-11-06

Task Node `task_ad6c310c053eb0f3b82d86b75c455218`. Source baseline:
`4b4da1b91493e2349f431b0c94652cc0bfa98d99`, including the
[November 6 handoff](../handoffs/2026-11-06___dravlic__first_real_base_sepolia_lock_released_and_bridge_phase_2_complete.md).
Documentation only; no runtime change, testnet transaction or production probe.

## Current guidance reconciled

| Pages | Correction |
|---|---|
| `README.md`, `docs/index.md` | November 6 review and reported fleet observation; built, rehearsed and deployed states remain distinct. |
| `docs/status/network-status.md`, `chain-state-current.md` | The November 6 handoff reports the same October 30 executable at height 1168. Original rollout and readiness dates remain intact; the stale October 21 “latest” paragraph is corrected. |
| `docs/architecture/pft-token-on-base.md`, `genesis-external-lanes.md` | Local real-Lock rehearsal, export-aware native accounting, default-off native export implementation and unfinished Base export path. |
| `docs/plans/active/pft-erc20-base-listing-milestone.md` | Audit package prepared; audit still open. P2.5's remaining audit is distinct from token, treasury, genesis and deployment gates. Old skeleton and estimates are reconciled with implemented work; owner labels use plain text. |
| `docs/runbooks/public-rpc-operator-policy.md`, `validator-signer-two-user-deploy-sheet.md`, `docs/security/index.md` | Three services under two users; reviewed guard and unit generation are built, actual deployment and release qualification remain open. |
| `docs/plans/active/next-validator-release-plan-20261115.md` | All three public CI workflows are green at the exact baseline, superseding that handoff's pending-CI observation only. |

Representative wording changes:

- Before: “No lane is configured on any network.” After: a Base Sepolia lane
  was configured on the local rehearsal fleet; no lane is active on the recorded
  `postfiat-wan-devnet-2` and no mainnet lane is deployed.
- Before: “The reverse export route is designed but refused until implemented.”
  After: the native half is implemented behind `exports_enabled`, default off;
  the Base verifier deployment, export prover and relay remain open.
- Before: “Nothing in Phase 2 starts before the mainnet genesis plan exists.”
  After: production activation needs that plan and its allocation/address
  bindings; completed source work and rehearsals do not authorize activation.

## Validation and preservation

Validation uses the existing docs workflow: handoff-index check, evidence-index
generation, docs-site redaction check, whitepaper implementation-boundary check,
public-link regression and full local-link checks, and `mkdocs build --strict`.
Current site guidance links to GitHub for evidence excluded from the built site.
Changed prose was checked for contradictory status claims and italic emphasis.

The historical handoffs, locked specifications, rehearsal records, test logs,
deployment records and runtime files are unchanged. The Current State page adds
a dated observation and updates its current summary; its older dated entries
retain their original text. Existing historical emphasis is preserved there.
