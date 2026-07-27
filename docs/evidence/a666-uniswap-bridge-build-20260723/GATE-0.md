# a666/wA666 Uniswap Bridge Build — Gate 0

Date: 2026-07-23

Status: implementation authorized through a `CONTROLLED` mainnet-fork or
Sepolia end-to-end. Ethereum mainnet deployment, a real pool, and real seed
liquidity remain prohibited pending explicit nazgûl/founder approval.

## Decision

Issue a fresh PFTL NAV asset (`a666` class, new 48-byte asset id) and use the
burn/mint movement model. The staged destination verifier is
operator-attested, so the PFTL registry, Ethereum controller, deployment
config, wallet route, and evidence must all report machine-readable trust class
`CONTROLLED`. Use the optimistic-refund commitment semantics already selected
for the controlled stage. Create a new hookless `wA666/USDC` Uniswap v4 pool on
an Ethereum mainnet fork first. Legacy `a651` is historical only and is neither
repointed nor migrated.

## Built and reusable

- PFTL consensus/state: primary subscription at finalized pre-inflow NAV,
  export debit, outstanding claims, destination consume marker, refund, return
  burn/import, route registry/status RPCs, deterministic transition receipts,
  replay verification, route caps, and supply-invariant enforcement.
- Ethereum: `ControlledPFTLReceiptVerifier`, `WrappedVenueNAVCoin`,
  `PFTLUniswapHandoffController` in the `VenueBridgeController` role,
  standalone `PacketReplayRegistry`, settlement adapter, pause/cap/replay
  protections, and Foundry tests.
- Uniswap: canonical mainnet-fork tooling bound to official v4 deployments,
  subscription-plus-export seed provenance, external buy/sell, StateView
  checks, and supply reconciliation.
- Wallet: primary mint, bridge mint-only/mint-and-swap, return route,
  node-authoritative config digests, explicit controlled-beta gates, and
  legacy-route rejection.
- StakeHub: legacy launch mechanics are reusable as implementation reference;
  the locked `a651` stack is not a deployment target.

## Missing for this build

- A genuinely new live-ce22 NAV asset and route registry entry. Existing
  `a666/wA666` values are fixtures and historical controlled evidence.
- Fresh Ethereum contract instances and a new pool identity bound to the live
  asset and node-produced config digest.
- A fresh seed packet derived from live `pfUSDC -> a666` primary subscription
  plus export, never hand-minted operator inventory.
- Runtime wallet metadata for the new asset/contracts/pool and a fresh full
  buy-and-sell evidence packet.
- Gate 5 direct/succinct PFTL finality verification. Its absence is why the
  staging result remains `CONTROLLED` and must not be called trustless.

## Deployment sequence

1. Verify live ce22 6/6 health and the deployed binary's NAV bridge consensus
   transaction support. Issue the new NAV asset and route additively without a
   fleet reset or quorum-reducing restart.
2. Produce the node-authoritative route and launch config digests, then execute
   canonical `pfUSDC -> a666 -> export` for the seed packet.
3. On an isolated Ethereum mainnet fork, deploy fresh controlled verifier,
   wrapped token, replay registry, controller, and settlement adapter; lock the
   intended controller bindings.
4. Consume the canonical seed packet, initialize and seed the hookless v4 pool,
   and execute external `USDC -> wA666` and `wA666 -> USDC`.
5. Wire wallet runtime metadata and display the three distinct actions:
   `Primary PFTL mint`, `Bridge export -> wA666`, and `Uniswap trade`, with
   machine-readable `CONTROLLED`.
6. Verify PFTL and EVM receipts, balances, packet state, pool state,
   consume-once rejection, and the section 7.4 conservation invariant after
   every transition.

## ETA

Estimated 8–14 hours to a fresh controlled mainnet-fork end-to-end if the live
ce22 fleet already runs the required consensus transaction version and the
authorized signer material is usable without restart. A safe rolling binary
deployment while keeping at least 5/6 validators online adds approximately
6–10 hours. Sepolia after the fork adds approximately 4–8 hours depending on
canonical v4 availability and test funding.
