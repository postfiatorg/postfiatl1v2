# User-proven shielded actions (W-1): specification

Date: 29 October 2026. Status: **draft, not locked**. The Text Improvement
Harness full gate scored it, without this paragraph, at 85.13/100 (GPT-6 Astra
Pro 87.00, Claude Fable 5.1 80.80, GLM 5.3 87.60; five runs per judge; run group
`user-proven-shielded-actions-spec-20261029-r1`; scored content SHA-256
`59bb690fa97b4754…`). It is below 86 and was not iterated; locking it is M1 of
the [milestone](../plans/active/user-proven-shielded-actions-milestone.md). The
same day, after one content pass bringing it up to date with main, the same
gate scored 84.20/100 (GPT-6 Astra Pro 87.60, Claude Fable 5.1 81.00, GLM 5.3
84.00; five runs per judge; run group
`user-proven-shielded-actions-spec-20261029-r2`; scored content SHA-256
`41ccab38ee41eef6…`). It is still below 86, was not iterated, and is not
locked. It authorizes no deployment and no transaction.

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
   intent. The gateway routes (`db9af4f2`, `d9095797`) and the validators'
   admission store and round service (`968da908`, `e37d77fd`) are on main, and
   an integration test runs them together against real validators (`3747a2e7`).
4. Recovery uses finalized public reads pinned to one snapshot token, with
   paging (`683a5b32`, `0c396fc8`). The NAV pricing claim and the egress binding
   reads are on main (`93454729`). Consensus enforces the claim's freshness,
   pair and mode behind a governed activation (`ed89c916`), and governs the
   egress policy through a registry behind its own activation (`899b26f8`).
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
| `crates/node/src/rpc_cli.rs:2086` | `shield_batch_orchard`, `shield_batch_orchard_deposit`, `shield_batch_orchard_withdraw` and `shield_batch_swap` build batches. | A built batch is not an admitted transaction: there is no durable admission, finality or receipt contract. |

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

The admission and round service is `crates/node/src/private_action_admission.rs`.
`crates/node/src/private_action_rpc.rs` serves it as `private_action_submit` and
`private_action_receipt`, behind `rpc-serve --allow-private-action-admission`
(`968da908`, `d7de5ec7`). A record waits until its validator is the proposer for
the next height. A record left `unknown` by a crash settles as accepted when its
batch lands (`e37d77fd`). The gateway is `wallet-proxy/private-actions.js`. It
submits the reserved bytes to every configured validator source and reconciles
their records (`db9af4f2`, `d9095797`).

## 5. The public action contract

The normative payloads are in `crates/types/src/shielded_bridge_governance.rs:369-536`
(`ShieldedSwapActionPayload` at 369, `AssetOrchardIngressV2ActionPayload` at 400,
`AssetOrchardPrivateEgressActionPayload` at 427, the `ShieldedAction` enum at 509).
The routes' request and response types are `crates/types/src/private_actions_v1.rs`.
Their fixtures, including vectors generated and pinned by
`crates/node/tests/private_actions_contract.rs`, are in
`crates/node/tests/fixtures/private-actions-v1/` (`346c79e4`).

| Kind | Public content | Checks before the state change |
| --- | --- | --- |
| `asset_orchard_ingress_v2` | The locally signed burn transaction, pool id, full asset id, integer amount, output commitment and encrypted output. No note field. | Chain and genesis, source and owner, issuer, sequence, amount, fee and commitment binding. The burn and the pool insertion are one atomic step. |
| `shielded_swap_v1` | `swap_json`, the exact locally proven Asset-Orchard action: anchor, nullifiers, encrypted outputs, accounting commitments, pricing claim, proof and spend authorizations. | Both JSON levels decode strictly: no unknown fields, no supplied paths, no note openings. |
| `asset_orchard_private_egress_v1` | The proof and spend signature with the public destination, asset, amount, fee, policy and disclosure binding, anchor and nullifier. | Verified before the note is consumed and the transparent account credited, atomically. |

`asset_orchard_ingress_v1` and `asset_orchard_egress_v1` are refused on this
path. Registering A666 or pfUSDC never authorizes them.

### 5.1 The intent id and once-only admission

The intent id is a domain-separated hash under the tag
`tasknode.private-intent.v1` over the JSON pair `(domain, action_json)`. The
wallet's domain (`PrivateDomain`) has exactly three fields: `chain_id`,
`genesis_hash` and `protocol_version`. The intent id therefore does not cover
the address, the pool or the session. The gateway binds those itself: the
address to the native session principal, and the pool to its route
configuration. The wallet's helper is `core/src/private_actions.rs:110` in the
wallet repository. The gateway recomputes the id and refuses a mismatch. The
admission rules:

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

On a validator, `postfiat-node private-action-resolve` settles a stuck `unknown`
record from the chain (`657ce706`). If its batch is not on chain, the operator
may instead abandon it with a reason. The receipt then reports `abandoned`
with that reason (`ea2213d0`). The wallet may resubmit the same bytes under the
same intent, which creates a fresh record, and the abandoned record stays in
the admission log.

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

The reads are `private_pool_snapshot`, `private_pool_page` and
`private_account_lines` (`683a5b32`, `crates/node/src/private_pool_reads.rs`),
and `private_pool_receipts` and `private_pool_market` (`0c396fc8`,
`crates/node/src/private_pool_receipt_market_reads.rs`). The gateway's snapshot
pages them to completeness under the client ceilings and refuses a mixed tip
(`f8b9a838`).

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
(`ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT`, `crates/node/src/execution_actions.rs:4386`;
pricing plan at `:4426`), so a pre-608 ratio is not a ce22 configuration.

The reads are on main (`93454729`): `asset_orchard_pricing_claim` and
`asset_orchard_private_egress_binding` (`crates/node/src/asset_orchard_pricing_reads.rs:368`,
`:533`). The claim is built by the function consensus uses. The egress binding
returns `ungoverned` because finalized state has no governed egress policy yet.
A fixture policy or an all-zero disclosure hash is never a substitute.

Consensus checks the epoch, the reserve-packet hash, the band, the tags, the
halt flag and the ratio within the band. Three more rules are implemented
behind a governed activation (`ed89c916`). The governance keys are
`asset_orchard_pricing_enforcement_activation_height` and
`asset_orchard_pricing_max_nav_age_blocks`.

- **Staleness:** NAV older than the governed limit is refused as
  `asset_orchard_pricing_nav_stale`.
- **Governed pair:** a pair without a route in `pftl_uniswap_routes` is
  refused as `asset_orchard_pricing_pair_not_governed`. Governance must not
  activate the rules before the A666/pfUSDC route exists there.
- **Mode label:** an at-NAV claim with another mode label is refused as
  `asset_orchard_pricing_mode_mismatch`.

The egress policy is governed through a registry (`899b26f8`). Each record is
an amendment of kind
`asset_orchard_egress_policy_v1:<asset_id>:<disclosure_schema_hash>:<policy_id>`,
whose value is its effective height.
After the global switch `asset_orchard_egress_policy_activation_height`, a
private egress must name its asset's governed policy, and its disclosure hash
must equal the governed schema hash. Otherwise it is refused as
`asset_orchard_egress_policy_mismatch` or `asset_orchard_egress_policy_not_governed`.
Records and the switch are ratified with `postfiat-node
ratify-asset-orchard-egress-policy` and
`ratify-asset-orchard-egress-policy-activation-height`. Until governance writes
the A666 and pfUSDC records, with an agreed disclosure schema, the egress
binding read stays `ungoverned`.

## 8. Readiness

Before admission, the fleet attests the deployed genesis, protocol, L1
revision, proof system, circuit, parameters and verifying keys, and that real
swap and egress verifiers are ready. Atomic shielded batches are activated by
ordinary signed governance at the effective height; before it, execution
refuses them with `shielded_atomic_batch_not_active`
(`crates/node/src/execution_actions.rs:3245`). A mock verifier, a copied store,
an unsafe file signer or a proxy-owned validator key never satisfies readiness.
Readiness is published by the running service and the deployed manifest.

The `private_readiness` read (`556cac38`, `crates/node/src/private_readiness_read.rs`)
reports the pinned and loaded verifier identities and the governed activations.
On `rpc-serve` it also reports the server's own admission switch (`657ce706`).
The release deployed on the devnet predates the read.

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

On main:

- The contract test.
- A gateway-to-node integration test with two real validators, covering
  restart and diverging-tip cases and a secret inspection of every file the
  proxy and the nodes wrote (`3747a2e7`, `46c5a956`, `c308b7af`;
  `crates/node/tests/private_actions_gateway.rs`).
- Six-validator tests with ingress actions and with real swap and egress proofs
  (`e37d77fd`, `e5f8f5b8`).

The stale-anchor and pricing-rule refusals and the ce22 run are not yet
covered.

## 11. What does not change

- **Wire formats.** `ShieldedAction` and its payloads keep their serialization.
- **The custody boundary.** Keys and openings stay on the device; validators
  keep proposal and vote signing.
- **Separate gates.** W-4, admission of outside money, and W-5, backing
  reconciliation, stay separate gates.

## 12. References

Section 12 was added after the first scoring.

- Contract fixtures: `crates/node/tests/fixtures/private-actions-v1/` (`schema.json`, `vectors.json`, `manifest.json`, `assets.json`).
- Shared request and response types: `crates/types/src/private_actions_v1.rs`.
- Vector generator and pins: `crates/node/tests/private_actions_contract.rs`.
- Gateway: `wallet-proxy/private-actions.js`; admission service: `crates/node/src/private_action_admission.rs`, `crates/node/src/private_action_rpc.rs`.
- Reads: `crates/node/src/private_pool_reads.rs`, `crates/node/src/private_pool_receipt_market_reads.rs`, `crates/node/src/private_readiness_read.rs`.
- Integration and secret inspection: `crates/node/tests/private_actions_gateway.rs`.
- [Milestone](../plans/active/user-proven-shielded-actions-milestone.md).
- [Private egress disclosure schema, version 1](asset-orchard-private-egress-disclosure-schema-v1.md): the governed schema hash the egress policy records commit to.
