# Task Node UNL V2 shadow comparison

**SHADOW_ONLY — decision support only; no submission, live mutation, or promotion is available.**

Overall: `SHADOW_ONLY_WITH_UNRESOLVED_IDENTITY_LIMITS`. This is not an all-green status.

Version: `2` · Policy: `tasknode-unl-admission-v2` · Report root: `b7742613b99021988205e3b8a3f2d127b90d33f1b0ed1b495cdd0d790ea198fc`

## Side-by-side verdicts

- `validator-bob-rotation: V1 HOLD (account_already_seated,control_epoch_changed) | V2 HOLD (ACCOUNT_ALREADY_SEATED,DECLARED_CONTROL_GROUP_EXISTING_BREACH,HOLD_CONTINUITY,SOCIAL_CLUSTER_SATURATED)`
- `validator-carol: V1 HOLD (connectivity_below_floor) | V2 HOLD (CONNECTIVITY_BELOW_FLOOR)`

## HOLD_CONTINUITY

- `validator-bob-rotation` / `account-bob` — ACCOUNT_ALREADY_SEATED,DECLARED_CONTROL_GROUP_EXISTING_BREACH,HOLD_CONTINUITY,SOCIAL_CLUSTER_SATURATED; report `1ec023e041aaf1caab199a3b673b6fb1c98340fa4e00712d7c37c5ddd64bee88`

## Admission denials

- `validator-bob-rotation` — `HOLD`: ACCOUNT_ALREADY_SEATED,DECLARED_CONTROL_GROUP_EXISTING_BREACH,HOLD_CONTINUITY,SOCIAL_CLUSTER_SATURATED; report `1ec023e041aaf1caab199a3b673b6fb1c98340fa4e00712d7c37c5ddd64bee88`
- `validator-carol` — `HOLD`: CONNECTIVITY_BELOW_FLOOR; report `84368900e393c76068494a2109cd4b95f4a769d5dbc68d4bb9c39b5efcc5f3cd`

## EXISTING_BREACH

- `DECLARED_CONTROL_GROUP` `ee393c09ccd2a8e241b20f7854bd82a881ecb6eea607494ff635346a794aa5e4`: 1 excess seat(s), review `FULL_WINDOW_REVIEW`, evidence 940eec1cd9bc021101d5d088773a80ab36583048411338240289677dba0fd05a

## Known control and evidence limits

- An unchanged-key control transfer is undetectable by these public inputs.
- A valid custody signature proves key control, not personhood or continuity of a human operator.
- Funding observations are audit-only and cannot create admission trust, control-group membership, or a veto.

A valid custody signature is not a personhood claim. An unchanged-key account sale or other control transfer is not detectable from these public inputs. Funding remains audit-only: it creates neither positive trust mass nor a unilateral veto.

## Bound roots

- `admission_policy_root`: `499c56b8dbb8804392b05aef6607c66e139ac82dec4e72bb63edc6c8420cdcd2`
- `cli_input_root`: `4bf42f811239baac280d4ee6674f5a43690a9c906fb85caadf258b2f09726dcf`
- `control_registry_root`: `28d80fb8c1017f34eb5d3e1082a7e0e5ee89ec0eea890496b676b3107f8afd2a`
- `declaration_root`: `4b0fae6b8275eb21e45b173dc316cf49dc307c01df98cff3eb90e5dc56ef0349`
- `evidence_input_root`: `411a5f1937aafed44ca3bfe1f60ce7da60ca27d90f05b8ce3c556652b8305b3a`
- `evidence_policy_root`: `1b697d1302a1a21bb576cdeac59c2a20fba8aa4ac4f3b311ebbe5ea56f3cda76`
- `frozen_window_root`: `3d88e653a8c4abfac40271149a8745028ead6cba1b78b003afa1d022d07712e6`
- `graph_root`: `cd3989ecd72f763c1b2b15d67a0accf8485329fca3885a60be77166eeb9133e0`
- `v1_reference_root`: `b0be5a7ccb2d744ac73225a474246fa36cb613c9344d29be5ebabd3966804fa4`
- `validator_registry_root`: `4a705ed273aa87bf8e5e24b102deec9e8ca16937ab4491a3a44dbadb2d7446e0`
- `window_root`: `dc13a547ec77f6e867844b852c6fe1e7d0585082e84633a0b813ef9dface27c0`

## Activation boundary

This report implements the locked V2 policy in shadow mode only. It does not alter V1, the validator registry, Cobalt ratification, quorum, score provenance, or token economics. Live promotion requires a separate approval.
