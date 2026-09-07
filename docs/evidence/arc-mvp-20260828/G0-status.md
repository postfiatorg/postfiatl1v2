# G0 status

Status as of 2026-08-28 UTC: **G0 accepted**. Arc ingress circuit work may begin.

| To-do | Status | Evidence |
|---|---|---|
| 1.1 arc-node pin and commit structure | Complete | `arc-node-pin.txt`, `arc-commit-structure.md` |
| 1.2 golden commit verification | Complete | two JSON fixtures, `conformance-test.log` |
| 1.3 receipt inclusion | Complete | typed + legacy fixture, `receipt-conformance.md`, mutation test |
| 1.4 precompiles and gas | Complete | deployed probe, four successful live receipts, receipt/internal gas table in `precompile-gas.md` |
| 1.5 USDC conformance | Complete | funded approve and exact 1,000,000-atom pull with balances and receipt logs in `usdc-conformance.md` |

The precompile stop condition did not trigger: SHA-256 and BN254 add, multiply, and pairing are present and correct on live Arc testnet. The commit-binding stop condition did not trigger: the pinned source provides the full certificate-to-execution-header chain.

The funding blocker was removed. Both probe deployments and all six acceptance
transactions have status `1`; at the receipt recheck block 59,331,927 they had
191--289 confirmations. The required contract-level SHA-256/BN254 outputs and
the funded vault-style USDC pull are recorded in the linked evidence files.

Contract preparation discovered and removed a latent Arc incompatibility: the inherited Tier-4 vault/anchor path required Nitro `ArbSys` and an Ethereum outbox. `arc-direct-ingress.md` documents the explicit direct mode and its 18-test compatibility/adversarial suite.
