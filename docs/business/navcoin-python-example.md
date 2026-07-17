# NAVCOIN Python Example

> The verification-layer builders (`build_profile_register_operation`,
> `build_attestor_register_operation`, `build_reserve_attest_operation`,
> `build_redeem_settle_operation`) and the source adapters
> (`hyperliquid.py`, `solana.py`, `basis_policy.py`) are documented in
> [NAVCOIN Proof of Reserves](navcoin-proof-of-reserves.md).

This example shows the NAVCOIN mechanics without private keys:

1. compute verified net assets;
2. compute NAV per unit;
3. build the reserve packet and roots;
4. build the native operation JSON for registration, reserve submission,
   finalization, minting, and redemption.

Run:

```bash
PYTHONPATH=python python3 docs/examples/navcoin_mint_and_nav.py
```

The default numbers are deliberately simple:

```text
cash                    622,300,000 micro-USD
broker positions        400,000,000 micro-USD
liabilities              40,000,000 micro-USD
pending redemptions               0 micro-USD
circulating supply             1,000 NAV
```

So:

```text
verified net assets = 622,300,000 + 400,000,000 - 40,000,000
                    = 982,300,000 micro-USD

NAV per unit        = 982,300,000 / 1,000
                    = 982,300 micro-USD
                    = $0.982300
```

The protocol path then uses that finalized value:

```text
reserve packet -> nav_reserve_submit -> nav_epoch_finalize
finalized NAV  -> nav_mint_at_nav
holder exit    -> nav_redeem_at_nav
```

## Output Shape

The script emits:

- `calculation`: gross assets, liabilities, net assets, and NAV per unit;
- `reserve_packet`: the canonical packet that gets hashed;
- `source_root`: root over source inputs;
- `attestor_root`: root over attestor/proof profile metadata;
- `operations`: native transaction operation objects.

The important invariant is:

```text
verified_net_assets == circulating_supply * nav_per_unit
```

The current native operation rejects reserve submissions that do not satisfy
that invariant exactly. For production, either use a small enough valuation unit
or define a governed rounding policy before finalization.

## Submitting The Operations

The Python wallet layer already has a generic asset-operation submitter. A
real devnet submission looks like this:

```python
from postfiat_rpc import PostFiatRpcClient, create_wallet, submit_asset_transaction
from postfiat_rpc.navcoin import NavInputs, build_packet_and_operations

client = PostFiatRpcClient("127.0.0.1:27650")

issuer = create_wallet(
    chain_id="postfiat-navcoin-devnet",
    wallet_dir="wallets/nav-issuer",
)
ap = create_wallet(
    chain_id="postfiat-navcoin-devnet",
    wallet_dir="wallets/nav-ap",
)

packet = build_packet_and_operations(
    NavInputs(
        issuer=issuer.address,
        ap_account=ap.address,
        asset_code="NAV",
        epoch=1,
        circulating_supply=1_000,
        mint_amount=1_000,
        redeem_amount=10,
        cash_micro_usd=622_300_000,
        broker_positions_micro_usd=400_000_000,
        liabilities_micro_usd=40_000_000,
        pending_redemptions_micro_usd=0,
        proof_profile="local-nitro-placeholder-v0",
    )
)

ops = packet["operations"]

submit_asset_transaction(client, wallet=issuer, operation=ops["asset_create"])
submit_asset_transaction(client, wallet=issuer, operation=ops["nav_asset_register"])
submit_asset_transaction(client, wallet=ap, operation=ops["ap_trustline"])
submit_asset_transaction(client, wallet=issuer, operation=ops["nav_reserve_submit"])
submit_asset_transaction(client, wallet=issuer, operation=ops["nav_epoch_finalize"])
submit_asset_transaction(client, wallet=issuer, operation=ops["nav_mint_at_nav"])
submit_asset_transaction(client, wallet=ap, operation=ops["nav_redeem_at_nav"])
```

In the full smoke test, each submitted transaction is sealed into a local batch
and replayed across the validator data directories. That test is:

```bash
scripts/navcoin-current-infra-smoke
```

The smoke also demonstrates NAV/PFT secondary liquidity through the existing
offer book. The offer book is useful for trading convenience; AP mint/redeem
and finalized reserve state are the NAV correctness path.
