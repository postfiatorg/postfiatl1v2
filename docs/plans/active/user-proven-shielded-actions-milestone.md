# User-proven shielded actions (W-1): the wallet app's private path

**Status:** Active from 2026-10-29. Authorizes no deployment and no transaction on ce22.
**Date:** 2026-10-29
**Source spec:** [User-proven shielded actions specification](../../specs/user-proven-shielded-actions-spec-20261029.md), Text Improvement Harness full gate 85.13 on 2026-10-29 (GPT-6 Astra Pro 87.00, Claude Fable 5.1 80.80, GLM 5.3 87.60; run group `user-proven-shielded-actions-spec-20261029-r1`), one pass, not iterated. It is not locked (M1).
**Basis:** the wallet team's [`docs/v3-w1-integration.md`](https://github.com/postfiatorg/tasknode-apple/blob/main/docs/v3-w1-integration.md) (private repository `postfiatorg/tasknode-apple`) and section 3 of [the wallet findings handoff](../../handoffs/2026-10-29___goodalexander__tasknode_wallet_findings_for_the_l1.md).

## BLUF

The wallet team's Mac app proves shielded A666 and pfUSDC actions on the
device. Shielded rounds today run through a wallet proxy that holds a validator
key and drives the round itself. This milestone moves admission and
certification to the validators: the gateway relays the user's exact public
bytes, once, and holds no key. W-2, the native session, is done (`e037829f`).
The finalized pricing and egress-binding reads are on main (`93454729`), and
consensus enforcement of the claim is in progress. My optimistic estimate is
five to eight weeks of engineering for S1 to S7, before the review and the
ce22 run.

Owners are marked *(this lane)* or *(wallet team)*.

## S1 — the contract (3–5 days)

- [x] S1.1 *(this lane)* Versioned request and response fixtures for `snapshot`, `submit` and `receipt`, including a typed terminal rejection (`state: rejected` with its code and fee).
- [x] S1.2 *(this lane)* Canonical vectors: an action of each kind, its intent id under `tasknode.private-intent.v1`, and its atomic batch id from `build_atomic_shielded_action_batch` (`crates/node/src/consensus_artifacts.rs:1591`).
  - The three accepted kinds are in `crates/types/src/shielded_bridge_governance.rs:334-501`; `asset_orchard_ingress_v1` and `asset_orchard_egress_v1` are refused.
- [ ] S1.3 *(this lane)* The deployed identity, verifier and activation manifest: genesis, protocol, revision, proof system, circuit, parameters and verifying keys.
  - [x] Identity, release and the swap and egress verifier pins.
  - [ ] The atomic-batch activation height: no read-only RPC on the deployed release reports it, and no record of the amendment exists here, so `manifest.json` holds `null`.
- [x] S1.4 *(this lane)* The full A666 and pfUSDC profile: full asset ids, issuers, proof profiles and precision, replacing the retired a651/a652 registry (`wallet-proxy/navswap-config-bridge.js:963`).
  - The governed swap pair is `null`: it arrives with `ed89c916` behind a governed activation, after the deployed release.
- [ ] S1.5 *(wallet team)* Their intent helper (`core/src/private_actions.rs:110`) reproduces the S1.2 vectors. To tick S1: the fixtures in the repository and both sides passing the vectors.

## Contract fixtures

In `crates/node/tests/fixtures/private-actions-v1/`, shared types in `crates/types/src/private_actions_v1.rs`:

- `schema.json`: schema `postfiat-private-actions-v1`, the three routes, admission states, refusal codes and examples.
- `vectors.json`: generated and pinned byte for byte by `crates/node/tests/private_actions_contract.rs`. One action of each kind with its intent id (96 hex, SHA3-384), action hash and atomic batch id for the devnet genesis, and six refusals. The proofs and signatures are placeholders and never verify.
- `manifest.json`: devnet identity, release `index-v3-20261027` (`d728f895`), verifier pins, atomic-batch activation (`null`).
- `assets.json`: A666 and pfUSDC from live reads on 2026-10-29, each field with its source; the governed pair is `null`.

## S2 — the gateway entry points (1–2 weeks)

- [x] S2.1 *(this lane)* `POST /api/private/v1/snapshot`, `/submit` and `/receipt` behind the browser relay or native session token, with the S1 schemas and size limits (1 MiB action, 2 MiB request, 4 MiB response): `wallet-proxy/private-actions.js`, wired in `wallet-proxy/server.js`. `submit` counts against the mutation rate window; `snapshot` and `receipt` are authenticated reads.
  - [x] The snapshot is assembled from the node reads `private_pool_snapshot`, `private_pool_page` and `private_account_lines` (`683a5b32`). It takes one token from both sources and refuses a mixed tip, pages every section under the wallet's ceilings with all pages pinned to that token, retries once when the tip moves, and takes the pricing claim, policy id and disclosure hash from the node. The `complete` flags combine the node's flags with whether paging reached each total.
    - The S1 response type has no field for account lines. The gateway reads them under the token and clears `complete.assets` when a listed asset's line is not usable or disagrees with its asset row.
    - Where a ceiling stops a section, its cursor stays `null`: the node's cursors mark page ends, not the item where the ceiling stopped.
- [x] S2.2 *(this lane)* Durable once-only admission. The gateway recomputes the intent id (SHA3-384, `tasknode.private-intent.v1`) and the action hash and atomic batch id from the re-serialized action, all three checked against the S1 vectors. It keeps the action text byte for byte, binds the intent to the session principal, the address, the configured domain and the pool, and claims the action hash and batch id with exclusive index files before writing the record (`queued`) and before any transport. A duplicate returns the record; only a reservation no validator has seen is resent, with the reserved bytes. An acknowledgement means pending; `receipt` is read-only and reconciles two validator sources.
- [x] S2.3 *(this lane)* No key and no validator filesystem access in the HTTP process. Dispatch goes only through the node methods `private_action_submit` and `private_action_receipt` on two configured RPC sources, never through `transport-peer-certified-batch-round` (`wallet-proxy/navswap-shielded.js:1421`), the data-directory configuration (`wallet-proxy/navswap-config-bridge.js:782`) or the local Asset-Orchard service (`:837`).
- [x] Crash-and-retry: `wallet-proxy/test_private_actions.js` kills the process after the reservation; on restart the intent is `queued`, no new batch is built, and the reserved batch is sent once.
- [ ] S2.4 *(wallet team)* The app moves from `/api/private-local/` to `/api/private/v1/`.
- S2 is ticked on the gateway side; the routes reach a validator only once S3 deploys the two node methods behind its operator switch.

## S3 — the validator-owned admission and round service (2–3 weeks)

- [ ] S3.1 *(this lane)* A validator-side service that accepts a verified atomic batch and runs proof, spend, retained-anchor, nullifier, pricing and policy checks before ordinary proposal, vote and apply.
- [ ] S3.2 *(this lane)* Proposer routing, height ownership, quorum, catch-up and durable intent reconciliation.
- [ ] S3.3 *(this lane)* Retire the HTTP-side certification: `transport-peer-certified-batch-round --key-file` (`wallet-proxy/navswap-shielded.js:1421`), the `/asset-orchard/swap-batch` call (`:2292`) and the swap execution that certifies it (`:2386`).
  - The `shield_batch_*` builders (`crates/node/src/rpc_cli.rs:2070`) may be reused after review; a built batch is never reported as committed.
- [ ] S3.4 *(this lane)* Receipts with the action, batch and transaction bindings, inclusion height, block hash, state root, outcome, code, fee and checkable finality evidence. To tick S3: S3.3's code paths unused on the W-1 route and an S6 test through the service.

## S4 — finalized public reads (1–2 weeks)

- [ ] S4.1 *(this lane)* A snapshot token pinning one finalized height, block hash and state root, with counts and cursors.
- [ ] S4.2 *(this lane)* Paged pool reads: commitments, nullifiers, accepted anchors from the retained tree prefixes, outputs with ciphertexts.
- [ ] S4.3 *(this lane)* Paged receipt and market reads, and asset rows bound to `asset_info`.
- [ ] S4.4 *(wallet team)* Versioned paging and incremental scan in the app, replacing today's ceilings (64 ciphertexts, 2 MiB of ciphertext, 4096 commitments, 16 assets, 1 MiB action, 2 MiB request, 4 MiB response). To tick S4: a missing, duplicated, reordered or mixed-tip page refused in tests.

## S5 — the pricing claim and egress binding reads (done)

- [x] S5 *(this lane)* `asset_orchard_pricing_claim` and `asset_orchard_private_egress_binding` (`93454729`).
  - `crates/node/src/asset_orchard_pricing_reads.rs:324` and `:444`; the claim is built by the function consensus uses.
  - Python: `python -m postfiat_rpc --method asset_orchard_pricing_claim` and `--method asset_orchard_private_egress_binding` (`python/postfiat_rpc/__main__.py:131-132`).
  - The egress read returns `ungoverned`: finalized state has no governed egress policy yet.

## S5b — consensus enforcement of the claim (in progress, another session)

Consensus checks the epoch, the reserve-packet hash, the band, the tags, the
halt flag and the ratio within the band.

- [ ] S5b.1 *(this lane)* A staleness limit, behind a governed activation.
- [ ] S5b.2 *(this lane)* A governed pair; today the base asset is accepted as its own quote. Behind the same activation.
- [ ] S5b.3 *(this lane)* A mode-label check for at-NAV claims. Behind the same activation.
- [ ] S5b.4 *(this lane)* A governance-registered egress policy, so the policy id and disclosure hash are governed and not only checked for form and bound into the proof.

## S6 — the six-validator test suite (1–2 weeks)

- [ ] S6.1 *(this lane)* Both ingress assets, swap and egress accepted on six validators.
- [ ] S6.2 *(this lane)* Refusals with no unintended state change: invalid proof, stale anchor or NAV, wrong issuer, domain or recipient, replay, inactive governance (`shielded_atomic_batch_not_active`, `crates/node/src/execution_actions.rs:3184`), tampered receipt.
- [ ] S6.3 *(this lane)* Crash after dispatch and lost responses; a new session recovers the same persisted action with no replacement or double spend.
- [ ] S6.4 *(this lane, wallet team)* Fresh-key full rescan and page boundaries.

## S7 — readiness and secret inspection (3–5 days)

- [ ] S7.1 *(this lane)* Readiness published by the running service and the deployed manifest; no mock verifier, copied store, unsafe file signer or proxy-owned key satisfies it.
- [ ] S7.2 *(this lane)* Logs and artifacts inspected for secrets; the gateway holds no wallet or validator key; no remote prover receives openings; no HTTP request or release setting turns off verification.

## S8 — the approved ce22 run (with the wallet team)

- [ ] S8 *(this lane, wallet team)* After review and the wallet team's HOLD approval: native shield, private swap and unshield bounded at 1 pfUSDC, with exact balances and fees, finality and fleet convergence. To tick it: the run record.

## W-2 — the native session (done)

- [x] W-2 *(this lane)* `POST /api/native-session/v1/open` (`e037829f`): an ML-DSA-65 install identity signed under `postfiat-l1-v2/native-session/v1`, an HMAC token `pfns1.<payload>.<mac>` bound to origin, chain id and genesis hash (15 minutes by default), the browser relay's caps, an optional install allowlist, query-string tokens refused on every route. Documented in [the web wallet page](../../wallets/web-wallet.md#native-app-sessions).
  - [ ] `docker-compose.wallet-public.yml` passes the new environment variables.
  - [ ] A rate limit on the open route itself.
  - [ ] A cross-check that a signature from the Rust crypto provider verifies in the proxy.
  - [ ] Individual token revocation.

## Mandate deliverables (AGENTS.md)

- [ ] M1 *(this lane)* Lock the source spec at 86 or above on the full gate, requested as a Task Node task.
- [ ] M2 *(this lane)* A Python CLI for the new public reads (S4), alongside the S5 commands.
- [ ] M3 *(this lane)* A read-only view that consumes the CLI output.
- [ ] M4 *(this lane)* Retire this document into `docs/plans/completed/` and document the path in the existing documentation.

## Not before

- No admission on ce22 before S7 and the governed activation of atomic shielded batches.
- No ce22 run before review and the wallet team's HOLD approval (S8).
- No two-party exchange: a counterparty, its liquidity and its reservation get their own specification.

## Does not change

- **Wire formats.** `ShieldedAction` and its payloads keep their serialization.
- **The custody boundary.** Keys and openings stay on the device; validators keep proposal and vote signing.
- **Separate gates.** W-4, admission of outside money, and W-5, backing reconciliation.

## References

- [User-proven shielded actions specification](../../specs/user-proven-shielded-actions-spec-20261029.md)
- The wallet team's [`docs/v3-w1-integration.md`](https://github.com/postfiatorg/tasknode-apple/blob/main/docs/v3-w1-integration.md) (private)
- [Wallet findings handoff](../../handoffs/2026-10-29___goodalexander__tasknode_wallet_findings_for_the_l1.md), section 3
- [Web wallet: native app sessions](../../wallets/web-wallet.md#native-app-sessions)
- `AGENTS.md` (the specification lock, the CLI and interface, retirement)
