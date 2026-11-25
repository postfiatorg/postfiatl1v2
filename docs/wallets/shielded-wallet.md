# Shielded Wallet

Shielded wallet flows are Orchard/Halo2-based. Public submission of shielded
actions is off on every validator of `postfiat-wan-devnet-2`
([Network Status](../status/network-status.md)); the commands below run against
a local devnet.

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
