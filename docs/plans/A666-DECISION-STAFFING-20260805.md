# A666 Decision Record — Single-Operator Staffing

- **Status:** DECIDED
- **Date:** 2026-08-05
- **Authority:** Principal via PFTerminal session on 2026-08-05.
- **Tracker evidence:** `docs/plans/A666-RECOVERY-EXECUTION-TRACKER-20260804.md`,
  Phase 3 staffing decision.
- **Operating-control source:** `docs/plans/A666-PATH-TO-ACTUALLY-GOOD-20260804.md`,
  §4 and §5.

## Decision

The Principal decided that **single operator plus automation** is the Phase 3
staffing accepted-risk shape.

Principal directive, recorded verbatim: **"no rotation. i am the only employee now."**

This record satisfies the tracker alternative “roles assigned — or signed
accepted-risk record.” It does not assign, name, or represent any additional
staff or role
(`A666-RECOVERY-EXECUTION-TRACKER-20260804.md`, Phase 3 staffing decision;
Principal directive above).

## Accepted risk and rationale

- **Accepted risk:** Phase 3 proceeds under the documented single-operator plus
  automation shape rather than a filled roles table. This is limited to the
  staffing decision alternative
  (`A666-RECOVERY-EXECUTION-TRACKER-20260804.md`, Phase 3 staffing decision).
- **Rationale:** the only recorded rationale is the Principal directive quoted
  above. No other staffing, access, or personnel claim is made.
- **Gate effect:** this satisfies the staffing-decision alternative only. The
  Phase 3 checkbox stays open until its operational work executes
  (`A666-RECOVERY-EXECUTION-TRACKER-20260804.md`, Phase 3;
  `A666-PATH-TO-ACTUALLY-GOOD-20260804.md`, §5).

## Compensating control

The §4 single-authority rule applies: automation may prepare and validate, but
cannot self-authorize a live mutation; every live step executes only after the
Principal confirms that step's preflight report hash
(`A666-PATH-TO-ACTUALLY-GOOD-20260804.md`, §4). This decision leaves
credential values, funds, data, and configuration untouched (Principal
directive; recovery-spec non-negotiable constraints in
`A666-PRODUCT-RECOVERY-AND-DEMO-RELAUNCH-SPEC-20260803.md`, §5).

## Scope and non-actions

- This record does not execute a live step, alter a credential, or change a
  fund, data, or configuration boundary (Principal directive; recovery spec
  §5 and §14).
- This record does not select a demo date, approve a canary, or confirm a
  preflight hash. Those decisions remain separately controlled by the tracker
  (`A666-RECOVERY-EXECUTION-TRACKER-20260804.md`, R7-R9).
- StakeHub remains a product indefinitely; its public-stack authority
  decoupling and the official-window stop/restart rule are separately recorded
  in the tracker R10 constraint
  (`A666-RECOVERY-EXECUTION-TRACKER-20260804.md`, R10).

## Sources

1. Principal directive via PFTerminal session, 2026-08-05.
2. `docs/plans/A666-RECOVERY-EXECUTION-TRACKER-20260804.md`, Phase 3 staffing
   decision.
3. `docs/plans/A666-PATH-TO-ACTUALLY-GOOD-20260804.md`, §4 and §5.
4. `docs/plans/A666-PRODUCT-RECOVERY-AND-DEMO-RELAUNCH-SPEC-20260803.md`,
   §5 and §14.
