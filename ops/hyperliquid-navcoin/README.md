# Hyperliquid NAVCoin on Solana Reserve Attestation Bridge (`ops/hyperliquid-navcoin/`)

Wires `HyperCoreReserveReader` (`crates/ethereum-contracts/src/HyperCoreReserveReader.sol`) reserve snapshot verification and non-issuer Ed25519 attestor signature validation to Solana NAVCoin swap readiness and a deterministic dust-swap rehearsal runner.

## What this enforces (fail-closed)

1. **`HyperCoreReserveReader` receipt & accounting verification (`hypercore_verifier.py`):**
   - Verifies policy-pinned `reader_contract`, deployed `reader_code_hash`, and `HyperCoreSnapshot(bytes32,uint64,bytes)` event topic0.
   - Verifies `keccak256(abi.encodePacked(payload, salt)) == commitment`.
   - Decodes the exact Solidity ABI payload and reproduces `tools/nav-reserve-proof/crates/reserve-proof-types/src/hyperliquid_receipt.rs` accounting:
     - `marginSummary.accountValue >= 0` and `>= withdrawable`.
     - Exact ordered perp set with `sum(|szi| * markPx * 100) == marginSummary.ntlPos * 100` (cannot hide unlisted perps).
     - Exact ordered spot token set (`150` HYPE, `404` XMR1, `weiDecimals = 8`), `hold <= total`, floor USD-e8 valuation.
   - Computes universal Pillar 7 / C4 exact floor NAV: `floor(V * 10^precision / S)`.
2. **Non-issuer attestor quorum (`attestor_verifier.py`):**
   - Rejects issuer self-attestation (`attestor == asset_issuer` -> `nav_fee_issuer_self_attestation_forbidden`).
   - Rejects unregistered attestors, duplicate votes, `pass=False` votes, stale votes, and invalid Ed25519 signatures.
   - Emits canonical `nav_reserve_attest` operations matching `python/postfiat_rpc/navcoin.py`.
3. **Solana swap readiness & dust-swap rehearsal (`solana_swap_bridge.py`):**
   - Mirrors `ops/sol-live/mainnet-health.py` fail-closed rules: epoch match, 12-minute epoch-boundary closure, observer lag `<= 150` slots, profile hash match, prover & relay healthy.
   - Enforces dust ceiling (`<= 10.00 USDC` default) and max spread bps before outputting a deterministic `postfiat.hyperliquid_solana_nav_rehearsal.v1` plan and L1 operation bundle.

## Usage & Tests

```bash
# Run deterministic unit & fixture tests
PYTHONPATH=ops/hyperliquid-navcoin pytest ops/hyperliquid-navcoin/tests/

# Verify a deterministic bundle via CLI
PYTHONPATH=ops/hyperliquid-navcoin python3 -m hyperliquid_navcoin.cli rehearse-dust-swap \
  --bundle ops/hyperliquid-navcoin/fixtures/deterministic_fixture.json
```
