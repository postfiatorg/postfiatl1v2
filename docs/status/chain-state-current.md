# PostFiat L1 Current State

Updated: `2026-08-26T04:00:00Z`

Status: **canonical operational-state reference**

This page separates the last observed controlled-devnet state, deployed runtime
lineage, repository state, and adversarial campaign. These are different planes.
A repository commit is not deployed unless a fleet receipt binds that source and
binary to the running services.

!!! warning "Point-in-time evidence"

    The latest all-six validator/RPC observation ran from
    `2026-08-26T01:40:51Z` through `2026-08-26T01:41:04Z`. It was authenticated
    and read-only, but it is not a real-time probe now. Re-probe before making a
    later “right now” claim. That run did not re-audit Cobalt authority or inspect
    the six shadow services; those claims retain their older freshness labels.

## Operational summary

| Plane | Recorded state | Exact identifier | Observed or updated at | Evidence and freshness |
| --- | --- | --- | --- | --- |
| Running devnet | Six validator and six RPC services were active. Every validator reported `running`, height 919, zero pending transactions, and equal tip/state roots. | Chain `postfiat-wan-devnet-2`; genesis `ce22ca8c…e90a9`. | `2026-08-26T01:40:51Z`–`01:41:04Z` | Freshest committed operator observation; read-only point-in-time probe, not a current network query. |
| Validator-trust authority | Last full audit recorded Cobalt as authority for validator-registry and trust-graph changes from height 916; the first Cobalt-authorized key rotation committed at 917. Consensus v2 remains block finality. | Authority scope `validator_trust_evolution_v1`; registry root `945768d5…05c37e`; trust root `9221316a…6b8b13`. | `2026-08-25T15:37:40Z` full audit; roots captured at height 919. | Last authenticated authority/shadow audit; not re-run by the later validator/RPC probe. |
| Deployed validator runtime | All six validator processes used the same release and binary. | Release `cobalt-verifier-92b63f5a`; embedded revision `92b63f5a`; SHA-256 `c7cb0c25001a0bfe22eba32ce870f3739f9710471906e27c32797670ea9f6337`. | `2026-08-26T01:40:51Z`–`01:41:04Z` | Direct process-path and binary-hash observation on all six hosts. |
| Auditor and shadow | The last audit used the separate read-only governance auditor. All six advisory shadows were active and validator-0 reported healthy transport, six peers, current catch-up, and the recorded roots. Shadows cannot mutate validator state or finalize blocks. | Auditor `cobalt-live-governance-audit-05507758`, SHA-256 `05507758…6293e`; shadow `cobalt-shadow-registry-reset-43ac8a7d`, SHA-256 `43ac8a7d…d935`. | `2026-08-25T15:37:40Z` | Older authenticated observation; not re-checked on 2026-08-26. |
| Repository | `main` contains deployment history, E1–E4 and E6 evidence, plus later E5 work. None of the post-deployment descendants is proven installed on the fleet. | Pushed E4 evidence commit `6c22f866e9ba56ec18f3a62fbf2b00ec9aa17103`; E4 packet root `93ba3db0bcc145144713088b612606fbb3b92c0f542809f258da49a555c14508`. | 2026-08-26 | Source/evidence state only. Use `git rev-parse HEAD` for the moving checkout identity. |
| Adversarial campaign | E1–E4 and design-only E6 passed. E5 live authority drills and milestone publication remain open, so the overall `KEEP_ACTIVE` decision is not yet earned. | E1 `9151c9…cbbc05`; E2 `8742d960…d7cba3`; E3 `9302b355…40b600`; E4 `93ba3db0…c14508`; E6 `52a4ef91…d6d05c81`. | E4 completed 2026-08-26 | [Active milestone](../plans/active/cobalt-adversarial-verification-milestone.md); E4 was isolated local evidence and did not query or mutate devnet. |

## Last observed devnet values

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Validator/RPC probe window | `2026-08-26T01:40:51Z`–`2026-08-26T01:41:04Z` |
| Validators | 6, all equal |
| Height | 919 |
| Mempool | 0 pending on every validator |
| Tip hash | `3a8a117af9ed40728717005d03edf032719a3ca3d696365415a2d5b0d9aeef1c509d06d54029e6c34660e29aab43d0fb` |
| State root | `ffa16323555800df7a4ff7cd336b9b151b0edfcf60954c207b704749133ff4b31ebd24444696d67e652f6e94510f7e60` |
| Registry root | `945768d593497541f59961d1ba3920560cfde7bf5037e40eb89dd5466637f221709bff05b69d2d40a36d5cff8505c37e` |
| Trust-graph root | `9221316ae7f0f0e7e58d734700167f73f29aa1240377a8d61c637e7f36c5deb728203fcbb283c9f8f3398fc41d6b8b13` |
| Validator-trust authority | Cobalt, last fully audited 2026-08-25 |
| Block finality | Consensus v2 |

Cobalt ratifies validator-registry and trust-graph changes. A separate layer
decides which validators deserve trust. Current proposals originate from
Foundation-administered validators. Cobalt does not order blocks, replace
Consensus v2, or prove operator decentralization.

## Evidence boundaries

The 2026-08-26 probe performed only authenticated service-state checks,
process-path and binary-hash reads, and `postfiat-node status`. It changed no
fleet files or services and invoked no mutation RPC. It did not re-run the
governance auditor or shadow-status checks.

The historical activation packet is under
`benchmarks/cobalt-activation-live/packet/`:

```bash
python3 benchmarks/cobalt-activation-live/packet/verify_packet.py
```

Its expected packet root is
`b603b59d0245a7c73e766d0ba7fb19975f11e1e39bdd7263bf87e65250438bfb`.
That verifier proves committed packet consistency; it does not query the fleet.

The E4 finality-isolation packet is under
`benchmarks/cobalt-adversarial-verification/e4/`:

```bash
python3 benchmarks/cobalt-adversarial-verification/e4/verify_packet.py
```

Its packet root is
`93ba3db0bcc145144713088b612606fbb3b92c0f542809f258da49a555c14508`.
E4 was a paired local 500+500-round campaign, not a devnet probe.

Dated handoffs and completed plans are historical snapshots. When their mutable
operational statements conflict with this page, this page owns the current
record; the capture time still limits every claim.
