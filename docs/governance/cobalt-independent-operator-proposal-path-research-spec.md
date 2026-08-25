# Cobalt Independent-Operator Proposal Path Research Specification

**Status:** Locked on 2026-08-25 — decision: reinstate the independent-operator gate as its own mandatory milestone
**Date:** 2026-08-25
**Decision owner:** Post Fiat
**Prior work:** [Cobalt Activation Research Specification](cobalt-activate-or-retire-research-spec.md), [Cobalt Adversarial Verification Research Specification](cobalt-adversarial-verification-research-spec.md), [independent-operator onboarding contract](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/cobalt-independent-operators/onboarding-contract.json)
**Decision scope:** proposal origination, operator/key custody, trust topology, and the evidence required before Post Fiat may claim operator decentralization

## Locked decision

The independent-operator gate from the Cobalt activation specification is
**reinstated as a separate mandatory milestone**. It is not cancelled, treated
as simulation, or silently folded into adversarial protocol testing.

The milestone begins after the adversarial-verification milestone. It must move
the six controlled-testnet validator slots to six genuinely independent
operators and prove that an independent operator can originate a registry
proposal without a Foundation process constructing or rewriting it.

This decision does not claim that those operators exist today, does not recruit
them inside the adversarial campaign, and does not authorize a live migration.
It locks the design and the next acceptance boundary. Until the later milestone
passes, every public surface must say that Cobalt proves protocol capability on
a Foundation-administered controlled testnet, not operator decentralization.

Cobalt may remain the controlled-testnet validator-trust ratification authority
while this gate is completed. A missing independence gate is a process and
decentralization gap; it is not by itself one of the live safety stop conditions
that authorizes rollback to Foundation validator-trust authority. The gate is
mandatory before any mainnet recommendation, any claim of independent
operation, or any declaration that the original activation program is fully
compliant.

## Current path and custody boundary

Today the Foundation operator performs every proposal-origin action:

1. A Foundation-run process reads the current registry, trust root, authority
   lineage, and intended registry delta.
2. Production helpers construct the proposal and deterministically assign the
   first validator as proposer. In the current code,
   'propose_nonuniform_governance_amendment' selects the first trust view and
   'certify_validator_registry_update' selects 'validators[0]';
   'verify_validator_registry_update' requires that same identity.
3. Foundation-administered Cobalt sidecars sign the RBC, ABBA, MVBA, and DABC
   transcript with their validator-domain ML-DSA keys.
4. The same Foundation-administered validator fleet provides the scoped
   validator-update authorizations. A Cobalt-authorized update requires the
   decision certificate and at least the active quorum of authorizations.
5. Consensus v2 orders the resulting governance batch. Consensus v2 remains
   block finality; Cobalt remains limited to validator-registry and trust-graph
   ratification.

The proposal builder does not itself possess quorum, but the same
administrative boundary presently controls every proposer, protocol signer, and
authorization signer. The live chain therefore demonstrates a protocol
boundary, not an independent operator boundary.

The current key classes are kept distinct:

| Key or identity | Current custodian | Authority |
| --- | --- | --- |
| Operator/onboarding master key | Foundation for current validators; future operator for its own slot | Signs operator manifest and control attestations; never signs blocks or directly mutates registry state |
| Validator hot ML-DSA key | Foundation-managed validator host today | Signs Consensus v2 votes, Cobalt protocol messages, and scoped validator-update authorizations for one validator |
| Proposal process identity | Foundation process today | Constructs proposal bytes but has no independent ratification power |
| Snapshot publisher | Separate deployment role | Signs snapshots only; has no Cobalt or Consensus v2 vote |
| Repository/release signer | Release process | Identifies software lineage only; has no chain authority |

Private keys must remain on the operator-controlled host or its declared offline
custody boundary. A coordinator, relay, browser, model, or Foundation service
may carry public proposal bytes but may not sign for another operator.

## Required non-Foundation proposal path

### Proposal envelope

Add one canonical, versioned proposal envelope signed by an admitted operator's
onboarding master key and its active validator hot key. It binds at least:

- chain ID, genesis hash, and protocol version;
- current authority-transition ID, registry root, trust-graph root, and accepted
  Cobalt history sequence;
- proposal slot, activation height, and expiry height;
- operation, subject validator, old and new registry records, and old and new
  trust roots;
- canonical registry-update payload hash and evidence-packet root;
- proposer validator ID, operator-manifest hash, onboarding challenge ID, and
  both signature algorithms;
- a unique nonce derived from the previous accepted history ID and proposal
  slot.

Both signatures are admission checks, not ratification. A valid proposer cannot
commit its own proposal.

The envelope fails closed on an unregistered proposer, stale registry or
authority lineage, wrong chain, wrong root, reused nonce, expired slot, payload
mismatch, operator-manifest mismatch, missing signature, or non-canonical
encoding. Rejection must leave Cobalt and chain state unchanged and must produce
a named receipt.

### Transport and deterministic selection

Any currently admitted validator operator may submit an envelope to any Cobalt
sidecar over the bounded authenticated RPC. Relays are untrusted and may only
forward exact bytes.

The current implicit 'validators[0]' proposer rule must be replaced. For one
proposal slot, every correct sidecar:

1. verifies each envelope against the same registry and operator-manifest set;
2. deduplicates by envelope ID and payload hash;
3. orders valid candidates by the canonical envelope ID;
4. admits the candidate whose proposer is selected by a domain-separated hash
   of the previous accepted Cobalt history ID, slot, and active validator set;
5. records every valid but unselected proposal as deferred, not rejected.

If the scheduled proposer submits nothing before the bounded view timeout, the
same hash selects the next validator for the next view. No coordinator chooses
the winner, and a Foundation relay cannot alter candidate bytes or selection.

### Ratification and ordering

The selected payload enters the existing signed RBC -> ABBA -> MVBA -> DABC
path. Acceptance still requires strong support under every correct validator's
local trust view and the active Cobalt certificate rules. The resulting
validator update then requires the existing scoped current-registry
authorizations at quorum. Proposal signatures cannot be counted as protocol
contributions or authorizations.

The ordered governance record must retain the envelope ID, proposer validator,
operator-manifest hash, selected view, decision-certificate ID, and exact
authorization identities. Consensus v2 orders and finalizes that governance
batch. Neither the proposal service nor Cobalt gains block-finality authority.

## Independent trust topology

The controlled-testnet migration target is six validators, quorum five, with
exactly six independent operator groups:

- one validator maximum per operator, provider-account boundary, host
  administration boundary, and validator-key custody boundary;
- at least three infrastructure domains;
- each local trust view starts with all six admitted validators, threshold
  q = 5, and tolerated Byzantine count t = 1;
- the Cobalt inequalities hold for each row: 1 < 2(5)-6 and 2 < 5;
- one operator controls one vote, so one operator cannot reach quorum;
- removal or outage of any one operator leaves five validators, so one operator
  cannot block quorum alone;
- no Foundation account, funding boundary, administrator key, custody system, or
  legal-control domain may appear in more than one operator group.

Non-identical trust views may be introduced only after the all-six baseline
passes the same compatibility, one-outage, catch-up, and authority-transition
tests. A label or self-attestation is not independence. The retained
'postfiat-cobalt-independent-operator-onboarding-v2' contract remains the
admission boundary, refreshed to the then-current chain, release, registry, and
trust roots.

## Mandatory follow-on milestone

The reinstated milestone must not be marked complete until all of these gates
pass:

1. Six operator receipt sets verify under six distinct onboarding master keys,
   provider accounts, host administrators, custody boundaries, funding
   boundaries, and legal-control domains.
2. The live registry contains the six independently held hot keys and exactly
   matches the refreshed onboarding contract.
3. The proposal-envelope implementation has canonical hashing, dual-signature
   verification, replay/expiry checks, bounded transport, deterministic
   proposer/view selection, persistence, replay, and named fail-closed tests.
4. A disposable-clone migration and forward rollback pass before live changes.
5. One non-Foundation operator originates a live proposal while the Foundation
   proposal builder is offline; the exact envelope, selected proposer, Cobalt
   signers, scoped authorizers, and Consensus v2 receipt are recorded.
6. Every single-operator outage still permits five-of-six ratification and
   Consensus v2 finality; two-operator loss safely halts validator-trust changes.
7. Wrong-chain, stale-root, replayed, expired, payload-swapped,
   manifest-substituted, coordinator-rewritten, and self-authorized envelopes
   all reject without durable mutation.
8. Catch-up, one key rotation, one trust-view change, Foundation-to-independent
   custody transition, and the separately authorized forward rollback all
   preserve one accepted history.
9. A checksum-bound public packet, CLI, read-only browser view, redaction scan,
   and independent control review pass.

Recruitment and live migration require separate operational authorization.
This specification supplies neither.

## Failure and publication rules

If the later milestone fails, keep the public state explicit:
'FOUNDATION_ADMINISTERED'; do not claim operator decentralization. Repair the
owning boundary and rerun the unchanged admission and proposal corpus.

Rollback of live Cobalt authority remains governed by the adversarial
specification's observed safety/finality stop conditions, not by a missing
operator receipt or a failed design test alone.

Every public summary must keep these three statements together:

1. Cobalt ratifies validator-registry and trust-graph changes.
2. A separate policy and operator-admission layer decides which validators
   deserve trust.
3. Until the reinstated milestone passes, proposal origination and all live
   validator authorizations remain Foundation-administered.

## Locked acceptance statement

The independent-operator requirement is reinstated, not deferred. Its own
milestone must implement the proposal envelope and complete the six-operator
live migration. The current adversarial milestone may close E6 when this
specification and its checksum-bound decision packet verify, but that closure
does not satisfy the reinstated operator milestone itself.
