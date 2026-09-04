# Task Node UNL fixture shadow derivation

**SHADOW_ONLY — no live authority, registry write, transaction, signable delta, or ratification.**

Report hash: `b22688a1b085a4652ee1408f2eeebdb27a8e4d56c571ee2385c9c7dbdca817ce`

## Proposed change

- Add `validator-22` — eligible_admission_candidate; all_gates_passed; selected_by_canonical_order; churn_guard_allow.

## Holds

- `validator-23` — missing_accountability; missing_required_evidence; work_digest_signature_verification_failed.
- `validator-25` — cluster_seat_cap_exceeded.
- `validator-26` — connectivity_below_floor.

## Rejections

- `validator-24` — accountability_below_floor.

## Churn guard

- Verdict: `allow`.
- One-round overlap: 95.2%.
- Two-round overlap: 86.4%.
