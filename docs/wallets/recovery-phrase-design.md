# Recovery Phrase Design

PostFiat transparent wallets currently use private seed material that feeds
the ML-DSA wallet backup and SDK signing flow. Recovery phrase support should
be added as a wallet UX layer over that path, not as a transaction or consensus
change.

## Target UX

- Generate 24-word BIP39 English phrases by default.
- Import 12, 15, 18, 21, and 24-word BIP39 phrases.
- Validate checksum and normalize phrase/passphrase input.
- Derive the existing PostFiat private wallet seed through a
  domain-separated KDF.
- Preserve the current SDK wallet backup and quote-bound signing model.
- Preview MetaMask and Phantom-compatible addresses from the same phrase so
  users can recognize imported recovery phrases.

## Compatibility Profiles

MetaMask preview:

```text
m/44'/60'/0'/0/index
```

Phantom preview:

```text
bip44change Solana: m/44'/501'/index'/0'
bip44change EVM:    m/44'/60'/0'/0/index
bip44 Solana:       m/44'/501'/index'
bip44 EVM:          m/44'/60'/1'/0/index
deprecated Solana:  m/501'/index'/0/0
deprecated EVM:     m/44'/60'/2'/0/index
```

These profiles are for preview, recovery, and account-linking UX. They do not
make MetaMask or Phantom native PostFiat signers.

## Native PostFiat Rule

PostFiat must not reuse XRP's SLIP-44 coin type `144'`. The public recovery
phrase profile should use a real assigned PostFiat coin type before public
launch.

Until then, controlled-testnet wallets must record an explicit controlled
testnet derivation profile in private backup metadata.

## Source Spec

The canonical internal design lives under `docs/specs/` in the repository.
