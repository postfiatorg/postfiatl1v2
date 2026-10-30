# User-proven shielded actions (W-1): specification

Date: 29 October 2026. Status: **locked** on 30 October 2026 at the full gate, run group `user-proven-shielded-actions-spec-20261029-r4`. The Text Improvement Harness full gate scores it without this paragraph, five runs per judge. r1: 85.13/100 (GPT-6 Astra Pro 87.00, Claude Fable 5.1 80.80, GLM 5.3 87.60; run group `user-proven-shielded-actions-spec-20261029-r1`; scored content SHA-256 `59bb690fa97b4754…`). r2, after one content pass bringing it up to date with main: 84.20/100 (GPT-6 Astra Pro 87.60, Claude Fable 5.1 81.00, GLM 5.3 84.00; `-r2`; `41ccab38ee41eef6…`). r3, after one improvement pass on the r2 critiques through `openai/gpt-5.6-sol-pro` (AGENTS.md step 4) and a fact check against main: 85.40/100 (GPT-6 Astra Pro 86.20, Claude Fable 5.1 82.40, GLM 5.3 87.60; `-r3`; `d2104b41ecf0c71a…`). r4, after one improvement pass on the r3 critiques through `openai/gpt-5.6-sol-pro` and a fact check against main: 86.27/100 (GPT-6 Astra Pro 85.60, Claude Fable 5.1 84.60, GLM 5.3 88.60; `-r4`; `95077bcfff875093…`), at or above 86, so it is locked; this is M1 of the [milestone](../plans/active/user-proven-shielded-actions-milestone.md). Deployed state on 30 October 2026: release `vote-records-20261030` (source `ae70d66d`) runs on all six ce22 validators since 11:14Z with the whole private path present but switched off: `rpc-serve` runs without `--allow-private-action-admission`, and the pricing enforcement and the egress policy are ungoverned (sections 7 and 8). It authorizes no deployment and no transaction.

## 1. BLUF

1. The wallet team's native Mac app proves actions for the two action-profile assets, A666 and pfUSDC, on the device. The deployed chain currently accepts no private-action submissions because admission is switched off. The earlier custody path takes shielded actions through a wallet proxy that holds validator material and drives a certified consensus round. W-1 replaces that custody model: a gateway relays immutable public bytes, while validators admit, verify, certify and apply them.
2. The public contract remains the existing `postfiat_types::ShieldedAction` serialization, restricted to `asset_orchard_ingress_v2`, `shielded_swap_v1` and `asset_orchard_private_egress_v1`. The legacy v1 ingress and egress payloads expose note openings or spend material and remain refused.
3. Normal admission is once-only: before transport, the gateway reserves the intent, the exact action bytes and the canonical batch id; retries reuse that reservation and never create another proof or round for the live record. An operator may abandon, on one validator, only a `queued` or `unknown` record whose batch is not on chain and that has no round in flight; a resubmission carries the same bytes, the same action hash and the same canonical batch, and consensus applies that batch at most once. Section 5.1 is the single normative lifecycle for gateway records, per-validator records, abandonment and resubmission.
4. Recovery uses finalized public reads pinned to one snapshot token, meaning one `{height, block_hash, state_root}` view. Pricing derives from consensus-finalized net asset value (NAV). Pricing and egress are governed consensus rules, not proxy policy. W-1 covers user-proven admission, not a two-party exchange. W-2, the completed native session transport, carries this contract but is not part of it.

### Terms used below

| Term | Meaning in this document |
| --- | --- |
| **W-1** | User-proven admission of locally proved shielded actions without proxy custody. |
| **W-2** | The completed native authenticated session used to transport this contract. |
| **W-4 / W-5** | Admission of outside money / backing reconciliation; both remain separate gates. |
| **A666 / pfUSDC** | The two assets named by the action profiles, ingress tests, governed pricing pair and egress policy. The first approved live run is bounded at 1 pfUSDC. |
| **Asset-Orchard** | The shielded action, pool, pricing and egress-policy subsystem named by the payloads and reads below. |
| **NAV** | The consensus-finalized pricing input from which an `AssetOrchardPricingClaim` is built. |
| **ce22** | The controlled devnet `postfiat-wan-devnet-2`, named by genesis-hash prefix `ce22ca8c…`, where the first approved live dust run takes place. |
| **Height 608** | `ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT`, where the NAV denominator changes units. A ratio using the pre-608 units is not a ce22 configuration. |
| **HOLD approval** | The wallet team's required approval before the ce22 live run. |
| **Gateway** | The W-2-authenticated relay that checks, durably reserves and forwards exact public bytes without wallet keys, validator keys or validator filesystem access. |
| **Validator fleet** | The validators (six on ce22) that verify, propose, vote, apply and certify actions. |
| **Gateway record** | The gateway's durable reservation and reconciled view of the configured validator sources. |
| **Validator record** | One validator's admission-log record. Abandonment changes this record on that validator only. |
| **Intent** | The domain-separated identity of an action submission, derived as specified in section 5.1. |
| **Canonical atomic batch** | The deterministic, all-or-nothing batch built from one admitted action against the actual genesis. |
| **Anchor** | A retained shielded-tree root against which a proof is verified. |
| **Nullifier** | The public spent-note marker used by consensus to prevent a note from being spent twice. |
| **Certification round** | Validator proposal, vote and certification for a batch. |
| **Proposer height** | A block height at which a particular validator is designated to propose. |
| **Quorum certificate** | Validator quorum evidence that certifies a block and its included receipt. |
| **Session principal** | The authenticated identity established by the W-2 session and sent as `session_scope`. |
| **Snapshot token** | The finalized `{height, block_hash, state_root}` tuple to which every recovery page is pinned. |
| **a651 / a652** | Legacy NAV test assets whose proxy quote registry is retired; A666 and pfUSDC replace them. |
| **FX-fix** | A negotiated swap path that settles at one public, governed foreign-exchange fix price. |
| **`pftl_swapd`** | A custodial swap service: it holds the user's funds, so it is not user-proven admission. |
| **`unknown` / `abandoned`** | An unresolved validator admission record / an operator-resolved validator record whose batch was not on chain and whose abandonment reason is retained. |

### Contract flow

| Step | Event |
| --- | --- |
| 1. Submit | The wallet proves, signs and persists the action locally, then submits its exact serialization over the W-2 session. |
| 2. Reserve | The gateway checks shape and size, recomputes the intent id, builds the canonical atomic batch and durably reserves the domain, intent, action hash, action bytes and batch id. |
| 3. Relay | The gateway sends the identical reserved bytes to every configured validator source. |
| 4. Certify | A validator waits for its proposer height, performs the proof, authorization, anchor, nullifier, pricing and policy checks, then uses ordinary proposal, vote and apply. |
| 5. Receipt and recovery | The HTTP acknowledgement reports admission as pending. The wallet checks a read-only receipt for finality and rebuilds notes from finalized snapshot reads. |

## 2. Goal

A wallet that holds its own keys submits a locally proven shielded action to any
gateway, and the validator fleet alone accepts it, rejects it or leaves it
pending. The gateway holds no wallet key, validator key or validator data
directory.

A crash, timeout or repeated request must not create a second spend, a new
gateway reservation or an independently rebuilt batch. A normal retry reuses
the live records and bytes. Explicit per-validator abandonment and fresh
resubmission follow section 5.1. Consensus batch and nullifier checks apply a
batch at most once.

The wallet rebuilds its notes from finalized public reads alone, without sending
a viewing key anywhere. The first live use is one approved dust run of
1 pfUSDC on ce22.

Source: the wallet team's handoff
[`docs/v3-w1-integration.md`](https://github.com/postfiatorg/tasknode-apple/blob/main/docs/v3-w1-integration.md)
in the private repository `postfiatorg/tasknode-apple`, at the wallet contract
baseline against this repository's corresponding baseline, and section 3 of
[the wallet findings handoff](../handoffs/2026-10-29___goodalexander__tasknode_wallet_findings_for_the_l1.md).
The baseline commits are listed in section 12. This document restates that
contract with this repository's references.

## 3. The current custody model

Shielded swaps on the wallet proxy are operator-run, not user-proven.

| Where | Named code surface | What it does | Why it blocks W-1 |
| --- | --- | --- | --- |
| `wallet-proxy/navswap-config-bridge.js` | `shieldedNavswapIngressConfig` | Ingress configuration needs a node binary, validator data directory, topology and validator key file. | The HTTP process reads validator state and holds a signing key. |
| `wallet-proxy/navswap-config-bridge.js` | `shieldedNavswapSwapConfig` | Swap availability depends on a local Asset-Orchard service. | A remote prover service is in the path; an on-device proof needs none. |
| `wallet-proxy/navswap-config-bridge.js` | `shieldedNavswapQuoteConfig` | Builds a651/a652 quotes and the proxy's own liquidity policy. | The registry is retired; prices are proxy-derived, not validator-derived. |
| `wallet-proxy/navswap-shielded.js` | `buildShieldedCertifiedRoundArgs` | Runs `postfiat-node transport-peer-certified-batch-round` with `--key-file`. | The proxy drives the certified round with a validator key. |
| `wallet-proxy/navswap-shielded.js` | `createShieldedSwapBatchViaLocalService` | Posts the swap to `/asset-orchard/swap-batch` on the local service. | Batch construction depends on that service. |
| `wallet-proxy/navswap-shielded.js` | `executeShieldedNavswapSwap` | Executes and certifies the swap on the HTTP side. | Certification belongs to the validators. |
| `crates/node/src/rpc_cli.rs` | `is_orchard_batch_create_method` and the `shield_batch_*` builders | `shield_batch_orchard`, `shield_batch_orchard_deposit`, `shield_batch_orchard_withdraw` and `shield_batch_swap` build batches. | A built batch is not an admitted transaction: there is no durable admission, finality or receipt contract. |

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
`rpc-serve --allow-private-action-admission`. A validator record waits until
that validator is the proposer for the next height. A record left `unknown` by
a crash settles as accepted when its batch lands.

The gateway submits the reserved bytes to every configured validator source
and reconciles their records. The gateway record and each validator record are
separate. Their lifecycle and reconciliation rules are in section 5.1; the
scope boundaries are in section 9.

On the deployed release, no RPC unit passes
`--allow-private-action-admission`. `private_action_submit` therefore returns
`rpc_method_not_allowed`, and the readiness admission switch reports `off`.

## 5. The public action contract

The normative wire type is `postfiat_types::ShieldedAction`. Its request and
response contract uses the shared private-action types, and its versioned
fixtures include generated and pinned vectors. The exact source locations are
listed in section 12.

| Kind | Public content | Checks before the state change |
| --- | --- | --- |
| `asset_orchard_ingress_v2` | The locally signed burn transaction, pool id, full asset id, integer amount, output commitment and encrypted output. No note field. | Chain and genesis, source and owner, issuer, sequence, amount, fee and commitment binding. The burn and pool insertion are one atomic step. |
| `shielded_swap_v1` | `swap_json`, the exact locally proven Asset-Orchard action: anchor, nullifiers, encrypted outputs, accounting commitments, pricing claim, proof and spend authorizations. | Both JSON levels decode strictly: no unknown fields, supplied paths or note openings. |
| `asset_orchard_private_egress_v1` | The proof and spend signature with the public destination, asset, amount, fee, policy and disclosure binding, anchor and nullifier. | Verification completes before the note is consumed and the transparent account credited, atomically. |

`asset_orchard_ingress_v1` and `asset_orchard_egress_v1` are refused on this
path. Registering A666 or pfUSDC never authorizes them.

### 5.1 The intent id and once-only admission

This subsection is the normative source for retry, record ownership,
abandonment and resubmission.

The intent id is a domain-separated hash under the tag
`tasknode.private-intent.v1` over the JSON pair `(domain, action_json)`. The
wallet's domain type, `PrivateDomain`, has exactly three fields: `chain_id`,
`genesis_hash` and `protocol_version`.

The intent id does not cover the address, pool or session. Those values cannot
be verified from the intent id itself. The gateway binds the address to the
native session principal and the pool to its route configuration. It
recomputes the id and refuses a mismatch. It then sends validators the full
domain: chain id, genesis hash, protocol version, pool id and `session_scope`,
which is the gateway's session principal. Each validator refuses a pool other
than its configured pool and refuses the same intent under a different domain
or different bytes as `private_intent_conflict`.

The admission invariants are:

1. The original action text is retained byte for byte so the wallet can compare
   it with its persisted action.
2. Different bytes under an existing intent id are refused. The action and
   canonical batch are also deduplicated, so a new intent cannot replay a
   spend.
3. The batch is built with `build_atomic_shielded_action_batch`, with
   `atomic: true`, and its id is checked against the actual genesis.
4. Before transport, the gateway durably reserves the domain, intent, action
   hash, action bytes and batch id. `queued`, `dispatched`, `accepted`,
   `rejected` and `unknown` remain distinct gateway states.
5. A transport retry resends the identical reserved batch. It does not create
   another gateway reservation, action proof or independently rebuilt batch.
6. An HTTP acknowledgement means admitted and pending, nothing more.
   `receipt` is read-only. A missing or `unknown` receipt remains pending and
   never authorizes a replacement action.
7. A receipt carries the action, batch and transaction bindings; inclusion
   height, block hash and state root; outcome, code and fee; and checkable
   finality evidence. That evidence is a validator quorum block certificate
   (`validator_quorum_block_certificate_v1`) for the inclusion block, and the
   gateway reports a final outcome only when at least two configured validator
   sources return the same one. Terminal rejections are typed and include their
   fee and state effects.
8. An `abandoned` validator record is the explicit exception to active-record
   deduplication: the same intent and identical bytes may create a fresh
   validator admission record after eligible abandonment. It still produces
   the same action hash and canonical batch, and consensus replay protection
   still applies.

The lifecycle distinguishes four operations:

| Operation | Meaning |
| --- | --- |
| Transport retry | Resend the identical reserved batch for the existing gateway and validator records. |
| Fresh admission | Create a new validator admission record only after the explicit abandonment path below and an explicit wallet resubmission. |
| Certification round | Validator proposal, vote and certification. The gateway does not drive it. |
| State change | Atomic application after validator verification and certification. |

#### Gateway record

The gateway record is the durable reservation and reconciliation view. It is
not a validator admission-log record.

| From | To | Authority and evidence | Retry or finality meaning |
| --- | --- | --- | --- |
| No record | `queued` | Before transport, the gateway durably reserves the domain, intent, action hash and bytes, and batch id. | Transport may use only that reservation. |
| `queued` | `dispatched` | The gateway relays the identical reserved batch to every configured validator source. | A repeated request remains tied to the same bytes and batch. |
| `dispatched` | `accepted` | At least two configured sources return the same accepted outcome, with a block certificate for its inclusion block. | The receipt is final only after the wallet checks those bindings. |
| `dispatched` | `rejected` | At least two configured sources return the same typed terminal rejection. | The receipt states the code, fee and state effects. |
| `dispatched` | `unknown` | The sources disagree (`private_action_source_mismatch`) or a source reports its record `unknown`. | It remains pending; neither timeout nor a missing receipt authorizes replacement. |
| Any non-final state | `abandoned` | A configured source reports its validator record `abandoned`. | Terminal until the wallet resubmits; the gateway notes the abandoning sources and never resends on its own. |
| `abandoned` | `queued` | The wallet explicitly resubmits. The gateway reuses the same bytes, action hash and canonical batch and sends them to the abandoning sources. | The gateway never performs this reset autonomously. |

When configured validator sources disagree, the gateway reports `unknown` with
`private_action_source_mismatch`. When only one source reports a final
outcome, the record stays pending with `private_receipt_unconfirmed`.

#### Per-validator record

Each validator keeps its own admission-log record. The command
`postfiat-node private-action-resolve` changes only the record on the validator
where it runs.

| From | To | Authority and evidence | Retry or finality meaning |
| --- | --- | --- | --- |
| No record | `queued` | The validator admits the gateway's domain and reserved action and batch bindings. | A repeat submission returns the existing live record. |
| `queued` | `dispatched` | At its proposer height, the validator starts handling the identical reserved batch through ordinary proposal, vote and apply. | The record remains bound to the same action and batch. |
| `dispatched` | `accepted` | The validator fleet accepts the batch. A crashed `unknown` record also settles this way when the batch lands. | The receipt supplies the inclusion and finality bindings. |
| `dispatched` | `rejected` | The validator fleet returns a typed terminal rejection. | The receipt states the code, fee and state effects. |
| `dispatched` | `unknown` | A crash leaves that validator's record unresolved. | It remains pending; a new submission returns the same record. |
| `unknown` | `accepted` | `private-action-resolve` or service reconciliation finds that the batch landed on chain. | The existing record settles; no fresh record is created. |
| `queued` or `unknown` | `abandoned` | The operator runs `private-action-resolve --abandon --reason` on that validator. The command holds that admission store's exclusive lock. It is refused if the batch is on chain (`private_action_on_chain`) or the record is `dispatched` (`private_action_round_in_flight`). | The receipt reports `abandoned` with code `private_action_abandoned`, the reason, no fee and no state effects. The record remains in that validator's admission log. |
| `abandoned` | New `queued` record | The wallet resubmits the same bytes under the same intent. | This is a fresh validator admission record, not a transport retry. |

The exclusive lock means no service step and no round runs on that validator
while its operator decides. Abandonment changes only that validator's record:
another validator keeps its own record, and if its round lands the batch, the
batch applies once. The normal retry path creates no fresh admission and
rebuilds no proof or batch. A second application of a spend is refused by
consensus: a spent nullifier fails as `duplicate_nullifier`.

The wallet compares the receipt's action with its byte-exact persisted action,
then checks the batch, transaction, inclusion and finality bindings. A gateway
cannot turn its acknowledgement into finality: validators alone verify and
certify, and the gateway has no validator key.

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
`ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT`. A ratio calculated in the units
used before height 608 is therefore not a valid ce22 configuration.

The `asset_orchard_pricing_claim` read builds the claim with the function used
by consensus. The `asset_orchard_private_egress_binding` read returns
`ungoverned` when finalized state has no governed egress policy. A fixture
policy or an all-zero disclosure hash is never a substitute.

Consensus checks the epoch, reserve-packet hash, band, tags, halt flag and
ratio within the band. Three additional rules apply behind governed
activation. Their governance keys are
`asset_orchard_pricing_enforcement_activation_height` and
`asset_orchard_pricing_max_nav_age_blocks`.

- **Staleness:** NAV older than the governed limit is refused as
  `asset_orchard_pricing_nav_stale`.
- **Governed pair:** a pair without a route in the governance route table
  `pftl_uniswap_routes` is refused as
  `asset_orchard_pricing_pair_not_governed`. Governance must not activate
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

On the deployed release:

- pricing-enforcement activation and `pricing_max_nav_age_blocks` are both
  `null`, so governed pricing enforcement is inactive;
- `a666_pfusdc_route` is `null`;
- `pftl_uniswap_routes` does contain A666/pfUSDC routes, but two routes match
  the asset codes A666 and PFUSDC;
- `a666_pfusdc_route` in
  `crates/node/src/private_readiness_read.rs` selects a route only when exactly
  one route matches, so the readiness reason does not mean that the route table
  lacks an A666/pfUSDC route;
- the egress policy remains ungoverned, and no W-1 pricing or egress activation
  is ratified.

Another implementation lane is working on the route ambiguity. The fix is not
landed. The milestone tracks it under S7.1; S7.1 remains open until pricing is
activated, exactly one governed A666/pfUSDC route is selected and readiness
reports ready.

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

A `private_readiness` read for pool `asset-orchard-v1` was taken from all six
ce22 validators on 30 October 2026 at 11:31Z. The six responses were
byte-identical at finalized height 1168 (block hash and state root in
section 12). They reported:

- the build revision for release `vote-records-20261030`, profile `release`;
- swap verifier `asset_orchard.swap.pricing_bound.v4`, loaded and matching its
  pin;
- egress verifier `asset_orchard.private_egress.v2`, loaded and matching its
  pin;
- atomic shielded batches effective at height 483 and active at the next
  height;
- pricing-enforcement activation and `pricing_max_nav_age_blocks` both `null`;
- `a666_pfusdc_route` `null`;
- admission switch `off`;
- `ready` false, with reasons
  `asset_orchard_pricing_enforcement_activation_absent` and
  `a666_pfusdc_route_absent`.

The deployed release therefore carries the readiness read but is not ready for
W-1 admission. The whole private path is installed and switched off.
`private_action_submit` returns `rpc_method_not_allowed`.

The contract manifest now records the deployed identity, release, both
verifier pins and atomic-batch activation height 483 from this read. Milestone
S1.3, the deployed identity, verifier and activation manifest, is ticked. The
S7.1 sub-item “published from the deployed release” is also ticked, while S7.1
itself remains open for the pricing activation and uniquely selected governed
route.

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
| Abandonment and resubmission | Section 5.1 is normative. Abandonment is per validator, carries a reason and is allowed only under that validator's store lock for a `queued` or `unknown` record whose batch is not on chain; the resubmission reuses the bytes, action hash and canonical batch. | Nothing beyond the operator command; the record is never re-dispatched by it. |
| Gateway reconciliation | Every configured validator source receives the same reserved bytes. Disagreement is `unknown` with `private_action_source_mismatch`; a final outcome from only some sources remains pending with `private_receipt_unconfirmed`. | — |
| Intent route binding | The intent id excludes address, pool and session. Validators record pool and session scope with the intent and refuse a different domain as `private_intent_conflict`. | — |
| Snapshot token | `{height, block_hash, state_root}` in `schema.json`; valid until the next finalized block. | — |
| Gateway refusal taxonomy | The stable codes are listed under `refusals` in `schema.json`, including `private_intent_mismatch`, `private_intent_conflict`, `private_action_duplicate` and `private_action_legacy_refused`. | — |
| Recovery scale | Current clients fail closed at the ceilings in section 6. | Production needs versioned paging and incremental tree, witness and scan support, not larger JSON limits. |

## 10. Acceptance

1. **Fixtures and vectors.** Versioned request and response fixtures; canonical
   action, intent and batch vectors; the deployed identity, release, verifier
   and activation manifest; and the full fixture profiles for A666 and pfUSDC,
   including issuer, proof profile and precision.
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
| Native session | W-2 is complete in the completed native-session transport change. It transports this contract but is not part of it. |
| Contract fixtures | The shared fixtures and pinned vectors are in the private-action fixtures and vectors change. The deployed manifest values are in the deployed-manifest recording change. |
| Gateway and validator admission | The gateway private-action route changes provide gateway routes, validator-source submission and record reconciliation. The validator admission and RPC changes provide the admission store, round service, RPC exposure and settlement of an `unknown` record when its batch lands. |
| Unknown resolution and abandonment | The chain-resolution change provides chain resolution and readiness reporting of the admission switch. The abandonment-receipt change provides the `abandoned` outcome and retained reason. |
| Recovery reads | The snapshot-read changes provide snapshot, page, account, receipt and market reads. The gateway completeness change provides paging to completeness, mixed-tip refusal and incomplete-history handling. |
| Pricing and egress | The pricing and egress read change is on main. The governed pricing-enforcement change and governed egress-policy change provide the governed rules and registry. On the deployed release, neither pricing enforcement nor the egress policy is governed for W-1. |
| Readiness | The readiness-read change reports pinned and loaded verifier identities and governed activations. The deployed release includes that read, reports the admission switch `off` and reports `ready` false for the two reasons in section 8. |
| Deployed release | `vote-records-20261030` runs on all six ce22 validators, built from main at the deployed-release source commit. The rollout completed without rollback. |

The following test evidence is on main:

- The contract test.
- A gateway-to-node integration test with two real validators, covering restart,
  diverging-tip cases and secret inspection of every file written by the proxy
  and nodes. Its evidence is the two-validator gateway integration changes in
  section 12; the test is
  `crates/node/tests/private_actions_gateway.rs`.
- Six-validator tests with ingress actions and real swap and egress proofs,
  provided by the validator admission and six-validator proof-test changes.
- A second release-only module with real swap proofs for stale-anchor and the
  three pricing-rule refusals, provided by the pricing-refusal test change.
- `wallet-proxy/test_private_actions.js`, in the gateway completeness change,
  refuses a mixed tip and clears completeness flags on incomplete node
  history.

Coverage maps to the acceptance criteria as follows:

| Acceptance item | Evidence on main | Remaining coverage |
| --- | --- | --- |
| 1. Fixtures and vectors | Contract test, shared fixtures, pinned vectors and the manifest populated from the deployed readiness read with identity, release, verifier pins and atomic-batch activation 483. | The wallet team's helper reproducing the vectors. |
| 2. Six-validator tests | Ingress actions, real swap and egress proofs, invalid proof, stale-anchor and pricing-rule refusals, domain and recipient failures, replay and reused-nullifier failures, inactive governance and tampered-receipt handling. | None. The wrong-issuer case is covered as an asset/issuer binding mismatch (`asset_orchard_ingress_v2_bad_payload`), because no A666/pfUSDC issuer profile exists on the ingress path yet. |
| 3. Crash and recovery | The six-validator suite kills the round service after `dispatched`; the record becomes `unknown`, a new submission returns the same record, and the next proposer lands the same reserved batch exactly once. It also kills the service after the certified send and settles the record as accepted from the chain. A fresh-key rescan reads outputs through `private_pool_page` with page limit 2, below the total, and matches the snapshot count. The two-validator gateway test kills the proxy after submit returns `queued`, restarts it on the same store, and confirms that repeat submit returns the same record and lands once. The gateway test refuses mixed tips and clears completeness flags for incomplete node history. | The app-side lock and cancel clauses, and versioned paging with incremental scan in the app, remain wallet-team acceptance requirements. |
| 4. Secret inspection | The integration test inspects every file written by the proxy and nodes, checks that the proxy holds no validator key, and checks that no request or setting turns off verification. | — |
| 5. The ce22 run | No live run has been made. | The run waits for review and the wallet team's HOLD approval. |

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
- Pricing and egress reads:
  `crates/node/src/asset_orchard_pricing_reads.rs`.
- Integration and secret inspection:
  `crates/node/tests/private_actions_gateway.rs`.
- Gateway tests: `wallet-proxy/test_private_actions.js`.
- [Milestone](../plans/active/user-proven-shielded-actions-milestone.md).
- [Private egress disclosure schema, version 1](asset-orchard-private-egress-disclosure-schema-v1.md):
  the governed schema hash to which egress-policy records commit.

### Pinned code locations

Line pins are review aids and may move as files change. Locate the named symbol
first; use the line only to identify the reviewed revision.

| Contract surface | Pinned location |
| --- | --- |
| Public shielded payloads | `crates/types/src/shielded_bridge_governance.rs`: `ShieldedSwapActionPayload` at line 369, `AssetOrchardIngressV2ActionPayload` at line 400, `AssetOrchardPrivateEgressActionPayload` at line 427, `ShieldedAction` at line 509; the reviewed payload block ends at line 536. |
| Wallet intent helper | `core/src/private_actions.rs` in the wallet repository, line 110. |
| Atomic batch builder | `build_atomic_shielded_action_batch` in `crates/node/src/consensus_artifacts.rs`, line 1591. |
| Recovery reads | `crates/node/src/private_pool_reads.rs` and `crates/node/src/private_pool_receipt_market_reads.rs`. |
| Pricing and egress reads | `asset_orchard_pricing_claim` at line 368 and `asset_orchard_private_egress_binding` at line 533 in `crates/node/src/asset_orchard_pricing_reads.rs`. |
| NAV activation and pricing plan | `ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT` at line 4386 and `asset_orchard_swap_pricing_plan` at line 4426 in `crates/node/src/execution_actions.rs`. |
| Atomic-batch activation refusal | `shielded_atomic_batch_not_active` in `crates/node/src/execution_actions.rs`, line 3245. |
| Legacy batch builders | `is_orchard_batch_create_method` and the `shield_batch_*` builders in `crates/node/src/rpc_cli.rs`, line 2086. |
| Legacy proxy configuration | `shieldedNavswapIngressConfig` at line 783, `shieldedNavswapSwapConfig` at line 858 and `shieldedNavswapQuoteConfig` at line 996 in `wallet-proxy/navswap-config-bridge.js`. |
| Legacy proxy execution | `buildShieldedCertifiedRoundArgs` at line 1421, `createShieldedSwapBatchViaLocalService` at line 2292 and `executeShieldedNavswapSwap` at line 2386 in `wallet-proxy/navswap-shielded.js`. |
| Readiness and route selection | `private_readiness` and `a666_pfusdc_route` in `crates/node/src/private_readiness_read.rs`. |

### Commit references

| Named change | Commit | Evidence carried by the change |
| --- | --- | --- |
| Wallet contract baseline and repository corresponding baseline | `45ba2334`, `48ecefef` | Wallet handoff baseline and this repository's corresponding baseline. |
| Completed native-session transport change | `e037829f` | Completed W-2 native session transport. |
| Private-action fixtures and vectors change | `346c79e4` | Generated and pinned private-action contract fixtures and vectors. |
| Gateway private-action route changes | `db9af4f2`, `d9095797` | Gateway routes, validator-source submission and gateway record reconciliation. |
| Validator admission and RPC changes | `968da908`, `d7de5ec7`, `e37d77fd` | Validator admission store, round service, RPC exposure, `unknown` settlement when the batch lands, six-validator ingress tests and covered crash/recovery cases. |
| Chain-resolution and abandonment-receipt changes | `657ce706`, `ea2213d0` | Chain resolution, readiness reporting of the admission switch, and the reason-bearing `abandoned` receipt outcome. |
| Snapshot-read and gateway completeness changes | `683a5b32`, `0c396fc8`, `f8b9a838` | Finalized snapshot, page, account, receipt and market reads; paging to completeness; mixed-tip refusal; incomplete-history handling. |
| Pricing and egress read change | `93454729` | Consensus-built pricing-claim and private-egress-binding reads. |
| Governed pricing-enforcement change | `ed89c916` | Governed staleness, pair and mode-label pricing enforcement. |
| Governed egress-policy change | `899b26f8` | Governed private-egress policy registry and activation. |
| Readiness-read change | `556cac38` | `private_readiness` reporting of pinned and loaded verifier identities and governed activations. |
| Two-validator gateway integration changes | `3747a2e7`, `46c5a956`, `c308b7af` | Gateway-to-node integration with two real validators, proxy restart, diverging-tip cases and inspection of written files. |
| Six-validator proof-test change | `e5f8f5b8` | Six-validator testing with real swap and egress proofs. |
| Pricing-refusal test change | `14c8e43e` | Six-validator stale-anchor and pricing-rule refusals with real swap proofs. |
| Deployed-release source change | `ae70d66d` | Source for `vote-records-20261030`, equal to main at this commit and reported as the deployed build revision. |
| Deployed-manifest recording change | `92425854` | Manifest values recorded from the deployed readiness read: identity, release, verifier pins and atomic-batch activation 483. |

### Deployed observation

| Observation | Values |
| --- | --- |
| `private_readiness` on all six ce22 validators, 2026-10-30 11:31Z | Height 1168, block `a20e406e…`, state root `39771b74…`; build `ae70d66d`; release executable SHA-256 `87cfe797…`. |
