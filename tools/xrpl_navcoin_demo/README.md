# XRP ↔ proven-NAV NAVcoin prototype

This is the second wallet ingress lane, parallel to BTC/Lightning. It uses
XRPL's native PREIMAGE-SHA-256 escrow and hardened PFTL escrow with the exact
same 32-byte secret and SHA-256 hash.

The demonstrated claim is deliberately limited to:

> non-custodial, conditionally-atomic, coordinator-trusted timing

The two ledgers do not prove a relationship between XRPL validated-ledger
close time and PFTL block height. The coordinator therefore enforces the
cross-clock safety margin and refuses stale or inverted plans.

## Live networks

- XRPL Testnet JSON-RPC: `https://s.altnet.rippletest.net:51234`
- PFTL devnet RPC: `tcp://127.0.0.1:31660` through `:31665`
- PFTL chain: `local-pftl-proven-nav-v2-20260724`
- Hardened binary revision: `ae3c53c9`
- Hardened binary SHA-256:
  `006167226531582cf81666dded004f26707beedc2ce3fa850caf5b0b82fd22e7`
- NAVcoin asset:
  `f912599013445352dc064b8b07be3815db5f494eff7e7097b2d6a72ff333bbfcaf51954e35fe28558525541f5fb945b5`

No mainnet, real money, GPU work, or ce22 state is used.

## Wallet flow

For XRP → NAVcoin:

1. Wallet generates a random 32-byte `S` and sends only `H=SHA256(S)` to the
   coordinator.
2. User creates the first/long XRPL escrow to the coordinator with
   `Condition=A0258020 || H || 810120` and `CancelAfter`.
3. Coordinator waits for validated `tesSUCCESS`, applies its cross-clock gate,
   and locks NAVcoin atoms in the second/short PFTL escrow to the user with the
   same condition bytes in canonical lowercase.
4. User finishes PFTL first with `a0228020 || S`.
5. Coordinator extracts/authenticates `S` and finishes XRPL.
6. If no claim occurs, PFTL refunds first, then XRPL refunds at its longer
   boundary.

For NAVcoin → XRP:

1. User locks NAVcoin first/long to the coordinator.
2. After certified finality and the cross-clock gate, the coordinator locks
   faucet XRP second/short to the user.
3. User finishes XRPL first. `EscrowFinish.Fulfillment` publishes `S`.
4. Coordinator authenticates that public preimage against the original
   condition and finishes the PFTL escrow.
5. The short XRPL refund precedes the long PFTL refund if no claim occurs.

Signed intents are durable before submission. Only validated `tesSUCCESS` is
accepted on XRPL. A durable idempotency key suppresses duplicate external
effects; unresolved intents require ledger reconciliation instead of blind
resubmission.

## Verification

The runtime evidence is:

`/home/postfiat/tmp/pftl-xrpl-navcoin-20260724/public/evidence`

Run:

```bash
.venv-xrpl-nav/bin/python -m tools.xrpl_navcoin_demo.verify_evidence \
  --runtime-root /home/postfiat/tmp/pftl-xrpl-navcoin-20260724
```

The verifier checks all six PFTL ledgers, the pinned binary hash, finalized NAV
checkpoint, terminal escrow states, exact NAVcoin atom conservation, all six
public XRPL transaction hashes, both public preimages, no open XRPL escrows,
and exact XRP conservation after validated fees.

Adversarial wrong-preimage, early-cancel, and late-finish tests are deliberately
rejected before signing or submission. That is what “mutation-free” means:
submitting a failing XRPL transaction would itself burn a fee and mutate the
account sequence.

