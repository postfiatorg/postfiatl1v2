# L1v2 Public Testnet: Genesis Registry and Launch Specification

**Status:** LOCKED — text-improvement-harness full gate 2026-08-30: gpt-5.6-sol-pro 88.00, Claude Fable 5 88.80, GLM 5.3 90.00, average 88.93 (threshold 86); first compliant score, locked per mandate  
**Date:** 2026-08-30  
**Implements:** the design surfaces of the [public-testnet path plan](../plans/active/l1v2-public-testnet-path-milestone.md), tracked as Task Node `task_510e7605cb2dff0dfd672b397d26f2a6`  
**Normative dependencies:** [whitepaper](../whitepaper.md) §6 (Cobalt registry transitions), [storage-scaling-fix-spec](storage-scaling-fix-spec.md) (storage qualification), `dynamic-unl-scoring/docs/DeterministicFinalScore.md` (deterministic scoring), and the [Dynamic UNL evidence-source note](../governance/dynamic-unl-l1-evidence-source-note.md)  
**Dependency locking:** before ratification, the launch bundle records the repository revision and SHA-256 digest of every normative dependency. Mutable paths above are review references only; an unpinned dependency fails genesis verification.

## 1. Purpose and scope

The public-testnet path plan has five phases:

- **Phase A — offline qualification:** prove the storage implementation against the qualification suite.
- **Phase B — rehearsal and deployment:** rehearse and deploy the qualified lineage.
- **Phase C — validator migration:** decide scoring authority (**C2**), construct the genesis registry (**C3**), and integrate the ratification client (**C4**).
- **Phase D — launch readiness:** satisfy the launch gates, including the public operations package (**D2**).
- **Phase E — public status interface:** expose gate state through one machine-readable source. **E6** names the Cobalt adversarial-verification experiment that concluded operator decentralization — a non-Foundation proposal path — requires its own milestone; that milestone is the recorded follow-on for removing the initial single-proposer dependency.

Phases A and B are specified by the storage documents. This specification defines:

1. how the fork’s scored operator community becomes l1v2’s genesis validator registry (C3);
2. how the validator sidecar becomes the l1v2 ratification client (C4);
3. the launch gates that make “public testnet” a checkable state (Phase D);
4. the status CLI and interface (Phase E); and
5. fail-closed handling of key loss, quorum degradation, and permanent registry-evolution halt.

Out of scope: mainnet, token migration or coexistence economics, changes to Consensus v2 finality or storage behavior, and Cobalt adoption claims beyond the controlled-devnet **KEEP_ACTIVE** boundary. `KEEP_ACTIVE` is the closing decision of the six-experiment Cobalt adversarial-verification campaign: Cobalt stays active on the controlled devnet in its bounded validator-registry and trust-graph ratification role. It is not evidence of operator decentralization or public-testnet readiness.

## 2. Actors, objects, and wire rules

| Term | Meaning |
| --- | --- |
| Fork / PFT Ledger | The live rippled-derived testnet with 51+ community validators, secp256k1 identities, and Dynamic UNL rounds using frozen evidence, deterministic selection, and sidecar-verified commit-reveal |
| Pipeline | The Dynamic UNL scoring pipeline. The C2 gate decides whether model output remains authoritative; after model authority is removed, authoritative scores are deterministic functions of frozen evidence |
| l1v2 | This repository’s chain using Consensus v2 finality, ML-DSA-65 authorization, qualified transactional storage, and Cobalt registry/trust-graph machinery in `crates/consensus_cobalt/` |
| `G_t` | The ordered validator registry active at transition index `t` |
| `T_t` | The ordered Cobalt trust graph associated with `G_t` |
| `χ_t` | The pinned transition checker version and executable digest |
| `π_t` | The versioned safety and evidence-limits profile |
| Ratification client | A validator sidecar extension that deterministically replays evidence, runs `χ_t`, and participates in commit-reveal; it has no discretionary approval input |
| Genesis payload | The unsigned object authenticated by genesis ratifiers |
| Genesis certificate | Detached ML-DSA signatures over the genesis payload hash |
| Genesis envelope | The launch artifact containing the payload, certificate, and artifact metadata; its hash is not part of the signed payload |

### 2.1 Canonical encoding and hashing

All consensus-adjacent objects defined here use versioned schemas and deterministic CBOR under RFC 8949 §4.2:

- map keys use the schema’s integer labels;
- arrays whose order is not semantically significant are sorted by the bytewise lexicographic encoding of their identifiers;
- duplicate map keys, operator IDs, public keys, receipts, deltas, signatures, and evidence records are rejected;
- integers use the shortest encoding and implementations reject arithmetic overflow;
- text is UTF-8 and must not be Unicode-normalized during verification;
- secp256k1 public keys use the 33-byte compressed SEC1 encoding;
- ML-DSA-65 keys and signatures use the raw encodings selected by the pinned l1v2 cryptography dependency;
- a CID is decoded to its multicodec and multihash, and the fetched bytes must reproduce that multihash.

The launch profile uses SHA-256. Every digest is domain-separated:

```text
digest(label, object) =
    SHA-256(uint16_be(len(label)) || ASCII(label) || deterministic_cbor(object))
```

Normative labels are stated below. Unknown object versions, algorithms, fields marked critical, or checker digests fail closed.

### 2.2 Genesis object separation

`GenesisPayloadV1` contains:

- object version and algorithm identifiers;
- chain ID and genesis-round ID;
- frozen-input CID and digest;
- selection-output digest;
- receipt deadline reference;
- ordered `G0`, ordered `T0`, `χ0`, and `π0`;
- witness-schema digest;
- pinned dependency revisions and digests;
- identity-receipt digests;
- transition-policy parameters; and
- launch-gate registry schema version.

Its signing hash is:

```text
genesis_payload_hash =
    digest("L1V2_GENESIS_PAYLOAD_V1", GenesisPayloadV1)
```

`GenesisCertificateV1` contains that payload hash, `n_S`, `q_S`, ordered signer IDs, and detached signatures. `GenesisEnvelopeV1` contains the payload and certificate and may have its own distribution hash.

The certificate and envelope hash are not fields of `GenesisPayloadV1`. This separation prevents the certificate from being included in the digest that its own signatures authenticate.

## 3. Genesis registry construction (plan C3)

### 3.1 Selection input

The candidate set is the output of a named, frozen Dynamic UNL round on the fork, called the **genesis round**. Its round ID, frozen-input CID and digest, scoring-rules digest, and selection-output digest are recorded in `GenesisPayloadV1`.

The evidence-source requirements come from the [Dynamic UNL evidence-source note](../governance/dynamic-unl-l1-evidence-source-note.md); score replay follows `dynamic-unl-scoring/docs/DeterministicFinalScore.md`. This specification does not claim that a qualifying round already exists: the cited documents define the evidence and replay machinery, while the selected launch round and its convergence report are required launch artifacts.

Let `Selected` be the ordered output after cutoff and concentration rules. Let `Receipted` be the operators with a unique valid receipt submitted before the receipt deadline. Genesis membership is:

```text
G0 = Selected ∩ Receipted
```

No unselected operator may be inserted or used as a backfill. Correcting evidence requires a new frozen round, new round ID, and complete rebuild of the payload.

### 3.2 Identity bridge (fork key → l1v2 key)

Each selected operator generates a fresh ML-DSA-65 validator key and submits one `IdentityReceiptV1` containing:

- receipt version;
- compressed fork master public key;
- raw ML-DSA-65 public key;
- chain ID;
- genesis-round ID;
- receipt deadline ledger hash and sequence;
- expiry as a fork-ledger close-time value;
- fork-key signature; and
- ML-DSA counter-signature.

The unsigned receipt body is hashed as:

```text
receipt_hash =
    digest("L1V2_IDENTITY_RECEIPT_V1", IdentityReceiptBodyV1)

fork_message =
    digest("L1V2_RECEIPT_FORK_SIGNATURE_V1", receipt_hash)

l1v2_message =
    digest("L1V2_RECEIPT_MLDSA_SIGNATURE_V1", receipt_hash)
```

The fork master key signs `fork_message`; the new ML-DSA key signs `l1v2_message`. Both signatures must verify, both keys must be unique, and the chain ID and round ID must exactly match the payload.

The receipt deadline is identified by a finalized fork ledger hash and sequence. Its close time is the sole time reference: a receipt is valid only if its expiry is later than that close time and it was included in the published receipt set by that deadline. Local clocks are not consulted. Receipt expiry limits pre-genesis reuse; it does not remove a validator after genesis.

An operator without a valid receipt is excluded by the deterministic intersection in §3.1. The slot is not backfilled. If fewer than the reviewed launch minimum remain, L2 fails.

### 3.3 Template trust graph

`T0` assigns every validator the same template trust view: one essential subset `S = G0`, with `n_S = |G0|` and:

```text
q_S = ceil(4 * n_S / 5)
t_S = min(ceil(n_S / 5), floor((q_S - 1) / 2), 2*q_S - n_S - 1)
```

These are reviewed launch-profile parameters, not Consensus v2 constants.

The `4/5` quorum targets agreement by a large supermajority while retaining approximately one-fifth non-participation headroom before integer rounding. `t_S` takes the most conservative of:

1. the same one-fifth launch-fault budget;
2. the bound required by `2*t_S < q_S`; and
3. the bound required by `t_S < 2*q_S - n_S`.

The checker enforces those inequalities and Cobalt linkage independently of this profile. Changing the profile therefore requires a new reviewed payload but cannot bypass checker soundness.

Worked launch examples:

```text
n_S = 12:
q_S = ceil(48/5) = 10
t_S = min(ceil(12/5), floor(9/2), 20-12-1)
    = min(3, 4, 7) = 3
checks: 3 < 8 and 6 < 10

n_S = 20:
q_S = 16
t_S = min(4, 7, 11) = 4
checks: 4 < 12 and 8 < 16
```

Uniform views reduce launch complexity; they are not permanent. A heterogeneous view is valid only through a signed, versioned trust-view proposal included in frozen transition evidence. `χ_t` deterministically checks its syntax, linkage, local soundness, and old-new matrix. A free-form operator declaration cannot alter `T_t`.

### 3.4 Genesis ratification

Each `G0` operator’s ratification client independently:

1. fetches and verifies every pinned dependency and evidence digest;
2. replays the genesis round to the published selection;
3. computes `G0 = Selected ∩ Receipted`;
4. validates every receipt and uniqueness rule;
5. reconstructs `T0`;
6. runs the pinned `χ0` against `G0`, `T0`, and `π0`; and
7. recomputes `genesis_payload_hash`.

A ratifier signs:

```text
digest(
  "L1V2_GENESIS_RATIFICATION_SIGNATURE_V1",
  genesis_payload_hash
)
```

The detached certificate is valid only with signatures from at least `q_S` distinct members of `G0`. Signatures from unknown, duplicate, or non-member keys are rejected.

After finalization, genesis ratifiers have no override authority under [whitepaper](../whitepaper.md) §6. Changing the payload or certificate produces a different genesis envelope and therefore a fork, not an in-chain governance action.

## 4. Post-genesis registry operation (plan C4)

Registry changes follow one deterministic loop:

```text
frozen pipeline round
  -> versioned proposal and deterministic delta
  -> Cobalt check under G_t and T_t
  -> ratification commit-reveal
  -> certificate, finalized activation, and anchored receipts
```

### 4.1 Transition identity and proposal

`TransitionProposalV1` contains:

- chain ID and transition version;
- transition index and pipeline round ID;
- parent registry and trust roots;
- frozen-evidence CID and digest;
- deterministic ordered delta;
- resulting registry and trust roots;
- checker and safety-profile digests;
- commit, reveal, and activation heights;
- evidence-limit profile;
- proposer ID and signature.

Its identifier is:

```text
transition_id =
    digest("L1V2_TRANSITION_PROPOSAL_V1", TransitionProposalV1)
```

A delta is applied in operator-ID order. Duplicate operations, missing removal targets, additions of existing IDs, key reuse, arithmetic overflow, or a computed root different from the proposal fail the round.

The initial authorized proposer is the Foundation-operated pipeline. This is a censorship and availability dependency, not a validation authority: it cannot make clients accept invalid evidence. The E6 multi-proposer follow-on must replace it with a separately reviewed deterministic conflict rule. Until then, two different validly signed proposals from the authorized proposer for the same parent and round are proposer equivocation and fail the entire round; neither proposal activates.

### 4.2 Commit-reveal ratification

The transition policy in `π_t` specifies deadlines as finalized l1v2 heights, not local time. It also fixes maximum evidence bytes, record count, decompressed bytes, verification work, evidence age, fetch retries, and completion blocks. These values are payload-bound launch parameters and must include rationale and boundary fixtures in the launch bundle.

After deterministic verification, a client creates an ML-DSA signature over:

```text
transition_signature_message =
    digest("L1V2_TRANSITION_SIGNATURE_V1", transition_id)
```

Using a fresh 32-byte nonce, it publishes:

```text
commitment =
    digest(
      "L1V2_TRANSITION_COMMIT_V1",
      (transition_id, signer_id, transition_signature, nonce)
    )
```

Before the reveal deadline it reveals the signature and nonce. A reveal is valid only if:

- its commitment appeared in the commit window;
- signer ID is a unique member of `G_t`;
- the signature verifies;
- chain ID, parent roots, round ID, and transition ID match; and
- the reveal has not appeared in another round or transition.

A signer committing to or revealing different transition IDs for the same parent and round has equivocated. Equivocation is anchored as evidence and the round fails. Commitments without timely valid reveals do not count.

Activation requires a reveal certificate accepted by the pinned Cobalt old-new transition check. Under the uniform launch profile this means at least `q_S` distinct valid reveals; later heterogeneous graphs use the exact certificate predicate defined by the pinned `χ_t`, not an implementation-selected interpretation of “quorum.” Activation occurs only at the proposal’s activation height after certificate finalization. A late or partial certificate changes nothing.

### 4.3 Evidence handling and key lifecycle

Evidence is **missing** if any required object cannot be fetched and digest-verified within the profile’s retry limit; **stale** if its finalized source reference exceeds the profile’s maximum age; **conflicting** if unique keys or source records have different values; and **oversized** if any byte, record, decompression, or work bound is exceeded. Every case produces a stable error code and leaves `G_t` and `T_t` unchanged.

A validator key rotation is a normal evidence-driven transition. It requires a versioned continuity receipt signed by the current and replacement keys. Removal for a publicly evidenced compromise or scoring ineligibility must arise from the frozen pipeline rules and still pass old-rule ratification. Loss of a key without a replacement signature does not create an override path; the operator can be removed by a normally ratified evidence-driven transition while quorum remains.

### 4.4 Quorum degradation and permanent halt

Operators monitor the number of currently reachable ratification clients and publish an alert when it falls below:

```text
required reveal threshold + π_t.liveness_margin
```

The margin is a reviewed profile parameter; it does not alter quorum. Key rotations and evidence-driven removals should be proposed while the old registry can still ratify them.

If the active registry can no longer produce a valid transition certificate:

1. the current registry and trust graph remain unchanged;
2. no administrator, Foundation key, launch signer, or genesis ratifier may lower quorum or edit membership;
3. Consensus and transition clients expose the state `REGISTRY_EVOLUTION_HALTED`;
4. operators continue attempts only under the existing rules; and
5. if quorum loss is judged permanent, recovery is a coordinated successor testnet with a new chain ID and new genesis payload, not an in-chain transition.

A successor genesis must use a new named frozen round, the same deterministic construction and receipt rules, and a new L1–L6 evaluation. Its launch record identifies the halted predecessor and the evidence used to classify the halt. This is an explicit terminal-state procedure: it preserves fail-closed operation and does not represent the successor as having been validated by unavailable old rules.

## 5. Launch gates (plan D)

“Public testnet” is declared only when every gate has a valid, unexpired evidence record:

| Gate | Requirement | Evidence and verification |
| --- | --- | --- |
| L1 Qualification | Storage status is `OFFLINE QUALIFIED`; the **G6 six-clone rehearsal**—the Phase B checkpoint running six independently deployed clones of the qualified lineage—has passed; the deployed lineage has fleet receipts | Qualification and rehearsal artifacts defined by [storage-scaling-fix-spec](storage-scaling-fix-spec.md) and the public-testnet path plan |
| L2 Registry | Genesis satisfies §3 with `n_S ≥ 12` valid receipts | Payload, receipts, replay report, checker output, and detached certificate |
| L3 Independence | No correlated operator group can reach or unilaterally block the launch quorum | Versioned correlation dataset, placement preflight, calculation, and evidence digest |
| L4 Verification | At least the required reveal quorum completes and activates two consecutive scheduled weekly transitions within the profile’s completion bound | Signed convergence reports, certificates, activation proofs, and failure-injection results |
| L5 Operations | Public join runbook, key-custody and rotation guidance, monitoring endpoints, halt status, and successor-testnet procedure are published | Phase D2 public operations package |
| L6 Decision | The authorized release operator records an explicit launch decision after independently verifying L1–L5 | Signed `LaunchDecisionV1` naming the chain ID, payload hash, gate-registry root, signer role, and decision time |

### 5.1 Rationale for launch parameters

`n_S ≥ 12` is a reviewed testnet admission parameter, not a proof that twelve validators are universally sufficient. At the minimum it yields `q_S = 10`, permits two unavailable validators without relaxing quorum, and—under L3’s maximum group size of two—requires at least six independently classified groups. Smaller registries make one- or two-validator correlation groups disproportionately powerful and provide too little operational diversity for the launch exercise. Any change requires a documented sensitivity calculation and new payload review.

L4 uses two consecutive seven-day scheduled rounds to demonstrate repeat operation across more than one operator-maintenance cycle and to detect one-off success. It is a launch observation window, not a claim of long-term liveness. Both rounds must reach reveal quorum, pass `χ_t`, finalize within `π_t.max_completion_blocks`, and activate. At least one rehearsal must inject a commit-without-reveal, stale evidence, and an unavailable client without causing an invalid activation.

### 5.2 Formal independence calculation

For L3, every validator has unit launch weight. An **operator group** is a connected component in a versioned correlation graph whose edges represent common beneficial ownership, controlling organization, validator-key custody, hosting account, or other control relation defined by the fork’s placement and admission-correlation rules. The dataset, rule version, unresolved records, and digest are published. An unresolved correlation fails L3 rather than being treated as independent.

For group `C`:

```text
group_weight(C) = number of G0 validators in C
reach_threshold = q_S
blocking_threshold = n_S - q_S + 1
```

L3 passes exactly when:

```text
for every C:
    group_weight(C) < q_S
    group_weight(C) < n_S - q_S + 1
```

The second inequality means removing any one group still leaves at least `q_S` validators. At `n_S = 12` and `q_S = 10`, every group is therefore limited to at most two validators.

L6 does not authorize protocol changes and cannot waive another gate. The single release-operator role is an acknowledged launch-coordination dependency; its signer identity and authorization source must be present in the gate registry.

## 6. Status CLI and interface (plan E)

A Python CLI in `tools/testnet_path/`, with entry point `testnet-path`, reads the versioned gate registry in `docs/status/` and renders:

- `testnet-path status` — gate table; color is optional;
- `testnet-path blockers` — open, invalid, or expired gates with owners;
- `testnet-path gate <id>` — requirement, evidence, digest, signer, verifier result, and freshness;
- `testnet-path verify` — schema, signatures, digests, freshness, and cross-gate checks.

The generated MkDocs status page reads the same verified registry. Shared input prevents representational disagreement; it does not by itself prove correctness, so generation fails if registry verification fails.

Each gate record contains:

```text
schema_version
gate_id
status: blocked | pending | passed | expired
owner
requirement_version
evidence_uri
evidence_sha256
producer_id
producer_signature
created_at
source_finality_reference
expires_at or freshness_policy
verifier_version
verifier_digest
verifier_result
```

The registry rejects unknown gates, duplicate IDs, unsigned status changes, missing evidence, digest mismatch, stale evidence, and `passed` records whose automated verifier did not return success. L6 additionally requires a valid `LaunchDecisionV1`.

## 7. Invariants

1. **No manual in-chain registry membership** — genesis and every transition trace to frozen evidence, published deterministic rules, and valid receipts. A successor after permanent halt is explicitly a new chain.
2. **Old rules validate new rules** — every in-chain transition, including genesis to the first transition, is checked under the active registry and trust rules.
3. **Ratification is verification** — clients have no approval input; signing and declining follow deterministic verification, and declines use anchored error codes.
4. **Verification must fit the reviewed commodity-hardware budget** unless C2 retains model authority. If retained, the pinned model, GPU replay lane, cost, and reproducibility limits are explicit L4 liabilities.
5. **Fail closed everywhere** — failed, missing, stale, conflicting, expired, or oversized inputs never mutate registry state.
6. **No consensus-byte changes** — this specification changes neither Consensus v2 nor storage consensus encoding.
7. **No hidden emergency authority** — key compromise, proposer failure, or quorum loss cannot lower thresholds or bypass evidence.
8. **Every normative dependency is pinned** — an unpinned or digest-mismatched checker, schema, rule document, or executable is unverifiable.

## 8. Acceptance criteria

- **Canonical objects:** implementations produce identical payload, receipt, proposal, transition, registry, and trust roots on two independent machines. Fixtures cover map ordering, duplicate keys, alternate key encodings, unknown critical fields, overflow, CID mismatch, and checker-version mismatch.
- **Genesis:** a dry-run build from a real frozen fork round reproduces the payload hash. Edited selection, unselected backfill, duplicate identity or key, missing or expired receipt, wrong chain ID or round ID, unsound `T0`, dependency mismatch, and a certificate placed inside the signed payload each fail with a named code.
- **Ratification:** signatures verify only in their stated domains. Cross-chain, cross-round, receipt-to-transition, and envelope-hash replay attempts fail.
- **Transitions:** a controlled-devnet rehearsal completes proposal, check, commit, reveal, certificate, finalization, and activation. Stale or oversized evidence, tampered delta, conflicting proposal, commitment without reveal, reveal without commitment, replayed reveal, signer equivocation, late certificate, wrong parent root, and insufficient quorum hold the registry.
- **Trust views:** uniform and heterogeneous fixtures pass only when local soundness, linkage, and old-new matrix checks pass under the pinned `χ_t`.
- **Independence:** the L3 verifier reproduces connected components and thresholds from the published correlation dataset; an unresolved correlation or a group of size `n_S - q_S + 1` fails.
- **Liveness and halt:** two consecutive L4 transitions activate within the configured bound. A fixture dropping participation below quorum enters `REGISTRY_EVOLUTION_HALTED`, performs no transition, and exposes the successor-testnet procedure.
- **Gate registry:** every gate has signed, digest-bound, fresh evidence and a successful verifier result. Tampered, stale, unsigned, or schema-invalid records fail both CLI and docs generation.
- **Tooling:** the CLI runs from a clean checkout with `pip install`-able dependencies, and the status page renders under strict MkDocs.

## 9. Open questions (tracked, not blocking review)

1. Token and migration economics between the fork and l1v2 remain a separate document.
2. Operator incentives and disclosures for the no-reward testnet belong in the L5/D2 operations package.
3. C2 must decide whether model output remains authoritative after shadow evaluation. The decision artifact must pin the applicable scoring path; retaining model authority adds the documented GPU replay liability to L4.
4. E6 must define multi-proposer admission and deterministic conflict resolution before the Foundation-operated proposer can be removed.