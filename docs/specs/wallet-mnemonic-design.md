# Wallet Mnemonic Creation And Import Design

Status: design v0
Date: 2026-05-21

This document defines the mnemonic support PostFiat should add on top of the
current transparent ML-DSA wallet flow. It is a wallet UX and key-derivation
design, not a protocol transaction change.

## Current State

Transparent wallet creation currently starts from a 32-byte
`master_seed_hex`. Python `create_wallet()` generates a random seed with
`secrets.token_hex(32)` when the caller does not provide one, then invokes
`postfiat-node wallet-keygen`. The Rust SDK validates the wallet backup and
derives the ML-DSA spend key from:

- `postfiat.wallet.seed.v1`;
- algorithm id;
- chain id;
- account index;
- key role;
- the 32-byte master seed.

This path is deterministic and already used by the SDK quote-bound signing
flow. Mnemonic support should feed this path instead of replacing it.

## Goals

- Give users a familiar 12/24-word wallet backup UX.
- Default new PostFiat wallets to 24-word BIP39 English mnemonics.
- Allow import of common BIP39 phrases from MetaMask and Phantom users.
- Preserve the current ML-DSA signing model and domain-separated PostFiat key
  derivation.
- Let one recovery phrase show PostFiat, EVM, and Solana account previews so a
  user can recognize the phrase they are importing.
- Avoid implying that MetaMask or Phantom can sign native PostFiat
  transactions without a future wallet integration.

## Non-Goals

- No consensus or transaction serialization change.
- No reuse of XRP coin type `144'`; it belongs to XRP in SLIP-44.
- No claim that MetaMask or Phantom can natively custody PostFiat ML-DSA keys.
- No browser extension, Snap, hardware wallet, or account key-rotation design
  in this document.
- No plaintext mnemonic storage in normal reports, logs, or public docs.

## Standards To Support

### BIP39

PostFiat should support BIP39 English mnemonics:

- generate 24 words by default;
- import 12, 15, 18, 21, and 24 words;
- validate wordlist membership and checksum;
- normalize mnemonic and passphrase with UTF-8 NFKD;
- derive the BIP39 seed using PBKDF2-HMAC-SHA512 with 2048 iterations;
- support an optional BIP39 passphrase as an advanced feature.

The default UX should not ask for a passphrase. A passphrase creates a separate
wallet from the same words and creates avoidable recovery risk for ordinary
users.

### BIP44 Shape

PostFiat should expose BIP44-shaped account paths for human compatibility:

```text
m / purpose' / coin_type' / account' / change / address_index
```

PostFiat must request a real SLIP-44 coin type before public mnemonic support
is considered final. Until then, controlled-testnet builds may use an explicit
`postfiat-controlled-testnet` derivation profile, but the generated backup must
record that profile so it cannot be mistaken for a public mainnet derivation.

Recommended native PostFiat path after assignment:

```text
m/44'/POSTFIAT_COIN_TYPE'/account'/0/address_index
```

Because PostFiat transparent accounts use ML-DSA, this path is metadata into a
PostFiat KDF. It is not secp256k1 or ed25519 BIP32 public-child derivation.

## Native PostFiat KDF

Keep the current 32-byte `master_seed_hex` as the internal input to existing
SDK wallet backup and signing code. Add a deterministic BIP39-to-PostFiat
master-seed conversion:

```text
postfiat_master_seed = first_32_bytes(hash_bytes(
  domain = "postfiat.wallet.bip39.master_seed.v1",
  canonical_payload = [
    bip39_seed_64,
    chain_id,
    derivation_profile,
    postfiat_path,
    account_index,
    address_index
  ]
))
```

`hash_bytes` should use the same domain-separated SHA3-384 helper already used
by the Rust crypto provider. The 32-byte output becomes the existing
`master_seed_hex`. The wallet backup then uses the same validation, identity,
and quote-bound signing flow that exists today.

The backup should add metadata fields in a versioned way:

- `seed_source`: `random_master_seed_hex` or `bip39`;
- `mnemonic_fingerprint`: domain-separated hash of the BIP39 seed, not the
  phrase;
- `derivation_profile`: for example `postfiat-slip44-v1` or
  `postfiat-controlled-testnet-v1`;
- `derivation_path`: full path string;
- `address_index`;
- `bip39_language`: `english`;
- `bip39_passphrase_used`: boolean only, never the passphrase.

The backup remains private because it contains `master_seed_hex`. A separate
encrypted recovery packet may store the mnemonic later, but plaintext mnemonic
storage should not be added to normal wallet reports.

## MetaMask Compatibility

MetaMask seed-phrase import supports the default EVM BIP44 path family:

```text
m/44'/60'/0'/0/index
```

PostFiat should support this as a preview/linking profile:

- derive the EVM address for indexes 0 through 19;
- show it during mnemonic import so users recognize the phrase;
- allow the user to bind/prove an EVM account separately if product UX needs
  account linking.

This profile must not be used to sign native PostFiat transactions. It derives
secp256k1 EVM keys, while PostFiat transparent spending uses ML-DSA keys.

## Phantom Compatibility

Phantom scans three recovery-phrase derivation path groupings. PostFiat import
should preview the same groups for user recognition:

### `bip44change`

```text
Solana: m/44'/501'/index'/0'
EVM:    m/44'/60'/0'/0/index
```

Optional display-only Bitcoin paths for users migrating from Phantom:

```text
SegWit:  m/84'/0'/0'/0/index
Taproot: m/86'/0'/0'/0/index
```

### `bip44`

```text
Solana: m/44'/501'/index'
EVM:    m/44'/60'/1'/0/index
```

### Deprecated

```text
Solana: m/501'/index'/0/0
EVM:    m/44'/60'/2'/0/index
```

PostFiat should scan the first 20 indexes for each grouping during import and
present derived public addresses. It should not need Ethereum, Solana, or
Bitcoin RPC access to derive these addresses; activity discovery can be added
later as an optional wallet-service feature.

## User Flows

### Create New Wallet

1. Generate 256 bits of entropy.
2. Encode a 24-word BIP39 English phrase.
3. Derive the BIP39 seed.
4. Derive the PostFiat 32-byte master seed using the native PostFiat KDF.
5. Create the normal wallet backup and key file.
6. Show the recovery phrase once and require user confirmation.
7. Store derivation metadata in the private backup.

### Import Existing Phrase

1. Accept a phrase through an interactive prompt, stdin, or a restricted file.
2. Normalize and validate BIP39.
3. Ask for optional passphrase only through an advanced flow.
4. Preview PostFiat account candidates plus MetaMask and Phantom-compatible
   addresses for indexes 0 through 19.
5. Let the user select the intended PostFiat account.
6. Write the normal private backup and key file.
7. Redact all mnemonic and BIP39 seed material from output.

### Recover Existing PostFiat Wallet

1. User enters the phrase and optional passphrase.
2. Wallet uses stored derivation metadata if available.
3. If metadata is absent, default to the public PostFiat derivation profile and
   scan first 20 address indexes.
4. Display candidate PostFiat addresses and let the user choose.
5. Restore the normal `WalletBackupFile` and verify public identity.

## CLI And Python API

Add SDK commands:

```bash
postfiat-rpc-sdk wallet-mnemonic-generate \
  --words 24 \
  --language english \
  --output wallet.mnemonic.recovery.json

postfiat-rpc-sdk wallet-backup-from-mnemonic \
  --chain-id postfiat-controlled-testnet \
  --mnemonic-stdin \
  --account-index 0 \
  --address-index 0 \
  --output wallet.backup.json

postfiat-rpc-sdk wallet-mnemonic-preview \
  --mnemonic-stdin \
  --profiles postfiat,metamask,phantom \
  --scan-indexes 20 \
  --output wallet.preview.redacted.json
```

Avoid accepting mnemonic words as ordinary command-line arguments because shell
history and process listings can retain them.

Add Python helpers:

```python
create_wallet_from_mnemonic(
    chain_id="postfiat-controlled-testnet",
    wallet_dir="wallets/alice",
    mnemonic_words=[...],
    account_index=0,
    address_index=0,
)

preview_mnemonic_accounts(
    mnemonic_words=[...],
    profiles=("postfiat", "metamask", "phantom"),
    scan_indexes=20,
)
```

The existing `create_wallet(master_seed_hex=...)` path should remain for
deterministic fixtures, custody integrations, and controlled testnet automation.

## Security Rules

- Mnemonic words, BIP39 seed bytes, passphrases, and derived master seed bytes
  are private key material.
- Reports must include `mnemonic_redacted: true` rather than any phrase words.
- Redaction checks must reject `mnemonic`, `seed_phrase`, `master_seed_hex`,
  and BIP39 seed fields in public reports.
- Test fixtures must clearly mark deterministic phrases as public fixtures and
  never reusable for funded wallets.
- The import path must bound phrase length, passphrase length, profile count,
  and scan indexes.
- Errors should not echo invalid phrase words back to logs.
- Backup metadata should identify passphrase use without storing the passphrase
  or enough data to brute-force it offline beyond the normal wallet backup
  exposure.

## Test Plan

- BIP39 checksum validation for all supported word counts.
- BIP39 official test vectors for seed derivation.
- PostFiat KDF vectors for phrase, chain id, profile, path, account index, and
  address index.
- Regression that existing `master_seed_hex` wallets produce unchanged
  addresses and signatures.
- MetaMask path derivation vectors for `m/44'/60'/0'/0/index`.
- Phantom path derivation vectors for the three supported path groupings.
- Redaction tests proving mnemonic words and BIP39 seed material do not appear
  in reports.
- Python helper tests for create/import/preview flows.

## Rollout

1. Add SDK BIP39 parsing, validation, and KDF helpers behind tests.
2. Add CLI mnemonic generation/import/preview commands with redacted output.
3. Add Python wrapper helpers.
4. Add docs and user-facing warnings.
5. Add controlled-testnet smoke that creates a mnemonic wallet, sends a PFT
   payment, restores from the phrase in a fresh directory, and signs a second
   payment from the restored backup.
6. Apply for SLIP-44 coin type before final public wallet documentation.

## References

- BIP39 mnemonic and seed derivation:
  `https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki`
- BIP44 path hierarchy and account discovery:
  `https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki`
- SLIP-44 registered coin types:
  `https://github.com/satoshilabs/slips/blob/master/slip-0044.md`
- MetaMask seed phrase import derivation path:
  `https://support.metamask.io/configure/wallet/importing-a-seed-phrase-from-another-wallet-software-derivation-path/`
- Phantom recovery-phrase path scanning:
  `https://docs.phantom.com/resources/faq`
  and
  `https://help.phantom.com/hc/en-us/articles/12988493966227-What-derivation-paths-does-Phantom-support`
