# Addresses, Keys And Signatures

This page describes transparent (non-shielded) accounts on Post Fiat L1: how an
address is formed, which signature scheme is used, how keys are derived and
bound, and what the chain does and does not check. Every rule cites the code
that implements it. Shielded (Orchard) keys are covered in
[Privacy](../privacy/overview.md).

## Summary

| Item | Rule | Source |
| --- | --- | --- |
| Signature scheme | ML-DSA-65 (FIPS 204) only; algorithm id `ML-DSA-65` | `crates/crypto_provider/src/lib.rs` |
| Public key | 1,952 bytes (3,904 lowercase hex characters) | `ML_DSA_65_PUBLIC_KEY_BYTES`, same file |
| Signature | 3,309 bytes (6,618 lowercase hex characters) | `ML_DSA_65_SIGNATURE_BYTES`, same file |
| Transaction signing context | `postfiat-l1-v2/tx/v1` (ML-DSA context string) | `TX_SIGNATURE_CONTEXT`, same file |
| Hash function | SHA3-384 with a domain prefix: `SHA3-384(domain ‖ 0x00 ‖ bytes)` | `hash_bytes`, same file |
| Address | `pf` + hex of the first 20 bytes of `SHA3-384("postfiat.address.v1" ‖ 0x00 ‖ public key)`; 42 characters | `address_from_public_key`, same file |
| Address checksum | Optional mixed-case display checksum; on-chain addresses stay lowercase | `crates/crypto_provider/src/address.rs` |
| Recipient format check on transfers | Noncanonical recipients are rejected at RPC submission and mempool admission; execution applies the rule only after `recipient_format_v2_activation_height` | `crates/node/src/mempool_proposals.rs`, `crates/types/src/shielded_bridge_governance.rs` |
| Key binding | The first spend stores the public key; later spends must use the same key | `crates/execution/src/entrypoints.rs` |
| Key rotation for accounts | Not implemented | [Account Key Rotation Boundary](../specs/account-key-rotation-boundary.md) |

Native transfers have no classical (secp256k1 or Ed25519) signature option: a
transfer whose `signature_algorithm_id` is not `ML-DSA-65` is rejected with
`unsupported_signature_algorithm` (`execute_transfer` in
`crates/execution/src/entrypoints.rs`).

## Address format

```text
address = "pf" + lowercase_hex( SHA3-384( "postfiat.address.v1" || 0x00 || public_key ) [0..20] )
```

- The result is always `pf` followed by 40 lowercase hex characters, for
  example `pf857c81edb95af1d64262ed6c0fdcf3ef7aff56fe` (the public test vector in
  [Transparent Transaction Envelope](../specs/transparent-transaction-envelope.md)).
- There is no network byte. The same public key gives the same address on every
  chain; signed `chain_id` and `genesis_hash` fields bind transactions to a network.
- An optional mixed-case checksum is available for display and input. Parsers
  accept the legacy lowercase form or the exact checksummed form, and normalize
  to lowercase before signing. On-chain addresses remain lowercase.

### Recipient validation

RPC submission, mempool admission and the wallet SDK reject noncanonical
recipient fields. Execution enforces the same rule only after governance
activates `recipient_format_v2_activation_height`; historical execution stays
unchanged before that height. This source is in the running build
(`04600868`); whether governance has set the height on the devnet is not
recorded here.

Use the published parser and checksum rather than lowercasing arbitrary input:
normalizing before checking the checksum would hide a mistyped mixed-case address.
The format, error codes, language implementations and shared test vectors are in
[Addresses And Checksums](../wallets/address-format.md). Lowercase legacy inputs
remain accepted without a checksum, so a well-formed lowercase typo can still
pass; integrators should use their normal withdrawal confirmation controls.

## Keys and derivation

### Signature scheme

Accounts and validators sign with ML-DSA-65 through the `fips204` crate
(`crates/crypto_provider/src/lib.rs`). Signing passes the context string
`postfiat-l1-v2/tx/v1` for transactions; block certificates, bridge witnesses
and admission receipts use their own context strings in the same file. See
[Quantum Authorization](../quantum/authorization.md) and
[Signature Size And Certificates](../quantum/signature-size-and-certificates.md)
for the design reasons and costs.

### Wallet seed derivation (Rust SDK and browser wallet)

The Rust SDK (`crates/rpc_sdk/src/wallet_sdk.rs`) and the browser wallet's
WebAssembly module (`crates/wallet_wasm`, which calls the same SDK functions)
derive one ML-DSA-65 key pair per backup record:

| Backup field | Value |
| --- | --- |
| `schema` | `postfiat-wallet-backup-v1` |
| `algorithm_id` | `ML-DSA-65` |
| `kdf` | `sha3-384-domain-truncate32` |
| `derivation_domain` | `postfiat.wallet.seed.v1` |
| `chain_id` | the network's chain ID |
| `account_index` | integer |
| `key_role` | `transparent-spend` |
| master seed | 32 secret bytes (hex) |

```text
payload = JSON array [derivation_domain, algorithm_id, chain_id, account_index, key_role, master_seed_hex]
seed32  = first 32 bytes of SHA3-384( "postfiat.wallet.seed.v1" || 0x00 || payload )
keypair = ML-DSA-65 KeyGen(seed32)        # FIPS 204 deterministic key generation
```

Constants are in `crates/rpc_sdk/src/protocol_requests.rs`; the derivation is
`derive_wallet_seed` in `crates/rpc_sdk/src/wallet_sdk.rs`.

Consequences for custody:

- **Derivation is chain-scoped.** The same master seed and account index give a
  different key, and therefore a different address, under a different
  `chain_id`. Back up the chain ID with the seed.
- **There is no public (watch-only) derivation.** ML-DSA has no BIP32-style
  extended public key. Each deposit address must be generated inside the custody
  boundary and then registered for watching by address
  ([Wallet Exchange Custody Model](../specs/wallet-exchange-custody-model.md)).
- **Mnemonic phrases are a design, not a protocol rule.** BIP39 phrase support
  is specified in [Wallet Mnemonic Design](../specs/wallet-mnemonic-design.md)
  and [Recovery Phrase Design](../wallets/recovery-phrase-design.md) as a
  wallet layer over the same master seed. Confirm what the wallet you use
  actually implements.

The backup file holds the master seed in plaintext JSON. It is a
controlled-testnet compatibility format, not a production custody format
([Plaintext Key File Boundary](../specs/plaintext-key-file-boundary.md)).

### Key binding at first spend

An address can receive funds before its public key appears on chain. Then
(`execute_transfer` in `crates/execution/src/entrypoints.rs`):

1. Every spend carries the full public key. Execution derives the address from
   it and rejects the transaction with `sender_mismatch` if it is not the `from`
   address.
2. The first accepted spend stores `public_key_hex` on the account.
3. Later spends must carry the same public key, or they are rejected with
   `sender_key_mismatch`.

There is no transaction that replaces an account key. If a key is lost, the
funds are lost. If a key may be compromised, sweep the funds to a fresh address
with an ordinary transfer
([Account Key Rotation Boundary](../specs/account-key-rotation-boundary.md)).
Validator key rotation is a separate governance process
([Emergency Key Rotation](../validators/emergency-key-rotation.md)).

## What a transfer signs

A native transfer signs UTF-8 text with one `key=value` field per line. The
order is consensus-critical
([Transparent Transaction Envelope](../specs/transparent-transaction-envelope.md);
`UnsignedTransfer::signing_bytes` in
`crates/types/src/transactions_mempool_receipts.rs`):

```text
postfiat.transfer.v1
chain_id={chain_id}
genesis_hash={genesis_hash}
protocol_version={protocol_version}
address_namespace=postfiat.address.v1
transaction_kind=transparent_transfer
signature_algorithm_id=ML-DSA-65
from={from}
to={to}
amount={amount}
fee={fee}
sequence={sequence}
```

- **Transaction ID.** `SHA3-384("postfiat.tx_id.v1" ‖ 0x00 ‖ signing bytes ‖
  "algorithm=…\npublic_key=…\nsignature=…\n")`, as 96 hex characters
  (`transfer_tx_id` in `crates/execution/src/tx_hashing.rs`).
- **Replay protection.** Execution rejects a wrong `chain_id`, `genesis_hash`,
  `protocol_version`, `address_namespace` or `transaction_kind` before touching
  balances, and requires `sequence` to equal the account's stored sequence
  plus one (`bad_sequence`).
- **No expiry.** The v1 envelope has no expiry field. A signed transfer stays
  valid until its sequence number is used.
- **No memo.** v1 transfers cannot carry a memo or destination tag.

`payment_v2` is the memo-capable native payment. It signs a
`postfiat.payment.v2` envelope with the same fields plus `memo_count` and, per
memo, the type, format and data with their byte lengths. Its transaction ID uses
the domain `postfiat.payment_v2.tx_id.v1`. Limits (`crates/types/src/core_chain.rs`):

| Memo limit | Value |
| --- | --- |
| Memos per payment | 4 |
| `memo_type` | 64 bytes |
| `memo_format` | 64 bytes |
| `memo_data` | 256 bytes |
| All memos together | 512 bytes |

Memo fields are lowercase hex in the envelope, and a memo must have at least one
non-empty field (`PaymentMemo::validate` in
`crates/types/src/transactions_mempool_receipts.rs`).

## Related pages

- [Exchange Integration Guide](exchange-guide.md): fees, deposits, withdrawals,
  confirmation.
- [RPC Methods](../rpc/methods.md) and [Python Wallet Functions](../python/wallet-functions.md).
- [Wallet Exchange Custody Model](../specs/wallet-exchange-custody-model.md).
