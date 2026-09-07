# Task Node UNL fixture shadow derivation

**SHADOW_ONLY — no live authority, registry write, transaction, signable delta, or ratification.**

Report hash: `aa46b806576557c02d2273a571c53cd1889a0f3f9c29ef3ff1f32b3f80d0ab80`

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
