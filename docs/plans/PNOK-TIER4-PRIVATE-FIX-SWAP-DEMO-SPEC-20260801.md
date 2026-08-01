# pNOK Tier-4 Mapping and Private pfUSDC/pNOK Fix Demo Specification

**Date:** 2026-08-01  
**Priority:** P0 demo path; P1 full Tier-4 parity  
**Status:** implementation specification; not yet implemented  
**PFTL repository:** `a666-eth-fast-lane-combined-20260724`  
**Norges Bank source repository:** `Norges-Bank-CBDC-Lab/cbdc-tokenization-sandbox`  
**Primary historical precedent:** `PFUSDC-TIER4-IMPLEMENTATION-PLAN-20260717.md`  
**Current pfUSDC precedent:** `PFUSDC-MAINNET-CAMPAIGN-HANDOFF-20260726.md`  
**Private execution precedent:** `proper-private-nav-swap-plan.md` and the
Asset-Orchard implementation under `crates/privacy_orchard`

`MUST`, `MUST NOT`, `SHOULD`, and `REQUIRED` are normative.

## 1. Executive summary

The objective is a working demonstration in which:

1. canonical sandbox WNOK is deposited into a governed vault on the Norges
   Bank Besu network;
2. the corresponding amount of `pNOK` is issued on PFTL;
3. a liquidity provider shields pNOK into Asset-Orchard;
4. Bob shields existing PFTL pfUSDC;
5. Bob and the liquidity provider atomically exchange private pfUSDC and pNOK
   at one public, governed FX fix;
6. Bob receives a private pNOK note and the liquidity provider receives a
   private pfUSDC note; and
7. an optional proof-of-backing leg exits whole pNOK units and releases the
   corresponding WNOK from the Besu vault.

The fix is public. The traded amounts, note owners, recipients, and note
openings are private inside Asset-Orchard. The source-chain WNOK deposit and
any WNOK withdrawal are public boundaries.

The demo is not a foreign-exchange order book and it does not discover a
price. It consumes pNOK liquidity posted in advance and clears at a single
pre-published price without an AMM curve or price slippage.

The fastest honest demo uses the existing Asset-Orchard price-bound swap and
a `CONTROLLED` WNOK bridge profile because the inspected Norges Bank sandbox
has a controlled validator topology. That proves the product flow and the
atomic private exchange, but it is not Tier 4. Full Tier-4 pNOK requires
proof-verified Besu finality on PFTL and proof-verified PFTL finality on Besu,
with no bridge signer or operator fallback. The two gates are deliberately
separate in this specification.

## 2. Product statement

### 2.1 User story

> I hold pfUSDC on PFTL. A pNOK facility has posted private NOK liquidity and
> a current USD/NOK fix. I approve an exact trade. My pfUSDC and the facility's
> pNOK change owners atomically. I receive pNOK privately at the displayed fix,
> without walking an AMM curve and without trusting the coordinator to settle
> only one leg.

### 2.2 Economic model

`pNOK` is a PFTL representation of WNOK already locked in the governed source
vault. The private FX trade does not mint pNOK. It transfers existing pNOK from
the facility to Bob and existing pfUSDC from Bob to the facility.

For the first demo:

```text
WNOK locked in Besu vault
  -> equal pNOK issued on PFTL
  -> pNOK shielded by the facility

Bob's pfUSDC note + facility pNOK note
  -> one Asset-Orchard atomic swap at the governed fix
  -> Bob pNOK note + facility pfUSDC note
```

There is no fractional reserve, synthetic pNOK issuance, operator IOU, or
unsecured credit leg.

### 2.3 Naming

The canonical product symbol in this specification is `pNOK`.

- `WNOK` means the allowlisted ERC-20 cash token in the Norges Bank sandbox.
- `pNOK` means the WNOK-backed issued asset on PFTL.
- `pfUSDC` means the existing Ethereum-USDC-backed PFTL settlement asset.
- `pfNOK` MUST NOT be used as a second alias after the pNOK asset definition is
  activated. A single symbol and a single asset identifier avoid route and UX
  ambiguity.

The demo and its documentation MUST describe pNOK as **sandbox WNOK-backed
pNOK**. They MUST NOT imply endorsement by Norges Bank or describe pNOK as an
official Norges Bank CBDC deployment without separate authorization.

## 3. Scope

### 3.1 Required for the first end-to-end demo

- a version-pinned WNOK source contract and Besu chain profile;
- an allowlisted WNOK bridge vault;
- exact WNOK deposit evidence bound to a PFTL recipient;
- a registered pNOK issued-asset definition and bridge route on PFTL;
- replay-safe pNOK issuance and vault accounting;
- a controlled pNOK-to-WNOK redemption path for round-trip evidence;
- pNOK support in Asset-Orchard ingress, note scanning, swap, and egress;
- a consensus-registered public FX fix packet;
- an exact pfUSDC/pNOK Asset-Orchard pricing policy derived from that packet;
- a pre-funded private pNOK liquidity note;
- one atomic private pfUSDC-to-pNOK trade;
- wallet or demo-runner status, receipts, balance evidence, and privacy labels;
- a fail-closed repeat/replay test; and
- a machine-readable evidence bundle.

### 3.2 Explicitly not required for the first demo

- an FX order book, batch auction, or blind matcher;
- price discovery or an oracle aggregation network;
- two-way public market making at arbitrary size;
- a public permissionless WNOK bridge;
- production custody, production KYC, or legal classification;
- multi-asset reserve facilities;
- pNOK on Ethereum or a pNOK Uniswap pool;
- hiding the public fix or the stable Asset-Orchard asset tags;
- hiding source-chain deposit or withdrawal events;
- changing WNOK monetary policy or granting PFTL authority to mint WNOK; or
- claiming Tier 4 while either bridge direction depends on a controlled
  checkpoint or operator assertion.

### 3.3 Production follow-on

Full Tier-4 parity, non-custodial two-party authorization, source-chain
validator hardening, public unattended UX, and scale qualification follow the
demo. They are specified below so the demo artifacts have a clean upgrade
path and do not become an incompatible one-off.

## 4. Ground truth: the original pfUSDC Tier-4 architecture

### 4.1 What Tier 4 meant

The historical pfUSDC Tier-4 design required cryptographic finality proofs in
both bridge directions:

1. PFTL independently verifies the exact canonical source-vault USDC deposit
   and source-chain finality before issuing pfUSDC.
2. The source vault independently verifies that the exact pfUSDC burn and
   withdrawal packet was accepted in a finalized PFTL block before releasing
   USDC.
3. Relayers transport proofs but cannot make false evidence valid.
4. A user can construct and relay proofs without a bridge signer committee.
5. Deposit identifiers, withdrawal identifiers, and proof nullifiers are
   replay protected.
6. Route epochs pin chain identity, genesis, contracts, runtime code hashes,
   proof program verification keys, limits, and activation heights.
7. An activated Tier-4 route cannot silently fall back to observer or
   threshold-signature verification.

Its bridge conservation identity was:

```text
V = S + D + B - R
```

where the canonical vault balance and every issued, pending, burned, and
released term are accounted without double counting.

### 4.2 Historical Arbitrum ingress

The original ingress path was:

```text
Arbitrum USDC deposit
  -> exact vault balance delta and recipient-bound output
  -> output included in a Nitro assertion sendRoot
  -> assertion confirmed under finalized Ethereum
  -> SP1 Groth16 proof
  -> PFTL verifies proof, route, recipient, amount, and replay key
  -> pfUSDC issued
```

This was technically coherent but commercially rejected. The trustless Nitro
confirmation path took approximately 6.4 days, which could not satisfy the
required 25-minute product journey.

### 4.3 Historical Tier-4 egress

The original egress path was:

```text
accepted pfUSDC burn on PFTL
  -> BridgeExitLeafV1 in an ordered exit root
  -> finalized consensus-v2 commit, accepted receipt, and Merkle path
  -> SP1 Groth16 proof
  -> stateful PFTL finality verifier on the source chain
  -> exact source-vault release
```

The exit leaf is created only for an accepted burn. Rejected transactions
create no leaf. The voted PFTL block commits the exit root before validators
vote, and the source verifier follows PFTL committee changes from a pinned
checkpoint.

### 4.4 Direct Ethereum pfUSDC successor

The shipping pfUSDC route replaced Arbitrum with canonical Ethereum-mainnet
USDC:

```text
Ethereum USDC approval and vault deposit
  -> finalized Ethereum evidence
  -> SP1 Groth16 ingress proof
  -> PFTL proposal, finalization, and claim
  -> spendable pfUSDC
```

The campaign proved exact conservation and replay rejection. The first full
25-USDC round trip was functionally correct but took 2h45m48s. A replacement
1-USDC run completed deposit inclusion through withdrawal inclusion in
20m12s, inside the 25-minute target.

The current status documentation qualifies one important trust-boundary
detail: the current Ethereum ingress is accepted under a disclosed PFTL BFT
checkpoint and registered route rather than an Ethereum light client living
inside PFTL. The current pfUSDC egress uses the proof-native Epoch-5 verifier
and vault. Therefore current pfUSDC must not be described as having identical
Tier-4 trust in both directions merely because the original Tier-4 plan exists.

### 4.5 Reusable implementation surface

The following pfUSDC components are architectural precedents and SHOULD be
generalized rather than copied into a parallel bridge:

| pfUSDC component | pNOK application |
|---|---|
| `VaultBridgeDepositEvidence` | exact WNOK deposit evidence |
| governed route binding and route epoch | Besu/WNOK/pNOK chain and contract binding |
| deterministic deposit ID | WNOK deposit replay key |
| bucket, claim, burn, and release accounting | pNOK vault and supply accounting |
| propose/attest/finalize/claim lifecycle | controlled demo ingress; proof lifecycle later |
| `BridgeExitLeafV1` discipline | accepted pNOK burn authorizes WNOK release |
| consensus-v2 finalized commit artifacts | PFTL finality input to Besu verifier |
| SP1 Groth16 verification plumbing | Besu ingress and PFTL egress guests |
| route activation and no-fallback rules | pNOK controlled-to-Tier-4 migration |
| evidence and conservation harnesses | pNOK round-trip qualification |

Primary code anchors include:

- `crates/types/src/pfusdc_tier4_types.rs`;
- `crates/types/src/account_owned_asset_types.rs`;
- `crates/execution/src/nav_vault_asset_execution.rs`;
- `crates/execution/src/nav_sp1_verifier.rs`;
- `crates/node/src/pfusdc_tier4.rs`;
- `crates/node/src/vault_bridge_workflows.rs`;
- `programs/pfusdc-egress`;
- `programs/pfusdc-eth-mainnet-ingress`;
- `crates/ethereum-contracts/src/ERC20BridgeVaultV2.sol`; and
- `crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol`.

The implementation MUST first decide which types can become asset-neutral
without changing historical pfUSDC encodings. Existing activated schemas MUST
NOT be reinterpreted. New generic encodings require new schema versions.

## 5. Ground truth: the WNOK source system

The inspected Norges Bank sandbox models WNOK as an allowlisted ERC-20 cash
token on Besu.

Relevant current properties are:

- `Wnok.sol` grants administrator-controlled mint and burn roles;
- mint, burn, transfer, and transfer-from paths enforce allowlist rules;
- `transferFrom` requires both `TRANSFER_FROM_ROLE` and normal ERC-20
  allowance;
- WNOK has zero decimal places;
- WNOK is sandbox wholesale cash, not a production CBDC representation;
- the repository is evolving, so implementation MUST pin a fresh upstream
  commit before contracts, genesis, or validator topology are assumed; and
- the inspected local checkout is stale relative to the upstream development
  branch and MUST NOT be used as an unreviewed deployment source.

The upstream sandbox's current controlled topology is suitable for a product
demonstration, not for a claim of Byzantine-finalized public money. A single
QBFT validator can produce deterministic blocks but does not supply BFT fault
tolerance.

The separately proposed `Wnok.settle()` authority path is not required for
pNOK. A bridge vault MUST NOT receive root-equivalent authority to debit
arbitrary WNOK holders. Deposits use explicit user approval plus a
recipient-bound vault call.

## 6. pNOK asset and denomination

### 6.1 Asset definition

The first pNOK definition MUST include:

```text
symbol                  = pNOK
source_asset            = WNOK
source_chain            = pinned Norges Bank Besu sandbox chain
source_decimals         = 0
pftl_decimals           = 0
backing_model           = exact locked source units
issuance_authority       = activated bridge route only
redemption_authority     = accepted pNOK burn route only
private_pool            = Asset-Orchard v1
```

Mirroring WNOK's zero decimals avoids an untracked scale or dust liability:

```text
1 WNOK locked = 1 pNOK issued
1 pNOK burned = 1 WNOK releasable
```

If WNOK later adopts øre or another decimal convention, that is a new pNOK
asset version and route epoch. Existing pNOK balances MUST NOT be silently
rescaled.

### 6.2 Supply conservation

At every finalized PFTL state:

```text
outstanding_pnok_supply
  = transparent_pnok
  + shielded_pnok
```

and across the bridge:

```text
wnok_vault_balance
  = outstanding_pnok_supply
  + finalized_burns_not_yet_released

cumulative_wnok_deposits
  = outstanding_pnok_supply
  + finalized_burns_not_yet_released
  + cumulative_wnok_releases
```

The exact ledger implementation may use the established `V = S + D + B - R`
term names, but it MUST publish one canonical equation and test every state
transition against it. Transparent-to-shielded movement and the inverse are
representation moves inside PFTL and MUST NOT change total pNOK supply.

The private FX swap MUST preserve each asset independently:

```text
pfUSDC inputs = pfUSDC outputs
pNOK inputs   = pNOK outputs
```

Any PFTL transaction fee is accounted outside these issued-asset conservation
equalities. The first demo displays a zero FX trading fee.

## 7. Target architecture

### 7.1 pNOK ingress

```text
allowlisted WNOK holder
  -> approves exact WNOK amount to WnokBridgeVaultV1
  -> calls deposit(amount, pftl_recipient, nonce)
  -> vault measures exact balance delta and emits recipient-bound event
  -> Besu block reaches the route's declared finality/checkpoint rule
  -> relayer builds bounded deposit evidence
  -> PFTL verifies route, chain, vault, WNOK, amount, recipient, and deposit ID
  -> exact pNOK is credited once
```

The vault and depositor MUST both satisfy WNOK allowlist policy. Fee-on-
transfer behavior is not expected, but the vault MUST use the exact received
balance delta rather than trusting a calldata amount.

### 7.2 Private fixed-rate swap

```text
public, finalized FxFixPacketV1
  + Bob private pfUSDC note
  + facility private pNOK note
  + both spend authorizations
  -> Asset-Orchard two-input/two-output proof
  -> consensus verifies proof, fix binding, expiry, policy, and nullifiers
  -> Bob private pNOK note
  -> facility private pfUSDC note
```

Both outputs are created or neither is. A coordinator can delay or withhold a
transaction, but cannot validly settle one leg without the other.

### 7.3 pNOK egress

```text
private pNOK note
  -> Asset-Orchard private egress to transparent pNOK
  -> accepted pNOK burn-to-redeem
  -> pNOK BridgeExitLeafV1 in finalized PFTL block
  -> controlled checkpoint evidence for demo, SP1 finality proof for Tier 4
  -> WnokBridgeVaultV1 consumes withdrawal nullifier
  -> exact WNOK released to allowlisted Besu recipient
```

The private PFTL middle does not make the WNOK withdrawal private. The exit
amount and Besu recipient are public at the source boundary.

## 8. Trust classes and release claims

Bridge trust and swap atomicity are independent properties. The demo can have
a cryptographically private, atomic PFTL swap while its WNOK bridge still uses
a controlled source checkpoint.

### 8.1 Demo profile: `CONTROLLED`

The first demo route MUST declare:

```text
route_trust_class = CONTROLLED
live_value_enabled = false
source_environment = Norges Bank sandbox
```

It MAY accept a pinned, signed Besu checkpoint from the controlled sandbox
operator, provided the checkpoint is chain-, block-, vault-, code-, and
route-bound and replay protected. The UI and receipts MUST say `Controlled
sandbox checkpoint`; they MUST NOT say `trustless`, `Tier 4`, or `BFT finality`.

This follows the deployed PFTL rule that `CONTROLLED` routes cannot enable
public live-value routing.

### 8.2 Tier-4 profile

The pNOK route reaches Tier 4 only when:

- PFTL verifies an SP1 proof of the exact WNOK deposit under a valid finalized
  Besu QBFT chain segment from a pinned checkpoint;
- the Besu vault verifies an SP1 proof of the exact accepted pNOK burn under a
  finalized PFTL chain segment;
- validator-set transitions on both sides are proven from prior accepted
  state;
- relayers are untrusted transports;
- proof, packet, deposit, withdrawal, and route replay protections are active;
- the route has no observer, operator-signature, or threshold fallback; and
- adversarial and recovery batteries pass against a multi-validator source
  network whose stated fault assumptions are actually met.

Tier-4 activation requires a new route epoch. A controlled deposit cannot be
retroactively relabeled Tier 4.

### 8.3 Required claim matrix

| Property | Controlled demo | Tier-4 target |
|---|---|---|
| WNOK backing | exact vault accounting | exact vault accounting |
| PFTL private swap proof | required | required |
| Atomic two-asset settlement | required | required |
| Public fix binding | required | required |
| Source deposit finality | controlled checkpoint | proof-verified QBFT segment |
| WNOK release authorization | controlled demo verifier | proof-verified PFTL finality |
| Public live value | forbidden | separately governed after qualification |
| “Trustless bridge” claim | forbidden | permitted only after both directions pass |

## 9. WNOK bridge contract

### 9.1 `WnokBridgeVaultV1`

The source vault SHOULD be a narrow, non-upgradeable demo contract or a
version-pinned implementation behind an explicitly governed proxy. Its public
surface is:

```solidity
deposit(uint256 amount, bytes pftlRecipient, bytes32 clientNonce)
release(PnokWithdrawalPacketV1 packet, bytes proof, bytes publicValues)
```

`deposit` MUST:

- require nonzero amount and a canonical PFTL recipient;
- pull WNOK with `transferFrom` only after explicit user approval;
- measure the vault's exact before/after WNOK balance;
- require the measured increase to equal the declared amount;
- emit all fields needed to recompute the deposit ID;
- bind source chain ID, vault, WNOK token, depositor, recipient, amount, nonce,
  and route epoch; and
- reject a duplicate client nonce for the same depositor and route.

The vault must be allowlisted and receive `TRANSFER_FROM_ROLE` if required by
the pinned WNOK implementation. It MUST NOT receive WNOK `MINTER_ROLE`,
`BURNER_ROLE`, or unrestricted settlement authority.

`release` MUST:

- verify the activated route, verifier, proof program, and public values;
- require an accepted PFTL burn receipt and exact packet commitment;
- bind the exact WNOK recipient and amount;
- require the recipient to be allowlisted under the pinned source policy;
- consume a full-width withdrawal nullifier before external token transfer;
- prevent reentrancy; and
- transfer exactly once or revert atomically.

### 9.2 Events

At minimum:

```text
WnokDeposited(
  route_epoch,
  deposit_id,
  depositor,
  pftl_recipient_hash,
  amount,
  client_nonce
)

WnokReleased(
  route_epoch,
  withdrawal_id,
  pftl_burn_tx_id_hash,
  recipient,
  amount,
  proof_nullifier
)
```

Every identifier is computed from canonical, domain-separated encodings. A
48-byte PFTL identifier MUST be hashed in full before use as a 32-byte EVM
commitment; truncation is forbidden.

## 10. Route and proof artifacts

### 10.1 Route profile

`PnokBridgeRouteProfileV1` may be implemented as a new version of the generic
vault route profile. It MUST pin:

- PFTL chain ID, genesis hash, and protocol version;
- source chain ID and genesis hash;
- source consensus/finality kind;
- source validator-set or checkpoint root;
- WNOK address and runtime code hash;
- vault address and runtime code hash;
- pNOK asset ID and decimals;
- ingress and egress schema versions;
- SP1 ingress and egress verification keys where applicable;
- proof and public-value size limits;
- deposit and withdrawal limits for controlled testing;
- activation and deactivation heights;
- route epoch;
- trust class and `live_value_enabled`; and
- route configuration digest.

Limits are operational safety controls, not a permanent pNOK maximum supply.

### 10.2 `PnokIngressPublicValuesV1`

The Tier-4 encoding MUST include at least:

- schema and proof program versions;
- PFTL chain/genesis/protocol, route epoch, and route profile hash;
- Besu chain ID and genesis hash;
- prior and resulting accepted Besu checkpoints;
- source validator-set root and any proven transition;
- finalized block number/hash and receipt root;
- transaction hash, receipt index, log index, and event commitment;
- WNOK and vault addresses plus runtime code hashes;
- depositor, PFTL recipient bytes/hash, amount, client nonce, and deposit ID;
- evidence root; and
- a domain-separated public-values commitment.

The controlled demo evidence MUST use a distinct schema or explicit trust-class
field. A controlled witness MUST not parse as Tier-4 public values.

### 10.3 `PnokEgressPublicValuesV1`

The Tier-4 encoding MUST include at least:

- schema and proof program versions;
- PFTL chain/genesis/protocol, route epoch, and route profile hash;
- prior and resulting proof-verified PFTL checkpoints;
- committee epoch/root and proven committee transitions;
- finalized height/view/block ID/state root/bridge-exit root;
- exact exit leaf index and commitment;
- accepted transaction receipt identifier and literal accepted receipt code;
- pNOK asset ID, burn transaction ID, withdrawal ID, amount, and recipient;
- source chain, WNOK, vault, and runtime code bindings;
- packet digest, full-width withdrawal commitment, and proof nullifier; and
- a domain-separated public-values commitment.

### 10.4 State continuity

Proof verification is stateful. Neither PFTL nor the Besu vault may accept an
arbitrary recent header supplied by the relayer. Each proof advances from a
previously accepted checkpoint. Validator-set changes are proven as part of
that advancement.

## 11. The FX fix

### 11.1 Public fixing packet

The first implementation introduces `FxFixPacketV1`:

```text
version
fix_id
base_asset_id                 = pfUSDC
quote_asset_id                = pNOK
base_asset_tag
quote_asset_tag
ratio_numerator
ratio_denominator
effective_pftl_height
expires_pftl_height
maximum_base_atoms
minimum_base_atoms
liquidity_facility_id
source_label
source_observation_commitment
governance_policy_hash
previous_fix_hash
packet_hash
```

The ratio uses asset atoms, not display units:

```text
quote_atoms = floor(base_atoms * ratio_numerator / ratio_denominator)
```

All multiplication uses checked `u128` intermediates and the committed values
fit the existing `u64` Asset-Orchard ratio fields. Floating point, decimal
strings in consensus, wall-clock expiry, and implicit locale conversion are
forbidden.

For the first demo, `source_label` may identify a manually approved demo fix.
The UX MUST say `Demo fix`. It MUST NOT call that price an official central-
bank fixing or independently sourced market rate unless the packet really is
derived under such a governed source policy.

### 11.2 Example

At a public fix of `10.500000 NOK per USD`, with pfUSDC using six decimal
atoms and pNOK using whole-NOK atoms:

```text
ratio_numerator   = 21
ratio_denominator = 2,000,000

20.000000 pfUSDC = 20,000,000 base atoms
quote_atoms       = floor(20,000,000 * 21 / 2,000,000)
                  = 210 pNOK
```

The demo amount MUST be selected so the remainder is zero. The circuit already
enforces deterministic floor rounding for other valid amounts; the wallet
MUST display the exact rounded pNOK output before signature and MUST NOT label
the result as an unrounded amount.

### 11.3 Capacity and expiry

The executable amount is the minimum of:

- remaining fix-packet capacity;
- the facility's spendable private pNOK notes;
- the user's spendable private pfUSDC notes;
- route and transaction safety caps; and
- the amount that can complete before the fix's PFTL expiry height.

Concurrent fills require reservations. A reservation is bound to the fix hash,
facility, asset pair, exact maximum input, wallet intent, and expiry height.
Reservations and fills MUST be consensus-visible bounded state so two workers
cannot overfill one fix.

### 11.4 Fix update

A new fix creates a new immutable packet linked to the prior packet. It does
not mutate an existing packet. Unfilled reservations on an expired packet are
released. A fill can execute only against the exact packet hash signed by both
parties.

## 12. Reuse of Asset-Orchard

### 12.1 Already implemented

The existing Asset-Orchard swap is a fixed two-input/two-output Halo2 circuit.
It already:

- consumes two private typed notes;
- proves spend authority and membership;
- prevents nullifier replay;
- preserves each input asset across the output permutation;
- binds a rational pricing claim to the two private input values;
- permits deterministic floor rounding smaller than the denominator;
- creates two encrypted output notes; and
- exposes commitments, nullifiers, stable asset tags, ratio, fee, and proof
  data without exposing note openings or raw values.

The current public pricing structure is `AssetOrchardPricingClaim`, with:

```text
nav_epoch
reserve_packet_hash
ratio_numerator
ratio_denominator
mode
band_bps
base_asset_tag
quote_asset_tag
```

Validator policy cross-multiplies the claimed ratio against the registered
policy using checked `u128` arithmetic.

### 12.2 Fast demo compatibility mapping

To avoid a new circuit and proving-key ceremony on the demo critical path, the
first controlled route MAY use the current V1 claim as follows:

| Existing V1 field | pNOK demo meaning |
|---|---|
| `nav_epoch` | `fix_epoch` compatibility slot |
| `reserve_packet_hash` | canonical `FxFixPacketV1.packet_hash` |
| `ratio_numerator` | fix numerator in asset atoms |
| `ratio_denominator` | fix denominator in asset atoms |
| `mode` | `negotiated` |
| `band_bps` | `0` |
| asset tags | exact registered pfUSDC/pNOK tags |

PFTL consensus MUST derive the active `AssetOrchardPricingPolicy` from the
finalized fix packet; a backend-supplied ratio is insufficient. With band zero,
the claimed ratio must equal the registered fix exactly.

This mapping is a controlled-demo compatibility device. A later
`AssetOrchardPricingClaimV2` SHOULD rename the NAV-specific fields to generic
`pricing_epoch` and `pricing_packet_hash` and add `fixed_fx` as a first-class
mode. V1 bytes and verification keys MUST remain immutable.

### 12.3 Privacy statement

For this demo, public observers can see:

- the Asset-Orchard pool and circuit identifiers;
- stable one-way asset tags identifying the registered pair;
- the public fix ratio and packet commitment;
- nullifiers, output commitments, encrypted outputs, proof bytes, fee, and
  transaction timing.

They do not see in the public swap action:

- raw note values;
- note openings;
- PFTL account identifiers for note owners or recipients;
- spending keys or viewing keys; or
- plaintext output ownership.

Source-chain deposits and withdrawals remain public and can create timing and
amount correlation. The demo MUST say `private execution on PFTL`, not
`private end to end`.

### 12.4 Two-party authorization boundary

The existing action builder needs both private input witnesses and both spend
authorizations to create one proof. The first demo MAY use two controlled test
wallets in one isolated local runner. That proves circuit-level atomic PvP,
pricing, replay resistance, and note ownership; it does not prove a
production non-custodial bilateral coordination protocol.

Before unattended external users, replace the shared controlled runner with a
reviewed two-party flow in which:

- Bob signs an intent bound to the exact fix, maximum pfUSDC, minimum pNOK,
  recipient viewing key, expiry, and idempotency key;
- the facility signs a matching liquidity authorization;
- no coordinator receives either long-term spending key;
- each party can independently verify the final action and output commitment;
  and
- timeout or coordinator failure leaves both input notes unspent.

## 13. Consensus operations

New operations SHOULD follow existing bounded transaction and stable-error
conventions:

```text
fx_fix_register_v1
fx_fix_pause_v1
fx_fix_reservation_create_v1
fx_fix_reservation_release_v1
pnok_bridge_deposit_propose_v1
pnok_bridge_deposit_finalize_v1
pnok_bridge_claim_v1
pnok_bridge_burn_to_redeem_v1
pnok_bridge_release_finalize_v1
```

The actual implementation MAY parameterize existing vault bridge operations
instead of adding pNOK-specific names, provided historical pfUSDC schemas do
not change and RPC responses identify the asset and schema unambiguously.

State MUST use bounded maps for active fixes, reservations, processed deposit
IDs, withdrawal IDs, and proof nullifiers. Expiry and pruning are based on
PFTL height, not wall clock. Every operation validates all fields before
mutating state, and batch execution uses the existing trial-state atomicity
model.

## 14. Detailed demo flow

### Phase A — freeze and deploy the source

1. Fetch and review the current upstream Norges Bank `development` revision.
2. Record the exact commit, Besu version, chain ID, genesis hash, consensus
   configuration, validator list, WNOK address, and WNOK runtime code hash.
3. Deploy `WnokBridgeVaultV1` from reviewed source.
4. Add the vault to the WNOK allowlist.
5. Grant only the minimum `TRANSFER_FROM_ROLE` required for explicit deposits.
6. Fund an allowlisted facility wallet with demo WNOK.
7. Record deployment bytecode, constructor arguments, transaction, receipt,
   administrator, and role state.

### Phase B — register pNOK on PFTL

1. Register the pNOK asset with zero decimals and no unrelated NAVCoin policy.
2. Register the controlled bridge route with `live_value_enabled=false`.
3. Pin every source and PFTL identifier from the route profile.
4. Register deterministic asset tags for pNOK and pfUSDC in Asset-Orchard.
5. Verify the six-validator fleet agrees on the route, asset definition, and
   state root.

### Phase C — issue and shield facility pNOK

1. Facility approves the vault for exactly `500 WNOK`.
2. Facility deposits `500 WNOK` with a PFTL recipient and unique nonce.
3. The controlled checkpoint adapter builds exact deposit evidence.
4. PFTL finalizes and claims exactly `500 pNOK` once.
5. Facility shields exactly `210 pNOK` into an Asset-Orchard note and leaves
   `290 pNOK` transparent. This creates the exact input denomination required
   by the fixed two-input/two-output swap.
6. Record public WNOK and pNOK deltas plus the private output commitment.

### Phase D — prepare Bob's pfUSDC

1. Use pfUSDC already finalized on PFTL; Ethereum ingress is outside the
   private-swap latency clock.
2. Bob shields `20.000000 pfUSDC` into Asset-Orchard.
3. Record the public pfUSDC debit and private output commitment.
4. Do not write Bob's note opening, viewing key, or spending key into the
   public evidence bundle.

### Phase E — publish the fix

1. Register a demo fix of `10.500000 pNOK per pfUSDC`.
2. Set a short but workable PFTL-height validity window.
3. Set capacity to at most the facility's verified pNOK liquidity.
4. Finalize the packet through PFTL consensus.
5. Have both clients recompute and display the packet hash and exact quote.

### Phase F — execute the private swap

1. Build one Asset-Orchard action consuming Bob's pfUSDC note and `210 pNOK`
   of facility liquidity. If the facility note is larger, note selection must
   first create an exact denomination or a reviewed multi-note/change path;
   the two-input/two-output swap itself cannot create a third change output.
2. Create Bob's `210 pNOK` output and the facility's `20.000000 pfUSDC`
   output.
3. Bind the action to the exact fix epoch, packet hash, ratio, asset tags,
   zero band, zero fee, and expiry.
4. Derive the action-binding hash from that immutable action, then reserve
   exactly `20.000000 pfUSDC` and `210 pNOK` against the exact fix hash,
   wallet-intent hash, and action-binding hash. The reservation cannot be
   created before the action exists and cannot be reused for another action.
5. Collect both spend authorizations and bind the finalized reservation into
   the batch pricing claim.
6. Prove, preflight against every validator, submit once, and wait for PFTL
   finality.
7. Scan the encrypted outputs with the intended recipient keys.
8. Verify both input nullifiers are spent exactly once and both outputs exist.
9. Release or mark the reservation filled atomically.

The facility should therefore create a `210 pNOK` note before the demo, not a
single indivisible `500 pNOK` note, unless the generalized note-splitting path
has been implemented and qualified.

### Phase G — optional backing demonstration

1. Bob privately exits `10 pNOK` to his transparent PFTL balance.
2. Bob burns `10 pNOK` to an allowlisted Besu recipient.
3. The controlled demo verifier authorizes exactly `10 WNOK` release.
4. Verify pNOK supply decreases by 10 and vault WNOK decreases by 10.
5. Replay the same release and verify hard failure with no second transfer.

This optional leg demonstrates redeemability. It remains a public boundary
and a controlled checkpoint until Tier 4 is completed.

## 15. Wallet and demo UX

The UX MUST be asset-driven, not hardcoded to A666 or one wallet.

The exchange screen shows:

```text
You pay             20.000000 pfUSDC
Fix                  1 pfUSDC = 10.500000 pNOK
You receive          210 pNOK
Price impact         0
Trading fee          0
Fix expires          PFTL height N
Liquidity source     pNOK demo facility
Execution privacy    Private on PFTL
Bridge trust         Controlled sandbox checkpoint
```

Before signing, the wallet independently verifies:

- chain/genesis/protocol;
- exact asset IDs and stable tags;
- exact fix packet hash and policy hash;
- numerator, denominator, rounding result, fee, capacity, and expiry;
- input note ownership and unspent status;
- output recipient keys;
- route trust class; and
- idempotency lineage.

The status machine is durable across navigation and refresh:

```text
funding confirmed
-> shielding pfUSDC
-> fix reserved
-> proving private exchange
-> submitted
-> finalized
-> private pNOK received
```

If the user leaves the page, the operation remains in recent activity and can
resume from its durable intent ID. A retry reuses the same lineage; it MUST NOT
create a second reservation or second spend attempt.

The wallet MUST not show `complete` merely because a backend accepted the
request. Completion means the finalized PFTL state contains both spent
nullifiers and the expected output commitments and the wallet has scanned its
owned output.

## 16. Service and key boundary

For the controlled demo:

- long-lived proving keys may be warmed by the resident prover;
- test spending keys remain on the isolated demo runner;
- logs, HTTP responses, evidence bundles, and process arguments MUST exclude
  spending keys, viewing keys, note openings, and plaintext encrypted outputs;
- the coordinator may transport public actions and proofs but cannot alter the
  fix or outputs after signatures; and
- only explicit test-value route profiles are enabled.

For unattended users:

- wallet spending/viewing material remains client-side;
- the prover receives only the minimum witness under an approved local or
  remote-proving trust model;
- the proxy receives only a signed transaction;
- cancellation and replacement semantics are complete;
- authentication and rate limiting are active; and
- the controlled two-key runner is disabled.

## 17. Failure behavior

Every failure is fail-closed and leaves no partial economic state.

Required stable failures include:

```text
pnok_route_not_active
pnok_source_checkpoint_untrusted
pnok_source_contract_mismatch
pnok_deposit_already_claimed
pnok_deposit_amount_mismatch
pnok_withdrawal_already_released
pnok_recipient_not_allowlisted
fx_fix_not_active
fx_fix_expired
fx_fix_pair_mismatch
fx_fix_ratio_mismatch
fx_fix_capacity_exhausted
fx_fix_reservation_conflict
fx_fix_rounding_not_displayed
asset_orchard_note_not_owned
asset_orchard_nullifier_already_spent
asset_orchard_output_recipient_mismatch
```

Retries classify errors as retryable transport/availability failures or
terminal policy/proof failures. A terminal failure cannot be retried under a
new packet without fresh user authorization.

## 18. Security and adversarial tests

### 18.1 Source bridge

- wrong Besu chain ID or genesis;
- wrong WNOK or vault address;
- correct address with wrong runtime code hash;
- non-allowlisted depositor, vault, or release recipient;
- amount/event/balance-delta mismatch;
- reverted transaction or removed/reorganized receipt;
- duplicate deposit nonce, event, proof, or claim;
- fabricated controlled checkpoint;
- stale or discontinuous Tier-4 checkpoint;
- forged validator-set transition;
- accepted proof under wrong route epoch;
- release proof for rejected PFTL burn;
- modified recipient or amount;
- truncated PFTL identifier collision attempt;
- withdrawal replay and reentrancy; and
- route deactivation while work is in flight.

### 18.2 Fix and reservation

- zero numerator or denominator;
- arithmetic overflow;
- swapped base/quote tags;
- correct ratio under wrong packet hash;
- stale, future, paused, or expired fix;
- capacity overfill by concurrent workers;
- reservation replay or cross-wallet substitution;
- fill after reservation expiry;
- backend quote differing by one atom from wallet recomputation; and
- packet update attempting to mutate an already signed fix.

### 18.3 Private swap

- wrong note owner or anchor;
- missing or forged spend authorization;
- duplicated input nullifier;
- wrong output recipient;
- wrong asset permutation;
- value creation in either asset;
- copied pricing claim from another action;
- ratio outside the zero-width policy band;
- malformed proof or proof for another circuit/vkey;
- exact transaction replay; and
- crash before submit, after submit, and after finality but before local note
  persistence.

### 18.4 Privacy scan

Machine-scan the public action, receipts, logs, API payloads, and evidence
bundle for:

- raw pfUSDC and pNOK asset IDs where only tags are permitted;
- private values;
- PFTL owner or recipient identifiers;
- note openings, seeds, spending keys, and viewing keys;
- plaintext memos; and
- accidental environment or process dumps.

The public fix ratio and stable tags are expected public fields and are not
privacy failures.

## 19. Performance budgets

The first demo measures three clocks separately:

1. WNOK source deposit to spendable pNOK on PFTL;
2. already-funded private pfUSDC/pNOK swap on PFTL; and
3. optional pNOK burn to finalized WNOK release.

Ethereum pfUSDC ingress is not charged to the private-swap clock when Bob is
already funded.

The existing resident private service most recently reported four-sample
issue p95 of 50.365 seconds and redeem p95 of 38.862 seconds against a
42-second issue qualification gate. Those A666 measurements are useful
operational precedent, not proof that the new two-party FX route meets a
particular SLO.

Initial controlled-demo targets are:

```text
private swap request accepted -> local finality       <= 60 seconds p95
warm proof DAG                                       <= 20 seconds p95
finality -> owned output visible in wallet            <= 10 seconds p95
duplicate/replay rejection                            <= 1 finalized attempt
```

No performance claim is published from fewer than 20 complete warm runs. A
production claim requires at least 100 successful swaps, fault injection, and
reported p50/p95/p99 by stage.

## 20. Evidence bundle

Each qualification run writes one immutable directory containing:

```text
manifest.json
source-route-profile.json
pftl-route-profile.json
source-deployment.json
source-before.json
source-deposit-receipt.json
pnok-claim-receipt.json
fix-packet.json
fix-policy.json
wallet-public-before.json
swap-public-action.json
swap-finality.json
wallet-public-after.json
private-output-scan.redacted.json
conservation-report.json
replay-report.json
privacy-scan.json
timings.json
checksums.sha256
```

The manifest pins repository revisions, build hashes, binary hashes, proof
verification keys, chain checkpoints, route epochs, and test-wallet public
identifiers. Secret material is never included.

## 21. Implementation phases and checklists

### Phase 0 — freeze the exact baseline

- [x] Sync and review the current official Norges Bank development revision.
- [x] Pin WNOK source, bytecode, decimals, roles, allowlist behavior, and
  deployment address.
- [x] Pin Besu version, chain/genesis, consensus configuration, validators,
  RPC, and block behavior.
- [x] Trace the current pfUSDC generic bridge types and list exact fields that
  can be reused without historical encoding changes.
- [x] Freeze pNOK asset, route, event, deposit, withdrawal, and fix encodings.
- [x] Generate mutation-sensitive Rust/Solidity/JSON conformance vectors.

**Gate:** one reviewed protocol artifact document names every byte binding and
trust assumption.

### Phase 1 — controlled WNOK/pNOK bridge

- [x] Implement and test `WnokBridgeVaultV1`.
- [x] Add pNOK route and asset types using bounded deterministic state.
- [x] Implement controlled checkpoint evidence under a schema that cannot be
  mistaken for Tier 4.
- [x] Implement exact pNOK claim and burn/release accounting.
- [x] Deploy to the sandbox and PFTL controlled fleet.
- [x] Pass deposit, release, conservation, and replay batteries.

**Gate:** `500 WNOK -> 500 pNOK -> 500 WNOK` completes with exact balances
and the route is visibly labeled controlled.

Implementation and automated release/conservation batteries are complete. The
live qualification deposited `500 WNOK` and issued `500 pNOK`; it deliberately
did not execute the optional live WNOK release, so the full live round-trip
form of this gate remains unclaimed.

### Phase 2 — fixed-rate private exchange

- [x] Implement `FxFixPacketV1`, consensus registration, pause, expiry, and
  capacity.
- [x] Derive the existing Asset-Orchard pricing policy from the finalized fix.
- [x] Register pNOK in Asset-Orchard.
- [x] Add bounded reservation/fill state.
- [x] Add exact pNOK note denomination preparation for the two-output circuit.
- [x] Build and execute the `20 pfUSDC -> 210 pNOK` private action.
- [x] Verify output ownership, nullifiers, zero fee, exact fix, and replay
  rejection.

**Gate:** the private action finalizes atomically at the registered fix and no
private opening or amount appears in public artifacts.

### Phase 3 — demo UX and repetition

- [x] Add asset-driven pNOK and FX-fix discovery to the wallet.
- [x] Add the quote card, explicit public/private boundaries, and trust label.
- [x] Persist run status across navigation and refresh.
- [x] Add recovery by durable intent ID.
- [x] Run the complete controlled demo 10 consecutive times.
- [x] Run one expired-fix, one duplicate-submit, one prover-restart, and one
  validator-unavailable recovery case.
- [x] Produce one redacted evidence bundle per run and an aggregate report.

**Gate:** 10/10 complete without manual state edits, duplicate economic
effects, or secret leakage.

### Phase 4 — Tier-4 pNOK ingress

- [ ] Specify and implement a Besu/QBFT finality SP1 guest.
- [ ] Pin a multi-validator source checkpoint and prove continuous headers,
  receipts, and validator transitions.
- [ ] Verify the exact WNOK deposit public values on PFTL.
- [ ] Activate a new no-fallback route epoch.
- [ ] Pass invalid-header, invalid-quorum, stale-checkpoint, fork, replay,
  proof-size, and recovery tests.

**Gate:** PFTL issues pNOK from proof-verified source finality without a bridge
signer or controlled assertion.

### Phase 5 — Tier-4 pNOK egress

- [ ] Reuse/version the PFTL exit-root commitment for pNOK burns.
- [ ] Implement the pNOK PFTL-finality SP1 guest and Solidity verifier.
- [ ] Make the Besu vault consume only proof-verified accepted burn packets.
- [ ] Prove PFTL committee transitions from a pinned checkpoint.
- [ ] Remove and technically forbid controlled fallback in the Tier-4 epoch.
- [ ] Pass adversarial, restart, duplicate, migration, and full conservation
  batteries.

**Gate:** both directions meet Tier 4 under an activated no-fallback route.

### Phase 6 — unattended-user hardening

- [ ] Replace the two-key demo runner with reviewed non-custodial bilateral
  authorization.
- [ ] Complete local key custody, backup, cancellation, and replacement.
- [ ] Add authentication, authorization, rate limiting, and abuse controls.
- [ ] Qualify 100+ swaps plus fault and privacy campaigns.
- [ ] Commission independent circuit, consensus, bridge, and contract review.
- [ ] Obtain legal/brand authorization for any claim involving Norges Bank.

**Gate:** production readiness is decided separately from Tier-4 protocol
correctness and separately from the controlled demo.

## 22. First demo acceptance criteria

The demo is a pass only when all of the following are true:

- [x] The exact upstream WNOK and Besu revision is pinned and recorded.
- [x] The WNOK vault is allowlisted and has no mint, burn, or broad settlement
  authority.
- [x] `500 WNOK` deposited creates exactly `500 pNOK` once.
- [x] pNOK total supply exactly matches bridge accounting.
- [x] The facility owns a spendable private pNOK note of the exact trade size.
- [x] Bob owns a spendable private `20.000000 pfUSDC` note.
- [x] A finalized public fix says exactly `10.500000 pNOK/pfUSDC`, zero band,
  zero fee, and a bounded expiry/capacity.
- [x] One Asset-Orchard proof consumes Bob's pfUSDC and facility pNOK inputs.
- [x] Bob scans and controls exactly `210 pNOK` after finality.
- [x] The facility scans and controls exactly `20.000000 pfUSDC` after
  finality.
- [x] Neither asset's total supply changes during the swap.
- [x] Both input nullifiers are spent exactly once.
- [x] Exact replay fails and creates no new output.
- [x] Public artifacts contain the fix and tags but no note openings, private
  values, owners, recipients, or secret keys.
- [x] The UX says `private on PFTL` and `controlled sandbox checkpoint`.
- [x] The complete demo passes 10 consecutive times without manual state
  repair.
- [ ] Optional backing proof releases exact WNOK only after an accepted pNOK
  burn and rejects replay.

Passing this section proves a controlled pNOK/private-FX demo. It does not
prove Tier 4 or production readiness.

## 23. Tier-4 acceptance criteria

pNOK may be called Tier 4 only when:

- [ ] exact WNOK ingress is proven under continuous Besu/QBFT finality;
- [ ] exact WNOK release is proven from finalized PFTL burn acceptance;
- [ ] both verifiers advance statefully from pinned checkpoints;
- [ ] validator-set transitions are proven, not assigned by a relayer;
- [ ] no observer, operator, or threshold fallback is callable for the active
  route;
- [ ] full-width identifiers and domain-separated nullifiers prevent replay;
- [ ] source vault balance equals the canonical pNOK liability equation after
  deposit, shielding, swap, egress, burn, release, failure, and restart;
- [ ] route migration preserves in-flight operations under their original
  epoch and prevents cross-epoch substitution;
- [ ] multi-validator source and PFTL adversarial batteries pass; and
- [ ] an independent reviewer can reproduce the result from the evidence
  bundle without privileged narrative.

## 24. Recommended execution order

The shortest path to the requested product proof is:

1. freeze the current WNOK/Besu source;
2. build the narrowly controlled WNOK-to-pNOK bridge;
3. issue and pre-denominate facility pNOK;
4. register one public demo fix;
5. reuse the current Asset-Orchard circuit for one private atomic swap;
6. put the flow into the existing asset-driven wallet and run it 10 times;
7. optionally prove pNOK-to-WNOK redemption; then
8. replace each controlled bridge boundary with its Tier-4 proof verifier.

This order produces the requested private fixed-rate trading demonstration
without pretending the Norges Bank sandbox already supplies a production
trustless finality boundary. It also preserves a direct path to the original
pfUSDC Tier-4 standard instead of creating an incompatible demo bridge.
