# Vault Bridge NAVCoin Profile Implementation Spec

Status: draft implementation spec
Target repo: `postfiatl1v2`

## Design decision

The PFTL primitive is not a separate stablecoin subsystem and it is not named
after one product instance. It is a generic vault-backed ERC20 bridge profile
for NAV-tracked issued assets.

It should be implemented as a constrained NAVCoin profile over an issued asset:

```text
issued asset
  + nav_asset_register
  + nav_profile_register
  + nav_reserve_submit / nav_epoch_finalize
  + source-labeled reserve receipts
  + bucket allocation accounting
  = vault-backed issued asset
```

A concrete deployment can configure any supported source ERC-20, but the code, transaction
kinds, hashes, status APIs, and EVM vault contract use generic `vault_bridge` /
`erc20_bridge` naming. The source token address and source domain are deployment
configuration, not the protocol identity.

The core invariant is:

```text
for each vault bridge asset_id and source_bucket:
  allocated_atoms <= counted_value_atoms
```

where:

```text
allocated_atoms =
  outstanding_vault_bridge_atoms
  + nav_subscription_allocations_atoms
  + redemption_queue_atoms
  + other_protocol_allocations_atoms
```

This makes each configured vault bridge asset a NAVCoin-style,
reserve-tracked issued asset. A dollar-denominated instance can use a fixed USD
unit, but the consensus primitive is asset-agnostic.

## Non-goals

- Do not create a bespoke stablecoin subsystem or token-specific consensus path.
- Do not add spot redemption at NAV as the default path.
- Do not pool strong and weak source domains in the MVP.
- Do not allow operator-controlled custody attestations to become countable
  vault bridge asset capacity.
- Do not make the issuer or operator the withdrawal custodian.
- Do not depend on Circle Arc for launch.
- Do not add floating-point risk models or nondeterministic oracle calls.
- Do not treat haircuts as legal recovery guarantees.

## Existing codebase primitives to reuse

The implementation should build on these existing surfaces:

| Existing primitive | File | vault bridge asset use |
|---|---|---|
| `AssetDefinition`, trustlines, issued transfer | `crates/types/src/lib_parts/ledger_assets.rs` | vault bridge asset balances are ordinary issued-asset balances. |
| `NavTrackedAsset` | `crates/types/src/lib_parts/ledger_assets.rs` | vault bridge asset is registered as a NAV-tracked asset. |
| `NavProofProfile` | `crates/types/src/lib_parts/ledger_assets.rs` | vault bridge asset registers a profile whose `valuation_policy_hash` commits to source-domain policy. |
| `NavReservePacket` | `crates/types/src/lib_parts/ledger_assets.rs` | Epoch-level reserve/supply packet for vault bridge asset. |
| `nav_asset_register` | `crates/types/src/lib_parts/transactions_mempool_receipts.rs` | Registers vault bridge asset as a NAV asset. |
| `nav_profile_register` | same | Registers proof/freshness/challenge parameters. |
| `nav_reserve_submit` / `nav_epoch_finalize` | same + `crates/execution/src/lib_parts/nft_escrow_asset_state.rs` | Publishes and finalizes counted reserve state. |
| `nav_halt` | same | Emergency pause for the entire vault bridge asset. |
| `PFTLWithdrawalVerifier` | `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol` | Verifies threshold-signed PFTL withdrawal packets, exposes a challenge/finality window, and authorizes exact packet/hash pairs for vault submission. |
| `ERC20BridgeVault` | `crates/ethereum-contracts/src/ERC20BridgeVault.sol` | Holds the configured ERC-20 source asset, emits canonical deposit events, requires accepted PFTL withdrawal verifier records, applies vault challenge/finality, and pays users directly. |
| `MarketOpsEnvelope` / replay bundle | `crates/types`, `crates/node` | Later integration point for NAVCoin subscriptions and market operations; not required for vault bridge asset MVP. |

## vault bridge asset as a NAVCoin profile

### Asset registration

vault bridge asset starts as a normal issued asset plus NAV registration.

Recommended registration shape:

```text
AssetDefinition:
  asset_id = issued_asset_id(issuer, code, metadata)
  issuer = vault_bridge_issuer
  precision = configured source-token atom precision
  max_supply = optional policy cap
  flags = issuer_can_clawback / freeze policy as explicitly chosen

NavTrackedAsset:
  asset_id = vault bridge asset_id
  issuer = vault_bridge_issuer
  reserve_operator = vault_bridge_reserve_operator
  proof_profile = vault_bridge profile_id
  valuation_unit = explicitly configured accounting unit
  redemption_account = vault_bridge_redemption_operator
```

For the MVP, the vault bridge keeps source-token atoms and PFTL issued-asset
atoms at par for the configured source domain:

```text
VAULT_BRIDGE_UNIT = 1_000_000
nav_per_unit = 1_000_000
```

That constant is a NAV reserve packet scale, not a token name or a requirement
that the PFTL asset represent USD. Source token decimals and the issued-asset
precision are deployment parameters.

### Proof profile

Use the existing `nav_profile_register` path.

For a controlled MVP with external source proofs:

```text
verifier_kind = "multi-fetch-quorum" or "sp1-groth16"
source_class = "vault_bridge:<source_domain>"
valuation_policy_hash = hash(policy document / source-domain config)
max_snapshot_age_blocks = nonzero
challenge_window_blocks = nonzero unless SP1-verifiable
max_epoch_gap_blocks = nonzero
settle_deadline_blocks = nonzero if redemption queue is enabled
min_attestations = nonzero for multi-fetch
tolerance_bp = 0 unless the profile explicitly tolerates observer variance
```

For an MVP that uses one source domain, the source domain is the deployed
source-chain vault plus configured source ERC-20 token. Operators may relay events, but the
receipt is not countable unless it carries bridge deposit evidence from the
vault:

```text
source_class = "vault_bridge:erc20_bridge_vault:<source_chain_id>:<vault_address>:<token_address>"
verifier_kind = "multi-fetch-quorum"
valuation_policy_hash = hash(canonical vault bridge asset source policy)
```

If SP1/light-client proof verification is ready for the source route, the same
profile can move to `sp1-groth16` without changing the vault bridge asset model.

### Launch trust boundary

The implementation must not count arbitrary operator custody receipts. The
minimum acceptable launch primitive is a deterministic bridge-deposit evidence
object derived from `ERC20BridgeVault.ERC20BridgeDeposited` logs plus a PFTL
proposal/challenge/finality record. A receipt is countable only when its
`evidence_root` equals the deterministic root of that evidence object and the
matching `VaultBridgeDepositRecord` is finalized.

That is not the same as a complete Ethereum light client. A public trustless
deposit claim still needs one of:

- SP1 or equivalent proof that the deposit log is included under a finalized
  source-chain receipt/header root.
- A PFTL-side optimistic challenge path that lets anyone freeze/reject a deposit
  evidence packet before it becomes countable. This path now exists for the
  launch bridge.
- A native light-client/receipt-proof verifier.

The current launch code is therefore bridge-evidence-bound with an optimistic
PFTL challenge window, but it is still not a fully trustless Ethereum receipt
inclusion verifier. Validators, challengers, or later SP1/light-client code must
verify that the proposed EVM log is actually included in the source-chain
canonical chain. For `sp1-groth16` vault bridge asset profiles, the deposit
record must also carry source-proof commitments: `source_proof_kind`,
`source_proof_hash`, and
`source_public_values_hash`. The public-values hash is deterministic over the
exact vault event evidence, evidence root, and policy hash. A deposit without
those commitments cannot be proposed or finalized under an SP1 profile. The
commitment is a replay binding, not by itself a verified Ethereum receipt
proof; the public trustless claim requires the source proof bytes/program to
verify source-chain receipt inclusion against finalized headers or an equivalent
light-client path.

Withdrawals are stricter: the source-chain vault holds the configured ERC-20 and will not
queue a withdrawal unless `PFTLWithdrawalVerifier` has accepted the exact
`withdrawalPacketDigest(packet)` and PFTL withdrawal-hash commitment. The
verifier requires threshold PFTL finality signatures and exposes a
challenge/finality window. The vault then applies its own challenge/finality
window, freezes challenged packets, and pays the user directly from the vault.

## Required new primitive: ReserveReceipt

`NavReservePacket` is an epoch-level aggregate. It is too coarse to prevent
double-use of individual source claims across vault bridge asset balances, NAVCoin
subscriptions, and redemption queues. Add receipt records as NAV-adjacent state,
not as a separate protocol.

Add to `crates/types/src/lib_parts/ledger_assets.rs`:

```text
VaultBridgeReceipt {
  receipt_id: String,               // 96 hex, deterministic id
  asset_id: String,                 // issued/NAV vault bridge asset id
  source_domain: String,            // erc20_bridge_vault:<chain>:<vault>:<token>
  source_asset: String,             // erc20:<chain>:<token>
  claim_type: String,               // bridge_deposit for launch
  amount_atoms: u64,
  source_tx_or_attestation: String,
  finality_ref: String,
  vault_id: String,
  policy_hash: String,              // profile policy hash: 96 hex legacy/profile or 64 hex SP1
  haircut_bps: u64,
  counted_value_atoms: u64,
  allocated_value_atoms: u64,
  bucket_id: String,                // 96 hex
  status: String,                   // pending/finalized/counted/paused/impaired/rejected/retired
  created_at_height: u64,
  finalized_at_height: u64,
  counted_at_height: u64,
  expires_at_height: u64,
  bridge_deposit_evidence: Option<VaultBridgeDepositEvidence>
}
```

For launch, `bridge_deposit_evidence` is mandatory and `claim_type` must be
`bridge_deposit` before a receipt can be counted. Other future claim types may be
introduced only with their own replayable verifier. They are not a substitute for
vault-backed vault bridge asset.

```text
VaultBridgeDepositEvidence {
  source_chain_id: u64,
  vault_address: String,             // 0x lower-case EVM address
  token_address: String,             // source ERC-20 address
  depositor: String,
  pftl_recipient: String,
  pftl_recipient_hash: String,       // event topic, 64 hex
  amount_atoms: u64,
  nonce: String,                     // bytes32, 64 hex
  deposit_id: String,                // vault event deposit_id, 64 hex
  block_hash: String,
  tx_hash: String,
  log_index: u64
}
```

```text
VaultBridgeDepositRecord {
  asset_id: String,
  evidence_root: String,             // H_vault_bridge_deposit_evidence
  evidence: VaultBridgeDepositEvidence,
  policy_hash: String,
  source_proof_kind: String,          // empty for multi-fetch; "sp1-groth16" for SP1
  source_proof_hash: String,          // 96 hex proof commitment for SP1 profiles
  source_public_values_hash: String,  // 96 hex deterministic public-values commitment
  proposer: String,
  status: String,                    // pending/challenged/finalized
  submitted_at_height: u64,
  finalized_at_height: u64,
  expires_at_height: u64,
  challenger: String,
  challenge_hash: String,
  challenge_bond: u64,
  attestations: Vec<VaultBridgeDepositAttestation>
}

VaultBridgeDepositAttestation {
  attestor: String,
  pass: bool,
  observation_root: String,
  attested_at_height: u64
}
```

Use `u64` initially because existing issued assets, trustlines, NAV supply, and
reserve packet quantities are `u64`. If vault bridge asset needs `u128`, that should be a
separate numeric migration for issued assets generally, not a vault bridge asset-only type.

Add to `LedgerState`:

```text
pub vault_bridge_receipts: Vec<VaultBridgeReceipt>
pub vault_bridge_deposits: Vec<VaultBridgeDepositRecord>
pub vault_bridge_bucket_states: Vec<VaultBridgeBucketState>
pub vault_bridge_allocations: Vec<VaultBridgeAllocation>
```

These vectors match the current ledger style. Any state-affecting scans must use
deterministic ordering and bounded validation. Do not use unordered maps in
consensus state.

### Receipt id

Receipt ids must be deterministic and domain separated:

```text
receipt_id = H_vault_bridge_receipt(
  chain_id,
  asset_id,
  source_domain,
  source_tx_or_attestation,
  finality_ref,
  amount_atoms,
  policy_hash
)
```

Use the repo's existing lower-hex hash conventions. Do not use serde/JSON as
the signed or hashed preimage.

### Bucket id

Bucket ids isolate risk:

```text
bucket_id = H_vault_bridge_bucket(
  asset_id,
  source_domain,
  policy_hash
)
```

The MVP should expose the bucket in RPC/status and may expose it in the display
asset name, e.g. `vault bridge asset.arbhl`.

## Required new primitive: bucket state

Add:

```text
VaultBridgeBucketState {
  asset_id: String,
  bucket_id: String,
  source_domain: String,
  policy_hash: String,
  gross_receipt_atoms: u64,
  counted_value_atoms: u64,
  outstanding_vault_bridge_atoms: u64,
  nav_subscription_allocations_atoms: u64,
  redemption_queue_atoms: u64,
  other_allocations_atoms: u64,
  impairment_factor_bps: u64,        // 10000 = unimpaired
  status: String,                    // active/paused/impaired/retired
  last_packet_epoch: u64,
  last_updated_height: u64,
}
```

Consensus must enforce:

```text
outstanding_vault_bridge_atoms
  + nav_subscription_allocations_atoms
  + redemption_queue_atoms
  + other_allocations_atoms
  <= counted_value_atoms
```

for every bucket after every vault bridge asset operation.

## Required new primitive: allocation ledger

Add:

```text
VaultBridgeAllocation {
  allocation_id: String,             // 96 hex
  receipt_id: String,
  asset_id: String,
  bucket_id: String,
  amount_atoms: u64,
  purpose: String,                   // vault_bridge_supply/nav_subscription/redemption/other
  consumer_id: String,
  created_at_height: u64,
  retired_at_height: u64,
}
```

This avoids double counting when the same counted receipt backs transferable
vault bridge asset, a NAVCoin subscription batch, or a redemption queue.

## Transaction model

The implementation should add the minimum new transaction kinds.

### 1. `vault_bridge_deposit_propose`

Creates a pending bridge-deposit evidence record from a replayed
`ERC20BridgeVault.ERC20BridgeDeposited` log. Any funded PFTL account may propose; the
proposer is a relay, not a custodian.

Fields:

```text
proposer
asset_id
evidence_root
evidence
policy_hash
source_proof_kind
source_proof_hash
source_public_values_hash
expires_at_height
```

Validation:

- `asset_id` must be registered as a NAV asset.
- The asset profile source class must match `evidence.source_domain()`.
- `policy_hash` must match the registered vault bridge asset source policy.
- `evidence_root == H_vault_bridge_deposit_evidence(evidence)`.
- For `multi-fetch-quorum` profiles, source-proof fields must be empty.
- For `sp1-groth16` profiles, source-proof fields are mandatory:
  `source_proof_kind = sp1-groth16`, `source_proof_hash` is a 96-hex proof
  commitment, and `source_public_values_hash` equals
  `H_vault_bridge_deposit_public_values(evidence_root, evidence, policy_hash)`.
- `expires_at_height` is greater than the current block height.
- Duplicate `(asset_id, evidence_root)` is rejected.
- Record status starts `pending`.

### 2. `vault_bridge_deposit_challenge`

Freezes a pending bridge-deposit evidence record before it can become counted.

Fields:

```text
challenger
asset_id
evidence_root
challenge_hash
bond
```

Validation:

- The referenced bridge-deposit record exists and is `pending`.
- The challenge is submitted before `submitted_at_height + challenge_window`.
- `bond >= profile.min_challenge_bond`.
- The challenger pays the bond if nonzero.
- Record status becomes `challenged`.
- Challenged records cannot be finalized or counted.

### 3. `vault_bridge_deposit_attest`

Records a registered observer's verdict on the proposed vault-event evidence.
For `multi-fetch-quorum` profiles, finalization requires enough distinct pass
attestations and no failing attestations.

Fields:

```text
attestor
asset_id
evidence_root
pass
observation_root
```

Validation:

- The referenced bridge-deposit record exists and is `pending`.
- The asset profile verifier is `multi-fetch-quorum`.
- `attestor` is registered via `nav_attestor_register`.
- Each attestor may attest once per `(asset_id, evidence_root)`.
- The attestation list is bounded by `MAX_NAV_ATTESTATIONS_PER_PACKET`.

### 4. `vault_bridge_deposit_finalize`

Finalizes an unchallenged bridge-deposit evidence record after the challenge
window closes.

Fields:

```text
finalizer
asset_id
evidence_root
```

Validation:

- The referenced bridge-deposit record exists and is `pending`.
- The asset profile source policy still accepts the record source domain and
  policy hash.
- For `multi-fetch-quorum`, there are no failing attestations and pass
  attestations are at least `profile.min_attestations`.
- For `sp1-groth16`, source-proof commitments are present and
  `source_public_values_hash` matches the deterministic public values for the
  exact evidence root, vault event evidence, and policy hash.
- The current height is at or after
  `submitted_at_height + challenge_window_blocks`.
- The current height is not past `expires_at_height` or the profile
  `max_snapshot_age_blocks`.
- Record status becomes `finalized`.

### 5. `vault_bridge_deposit_claim`

Permissionlessly turns finalized vault deposit evidence into user-held vault bridge asset.
The claimer pays the PFTL fee, but the minted vault bridge asset always goes to
the `pftl_recipient` committed by the source-chain deposit event.

Fields:

```text
claimer
asset_id
evidence_root
policy_hash
recipient
amount_atoms
```

Validation:

- The referenced bridge-deposit record exists and is `finalized`.
- `policy_hash` matches the finalized record and the vault bridge asset source profile.
- `recipient == VaultBridgeDepositEvidence.pftl_recipient`.
- `amount_atoms == VaultBridgeDepositEvidence.amount_atoms`.
- The evidence is not expired and is within profile freshness bounds.
- Recipient has a movable vault bridge asset trustline with enough limit.
- The claim creates or reuses the deterministic `VaultBridgeReceipt`, counts it 1:1
  with `haircut_bps = 0`, creates a `vault_bridge_supply` allocation, updates the
  source bucket, and mints exactly `amount_atoms` vault bridge asset to the recipient.
- Duplicate claims fail because the receipt capacity is already allocated.
- The resulting vault bridge asset balance is an ordinary issued-asset balance and can be
  transferred, escrowed, offered, or swapped through the existing PFTL rails.

### 6. `vault_bridge_receipt_submit`

Creates a pending or finalized source receipt.

Fields:

```text
operator
asset_id
source_domain
source_asset
claim_type
amount_atoms
source_tx_or_attestation
finality_ref
vault_id
policy_hash
expires_at_height
bridge_deposit_evidence
```

Validation:

- `asset_id` must be registered as a NAV asset.
- `operator` must be the asset issuer or `reserve_operator`.
- `policy_hash` must match the asset profile's `valuation_policy_hash`.
- `amount_atoms > 0`.
- `claim_type == bridge_deposit` for the launch path.
- `bridge_deposit_evidence` is present and internally valid.
- `source_domain == bridge_deposit_evidence.source_domain()`.
- `source_tx_or_attestation == erc20_bridge_deposit:<deposit_id>`.
- `finality_ref == evm_log:<chain_id>:<block_hash>:<tx_hash>:<log_index>`.
- `vault_id == evm:<chain_id>:<vault_address>:<token_address>`.
- `source_domain` must match the proof profile `source_class` for MVP, or be in
  the registered source policy for later multi-source profiles.
- Duplicate `receipt_id` is rejected.
- Status starts `pending` unless the source proof is consensus-verifiable at
  submit time.

### 7. `vault_bridge_receipt_count`

Moves a receipt from `pending` or `finalized` to `counted`.

Fields:

```text
operator
asset_id
receipt_id
haircut_bps
counted_value_atoms
evidence_root
policy_hash
```

Validation:

- Receipt exists and is not already counted/retired/rejected.
- Operator is issuer or reserve operator.
- `haircut_bps <= 10000`.
- `counted_value_atoms == floor(amount_atoms * (10000 - haircut_bps) / 10000)`.
- Source proof freshness is within the asset proof profile.
- `policy_hash` matches receipt and profile.
- `claim_type == bridge_deposit`.
- `evidence_root == H_vault_bridge_deposit_evidence(bridge_deposit_evidence)`.
- A matching `(asset_id, evidence_root)` `VaultBridgeDepositRecord` exists.
- The bridge-deposit record is `finalized`.
- The finalized record evidence and policy hash match the receipt.
- Bucket state is updated.
- Bucket invariant holds after update.

### 8. `vault_bridge_mint_from_receipts`

Mints the issued vault bridge asset to a trustline from counted, unallocated receipt
capacity.

Fields:

```text
issuer
to
asset_id
bucket_id
amount_atoms
receipt_ids[]
epoch
reserve_packet_hash
```

Validation:

- Use existing NAV liveness check against `epoch` and `reserve_packet_hash`.
- Asset issuer matches.
- Recipient has a trustline and line can move.
- Sum of available counted receipt capacity in `receipt_ids` covers
  `amount_atoms`.
- Allocation records are created with `purpose = vault_bridge_supply`.
- Trustline balance increases exactly like `nav_mint_at_nav`.
- Issued supply cap and trustline limit are enforced.
- Bucket invariant holds.

This is intentionally close to `nav_mint_at_nav`, but it should not infer mint
capacity from `NavTrackedAsset.circulating_supply` alone. It must allocate
receipt capacity.

### 9. `vault_bridge_burn_to_redeem`

Burns vault bridge asset from an owner and creates a source-specific redemption queue entry
plus a PFTL withdrawal packet for the source vault.

Fields:

```text
owner
issuer
asset_id
bucket_id
amount_atoms
epoch
reserve_packet_hash
destination_ref
```

Validation:

- Use existing NAV liveness check.
- Owner has sufficient vault bridge asset trustline balance.
- Bucket is active or impaired-but-redeemable.
- Burn decreases `outstanding_vault_bridge_atoms`.
- Redemption queue allocation increases for the same bucket.
- No automatic par guarantee is implied.
- Output redemption record references `bucket_id`.
- For an EVM ERC-20 source, `destination_ref` must be
  `evm-erc20:<evm_chain_id>:<0xrecipient>`.
- The redemption record includes `withdrawal_packet`,
  `withdrawal_packet_hash`, and `withdrawal_packet_evm_digest`, where the packet commits to:
  `pftl_chain_id`, `vault_bridge_asset_id`, `burn_tx_id`, `withdrawal_id`, `recipient`,
  `amount_atoms`, `source_bucket_id`, `destination_hash`, `finalized_height`, and
  `evidence_root`.
- `withdrawal_packet_hash` is the 48-byte PFTL-domain packet hash.
  `withdrawal_packet_evm_digest` is `keccak256(abi.encode(...))` over the exact
  `ERC20BridgeVault.WithdrawalPacket` fields and must equal
  `ERC20BridgeVault.withdrawalPacketDigest(packet)`.

`VaultBridgeRedemption` is the preferred record. It avoids reusing generic
spot-at-NAV redemption semantics for source-vault claims.

### 10. `vault_bridge_redeem_settle`

Records exceptional settlement metadata for a redemption. This is not the
canonical vault bridge asset cash-out path. The canonical path is direct user claim from
`ERC20BridgeVault` after a relayed PFTL withdrawal packet clears the challenge window.

Fields:

```text
issuer_or_redemption_account
asset_id
redemption_id
settlement_receipt_hash
settled_atoms
```

Validation:

- Caller is issuer or redemption account.
- Redemption exists and is pending.
- `settled_atoms <= redemption.amount_atoms`.
- Settled amount retires redemption queue allocation.
- If partial settlement is allowed, remaining amount stays pending.
- Settlement receipt hash is recorded.

If `NavRedeemSettleOperation` is reused, it needs enough fields to avoid hiding
partial settlement and bucket provenance.

### 11. `vault_bridge_bucket_impair`

Applies source-domain impairment without halting unrelated buckets.

Fields:

```text
operator
asset_id
bucket_id
updated_counted_value_atoms
impairment_factor_bps
reason_hash
policy_hash
```

Validation:

- Operator is issuer, reserve operator, or a policy-authorized emergency role.
- `impairment_factor_bps <= 10000`.
- Bucket moves to `impaired` or remains `paused`.
- New counted value does not exceed previous counted value unless paired with a
  new counted recapitalization receipt.
- Allocations are not silently moved to stronger buckets.
- Bucket invariant is checked with impaired counted value.

Default loss allocation:

```text
bucket_claim_atoms =
  outstanding_vault_bridge_atoms
  + nav_subscription_allocations_atoms
  + redemption_queue_atoms
  + other_allocations_atoms

bucket_factor_bps =
  min(10000, floor(recoverable_counted_atoms * 10000 / bucket_claim_atoms))
```

Claims in that bucket are redeemable pro rata:

```text
redeemable_atoms = floor(claim_atoms * bucket_factor_bps / 10000)
```

## Reserve packet integration

`NavReservePacket` should remain the epoch-level aggregate.

For vault bridge asset, the packet fields should mean:

```text
nav_per_unit = VAULT_BRIDGE_UNIT
circulating_supply = valid outstanding vault bridge asset atoms
verified_net_assets = aggregate counted_value_atoms across active buckets
source_root = root of source bucket summaries
attestor_root = existing attestor root or observation root
reserve_packet_hash = hash of canonical vault bridge asset reserve packet
```

The packet's `verified_net_assets` must equal the sum of active bucket counted
cash after impairment:

```text
verified_net_assets =
  sum(bucket.counted_value_atoms for active or impaired buckets)
```

The packet's `circulating_supply` must equal issued supply for the vault bridge asset
unless the implementation deliberately excludes escrowed/unsettled balances. If
exclusion is needed, add a generic "valid supply" packet concept rather than a
vault bridge asset-only shortcut.

## Source roots

The existing `source_root` field should commit to bucket summaries.

Canonical source summary leaf:

```text
VaultBridgeSourceBucketSummary {
  asset_id
  bucket_id
  source_domain
  policy_hash
  gross_receipt_atoms
  counted_value_atoms
  outstanding_vault_bridge_atoms
  nav_subscription_allocations_atoms
  redemption_queue_atoms
  impairment_factor_bps
  status
  last_updated_height
}
```

The MVP can hash sorted leaves directly before adding a Merkle tree. Sorting
must be lexicographic by `(asset_id, bucket_id)`.

## Haircut policy

Do not implement a hardcoded heuristic in transaction handlers.

The deterministic policy should live in a small pure module, likely:

```text
crates/execution/src/lib_parts/vault_bridge_policy.rs
```

Functions:

```text
compute_counted_value(amount_atoms: u64, haircut_bps: u64) -> Result<u64, VaultBridgePolicyError>
bucket_claim_atoms(bucket: &VaultBridgeBucketState) -> Result<u64, VaultBridgePolicyError>
bucket_factor_bps(recoverable_counted_atoms: u64, bucket_claim_atoms: u64) -> Result<u64, VaultBridgePolicyError>
redeemable_atoms(claim_atoms: u64, bucket_factor_bps: u64) -> Result<u64, VaultBridgePolicyError>
```

All arithmetic:

- checked integer math
- no floats
- no wall clock
- no randomness
- no unordered iteration
- issuance rounding goes down
- required backing rounding goes up where applicable

The first implementation should not attempt to compute the haircut from external
market data inside consensus. It should verify that the submitted haircut and
counted value match the registered `policy_hash` and replay evidence. More
automatic scoring can be added later.

## NAVCoin subscription integration

vault bridge asset should settle NAVCoin subscriptions by allocation, not by bespoke cash
logic.

When a NAVCoin primary subscription consumes vault bridge asset:

```text
vault bridge asset balance or counted receipt capacity
  -> allocation purpose = nav_subscription
  -> NAVCoin MintEscrow release checks counted value
```

The NAVCoin mint path should accept:

```text
asset_id = navcoin asset id
settlement_asset_id = vault bridge asset id
settlement_bucket_id
settlement_allocation_id
settlement_amount_atoms
```

The subscription release must check:

```text
settlement_allocation.purpose == nav_subscription
settlement_allocation.asset_id == settlement_asset_id
settlement allocation is not retired
vault bridge asset bucket invariant still holds
NAVCoin post-mint backing check passes
```

This can be added after vault bridge asset itself works. The vault bridge asset MVP only needs to produce
auditable counted value allocations.

## RPC and CLI

Add minimal operator/query surfaces:

```text
postfiat-node vault-bridge-status --asset-id <id>
postfiat-node vault-bridge-receipts --asset-id <id> [--bucket-id <id>]
postfiat-node vault-bridge-burn-to-redeem-bundle --owner <account> --asset-id <id> --amount-atoms <amount> --destination-ref evm-erc20:CHAIN_ID:<0xrecipient> --bundle <dir>
postfiat-node vault-bridge-withdrawal-plan --asset-id <id> --redemption-id <id>
postfiat-node vault-bridge-export-reserve-packet --asset-id <id> --epoch <n>
postfiat-node vault-bridge-replay-reserve-packet --bundle <dir>
```

RPC methods can mirror current NAV/market-ops style:

```text
vault_bridge_status
vault_bridge_receipts
vault_bridge_buckets
vault_bridge_replay_bundle
```

`vault-bridge-status` must include:

- `bridge_deposit_count`.
- `bridge_deposits[]` rows keyed by `evidence_root`.
- For each bridge deposit: source chain, vault, token, depositor, PFTL
  recipient, amount, deposit id, block hash, tx hash, log index, proposer,
  status, submitted/finalized/expiry heights, challenger, challenge hash, and
  challenge bond.
- Pass/fail attestation counts plus attestation rows containing attestor,
  verdict, observation root, and attested height.
- `redemptions[]` rows must expose the PFTL burn id, withdrawal recipient,
  withdrawal evidence root, 48-byte PFTL `withdrawal_packet_hash`, and 32-byte
  EVM `withdrawal_packet_evm_digest`.

Public status must avoid language implying guaranteed redemption, stable value,
instant exit, par support, or insurance.

## Operator runbook: bridge source ERC-20 into PFTL capacity

The launch path is vault-first. The configured source ERC-20 lives in `ERC20BridgeVault` on the source chain.
Deposits emit canonical events. PFTL counts vault bridge asset capacity only from replayed
vault deposit evidence. Operators can relay transactions and replay bundles, but
they do not custody user tokens, invent deposits, or choose withdrawal recipients.

### No token-specific bridge primitive

The vault bridge is a generic issued-asset primitive. There is no consensus
path named after a particular wrapped token, no source-chain contract that
hard-codes a chain/token pair, and no mandatory asset code. A launch profile is
just configuration:

```text
source_chain_id = <source chain id>
source_token = <source ERC20 token address>
source_contract = ERC20BridgeVault
asset_code = <operator-chosen PFTL asset code>
asset_precision = <source token decimals>
valuation_unit = <operator-chosen accounting unit>
```

The PFTL ledger sees an ordinary issued asset registered under a
`vault_bridge:<source_domain>` proof profile. Deposits, PFTL mint/count,
swapability, burns, withdrawal proof/challenge/finality, and direct user claims
use the same vault bridge operations for every source ERC-20. Operators may
choose a USD-denominated asset code for a deployment, but that name is not
compiled into the L1, the contracts, or the bridge state machine.

Initial source domain:

```text
source_domain = erc20_bridge_vault:CHAIN_ID:<vault_address>:<token_address>
source_asset = erc20:CHAIN_ID:<token_address>
claim_type = bridge_deposit
vault_id = evm:CHAIN_ID:<vault_address>:<token_address>
policy_hash = <registered vault bridge asset profile valuation_policy_hash>
```

End-to-end sequence:

1. Choose the PFTL issued-asset parameters and derive the vault bridge asset id:
   `issued_asset_id(pftl_chain_id, issuer, asset_code, asset_version)`. This
   value is independent of the source-chain vault address and must be supplied
   to `ERC20BridgeVault` at deployment. Operators should use the node helper to
   write the deploy-time env fragment:

   ```bash
   postfiat-node vault-bridge-asset-id \
     --pftl-chain-id "$PFTL_CHAIN_ID" \
     --issuer "$PFTL_ISSUER" \
     --asset-code "$VAULT_BRIDGE_ASSET_CODE" \
     --asset-version "$VAULT_BRIDGE_ASSET_VERSION" \
     --env-file vault-bridge-asset.env \
     --overwrite
   ```
2. Deploy `PFTLWithdrawalVerifier` with the controlled-launch PFTL signer set,
   threshold, verifier `challenge_delay`, and verifier `execution_window`.
3. Deploy `ERC20BridgeVault` with the configured source ERC-20 token, the
   withdrawal verifier, the PFTL numeric chain id, the 48-byte vault bridge
   asset id, vault `challenge_delay`, and vault `execution_window`.

The Foundry deployment harness is:

```bash
cd crates/ethereum-contracts
cp script/erc20-bridge.env.example .env
$EDITOR .env
set -a
. ./.env
. ./vault-bridge-asset.env
set +a
forge script script/DeployERC20Bridge.s.sol:DeployERC20Bridge \
  --rpc-url "$SOURCE_CHAIN_RPC_URL" \
  --broadcast
```

4. Register the PFTL issued asset and NAV proof profile using the deployed
   source-chain vault address. The bootstrap bundle writes deterministic
   operation JSON for:
   - `nav_profile_register` with `source_class = vault_bridge:<source_domain>`
   - `asset_create` for the configured bridge asset code/precision/version
   - `nav_asset_register` with the derived profile id
   - optional initial `trust_set` operations for known holders or market makers

```bash
postfiat-node vault-bridge-bootstrap-bundle \
  --pftl-chain-id <pftl chain id> \
  --source-chain-id <source chain id> \
  --vault-address <ERC20BridgeVault address> \
  --token-address <source ERC20 token address> \
  --issuer <pftl issuer account> \
  --asset-code <bridge asset code> \
  --asset-version <version> \
  --asset-precision <source token decimals> \
  --asset-display-name <display name> \
  --valuation-unit <accounting unit> \
  --valuation-policy-hash <policy hash> \
  --trust-accounts <optional comma-separated pftl accounts> \
  --bundle bootstrap-bundle
```

The bundle report exposes `asset_id`, `profile_id`, `source_domain`, and
`source_class`. The script uses the existing asset transaction
quote/sign/submit flow:

```bash
ISSUER_KEY_FILE=issuer.key \
TRUST_ACCOUNT_0_KEY_FILE=holder.key \
bash bootstrap-bundle/commands.sh
```

5. User prepares a source-chain deposit intent. The intent computes the
   `pftl_recipient_hash`, expected vault `deposit_id`, source-domain string,
   and cast-ready approve/deposit commands. The `--depositor` must be the
   eventual `msg.sender`; otherwise the vault event will have a different
   `deposit_id` and PFTL will not count it against this intent.

```bash
postfiat-node vault-bridge-deposit-intent \
  --source-chain-id <source chain id> \
  --vault-address <ERC20BridgeVault address> \
  --token-address <source ERC20 token address> \
  --depositor <source-chain sender> \
  --amount-atoms <source token atoms> \
  --pftl-recipient <pftl account> \
  --nonce <32-byte nonce> \
  --asset-id <vault bridge asset id> \
  --policy-hash <policy hash> \
  --proposer <pftl relayer account> \
  --expires-at-height 100000 \
  --bundle deposit-relay-bundle
```

6. User approves the configured source ERC-20 token and calls:

```text
ERC20BridgeVault.deposit(amount, pftlRecipient, nonce)
```

7. After the source-chain deposit transaction is mined, relay from the EVM RPC,
   not from hand-edited event JSON. The RPC relay command runs
   `cast receipt --json`, persists `source-receipt.json`, verifies the
   transaction hash, success status, vault address, token address, event topic,
   ABI payload, recipient hash, and deposit-id preimage, then writes the PFTL
   propose/attest/finalize/claim operation bundle:

```bash
postfiat-node vault-bridge-deposit-relay-rpc-bundle \
  --source-rpc-url "$SOURCE_CHAIN_RPC_URL" \
  --tx-hash "$DEPOSIT_TX_HASH" \
  --vault-address "$ERC20_BRIDGE_VAULT" \
  --token-address "$ERC20_BRIDGE_TOKEN" \
  --asset-id "$VAULT_BRIDGE_ASSET_ID" \
  --policy-hash "$VAULT_BRIDGE_POLICY_HASH" \
  --proposer "$PFTL_RELAYER" \
  --attestor "$PFTL_ATTESTOR" \
  --expires-at-height "$PFTL_DEPOSIT_EXPIRES_AT_HEIGHT" \
  --bundle deposit-relay-bundle
```

The older file-based `vault-bridge-deposit-relay-bundle --receipt-file` path is
still useful for replaying archived receipts and tests. The operator launch path
should use the RPC form so the relay artifact records exactly which source
transaction was fetched.

For archived replay or tests, the lower-level file commands remain available:
`vault-bridge-deposit-plan` and `vault-bridge-deposit-relay-bundle` accept
either `--receipt-file deposit-receipt.json --vault-address ... --token-address
...` or `--log-file deposit-log.json`.

The resulting `propose_operation`, `attest_operation`, `finalize_operation`,
and `claim_operation` are signed with the existing
`wallet-sign-asset-transaction` flow and submitted through the normal mempool
or ordered-batch path. The planner validates the vault event payload; it is not
an Ethereum receipt-inclusion proof by itself. For a trustless public claim,
the proposal must also carry the configured `sp1-groth16` source proof fields
or another accepted light-client/challenge-finalized proof path.

```json
{
  "operation": "vault_bridge_deposit_propose",
  "proposer": "<pftl relayer account>",
  "asset_id": "<vault bridge asset id>",
  "evidence_root": "<96 hex chars>",
  "policy_hash": "<policy hash>",
  "source_proof_kind": "",
  "source_proof_hash": "",
  "source_public_values_hash": "",
  "expires_at_height": 100000,
  "evidence": {
    "source_chain_id": CHAIN_ID,
    "vault_address": "<vault>",
    "token_address": "<token>",
    "depositor": "<0xdepositor>",
    "pftl_recipient": "<pftl account>",
    "pftl_recipient_hash": "<64 hex>",
    "amount_atoms": 10000000,
    "nonce": "<64 hex>",
    "deposit_id": "<64 hex>",
    "block_hash": "<64 hex>",
    "tx_hash": "<64 hex>",
    "log_index": 0
  }
}
```

For `sp1-groth16` profiles, the same proposal must include
`source_proof_kind = "sp1-groth16"`, a 96-hex `source_proof_hash`, and
`source_public_values_hash =
H_vault_bridge_deposit_public_values(evidence_root, evidence, policy_hash)`.
For `multi-fetch-quorum`, those fields are empty and the profile relies on
registered observer attestations plus the challenge window.

6. Registered observers verify the source-chain vault event and attest:

```json
{
  "operation": "vault_bridge_deposit_attest",
  "attestor": "<registered observer>",
  "asset_id": "<vault bridge asset id>",
  "evidence_root": "<96 hex chars>",
  "pass": true,
  "observation_root": "<96 hex chars>"
}
```

For `multi-fetch-quorum`, at least `min_attestations` distinct pass attestations
are required and any failing attestation blocks finalization.

7. After the profile challenge window closes, finalize the unchallenged and
   quorum-attested deposit evidence:

```json
{
  "operation": "vault_bridge_deposit_finalize",
  "finalizer": "<any pftl account>",
  "asset_id": "<vault bridge asset id>",
  "evidence_root": "<96 hex chars>"
}
```

If the evidence is wrong, any challenger submits `vault_bridge_deposit_challenge`
before the window closes. A challenged deposit cannot be finalized or counted.

8. Claim the finalized deposit into the recipient's vault bridge asset trustline. This step
   is permissionless: a relayer may submit it, but the destination and amount
   are fixed by the vault event evidence.

```json
{
  "operation": "vault_bridge_deposit_claim",
  "claimer": "<any pftl account>",
  "asset_id": "<vault bridge asset id>",
  "evidence_root": "<96 hex chars>",
  "policy_hash": "<policy hash>",
  "recipient": "<pftl account from the deposit event>",
  "amount_atoms": 10000000
}
```

The claim creates or reuses the deterministic receipt, counts it 1:1, allocates
the receipt as `vault_bridge_supply`, updates the source bucket, and mints ordinary
issued vault bridge asset to the committed recipient. From here the holder can use
`issued_payment`, escrow, or the offer book to swap the vault bridge asset balance.

9. Advanced/internal path: the issuer or reserve operator may still submit and
   count receipts separately when capacity is being routed into non-holder uses
   such as NAVCoin primary subscriptions:

```json
{
  "operation": "vault_bridge_receipt_submit",
  "operator": "<vault_bridge issuer or reserve operator>",
  "asset_id": "<vault bridge asset id>",
  "source_domain": "erc20_bridge_vault:CHAIN_ID:<vault>:<token>",
  "source_asset": "erc20:CHAIN_ID:<token>",
  "claim_type": "bridge_deposit",
  "amount_atoms": 10000000,
  "source_tx_or_attestation": "erc20_bridge_deposit:<deposit_id>",
  "finality_ref": "evm_log:CHAIN_ID:<block_hash>:<tx_hash>:<log_index>",
  "vault_id": "evm:CHAIN_ID:<vault>:<token>",
  "policy_hash": "<policy hash>",
  "expires_at_height": 100000,
  "bridge_deposit_evidence": "<same VaultBridgeDepositEvidence object>"
}
```

10. The old `vault_bridge_mint_from_receipts` path is not the normal user bridge
    product path. It remains available for operator-administered capacity
    routing from counted receipts:

```json
{
  "operation": "vault_bridge_mint_from_receipts",
  "issuer": "<vault_bridge issuer>",
  "to": "<holder address>",
  "asset_id": "<vault bridge asset id>",
  "bucket_id": "<bucket id>",
  "amount_atoms": 5000000,
  "receipt_ids": ["<receipt id>"],
  "epoch": 1,
  "reserve_packet_hash": "<finalized vault_bridge reserve packet hash>"
}
```

11. Or allocate counted receipt capacity directly into a NAVCoin primary
   subscription:

```json
{
  "operation": "vault_bridge_nav_subscription_allocate",
  "operator": "<vault_bridge issuer or reserve operator>",
  "nav_asset_id": "<navcoin asset id>",
  "settlement_asset_id": "<vault bridge asset id>",
  "settlement_bucket_id": "<bucket id>",
  "settlement_receipt_id": "<receipt id>",
  "settlement_amount_atoms": 5000000
}
```

12. Mint the NAVCoin against the retired-on-use vault bridge asset allocation:

```json
{
  "operation": "nav_mint_at_nav",
  "issuer": "<navcoin issuer>",
  "to": "<subscriber address>",
  "asset_id": "<navcoin asset id>",
  "amount": 5,
  "epoch": 1,
  "reserve_packet_hash": "<finalized navcoin reserve packet hash>",
  "settlement_asset_id": "<vault bridge asset id>",
  "settlement_bucket_id": "<bucket id>",
  "settlement_allocation_id": "<allocation id>",
  "settlement_amount_atoms": 5000000
}
```

13. To withdraw the source ERC-20 later, burn vault bridge asset to a source-vault recipient:

```bash
postfiat-node vault-bridge-burn-to-redeem-bundle \
  --owner <holder account> \
  --asset-id <vault bridge asset id> \
  --amount-atoms 1000000 \
  --destination-ref evm-erc20:CHAIN_ID:<0xrecipient> \
  --bundle burn-to-redeem-bundle

OWNER_KEY_FILE=<holder key> bash burn-to-redeem-bundle/commands.sh
```

The bundle reads finalized PFTL ledger state and, when unambiguous, fills in
the issuer, bucket id, epoch, and finalized reserve packet hash. If multiple
eligible source buckets can satisfy the burn, operators must pass `--bucket-id`
explicitly so the source release path is deterministic.

```json
{
  "operation": "vault_bridge_burn_to_redeem",
  "owner": "<holder address>",
  "issuer": "<vault_bridge issuer>",
  "asset_id": "<vault bridge asset id>",
  "bucket_id": "<bucket id>",
  "amount_atoms": 1000000,
  "epoch": 1,
  "reserve_packet_hash": "<finalized vault_bridge reserve packet hash>",
  "destination_ref": "evm-erc20:CHAIN_ID:<0xrecipient>"
}
```

The finalized redemption record exposes a `withdrawal_packet`, a 48-byte
`withdrawal_packet_hash`, and a 32-byte `withdrawal_packet_evm_digest`. A
relayer first runs `postfiat-node vault-bridge-withdrawal-signature-bundle` to
derive the exact packet tuple, PFTL hash commitment, verifier pending proof id,
and deployment-specific digest to sign. Finality signers sign that raw 32-byte
digest; the generated `signatures.json` must contain threshold 65-byte
signatures sorted by recovered signer address. The generated relay-bundle stage
then writes a staged source-chain relay script. The first source-chain stage
submits the EVM digest, PFTL withdrawal-hash commitment, finalized PFTL height,
and threshold signatures to
`PFTLWithdrawalVerifier.submitProof`.

```text
PFTLWithdrawalVerifier.submitProof(
  withdrawal_packet_evm_digest,
  keccak256(withdrawal_packet_hash_bytes),
  pftl_finalized_height,
  threshold_signatures
)
```

Anyone can challenge that proof. After the verifier finalizes it as accepted,
a relayer submits the packet and 48-byte PFTL hash to
`ERC20BridgeVault.submitWithdrawal`; the vault recomputes
`withdrawalPacketDigest(packet)`, requires verifier acceptance for that exact
digest/hash pair, and starts the vault challenge window. If the vault packet is
unchallenged after `challenge_delay`, `finalizeWithdrawal` accepts it and the
user calls `claimWithdrawal` to receive the source ERC-20 directly from the vault.

The withdrawal relay bundle writes `plan.json` plus a `commands.sh` with
explicit stages, so operators do not accidentally run through challenge windows:

```bash
postfiat-node vault-bridge-withdrawal-signature-bundle \
  --asset-id <vault bridge asset id> \
  --redemption-id <redemption id> \
  --evm-chain-id CHAIN_ID \
  --verifier-address <PFTLWithdrawalVerifier> \
  --bundle withdrawal-signature-bundle

RUN_STAGE=sign \
PFTL_WITHDRAWAL_SIGNER_PRIVATE_KEY=0x... \
bash withdrawal-signature-bundle/commands.sh

# after signatures.json is populated with threshold signatures:
RUN_STAGE=relay-bundle bash withdrawal-signature-bundle/commands.sh

RUN_STAGE=submit-proof bash withdrawal-signature-bundle/relay-bundle/commands.sh
# wait for the verifier challenge window
RUN_STAGE=finalize-proof bash withdrawal-signature-bundle/relay-bundle/commands.sh
RUN_STAGE=submit-withdrawal bash withdrawal-signature-bundle/relay-bundle/commands.sh
# wait for the vault challenge window
RUN_STAGE=finalize-withdrawal bash withdrawal-signature-bundle/relay-bundle/commands.sh
RUN_STAGE=claim bash withdrawal-signature-bundle/relay-bundle/commands.sh
```

The lower-level relay bundle remains available when signatures are already
assembled:

```bash
postfiat-node vault-bridge-withdrawal-relay-bundle \
  --asset-id <vault bridge asset id> \
  --redemption-id <redemption id> \
  --evm-chain-id CHAIN_ID \
  --verifier-address <PFTLWithdrawalVerifier> \
  --signatures-file signatures.json \
  --bundle withdrawal-relay-bundle

RUN_STAGE=submit-proof bash withdrawal-relay-bundle/commands.sh
# wait for the verifier challenge window
RUN_STAGE=finalize-proof bash withdrawal-relay-bundle/commands.sh
RUN_STAGE=submit-withdrawal bash withdrawal-relay-bundle/commands.sh
# wait for the vault challenge window
RUN_STAGE=finalize-withdrawal bash withdrawal-relay-bundle/commands.sh
RUN_STAGE=claim bash withdrawal-relay-bundle/commands.sh
```

For live operation, quote each operation with:

```text
postfiat-node asset-fee-quote --source <signer> --operation-json '<json>' > quote.json
```

Then sign with the operator key and submit:

```text
postfiat-node wallet-sign-asset-transaction --key-file <operator key> --quote-file quote.json > signed.json
postfiat-node mempool-submit-signed-asset-transaction --signed-asset-transaction-json "$(cat signed.json)"
```

Verification:

```text
postfiat-node vault-bridge-status --asset-id <vault bridge asset id>
postfiat-node vault-bridge-receipts --asset-id <vault bridge asset id> --bucket-id <bucket id>
postfiat-node vault-bridge-burn-to-redeem-bundle --owner <holder account> --asset-id <vault bridge asset id> --amount-atoms <amount> --destination-ref evm-erc20:CHAIN_ID:<0xrecipient> --bundle <dir>
postfiat-node vault-bridge-withdrawal-plan --asset-id <vault bridge asset id> --redemption-id <redemption id> --evm-chain-id CHAIN_ID --verifier-address <PFTLWithdrawalVerifier>
postfiat-node vault-bridge-withdrawal-signature-bundle --asset-id <vault bridge asset id> --redemption-id <redemption id> --evm-chain-id CHAIN_ID --verifier-address <PFTLWithdrawalVerifier> --bundle <dir>
postfiat-node vault-bridge-export-reserve-packet --asset-id <vault bridge asset id> --epoch <n> --bundle <dir>
postfiat-node vault-bridge-replay-reserve-packet --bundle <dir>
```

Expected public state after a NAV subscription allocation:

```text
bucket.nav_subscription_allocations_atoms increases by settlement_amount_atoms
receipt.allocated_value_atoms increases by settlement_amount_atoms
allocation.purpose = nav_subscription
allocation.consumer_id = nav_subscription:<navcoin asset id>
nav_mint_at_nav retires allocation.retired_at_height on successful mint
reusing the same allocation is rejected
```

## MVP build order

### Task 1: Types

- Add `VaultBridgeReceipt`, `VaultBridgeBucketState`, `VaultBridgeAllocation`.
- Add deterministic id/hash helpers.
- Add fields to `LedgerState`.
- Add validate methods and serialization tests.

Done check:

```text
cargo test -p postfiat-types vault_bridge
```

### Task 2: Policy math

- Add `vault_bridge_policy.rs`.
- Implement counted value, bucket claim, impairment factor, redeemable amount.
- Add nominal example from `content/blog/erc20_bridge.md`.

Nominal example:

```text
amount_atoms = 100000000000
haircut_bps = 25
counted_value_atoms = 99750000000
minted = 60000000000
nav_subscription_allocation = 50000000000
remaining_outstanding_vault_bridge = 10000000000
unallocated_counted_capacity = 39750000000
```

Done check:

```text
cargo test -p postfiat-execution vault_bridge_policy
```

### Task 3: Receipt submit/count execution

- Add transaction kinds:
  - `vault_bridge_deposit_propose`
  - `vault_bridge_deposit_challenge`
  - `vault_bridge_deposit_attest`
  - `vault_bridge_deposit_finalize`
  - `vault_bridge_deposit_claim`
  - `vault_bridge_receipt_submit`
  - `vault_bridge_receipt_count`
- Add operations to `AssetTransactionOperation`.
- Add execution handlers beside existing NAV handlers.
- Enforce proof profile/source policy linkage.

Done check:

```text
cargo test -p postfiat-execution vault_bridge_receipt
```

### Task 4: Mint from receipts

- Add `vault_bridge_mint_from_receipts`.
- Reuse issued-asset trustline mint mechanics from `nav_mint_at_nav`.
- Require allocation of counted receipt capacity.
- Enforce bucket invariant.

Done check:

```text
cargo test -p postfiat-execution vault_bridge_mint
```

### Task 5: Source-specific redemption

- Add `vault_bridge_burn_to_redeem`.
- Add `vault_bridge_redeem_settle` or carefully extend `NavRedeemSettleOperation`.
- Preserve bucket provenance.
- Do not imply par redemption.

Done check:

```text
cargo test -p postfiat-execution vault_bridge_redeem
```

### Task 6: Impairment path

- Add `vault_bridge_bucket_impair`.
- Add source-domain pause/impair status transitions.
- Implement pro-rata claim math.
- Ensure strong buckets cannot be drained to cover impaired buckets.

Done check:

```text
cargo test -p postfiat-execution vault_bridge_impairment
```

### Task 7: Reserve packet/replay

- Commit bucket summaries through `NavReservePacket.source_root`.
- Add export/replay CLI.
- Verify packet aggregate equals bucket state and issued supply.

Done check:

```text
cargo test -p postfiat-node vault_bridge_replay
```

### Task 8: NAVCoin subscription hook-up

- Allow NAVCoin primary subscription logic to consume vault bridge asset allocations.
- Check `nav_subscription` allocation purpose.
- Keep NAVCoin market-ops envelope unchanged unless it needs to display
  settlement asset provenance.

Done check:

```text
cargo test -p postfiat-execution nav_subscription_vault_bridge
```

## Test plan

Required tests:

1. Receipt id changes when any source field changes.
2. Duplicate receipt is rejected.
3. Pending receipt cannot mint vault bridge asset.
4. Counted value matches floor BPS math.
5. Bridge-deposit claim mints vault bridge asset to the committed recipient and the balance
   can be posted into an offer.
6. Mint from counted receipts updates trustline and allocation ledger.
7. Mint cannot exceed unallocated counted capacity.
8. Bucket invariant holds after mint, burn, redeem, settle, and impairment.
9. Source A cannot redeem against source B unless explicit pooling is enabled.
10. Impairment applies pro-rata only within affected bucket.
11. NAV reserve packet verified_net_assets equals bucket counted sum.
12. NAV reserve packet circulating_supply equals issued vault bridge asset supply.
13. Stale source proof cannot be counted.
14. Unknown policy hash is rejected.
15. Unknown source domain is rejected for MVP.
16. Replay bundle recomputes the same source_root and reserve_packet_hash.

## Security notes

- This is consensus state. Keep all state-changing calculations deterministic.
- Do not iterate unordered maps in hash/signing/state roots.
- Use existing text validation and lower-hex length validation.
- Bound receipt lists in `vault_bridge_mint_from_receipts`.
- Bound bucket/source strings.
- Reject arithmetic overflow.
- Do not allow governance or operator actions to mint around allocation checks.
- Do not silently move impaired claims into stronger buckets.
- Do not call external APIs from execution.

## Open questions before implementation

1. Should `NavRedemption` be extended with `bucket_id`, or should vault bridge asset use a
   separate `VaultBridgeRedemption` to avoid spot-at-NAV semantics?
2. Should vault bridge asset source labels be represented as separate asset IDs
   (`vault bridge asset.arbhl`) or one asset ID with mandatory bucket metadata?
3. Should `NavReservePacket.circulating_supply` include queued redemptions after
   burn, or should queued redemptions be represented only as bucket claims?
4. What is the second source domain after the source-chain ERC-20
   `ERC20BridgeVault`: CCTP, Hyperliquid route, or another vault-backed source?
5. Do we want SP1 proof verification for source receipts in the first build, or
   launch with multi-fetch quorum plus replay bundles while keeping the same
   evidence-root interface for SP1 later?

## Recommended MVP answer

For the first implementation:

```text
one vault bridge asset_id
one initial source_domain: source-chain ERC-20 held in ERC20BridgeVault
one bucket_id for that source_domain
multi-fetch-quorum profile with PFTL proposal/challenge/finality
threshold PFTL withdrawal verifier before vault submission
source-specific withdrawal only through the source vault
no pooled redemption
no spot-at-NAV promise
no Arc dependency
custom source vault contract; no operator custody route
```

That fits the current NAVCoin architecture and minimizes custom tooling while
still adding the one missing primitive the NAVCoin stack does not currently
have: source-labeled receipt allocation.
