# a666 clearance report — 2026-07-23

Status: **PARTIAL / CONTROLLED / rollout held**.

No mainnet deployment or spend occurred. No ce22 validator was restarted or
rolled. The founder `$5` job was not read, consumed, or modified.

## Persistent Sepolia route

The durable `CONTROLLED` stack is deployed on Ethereum Sepolia:

- `wA666`: `0xF226c4f3d01ba63900032aD72203e09Ca6A62Cb3`
- controlled verifier: `0x9c71e82576F322ac9b52D7E31b208B109114c3C9`
- replay registry: `0xcFAd887C431Ae365EB063F0202b33a15a8fB5F4d`
- bridge controller: `0xeAF7503663A901c4e2ebAD4Fb90C6f17D2A06764`
- settlement adapter: `0xEb347ffD804049f5EB7c10bc5f06EfE08289C87C`
- v4 router: `0x9188EBC8bbD7382FeC052fA1E7DB4E783b0E8032`
- launch helper: `0x74f4A27Acd503B3aABE955659BFEda33082e3340`
- pool ID:
  `0x6a6ec0e8565df8e9fc38ecf94aad82988f6801cad477f90382f4e9217823289d`

The pool is initialized against Circle Sepolia USDC and the official Uniswap
v4 contracts. Supply, bridge outstanding, and pool liquidity remain zero.
There was no hand mint and no seed liquidity. All ownership, immutable
bindings, authorizations, pool slot0, supply, and route-digest readbacks are in
`sepolia-persistent/state.json`.

An earlier zero-supply deployment bound to a stale pfUSDC fixture ID was
immediately rejected and is explicitly quarantined under
`sepolia-invalid-pfusdc-id/`; it has no supply or liquidity.

## pfUSDC dust funding

The authorized 10-atom issuer transfer to
`pfab9b9228942e5c529633a13aa271d5297bec6353` was quoted successfully but
failed closed before proposal:

```text
global supply 20 exceeds finalized NAV circulating supply 10
```

No mempool or ledger mutation occurred. The live pfUSDC finalized cap is
already fully occupied and all identified 10-atom Orchard notes are spent.
Funding therefore remains blocked without a separately authorized backing
checkpoint or an identified transferable pfUSDC balance. The founder job is
not an acceptable workaround.

Evidence: `live-ce22/07-pfusdc-dust-fund/`.

## Cap-growth preservation

The deployed fleet binary hash is
`0c27df02d0e59f89deafb7f2d9d7fed96bcc714f04a1949c48d9b7a7dfc12a2c`.
All six validators run that exact binary.

The a666 candidate was rebuilt after applying the exact proof-bounded
`cap-growth-sp1-backing-20260722` delta to the a666 source. The candidate hash
recorded in the signed stage is
`7b16ed48da0ad8b8b01075887141d1f657050e8d4a98430eb592ee8fc090bc7c`.
Both the cap-growth regression and the a666 subscribe/export/refund
conservation regression passed.

## Rollout stage and hard hold

The signed candidate and its immutable active-binary backup exist on all six
hosts:

```text
postfiat-node.pre-a666-20260723  sha256 0c27df02...a2c
postfiat-node.a666-20260723      sha256 7b16ed48...c7c
```

Safe-rollout preflight is green with:

- six cloud/inventory bindings reconciled;
- six split local signers bound to one complete registry root;
- six converged RPC statuses at height 296;
- empty mempools;
- zero deletion actions;
- `applied: []`.

The mandatory signed snapshot transition remains fail-closed with
`backup.verified: false`. The active binary first rejected historical
extension-kind supersession records even though they match the original
constructor. A narrow constructor-compatible verifier regression fixed that
check, but the next audit gate rejected a historical governance batch ID.
No archived-batch validation was weakened and rollout state was not edited.
Consequently `apply-next` cannot run even if invoked.

Current fleet after all staging:

```text
validators: 6/6
height: 296
mempool pending: 0 on every validator
tip: 7bea52c025b519ed3c1f60cf9c3afd1fa11416b063b1dd40998ec9ef655da514c084790a2d45a1a9a656e4eca500bf22
state root: ae09bfefa1b870c3aacda61913c850d836395b3e6bb74c00bf62e5c28445634a8b267cd74b896f228af625ecbf418296
running binary: 0c27df02...a2c on all six
rollout applied: []
```

Evidence: `rollout-stage/rollout-state.json`.

## Verification and deferred work

- `PFTLUniswapHandoffControllerTest`: 36/36 passed.
- Safe-rollout tests: 14/14 passed.
- Governance extension-kind regression: passed.
- Sepolia readbacks: green, zero supply/liquidity.

The live primary subscription, export, wA666 mint, v4 buy/sell,
convert-back, §7.4 live reconciliation, replay attempt, and five-step live
wallet completion are intentionally deferred. They require both an explicit
rollout GO and actual pfUSDC funding. The route remains labeled
`CONTROLLED`; no Gate-5/trustless claim is made.
