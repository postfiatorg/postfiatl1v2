# Local Anvil USDC ↔ proven-NAV NAVcoin HTLC lane

This test-only lane pairs a minimal ERC-20 HTLC with hardened-PFTL escrow using
the same `H = SHA256(32-byte preimage)`. It demonstrates both directions,
refunds, public preimage recovery, exact conservation, and mutation-free
adversarial probes.

The demonstrated claim is:

> non-custodial, conditionally-atomic

## Test networks

- Anvil chain ID `31337`, persistent RPC `http://127.0.0.1:39545`
- `MockUSDC` has 6 decimals and exactly 2,000,000 test atoms
- Hardened PFTL RPC `tcp://127.0.0.1:31660` through `:31665`
- Hardened revision `ae3c53c9`
- NAVcoin asset:
  `f912599013445352dc064b8b07be3815db5f494eff7e7097b2d6a72ff333bbfcaf51954e35fe28558525541f5fb945b5`

No Sepolia or Ethereum mainnet, real USDC, real value, GPU work, or ce22 state
is used.

## Contract

`USDCNavHTLC` stores an immutable mock-USDC token and one state machine per
swap ID. `lock` transfers the exact principal into the contract. `redeem`
requires `block.timestamp < refundTime` and an exact SHA-256 preimage. It emits
the preimage publicly before the paired PFTL finish uses it. `refund` requires
`block.timestamp >= refundTime` and the original locker. State is changed
before the ERC-20 transfer, and terminal swaps reject duplicates.

The first locker receives the longer timeout; the second locker receives the
shorter timeout; the second mover claims first. EVM wall time and PFTL block
height have no protocol-enforced relationship, so configured margins,
monitoring, and transaction liveness remain operational requirements.

## Verify

Run contract tests:

```bash
/home/postfiat/.foundry/bin/forge test -vv
```

Run the live scenario against the already deployed persistent Anvil instance:

```bash
python3 -m tools.usdc_navcoin_demo.run_live_demo \
  --runtime-root /home/postfiat/tmp/pftl-usdc-navcoin-anvil-20260725
```

Independently verify compiled deployment bytecode and constructor arguments,
receipts, event preimages, live contract state, exact mock-USDC conservation,
all six PFTL ledgers, proven NAV, terminal escrows, each of the six
case-sensitive `accepted` PFTL receipt codes with exact principal deltas, and
exact NAVcoin conservation:

```bash
python3 -m tools.usdc_navcoin_demo.verify_evidence \
  --runtime-root /home/postfiat/tmp/pftl-usdc-navcoin-anvil-20260725
```

The result is written to:

```text
/home/postfiat/tmp/pftl-usdc-navcoin-anvil-20260725/public/evidence/independent-verification.json
```

The nazgul-verified XRP reference bundle remains at:

```text
/home/postfiat/tmp/pftl-xrpl-navcoin-20260724/public/evidence/independent-verification.json
```
