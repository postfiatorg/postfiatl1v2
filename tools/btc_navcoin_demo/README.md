# Bitcoin regtest ↔ proven-NAV NAVcoin HTLC lane

This lane follows the nazgul-verified XRP lane's evidence format while replacing
XRPL native escrow with a Bitcoin P2WSH contract:

```text
OP_IF
  OP_SHA256 <H> OP_EQUALVERIFY
  <claim-pubkey> OP_CHECKSIG
OP_ELSE
  <absolute-height> OP_CHECKLOCKTIMEVERIFY OP_DROP
  <refund-pubkey> OP_CHECKSIG
OP_ENDIF
```

The paired hardened-PFTL escrow uses the same `H = SHA256(32-byte preimage)`.
The demonstrated claim is:

> non-custodial, conditionally-atomic

## Test-only networks

- Bitcoin Core `v31.0.0`, isolated regtest at `http://127.0.0.1:18443`
- Funding: locally mined regtest coinbase, then an exact 100,000-sat funding UTXO
- Hardened PFTL devnet: `tcp://127.0.0.1:31660` through `:31665`
- Hardened revision: `ae3c53c9`
- NAVcoin:
  `f912599013445352dc064b8b07be3815db5f494eff7e7097b2d6a72ff333bbfcaf51954e35fe28558525541f5fb945b5`

No Bitcoin mainnet, real-value asset, GPU work, or ce22 state is used.

## Timing and atomicity boundary

The first locker receives the longer timeout and the second locker the shorter
timeout. The second mover claims first, publishing the preimage for the other
ledger.

Bitcoin and PFTL block heights have no protocol-enforced relationship. The
configured margins therefore assume active monitoring and transaction-fee
liveness. A standard Bitcoin HTLC has an additional, important property: after
CLTV maturity the hash branch remains valid until the refund actually spends
the output. The refund transaction closes that race; this prototype does not
claim otherwise.

## Run and independently verify

The live runner creates an isolated Core wallet, mines regtest maturity blocks,
and creates an exact confirmed funding UTXO before mutating either ledger:

```bash
python3 -m tools.btc_navcoin_demo.run_live_demo \
  --runtime-root /home/postfiat/tmp/pftl-btc-navcoin-regtest-v2-20260725
```

Independent verification reads transaction bytes back from Bitcoin Core,
validates the P2WSH scripts and witness preimages, obtains merkle inclusion
proofs from the regtest node, compares all six PFTL ledgers, checks the
proven-NAV checkpoint, terminal escrow states, and exact satoshi/NAVcoin
conservation:

```bash
python3 -m tools.btc_navcoin_demo.verify_evidence \
  --runtime-root /home/postfiat/tmp/pftl-btc-navcoin-regtest-v2-20260725
```

The output is:

```text
/home/postfiat/tmp/pftl-btc-navcoin-regtest-v2-20260725/public/evidence/independent-verification.json
```

The reference XRP bundle is:

```text
/home/postfiat/tmp/pftl-xrpl-navcoin-20260724/public/evidence/independent-verification.json
```
