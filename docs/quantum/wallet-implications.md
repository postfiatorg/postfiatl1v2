# Wallet Implications

ML-DSA-style account authorization changes wallet design.

## Differences From Classical Wallets

- Public keys and signatures are larger.
- Deposit and custody systems cannot assume BIP32-style derivation.
- Watch-only and exchange deposit flows need explicit design.
- Hardware wallet support is a separate integration track.
- Key rotation and account boundaries must be documented.

## Private payments on the rehearsal network

The PFSH02 wallet derives distinct spend and ML-KEM receive secrets locally. Only a shield needs the public account’s ML-DSA-65 signature. Payments and exits cover the fee from private notes of the selected asset. A recovery backup also needs its independent wrapping key, network identity and scan birthday. Keep the wrapping key outside the wallet directory. The browser delegates proving and account signing to the native daemon.

At a depth-24 rollover, recipients retain witnesses for old notes and scan new pairs in the writable epoch. In the checked-frontier rehearsal an epoch-0 note moved into epoch 2 and could be spent there. On the separate fresh chain a governance pause left finalized notes recoverable from the signed snapshot. Public deposits, exits, fee amounts and timing can still link activity. [Shielded wallet](../wallets/shielded-wallet.md) · [Rehearsal record](../evidence/pq-private-payments-m8/rehearsal-record.md).

## Current Sources

- `docs/specs/account-key-rotation-boundary.md`
- `docs/specs/transparent-transaction-envelope.md`
- `docs/specs/wallet-exchange-custody-model.md`
- `docs/runbooks/sdk-wallet-flow.md`
