# PostFiat L1 Bridge, Ethereum, and Contract Security Audit

**Audit date:** 2026-07-16
**Code baseline:** `4b5af7bc6bb6e793ed8a60219d13d6d35be03058`
**Status:** STEP 1 evidence; unverified PFTL-Uniswap transitions are live-disabled in the local candidate

## 1. Executive verdict

The repository contains several different systems called “bridge.” They do not
share one trust model and must not be presented as one trustless canonical
bridge:

1. a generic PFTL bridge state machine with ML-DSA witness certificates;
2. issued-asset/vault-bridge operations with NAV profiles, source observations,
   SP1 or multi-fetch evidence and an Ethereum vault;
3. a PFTL-to-Uniswap handoff/return route with controlled or optimistic receipt
   verifiers;
4. Ethereum market-operations contracts whose owner/proposer posts PFTL-derived
   envelopes;
5. an EVM withdrawal verifier using a configured secp256k1 signer threshold.

Two confirmed P0s cross these surfaces:

- `P0-BRIDGE-01`: PFTL Uniswap consume/import/refund operations accept asserted
  Ethereum hashes/heights or locally recomputable “non-consumption” values
  without Ethereum header/receipt/log/finality/absence proof. The corresponding
  controlled EVM receipt verifier can even self-label an owner toggle as
  `TRUSTLESS_FINALITY`.
- `P0-SUPPLY-01`: at baseline, `MintController.releaseMint` released transferable
  issued tokens based on a beneficiary-authored struct with no verifier.

These are supply/custody integrity defects, not maturity-label issues. The
public release candidate must disable the affected transitions/contracts until
complete proofs are implemented and tested.

**Local containment update:** strict PFTL execution now rejects every
`PftlUniswap*` operation before mutation; only authenticated historical replay
retains the old decoder. `ControlledPFTLReceiptVerifier` can no longer claim
`OPTIMISTIC` or `TRUSTLESS_FINALITY`. This contains P0-BRIDGE-01; it does not
implement the trustless verifier described below or authorize a production
bridge deployment.

`MintController` now requires an immutable settlement-verifier contract and
accepts release only when the verifier returns a one-time result bound to the
pending mint, escrow, beneficiary, exact mint amount, and proof hash; caller
booleans/values must exactly match that result. Fabrication, value inflation,
cross-escrow reuse, replay, and verifier replacement regressions pass. No
production deployment is authorized until a concrete verifier implementation
and runtime code-hash policy are selected and audited.

## 2. Contract inventory

| Contract | Authority and purpose | Material trust boundary | Audit status |
|---|---|---|---|
| `PolicyRegistry.sol` | Owner registers/deactivates immutable policy identities | A single owner is the policy authority; no PFTL governance proof | Controlled, not decentralized governance |
| `PFTLBridgeAdapter.sol` | Approved proposer posts market envelope; permissionless challenge; delay then acceptance | No PFTL certificate verifier. Owner selects proposers and pause state; challenge freezes but does not prove favorable claims | Controlled/optimistic only; public claims must say so |
| `MarketOpsVault.sol` | Holds reserve tokens and executes owner-configured venues under accepted envelope caps | Owner chooses venue; bridge adapter asserts envelope; external venue/token behavior | Requires contract fuzz/invariant and exact controlled trust disclosure |
| `MintController.sol` | Mints into escrow and releases after verifier-bound settlement/backing | Deployment inherits the complete trust/safety of its immutable verifier | Boundary fixed; production verifier/code hash still required |
| `NAVGuardHook.sol` | Owner posts PFTL state; pool manager gates/records swaps/depth | Owner statement and configured pool manager; not a PFTL light client | Controlled oracle/hook, not trustless NAV verification |
| `PFTLWithdrawalVerifier.sol` | Threshold ECDSA signer proof plus challenge delay | Owner rotates signers/threshold/challenge authority; signers attest PFTL finality | Explicit federation; needs epoch/snapshot/rotation protocol |
| `ERC20BridgeVault.sol` | Custodies an ERC20, emits deposits, pays verified PFTL withdrawals | Relies on withdrawal verifier; owner pause/challenge authority; token semantics | Plausible federated vault skeleton; not enough to prove PFTL mint side |
| `ControlledPFTLReceiptVerifier` | Owner toggles receipt commitments accepted | Single-owner assertion | Must never expose `TRUSTLESS_FINALITY` class |
| `OptimisticPFTLReceiptVerifier` | Bonded claim/challenge resolved by one configured resolver | Resolver decides validity; silence accepts a claim | Optimistic trusted resolver, not cryptographic PFTL proof |
| `PacketReplayRegistry` | Controller-only packet/receipt/return nonce replay set | Authorized controller configuration | Useful replay protection, not validity proof |
| `WrappedVenueNAVCoin` | Controller-only mint and bridge burn | Controller compromise controls supply | Must be immutable to one audited controller and bounded by proof |
| `PFTLUniswapHandoffController` | Consumes accepted source packet, mints, optionally swaps, burns return | Validity inherited entirely from receipt verifier; route trust class supplied at deployment | `P0-BRIDGE-01` until exact verifier/trust labeling fixed |
| `UniswapSettlementAdapter` and V4 harness | Exact-input routing/pool-bound demo helpers | Router, Permit2, pool manager, token and slippage/deadline assumptions | Reference/demo until deployed-code and fork invariants are complete |

Generated Foundry `out/`, `cache/`, and `broadcast/` directories exist in the
working checkout but are ignored and not tracked. Release builds must recreate
them deterministically and publish source/compiler/settings/bytecode hashes;
they must not be treated as audited artifacts merely because they exist locally.

## 3. P0-BRIDGE-01 — asserted external reality

### 3.1 PFTL execution paths

The PFTL types and execution functions prove internal syntax and arithmetic but
not the referenced Ethereum events:

- `PftlUniswapDestinationConsumeOperation` carries an operator, packet/route
  hashes, asserted Ethereum transaction hash, and asserted heights;
- `apply_pftl_uniswap_destination_consume` verifies policy/status and height
  arithmetic, not a canonical Ethereum header, receipt or event;
- `PftlUniswapReturnImportOperation` carries asserted burn fields and heights;
- `apply_pftl_uniswap_return_import` recomputes a hash of those assertions and
  checks height separation, not an actual burn;
- `apply_pftl_uniswap_refund_source` calls a locally recomputable hash a
  non-consumption proof. It cannot prove absence of a destination mint.

Consequences include fictitious return burns, double representation across
chains, and delayed-consume/refund races.

### 3.2 EVM verifier labeling defect

`ControlledPFTLReceiptVerifier` is correctly implemented as an owner-controlled
boolean map, but its constructor accepts any of four labels, including
`TRUST_CLASS_TRUSTLESS_FINALITY`. `PFTLUniswapHandoffController` checks only that
its configured label equals `receipt_verifier.routeTrustClass()`. An owner-toggle
verifier can therefore be deployed under the trustless label even though no
finality proof is verified. This is a code-enforced credibility and configuration
failure.

The optimistic verifier is also not cryptographic finality: anyone can post a
bonded claim, silence lets it become accepted, and a single configured challenge
resolver decides disputes. It must be labeled as an optimistic federation/oracle
with explicit economic and availability assumptions.

### 3.3 Required fix

Immediately make these combinations unrepresentable:

- controlled verifier + `TRUSTLESS_FINALITY`;
- optimistic verifier + any trustless label;
- disabled verifier with a live consume/mint/return function;
- PFTL import/consume/refund without the exact configured evidence type.

For a retained trustless path, implement a verifier bound to:

- Ethereum chain ID and governed checkpoint/finality rule;
- canonical block header and ancestry/finality evidence;
- receipt trie inclusion and successful receipt status;
- exact contract address and deployed code hash;
- exact event topic and ABI-decoded indexed/non-indexed fields;
- route/packet/burn/return nonce, token, amount, sender, recipient and domain;
- canonical header/reorg handling and replay state;
- destination/source mutual exclusion and a refund/cancel artifact that makes
  any later mint impossible.

If a federation is retained instead, implement a distinct-signer threshold
certificate with committee epoch/root, chain/event binding, rotation/drain,
revocation, evidence and recovery, and call it federated rather than trustless.

## 4. P0-SUPPLY-01 — fictitious settlement releases mint

At the audited baseline, `MintController.requestMint` placed newly minted assets
in escrow under an accepted envelope and cap, but `releaseMint` then destroyed
the safety boundary:

- anyone may call it;
- `SettlementProof` is not a proof artifact—only recipient, two values, two
  booleans and an arbitrary hash;
- the only authenticity checks are recipient equality, nonzero/unused hash and
  at least one true boolean;
- `_proofValueUsdE8` trusts the supplied values;
- the backing inequality adds those untrusted values before releasing tokens.

A beneficiary can request a mint and self-release it by claiming fictitious
settled proceeds large enough to satisfy the inequality. The current tests build
the same unauthenticated struct as their happy path; they demonstrate intended
API use, not settlement validity.

Implemented boundary fix and remaining deployment requirement:

1. the controller now binds one immutable verifier at construction and cannot
   replace it;
2. verifier output binds pending ID, escrow ID, beneficiary, exact mint amount,
   proof hash, and verifier-authorized backing value;
3. proof hashes are consumed once and caller data cannot inflate verifier output;
4. production deployment remains disabled until the verifier itself binds the
   external chain/controller/token/envelope/payer/venue/evidence domain and its
   runtime code hash is pinned;
5. the final release candidate still needs a global supply/backing oracle across
   request, cancel/expiry, partial, release, burn, bridge and replay.

## 5. EVM withdrawal federation review

`PFTLWithdrawalVerifier` verifies sorted/deduplicated recoverable ECDSA
signatures over a digest bound to EVM chain ID, verifier contract, packet digest,
PFTL withdrawal-hash commitment and finalized PFTL height. It then applies a
challenge delay and execution window. `ERC20BridgeVault` binds source chain,
vault, token, asset ID, recipient, amount and identifiers; prevents burn and
withdrawal replay; applies a second challenge window; and transfers directly to
the destination recipient.

This is a federated bridge, not verification of ML-DSA PFTL block finality on
Ethereum. Before production it needs:

- signer-set epoch/root included in the signed digest;
- immutable snapshot semantics for pending/accepted proofs across signer and
  threshold rotation;
- minimum threshold policy consistent with the stated Byzantine bound;
- two-step/timelocked owner, signer, threshold and challenge-authority changes;
- owner/key compromise and emergency pause/recovery specification;
- chain reorganization and duplicate-finality policy;
- token behavior checks for fee-on-transfer, rebasing, callbacks, false/no
  return values and decimals;
- invariant tests that vault outflow never exceeds accepted unique PFTL burns
  and that PFTL mint never exceeds unique finalized deposits.

The ECDSA threshold proves only what its configured signers attest. Documentation
must state that trust model and distinguish it from PFTL's ML-DSA validator
certificate.

## 6. Market-operations controlled boundary

`PolicyRegistry`, `PFTLBridgeAdapter`, `NAVGuardHook`, `MarketOpsVault`, and
`MintController` form a controlled EVM market-operations stack:

- owners choose policy, proposers, pool state, venues, pause state and contract
  relationships;
- an approved proposer supplies the PFTL-derived envelope;
- challenge windows freeze disputed data but do not independently prove valid
  data when no challenger acts;
- reserve deployment has caps, cooldown, price and observed-balance checks;
- mint release currently lacks even that controlled authority check.

This can be a disclosed controlled launch architecture after P0-SUPPLY-01 is
closed. It cannot be described as an Ethereum light client, trustless bridge,
or autonomous proof of PFTL state.

## 7. Required pre-fix reproductions

- [x] Forge: beneficiary requests mint and releases with fabricated settlement
      booleans/value/hash while all external token/reserve balances prove zero
      settlement.
- [x] Rust execution: fictitious Ethereum burn assertion credits PFTL return.
- [x] Rust execution: refund source, then accept/deliver delayed destination
      consume for the same packet.
- [x] Forge: deploy `ControlledPFTLReceiptVerifier` labeled
      `TRUSTLESS_FINALITY`, owner toggles arbitrary receipt, controller mints.
- [x] Forge: optimistic false claim becomes accepted through no challenge.
- [ ] Cross-system: reuse/wrong-chain/wrong-contract/wrong-topic/wrong-token/
      wrong-amount/wrong-recipient/wrong-route proofs all fail.

## 8. STEP 2 acceptance gates

- [x] A public-build feature/profile makes every unsafe legacy/assertion path
      unreachable before any deeper implementation proceeds.
- [ ] Selected external evidence and trust class are encoded as distinct types,
      not user-selectable strings/labels.
- [ ] Every contract authority and upgrade/configuration change has an explicit
      owner/timelock/multisig/governance and recovery model.
- [x] `forge test` passes the 90-test offline suite and the separately configured
      pinned-mainnet fork test on the pinned compiler.
- [ ] Static analysis and symbolic/property tooling run on exact release
      bytecode; all findings receive recorded disposition.
- [x] Local Ethereum fork gate verifies official deployment code at pinned block
      25,440,306 and fails rather than silently skipping when the RPC URL is absent.
- [ ] Cross-language digest/ABI vectors match Rust and Solidity byte-for-byte.
- [ ] Cross-domain supply oracle proves deposits, PFTL mints/burns, EVM mints/
      burns, refunds, returns, escrow releases and vault outflows conserve value.
- [ ] Reentrancy, malicious token/router/venue, fee-on-transfer, rebasing,
      callback, front-run, sandwich, stale oracle, max integer and rounding fuzz
      suites pass.
- [ ] Deployment manifests pin compiler, optimizer, source tree, constructor
      arguments, chain ID, addresses, runtime bytecode hashes, owners/signers/
      thresholds and verified source.
- [ ] README/whitepaper/UI report the exact controlled/federated/optimistic/
      cryptographic trust class and never infer trust from a configurable label.

No external bridge or EVM mint/return claim is publication-ready until these
gates are closed or the full affected surface is removed from the candidate.
