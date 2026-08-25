# PostFiat L1 Current State

Updated: `2026-08-25T17:01:49Z`

Status: authoritative operational-state reference

This page separates the running controlled devnet, its active binaries, the
activation/audit tooling, the current Git checkout, and the adversarial campaign.
Those identities are different and must not be collapsed into “`main` is live.”

!!! note "Freshness"

    An authenticated, read-only probe of all six validators completed at
    `2026-08-25T15:37:40Z`. It observed the runtime state documented here.
    This is fresher than the committed activation packet, but it is still a
    point-in-time observation; re-probe the fleet before making a later “right
    now” claim. The later E2 campaign was isolated and did not query or mutate
    the fleet.

## State Summary

| Plane | Current recorded state | Exact identity |
| --- | --- | --- |
| Running devnet | All six validator, RPC, and Cobalt shadow services were active. Every validator reported `running`, height 919, zero pending mempool items, and identical tip and state roots. | Chain `postfiat-wan-devnet-2`; probe completed `2026-08-25T15:37:40Z`. |
| Validator-trust authority | Auditor-backed `live-status` on validator-0 returned `ACTIVATED`; all checks passed. Cobalt can ratify validator-registry and trust-graph changes. Consensus v2 remains block finality. | Activation height 916; validator-5 key rotation height 917; authority scope `validator_trust_evolution_v1`. |
| Active consensus runtime | All six validator services run release `cobalt-activation-8694b99d` with the same binary hash. The release manifest names Git revision `8694b99d`; the binary's embedded build revision is `116bed84`. | Binary SHA-256 `431f194ba28391eba16c18a96d49c358bd2047d3b37eb0115216f46c6a6783f4`. |
| Governance auditor | The full Cobalt authority/history verification uses a separate, read-only node binary. It is not the active validator service binary. | Release `cobalt-live-governance-audit-05507758`; SHA-256 `055077582342dd54af0212df82e626cc83aae8af119c09f1cd1309dad906293e`. |
| Cobalt shadow runtime | All six shadow services were active with the same binary. Validator-0 reported healthy transport, six peers, current catch-up, and the live registry/trust roots. Shadow mode remains advisory and cannot mutate validator state or finalize blocks. | Release `cobalt-shadow-registry-reset-43ac8a7d`; SHA-256 `43ac8a7df13f41d5cfdd783fc983de4e8e91b625e6a705ae342569c5771ad935`. |
| Repository | `main` contains the E2 frozen harness and manifest plus the committed passing result packet. These source/evidence commits are later than the active runtime and are not deployed. | E2 freeze `15ef2307732cf46ff3b921bf02f3ad096dda15f3`; E2 evidence `b78809908821b77ce0a9943f08ec3c7cae69bf84`; use `git rev-parse HEAD` for the moving documentation HEAD. |
| Adversarial verification | E1 is complete across all 10,240 generated trust graphs. E2 is complete across 108 validator/strategy cases and 442,368 schedules with zero conflicting roots, false accepts, false halts, synchrony violations, or rejected-state mutations. E3-E6 have not started, so the overall `KEEP_ACTIVE` gate remains open. | E1 completion `9ffa9992`; E2 evidence `b7880990`; packet root `8742d960…d7cba3`; [active milestone](../plans/active/cobalt-adversarial-verification-milestone.md). |

## Observed Devnet State

| Field | Value |
| --- | --- |
| Chain | `postfiat-wan-devnet-2` |
| Genesis hash | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Probe completed | `2026-08-25T15:37:40Z` |
| Validators | 6, all equal |
| Service state | All validator, RPC, and Cobalt shadow units active |
| Height | 919 |
| Mempool | 0 pending on every validator |
| Tip hash | `3a8a117af9ed40728717005d03edf032719a3ca3d696365415a2d5b0d9aeef1c509d06d54029e6c34660e29aab43d0fb` |
| State root | `ffa16323555800df7a4ff7cd336b9b151b0edfcf60954c207b704749133ff4b31ebd24444696d67e652f6e94510f7e60` |
| Registry root | `945768d593497541f59961d1ba3920560cfde7bf5037e40eb89dd5466637f221709bff05b69d2d40a36d5cff8505c37e` |
| Trust-graph root | `9221316ae7f0f0e7e58d734700167f73f29aa1240377a8d61c637e7f36c5deb728203fcbb283c9f8f3398fc41d6b8b13` |
| Validator-trust authority | Cobalt |
| Block finality | Consensus v2 |

Cobalt does not decide which validators deserve trust, originate proposals,
control unrelated governance, order blocks, or replace Consensus v2. Foundation
operators currently administer proposals and the six validator services. The
activation proves bounded protocol capability, not operator decentralization.

## Runtime, Auditor, And Packet Lineage

The repository previously mixed three separate node identities:

1. **Active consensus runtime:** `cobalt-activation-8694b99d`, binary hash
   `431f194b…783f4`, on all six validator services. Its deployment manifest names
   revision `8694b99d`; its embedded build revision is `116bed84`.
2. **Governance auditor:** `cobalt-live-governance-audit-05507758`, binary hash
   `05507758…6293e`. The current `live-status` command uses this binary to verify
   the non-uniform Cobalt authority history against persisted node state.
3. **Shadow runtime:** `cobalt-shadow-registry-reset-43ac8a7d`, binary hash
   `43ac8a7d…d935`, active separately on all six machines in advisory mode.

The frozen activation packet's `source-pins.json` labels
`cobalt-verifier-92b63f5a` as `postfiat-node-live-consensus`. That label does not
match the active consensus services observed by the fresh probe. The packet is
checksum-bound and remains valid as a historical activation artifact, so it is
not edited in place. Do not use that field as the current service identity.

The manifest/build revision split is explainable but remains explicit:
`git diff 116bed84..8694b99d` changes only
`benchmarks/cobalt-handoff-rehearsal/run_rehearsal.py`, not Rust runtime source.
The release manifest records the later deployment checkout while the binary
retains its earlier embedded build revision.

The packet's source base `09774843` and deployment-record commit `6bdcdd24`
describe the activation/audit source capture and receipts. They are not the
identity of the active validator binary. The later E2 freeze
`15ef2307732cf46ff3b921bf02f3ad096dda15f3`, evidence commit
`b78809908821b77ce0a9943f08ec3c7cae69bf84`, and this documentation
descendant are also not deployed.

## Probe And Evidence Boundary

The fresh probe was non-mutating. It performed only:

- authenticated `systemctl is-active` checks for each validator, RPC, and shadow
  service;
- SHA-256 checks of each active consensus and shadow binary;
- `postfiat-node status` against each validator data directory;
- `verify-governance --cobalt-mode non-uniform` with the separate governance
  auditor on validator-0;
- auditor-backed `live-status` plus shadow status on validator-0.

No services were restarted, no files were changed on the fleet, and no RPC write
method was invoked. The full auditor-backed authority verification was run on
validator-0; all-six convergence was independently established from each node's
status, service state, and binary hash.

The earlier checksum-bound activation packet remains under
`benchmarks/cobalt-activation-live/packet/`:

```bash
python3 benchmarks/cobalt-activation-live/packet/verify_packet.py
```

Its expected manifest root is
`b603b59d0245a7c73e766d0ba7fb19975f11e1e39bdd7263bf87e65250438bfb`.
That verifier proves internal packet consistency; it does not query the fleet.

## Repository And Campaign Boundary

The current source branch contains the deployment history, later documentation,
the off-chain E1 oracle/harness and evidence, and the completed E2 harness and
packet. None of those facts means the source HEAD is installed on validators.
A deployment claim requires a service path, binary hash, and node observation;
a source claim requires a branch and commit; an experiment claim requires its
frozen corpus and result packet.

Dated handoffs and completed plans remain historical snapshots. They must link
here when later events supersede their operational statements.
