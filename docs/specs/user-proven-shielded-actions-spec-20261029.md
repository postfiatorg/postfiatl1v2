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
`41ccab38ee41eef6…`). After one improvement pass on the r2 critiques through
`openai/gpt-5.6-sol-pro` (AGENTS.md step 4) and a fact check against main, the
same gate scored 85.40/100 (GPT-6 Astra Pro 86.20, Claude Fable 5.1 82.40,
GLM 5.3 87.60; five runs per judge; run group
`user-proven-shielded-actions-spec-20261029-r3`; scored content SHA-256
`d2104b41ecf0c71a…`). It is below 86 and is not locked; no further pass was
made. It authorizes no deployment and no transaction.

## 1. BLUF

1. The wallet team's native Mac app proves shielded A666 and pfUSDC actions on
   the device. Today the chain takes those actions only through a wallet proxy
   that holds validator material and drives the certified round. W-1 replaces
   that custody model: the gateway relays immutable public bytes, while
   validators admit, verify, certify and apply them.
2. The public contract remains the existing
   `postfiat_types::ShieldedAction` serialization, restricted to
   `asset_orchard_ingress_v2`, `shielded_swap_v1` and
   `asset_orchard_private_egress_v1`. The legacy v1 ingress and egress
   payloads expose note openings or spend material and remain refused.
3. Normal admission is once-only: before transport, the gateway reserves the
   intent, exact action bytes and canonical batch id; retries reuse that
   reservation and never create another proof or round. An operator may
   abandon only a `queued` or `unknown` record whose batch is not on chain and
   that has no round in flight; a resubmission carries the same bytes, the same
   action hash and the same canonical batch (section 5.1).
4. Recovery uses finalized public reads pinned to one snapshot token. Pricing
   and egress are governed consensus rules, not proxy policy. W-1 covers
   user-proven admission, not a two-party exchange. W-2 is the completed native
   session transport and is not part of this contract.

### Terms used below

| Term | Meaning in this document |
| --- | --- |
| **W-1** | User-proven admission of locally proved shielded actions without proxy custody. |
| **W-2** | The native session used to transport this contract. |
| **W-4 / W-5** | Admission of outside money / backing reconciliation; both remain separate gates. |
| **A666 / pfUSDC** | The two assets named by the action profiles, ingress tests, governed pricing pair and egress policy. The first live run is bounded at 1 pfUSDC. |
| **Asset-Orchard** | The shielded action, pool, pricing and egress-policy subsystem named by the payloads and reads below. |
| **NAV** | The consensus-finalized pricing input from which an `AssetOrchardPricingClaim` is built. |
| **ce22** | The controlled devnet `postfiat-wan-devnet-2`, named by its genesis hash prefix (`ce22ca8c…`), where the first approved live dust run takes place. A pre-height-608 NAV ratio is not a ce22 configuration. |
| **Height 608** | `ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT`, where the NAV denominator changes units. |
| **HOLD approval** | The wallet team's required approval before the ce22 run. |
| **Gateway** | The W-2-authenticated relay that checks, durably reserves and forwards exact public bytes without wallet or validator keys. |
| **Validator fleet** | The validators that verify, propose, vote, apply and certify the action. |
| **`unknown` / `abandoned`** | An unresolved validator admission record / an operator-resolved record whose batch was not on chain and whose abandonment reason is retained. |

### Contract flow

| Step | Event |
| --- | --- |
| 1. Submit | The wallet proves, signs and persists the action locally, then submits its exact serialization over the W-2 session. |
| 2. Reserve | The gateway checks shape and size, recomputes the intent id, builds the canonical atomic batch and durably reserves the domain, intent, action hash, action bytes and batch id. |
| 3. Relay | The gateway sends the identical reserved bytes to every configured validator source. |
| 4. Certify | The responsible validator waits for its proposer height, performs the proof, authorization, anchor, nullifier, pricing and policy checks, then uses ordinary proposal, vote and apply. |
| 5. Receipt and recovery | An HTTP acknowledgement remains pending. The wallet checks a read-only receipt for finality and rebuilds notes from finalized snapshot reads. |

## 2. Goal

A wallet that holds its own keys submits a locally proven shielded action to
any gateway, and the validator fleet alone accepts it, rejects it or leaves it
pending. The gateway holds no wallet key, no validator key and no validator
data directory. A crash, timeout or repeated request must never produce a
second spend or a second round. Explicit abandonment and resubmission follow
the rules in section 5.1. The wallet rebuilds its notes from finalized public reads alone,
without sending a viewing key anywhere. The first live use is one approved
dust run of 1 pfUSDC on ce22.

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
| `wallet-proxy/navswap-config-bridge.js:783` | Ingress configuration needs a node binary, a validator data directory, the topology and a validator key file. | The HTTP process reads validator state and holds a signing key. |
| `wallet-proxy/navswap-config-bridge.js:858` | Swap availability depends on a local Asset-Orchard service. | A remote prover service is in the path; an on-device proof needs none. |
| `wallet-proxy/navswap-config-bridge.js:996` | Builds a651/a652 quotes and the proxy's own liquidity policy. | The registry is retired; prices are proxy-derived, not validator-derived. |
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
- **The gateway** authenticates the session through W-2, checks shape and size,
  recomputes the intent id, reserves the admission durably and relays the exact
  bytes. It has no key and no validator filesystem access.
- **The validators** run the admission and round service. They perform the real
  proof, spend-authorization, retained-anchor, nullifier, pricing and policy
  checks, followed by ordinary proposal, vote and apply. Proposer routing,
  height ownership, quorum, catch-up and durable intent reconciliation belong
  to them.

The validator service exposes `private_action_submit` and
`private_action_receipt` behind
`rpc-serve --allow-private-action-admission`. A record waits until its
validator is the proposer for the next height. A record left `unknown` by a
crash settles as accepted when its batch lands.

The gateway submits the reserved bytes to every configured validator source
and reconciles their records. The reconciliation and abandonment rules are in
section 5.1 and the boundaries in section 9.

## 5. The public action contract

The normative wire type is `postfiat_types::ShieldedAction`. Its request and
response contract uses the shared private-action types, and its versioned
fixtures include generated and pinned vectors. The exact source locations are
listed in section 12.

| Kind | Public content | Checks before the state change |
| --- | --- | --- |
| `asset_orchard_ingress_v2` | The locally signed burn transaction, pool id, full asset id, integer amount, output commitment and encrypted output. No note field. | Chain and genesis, source and owner, issuer, sequence, amount, fee and commitment binding. The burn and pool insertion are one atomic step. |
| `shielded_swap_v1` | `swap_json`, the exact locally proven Asset-Orchard action: anchor, nullifiers, encrypted outputs, accounting commitments, pricing claim, proof and spend authorizations. | Both JSON levels decode strictly: no unknown fields, no supplied paths and no note openings. |
| `asset_orchard_private_egress_v1` | The proof and spend signature with the public destination, asset, amount, fee, policy and disclosure binding, anchor and nullifier. | Verification completes before the note is consumed and the transparent account credited, atomically. |

`asset_orchard_ingress_v1` and `asset_orchard_egress_v1` are refused on this
path. Registering A666 or pfUSDC never authorizes them.

### 5.1 The intent id and once-only admission

The intent id is a domain-separated hash under the tag
`tasknode.private-intent.v1` over the JSON pair `(domain, action_json)`. The
wallet's domain, `PrivateDomain`, has exactly three fields: `chain_id`,
`genesis_hash` and `protocol_version`.

The intent id therefore does not cover the address, pool or session. Those
values cannot be verified from the intent id itself. The gateway binds the
address to the native session principal and the pool to its route
configuration. The gateway recomputes the id and refuses a mismatch. It then
sends the validators the full domain: chain id, genesis hash, protocol
version, pool id and session scope (the gateway's session principal). Each
validator refuses a pool other than its configured one, and refuses the same
intent under a different domain or different bytes as
`private_intent_conflict`.

The admission rules are:

1. The original action text is retained byte for byte so the wallet can compare
   it with its persisted action.
2. Different bytes under an existing intent id are refused. The canonical batch
   and action are also deduplicated, so a new intent cannot replay a spend.
3. The batch is built with `build_atomic_shielded_action_batch`, with
   `atomic: true`, and its id is checked against the actual genesis.
4. Before any transport, the domain, intent, action hash, action bytes and batch
   id are reserved durably. `queued`, `dispatched`, `accepted`, `rejected` and
   `unknown` remain distinct states.
5. A retry resends the identical reserved batch. The gateway never re-signs or
   changes it, and a crash or duplicate request never starts another proof or
   round for the live record.
6. An HTTP acknowledgement means admitted and pending, nothing more.
   `receipt` is read-only. A missing or `unknown` receipt remains pending and
   never authorizes a replacement action.
7. A receipt carries the action, batch and transaction bindings; inclusion
   height, block hash and state root; outcome, code and fee; and finality
   evidence the wallet can check. That evidence is either a validator quorum
   certificate with receipt inclusion or an independently sourced validator
   view. Terminal rejections are typed and include their fee and state effects.

The lifecycle distinguishes four operations:

| Operation | Meaning |
| --- | --- |
| Transport retry | Resend the identical reserved batch for the existing record. |
| Fresh admission | Create a new record; this occurs only after the explicit abandonment path described below. |
| Certification round | Validator proposal, vote and certification. The gateway does not drive it. |
| State change | Atomic application after validator verification and certification. |

The defined state transitions are:

| From | To | Authority and evidence | Retry or finality meaning |
| --- | --- | --- | --- |
| No record | `queued` | Before transport, the gateway durably reserves the domain, intent, action hash and bytes, and batch id. | Transport may use only that reservation. |
| `queued` | `dispatched` | The gateway relays the identical reserved batch. | A repeated request remains tied to the same bytes and batch. |
| `dispatched` | `accepted` | The validator fleet accepts the batch; a crashed `unknown` record also settles this way when the batch lands. | The receipt supplies the inclusion and finality bindings. |
| `dispatched` | `rejected` | The validator fleet returns a typed terminal rejection. | The receipt states the code, fee and state effects. |
| `dispatched` | `unknown` | A crash leaves the validator record unresolved. | It remains pending; neither timeout nor a missing receipt authorizes replacement. |
| `unknown` | `accepted` | `postfiat-node private-action-resolve` finds that the batch landed on chain. | The existing record settles; no fresh record is created. |
| `queued` or `unknown` | `abandoned` | The operator runs `private-action-resolve --abandon --reason` on that validator. It is refused if the batch is on chain (`private_action_on_chain`) or the record is `dispatched` (`private_action_round_in_flight`). | The receipt reports `abandoned` with code `private_action_abandoned`, the reason, no fee and no state effects; the record remains in the admission log. |
| `abandoned` | New `queued` record | The wallet resubmits the same bytes under the same intent. | This is a fresh admission record, not a transport retry. |

The wallet compares the receipt's action with its byte-exact persisted action,
then checks the batch, transaction, inclusion and finality bindings. A gateway
cannot turn its acknowledgement into finality: validators alone verify and
certify, and the gateway has no validator key. The finality evidence is a
validator quorum block certificate (`validator_quorum_block_certificate_v1`).
When the configured validator sources disagree, the gateway reports `unknown`
with `private_action_source_mismatch`; when only some sources report a final
outcome, it stays pending with `private_receipt_unconfirmed`.

The normal retry path never creates a fresh admission or another round.
Abandonment is the only path to a fresh record. The resolve command holds the
admission store's exclusive lock, so no service step and no round runs while it
decides, and it refuses a dispatched record or a batch already on chain. The
fresh record carries the same bytes, so the same action hash and the same
canonical batch; the gateway's own record returns to `queued` only on an
explicit resubmission from the wallet, never on its own. A second application
of a spend is refused by consensus: a spent nullifier fails as
`duplicate_nullifier`.

## 6. Recovery reads

The wallet rebuilds its notes from one finalized snapshot. A snapshot holds:

- the chain id, genesis hash, protocol version and address;
- the finalized height, block hash and state root;
- ordered commitments, spent nullifiers and genuinely spendable accepted
  anchors;
- outputs and their ciphertexts;
- asset rows;
- the base and quote asset ids and pricing claim;
- the policy id and disclosure hash; and
- an explicit completeness flag.

The recovery rules are:

- Every page, market and account read is pinned to one snapshot token, with
  counts and cursors. Missing, duplicated, reordered or mixed-tip pages are
  refused.
- An empty pool before the first ingress is valid; missing history after it is
  not.
- Accepted anchors come from retained tree prefixes, not from a history of
  anchors that earlier spends used.
- Ciphertexts needed for phrase recovery are retained. The wallet scans with
  its own viewing key and reseals under a fresh key; it never sends that key.
- Asset rows bind issuer, code, precision and `requires_authorization` to
  `asset_info`; frozen or unauthorized lines fail.

The reads are `private_pool_snapshot`, `private_pool_page`,
`private_account_lines`, `private_pool_receipts` and
`private_pool_market`. The gateway pages a snapshot to completeness under the
client ceilings and refuses a mixed tip.

The client's current ceilings are 64 ciphertexts, 2 MiB of ciphertext,
4096 commitments, nullifiers or anchors, 16 assets, 1 MiB of action text, a
2 MiB request and a 4 MiB response. It fails closed above them.

## 7. Pricing and egress binding

A swap's pricing claim is the typed `AssetOrchardPricingClaim` built from
consensus-finalized NAV for the intended inclusion height. It contains the
epoch, reserve-packet hash, ratio, mode and band, and both asset tags. Stale,
halted or mismatched state is refused.

The NAV denominator changes units at height 608,
`ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT`. Consequently, a pre-608 ratio is
not a ce22 configuration.

The `asset_orchard_pricing_claim` read builds the claim with the function used
by consensus. The `asset_orchard_private_egress_binding` read returns
`ungoverned` because finalized state has no governed egress policy yet. A
fixture policy or an all-zero disclosure hash is never a substitute.

Consensus checks the epoch, reserve-packet hash, band, tags, halt flag and
ratio within the band. Three additional rules apply behind the governed
activation. Their governance keys are
`asset_orchard_pricing_enforcement_activation_height` and
`asset_orchard_pricing_max_nav_age_blocks`.

- **Staleness:** NAV older than the governed limit is refused as
  `asset_orchard_pricing_nav_stale`.
- **Governed pair:** a pair without a route in `pftl_uniswap_routes` is refused
  as `asset_orchard_pricing_pair_not_governed`. Governance must not activate
  these rules before the A666/pfUSDC route exists there.
- **Mode label:** an at-NAV claim with another mode label is refused as
  `asset_orchard_pricing_mode_mismatch`.

The egress policy is governed through a registry. Each record is an amendment
of kind
`asset_orchard_egress_policy_v1:<asset_id>:<disclosure_schema_hash>:<policy_id>`,
whose value is its effective height.

After the global switch
`asset_orchard_egress_policy_activation_height`, private egress must name the
asset's governed policy, and its disclosure hash must equal the governed
schema hash. Otherwise it is refused as
`asset_orchard_egress_policy_mismatch` or
`asset_orchard_egress_policy_not_governed`.

Records and the switch are ratified with
`postfiat-node ratify-asset-orchard-egress-policy` and
`ratify-asset-orchard-egress-policy-activation-height`. Until governance
writes the A666 and pfUSDC records with an agreed disclosure schema, the
egress-binding read remains `ungoverned`.

## 8. Readiness

Before admission, the fleet attests the deployed genesis, protocol, L1
revision, proof system, circuit, parameters and verifying keys, and confirms
that the real swap and egress verifiers are ready.

Atomic shielded batches are activated by ordinary signed governance at the
effective height. Before activation, execution refuses them with
`shielded_atomic_batch_not_active`. A mock verifier, copied store, unsafe file
signer or proxy-owned validator key never satisfies readiness. Readiness is
published by the running service and the deployed manifest.

The `private_readiness` read reports the pinned and loaded verifier identities
and governed activations. On `rpc-serve`, it also reports the server's own
admission switch.

## 9. Scope: admission, not exchange

The wallet team's accepted fixture is one owner spending two of its own notes
into two outputs. It proves the proof format, transport and recovery. W-1
delivers user-proven admission of such actions.

A real exchange—a user buying A666 with only a pfUSDC note against a
counterparty—needs participant-owned spend authorization, output-recipient
binding, quote expiry and reservation, with no participant's openings moving
to a server or the other wallet. That protocol, its circuit and its app
changes require their own specification.

The negotiated FX-fix path has consensus reservations but different fee and
expiry rules, so it is not a drop-in replacement. A custodial `pftl_swapd`
route does not close W-1.

The contract boundaries are consolidated here:

| Surface | Rule on main | Open |
| --- | --- | --- |
| Abandonment and resubmission | Only a `queued` or `unknown` record whose batch is not on chain and that has no round in flight can be abandoned, under the store lock; the resubmission reuses the bytes, action hash and canonical batch. | Nothing beyond the operator command; the record is never re-dispatched by it. |
| Gateway reconciliation | Every configured validator source receives the same reserved bytes; disagreement is `unknown` (`private_action_source_mismatch`); a final outcome from only some sources is pending (`private_receipt_unconfirmed`). | — |
| Intent route binding | The intent id excludes address, pool and session; the validators record pool and session scope with the intent and refuse a different domain (`private_intent_conflict`). | — |
| Snapshot token | `{height, block_hash, state_root}` in `schema.json`; valid until the next finalized block. | — |
| Gateway refusal taxonomy | The stable codes are listed under `refusals` in `schema.json`, for example `private_intent_mismatch`, `private_intent_conflict`, `private_action_duplicate` and `private_action_legacy_refused`. | — |
| Recovery scale | Current clients fail closed at the ceilings in section 6. | Production needs versioned paging and incremental tree, witness and scan support, not larger JSON limits. |

## 10. Acceptance

1. **Fixtures and vectors.** Versioned request and response fixtures; canonical
   action, intent and batch vectors; the deployed identity, verifier and
   activation manifest; and the full A666 and pfUSDC profile with issuer, proof
   profile and precision.
2. **Six-validator tests.** Both ingress assets, swap and egress. Invalid proof,
   stale anchor or NAV, wrong issuer, domain or recipient, replay or reused
   nullifier, inactive governance and a tampered receipt each fail with no
   unintended state change.
3. **Crash and recovery.** Crash after dispatch and lost responses, followed by
   a new session recovering the same persisted action with no replacement or
   double spend. A fresh-key full rescan, page boundaries and incomplete
   history. Lock or cancel leaves no helper process and no usable proof or
   submission.
4. **Secret inspection.** Logs and artifacts hold no secrets; the gateway holds
   no wallet or validator key; no remote prover receives openings; and an HTTP
   request or release setting cannot turn off validator verification.
5. **The ce22 run.** After review and the wallet team's HOLD approval: a native
   shield, private swap and unshield bounded at 1 pfUSDC, with exact public and
   private balances and fees, finality and fleet convergence.

### Implementation status

| Area | Current status and evidence |
| --- | --- |
| Native session | W-2 is done at `e037829f`. It is the transport for this contract, not part of it. |
| Contract fixtures | The shared fixtures and pinned vectors are at `346c79e4`. |
| Gateway and validator admission | The gateway routes are on main at `db9af4f2` and `d9095797`. The validators' admission store and round service are on main at `968da908` and `e37d77fd`; RPC exposure is also anchored at `d7de5ec7`. |
| Unknown resolution and abandonment | Chain resolution is at `657ce706`. The `abandoned` receipt outcome and retained reason are at `ea2213d0`. |
| Recovery reads | Snapshot, page and account reads are at `683a5b32`; receipt and market reads are at `0c396fc8`. Gateway paging to completeness and mixed-tip refusal are at `f8b9a838`. |
| Pricing and egress | Pricing-claim and egress-binding reads are on main at `93454729`. Governed pricing enforcement is at `ed89c916`; the governed egress-policy registry is at `899b26f8`. |
| Readiness | The readiness read is at `556cac38`, with the server admission switch also reported at `657ce706`. The release deployed on the devnet predates the read. |

The following test evidence is on main:

- The contract test.
- A gateway-to-node integration test with two real validators, covering restart
  and diverging-tip cases and a secret inspection of every file the proxy and
  nodes wrote (`3747a2e7`, `46c5a956`, `c308b7af`;
  `crates/node/tests/private_actions_gateway.rs`).
- Six-validator tests with ingress actions and with real swap and egress proofs
  (`e37d77fd`, `e5f8f5b8`), and a second release-only module with real swap
  proofs for the stale-anchor and the three pricing-rule refusals
  (`14c8e43e`).

Coverage maps to the acceptance criteria as follows:

| Acceptance item | Evidence on main | Remaining coverage |
| --- | --- | --- |
| 1. Fixtures and vectors | Contract test, shared fixtures and pinned vectors. | The wallet team's helper reproducing the vectors. |
| 2. Six-validator tests | Ingress actions, real swap and egress proofs, and the stale-anchor and pricing-rule refusals. | None; the wrong-issuer case is an asset/issuer binding mismatch, because no A666/pfUSDC issuer profile exists on the ingress path yet. |
| 3. Crash and recovery | The two-validator integration test covers restart and diverging-tip cases. | The remaining crash, lost-response, full-rescan, page-boundary, incomplete-history, lock and cancel clauses remain acceptance requirements. |
| 4. Secret inspection | The integration test inspects every file written by the proxy and nodes, checks that the proxy holds no validator key, and checks that no request or setting turns off verification. | — |
| 5. The ce22 run | None stated. | The ce22 run is not yet covered. |

## 11. What does not change

- **Wire formats.** `ShieldedAction` and its payloads keep their serialization.
- **The custody boundary.** Keys and openings stay on the device; validators
  keep proposal and vote signing.
- **Separate gates.** W-4, admission of outside money, and W-5, backing
  reconciliation, remain separate gates.

## 12. References

- Contract fixtures:
  `crates/node/tests/fixtures/private-actions-v1/` (`schema.json`,
  `vectors.json`, `manifest.json`, `assets.json`).
- Shared request and response types:
  `crates/types/src/private_actions_v1.rs`.
- Vector generator and pins:
  `crates/node/tests/private_actions_contract.rs`.
- Gateway: `wallet-proxy/private-actions.js`; admission service:
  `crates/node/src/private_action_admission.rs`,
  `crates/node/src/private_action_rpc.rs`.
- Reads: `crates/node/src/private_pool_reads.rs`,
  `crates/node/src/private_pool_receipt_market_reads.rs`,
  `crates/node/src/private_readiness_read.rs`.
- Integration and secret inspection:
  `crates/node/tests/private_actions_gateway.rs`.
- [Milestone](../plans/active/user-proven-shielded-actions-milestone.md).
- [Private egress disclosure schema, version 1](asset-orchard-private-egress-disclosure-schema-v1.md):
  the governed schema hash the egress policy records commit to.

### Pinned code locations

| Contract surface | Location |
| --- | --- |
| Public shielded payloads | `crates/types/src/shielded_bridge_governance.rs:369-536`: `ShieldedSwapActionPayload` at 369, `AssetOrchardIngressV2ActionPayload` at 400, `AssetOrchardPrivateEgressActionPayload` at 427 and the `ShieldedAction` enum at 509. |
| Wallet intent helper | `core/src/private_actions.rs:110` in the wallet repository. |
| Atomic batch builder | `build_atomic_shielded_action_batch` at `crates/node/src/consensus_artifacts.rs:1591`. |
| Recovery reads | `crates/node/src/private_pool_reads.rs` and `crates/node/src/private_pool_receipt_market_reads.rs`. |
| Pricing and egress reads | `asset_orchard_pricing_claim` and `asset_orchard_private_egress_binding` at `crates/node/src/asset_orchard_pricing_reads.rs:368` and `:533`. |
| NAV activation and pricing plan | `ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT` at `crates/node/src/execution_actions.rs:4386`; pricing plan at `:4426`. |
| Atomic-batch activation refusal | `crates/node/src/execution_actions.rs:3245`. |
| Readiness read | `crates/node/src/private_readiness_read.rs`. |

### Commit anchors

| Anchor | Evidence carried by the anchor |
| --- | --- |
| `45ba2334`, `48ecefef` | Wallet contract baseline and this repository's corresponding baseline. |
| `e037829f` | Completed W-2 native session transport. |
| `346c79e4` | Generated and pinned private-action contract fixtures and vectors. |
| `db9af4f2`, `d9095797` | Gateway private-action routes, validator-source submission and record reconciliation. |
| `968da908`, `d7de5ec7`, `e37d77fd` | Validator admission store, round service, RPC exposure and settlement of an `unknown` record when its batch lands. |
| `657ce706`, `ea2213d0` | Chain resolution, readiness reporting of the admission switch, and the reason-bearing `abandoned` receipt outcome. |
| `683a5b32`, `0c396fc8`, `f8b9a838` | Finalized snapshot, page, account, receipt and market reads; paging to completeness; mixed-tip refusal. |
| `93454729` | Consensus-built pricing-claim and private-egress-binding reads on main. |
| `ed89c916` | Governed staleness, pair and mode-label pricing enforcement. |
| `899b26f8` | Governed private-egress policy registry and activation. |
| `556cac38` | `private_readiness` reporting of pinned and loaded verifier identities and governed activations. |
| `3747a2e7`, `46c5a956`, `c308b7af` | Gateway-to-node integration with two real validators, restart and diverging-tip cases, and inspection of written files. |
| `e5f8f5b8` | Six-validator testing with real swap and egress proofs. |
| `14c8e43e` | Six-validator stale-anchor and pricing-rule refusals with real swap proofs. |