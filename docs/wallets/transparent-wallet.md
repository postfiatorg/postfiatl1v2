# Transparent Wallet

Transparent wallet flows use post-quantum account authorization.

## Current Capabilities

- account generation and status inspection;
- fee quote;
- local signing;
- SDK wallet flow;
- controlled write submission;
- transaction finality through read RPC;
- receipt and account history lookup.

Recovery phrase support is planned as a BIP39 UX layer over the same ML-DSA
wallet backup path. It should generate 24-word phrases by default, import
common BIP39 phrases, and preview MetaMask/Phantom-compatible addresses without
claiming those wallets can sign native PostFiat transactions.

## Wallet Flow

```mermaid
flowchart LR
  Seed[Seed or recovery phrase]
  Keygen[Derive ML-DSA account key]
  Restore[Restore account address<br/>and local wallet metadata]
  Quote[Fetch fee quote<br/>and account sequence]
  Sign[Sign transfer offline<br/>amount, recipient, fee, sequence]
  Submit[Submit signed transfer<br/>controlled write RPC]
  Finality[Query transaction finality<br/>receipt and block certificate]
  History[Read account history<br/>and updated balance]

  Seed --> Keygen --> Restore --> Quote --> Sign --> Submit --> Finality --> History
  Restore --> Quote
```

## Evidence

- `scripts/testnet-wallet-test-vectors-smoke`
- `scripts/testnet-wallet-minimum-smoke`
- `scripts/testnet-live-wallet-finality`
- `reports/testnet-live-wallet-finality/`

## Source

- `crates/rpc_sdk/src/lib.rs`
- `crates/rpc_sdk/examples/tcp_wallet_flow.rs`
- `docs/runbooks/sdk-wallet-flow.md`
