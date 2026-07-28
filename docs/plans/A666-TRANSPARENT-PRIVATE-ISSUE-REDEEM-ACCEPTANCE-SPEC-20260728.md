# A666 Transparent and Private Issue/Redeem Acceptance Spec

**Date:** 2026-07-28

**Priority:** P0

**Status:** execution specification

**Governing economics:** `A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md`

**Sequence:** transparent issue debug → transparent issue verification →
private issue attempt/fix → transparent redeem attempt/fix → private redeem
attempt/fix

**Execution status (2026-07-28):** A0, A1, A2, and the A3 attempt gate PASS.
The fresh Phase 2
mainnet run completed hands-off in `1,464 seconds` with exact conservation,
six-validator convergence, unchanged Uniswap liquidity, and PFTL/Ethereum
replay rejection. Evidence:
`../evidence/a666-acceptance-20260728/phase-2b-transparent-issue-slo-verify/README.md`.
Phase 3 then stopped without mutation at
`UNSUPPORTED_PRIVATE_PRIMARY_ISSUE`: the deployed wallet/RPC/consensus action
surface has no private-primary issue action, and the existing private swap is
conservation-only. Evidence:
`../evidence/a666-acceptance-20260728/phase-3-private-issue-attempt/README.md`.
A4 through A8 remain pending.

## 1. Objective

Prove, in increasing order of complexity, that a user can:

1. deposit Ethereum-mainnet USDC;
2. create new A666 supply at the posted primary issue price;
3. hold the newly issued A666 on PFTL;
4. export it and receive wA666 on Ethereum;
5. return wA666 to PFTL;
6. retire A666 through primary redemption at the posted redemption price; and
7. receive Ethereum-mainnet USDC back.

The transparent route MUST be made repeatable before privacy is added. The
private route MUST reuse the same economic, reserve, supply, proof, and replay
rules; privacy MUST NOT create an alternative mint, redemption reserve, or
operator-authorized settlement path.

This document defines campaign order and acceptance. It does not change:

- the `1.005 × NAV` issue price;
- the `0.9995 × NAV` redemption price;
- the absence of a permanent A666 maximum supply;
- policy/route/order/packet capacity limits;
- PFTL as the canonical NAV and supply ledger;
- `TRUSTLESS_FINALITY` for PFTL-to-Ethereum export; or
- the disclosed `BFT_CHECKPOINT` trust boundary for Ethereum-to-PFTL return.

If this document conflicts with the governing economic specification, the
economic specification controls.

## 2. Current baseline

The 2026-07-28 transparent mainnet campaign functionally passed:

```text
100.500000 Ethereum USDC
  -> 100.500000 PFTL pfUSDC
  -> 100.000000 newly issued A666
  -> 100.000000 Ethereum wA666
```

Evidence is in
`../evidence/a666-joe-mainnet-e2e-20260728/README.md`.

That run is a debug baseline, not the clean verification required by this
specification:

- it took `42.2 minutes`, exceeding the `25-minute` SLO;
- it required manual orchestration;
- an old private pfUSDC note had to be reconciled before the deposit claim;
- the generic export snapshot path failed and a direct witness was built; and
- prover/relay configuration defects were repaired during the campaign.

The validator release correction occurred before the measured deposit and did
not contribute to the `42.2-minute` wall time. It nevertheless proves that
binary/config parity must be a hard pre-deposit gate.

Asset-Orchard has real Halo2 proof verification, private notes, nullifiers,
encrypted outputs, a pricing-bound private swap, a private-send construction,
and proof-based private egress. The current wallet configuration is still
centered on legacy a651/a652 private NAV swap pairs. A production A666 primary
issue or primary redemption funded directly from private notes has not been
accepted.

Private egress hides the consumed note opening. It does not make the public
exit asset, amount, destination, later Ethereum transaction, or timing
invisible. The specification uses “private” only for the protected PFTL
middle, with boundary disclosure stated explicitly.

## 3. Mandatory execution order

The phases MUST execute in this order:

| Phase | Name | Purpose |
|---:|---|---|
| 1 | Transparent issue debug | Repeat the proven issue/export route and expose every remaining failure. |
| 2 | Transparent issue verification | Execute the frozen route cleanly, hands-off, and within the SLO. |
| 3 | First private issue attempt | Attempt the same primary issue with a protected PFTL middle and record the first real unsupported or failing boundary. |
| 4 | Private issue repair and verification | Fix all observed private-issue defects, then pass a fresh clean run. |
| 5 | First transparent redemption attempt | Burn/return wA666, redeem A666 for pfUSDC, and withdraw USDC. |
| 6 | Transparent redemption repair and verification | Fix every observed redemption defect, then pass a fresh clean run. |
| 7 | First private redemption attempt | Attempt return and primary redemption with a protected PFTL middle. |
| 8 | Private redemption repair and verification | Fix every observed private-redemption defect, then pass a fresh clean run. |

No phase may be marked complete merely because its first attempt produced a
useful failure. Attempt phases produce a defect ledger. Verification phases
produce clean acceptance evidence.

Phases 3 and 7 MUST NOT be described as end-to-end private. Ethereum deposit,
Ethereum burn/mint/withdrawal, public supply changes, and the transitions into
and out of the shielded pool remain observable.

## 4. Campaign controls

### 4.1 Frozen manifest

Before each live-value attempt, publish a content-addressed campaign manifest
containing:

- Git commit and reproducible binary hash;
- the binary and topology hash running on every validator;
- chain/genesis/protocol identifiers;
- pfUSDC asset, vault, verifier, source token, policy, and route identifiers;
- A666 asset, primary policy, NAV epoch, reserve packet, route, and limits;
- wA666 token, verifier, controller, Uniswap pool, and runtime code hashes;
- SP1 program vkey, ELF hash, proof schemas, and prover image hash;
- Asset-Orchard pool, circuit, parameter, proving-key, and verifying-key
  hashes for private phases;
- user PFTL and Ethereum destinations;
- exact maximum USDC principal and maximum gas/prover budget;
- expected issue or redemption arithmetic;
- privacy claim and boundary-disclosure table;
- per-stage timeout and recovery action; and
- evidence output directory.

The manifest MUST fail closed if any live readback differs. No identifier may
be copied from a previous report without a fresh chain/fleet read.

### 4.2 Release and service readiness

Before user funds move:

- all validators MUST report the same release, topology, finalized height, and
  state root;
- required source-proof kinds MUST be enumerated by the running binary;
- the ingress and export provers MUST be warm and report CUDA readiness;
- no validator rollout, route migration, or binary replacement may be pending;
- relayer binaries, RPC endpoints, key-file references, and remote work
  directories MUST be staged and read-only checked;
- the generic finalized-witness exporter MUST successfully dry-run against
  current live history;
- the durable workflow journal MUST survive a forced restart;
- the test wallet MUST have no unexpected transparent or private notes; and
- global pfUSDC/A666 supply checks MUST include transparent balances,
  Asset-Orchard custody, pending deposits, burns, releases, bridge claims,
  reservations, and refunds.

A failure here blocks deposit. It is not repaired after taking user funds.

### 4.3 Live-value bound

Every first attempt uses the smallest meaningful authorized amount. Principal
is frozen in the campaign manifest and MUST NOT be increased by the operator.
A larger capacity run requires separate authorization.

Debug and verification runs MUST use distinct workflow IDs, nonces, deposit
IDs, reservations, packets, proof jobs, and user notes. A verification run
cannot reuse value transitions from its debug run.

### 4.4 Debug versus verification

A debug run MAY:

- stop at a safe recovery boundary;
- capture additional diagnostics;
- restart an idempotent service; and
- use a documented refund/return path.

A debug run MUST NOT:

- edit validator ledger files;
- manufacture or replace a receipt;
- bypass proof verification;
- use an owner mint;
- silently substitute legacy a651/a652;
- use operator inventory instead of primary issuance;
- reinterpret an old private note as new user funding; or
- declare PASS after manual state repair.

A verification run is stricter:

- no code, binary, config, route, or manifest change after deposit/burn;
- no manual proposal construction or one-off witness surgery;
- no manual custody reconciliation;
- no duplicate proof job after timeout;
- orchestration resumes only from its persisted workflow journal; and
- the run either completes or enters a documented, user-safe recovery state.

## 5. Phase 1 — Transparent issue debug

### 5.1 Flow

```text
Joe USDC on Ethereum
  -> proof-gated pfUSDC vault deposit
  -> finalized Ethereum ingress proof
  -> transparent pfUSDC credit on PFTL
  -> primary reservation at frozen NAV/policy
  -> transparent primary subscription
  -> new transparent A666 balance on PFTL
  -> entitlement-bound A666 export
  -> finalized PFTL receipt proof
  -> exact wA666 mint to Joe on Ethereum
```

The PFTL A666 balance MUST be read and recorded after subscription and before
export. A combined operator script that skips this observation does not satisfy
the phase.

### 5.2 Required fixes exercised by the repeat

The repeat MUST specifically prove:

- release parity is detected before deposit;
- CUDA/prover readiness is detected before finality;
- relay/key/cast staging needs no interactive repair;
- a clean wallet does not trigger an Asset-Orchard supply mismatch;
- the supply-cap calculation includes all shielded custody even if such
  custody exists elsewhere in the system;
- finalize/claim templates select the exact intended operations;
- the standard snapshot/witness path supports the current governance history;
- every PFTL transition is submitted through the durable workflow; and
- each stage emits monotonic timestamps and structured failure codes.

### 5.3 Debug exit

Phase 1 exits when:

- the full flow completes or safely recovers;
- every intervention and failure has a defect ID;
- each defect has root cause, affected invariant, fix location, regression
  test, and owner;
- no unexplained supply/custody delta remains; and
- a candidate release is frozen for Phase 2.

Completion within 25 minutes is measured but is not required for the debug
exit.

## 6. Phase 2 — Transparent issue verification

Phase 2 MUST start from a new deposit and the frozen candidate release.

### 6.1 Functional pass

For issue output `Q`, finalized pre-inflow NAV `N`, and issue multiplier
`10050/10000`:

```text
base reserve increase       = Q × N
user settlement debit       = priced by the canonical integer issue formula
non-NAV issue spread        = settlement debit - base reserve increase
A666 authorized supply      = previous supply + Q
PFTL user A666 after issue  = previous balance + Q
PFTL user A666 after export = pre-issue balance
outstanding bridge claims   = previous claims + Q
wA666 total supply          = previous wrapped supply + Q
Ethereum user wA666         = previous user balance + Q
```

All arithmetic uses canonical integer formulas and explicit rounding from the
governing specification. Floating point is forbidden.

### 6.2 Operational pass

- deposit inclusion to spendable wA666 MUST be no more than `25 minutes`;
- no human action occurs between deposit broadcast and terminal readback;
- every stage uses one persisted idempotency key;
- all six validators finalize identical heights and state roots;
- both proof packets verify under the frozen keys;
- the export packet is consumed exactly once;
- replay attempts reject without state mutation;
- Uniswap liquidity is not consumed by primary issuance; and
- the final wA666 is transferable and eligible for the live USDC/wA666 pool.

Phase 2 fails if the result is correct but requires an unplanned intervention.

## 7. Phase 3 — First private issue attempt

### 7.1 Target privacy shape

```text
public Ethereum USDC deposit
  -> public pfUSDC issuance boundary on PFTL
  -> public Asset-Orchard ingress
  -> private pfUSDC note
  -> private primary subscription at 1.005 × NAV
  -> private A666 note
  -> proof-based private egress to the export account
  -> public A666 export
  -> public Ethereum wA666 mint
```

The protected middle SHOULD hide raw PFTL asset IDs, note values, note owners,
recipient keys, and note linkage from the shielded action. The public
boundaries still reveal enough information to correlate a unique transaction,
especially when no other users share the block or amount.

### 7.2 First-attempt rule

The first attempt MUST use the production A666 and pfUSDC identifiers. It MUST
NOT substitute a651/a652 merely because the existing wallet route supports
those pairs.

Preflight is expected to determine whether the current system can:

1. consume private pfUSDC settlement;
2. increase public canonical A666 supply and reserve accounting;
3. create a private A666 output note;
4. allocate issue capacity exactly once; and
5. create an export entitlement bound to the eventual public exit account.

If the capability is absent, the phase stops before irreversible live value is
moved and records `UNSUPPORTED_PRIVATE_PRIMARY_ISSUE` with the exact rejected
operation/API. That is a valid first-attempt result, but not a PASS.

The existing private swap circuit cannot be treated as primary issuance merely
by configuring a pfUSDC/A666 pair. A conservation swap transfers existing
inventory; primary issuance changes reserve value and A666 supply. The private
action must prove or consensus-bind those primary-market effects.

### 7.3 First-attempt evidence

Record:

- the last successful state and first failing state;
- whether failure occurred in wallet, action construction, proof generation,
  RPC admission, consensus validation, accounting, note recovery, egress, or
  export;
- public fields emitted by every attempted shielded action;
- a redaction scan for note openings, spending keys, viewing keys, raw asset
  IDs, values, owners, and recipients;
- supply and custody before/after, including no mutation on failure; and
- warm/cold proof and consensus timing.

## 8. Phase 4 — Private issue repair and verification

Every Phase 3 defect MUST receive:

1. a minimal reproducible fixture;
2. a deterministic unit or conformance test;
3. a no-mutation rejection test;
4. a replay/restart test where applicable;
5. a bounded proof/input/resource test;
6. a mainnet-state fork rehearsal; and
7. a clean live rerun under a new workflow ID.

The implementation MAY introduce a versioned private-primary action, for
example `AssetOrchardPrimarySubscribeV1`, but its semantics MUST:

- bind chain, genesis, protocol, pool, A666 asset, pfUSDC asset, policy, NAV
  epoch, reserve packet, issue price, capacity, nonce, expiry, output note, and
  export entitlement;
- consume an unspent private pfUSDC note under a retained anchor;
- reject stale anchors and duplicate nullifiers;
- debit settlement and increase counted reserve value atomically;
- increase canonical A666 supply exactly once;
- create only the proven private A666 output value;
- allocate spread to the disclosed non-NAV spread account;
- create no transparent A666 user balance before private egress;
- preserve deterministic state roots across all validators; and
- expose only the minimum consensus data required for public supply and
  capacity accounting.

Phase 4 passes only after a new live-value run reaches spendable Ethereum
wA666 without disclosing note openings or private keys and without manual
intervention. Its evidence MUST state the exact residual boundary leakage.

The `25-minute` issue SLO remains binding. Privacy proof time is included; it
does not receive a separate user-visible latency exemption.

## 9. Phase 5 — First transparent redemption attempt

### 9.1 Flow

```text
Joe burns wA666 on Ethereum for PFTL return
  -> finalized Ethereum receipt and BFT_CHECKPOINT proof
  -> transparent A666 return credit on PFTL
  -> permissionless primary redemption at 0.9995 × NAV
  -> A666 retired and reserve principal released as pfUSDC
  -> pfUSDC exit/burn packet
  -> finalized PFTL proof accepted by the Ethereum vault
  -> exact mainnet USDC released to Joe
```

The attempt MUST use proof-minted wA666 from an accepted issue/export lineage.
It MUST NOT use migration-contract inventory or buy replacement inventory from
Uniswap.

### 9.2 Required properties

- Ethereum wA666 supply decreases by the returned amount;
- outstanding wrapped exposure decreases exactly once;
- the return event is imported exactly once;
- the user receives returned native A666 before redemption;
- primary redemption requires no issuer completion signature;
- A666 canonical supply decreases by the redeemed amount;
- counted NAV reserve principal decreases by the base redemption value;
- user pfUSDC output follows the `0.9995 × NAV` integer formula;
- redemption spread is posted to disclosed non-NAV accounting;
- pfUSDC burn and Ethereum vault release conserve supply/value;
- inbound issue/export pause does not strand return or redemption; and
- no Uniswap trade or LP withdrawal funds the redemption.

### 9.3 Attempt exit

The first attempt may fail. It exits only after:

- the user is either fully redeemed or in a proven recoverable state;
- every wA666 burn remains returnable;
- no returned A666, pfUSDC, or USDC is duplicated or lost;
- the first failing boundary and all interventions are recorded; and
- a complete transparent-redemption defect ledger exists.

## 10. Phase 6 — Transparent redemption repair and verification

Fix every Phase 5 defect and execute a new burn using the frozen release.

The clean run MUST be hands-off from Ethereum burn through Ethereum USDC
release. It MUST prove:

- deterministic return receipt verification and log inclusion;
- wrong chain/header/root/trie/log/event rejection;
- return, redemption, pfUSDC exit, and vault-release replay rejection;
- restart recovery after each broadcast/finalization boundary;
- exact pre/post supply and reserve arithmetic;
- no issuer discretionary signature;
- no mutation on expired/stale/underfunded policy rejection; and
- terminal Ethereum USDC is spendable by the intended recipient.

The campaign manifest MUST define a redemption latency budget before the burn.
Until a separate public redemption SLO is approved, the acceptance target is
`25 minutes` from finalized Ethereum burn inclusion to spendable Ethereum
USDC. Any exception is reported as an SLO failure, not hidden inside proof
time.

## 11. Phase 7 — First private redemption attempt

### 11.1 Target privacy shape

```text
public Ethereum wA666 burn
  -> public A666 return/import boundary on PFTL
  -> public Asset-Orchard ingress
  -> private A666 note
  -> private primary redemption at 0.9995 × NAV
  -> private pfUSDC note
  -> proof-based private egress to the pfUSDC exit account
  -> public pfUSDC bridge exit
  -> public Ethereum USDC release
```

The protected middle MUST NOT expose note openings or spending/viewing keys.
The Ethereum burn and release, PFTL turnstile boundaries, public A666 supply
reduction, public reserve reduction, and public pfUSDC/USDC exit remain
observable. A single isolated redemption can therefore remain amount/timing
linkable even when its internal note ownership and linkage are hidden.

### 11.2 First-attempt rule

The first private-redemption attempt MUST determine whether current code can:

1. shield returned production A666;
2. consume a private A666 note;
3. retire canonical A666 supply;
4. release subscription-funded reserve principal;
5. create a private pfUSDC note;
6. enforce redemption capacity, NAV, nonce, expiry, and price; and
7. privately egress the pfUSDC note without revealing its opening.

The legacy a651/a652 private swap and disclosed egress are forbidden
substitutes. If the required action is absent, stop safely and record
`UNSUPPORTED_PRIVATE_PRIMARY_REDEEM`.

## 12. Phase 8 — Private redemption repair and verification

The repair requirements from Phase 4 apply symmetrically. A versioned
private-primary redemption action, if introduced, MUST:

- bind the same chain/pool/policy/NAV/reserve domains as private issue;
- prove ownership and nullification of the private A666 input;
- retire exactly the proven A666 value;
- reduce counted reserve value atomically;
- create exactly the policy-priced private pfUSDC output;
- post the redemption spread to disclosed non-NAV accounting;
- consume redemption capacity and nonce exactly once;
- require no issuer completion signature;
- reject replay, stale anchor, stale policy, stale NAV, wrong asset, wrong
  recipient, insufficient reserve principal, and forged value conservation
  without mutation; and
- keep return/refund recovery available while new issue/export is paused.

Phase 8 passes only when a new live burn completes through spendable Ethereum
USDC with no manual intervention, no secret leakage, exact supply/reserve
conservation, and an explicit residual-leakage report.

## 13. Cross-phase invariants

Every accepted run MUST prove these identities from independent pre/post
readbacks.

### 13.1 pfUSDC

```text
Ethereum vault obligations
  = transparent spendable pfUSDC
  + Asset-Orchard pfUSDC custody
  + accepted deposits not yet issued
  + burned pfUSDC not yet released
  - released/settled redemption terms defined by the canonical rail
```

Equivalently, the canonical `V = S + D + B - R` identity treats `S` as issued
pfUSDC across transparent and shielded custody lanes. The exact production
identity and sign convention from the governing pfUSDC specification controls.
The campaign collector MUST include shielded custody; it may not infer supply
from transparent account balances alone.

### 13.2 A666

```text
authorized valid A666 supply
  = transparent spendable A666
  + Asset-Orchard A666 custody
  + outstanding bridge claims
  + other explicitly enumerated nonspendable canonical custody
```

Primary issue increases reserve principal and valid supply together. Primary
redemption decreases them together. Export/import only moves units between
custody lanes and MUST NOT change global valid supply.

### 13.3 Ethereum wrapper

```text
wA666 total supply = finalized outstanding PFTL bridge claims
```

Allow only explicitly modeled in-flight proof states. No owner/operator mint,
unproven burn import, or supply repair is permitted.

### 13.4 Privacy accounting

- each private input nullifier is unique;
- each output commitment is bound to the exact action;
- private value creation equals the consensus-authorized primary delta;
- turnstile ingress/egress and private custody reconcile;
- note scans recover intended outputs after restart;
- no rejected action changes an anchor, nullifier set, commitment tree,
  capacity, reserve, or supply; and
- deterministic replay yields identical state roots on every validator.

## 14. Required adversarial tests

Before a fix enters a verification release, test:

- duplicate deposit, claim, reservation, subscription, export, proof, mint,
  burn, return, redemption, egress, and withdrawal;
- wrong chain/genesis/protocol/route/asset/policy/NAV/reserve packet;
- one atom below/at/above balances and capacities;
- rounding boundaries for `1.005` and `0.9995`;
- expired reservations, policies, NAV proofs, anchors, and packets;
- forged or oversized SP1/Halo2 proofs and public values;
- duplicate/nullifier-reordered shielded actions;
- wrong encrypted output, recipient binding, asset tag, or value commitment;
- RPC disagreement and Ethereum reorganization before finality;
- proposer/validator/prover/relayer restart at every workflow state;
- one-validator outage and below-quorum no-advance behavior;
- snapshot/export/reimport with mixed transparent and shielded custody;
- cancellation versus consume races;
- issue and redemption competing in one block;
- privacy artifact redaction and wallet-secret rejection; and
- bounded CPU, memory, proof size, action count, retry count, and queue depth.

Any consensus or persistent-state fix additionally requires deterministic
replay, state-root comparison, fuzz/property coverage for the affected input,
and a rollback/recovery plan.

## 15. Timing and observability

Every run records monotonic durations for:

```text
Ethereum inclusion
Ethereum finality wait
ingress proof queue / prove / verify
PFTL propose / vote / certify / apply
Asset-Orchard prove / verify / certify
reservation
primary issue or redemption
export or return import
PFTL finality proof queue / prove / verify
Ethereum receipt acceptance
Ethereum mint, burn, or vault release
recovery and retry time
```

The summary MUST distinguish:

- protocol/security wait;
- proof compute;
- consensus time;
- Ethereum inclusion time;
- queued resource time; and
- operator/tooling failure time.

Pre-deposit preparation is reported separately from deposit-to-completion
latency. No pre-deposit rollout may be blamed for measured transaction wall
time.

## 16. Evidence package

Each phase writes a new immutable directory containing:

1. frozen manifest and its hash;
2. preflight and fleet convergence report;
3. pre-state balances, supplies, custody, capacity, NAV, and reserve state;
4. signed user intents with secret material redacted;
5. transaction, proof-job, consensus-round, and receipt identifiers;
6. proof/public-value hashes and verifier readbacks;
7. post-state and independently computed deltas;
8. replay/negative-test results;
9. timing trace and SLO verdict;
10. privacy public-field inventory and leakage statement;
11. secret/redaction scan;
12. defect ledger and intervention log; and
13. one machine-readable `PASS`, `FAIL`, or `RECOVERY_REQUIRED` summary.

Zero-byte reports, missing validator votes, unexplained state-root differences,
or absent pre-state data make the evidence incomplete.

## 17. Gates

| Gate | Pass condition |
|---|---|
| A0 — readiness | Frozen identifiers, release parity, warm provers, clean wallet, standard witness export, and complete accounting preflight pass before funds move. |
| A1 — transparent issue debug | Repeat completes or safely recovers; every failure/intervention has a defect and regression-test plan. |
| A2 — transparent issue verified | Fresh hands-off run creates and exports exact new supply within 25 minutes with replay/conservation checks. |
| A3 — private issue attempted | Production A666 private boundary is exercised or safely rejects as unsupported; no legacy substitution or unexplained mutation. |
| A4 — private issue verified | Fresh hands-off run creates private A666 through primary issuance and exports exact wA666 with explicit boundary leakage. |
| A5 — transparent redeem attempted | Proof-minted wA666 traverses burn, return, primary redemption, pfUSDC exit, and USDC release or a proven recovery state. |
| A6 — transparent redeem verified | Fresh hands-off round trip retires exact supply and releases exact reserve-funded USDC without issuer discretion. |
| A7 — private redeem attempted | Production A666 private-redemption boundary is exercised or safely rejects as unsupported. |
| A8 — private redeem verified | Fresh hands-off private-middle redemption returns exact Ethereum USDC with no secret leakage or accounting drift. |

Failure at a gate blocks later gates. Fixing a failed attempt does not satisfy
the corresponding verification gate; a fresh run is mandatory.

## 18. Definition of done

This program is complete only when:

- transparent issue/export passes cleanly and repeatedly;
- private-middle issue/export passes with accurate privacy labeling;
- transparent return/redemption/withdrawal passes cleanly;
- private-middle return/redemption/withdrawal passes with accurate privacy
  labeling;
- both issue modes meet the `25-minute` issue SLO;
- both redemption modes meet the frozen redemption SLO;
- supply, reserves, shielded custody, bridge claims, and Ethereum wrapper
  supply reconcile after every run;
- replays and malformed proofs fail without mutation;
- every workflow resumes safely after process failure;
- no step depends on an owner mint, issuer discretion, OTC inventory, or
  Uniswap liquidity; and
- the evidence index distinguishes functional PASS, privacy boundary, and SLO
  verdict without qualification hidden in prose.

Capacity and load campaigns follow this sequence. They do not precede the
single-user correctness gates.
