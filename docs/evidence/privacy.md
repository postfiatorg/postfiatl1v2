# Privacy Evidence

Privacy evidence proves the Orchard/Halo2 controlled privacy path.

## Main Packets

- `reports/testnet-live-orchard-full-flow/live-orchard-full-flow-20260515T183724Z/testnet-live-orchard-full-flow.json`
- `reports/testnet-orchard-privacy-audit-packet/orchard-privacy-audit-packet-20260515T185212Z/orchard-privacy-audit-packet.json`
- `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T153630Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`
- `reports/orchard-verification-budget-v1-report.json`
- `reports/privacy-assurance-receipt/*/privacy-assurance-receipt-report.json`
- `reports/shielded-asset-predicate-registry/*/shielded-asset-predicate-registry-report.json`
- `reports/privacy-metadata-anonymity-bound/*/privacy-metadata-anonymity-bound-report.json`
- `reports/privacy-deanonymization-bound/*/privacy-deanonymization-bound-report.json`
- `reports/privacy-correlation-bound/*/privacy-correlation-bound-report.json`
- `reports/privacy-observer-inference-model/*/privacy-observer-inference-model-report.json`
- `reports/privacy-floor-calibration/*/privacy-floor-calibration-report.json`
- `reports/privacy-longitudinal-linkage/*/privacy-longitudinal-linkage-report.json`

## What They Prove

- transparent-to-Orchard deposit;
- Orchard spend;
- Orchard withdraw;
- one-validator outage during shielded flow;
- replay/recovery;
- pool counters;
- public write edge remains controlled;
- redaction-safe audit packaging.
- local Orchard/Halo2 verification budget with cached verifier timing, proof
  size, and wallet-side proving cost class.
- assurance receipt packet shape, scoped-disclosure constraints, policy-root
  binding, evidence artifact hashes, and invalid-fixture rejection.
- shielded asset predicate registry boundaries: transparent supply, private
  flow, content-addressed predicate IDs, Cobalt-governed registry roots,
  owner-scoped-only predicates, privacy-preservation public artifact shape, and
  rejection of private supply, DvP, arbitrary zkVM, shared mutable AMM, recipient
  identity transfer policy, root mismatch, ungoverned issuer rules, and full
  viewing-key leakage.
- metadata, deanonymization, correlation, and observer-inference route
  boundaries: low activity, bursty timing, thin asset partitions, unique
  disclosure, direct RPC observation, stale observations, side information,
  missing channels, and policy-root mismatch downgrade, hold, disclosure, or
  fail closed instead of claiming baseline privacy.
- floor-calibration route boundaries: the `k >= 16`, 625 bps, five-minute
  timing bucket, 128-action activity window, and batch-size floors derive from
  a bounded single-target deanonymization adversary and reject correlated-flow
  collapse, exchange-side batching observers, direct RPC observation, and
  side information.
- longitudinal-linkage route boundaries: repeated observations use measured
  linked-window intersections rather than multiplied probabilities, multi-target
  search uses a union bound, and repeated disclosure, recurring timing, direct
  RPC reuse, exchange-side batching, thin asset-policy reuse, and side
  information downgrade, hold, require disclosure, or fail closed.
