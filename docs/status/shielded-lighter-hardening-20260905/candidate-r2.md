# Lighter self-custody funding extension

**Date:** 2026-09-05  
**Status:** Draft; research lock pending  
**Task Node:** `task_1b81da68342923f814770ea149d3f048`

## 1. Purpose, scope, and inheritance

This extension serves one objective: fund a fresh Lighter account from an existing Ethereum self-custody wallet without a CEX, while making an explicitly measured public-observer funding-privacy claim.

It extends, but does not edit or replace, the locked [Hyperliquid specification](shielded-perp-venue-funding-privacy-hardening-20260905.md) and preserves that specification’s accepted implementation task. Unless this document states a venue-specific difference, it inherits the parent specification’s:

- private-note boundary;
- separation of operator cover from independent ownership;
- consent and local-authorization requirements;
- holding and release rules;
- durable operation IDs and recovery semantics;
- authenticated-history and evidence requirements;
- privacy-model definitions and policy hashing;
- prohibition on claiming production privacy before funded qualification.

This document is not research-locked, does not claim implementation, and grants no authority to move funds.

### 1.1 Exact completion boundary

The funding milestone ends when exactly 1 ETH is credited to the intended fresh Lighter account’s **spot** balance through the specified route.

It does not:

- enable ETH margin or unified trading;
- establish that spot ETH is already perps collateral;
- place a trade;
- independently verify Lighter L2 state;
- prove end-to-end privacy merely because the deposit succeeded;
- introduce a CEX, an Ethereum-to-Arbitrum bridge, a USD swap, or a testnet route.

Enabling ETH margin or unified trading is a later, explicit user action through the existing Lighter SDK helper. It may require local API-key creation and account settings. Source and destination identities must remain separated when API access is enabled.

## 2. Glossary

| Term | Meaning |
|---|---|
| `T` | Existing user-controlled Ethereum source wallet. |
| `T'` | Authenticated public PFTL recipient used for the public source-series mint before Orchard ingress. |
| `E_i` | Fresh Ethereum EOA credited by Lighter. Its key remains on user-controlled hardware. |
| `P_i` | Fresh public PFTL destination associated with `E_i`; activated by the common PFTL sponsor before egress can deliver to it. |
| PFTL | The public Post Fiat L1 inherited from the parent specification. |
| PFT | Native PFTL fee or activation asset. |
| pfETH | Authenticated public issued asset outside Orchard representing the deposited WETH and retaining its exact source-series identity. |
| Asset-Orchard | The encrypted 1-ETH note ingress and private-egress mechanism inherited from the parent specification. |
| Source series | The exact public asset ID and source bucket created by a particular vault-backed issuance route. Distinct source-series IDs remain distinct inference candidates. |
| Public-observer claim | Privacy result computed without private operator ownership and assignment maps. |
| Operator-aware claim | Privacy result computed for a named operator assumed to know its own cover ownership and assignments. |
| `q_max` | Maximum modeled posterior probability assigned to any plausible destination cluster, as defined by the inherited privacy policy. |
| Common sponsors | The common PFTL sponsor and common Ethereum sponsor are separate keys with separate nonce ledgers on their respective chains. The PFTL sponsor performs PFTL activation; the Ethereum sponsor submits the bounded Ethereum type-4 transaction. |
| Funding completion | Finalized Ethereum execution plus verified Lighter spot credit for the same transaction and exact amount. |
| Funded privacy qualification | Separate evidence that a real user-funded route satisfies the selected privacy claim and all inherited evidence gates. |

## 3. Numeric provenance

The fixed 1-ETH denomination, `10^18` base-unit call values, Ethereum chain ID 1, two-call batch, and nonce-zero compartment are route scope or transaction invariants, not newly selected privacy policy.

The following numerical policies are inherited unchanged from the locked Hyperliquid specification:

- authorization deadline no more than 900 seconds after signing;
- minimum shielding hold of six hours;
- hourly release windows as an operational convention, not an assumption of atomic cohort settlement;
- at least eight plausible destination clusters for a qualifying selected claim;
- `q_max <= 1/8` in every applicable inference model, conditioned on `T` completing an exit.

Any change to an inherited numerical policy belongs in a separately versioned parent-policy change. This extension must not silently alter the locked Hyperliquid policy hash.

## 4. Route

| Step | Stage | Required action and evidence boundary |
|---:|---|---|
| 1 | Ethereum source | `T` deposits exactly 1 WETH into the pinned shared WETH vault. Native fees are separate from principal. |
| 2 | Public source-series issuance | Authenticated public mint of the exact pfETH source-series asset to public PFTL recipient `T'` outside Orchard. |
| 3 | Private ingress | Encrypted 1-ETH Asset-Orchard ingress using that exact source series. |
| 4 | Hold and history | Enforce the inherited holding policy and import authenticated history before release analysis. |
| 5 | Activation | Activate `P_i` through the common PFTL sponsor under its PFTL nonce ledger, and record the public activation data. Egress cannot deliver to `P_i` before activation. |
| 6 | Privacy gate | After activation, run local proof checks and the versioned public-observer and operator-aware inference models, including the public activation data. |
| 7 | PFTL egress | Export the exact public exit proof to the already activated fresh `P_i`. This is the first irreversible privacy boundary. |
| 8 | Burn and release | Burn the exact held source-series asset and release exactly 1 WETH, proof-bound, to fresh Ethereum EOA `E_i`. |
| 9 | Ethereum batch | Use the reviewed `ExitExecutorV1` builder’s chain-1 EIP-7702 authorization and EIP-712 batch to unwrap WETH and submit the Lighter deposit atomically under common Ethereum sponsorship. |
| 10 | Reconciliation | Verify the canonical finalized Ethereum transaction and actual Lighter deposit credit. |
| 11 | Completion | Mark `complete` only after both execution and venue-credit gates pass. |

The venue-specific state path after `weth_released` is:

`venue_deposit_pending` → `complete`

The implementation must not manufacture an `arbitrum_funded` event or state for Lighter. Before `weth_released`, it must reuse the parent specification’s shared durable stages and exact operation IDs.

## 5. Lighter-specific integration

Lighter’s Ethereum deposit accepts an explicit credited L1 address. Unlike the Hyperliquid tail, this route requires no Ethereum-to-Arbitrum bridge and no USD swap.

The intended reviewed batch is:

1. `WETH.withdraw(10^18)`
2. `deposit(E_i, 1, 1, 10^18)` with transaction value `10^18`

The second `1` is intended to select the spot route; non-USDC assets must not be sent directly to the perps route. All argument semantics, asset eligibility, amount units, and current metadata must be checked before admission. Builder constants are not evidence of live eligibility.

The [official deposit documentation](https://apidocs.lighter.xyz/docs/deposits-transfers-and-withdrawals) is the source for recipient, asset, route, and base-unit behavior. It currently states the mainnet contract as `0x3B4D794a66304F130a4Db8F2551B0070dfCf5ca7`; this document does not claim that address, runtime, proxy structure, implementation, ABI, event layout, or metadata has been independently verified.

## 6. Threat model

### 6.1 Protected secrets and linkages

The system must protect:

- the private key for `E_i`;
- the source wallet and source-linked credentials;
- private notes, witnesses, ownership maps, and source-to-destination assignments;
- local API keys and authenticated venue sessions;
- relay metadata that could join `T`, `T'`, `P_i`, and `E_i`.

The common PFTL sponsor may receive the public activation material required on PFTL, and the separate common Ethereum sponsor may receive public exit signatures and the bounded transaction material required for Ethereum sponsorship. Neither sponsor may receive `E_i`’s private key, the source wallet, a private note, a witness, or the private ownership map.

### 6.2 Adversaries and trust boundaries

The analysis includes:

1. **Public observer:** sees public Ethereum, PFTL activity, activation, timing, source-series IDs, destination behavior, and publicly known clustering.
2. **Named operator:** additionally knows its own cover ownership and assignments. Operator-owned cover cannot protect a user from that same fully informed operator.
3. **Sponsors:** the separate common PFTL and Ethereum sponsors may delay, censor, replay, or mishandle a response, but each must be constrained by exact signed payloads, deadlines, its chain-specific key and nonce ledger, and durable reconciliation.
4. **Network or relay observer:** may correlate sessions, credentials, timing, or fallback transports.
5. **Upgradeable-contract or governance actor:** may change proxy implementations, beacons, runtime behavior, or admissible metadata.
6. **Venue/API operator:** may delay or misreport credit history. The authenticated API remains a disclosed trust boundary unless Lighter L2 state is independently reconstructed and verified.
7. **Public-transaction observer or front-runner:** sees the public Ethereum tail. This specification does not claim concealment of that tail or resistance beyond exact authorization and transaction binding.
8. **Local failure or crash:** may occur after signing, delegation, nonce consumption, broadcast, execution, or venue acceptance.

### 6.3 Explicit non-claims

The design does not claim:

- protection from an operator that knows all relevant cover assignments unless the operator-aware models independently pass;
- that operator sockpuppets are independently owned;
- that elapsed time creates anonymity;
- that simulation predicts execution or venue credit;
- that an HTTP success response, account creation, or JSON coverage flag is production evidence;
- that unknown private swaps or issuance can be ignored without conservative proof;
- that later disclosures or trading behavior cannot invalidate a prior inference result.

## 7. Numbered, testable gates

A gate failure blocks the stated transition. “Pending” is not success and must not trigger repayment or a duplicate deposit.

### G1. Admission, consent, and capital authorization

Before admitting a user:

- obtain the inherited user consent and local authorization;
- reserve cover principal, sponsor ETH, PFT, proof costs, concurrent capacity, and the venue holding horizon;
- keep user principal separate from cover and fees;
- stop new admissions when any required capacity is depleted;
- prohibit cover recycling during the pilot, dust sweeping, venue-account consolidation, and spending user principal on cover.

**Pass evidence:** durable reservation records, policy hashes, consent, and local authorization bound to the exact route.  
**Failure result:** admission blocked.

### G2. Chain, contract, proof-route, and upgrade pins

Before any spend, pin:

- Ethereum chain ID;
- finalized block number and hash;
- shared vault and proof route;
- WETH;
- executor;
- Lighter proxy runtime;
- applicable implementation or beacon slots;
- implementation bytecode.

Record governance and upgrade trust separately. Recheck all pins at pending and finalized state. A changed implementation halts new admissions and requires review.

**Pass evidence:** content-addressed pin set and successful rechecks.  
**Failure result:** admission or progression blocked; no silent repinning.

### G3. Asset metadata and batch semantics

Before signing, verify current deployed behavior and live metadata for:

- the Lighter recipient field;
- the intended ETH asset identifier;
- the spot route selector;
- base-unit conversion;
- deposit eligibility;
- payable value;
- the exact two-call batch.

Do not infer eligibility or semantics solely from builder constants.

**Pass evidence:** recorded metadata and deployed-code/ABI review tied to the pin set.  
**Failure result:** signing blocked.

### G4. Fresh `E_i` compartment

Immediately before signing, `E_i` must have:

- exactly 1 WETH;
- zero native ETH;
- no code or active delegation;
- account nonce zero.

The exit key must remain on user-controlled hardware.

**Pass evidence:** state reads at the pinned block and a matching pre-sign recheck.  
**Failure result:** signing blocked; use a newly authorized compartment rather than silently changing the destination.

### G5. Exact authorization and sponsorship

Bind the chain-1 authorization and EIP-712 batch to:

- executor;
- `E_i`;
- executor nonce zero;
- both calls and their values;
- the exact amount and destination;
- an inherited deadline no more than 900 seconds away.

The common Ethereum sponsor signs only its own bounded type-4 transaction under a shared durable Ethereum nonce lock. Its key and nonce ledger must be separate from the common PFTL sponsor’s key and PFTL nonce ledger.

**Pass evidence:** decoded signed payload, deadline, Ethereum sponsor nonce-lock record, separation from the PFTL sponsor key and nonce ledger, and local secret scan.  
**Failure result:** broadcast blocked.

### G6. Exact transaction simulation

Simulate the exact sponsored transaction against the pinned state and retain the result.

A successful simulation establishes neither future execution nor Lighter credit.

**Pass evidence:** simulation input hash matching the signed transaction and recorded output.  
**Failure result:** broadcast blocked.

### G7. History completeness and timing evidence

Replay the native public snapshot while preserving chain, genesis, and checkpoint pins. Index the base asset and every registered source series. Each record must retain:

- exact asset and source bucket;
- amount;
- commitment position;
- relevant transaction and content hashes.

Never remove old notes or failed attempts to improve a score.

PFTL headers have no wall-clock field. Collect contemporaneous observations with uncertainty intervals. Do not fabricate historical timestamps from current observation time.

**Pass evidence:** complete, content-hashed coverage for the selected cohort and timing intervals sufficient for the exact timing model.  
**Failure result:** privacy gate fails or remains inconclusive.

### G8. Complete route linkage

The importer must link, where applicable:

- Ethereum vault deposits;
- PFTL mints and ingresses;
- commitment membership;
- egress nullifiers;
- burns and releases;
- sponsor operations;
- finalized Ethereum deposit execution;
- Lighter credits.

Coverage boundaries and content hashes are mandatory. Unknown private swaps or issuance remain inconclusive unless conservative analysis proves they cannot affect the selected source series or cohort. A JSON coverage flag alone is not evidence.

**Pass evidence:** reconciled linkage graph with explicit coverage boundaries.  
**Failure result:** completion or privacy qualification blocked.

### G9. Venue-conditioned privacy analysis

After `P_i` has been activated and immediately before exporting the public PFTL exit proof, obtain fresh authenticated history and run the same four precisely versioned inherited inference models. The gate must include the confirmed public activation, its common PFTL sponsor, timing, key-visible data, and other public activation data in every applicable model:

1. uniform;
2. timing;
3. ownership concentration;
4. cover/venue behavior.

Run and display each model under both observer views:

- **Public-observer claim:** every one of the four public-observer models must meet the inherited threshold.
- **Named-operator-aware claim:** every one of the four operator-aware models must independently meet the inherited threshold. Operator-owned alternatives known to that operator do not count as unknown destinations.

The selected claim and policy hashes must be bound to the exact proof payload. A public-observer pass must never be represented as an operator-aware pass.

**Pass evidence:** confirmed prior activation of `P_i`, versioned model inputs including public activation data, outputs, claim selection, policy hashes, and proof-payload binding.  
**Failure result:** proof export blocked for users. An explicit no-privacy operator label may be used only for operator cover bootstrap.

### G10. Lighter cohort and cover quality

Privacy analysis must use a separate Lighter cohort. If an observer knows `T` is funding Lighter, Hyperliquid-only destinations must be excluded. Eight destinations distributed across two venues do not constitute eight plausible Lighter destinations.

The model must include:

- publicly known destination clustering within the Lighter cohort;
- public requests or previously visible Lighter intent;
- venue-choice uncertainty as an explicit versioned model input;
- exact source-series distinctions;
- public activation by the common PFTL sponsor, including its separate PFTL key and nonce-ledger effects;
- later visible venue behavior.

Cover must consist of actual operator-funded 1-ETH notes using the same vault, source series, proof, activation, common PFTL sponsor, separate common Ethereum sponsor, Lighter deposit, and venue behavior as users. Schedules must be finite, cryptographically randomized independently of user arrivals, and committed before admission.

For the selected claim, every applicable model must have at least eight plausible destination clusters and `q_max <= 1/8`, conditioned on `T` completing an exit. These thresholds are inherited from the locked parent specification.

One user plus seven operator notes is only a theoretical minimum. Timing, source series, destination behavior, ownership knowledge, or later disclosures may require more inventory or cause failure. Cover-only traffic cannot create a user privacy result.

**Pass evidence:** committed schedule, real cover provenance, cluster analysis, and threshold results.  
**Failure result:** user proof export blocked.

### G11. Irreversible egress handling

The exact public exit on PFTL is the first irreversible privacy boundary.

Before first export, any change to the anchor, destination, amount, or privacy policy requires new local authorization. Once egress is public, complete its already authorized public tail even if later privacy analysis worsens.

**Pass evidence:** export record matching the authorized proof payload.  
**Failure result:** before export, block and reauthorize; after export, reconcile and finish only the authorized tail.

### G12. Durable broadcast and no-duplicate rule

Persist signed bytes before broadcasting. Reconcile the exact:

- proof and nullifier;
- transaction bytes and hash;
- PFTL activation sponsor key and nonce;
- Ethereum sponsor key and transaction nonce;
- executor and account nonces;
- receipt.

A timeout or lost response never authorizes a second deposit.

**Pass evidence:** durable signed-byte record and exact reconciliation state.  
**Failure result:** remain pending and investigate the original operation.

### G13. Finalized Ethereum execution

Completion requires a canonical finalized Ethereum receipt matching the signed transaction and all of the following:

- `BatchExecuted(E_i, 0, 2)`;
- the exact WETH withdrawal;
- the deployed Lighter contract’s actual deposit event fields for `E_i`, ETH, spot route, and amount.

The event ABI and layout must be verified against deployed code. Do not invent an event signature.

**Pass evidence:** canonical receipt, signed-transaction match, decoded events using the verified deployed ABI, and finalized pin recheck.  
**Failure result:** `venue_deposit_pending`, failed, or recovery state as appropriate; never `complete`.

### G14. Actual Lighter credit

Ethereum execution alone is insufficient. Require actual deposit credit from the intended Lighter account.

Use the official SDK deposit-history schema as the adapter contract:

- `id`;
- `amount`;
- `timestamp`;
- `status`;
- `l1_tx_hash`;
- `asset_id`.

The schema source is [Lighter’s deposit history item](https://github.com/elliottech/lighter-python/blob/main/lighter/models/deposit_history_item.py).

Require:

- completed status;
- exact finalized L1 transaction hash;
- exact ETH amount;
- expected asset ID;
- returned account whose L1 address is `E_i`.

Read complete pages, reject repeated cursors and duplicate identities, and verify amount units against live asset metadata and a recorded deposit. Use integer or exact-decimal arithmetic.

Unavailable or ambiguous authenticated history remains pending verification and is never a reason to repay or submit another deposit. The venue API remains a disclosed trust boundary unless the L2 state is independently reconstructed and verified; this extension makes no such claim.

**Pass evidence:** complete authenticated history response satisfying every field and pagination check.  
**Failure result:** remain `venue_deposit_pending`.

### G15. Completion and conservation

Mark `complete` only after G13 and G14 both pass and integer conservation confirms:

- exactly 1 WETH was released to `E_i`;
- exactly 1 WETH was unwrapped;
- exactly 1 ETH principal was submitted to Lighter;
- native fees were accounted for separately;
- no duplicate principal movement occurred.

The credited result must be labeled **ETH spot balance**, not enabled perps collateral.

**Pass evidence:** reconciled conservation ledger, finalized receipt, and authenticated completed credit.  
**Failure result:** no completion claim.

## 8. Privacy maintenance after release

Later evidence can change an inference result. The importer and model runner must incorporate:

- leaked cover ownership or assignment maps;
- visible trading differences;
- destination clustering;
- venue-account consolidation;
- source-series distinctions;
- changed venue behavior.

These updates must affect both public-observer and named-operator-aware results as applicable and may invalidate a prior inference. The system must not infer a pass from elapsed time or preserve a stale score by deleting history.

Private operator maps remain local and separate. They must never be exported with relay jobs. Network and venue sessions must not reuse source-linked credentials or silently fall back from the selected privacy transport.

## 9. Failure and recovery

### 9.1 Reverted or partially applied EIP-7702 execution

Ethereum delegation may persist even if the batch fails. A failure may also consume account or executor nonces.

On revert, preserve:

- the canonical receipt;
- actual fee paid;
- EOA delegation state;
- executor and account nonces;
- retained WETH and ETH balances.

A new signature requires explicit, bounded recovery authorization for the same compartment. Do not assume the original preconditions still hold.

### 9.2 Delayed or ambiguous venue credit

Continue reconciling the original deposit. Do not:

- issue a second deposit;
- repay based on timeout;
- mark completion from an HTTP response;
- treat account creation as credit.

### 9.3 Withdrawal recovery

Lighter withdrawal helpers may return funds to `E_i` through Lighter’s secure withdrawal path, subject to venue availability. Before exposing an executable recovery action, verify the withdrawal route and final-credit behavior.

Never withdraw automatically and never automatically return funds to `T`. If the user explicitly chooses `T`, disclose the resulting linkage before authorization. A return is operational recovery, not a new privacy claim.

## 10. Implementation and interface requirements

The accepted extension task governs research locking, a concise milestone, a Python CLI, and then its working local interface. This document does not claim those artifacts exist.

The CLI must execute the real configured route. Any unavailable stage must return a specific pending or blocked state. Interface buttons must invoke the same controller rather than print manual commands.

The CLI and interface must show:

- route and current stage;
- credited account and denomination;
- fees and ETH exposure;
- selected privacy claim;
- all four model results under both observer views;
- sponsor and cover capacity;
- available recovery choices.

Private source maps, keys, notes, witnesses, ownership assignments, and source-linked credentials must not appear in browser responses, logs, telemetry, or sponsor payloads.

## 11. Required test and evidence matrix

| Area | Required acceptance evidence |
|---|---|
| Native route conformance | Exact ingress, private egress, burn, release, and source-series conservation. |
| History | Series-aware completeness, retained failed attempts, missing-evidence refusal, and timing-uncertainty handling. |
| Privacy | Venue-conditioned Lighter fixtures; all four models under public and operator-aware views; explicit proof that operator-known cover is excluded from operator-aware alternatives. |
| Sponsorship | Separate common PFTL and Ethereum sponsor keys and chain-specific nonce ledgers; per-chain shared nonce concurrency, lost responses, restart recovery, bounded deadline, and exact signed-byte reconciliation. |
| Ethereum tail | Exact transaction simulation, finalized receipt, verified deployed event layout, WETH withdrawal, and `BatchExecuted(E_i, 0, 2)`. |
| Lighter credit | Complete pagination, duplicate and cursor rejection, exact hash, account, asset, amount, units, and completed status. |
| Recovery | Reverted EIP-7702 execution, persistent delegation, nonce changes, retained-balance accounting, and no duplicate deposit after timeout. |
| Conservation | Integer accounting for principal, cover, sponsor ETH, PFT, proof costs, and actual fees. |
| Secret handling | CLI and interface scans covering responses, logs, telemetry, relay jobs, and sponsor requests. |
| Regression | Focused tests proving this venue path does not manufacture Hyperliquid or `arbitrum_funded` states. |

Available test routes must be validated rather than assumed. This specification does not posit a full testnet vault or add a testnet route.

Evidence classes remain distinct:

- local real proofs establish primitive conformance only;
- mocked receipts are simulation only;
- a prefunded live deposit tail proves only that tail;
- none substitutes for a self-custody-to-venue funded run;
- none substitutes for real-history public-observer qualification;
- none substitutes for separate named-operator-aware qualification.

## 12. Implementation, activation, and qualification gates and residual trust

The repository may lock the research design before implementation. Research lock does not verify or close the following implementation, activation, mainnet-admission, or funded-qualification gates. Record, review, and satisfy each item before the implementation stage or live action to which it applies:

1. current Lighter asset metadata and exact amount units;
2. deployed proxy/runtime, implementation or beacon structure, bytecode, and governance trust;
3. exact deposit-call argument semantics;
4. actual deployed deposit-event ABI and fields;
5. official SDK pagination and account-binding behavior;
6. behavior of the later margin or unified-trading helper without conflating it with funding;
7. available real test routes, without assuming a testnet vault;
8. versioned venue-choice conditioning for the inherited privacy models;
9. cover-capacity and holding-horizon sizing for the proposed cohort;
10. recovery behavior for persistent delegation, nonce consumption, delayed credit, and secure withdrawals.

These remain explicit implementation, activation, mainnet-admission, or funded-qualification verification gates after research lock, not claims about deployed behavior. Research lock records the design and does not represent satisfaction or closure of any of these gates.

## 13. Mainnet authorization and release evidence

Before any mainnet execution, prepare a concrete memo naming:

- isolated wallet addresses;
- source principal;
- cover principal;
- fee ceilings per chain and operation;
- exact contract, implementation, proof, metadata, and policy pins with hashes;
- backup and recovery paths;
- proposed commands and capital actions.

Obtain explicit approval for the specified capital and actions. This design note grants no fund movement.

A small approved mechanics probe cannot qualify the 1-ETH privacy cohort. Keep the following evidence tracks separately open until each independently passes its gates:

1. end-to-end self-custody-to-Lighter execution;
2. actual Lighter spot-credit reconciliation;
3. real-history public-observer privacy qualification;
4. named-operator-aware privacy qualification.

No track may borrow the claim of another, and none is claimed complete by this draft.
