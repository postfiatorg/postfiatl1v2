# Genesis Registry Proposal Path Design

**Status:** Design document — Text Improvement Harness full gate passed on 2026-09-01 — average 88.93/100 (GPT 90.20, Fable 88.80, GLM 87.80; five runs per lane; run group `l1-genesis-registry-proposal-path`); scored content SHA-256 `9a98590c14cc66237ed28518f26ff5180dbfc4656f79a73b8e34af33d8a3c650`

**Date:** 2026-09-01

**Author:** Domagoj Ravlić (`dravlic`)

**Decision owner:** Post Fiat

**Implements:** milestone item C3 of the [public-testnet path plan](../plans/active/l1v2-public-testnet-path-milestone.md)

**Related:** the locked [testnet genesis and launch specification](l1v2-testnet-genesis-and-launch-spec.md) (commit `3318ab23`, harness 88.93), the [Dynamic UNL L1 evidence-source note](../governance/dynamic-unl-l1-evidence-source-note.md) (Option C, `SHADOW_ONLY`), the [validator evaluator alternatives note](../governance/validator-evaluator-alternatives-note.md) (Dynamic UNL content inside the DGA envelope), the [L1 observer](../governance/l1-observer-research-spec.md) and [anchor profile](../governance/l1-anchor-profile-research-spec.md) research specifications, the [deferred Dynamic UNL proposal-source milestone](../deferred-plans/dynamic-unl-proposal-source-milestone.md), and the [pending operator decisions sheet](../governance/pending-operator-decisions.md)

## 1. Purpose and scope

This document designs one pipeline: the PFT Ledger fork's scored operator
community becomes the proposed l1v2 genesis validator registry plus a template
trust graph, and that proposal is checked and ratified through the Cobalt
machinery with Dynamic UNL Phase 3A/3B semantics. Phase 3A means the proposal
content is decided by a frozen, replayable scoring round, not by a Foundation
edit. Phase 3B means the registry is protocol state whose acceptance is
ratified by validators under published prior rules, not by a publisher key.

The locked genesis specification already defines the destination objects:
`GenesisPayloadV1`, `IdentityReceiptV1`, the membership rule
`G0 = Selected ∩ Receipted`, the uniform template trust graph, and detached
genesis certificates (its §2–§3). This document designs the path into those
objects: which fork artifacts are the source of truth, how each carried field
is bound, what the intermediate proposed-registry object looks like, who
submits it, what the Cobalt checker validates, and how anyone recomputes the
result. It is a design, not a milestone and not code. It authorizes nothing.

## 2. Source data: the frozen fork round artifacts

### 2.1 The genesis round

The candidate operator set comes from one named, frozen Dynamic UNL round on
the fork — the **genesis round**. The genesis round does not exist yet; it must
be a future weekly production round that satisfies milestone item C1
(fork governance verification G.6/G.7 and Evidence Transparency E.1 in
`dynamic-unl-scoring/docs/CurrentRoadmap.md`) and the Phase 3A qualification
thresholds recorded in the deferred proposal-source milestone (four consecutive
rounds, at least 10 commit-reveal participants each, convergence above 95%).

Every frozen fork round publishes one content-addressed bundle (IPFS CID,
announced and anchored by a signed PFT Ledger memo transaction) with the Phase
2 layout defined in `dynamic-unl-scoring/docs/phase2/ArtifactBundleAudit.md`:

```text
round CID/
  bundle.json                          # entry point, per-file SHA-256 map
  inputs/validator_evidence.json       # normalized per-validator evidence
  inputs/validator_map.json            # scored-identity inventory
  inputs/previous_unl.json             # churn-control baseline
  runtime/execution_manifest.json      # pinned model, formula, selector params
  outputs/validator_scores.json        # model sub-scores per master key
  outputs/final_scores.json            # deterministic {master_key, final_score}
  outputs/selected_unl.json            # ordered selected master keys
  outputs/signed_validator_list.json   # published VL (Phase 3A: Foundation-signed)
  outputs/verification_hashes.json     # canonical hashes sidecars commit to
  raw/                                 # raw source evidence for audit
```

Rounds 12–19 of exactly this pipeline are already archived in this repository
with per-file SHA-256 digests
(`benchmarks/ai-governance/dunl-subscorer-shadow-20260901/rounds/`,
`rounds-manifest.json`) and were replayed for the C2 shadow evaluation. They
are the rehearsal fixtures for this design. They are not the genesis round:
the archived subset omits `outputs/final_scores.json` and the signed VL, and
predates the C1 evidence gates.

### 2.2 Which artifacts identify the scored operator set

The scored operator set is fixed by four artifacts of the genesis round, in
this order of authority:

1. `runtime/execution_manifest.json` — pins the model and runtime, the score
   formula content hash (`compute_final_score` per
   `dynamic-unl-scoring/docs/DeterministicFinalScore.md`:
   `min((50c+20r+10s+10d+10i)//100, c+25)` over integer sub-scores), and the
   selector parameters (cutoff 40, maximum size, minimum churn gap, previous
   UNL reference). Nothing outside the manifest may parameterize selection.
2. `outputs/final_scores.json` — the deterministic per-validator final scores,
   sorted by master key.
3. `outputs/selected_unl.json` — the ordered selector output. `Selected` in
   the genesis membership rule is exactly this list; the tie-break is the
   published `(score desc, master_key asc)` rule.
4. The round's convergence report — the sealed commit-reveal record proving
   independent sidecars reproduced the same output hashes
   (`dynamic-unl-scoring/docs/phase2/ConvergenceReporting.md`).

### 2.3 Exact fields carried per operator

For each operator in `Selected`, the proposal path carries forward exactly
three field groups and nothing else:

| Field | Source artifact | Form carried over |
| --- | --- | --- |
| Fork master key | `outputs/selected_unl.json` entry | The validator's secp256k1 master public key, decoded from its base58 `n…` form to the 33-byte compressed SEC1 encoding required by the genesis specification §2.1 |
| Final score | `outputs/final_scores.json` entry for that master key | The integer 0–100 deterministic final score, plus the round's cutoff value, so the score is interpretable without refetching the manifest |
| Identity evidence | `inputs/validator_evidence.json` and `inputs/validator_map.json` records for that master key | Not copied by value: carried as the SHA-256 digest of the operator's canonical evidence record (domain, verified-domain status, declared provider and country fields), so the proposed registry binds the evidence without republishing raw observation data |

Model sub-scores, reasoning text, and the network report are deliberately not
carried: under prompt v8 semantics they are advisory inputs to the formula,
and the formula output is the only authoritative score. No field may be
edited, backfilled, or supplemented after the round freezes; correcting
anything requires a new frozen round, as the genesis specification §3.1
already requires.

## 3. Identity binding: fork key to l1v2 ML-DSA identity

The fork identity is a secp256k1 master key; the l1v2 registry identity is a
validator ID with an ML-DSA-65 public key (`docs/governance/validator-registry.md`,
`crates/types/src/core_chain.rs`). The bridge is the genesis specification's
`IdentityReceiptV1` (§3.2): the selected operator generates a fresh ML-DSA-65
key and publishes one receipt in which the fork master key signs the receipt
hash and the new ML-DSA key counter-signs it, with chain ID, genesis-round ID,
receipt-deadline ledger reference, and expiry bound inside the signed body.

This design adds the governed-binding rules recorded in the evidence-source
note, applied to receipts:

- **One-to-one and exhaustive.** Each fork master key maps to at most one
  ML-DSA key and each ML-DSA key to at most one master key. Duplicates on
  either side invalidate both colliding receipts.
- **No post-hoc mapping.** A receipt is valid only if it is included in the
  published receipt set at the receipt-deadline ledger. There is no appeal,
  substitution, or operator-edited mapping afterward; a missing receipt means
  the deterministic intersection excludes the operator and the seat is not
  backfilled.
- **Signature delegation matches the fork's custody model.** The fork
  signature is produced through `postfiatd validator-keys` exactly as sidecar
  commit-reveal payloads are signed today
  (`validator-scoring-sidecar/docs/Overview.md`); the sidecar never holds the
  master-key seed, and this path must not require it to.
- **Expiry and revocation.** Receipt expiry is a fork-ledger close-time value
  and limits pre-genesis reuse only. Before the deadline, an operator may
  supersede its own receipt exactly once per round by publishing a
  replacement signed by the same master key; after the deadline the set is
  frozen.
- **Continuity, not measurement transfer.** A valid receipt proves the same
  operator controls both keys. It does not transfer the fork's performance
  evidence onto the l1v2 process — the recorded Option C boundary. The final
  score travels as provenance-labeled fork evidence, never as an l1v2-native
  measurement.

Post-genesis, the equivalent binding object is `DynamicUnlValidatorBindingV1`
from the deferred proposal-source milestone (`new:
crates/types/src/dynamic_unl_proposal.rs`); genesis receipts are the bootstrap
case of the same rule set and must share encoding and digest conventions.

## 4. The proposed registry object and template trust graph

### 4.1 Schema sketch

One canonical object carries the full proposal. Encoding follows the genesis
specification §2.1: deterministic CBOR, integer map labels, bytewise
lexicographic array ordering, duplicate rejection, fail-closed unknowns.

```text
ProposedGenesisRegistryV1:
  version                     # object version, critical
  chain_id                    # the new l1v2 chain, not the controlled devnet
  genesis_round:
    fork_network              # e.g. "testnet"
    round_number
    bundle_cid                # multihash-checked content address
    bundle_digest             # SHA-256 of canonical bundle.json
    manifest_digest           # runtime/execution_manifest.json
    final_scores_digest       # outputs/final_scores.json
    selected_unl_digest       # outputs/selected_unl.json
    convergence_report_digest
    anchor_tx_hash            # PFT Ledger memo transaction that anchored the round
  receipt_deadline:
    fork_ledger_hash
    fork_ledger_seq
  entries: [ProposedGenesisEntryV1]   # sorted, see §4.2
  template_trust_graph:
    n_S                       # = len(entries)
    q_S                       # = ceil(4 * n_S / 5)
    t_S                       # = min(ceil(n_S/5), floor((q_S-1)/2), 2*q_S - n_S - 1)

ProposedGenesisEntryV1:
  fork_master_key             # 33-byte compressed SEC1
  final_score                 # integer 0–100
  cutoff                     # the round's eligibility cutoff (40 today)
  selection_index             # position in outputs/selected_unl.json
  identity_evidence_digest    # canonical evidence record for this master key
  identity_receipt_digest     # digest("L1V2_IDENTITY_RECEIPT_V1", body)
  mldsa_public_key            # raw ML-DSA-65 encoding from the receipt
```

Rust types belong in `new: crates/types/src/genesis_registry.rs`, wired
through `crates/types/src/core_chain.rs` beside the existing governance
objects, and shared with the post-genesis schemas in `new:
crates/types/src/dynamic_unl_proposal.rs` rather than duplicated.

### 4.2 Deterministic ordering and membership

`entries` contains exactly `Selected ∩ Receipted`, sorted by the bytewise
lexicographic order of the 33-byte `fork_master_key`. `selection_index`
preserves the selector's ranked order so the sort is reversible; a mismatch
between the two orderings for the same key set is a build error, not a
tolerated variation. Any entry whose master key is absent from
`outputs/selected_unl.json`, any duplicate key on either chain's side, and
any receipt digest that fails verification each invalidate the whole object.

### 4.3 Content hash

```text
proposed_registry_hash =
    digest("L1V2_PROPOSED_GENESIS_REGISTRY_V1", ProposedGenesisRegistryV1)
```

using the genesis specification's domain-separated digest construction. This
hash is what gets announced, checked, signed against, and recomputed. The
template trust graph is inside the hashed object, so registry and trust graph
cannot be ratified separately or swapped independently. From this object, the
final `GenesisPayloadV1` fields (`G0` ordered registry, `T0` ordered trust
graph, identity-receipt digests, frozen-input digests) are a deterministic
projection; two builders must produce byte-identical payloads from the same
proposed object.

### 4.4 Why the trust graph is a template

Every entry receives the same uniform trust view — one essential subset
`S = G0` with the `q_S` and `t_S` formulas above, worked examples and safety
inequalities per the genesis specification §3.3. The template is computed, not
chosen: no per-operator trust editing exists on this path. Heterogeneous
views come later, only through signed, versioned trust-view proposals checked
by the pinned transition checker.

## 5. The proposal path

### 5.1 Roles

| Role | Who | Bound |
| --- | --- | --- |
| Content source | The genesis round's frozen pipeline (Phase 3A semantics: no human selects or edits the list) | §2 artifacts only |
| Builder | Any party — the build is deterministic; the reference implementation is `new: python/postfiat_rpc/genesis_registry.py` | Emits byte-identical objects or fails with a named error |
| Submitter | An admitted independent operator using the envelope from the [independent-operator proposal-path specification](../governance/cobalt-independent-operator-proposal-path-research-spec.md); the owner is `unassigned` on the pending-decisions sheet | Submits unchanged canonical bytes; cannot rewrite content |
| Checker | The pinned Cobalt transition checker (`crates/consensus_cobalt/src/trust_graph_governance.rs`, `core_types.rs`, admission in `validator_admission_policy.rs` plus `new: crates/consensus_cobalt/src/dynamic_unl_source.rs`) | Deterministic validation, no discretionary input |
| Ratifiers | The proposed operators themselves, each running the C4 ratification client (the `validator-scoring-sidecar` extension) | Sign only after independent replay; commit-reveal |

### 5.2 What Cobalt checks and ratifies

The Cobalt-checked path validates, in order, all of:

1. **Source admission** — the genesis round is sealed, anchored, and satisfies
   the Phase 3A qualification thresholds; every referenced digest matches its
   fetched artifact; the convergence report meets the participation and
   convergence bounds (`new: crates/consensus_cobalt/src/dynamic_unl_source.rs`).
2. **Membership derivation** — `entries` equals the recomputed
   `Selected ∩ Receipted`; ordering, uniqueness, and receipt validity hold.
3. **Trust-graph soundness** — the template graph satisfies the Cobalt
   inequalities (`2·t_S < q_S` and `t_S < 2·q_S − n_S`), linkage, and the
   reviewed launch profile, under the same trust-view support rules the live
   controlled devnet uses (`crates/consensus_cobalt/src/core_types.rs`).
4. **Policy bounds** — the DGA envelope from the alternatives note applies at
   admission: the governed policy pins the admitted source and schema
   versions, the model/runtime and formula versions, the identity-binding
   rules, and the registry size bounds (the launch profile's `n_S ≥ 12`
   minimum and the fork selector's maximum). The policy may reject or hold;
   it must not rescore validators, reorder the ranking, or substitute
   members. Because genesis has no predecessor registry on the new chain,
   the per-transition churn budget does not apply at genesis; it binds every
   post-genesis transition.

Ratification is the genesis specification's §3.4 procedure with Phase 3B
semantics: each proposed operator's client independently rebuilds the object
from the published artifacts, runs the pinned checker, and signs the payload
hash through commit-reveal; the detached certificate needs `q_S` distinct
member signatures. The published deterministic construction rules play the
"previous rules" role that an existing registry plays in later transitions.

### 5.3 What Consensus v2 orders

On the new l1v2 chain, nothing: genesis is the first state, and the payload
plus certificate are launch artifacts, not transactions. Consensus v2 ordering
enters twice at the edges. First, every post-genesis registry transition is a
Cobalt-ratified governance update ordered and finalized by Consensus v2
exactly as on the controlled devnet today (`crates/node/src/cobalt_handoff.rs`,
`crates/node/src/governance.rs`). Second, the `SHADOW_ONLY` rehearsal of this
path runs on the controlled devnet, where the shadow proposal's admission
dry-run and its rejection paths are exercised against the live Cobalt lane
(`crates/node/src/cobalt_shadow.rs`) without mutating any registry; Consensus
v2 keeps finalizing blocks throughout, unchanged.

## 6. Verification story

Anyone must be able to recompute the proposed registry from public data with
no privileged access:

1. Fetch the genesis round bundle by CID; verify every file against
   `bundle.json` and `outputs/verification_hashes.json`; verify the round's
   announcement and anchor memo transactions on the PFT Ledger.
2. Recompute the final scores from the frozen sub-scores with the
   manifest-pinned formula, and rerun the selector with the manifest-pinned
   parameters and frozen previous UNL; the result must equal
   `outputs/final_scores.json` and `outputs/selected_unl.json` byte for byte.
   The C2 shadow evaluation already demonstrated exactly this replay across
   rounds 12–19 from the archived artifacts.
3. Fetch the published receipt set at the receipt-deadline ledger; verify both
   signatures on every receipt; apply the uniqueness and deadline rules.
4. Rebuild `ProposedGenesisRegistryV1`, recompute `proposed_registry_hash`,
   and compare with the announced hash. Then project `GenesisPayloadV1` and
   compare with the payload the certificate signs.

The reference tool is `new: python/postfiat_rpc/genesis_registry.py`
(`build`, `verify`, `explain` subcommands, following the verifier pattern of
`python/postfiat_rpc/storage_scaling.py`), with tests in `new:
python/tests/test_genesis_registry.py`. The acceptance bar from the genesis
specification §8 applies: an independent implementation that shares no
canonicalization code must reproduce the same hashes, and every mutated
fixture must fail with a named error.

## 7. Boundaries

- **`SHADOW_ONLY` until the recorded gates pass.** Everything this document
  designs runs as shadow work under the Option C decision: real fork rounds
  and real bindings may drive builds, checker runs, and controlled-devnet
  admission dry-runs, but no output of this path may mutate any registry until
  the deferred milestone's gates (E1–E5) and the C1 fork gates pass and a
  separate recorded decision grants authority.
- **No authority from this document.** It grants no proposer, submitter,
  model, observer, or operator any authority, changes no chain state, sends no
  transaction, and locks nothing by itself.
- **Public testnet stays blocked.** Community-facing execution of this path —
  genesis outreach, receipt collection from real operators, ratification
  clients on operator machines — is Gate Zero work (Z1–Z3) and separately an
  operator decision (D4). Only the design and fixture-driven implementation
  proceed now.
- **Open operator decisions this path depends on**, from the
  [pending-decisions sheet](../governance/pending-operator-decisions.md): the
  confirmation of Dynamic UNL as proposal-content source inside the DGA
  envelope; the confirmation of evidence sequence Option C with all
  PFT-derived integration `SHADOW_ONLY`; the L1 observer service owner; the
  independent-operator submitter owner; who bears pinned model inference
  cost; and the Task Node lock of the observer and anchor-profile
  specifications. An unfavorable answer to the first two rows changes this
  design's content source; unassigned owners block the submitter and the
  post-genesis evidence path, not the schema work.
- **Known dependency risks.** The fork's agreement evidence still has one
  Foundation observer until E.1/E.2 close; the signed VL and round anchors
  are Foundation-published until Phase 3A completes; and operator overlap
  between the two chains (evidence-source note question 1) determines whether
  the genesis round's selected set is a representative registry or a small
  bootstrap set near the `n_S ≥ 12` floor.

## 8. Work sequence (for later implementation)

1. **Implemented** (commit `fa7e67ff`) — Canonical types and fixtures:
   `crates/types/src/genesis_registry.rs`, golden vectors plus one-field
   mutations under `benchmarks/genesis-registry/fixtures/`, reusing archived
   rounds 12–19 as build inputs.
2. **Implemented** (commit `83546dcb`) — Reference builder and verifier CLI:
   `python/postfiat_rpc/genesis_registry.py` with
   `python/tests/test_genesis_registry.py`; two-implementation hash agreement.
3. Checker integration: run the pinned Cobalt checker against fixture
   `G0`/`T0` pairs across `n_S` = 12–20, including every rejection case, in
   `crates/consensus_cobalt` tests.
4. Source admission: `new: crates/consensus_cobalt/src/dynamic_unl_source.rs`
   shared with the deferred milestone's E2, covering the Phase 3A threshold
   checks and named reason codes.
5. Controlled-devnet `SHADOW_ONLY` dry-run through the independent-operator
   envelope: admission, dry-run record, restart, and rejection paths; no
   registry mutation.
6. C4 ratification client (the sidecar extension) — its own milestone item,
   out of scope here.
7. Real genesis-round selection, receipt collection, and payload build —
   Gate Zero-blocked, operator-decided, per the locked genesis specification.

Each step lands with focused Cobalt/governance tests only; no step here
crosses an Orchard boundary.
