# Task Node UNL fixture shadow derivation

**SHADOW_ONLY — no live authority, registry write, transaction, signable delta, or ratification.**

Report hash: `0fe852b32b7ef7d5e8e4ecaf2c1befc6eae43d6a30ca141d90870f02447c6132`

## Proposed change

- No change.

## Holds

- `validator-22` — binding_registry_key_join_hold; missing_required_evidence; missing_rho; wallet_account_mapping_hold.
- `validator-23` — binding_registry_key_join_hold; missing_accountability; missing_required_evidence; missing_rho; wallet_account_mapping_hold; work_digest_signature_verification_failed.
- `validator-25` — binding_registry_key_join_hold; cluster_seat_cap_exceeded; missing_required_evidence; missing_rho; wallet_account_mapping_hold.
- `validator-26` — binding_registry_key_join_hold; connectivity_below_floor; missing_required_evidence; missing_rho; wallet_account_mapping_hold.

## Rejections

- `validator-24` — accountability_below_floor; binding_registry_key_join_hold; missing_required_evidence; missing_rho; wallet_account_mapping_hold.

## Churn guard

- Verdict: `allow`.
- One-round overlap: 100.0%.
- Two-round overlap: 90.5%.
