# Python Client API

The source is `python/postfiat_rpc/client.py`.

## Primary Operations

The client supports bounded reads plus gated wallet write helpers:

- status;
- server information;
- ledger;
- fee;
- transfer fee quotes;
- signed transparent transfer submit;
- issued-asset fee quotes and signed transaction submit;
- escrow fee quotes and signed transaction submit;
- NFT fee quotes and signed transaction submit;
- DEX offer fee quotes and signed transaction submit;
- asset, trustline, escrow, NFT, and offer read methods;
- atomic settlement template construction;
- gated Orchard deposit batch creation;
- validators;
- receipts;
- transaction finality;
- account state;
- account transaction history;
- FastPay owned-object lookup, wrap, transfer vote/apply, and signed standard
  unwrap vote/apply;
- PFTL-to-Uniswap NAVCoin bridge route, packet, claim, supply, and receipt
  replay reads;
- Orchard public pool report.

## Design Notes

- Keep request sizes bounded.
- Treat RPC errors as data.
- Preserve response validation.
- Keep private keys local. Python wallet helpers call the Rust node/SDK binaries
  for ML-DSA signing and Orchard/Halo2 action creation, then submit signed or
  public action JSON over RPC.

## Wallet Helpers

`postfiat_rpc.wallet` exports:

- `create_wallet()`
- `request_faucet_pft()`
- `send_pft()`
- `send_payment()`
- `wrap_fastpay()`, `send_fastpay()`, and `unwrap_fastpay()`
- `create_issued_asset()` and `mint_token()`
- `create_asset_trustline()` and `set_trustline()`
- `authorize_trustline()`, `freeze_trustline()`,
  `unfreeze_trustline()`, and `revoke_trustline_authorization()`
- `send_issued_asset()` and `send_token()`
- `clawback_token()`
- `create_escrow()`, `finish_escrow()`, and `cancel_escrow()`
- `build_atomic_swap_template()`
- `mint_non_fungible_token()`, `transfer_non_fungible_token()`, and
  `burn_non_fungible_token()`
- `place_offer()` and `cancel_offer()`
- `create_orchard_wallet()`
- `send_shielded_pft()`
- `scan_orchard_wallet()`

The XRP-style helper names are convenience wrappers over the canonical
PostFiat transaction flow. They do not bypass quote-bound signing, sequence
selection, mempool submit, or optional local finalization.

FastPay signed operations are available under normal RPC startup. An operator
can explicitly disable the lane during an incident; clients must inspect
`owned_lane_enabled` and hide mutation controls when it is false. Wallet keys
remain client-side. `wrap_fastpay()` locally signs `OwnedDepositV1`, submits it
through consensus, and requires the exact `owned_deposit_applied` accepted
receipt. `send_fastpay()` and `unwrap_fastpay()` require the governed v3
recovery capability, bind its exact domain/committee/window, sign through the
Rust SDK, collect a distinct-validator certificate, and verify a governed
quorum of signed durable-apply acknowledgements. An unavailable recovery
capability fails closed before signing. The legacy unsigned `wrap_owned` and
`unwrap_owned` RPC methods are not exposed by the Python client.

For full transaction examples, see
[XRP-Style Python Transactions](xrp-style-transactions.md). For compact helper
signatures, see [Python Wallet Functions](wallet-functions.md).

## Relation To Rust SDK

The Rust SDK in `crates/rpc_sdk` is the protocol-near client surface. The Python
client is the integration and analysis surface for operators and app engineers.
