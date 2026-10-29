# User-proven shielded actions (W-1): specification

Date: 29 October 2026. Status: **draft, not locked**. The Text Improvement
Harness full gate scored it, without this paragraph, at 85.13/100 (GPT-6 Astra
Pro 87.00, Claude Fable 5.1 80.80, GLM 5.3 87.60; five runs per judge; run group
`user-proven-shielded-actions-spec-20261029-r1`; scored content SHA-256
`59bb690fa97b4754…`). It is below 86 and was not iterated; locking it is M1 of
the [milestone](../plans/active/user-proven-shielded-actions-milestone.md). It
authorizes no deployment and no transaction.

## 1. BLUF

1. The wallet team's native Mac app proves shielded A666 and pfUSDC actions on
   the device. Today the chain only takes such actions through a wallet proxy
   that holds validator material and runs the certified round itself. W-1
   removes that: the gateway relays the user's immutable public bytes, and the
   validators admit, verify and certify them.
2. The public contract is the existing `postfiat_types::ShieldedAction`
   serialization, restricted to three kinds: `asset_orchard_ingress_v2`,
   `shielded_swap_v1` and `asset_orchard_private_egress_v1`. The legacy v1
   ingress and egress payloads carry note openings or spend material and stay
   refused on this path.
3. Admission is once-only. The gateway recomputes the client's intent id,
   durably reserves the intent, the action bytes and the canonical batch id
   before any transport, and never builds a second round or proof for the same
   intent.
4. Recovery uses finalized public reads pinned to one snapshot token, with
   paging. The NAV pricing claim and the egress binding reads are already on
   main (`93454729`); consensus enforcement of the claim is in progress.
5. User-proven admission is not a two-party exchange. A counterparty, its
   liquidity and its reservation get their own specification later.
6. W-2, the native session, is done (`e037829f`). It is the transport for this
   contract, not part of it.

## 2. Goal

A wallet that holds its own keys submits a locally proven shielded action to
any gateway, and the action is accepted, rejected or left pending by the
validator fleet alone. The gateway holds no wallet key, no validator key and no
validator data directory. A crash, a timeout or a repeated request never
produces a second spend or a second round. The wallet can rebuild its notes
from finalized public reads alone, without sending a viewing key anywhere. The
first live use is one approved dust run of 1 pfUSDC on ce22.

Source: the wallet team's handoff
[`docs/v3-w1-integration.md`](https://github.com/postfiatorg/tasknode-apple/blob/main/docs/v3-w1-integration.md)
in the private repository `postfiatorg/tasknode-apple` (contract baseline
`45ba2334`, against this repository at `48ecefef`), and section 3 of
[the wallet findings handoff](../handoffs/2026-10-29___goodalexander__tasknode_wallet_findings_for_the_l1.md).
This document restates that contract with this repository's references.

## 3. The current custody model

Shielded swaps on the wallet proxy are operator-run, not user-proven.

| Where | What it does | Why it blocks W-1 |
| --- | --- | --- |
| `wallet-proxy/navswap-config-bridge.js:782` | Ingress configuration needs a node binary, a validator data directory, the topology and a validator key file. | The HTTP process reads validator state and holds a signing key. |
| `wallet-proxy/navswap-config-bridge.js:837` | Swap availability depends on a local Asset-Orchard service. | A remote prover service is in the path; an on-device proof needs none. |
| `wallet-proxy/navswap-config-bridge.js:963` | Builds a651/a652 quotes and the proxy's own liquidity policy. | The registry is retired; prices are proxy-derived, not validator-derived. |
| `wallet-proxy/navswap-shielded.js:1421` | Runs `postfiat-node transport-peer-certified-batch-round` with `--key-file`. | The proxy drives the certified round with a validator key. |
| `wallet-proxy/navswap-shielded.js:2292` | Posts the swap to `/asset-orchard/swap-batch` on the local service. | Batch construction depends on that service. |
| `wallet-proxy/navswap-shielded.js:2386` | Executes and certifies the swap on the HTTP side. | Certification belongs to the validators. |
| `crates/node/src/rpc_cli.rs:2070` | `shield_batch_orchard`, `shield_batch_orchard_deposit`, `shield_batch_orchard_withdraw` and `shield_batch_swap` build batches. | A built batch is not an admitted transaction: there is no durable admission, finality or receipt contract. |

The internals of the last row may be reused after review. A successful
batch-build response is never reported as a committed action.

## 4. The target model

- **The wallet** proves, signs and persists the action and its intent locally.
  Seeds, mnemonics, data-encryption keys, incoming viewing keys, note files and
  signing keys never leave it.
- **The gateway** authenticates the session (W-2), checks shape and size,
  recomputes the intent id, reserves the admission durably and relays the exact
  bytes. It has no key and no validator filesystem access.
- **The validators** run an admission and round service: real proof, spend
  authorization, retained anchor, nullifier, pricing and policy checks, then
  ordinary proposal, vote and apply. Proposer routing, height ownership, quorum,
  catch-up and durable intent reconciliation are theirs.

## 5. The public action contract

The normative payloads are in `crates/types/src/shielded_bridge_governance.rs:334-501`
(`ShieldedSwapActionPayload` at 334, `AssetOrchardIngressV2ActionPayload` at 365,
`AssetOrchardPrivateEgressActionPayload` at 392, the `ShieldedAction` enum at 474).

| Kind | Public content | Checks before the state change |
| --- | --- | --- |
| `asset_orchard_ingress_v2` | The locally signed burn transaction, pool id, full asset id, integer amount, output commitment and encrypted output. No note field. | Chain and genesis, source and owner, issuer, sequence, amount, fee and commitment binding. The burn and the pool insertion are one atomic step. |
| `shielded_swap_v1` | `swap_json`, the exact locally proven Asset-Orchard action: anchor, nullifiers, encrypted outputs, accounting commitments, pricing claim, proof and spend authorizations. | Both JSON levels decode strictly: no unknown fields, no supplied paths, no note openings. |
| `asset_orchard_private_egress_v1` | The proof and spend signature with the public destination, asset, amount, fee, policy and disclosure binding, anchor and nullifier. | Verified before the note is consumed and the transparent account credited, atomically. |

`asset_orchard_ingress_v1` and `asset_orchard_egress_v1` are refused on this
path. Registering A666 or pfUSDC never authorizes them.

### 5.1 The intent id and once-only admission

The intent id is a domain-separated hash under the tag
`tasknode.private-intent.v1` over the JSON pair `(domain, action_json)`, where
the domain names the chain, genesis, protocol, pool and session scope. The
wallet's helper is `core/src/private_actions.rs:110` in the wallet repository.
The gateway recomputes it and refuses a mismatch. The admission rules:

1. The original action text is kept byte for byte, so the wallet can compare it.
2. Different bytes under an existing intent id are refused. The canonical batch
   and action are deduplicated too, so a new intent cannot replay a spend.
3. The batch is built with `build_atomic_shielded_action_batch`
   (`crates/node/src/consensus_artifacts.rs:1591`) with `atomic: true`, and its
   id is checked against the actual genesis.
4. Before transport, the domain, intent, action hash and bytes and the batch id
   are reserved durably. The states are queued, dispatched, accepted, rejected
   and unknown, and they stay distinct.
5. A retry resends the identical reserved batch. The gateway never re-signs or
   changes it, and a crash or duplicate request never starts a new round.
6. An HTTP acknowledgement means admitted and pending, nothing more. `receipt`
   is read-only. A missing or unknown receipt stays pending and never
   authorizes a replacement action.
7. A receipt carries the action, batch and transaction bindings, the inclusion
   height, block hash and state root, the outcome, code and fee, and finality
   evidence the wallet can check: a validator quorum certificate with receipt
   inclusion, or an independently sourced validator view. Terminal rejections
   are typed, with their fee and state effects.

## 6. Recovery reads

The wallet rebuilds its notes from one finalized snapshot. A snapshot holds the
chain id, genesis hash, protocol version and address; the finalized height,
block hash and state root; ordered commitments, spent nullifiers and genuinely
spendable accepted anchors; outputs with their ciphertexts; asset rows; the
base and quote asset ids; the pricing claim; the policy id and disclosure hash;
and an explicit completeness flag.

- Every page, market and account read is pinned to one snapshot token, with
  counts and cursors. Missing, duplicated, reordered or mixed-tip pages are
  refused.
- An empty pool before the first ingress is valid; missing history after it is not.
- Accepted anchors come from the retained tree prefixes, not from a history of
  anchors that earlier spends used.
- Ciphertexts needed for phrase recovery are retained. The wallet scans with its
  own viewing key and reseals under a fresh key; it never sends that key.
- Asset rows bind issuer, code, precision and `requires_authorization` to
  `asset_info`; frozen or unauthorized lines fail.

The client's current ceilings are 64 ciphertexts, 2 MiB of ciphertext,
4096 commitments, nullifiers or anchors, 16 assets, 1 MiB of action text, a
2 MiB request and a 4 MiB response, and it fails closed above them. Production
needs versioned paging and incremental tree, witness and scan support, not
larger JSON limits.

## 7. Pricing and egress binding

A swap's pricing claim is the typed `AssetOrchardPricingClaim` built from
consensus-finalized NAV for the intended inclusion height: epoch, reserve-packet
hash, ratio, mode and band, and both asset tags. Stale, halted or mismatched
state is refused. The NAV denominator changes units at height 608
(`ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT`, `crates/node/src/execution_actions.rs:4267`;
pricing plan at `:4284`), so a pre-608 ratio is not a ce22 configuration.

The reads are on main (`93454729`): `asset_orchard_pricing_claim` and
`asset_orchard_private_egress_binding` (`crates/node/src/asset_orchard_pricing_reads.rs:324`,
`:444`). The claim is built by the function consensus uses. The egress binding
returns `ungoverned` because finalized state has no governed egress policy yet.
A fixture policy or an all-zero disclosure hash is never a substitute.

Consensus enforcement of the claim is in progress. Consensus checks the epoch,
the reserve-packet hash, the band, the tags, the halt flag and the ratio within
the band. It has no staleness limit, no governed pair (the base asset is
accepted as its own quote) and no mode-label check for at-NAV claims; those
three need a governed activation. The egress policy and disclosure hash are
checked for form and bound into the proof but not governed; that needs a
governance-registered egress policy.

## 8. Readiness

Before admission, the fleet attests the deployed genesis, protocol, L1
revision, proof system, circuit, parameters and verifying keys, and that real
swap and egress verifiers are ready. Atomic shielded batches are activated by
ordinary signed governance at the effective height; before it, execution
refuses them with `shielded_atomic_batch_not_active`
(`crates/node/src/execution_actions.rs:3184`). A mock verifier, a copied store,
an unsafe file signer or a proxy-owned validator key never satisfies readiness.
Readiness is published by the running service and the deployed manifest.

## 9. Scope: admission, not exchange

The wallet team's accepted fixture is one owner spending two of its own notes
into two outputs. It proves the proof format, the transport and recovery. W-1
delivers user-proven admission of such actions.

A real exchange, a user buying A666 with only a pfUSDC note against a
counterparty, needs participant-owned spend authorization, output-recipient
binding, quote expiry and reservation, with no participant's openings moving
to a server or the other wallet. That protocol, its circuit and its app changes
get their own specification. The negotiated FX-fix path has consensus
reservations but different fee and expiry rules, so it is not a drop-in
replacement. A custodial `pftl_swapd` route does not close W-1.

## 10. Acceptance

1. **Fixtures and vectors.** Versioned request and response fixtures; canonical
   action, intent and batch vectors; the deployed identity, verifier and
   activation manifest; the full A666 and pfUSDC profile with issuer, proof
   profile and precision.
2. **Six-validator tests.** Both ingress assets, swap and egress. Invalid
   proof, stale anchor or NAV, wrong issuer, domain or recipient, replay or
   reused nullifier, inactive governance and a tampered receipt each fail with
   no unintended state change.
3. **Crash and recovery.** Crash after dispatch and lost responses, then a new
   session recovers the same persisted action with no replacement or double
   spend. A fresh-key full rescan, page boundaries and incomplete history.
   Lock or cancel leaves no helper process and no usable proof or submission.
4. **Secret inspection.** Logs and artifacts hold no secrets; the gateway holds
   no wallet or validator key; no remote prover receives openings; an HTTP
   request or release setting cannot turn off validator verification.
5. **The ce22 run.** After review and the wallet team's HOLD approval: a native
   shield, private swap and unshield bounded at 1 pfUSDC, with exact public and
   private balances and fees, finality and fleet convergence.

## 11. What does not change

- **Wire formats.** `ShieldedAction` and its payloads keep their serialization.
- **The custody boundary.** Keys and openings stay on the device; validators
  keep proposal and vote signing.
- **Separate gates.** W-4, admission of outside money, and W-5, backing
  reconciliation, stay separate gates.
