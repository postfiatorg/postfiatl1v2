# Dynamic UNL Proposal Source Milestone

**Status:** Deferred draft — not authorized, not current execution work; PUBLIC TESTNET BLOCK unchanged
**Drafted:** 2026-08-27 by Domagoj Ravlić (`dravlic`)
**Decision owner:** Post Fiat
**Research specification:** [Dynamic UNL Proposal Source](../governance/dynamic-unl-proposal-source-research-spec.md) — Text Improvement Harness full gate passed on 2026-08-26, average 89.13/100 (GPT 92.00, Fable 86.00, GLM 89.40; five runs per lane; run group `dynamic-unl-proposal-source-research-spec`); scored content SHA-256 `62938fea81f10cc0dbda531593666e9b469eebb80d02c948db644c90e58647c8`; Task Node lock pending the operator's decision
**Prerequisites before activation:** the operator picks this as the next milestone; Task Node locks the specification, or an explicit operator exception is recorded as it was for the storage milestone; the storage-scaling milestone is closed or explicitly de-prioritised

On this L1 the validator set is governed registry state, Cobalt has ratified
registry and trust-graph changes since height 916, and every proposal so far
has originated from Foundation-administered validators. See
[Current State](../status/chain-state-current.md) and the
[Deterministic Governance Overview](../governance/deterministic-governance-overview.md).
This milestone would make a sealed Dynamic UNL round, backed by four-round
qualification, the deterministic content source for those proposals without
changing Cobalt's authority or Consensus v2.

## E1 — canonical proposal adapter and standalone verifier

- [ ] Define closed, bounded, domain-separated, canonically encoded, and
  version-hashed `DynamicUnlSourceCertificateV1`,
  `DynamicUnlValidatorBindingV1`, and `DynamicUnlRegistryProposalV1` schemas.
- [ ] Consume the sealed report, frozen input package, execution manifest,
  final bundle, commit-reveal records, announcement and anchor evidence,
  four-round history, governed identity bindings, active DGA policy, and an
  authenticated L1 snapshot without discretionary input.
- [ ] Recompute every file hash, formula and selector output, selected list,
  convergence value, identity projection, registry delta, resulting root, and
  proposal bytes; bind the PFT Ledger network/genesis, round, announcement,
  input CID/hash, manifest, formula/selector, final bundle, selected list,
  report, anchor, and artifact lineage into the source certificate, and bind
  proposal bytes to the L1 chain/genesis/protocol, authority transition and
  history, current registry/trust roots, slot, activation/expiry, source
  round/certificate, target list, exact delta, resulting roots, and evidence
  root.
- [ ] Authenticate announcement, commit/reveal, and report-anchor transactions
  against the exact pinned PFT Ledger network/genesis and validated-ledger
  checkpoint/finality rule; disclose any residual RPC/publisher trust and treat
  IPFS only as content transport.
- [ ] Make offline packet and online resolution modes emit identical canonical
  bytes regardless of fetch order, gateway, RPC, JSON field order, or local
  time; compare at least two independent PFT Ledger RPCs and two artifact
  gateways or fail with a named source-trust error.
- [ ] Freeze valid vectors and one-field mutations for every bound field,
  identity ambiguity, absent file, non-canonical encoding, oversized artifact,
  unavailable source, wrong chain/root, and stale snapshot.
- [ ] Have two independent implementations reproduce byte-identical source
  certificates, identity projections, deltas, proposal bytes, and hashes
  without sharing canonicalization or adapter code.
- [ ] Recompute the selected list from the frozen input, manifest, formula, and
  selector and match the sealed result and final bundle exactly; prove every
  affected identity was governed and valid before the source round.
- [ ] Prove every invalid or unavailable vector rejects or holds before Cobalt
  voting with zero state mutation and no post-hoc, partial, or operator-edited
  identity mapping.

Primary code (candidates):

- `new: python/postfiat_rpc/dynamic_unl_proposal.py` — CLI, offline verifier,
  online resolver, and loopback read-only browser.
- `python/postfiat_rpc/storage_scaling.py` — existing verifier/browser pattern.
- `new: python/tests/test_dynamic_unl_proposal.py`
- `new: crates/types/src/dynamic_unl_proposal.rs`
- `crates/types/src/core_chain.rs`
- `new: benchmarks/dynamic-unl-proposal-source/e1/`
- `new: docs/evidence/dynamic-unl-proposal-source.md`

## E2 — Phase 3A ratification preconditions

- [ ] Add a Dynamic UNL source precondition to Cobalt proposal admission that
  accepts only normal rounds and validates the source certificate, current L1
  bindings, and four-report qualification certificate.
- [ ] Freeze integer-basis-point arithmetic: participation counts distinct,
  eligible, signature-valid validators with accepted in-window reveals, and
  convergence divides matching acceptance-level hashes by that frozen eligible
  participant denominator.
- [ ] Require four consecutive round numbers, at least 10 participants in each,
  and convergence strictly greater than 9,500 basis points (>95%) in each.
- [ ] Return distinct reason codes for an unsealed report; missing or invalid
  anchor; fewer than four rounds; stale, duplicate, skipped, non-converged, or
  below-10 rounds; wrong-round artifact; wrong input CID/hash; manifest,
  formula, selector, or list mismatch; report/anchor hash mismatch;
  source-chain replay; wrong L1 chain, registry, trust, authority-history,
  slot, activation, or expiry binding; unavailable artifacts; and proposal
  bytes that differ from standalone recomputation.
- [ ] Keep all network resolution outside L1 consensus, bind Cobalt votes to
  both source-certificate and proposal hashes, and persist enough canonical
  evidence for restart and catch-up without mutable endpoint access.
- [ ] Prove every correct validator admits the same valid bytes and assigns the
  same reason code to every invalid vector.
- [ ] Pass participation 10 and convergence above 95% for four consecutive
  sealed rounds; reject or hold participation 9, convergence exactly 95%, any
  broken streak, and every named negative case without durable mutation.
- [ ] Prevent duplicated identities, copied or missing reveals, ineligible
  validators, diagnostic-only selected-list agreement, and unrecomputed
  Foundation summary fields from satisfying qualification.
- [ ] Reproduce the same admission decision and proposal hash through restart,
  replay, and honest catch-up with no external I/O during consensus.

Primary code (candidates):

- `new: crates/consensus_cobalt/src/dynamic_unl_source.rs`
- `crates/consensus_cobalt/src/validator_admission_policy.rs`
- `crates/consensus_cobalt/src/core_types.rs`
- `crates/node/src/cobalt_handoff.rs`
- `crates/node/src/governance.rs`
- `new: python/tests/test_dynamic_unl_proposal.py`
- `new: benchmarks/dynamic-unl-proposal-source/e2/`
- `new: docs/evidence/dynamic-unl-proposal-source.md`

## E3 — selector churn and Cobalt quorum alignment

- [ ] Reuse the frozen 10,240-case E1 corpus unchanged across 6–20 validators
  and every strict linkage boundary.
- [ ] Generate current registries, previous lists, score vectors, cutoff
  crossings, incumbent failures, ties, maximum-size pressure, and
  displacement-gap challenges; run the manifest-pinned selector, project only
  pre-bound identities, and calculate the direct add/remove/rotate delta.
- [ ] Derive the safe per-round displacement budget from old-registry Cobalt
  essential subsets, local trust views, tolerated Byzantine count, blocking
  sets, certificate quorum, and post-transition linkedness.
- [ ] Cover mass cutoff failure, maximum-size reduction, binding expiry, one
  incumbent refusing to ratify removal, and candidate sets beyond the L1
  registry limit.
- [ ] Treat maximum size and displacement gap only as evidence inputs; require
  deterministic staging or no-op when needed, with original ranking retained,
  each later step backed by a fresh qualifying report, and old-rule
  ratification.
- [ ] Across all 10,240 graphs and generated transitions, admit zero proposal
  that exceeds its graph budget, violates either Cobalt inequality, loses
  linkedness, permits new-set self-authorization, or forks an accepted root.
- [ ] Make two independent implementations emit one byte-identical direct,
  staged, or no-op result for the same manifest settings, adapter version, and
  state.
- [ ] Hold or split every unsafe transition deterministically; permit no
  operator truncation, reordering, substitution, or hand editing.
- [ ] Preserve five-of-six progress and four-of-six halt before, during, and
  after every admissible step, and reverify the unchanged E1 corpus hashes and
  production classifications.

Primary code (candidates):

- `crates/consensus_cobalt/src/trust_graph_governance.rs`
- `crates/consensus_cobalt/src/validator_admission_policy.rs`
- `new: crates/consensus_cobalt/src/dynamic_unl_source.rs`
- `crates/cobalt_adversarial_oracle/src/lib.rs`
- `benchmarks/cobalt-adversarial-verification/e1/corpus-manifest.json`
- `new: benchmarks/dynamic-unl-proposal-source/e3/`
- `new: python/tests/test_dynamic_unl_proposal.py`
- `new: docs/evidence/dynamic-unl-proposal-source.md`

## E4 — end-to-end controlled-devnet shadow and live drill

- [ ] Select one real, sealed Dynamic UNL normal round only after all public
  artifacts exist; freeze its certificate, four-round evidence, bindings, DGA
  policy, and L1 snapshot before inspecting the non-noop bounded delta, with no
  human edits to the selected list or mapping.
- [ ] Run `SHADOW_ONLY` first: all six L1 validators independently resolve the
  artifacts, emit one byte-identical source certificate and proposal, record
  their vote decision, and leave the registry unchanged.
- [ ] Rehearse the exact ratification, Consensus v2 ordering, restart, catch-up,
  post-change finality, and forward-rollback sequence on disposable
  six-validator clones bound to the current chain, registry, authority history,
  trust state, identities, deployed binary, and authenticated tip; prove the
  post-rollback roots and history are the expected forward state.
- [ ] Require the clone packet, E5 signed-vote lineage, and separate live
  operational authorization before any live submission.
- [ ] Submit only the frozen bytes through the current proposal path, ratify
  with Cobalt, order with Consensus v2, verify resulting registry/trust roots,
  and restore policy state only through a separately authorized new finalized
  forward transition.
- [ ] Rehearse wrong-root, stale-report, report-swap, mapping-swap, and
  rollback-replay cases on clones before sending the exact negative cases live.
- [ ] Commit the valid live proposal exactly once with the expected proposer,
  Cobalt signers, old-registry authorizers, Consensus v2 receipt, roots, and
  source lineage; reject every negative case live without durable mutation.
- [ ] Keep Consensus v2 finalizing throughout and prove Cobalt gains no
  authority outside validator-registry and trust-graph changes.
- [ ] Record whether the submitter was Foundation-administered and make no
  proposal-submission decentralization claim.

Primary code (candidates):

- `crates/node/src/cobalt_shadow.rs`
- `crates/node/src/cobalt_handoff.rs`
- `crates/node/src/cobalt_e5_live_drill.rs`
- `crates/node/src/governance.rs`
- `new: python/postfiat_rpc/dynamic_unl_proposal.py`
- `new: benchmarks/dynamic-unl-proposal-source/e4/`
- `new: docs/evidence/dynamic-unl-proposal-source.md`

## E5 — signed validation-vote lineage

- [ ] Implement or consume Dynamic UNL roadmap Evidence milestone E.1: persist
  the signed PFT Ledger validation votes for each agreement window and publish
  canonical, sorted, deduplicated, content-hashed per-window vote sets linked to
  the exact 1-hour, 24-hour, and 30-day scoring inputs.
- [ ] Verify every available signature and reproduce every agreement score used
  by the model input exactly from signature-valid published votes.
- [ ] Bind vote-set roots, window boundaries, known-validator inventory, ledger
  count, recomputation result, and E.1 status into the source certificate and
  proposal evidence root.
- [ ] Reject omission, duplication, bad signatures, wrong windows, inventory
  substitution, ledger-count substitution, and score/report mismatch with
  named reasons; never represent incomplete lineage as complete.
- [ ] Keep status `VOTE_LINEAGE_PENDING` and permit only shadow or disposable
  clone work until a proposal links to the required signed votes.
- [ ] State the residual observation-completeness assumption: E.1 proves scores
  follow from published votes, while only the planned independent observer
  federation in E.2 reduces single-observer omission risk.
- [ ] If publication remains pending, set the overall decision to
  `SHADOW_ONLY` and publish no live-authority claim.

Primary code (candidates):

- `new: python/postfiat_rpc/dynamic_unl_proposal.py`
- `new: python/tests/test_dynamic_unl_proposal.py`
- `new: crates/types/src/dynamic_unl_proposal.rs`
- `docs/evidence/xrpl-unl-coordination.md`
- `new: benchmarks/dynamic-unl-proposal-source/e5/`
- `new: docs/evidence/dynamic-unl-proposal-source.md`

## Gates

### ADOPT FOR CONTROLLED DEVNET

- [ ] E1: two independent verifiers emit byte-identical, current-state-bound
  proposal bytes from authenticated public artifacts and pre-existing identity
  bindings.
- [ ] E2: four consecutive rounds each have at least 10 participants and
  greater than 95% convergence, and every negative precondition rejects
  identically.
- [ ] E3: every emitted transition fits its graph-specific Cobalt budget across
  the unchanged 10,240-case corpus; unsafe targets stage or hold
  deterministically.
- [ ] E4: the all-six shadow run, exact clone rehearsal, authorized live
  ratification, restart, catch-up, negative cases, and forward rollback pass.
- [ ] E5: signed vote sets reproduce every agreement score and the proposal
  binds their roots.
- [ ] The CLI, read-only browser, checksum-bound packet, independent verifier,
  strict docs build, redaction scan, and required publication pass.

### SHADOW ONLY

- Remain `SHADOW_ONLY` whenever any authority prerequisite is absent, including
  E.1 vote lineage, a self-contained and disclosed source-chain trust rule,
  four consecutive Phase 3A-qualified reports, a complete pre-existing identity
  map, safe churn, clone rehearsal, or separate live authorization.
- Shadow output may be published as research evidence but cannot be submitted
  as an authority-bearing proposal.

### REJECT OR REMEDIATE

- Fail the gate for a conflicting canonical result, source-proof failure,
  accepted stale or non-converged proposal, identity ambiguity, unsafe churn,
  wrong-root acceptance, Cobalt safety/liveness regression, Consensus v2
  regression, or unverifiable rollback.
- Preserve the corpus and failed receipt, repair the owning boundary, and rerun
  the unchanged affected experiment without weakening thresholds or
  reinterpreting an artifact after its result.

## Interfaces and completion

- [ ] Deliver the Python CLI first:
  `python -m postfiat_rpc.dynamic_unl_proposal verify PACKET`, with human-readable
  source, threshold, identity, churn, proposal, and status explanations.
- [ ] After the CLI works, deliver a loopback-only, read-only browser backed by
  the same verified packet, with no proposal, ratification, mutation, or
  rollback route.
- [ ] Bind the packet, all evidence, and both interfaces to one checksum
  manifest and fail closed on missing, mutated, stale, non-canonical,
  mismapped, unsafe, or internally inconsistent content.
- [ ] Deliver an independent verifier that shares neither canonicalization nor
  adapter implementation with the primary path.
- [ ] Pass `.venv-docs/bin/mkdocs build --strict` from a clean source tree.
- [ ] Pass the packet and publication redaction scan.
- [ ] Publish the required architecture, arithmetic, trust, identity, selector,
  churn, vote-lineage, proposer/ratifier, concentration, failure, and
  `SHADOW_ONLY` disclosures.
- [ ] Move this milestone to completed only when every PASS gate holds.

Until then this grants no authority, changes no chain, and public testnet stays
blocked.
