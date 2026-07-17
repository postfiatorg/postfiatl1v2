# Shielded Wallet

Shielded wallet flows are Orchard/Halo2-based.

## Current Commands

- `orchard-keygen`;
- `orchard-view-key-export`;
- `orchard-scan`;
- `orchard-disclose`;
- `orchard-disclosure-verify`;
- `orchard-deposit-create`;
- `shield-batch-orchard-deposit`;
- `orchard-spend-create`;
- `orchard-withdraw-create`;
- `shield-batch-orchard-withdraw`.

## Current Flow

1. create or import Orchard keys locally;
2. deposit transparent value into the Orchard pool;
3. scan for decrypted outputs;
4. spend or withdraw from retained witness material;
5. disclose selected facts when needed.

## Source

- `crates/node/src/privacy.rs`
- `crates/privacy_orchard/src/lib.rs`
- `docs/status/privacy-production-burndown.md`
