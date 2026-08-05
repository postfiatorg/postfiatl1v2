# A666 Decision Record — Key Rotation

- **Status:** DECIDED
- **Date:** 2026-08-05
- **Authority:** Principal via PFTerminal session on 2026-08-05.
- **Tracker evidence:** `docs/plans/A666-RECOVERY-EXECUTION-TRACKER-20260804.md:207-208`.
- **Gate context:** recovery spec §9, R8 Live preflight requires a preflight
  decision record; R8 remains frozen until its full requirements pass
  (`docs/plans/A666-PRODUCT-RECOVERY-AND-DEMO-RELAUNCH-SPEC-20260803.md`,
  §9 and §14).

## Decision

The Principal decided: **no credential/key rotation**.

Principal directive, recorded verbatim: **"no rotation. i am the only employee now."**

This is the R8 tracker alternative to “rotate live operator/signer/publisher
keys, or signed accepted-risk record”; this record is the accepted-risk record
for that alternative
(`A666-RECOVERY-EXECUTION-TRACKER-20260804.md:207-208`; Principal directive
above).

## Accepted risk and rationale

- **Accepted risk:** existing credential and key material remains unrotated.
  This is limited to the explicit R8 decision alternative
  (`A666-RECOVERY-EXECUTION-TRACKER-20260804.md:207-208`).
- **Rationale:** the only recorded rationale is the Principal directive quoted
  above. This record makes no additional claim about personnel, access history,
  or credential exposure.
- **R8 effect:** the decision satisfies the key-rotation/accepted-risk
  alternative only. It does not open or close R8, and the tracker checkbox
  remains unchecked because the other R8 requirements remain
  (`A666-RECOVERY-EXECUTION-TRACKER-20260804.md:205-215`; recovery spec §9).

## Compensating controls

- Any operational reference remains a path or vault label only; no credential
  value appears in code or evidence (Principal directive; recovery spec
  §13 report/evidence exclusion).
- The Principal confirms every live R9 step after prior-step evidence; this
  decision grants no self-authorization for a live migration
  (`A666-RECOVERY-EXECUTION-TRACKER-20260804.md:219-230`; recovery spec
  §11 and §14).
- This record itself performs no live mutation. Keys, funds, data, and
  configuration remain untouched (Principal directive; recovery spec §11 and
  §14).
- R8 remains frozen until the signed recovery snapshot, all-six convergence,
  route pause, signer-separation, rollback rehearsal, and preflight-hash
  decision are all satisfied
  (`A666-RECOVERY-EXECUTION-TRACKER-20260804.md:205-215`; recovery spec §9).

## Scope and non-actions

- This is a decision record only. It does not rotate, read, export, move,
  revoke, or otherwise operate any credential or key.
- This is not a live-chain, fund, data, or configuration action.
- This does not select a demo date, assign staffing, fire a journey, or alter
  any other decision. Those remain separately controlled
  (`A666-RECOVERY-EXECUTION-TRACKER-20260804.md:203,219-230,261-263`).

## Sources

1. Principal directive via PFTerminal session, 2026-08-05.
2. `docs/plans/A666-RECOVERY-EXECUTION-TRACKER-20260804.md:205-215`.
3. `docs/plans/A666-PRODUCT-RECOVERY-AND-DEMO-RELAUNCH-SPEC-20260803.md`,
   §9, §11, §13, and §14.
