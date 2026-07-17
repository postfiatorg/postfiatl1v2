# Plaintext Key File Boundary

Status: controlled-testnet compatibility format
Last updated: 2026-05-22

PostFiat L1 still has local JSON key-file formats that contain raw secret
material. These files are compatibility artifacts for the controlled testnet
tooling and must not be treated as a production custody format.

## Plaintext Secret Fields

The following fields are secret-bearing and must never be printed in CLI
reports, RPC responses, logs, review artifacts, or public docs except as field
names:

- `private_key_hex`
- `master_seed_hex`
- `spending_key_hex`
- `signature_seed_hex`

Current files that may contain those fields include:

- `DevKeyFile`
- `WalletBackupFile`
- `ValidatorKeyRecord`
- `ValidatorKeyFile`
- `OrchardWalletKeyFile`

## Current Guardrails

- Secret files are written through the shared storage atomic writer.
- Wallet, validator, and Orchard key-file writes set private filesystem
  permissions on Unix.
- Key-file readers validate private permissions on Unix before accepting local
  secret-bearing files.
- Public CLI/RPC reports must redact secret material and expose only public keys,
  addresses, hashes, counts, and explicit redaction flags.
- `WalletTestVectorReport` v2 contains only public derivations and never echoes
  its caller-supplied master/signature seed inputs; its regression checks both
  secret field names and exact supplied values.

## Required Handling

- Operators must store these files on encrypted disks or equivalent host
  controls.
- `REPORT`, log, and test artifact paths must not receive raw key-file JSON.
- New code must pass secret fields by reference or scoped `Zeroizing` buffers
  where feasible.
- New production custody work must use an encrypted key-file envelope before
  these files are used outside controlled testnets.

## Future Format

The production replacement should introduce an encrypted envelope with:

- `schema`, `kdf`, `kdf_params`, `cipher`, `nonce`, and `ciphertext`
- authenticated metadata for chain/domain/account role
- no plaintext secret fields outside the ciphertext
- migration tooling that reads this compatibility format only from private local
  files and emits the encrypted envelope
