# L1 validator binding contract v1

Network: `l1-devnet`; algorithm: pure FIPS 204 **ML-DSA-65**. The challenge is the exact UTF-8 string issued by Task Node, with no newline, trimming, prehash or JSON serialization. Context bytes are UTF-8 `postfiat-l1-v2/tasknode-validator-binding/v1`. Public key: 1952 bytes / 3904 lowercase hex characters. Signature: 3309 bytes / 6618 lowercase hex characters.

Registry URL (LIVE under D54, externally verified with trusted TLS): `https://178-156-143-199.sslip.io:18443/l1-registry/registry.json`.
Schema `postfiat-tasknode-l1-registry-v1`: network, chain_id, genesis_hash, generated_at (ISO UTC), height, registry_root, source_heights (two distinct agreeing validator RPC observations), latest_commit (height/hash/actual precommit_signers), validators [{validator_id, ml_dsa_public_key, active, participation}]. Current chain_id is `postfiat-wan-devnet-2`. Do not use the stale genesis in the fleet text file.

## Task Node verification

1. Fetch only the fixed trusted HTTPS registry URL; timeout and bound size (1 MiB sufficient). No redirects. Validate schema/network/chain, unique ids and keys, positive integer heights, 96-character registry root/genesis, source heights consistent with height/root, and generated_at. Reject >3600 seconds old or >60 seconds in the future. A fresh generated_at proves a fresh RPC observation, not recent block production.
2. Look up validator_id and require active===true. Require proof.ml_dsa_public_key EXACTLY matches the registry key (reject unknown ids, inactive validators, stale/rotated keys). Never trust a caller-supplied key alone.
3. Verify the signature against the unconsumed account-bound challenge with expiry as already implemented for Ledger. Atomically consume the connect token/challenge and insert binding under unique network+chain+validator_id and public-key identity; concurrent/replayed proofs must fail. Retain the actual public key, network and validator id.
4. Verify in Node with pinned `@noble/post-quantum` **0.7.1**, tested on Node 20.20.2:

```js
import { ml_dsa65 } from '@noble/post-quantum/ml-dsa.js';
// Strictly validate lengths and lowercase hex BEFORE Buffer.from.
const valid = ml_dsa65.verify(
  Uint8Array.from(Buffer.from(signature, 'hex')),
  new TextEncoder().encode(challenge),
  Uint8Array.from(Buffer.from(registryKey, 'hex')),
  { context: new TextEncoder().encode('postfiat-l1-v2/tasknode-validator-binding/v1') },
);
```

Catch verification errors and refuse. Use `.verify`, not `.internal.verify` or `.prehash`. API order is signature, message, public key, options. Primary library API: https://github.com/paulmillr/noble-post-quantum . `l1-vectors.json` has 3 positives and wrong-key/context/message + truncated-signature negatives; all 7 pass Rust-to-Node verification. Every vector uses the required verification context; the wrong-context signature was actually generated under the transaction context (recorded as `signed_context`). Run all vectors through the fixed-context production verifier. Vector challenge expiries are fixed fixtures: cryptographic tests only, never usable connect tokens.

## One-command handoff

Serve `l1-validator.sh` from Track C public root with Cache-Control: no-store. Existing challenge route payload stays {token}. Completion payload is {token, network:"l1-devnet", validator_id, ml_dsa_public_key, signature}. The static Rust helper emits only the three proof fields. The wrapper identifies the running `postfiat-node transport-validator-serve` process and reads its exact --key-file/--data-dir metadata; node id comes from the public node_state.json object before its appended MAC. No directory search for secret files. It downloads the SHA-256-pinned helper to a sealed memfd, runs it via `unshare --net`, reads the existing private key locally, and posts only the proof plus token. No helper/key/proof files are written on the validator host. Root is required for network namespace creation. Linux x86_64 currently supported; a clear error is returned elsewhere. --discover reads process metadata and node id only, without opening the key.

Helper URL: https://178-156-143-199.sslip.io:18443/l1-registry/l1-validator-signer-linux-x86_64 . Binary SHA-256: 63c5e3d56556447dc06cbcd9fd0b4d8e620dc17068166bb51d3cdb41f50f6eca . Built with Rust 1.95.0, musl static PIE (no dynamic interpreter), using repository crates/crypto_provider and vendored fips204. No consensus or validator release changes.

## Participation and rewards: evidence HOLD

The live RPC's blocks (height 1160) expose `header.consensus_v2_commit.precommit_qc.votes`, but have NO UTC timestamps. Transport/RPC event logs also lack UTC timestamps. This prevents an honest retrospective 30-day score from these sources alone. HOLD-E-participation-clock-20261028T1440Z.md requests a historical clock source or an explicit manager decision. Signing and account binding remain independent of reward eligibility.

Each participation object has window_start/window_end for the last 30 complete UTC days, daily [{date, complete, reported, committed_blocks, signed_blocks, score, from_height, to_height}], reported_days, share (block-weighted), mean_daily_score (D1's unweighted daily mean), available, eligible and eligibility_reason. Unknown scores are null, never 0 or 1. Initial reported_days=0, available=false, eligible=false, reason=historical_clock_unavailable. **Do not pay L1 rewards while this HOLD is unresolved.**

The exporter begins timestamped observations now. Actual precommit votes are counted, never committee membership. Observed head transitions are bounded by consecutive RPC read intervals; changed heads spanning midnight and gaps >15 minutes make the affected days incomplete. A day with no committed blocks has undefined score and is not reported. Observer-derived coverage is explicitly time_basis=observer_brackets and cannot silently replace the missing historical consensus clock. State, source heights and latest-commit signer list are retained for manual reproduction. Registry publication fails closed on chain reset, source disagreement, missing certificates or invalid vote structure, leaving the prior JSON to expire.

## Live publication

D54 activated the additive TLS routes on 2026-10-28 at 14:45 UTC. External trusted-TLS checks returned HTTP200 for research health, registry and helper. Registry had six validators at height1160, age212 seconds, Cache-Control:no-store. Helper is675448 bytes with the SHA256 above. The foundation user timer `l1-tasknode-registry.timer` is enabled for ten-minute refreshes; failed reads preserve the previous JSON until it expires. Activation and rollback scripts/evidence are in the private mission E-release and acceptance/E directories.

## Acceptance ownership

E1: manager runs the fresh Track C one-liner on Foundation L1 validator-0; C verifies production binding twice and records network/id/key. E2: C checks wrong key/context, replay, unknown/inactive validator and stale registry against production. E's crypto vectors already cover signature-level negatives. E3: E publishes trusted TLS registry and manually compares RPC certificate votes; full historical participation remains explicitly unavailable unless the clock HOLD is resolved.
