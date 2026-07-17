# Account Key Rotation Boundary

Status: controlled-testnet boundary
Date: 2026-05-14

This document defines the current account key-rotation boundary for transparent
ML-DSA accounts. Validator hot-key rotation is implemented through governance
and operator runbooks. Account key rotation is not implemented in the
controlled-testnet transparent transfer protocol.

## Current Account Rule

Transparent accounts use first-spend public-key binding:

- an address is derived from an ML-DSA public key;
- a recipient can be funded before the public key is stored on-chain;
- the first valid spend from the account stores `public_key_hex` on the account;
- subsequent spends must use the same public key;
- a different public key for the same address is rejected.

This protects against arbitrary key replacement, but it also means the chain has
no current transaction that can replace an account key after it is bound.

## Controlled-Testnet UX

For controlled testnet:

- wallet backup and restore are the recovery mechanism;
- if a key may be compromised but is still usable, the operator should sweep
  funds to a fresh address generated from a fresh backup;
- if the key is lost, protocol recovery is not available;
- custodians must disable deposit addresses whose backup status is uncertain;
- public language must not claim account key rotation, social recovery, or
  hardware-wallet recovery.

Validator key rotation is separate and remains governed by the validator
registry lifecycle and emergency key-rotation runbook.

## Future Protocol Shape

A production account rotation feature should be a versioned transaction type,
not an ad hoc mutation of account state. The likely requirements are:

- old account key signature over the rotation intent;
- new account key signature proving possession of the replacement key;
- chain id, genesis hash, protocol version, account address, old public key,
  new public key, sequence, fee, and expiry bound into signing bytes;
- replay protection through the account sequence;
- finality receipt and tx-finality proof like ordinary transfers;
- wallet and custody policies for pending rotation, cancellation, and emergency
  sweep fallback;
- explicit tests for wrong old key, wrong new key, stale sequence, replay,
  reserve/fee failure, and finality evidence.

Until that transaction exists, account key rotation remains out of scope for
controlled testnet.
