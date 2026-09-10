# Task Node UNL V2 shadow comparison

**SHADOW_ONLY — decision support only; no submission, live mutation, or promotion is available.**

Overall: `SHADOW_ONLY_WITH_UNRESOLVED_IDENTITY_LIMITS`. This is not an all-green status.

Version: `2` · Policy: `tasknode-unl-admission-v2` · Report root: `12cef76c1bb3456bca10de84fc0f140e298a95e7454417b1980c0f1c1dedd7c6`

## Side-by-side verdicts

- `validator-bob-rotation: V1 HOLD (account_already_seated,control_epoch_changed) | V2 HOLD (ACCOUNT_ALREADY_SEATED,DECLARED_CONTROL_GROUP_EXISTING_BREACH,HOLD_CONTINUITY,SOCIAL_CLUSTER_SATURATED)`
- `validator-carol: V1 HOLD (connectivity_below_floor) | V2 HOLD (CONNECTIVITY_BELOW_FLOOR,HOLD_CONTINUITY)`

## HOLD_CONTINUITY

- `validator-bob-rotation` / `account-bob` — `ACCOUNT_ALREADY_SEATED,DECLARED_CONTROL_GROUP_EXISTING_BREACH,HOLD_CONTINUITY,SOCIAL_CLUSTER_SATURATED`; report `e4127a3307b55480073bcdfc3ffee4796b96708d6503dba5cce03fc9f5d7e232`
- `validator-carol` / `account-carol` — `CONNECTIVITY_BELOW_FLOOR,HOLD_CONTINUITY`; report `9341f99921f4c4fc378306708c6967d1f061d5a4a7dcec3119e0bc2a57530e63`

## Admission denials

- `validator-bob-rotation` — `HOLD`: `ACCOUNT_ALREADY_SEATED,DECLARED_CONTROL_GROUP_EXISTING_BREACH,HOLD_CONTINUITY,SOCIAL_CLUSTER_SATURATED`; report `e4127a3307b55480073bcdfc3ffee4796b96708d6503dba5cce03fc9f5d7e232`
- `validator-carol` — `HOLD`: `CONNECTIVITY_BELOW_FLOOR,HOLD_CONTINUITY`; report `9341f99921f4c4fc378306708c6967d1f061d5a4a7dcec3119e0bc2a57530e63`

## EXISTING_BREACH

- `DECLARED_CONTROL_GROUP` `ee393c09ccd2a8e241b20f7854bd82a881ecb6eea607494ff635346a794aa5e4`: `1` excess seat(s), review `FULL_WINDOW_REVIEW`, evidence `940eec1cd9bc021101d5d088773a80ab36583048411338240289677dba0fd05a`

## Known control and evidence limits

- An unchanged-key control transfer is undetectable by these public inputs.
- A valid custody signature proves key control, not personhood or continuity of a human operator.
- Funding observations are audit-only and cannot create admission trust, control-group membership, or a veto.

A valid custody signature is not a personhood claim. An unchanged-key account sale or other control transfer is not detectable from these public inputs. Funding remains audit-only: it creates neither positive trust mass nor a unilateral veto.

## Bound roots

- `admission_policy_root`: `499c56b8dbb8804392b05aef6607c66e139ac82dec4e72bb63edc6c8420cdcd2`
- `cli_input_root`: `17f858fe7d3842d939afc790ef110cb3e771f20918c76f9b3f88a8b7862eae7c`
- `control_registry_root`: `28d80fb8c1017f34eb5d3e1082a7e0e5ee89ec0eea890496b676b3107f8afd2a`
- `declaration_root`: `4b0fae6b8275eb21e45b173dc316cf49dc307c01df98cff3eb90e5dc56ef0349`
- `evidence_input_root`: `411a5f1937aafed44ca3bfe1f60ce7da60ca27d90f05b8ce3c556652b8305b3a`
- `evidence_policy_root`: `1b697d1302a1a21bb576cdeac59c2a20fba8aa4ac4f3b311ebbe5ea56f3cda76`
- `frozen_window_root`: `e6da334c08a778f0a03996be78eb7e8e033d68311478f0b180d430fa81810e26`
- `graph_root`: `cd3989ecd72f763c1b2b15d67a0accf8485329fca3885a60be77166eeb9133e0`
- `v1_reference_root`: `b0be5a7ccb2d744ac73225a474246fa36cb613c9344d29be5ebabd3966804fa4`
- `validator_registry_root`: `4a705ed273aa87bf8e5e24b102deec9e8ca16937ab4491a3a44dbadb2d7446e0`
- `window_root`: `dc13a547ec77f6e867844b852c6fe1e7d0585082e84633a0b813ef9dface27c0`

## Activation boundary

This report implements the locked V2 policy in shadow mode only. It does not alter V1, the validator registry, Cobalt ratification, quorum, score provenance, or token economics. Live promotion requires a separate approval.
