# NAVCoin Reference Posts

The NAVCoin public series defines the product and proof boundaries. The L1 docs
then map those claims to the local PFTL implementation.

## Public posts

| Post | Why it matters |
|---|---|
| [The NAVCoin Proposal](https://postfiat.org/blog/navcoin-proposal/) | Defines a NAVCoin as a machine-verified NAV-tracked asset, not a stablecoin or peg. |
| [Minting a651: One Portfolio, Many Access Venues](https://postfiat.org/blog/navcoin-ethereum/) | Defines a651 as one NAVCoin instance and separates global backing from local access venues. |
| [Proof of Disclosed Leverage](https://postfiat.org/blog/proof-of-leverage/) | Documents the SP1 reserve-evidence primitive and its limits: disclosed account set, not global solvency. |
| [NAVCoin Counterparty Risk](https://postfiat.org/blog/navcoin-counterparty-risk/) | Treats venue/source credit risk as an explicit field instead of hiding it behind reserve totals. |
| [NAVCoin Collateralization Without Spot Redemption](https://postfiat.org/blog/navcoin-collateralization/) | Defines bounded market operations, PFTL-finalized envelopes, and Uniswap venue evidence without promising standing spot redemption. |
| [pfUSDC: Source-Labeled Cash Receipts for NAVCoin](https://postfiat.org/blog/pfusdc/) | Explains why PFTL must label, finalize, haircut, and allocate cash claims before using them in NAVCoin settlement. |
| [Private NAV Subscriptions and OTC Swaps](https://postfiat.org/research/private-nav-otc-swaps/) | Defines the private subscription and secondary OTC design target, including the auditability/privacy tradeoff. |
| [pfUSDC x NAVCoin: A Proven End-to-End OTC Swap MVP](https://postfiat.org/blog/navcoin-otc-mvp-proven/) | Reports the live Arbitrum plus PFTL WAN devnet round trip, including pfUSDC bridge-in/out, a651 primary mint/exit, and a651/a652 swap evidence. |
| [Heavy ZK: Circuit Anatomy and Prover Optimization for Shielded NAVCoin Swaps](https://postfiat.org/research/heavy-zk-optimization-v2/) | Explains the Asset-Orchard shielded swap circuit, measured prover costs, and optimization path. |

## Hosted L1 docs

| Doc | Role |
|---|---|
| [Detailed Proof Of Reserves](../business/navcoin-proof-of-reserves.md) | Full proof-profile, attestor, multi-fetch, challenge, and source-adapter design. |
| [a651 Uniswap Pool](uniswap-pool.md) | Live Ethereum a651/USDC Uniswap v4 venue details, pool id, launch configuration, and caveats. |
| [Current Infrastructure](../business/navcoin-current-infra.md) | Native NAV settlement rail and end-to-end smoke behavior. |
| [Python Example](../business/navcoin-python-example.md) | Minimal Python reserve-packet and operation-builder walkthrough. |
| [Proper Private NAV Swap Plan](../plans/proper-private-nav-swap-plan.md) | Current user-facing plan for private pfUSDC/a651 swaps and exit boundaries. |
| [Archive Summary](../evidence/navcoin-archive-summary.md) | Inventory of archived legacy NAV/VAN applications and contract context. |

## Local implementation references

These paths are useful when reviewing the source tree. Some are intentionally
excluded from the hosted site nav because they are operator handoffs, runbooks,
or specs rather than public docs pages.

| Path | Contents |
|---|---|
| `docs/specs/vault-bridge-navcoin-profile.md` | Generic vault-bridge/NAV profile behind pfUSDC-style source-labeled cash receipts. |
| `docs/specs/otc-swaps-mvp-guide.md` | End-to-end NAVCoin OTC swap MVP flow. |
| `docs/specs/private-otc-shielded-scope.md` | Shielded phase scope for private NAV OTC swaps. |
| `docs/specs/asset-orchard-swap-circuit-design-v2.md` | Production-candidate Asset-Orchard swap circuit design. |
| `docs/status/otc-swaps-mvp-proven-2026-06-19.md` | Proven WAN devnet round-trip evidence record. |
| `docs/status/shielded-layer-map.md` | Current-state shielded layer map for private NAV OTC. |
| `docs/status/pfusdc-bridge-handoff-2026-06-19.md` | Vault-bridge implementation handoff and current command list. |
| `docs/status/arbitrum-contracts-code-review-2026-06-19.md` | Internal review record for the bridge and venue contracts. |
| `docs/runbooks/wan-devnet-full-live-end-to-end-run.md` | Operator flow for the full live WAN devnet round trip. |
| `crates/ethereum-contracts/src/` | Solidity bridge, verifier, market-operation, hook, and mint-controller contracts. |
| `python/postfiat_rpc/` | Python NAVCoin operation builders and external-source adapters. |
| `scripts/` | Smoke, bridge, SP1, market-op, and shielded-swap execution scripts. |
