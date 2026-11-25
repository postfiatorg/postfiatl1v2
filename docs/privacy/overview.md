# Privacy Overview

PostFiat privacy is built around an Orchard/Halo2 shielded path using the
upstream Rust/Zcash Orchard and Halo2 implementations. PostFiat implements the
chain adapter and application circuits; it does not implement a new Halo2 proof
system. The exact dependency and narrow local compatibility patch are described
in [Halo2 Dependency And Local Patch Boundary](../security/halo2-dependency.md).

The goal is confidential settlement for workflows where transparent ledgers leak
too much: balances, counterparties, timing, execution intent, treasury movement,
and position changes.

## What Exists

- Orchard/Halo2 adapter using the upstream Rust/Zcash Orchard stack.
- Direct transparent-to-Orchard deposit envelopes.
- One-note Orchard spend construction.
- One-note Orchard withdraw construction.
- Nullifier persistence and duplicate-spend rejection.
- Retained roots and scan material for local wallet spending.
- `orchard-scan`, `orchard-disclose`, and `orchard-disclosure-verify`.
- Privacy assurance receipt fixture and verifier for scoped institutional
  disclosure without full viewing-key access.
- Shielded asset predicate registry fixture for transparent supply, private
  flow, and closed owner-scoped asset predicates.
- Privacy metadata/anonymity-bound fixture for low-volume, bursty, thin-asset,
  unique-disclosure, direct-RPC, and forbidden-public-field routes.
- Privacy deanonymization-bound fixture that converts metadata floor counts
  into equal-weight estimates in integer basis points and joint-candidate routes.
- Privacy correlation-bound fixture that requires observed joint partitions,
  pairwise intersections, temporal-link cohorts, and RPC cohorts.
- Privacy observer-inference fixture that derives candidate count `k` by
  intersecting timing, fee, RPC, disclosure, asset/policy, and off-chain
  side-information partitions.
- Privacy floor-calibration fixture that derives the v1 `k >= 16`, 625 bps,
  five-minute bucket, 128-action window, and batch-size floors from a bounded
  deanonymization adversary.
- Privacy longitudinal-linkage fixture that routes repeated observations,
  multi-target search, repeated disclosure, recurring timing, direct RPC reuse,
  exchange-side batching, and thin asset-policy reuse.
- Public Orchard pool telemetry through `orchard_pool_report`.
- Bounded RPC batch creation for Orchard actions and direct deposits.
- Malformed proof, rate-limit, child-isolation, and concurrency-limit evidence.
- Live five-validator deposit, spend, withdraw, outage, restart, snapshot, soak,
  and audit-packet evidence from May 2026 (history: the network then had five
  validators; the reports are archived outside this repository, see
  `reports/README.md`).

## Status On The Live Devnet

Checked on 25 November 2026.

- The code listed above is on `main`.
- On `postfiat-wan-devnet-2`, a controlled devnet with six validators, public
  submission of shielded actions is off on every validator
  ([handoff of 23 November](../status/handoff-2026-11-23.md)).
- `orchard_pool_report` on validator 0 at height 1900 reported pool
  `asset-orchard-v1` with 177 outputs, 165 nullifiers and 41 accepted anchors,
  all report checks passing. These counts come from earlier actions; the
  report claims no active-user count.
- The RPC server admits user-proven private actions (`private_action_submit`)
  only when it is started with `rpc-serve --allow-private-action-admission`;
  the switch is off unless set (`crates/node/src/private_action_rpc.rs`).
- Post-quantum private payments are not on `main`.
- The assurance-receipt, predicate-registry, metadata, deanonymization,
  correlation, observer-inference, floor-calibration and longitudinal-linkage
  items are schemas and fixtures under `docs/governance/agent/`. Their verifier
  scripts are archived outside this repository, and no code in `crates/` or
  `python/` implements them.

## System Diagram

```mermaid
flowchart LR
  Transparent[Transparent account] --> Deposit[Deposit envelope]
  Deposit --> Orchard[Orchard pool]
  Orchard --> Spend[Private spend]
  Orchard --> Withdraw[Withdraw envelope]
  Withdraw --> Transparent2[Transparent recipient]
  Orchard --> Scan[Wallet scan]
  Scan --> Disclose[Selective disclosure]
  Disclose --> Assurance[Assurance receipt]
  Orchard --> AssetPred[Shielded asset predicates]
  Orchard --> MetaBound[Metadata bounds]
  MetaBound --> DeanonBound[Deanonymization bounds]
  DeanonBound --> CorrBound[Correlation bounds]
  CorrBound --> ObserverModel[Observer inference model]
  ObserverModel --> FloorCalibration[Floor calibration]
  FloorCalibration --> Longitudinal[Longitudinal linkage]
```

## Main Sources

- `crates/privacy/src/lib.rs`
- `crates/privacy_orchard/src/lib.rs`
- `crates/privacy_orchard/src/types.rs`
- `crates/privacy_orchard/src/verify.rs`
- `crates/node/src/privacy.rs`
- `docs/status/privacy-production-burndown.md`
- [Assurance Receipts](assurance-receipts.md)
- [Shielded Asset Predicate Registry](shielded-asset-predicates.md)
- [Privacy Metadata And Anonymity Bounds](metadata-anonymity-bounds.md)
- [Privacy Deanonymization Bounds](deanonymization-bounds.md)
- [Privacy Correlation Bounds](correlation-bounds.md)
- [Privacy Observer Inference Model](observer-inference-model.md)
- [Privacy Floor Calibration](floor-calibration.md)
- [Privacy Longitudinal Linkage](longitudinal-linkage.md)
