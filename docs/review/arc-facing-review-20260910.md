# Arc-facing code review — 2026-09-10

Status: fresh-eyes source review; fixes pending

Reviewed checkout: `b8560de9906d60495d746939fc7704bea7e9c984`

## Scope and method

The review traced the pfUSDC ingress and egress guests, bounded proof libraries,
Ethereum receipt parsing, bridge state transitions, A666 primary-market and
pfUSDC vault contracts, and the current mainnet round-trip orchestrator. It
followed proof inputs through their consumers rather than treating fixtures or
simulation output as live authority. Frozen deployment and simulation evidence
was read only.

Focused baselines were green before repair:

- `cargo test --locked -p postfiat-pfusdc-proofs -p postfiat-bridge --lib`:
  `38 passed` in `postfiat-bridge` and `5 passed` in
  `postfiat-pfusdc-proofs`.
- `forge test --root crates/ethereum-contracts -q`: pass.

## Findings

1. **P1 — V2 source-debited exports have no realizable refund path.**

   [`PFTLUniswapPrimaryMarketV2.sol`](../../crates/ethereum-contracts/src/PFTLUniswapPrimaryMarketV2.sol)
   lines 73–87 declares only consume, return-burn, and pause events, while lines
   164–210 exposes destination consumption but no expired-packet cancellation.
   The PFTL verifier in
   [`pftl_uniswap_ethereum_verification.rs`](../../crates/execution/src/pftl_uniswap_ethereum_verification.rs)
   lines 235–277 requires a receipt containing the controller's exact
   `PacketCancelled(bytes32,bytes32,bytes32,uint64,uint64)` event before it will
   refund a trustless route. It also computes only the legacy raw source-packet
   commitment, whereas the V2 controller consumes the domain-separated
   commitment at `PFTLUniswapPrimaryMarketV2.sol:172–180`.

   Concrete failure scenario: a schema-V2 export debits up to the governed
   250,000-A666 per-packet cap, but no relayer completes the destination mint.
   After the deadline, neither the user nor a relayer can create the required
   cancellation log at the route-bound V2 controller. The source refund then
   rejects forever, leaving the exported A666 locked as an outstanding bridge
   claim. This reopens the abandoned-lock liveness failure that the bridge
   contract requires cryptographic consume/cancel mutual exclusion to prevent.

2. **P1 — The top-level mainnet round-trip command mutates live systems without
   an execution interlock.**

   [`a666-mainnet-run-one-full-round.sh`](../../scripts/a666-mainnet-run-one-full-round.sh)
   lines 11–20 accepts identity and output arguments but no live-execution flag
   or intent confirmation. Once the expected environment and key file are
   present, lines 58–81 contact the validator fleet and Ethereum, lines 99–102
   start the mainnet funding step, and lines 106–118 invoke the live issue and
   round-trip stages. The subordinate Python mutations have individual
   execution flags in places, but the orchestrator supplies or bypasses those
   choices internally.

   Concrete failure scenario: an operator invokes the named campaign command
   with a fresh output directory intending to inspect or rehearse it. Merely
   satisfying its documented inputs proceeds to an Ethereum-mainnet deposit,
   PFTL submissions, swaps, burn, and redemption; there is no final command-line
   acknowledgement separating argument validation from live mutation.

3. **P3 — The dedicated Ethereum-mainnet ingress guest carries a stale Fulu
   epoch constant.**

   [`pfusdc-eth-mainnet-ingress`](../../programs/pfusdc-eth-mainnet-ingress/src/lib.rs)
   line 294 and
   [`eth-l1-mainnet-fast-lane-p0`](../../tools/eth-l1-mainnet-fast-lane-p0/src/main.rs)
   line 974 pin epoch `411648`. The shared ingress verifier at
   [`pfusdc-ingress`](../../programs/pfusdc-ingress/src/lib.rs) line 341 and its
   Tier-4 capture helper at
   [`ingress_capture.rs`](../../tools/pfusdc-tier4-prover/src/ingress_capture.rs)
   line 910 pin `411392`. The mismatch is source-visible even though both paths
   now select the same Fulu fork version after the later epoch.

   Concrete failure scenario: reproducing a proof whose finalized beacon epoch
   falls in `[411392, 411648)` yields different fork-domain selection between
   the dedicated mainnet guest and the shared canonical path. A historical
   proof can therefore fail in one implementation while passing the other.
   Under the campaign rule this P3 inconsistency is recorded, not repaired.

## Areas with no findings

- The proof consumers re-bind chain, genesis, route, evidence, finality state,
  amount, recipient, and replay identities; witness-selected public values do
  not bypass the governed authorization boundary.
- The reviewed RLP/Merkle-Patricia parser applies explicit proof, node, depth,
  item, log, topic, and ABI bounds before accepting an Ethereum event.
- The pfUSDC vaults check exact token balance deltas, proof-consumption replay
  keys, bounded recipient text, route bindings, and withdrawal obligations.
- The A666 V2 mint digest intentionally excludes receipt coordinates, but the
  receipt verifier binds those coordinates together with the route digest and
  packet digest before mint acceptance.

## Repair boundary

Findings 1 and 2 require minimal code repairs and focused regressions. Repairing
finding 1 in source does not retrofit the immutable deployed controller or
authorize a route migration; that operational gap remains explicit. Finding 3
is P3 and remains recorded without a golden or guest regeneration.
