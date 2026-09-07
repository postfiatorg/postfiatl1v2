# PostFiat L1 Status

## Current Phase: Controlled Testnet

The repository implements controlled local and remote multi-validator workflows.
It is not a public-mainnet release. Source capability, a retained deployment
receipt and current network health are separate claims. See the
[whitepaper overview and alignment audit](docs/architecture/whitepaper-overview.md)
for a revision-pinned protocol review.

## Operational State Boundary

The [Current State](docs/status/chain-state-current.md) page owns the dated
operational record. The retained August 31 storage receipt records:

- release `storage-lease-af9b83c3`, source
  `af9b83c355267f18cd2b1b173b25fed57553a8ed`;
- node binary SHA-256
  `383f4325a157f554786b6c8868defedcef8faeb320f9998eeef69c07c7141a7a`;
- transactional `redb` activation at height 930 and six-validator continuation
  through height 931;
- replicated-state-v2 commitment and zero reported full-history scans in that
  recorded final state; and
- Z1 observation start `2026-08-31T04:29:41Z`, which does not itself prove Z1
  completed or that public-testnet gates passed.

Source: `deployments/storage-lease-20260831/deploy-receipt.json`. This document
reconciles that retained record; it does not report a new fleet probe.

The earlier Cobalt E5 observation through height 924 remains evidence for its
signed rollback/return at 922/923, nine negative rejections, and legitimate
validator-5 rotation. E1–E6 closed `KEEP_ACTIVE` for the bounded validator-trust
scope. Consensus v2 remains block finality. A later repository commit is not
itself a deployed release.

## Implemented Source Capabilities

| Surface | Boundary |
| --- | --- |
| Consensus v2 | Activated prepare/precommit, durable locks, signed timeouts and deterministic proposer rotation; legacy single-view behavior remains below an explicit activation boundary. |
| Transparent and issued assets | Payments, account history, asset/trustline, escrow/NFT/offer and dual-authorized atomic-swap paths. Native supply is fixed; issued assets follow their own mint/burn rules. |
| Settlement lanes | FastPay and FastSwap use distinct prefunded-object certificate, durability and recovery rules; block finality alone is not their complete success condition. |
| Shielded settlement | Asset-Orchard proof/authorization checks, anchors/nullifiers and public turnstile accounting; public ingress/egress fields and classical privacy assumptions remain. |
| Governance | Signed Foundation governance and activated Cobalt validator-trust decisions, with exclusive scope and consensus ordering. Off-chain model/shadow output is not live authority. |
| Authorization | ML-DSA-65 transparent-account and validator signatures; no implemented alternate SLH-DSA recovery family. |
| NAV/reserves | Typed reserve profiles, finalization, challenges and bounded proof verification. Proofs cover disclosed evidence and do not eliminate custody or undisclosed-liability risk. |
| Storage | Versioned transactional finality storage and legacy import/replay, with explicit activation and startup checks. |
| Interfaces | Read/controlled-write RPC, Python clients, wallet tooling and MkDocs engineering documentation. |

Clients must verify both the block certificate and the matching accepted
transaction receipt. A certified block can contain a rejected transaction.

## Remaining Limits

- No public-mainnet claim or independent public-validator diversity is established.
- HSM/remote validator signing is not implemented. Legacy/JSONL long-running
  operation requires its unsafe-devnet acknowledgement; a ready activated
  transactional generation has a separate startup gate.
- Historical storage candidates `d0ae79f3` and `10dd9f20` remain disqualified by
  their recorded failures. The later distinct deployment does not retrospectively
  qualify them or close every item in the storage/testnet journals.
- No automatic Negative-UNL quorum reduction, complete LaunchCertificate or
  independently verified replay-certificate-to-admission pipeline is established.
- Orchard proving/verification cost, disclosure leakage, recovery controls and
  cryptographic review remain action- and deployment-specific requirements.
- The [whitepaper gap backlog](docs/architecture/whitepaper-gaps.md) identifies
  missing original measurement packets, fee/wire-cost accounting and proof-to-code
  composition work. Historical latency numbers are not current service commitments.

## Evidence

Use the [Evidence Index](docs/evidence/index.md),
[Cobalt adversarial results](docs/governance/cobalt-adversarial-verification-results.md),
[E5 packet](benchmarks/cobalt-adversarial-verification/e5/README.md), and the
[operational-state page](docs/status/chain-state-current.md). The
[audit validation record](docs/architecture/whitepaper-validation.md) distinguishes
checks run now from retained tests and historical reports.
