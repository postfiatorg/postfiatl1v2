# Wallet Implications

ML-DSA-style account authorization changes wallet design.

## Differences From Classical Wallets

- Public keys and signatures are larger.
- Deposit and custody systems cannot assume BIP32-style derivation.
- Watch-only and exchange deposit flows need explicit design.
- Hardware wallet support is a separate integration track.
- Key rotation and account boundaries must be documented.

## Current Sources

- `docs/specs/account-key-rotation-boundary.md`
- `docs/specs/transparent-transaction-envelope.md`
- `docs/specs/wallet-exchange-custody-model.md`
- `docs/runbooks/sdk-wallet-flow.md`
