# SDK Wallet Flow

Status: controlled-testnet SDK v0
Date: 2026-05-21

This runbook documents the transport-free Rust SDK path for non-operator
clients. It does not require invoking `postfiat-node` for wallet creation,
restore, or transfer signing. The SDK still leaves HTTP transport to the
embedding client so operators can choose their own retry, timeout, and endpoint
policy.

## Capability Boundary

Implemented in `postfiat-rpc-sdk`:

- deterministic wallet backup creation from a 32-byte master seed;
- public wallet identity restore from the backup;
- CLI commands for backup creation, identity restore, and quote-bound signing;
- transfer fee quote request construction;
- validated quote decoding;
- validated account summary decoding for balance polling;
- quote-bound ML-DSA transfer signing;
- signed-transfer submit request construction;
- validated mempool-submit summary decoding;
- validated receipt-list summary decoding;
- tx-finality request construction from submitted tx id;
- validated finality decoding with compact registry-root-bound certificates.

Not included in SDK v0:

- built-in HTTP client or endpoint discovery;
- secret manager, hardware wallet, or custody integration;
- exchange deposit attribution;
- account-history indexing;
- BIP39 mnemonic create/import support.

Mnemonic support is specified for the next wallet UX slice in
`docs/specs/wallet-mnemonic-design.md`. The design keeps the current
32-byte `master_seed_hex` backup and ML-DSA signing path as the internal
authority, then derives that seed from a BIP39 recovery phrase through a
PostFiat domain-separated KDF.

## Minimal Client Flow

```rust
use postfiat_rpc_sdk::{
    decode_mempool_submit_signed_transfer_summary, decode_transfer_fee_quote_summary,
    decode_tx_finality_summary, mempool_submit_signed_transfer_json_request,
    transfer_fee_quote_request, tx_finality_request_from_submit, wallet_backup_from_master_seed,
    wallet_identity_from_backup, wallet_sign_transfer_from_quote, RpcResponse,
};

fn build_signed_flow(
    quote_response: &RpcResponse,
    submit_response: &RpcResponse,
    finality_response: &RpcResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let backup = wallet_backup_from_master_seed(
        "postfiat-controlled-testnet",
        "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        0,
    )?;
    let identity = wallet_identity_from_backup(&backup)?;

    let quote_request = transfer_fee_quote_request(
        "quote-1",
        identity.address.clone(),
        "pf1-recipient",
        25,
        None,
    );
    let _quote_request_json = quote_request.to_pretty_json()?;

    let quote = decode_transfer_fee_quote_summary(quote_response)?;
    let signed = wallet_sign_transfer_from_quote(&backup, &quote)?;
    let signed_json = serde_json::to_string(&signed)?;

    let submit_request =
        mempool_submit_signed_transfer_json_request("submit-1", signed_json);
    let _submit_request_json = submit_request.to_pretty_json()?;

    let submit = decode_mempool_submit_signed_transfer_summary(submit_response)?;
    let finality_request = tx_finality_request_from_submit("tx-1", &submit);
    let _finality_request_json = finality_request.to_pretty_json()?;

    let finality = decode_tx_finality_summary(finality_response)?;
    assert_eq!(finality.tx_id, submit.tx_id);
    Ok(())
}
```

The embedding client sends each request JSON to its selected RPC endpoint and
passes the returned JSON into `RpcResponse`. The SDK validates response shape
before returning summaries.

## CLI Flow

The SDK binary exposes the same transport-free wallet path:

```bash
postfiat-rpc-sdk wallet-backup \
  --chain-id postfiat-controlled-testnet \
  --master-seed-hex 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f \
  --account-index 0 \
  --output wallet.backup.json

postfiat-rpc-sdk wallet-identity \
  --backup-file wallet.backup.json \
  --output wallet.identity.json

postfiat-rpc-sdk wallet-sign-quote \
  --backup-file wallet.backup.json \
  --quote-response transfer-fee-quote.response.json \
  --output wallet.signed-transfer.json

postfiat-rpc-sdk request \
  --method mempool_submit_signed_transfer \
  --id submit-1 \
  --signed-transfer-json-file wallet.signed-transfer.json \
  --output wallet.submit-request.json
```

Smoke evidence:

- `scripts/testnet-sdk-wallet-cli-smoke`
- `reports/testnet-sdk-wallet-cli-smoke/testnet-sdk-wallet-cli-smoke.json`
- `SIGNER_MODE=sdk scripts/testnet-wallet-sign-transfer-smoke`
- `reports/testnet-wallet-sign-transfer-smoke/sdk-signer-rpc-flow/testnet-wallet-sign-transfer-smoke.json`
- `P0_WALLET_SIGNER_MODE=sdk P0_MODE=local scripts/testnet-p0-network-gate`
- `reports/testnet-p0-network-gate-local-sdk-signer-readiness/testnet-p0-network-gate-20260514T014214Z.json`
- `scripts/testnet-wallet-receipt-packet-smoke`
- `reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-wallet-receipt-packet-20260517T034103Z.json`

`scripts/testnet-wallet-receipt-packet-smoke` is the current local operator
receipt packet. It combines SDK quote-bound signing with read-only RPC `tx`
finality, sender/recipient `account_tx_history`, and CSV exports while keeping
wallet backup/key material under `/tmp` and deleting it before final reporting.

## TCP RPC Example

The node's current `rpc-serve` surface is newline-delimited JSON over TCP. The
SDK crate includes a compile-checked wallet transport example:

```bash
cargo run -p postfiat-rpc-sdk --example tcp_wallet_flow -- \
  --quote-addr 127.0.0.1:19691 \
  --submit-addr 127.0.0.1:19692 \
  --tx-addr 127.0.0.1:19693 \
  --backup-file wallet.backup.json \
  --to pfrecipient... \
  --amount 15
```

The example derives the wallet identity from the backup, requests a fee quote,
signs the quoted transfer inside the SDK, submits the signed transfer, polls
`tx` finality, and prints a redacted summary. It bounds request and response
bytes and does not print private seed material.

## Safety Checks

- Wallet backup validation rejects unsupported schemas, algorithms, KDFs,
  derivation domains, key roles, malformed chain ids, and non-canonical master
  seeds.
- Quote-bound signing rejects a quote whose `from` address does not match the
  restored wallet identity.
- Explicit-field signing rejects chain ids that do not match the wallet backup,
  zero amount, zero fee, and zero sequence.
- Signed transfers self-verify before the SDK returns them.
- Submit summaries intentionally omit raw public key and signature fields.
- Finality summaries are only returned after full `tx` response validation,
  including receipt id matching, private-key leak rejection, and compact
  certificate registry-root checks.

## Operational Notes

- A wallet backup contains the master seed and is private material. The
  `WalletIdentity` report is public and redacts private key material.
- Future mnemonic backups must treat mnemonic words, BIP39 seed bytes,
  passphrases, and derived `master_seed_hex` as private key material.
- The SDK signs with the chain id embedded in the wallet backup. Use a separate
  backup for a different chain id.
- Public RPC write access remains operator-policy controlled. For controlled
  testnet, only submit signed transfers to endpoints that explicitly allow
  remote writes.
