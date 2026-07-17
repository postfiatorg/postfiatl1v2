# Validator Admission Bin Policy

The validator-admission bin policy turns narrative admission evidence into
checker-enforced bins before any model output can affect a selector route.
It is a design fixture for controlled-testnet governance work. It is not a
live registry mutation and it does not transfer authority.

## What It Checks

The policy defines five deterministic bins:

| Bin | Admission rule |
| --- | --- |
| `exposure` | `2` requires at least two current independent sources across distinct control surfaces and attestor groups. Self-description does not count. |
| `accountability` | `2` requires at least three current accountability proofs, such as manifest signature, domain control, jurisdiction contact, and incident-response contact. |
| `reliability` | `2` requires uptime at or above `9950` bps in the measured window. |
| `attack` | `2` rejects the candidate; `1` can only pass if every other gate clears. |
| `rho` | `0` is required. Hidden shared release management, KMS, or funding control is a hard reject. |

This is the missing boundary between "the model classified this packet" and
"the selector can consume this packet." The model cannot change a bin, waive a
source-independence rule, or turn stale or contradictory evidence into an admit.

## Required Cases

The fixture suite requires the selector to handle these cases:

| Case | Required route | Why |
| --- | --- | --- |
| `case-01-clean-admit` | `admit` | Independent exposure, accountability, reliability, low attack surface, and `rho = 0`. |
| `case-02-self-only-exposure-hold` | `hold` | Candidate self-description is not independent exposure. |
| `case-03-rented-wash-exposure-challenge` | `hold-for-challenge` | Evidence may be volume-shaped or rented, so it needs a challenge record. |
| `case-04-hidden-shared-control-reject` | `reject` | Hidden shared control breaks the correlation cap. |
| `case-05-stale-attestation-hold` | `hold` | Stale required evidence must be refreshed. |
| `case-06-contradictory-receipts-hold` | `hold` | Contradictory required evidence is not averaged away. |
| `case-07-high-attack-reject` | `reject` | High attack surface exceeds the cap. |
| `case-08-rolling-decay-hold` | `hold` | Exposure decay reopens the challenge window. |

## Verification

Run the verifier from the repository root:

```bash
scripts/validator-admission-bin-policy-verify --fixtures
scripts/validator-admission-bin-policy-verify --write-report
scripts/validator-admission-bin-policy-verify --verify-report
```

Current fixture roots:

| Artifact | SHA3-384 |
| --- | --- |
| Valid packet | `a1df3c92baad3714a8c4fc16c2a812b8123c19c11c9354a4ef4a13d1a39127512fa12f7700c2fa329f2b7f0284907821` |
| Statement | `8133e640794fe9885a7bb77d727c61659d343e175e1732eede81ca3900db89aeca4d53e145448e174b0f77482efc1b21` |
| Policy root | `a9421b3d49e06d95c6a4ebef23cb6d8830d20efe2f1b375896e2e80d25d6bde16fd7fa63b8c22bcbe166d09078b82d81` |

The verifier also binds the packet to existing admission evidence:

| Evidence artifact | SHA3-384 |
| --- | --- |
| `reports/validator-admission-policy-v1-report.json` | `7421cfe25d3e1c3058a8893b6a9bd64be213e62485e478104692577314295b378385b3ea2acfdfeba39923ec9980c1dd` |
| `docs/governance/validator_admission_benchmark/packets.json` | `78fe267c1423f378d2af6eaee89136cbc1dfc4a51f8a4b9c92a336d7015f6c08178809acdc51db3894b1751e20f20e79` |
| `reports/validator-admission-benchmark/score/20260528T210501Z/summary.json` | `fb1ed97a70f7f16e87fe70c55ce42fc98115142779232cea4adc5cbc2a5519b1b2646bbf6348e65d942c09332601ac34` |

## Status

This packet is a controlled-testnet design artifact. The next implementation
step is to bind this verifier into registry-delta preflight so an admission
proposal cannot enter a Cobalt transition unless its bins and route are
recomputed from the packet and match the committed selector output.
