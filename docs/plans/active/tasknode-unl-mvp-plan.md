# Task Node Identity-Derived UNL MVP Execution Plan

**Status:** Active execution plan — steps A through F and the fixture-driven slice of G complete; every output remains `SHADOW_ONLY`

**Date:** 2026-09-04

**Source proposal:** [Derive the UNL from Task Node identity and ratify it through Cobalt](https://postfiat.org/research/deterministic-unl-task-node-cobalt/)

## Objective and authority boundary

This MVP implements Phase 0 of the source proposal plus a fixture-driven
shadow-derivation runner. It deterministically projects public, replayable Task
Node identity evidence into the fields and preconditions needed to exercise
the existing Admission Policy V1 selector shape.

Everything is additive and `SHADOW_ONLY`. This plan changes neither the shape
nor behavior of Admission Policy V1 in
`crates/consensus_cobalt/src/validator_admission_policy.rs`. It authorizes no
registry mutation, Cobalt proposal, ratification, transaction submission,
wallet spend, production deployment, Task Node server change, or other live
action. No output produced under this plan may be presented as an approved UNL
or as authority to change one.

The implementation is an offline Python reference path under
`python/postfiat_rpc/`, following the existing module CLI convention used by
`python/postfiat_rpc/cobalt.py` and
`python/postfiat_rpc/genesis_registry.py`. Focused tests live under
`python/tests/`; frozen fixtures live under
`python/tests/fixtures/tasknode_unl/`. The CLI must mark every machine and
human-readable output `SHADOW_ONLY` and expose no submit or ratify command.

## Inputs and invariants

The source proposal fixes the scoring weights, graph weights, walk parameters,
connectivity floor, cluster threshold, seat cap, and pre-39 churn rule. The
implementation must copy those values exactly into
`python/postfiat_rpc/tasknode_unl_schema.py`, reject per-run overrides, and
cover each constant with a focused test.

Calculations use exact rational arithmetic and canonical lexical ordering; no
binary floating point, local clock, unordered set or mapping iteration,
network-dependent ordering, model judgment, or private field may affect a
result. A caller supplies an explicit evaluation-window end and registry round.
Inputs are bounded, schema-versioned, content-hashed, and fail closed on missing,
stale, conflicting, duplicate, or malformed required evidence.

The runner must reject the forbidden rule inputs listed in
`docs/governance/validator-evidence-field-registry.md`: social-media
reputation, private KYC status, private messages, uncollected search results,
unbounded browsing, unredacted secrets, raw IP geolocation as jurisdiction
proof, human labels absent from the packet, and fields from an unrelated packet
without packet-root lineage. Nostr private messages are never graph edges.

## Build checklist

### A. Accountability scoring engine

**Targets:** `python/postfiat_rpc/tasknode_unl_schema.py`,
`python/postfiat_rpc/tasknode_unl_accountability.py`,
`python/tests/test_tasknode_unl_accountability.py`, and
`python/tests/fixtures/tasknode_unl/accountability.json`.

- [x] Define a rolling 180-day evaluation window with an explicit end instant.
      Count only accepted Network Tasks in `work`; Personal tasks never count
      (`python/postfiat_rpc/tasknode_unl_accountability.py`).
- [x] Clamp every term independently to the closed interval `[0, 1]`, then
      compute the exact weighted sum:
      `35 * clamp(accepted_network_tasks / 40) + 25 * clamp(days_since_first_rewarded_task / 365) + 20 * clamp(verification_pass_rate) + 10 * clamp(1 - open_disputes / 3) + 10 * badge`,
      where `badge` is one only when the verified operator badge is current at
      the window end (`python/postfiat_rpc/tasknode_unl_schema.py`).
- [x] Use window events for work, quality, and open-dispute state; use bounded
      history only to locate the first rewarded task; evaluate badge freshness
      at the window end. A missing verification denominator, first-reward fact,
      dispute state, or badge state holds instead of becoming zero
      (`python/postfiat_rpc/tasknode_unl_accountability.py`).
- [x] Preserve the weighted result as an exact rational in evidence. At the
      existing integer Admission Policy V1 boundary, take the floor once, after
      the complete sum; never round an input term. Record both the exact value
      and projected integer so the conversion is auditable
      (`python/postfiat_rpc/tasknode_unl_accountability.py`).
- [x] Prove with boundary fixtures that clamping works, Personal tasks are
      excluded, 69 remains below the existing floor, 70 passes it, and the
      proposal's representative established operator remains able to exceed 70
      (`python/tests/test_tasknode_unl_accountability.py`).

### B. Trust-graph walk and cluster controls

**Targets:** `python/postfiat_rpc/tasknode_unl_trust_graph.py`,
`python/tests/test_tasknode_unl_trust_graph.py`, and
`python/tests/fixtures/tasknode_unl/trust-graphs.json`.

- [x] Build the seed vector from the currently ratified validator list at the
      window start after removing Foundation-bound validators. Give every
      remaining seed equal mass; an empty or ambiguous seed set holds
      (`python/postfiat_rpc/tasknode_unl_trust_graph.py`).
- [x] Apply proposal weights exactly: vouch `1`; co-work `1` for each shared
      Hive project or Team grant, capped at `3`; funding `2`. Normalize each
      non-empty row to sum to one
      (`python/postfiat_rpc/tasknode_unl_trust_graph.py`).
- [x] Run exactly 20 power-iteration steps using
      `p_next = 0.85 * transpose(P) * p + 0.15 * seed`. Redirect a dangling
      row to the uniform seed vector, retain exact fractions, and sort nodes and
      edges canonically before every reduction
      (`python/postfiat_rpc/tasknode_unl_trust_graph.py`).
- [x] Lock graph direction, duplicate-edge treatment, conductance volume,
      candidate-cut ordering, and tie-breaking in golden fixtures before
      implementation. Vouches are directed; shared-work and qualifying funding
      relations contribute both directions. These mechanics may disambiguate
      the published algorithm but must not change a published numeric constant
      (`python/tests/fixtures/tasknode_unl/trust-graphs.json`).
- [x] Cut clusters only at conductance strictly below `0.1`, with deterministic
      member ordering. Require stationary mass at least `1 / (2N)`, where
      `N` is baseline-list size; disconnected and just-below-floor accounts
      hold (`python/postfiat_rpc/tasknode_unl_trust_graph.py`).
- [x] Halve an account's outgoing vouch weight for the next window when two
      accounts it vouched for later collapse into the same cluster. Bind the
      penalty to the prior window so current-window iteration cannot feed back
      into its own inputs
      (`python/postfiat_rpc/tasknode_unl_trust_graph.py`).
- [x] Enforce no more than `max(2, 10% of N)` seats per cluster by comparing
      the integer seat count to the exact rational limit. Test `N=20`, the
      17-seed current-list shape, the threshold boundary, seed exclusion,
      dangling rows, isolated Sybil rings, the two-cross-cluster-edge
      connectivity case, vouch-weight penalties, and ties
      (`python/tests/test_tasknode_unl_trust_graph.py`).

### C. Validator-key-to-wallet binding CLI

**Targets:** `python/postfiat_rpc/tasknode_unl_binding.py`,
`python/postfiat_rpc/tasknode_unl.py`,
`python/tests/test_tasknode_unl_binding.py`, and
`python/tests/fixtures/tasknode_unl/bindings.json`.

- [x] Add an offline CLI workflow that prepares canonical, domain-separated
      challenge bytes; obtains a validator-master-key signature and a Task Node
      wallet countersignature through custody-preserving signer adapters;
      verifies both; and prints the bounded PFT Ledger memo payload. The CLI
      never accepts, prints, logs, or persists either private key
      (`python/postfiat_rpc/tasknode_unl.py`).
- [x] Emit the new evidence fields
      `validator.identity.tasknode_binding.wallet_address`,
      `validator.identity.tasknode_binding.tx_hash`,
      `validator.identity.tasknode_binding.challenge_digest`,
      `validator.identity.tasknode_binding.validator_signature`, and
      `validator.identity.tasknode_binding.wallet_signature`
      (`python/postfiat_rpc/tasknode_unl_schema.py`).
- [x] Implement replay rules: one wallet binds at most one validator; a second
      binding attempt is shared-control evidence; a later valid memo from the
      same wallet supersedes the earlier binding; a memo signed by either key
      revokes; and a validator-key rotation must be rebound within one
      evaluation window or hold
      (`python/postfiat_rpc/tasknode_unl_binding.py`).
- [x] On validator-side revocation of a compromised wallet, freeze that wallet's
      work history at the revocation ledger index. Permit reattachment to a new
      wallet only when two accounts that held co-work edges with the old wallet
      publish valid vouches; otherwise hold
      (`python/tests/test_tasknode_unl_binding.py`).
- [x] Keep memo creation separate from transaction preparation or submission.
      No CLI subcommand may send a memo, contact Task Node, mutate a registry,
      or invoke Cobalt (`python/postfiat_rpc/tasknode_unl.py`).

### D. Signed work digest and verifier

**Targets:** `python/postfiat_rpc/tasknode_unl_work_digest.py`,
`python/tests/test_tasknode_unl_work_digest.py`,
`python/tests/fixtures/tasknode_unl/work-digests.json`,
`python/tests/fixtures/tasknode_unl/ledger-pointers.json`, and
`python/tests/fixtures/tasknode_unl/publishing-keys.json`.

- [x] Define a canonical signed digest per account and evaluation window. It
      contains the ordered `pf.ptr/v4` pointer hashes counted, the outcome
      assigned to each pointer, the resulting inputs to all five accountability
      terms, its schema and window, the Task Node publishing-key identity, and
      the digest's PFT Ledger anchor transaction hash
      (`python/postfiat_rpc/tasknode_unl_work_digest.py`).
- [x] Verify the publishing-key signature, schema, bounds, canonical order,
      account/window binding, digest hash, and frozen-view anchor transaction
      before using any score input
      (`python/postfiat_rpc/tasknode_unl_work_digest.py`).
- [x] Reconcile every listed pointer against a frozen PFT Ledger view: it must
      exist, match the claimed hash and window, and have been emitted by the
      wallet bound to the validator. Any mismatch, duplicate, unknown outcome,
      wrong sender, or missing pointer holds
      (`python/tests/test_tasknode_unl_work_digest.py`).
- [x] State the evidence limit honestly: public reconciliation proves existence
      and sender of counted pointers, not the encrypted review outcome or
      completeness of eligible tasks. Do not read the live Task Node database
      or claim the signed digest removes the Foundation attestation boundary
      (`python/postfiat_rpc/tasknode_unl_work_digest.py`).

### E. Public edge extractors

**Targets:** `python/postfiat_rpc/tasknode_unl_edges.py`,
`python/tests/test_tasknode_unl_edges.py`,
`python/tests/fixtures/tasknode_unl/vouch-memos.json`,
`python/tests/fixtures/tasknode_unl/cowork-pointers.json`,
`python/tests/fixtures/tasknode_unl/funding-transfers.json`, and
`python/tests/fixtures/tasknode_unl/funding-exclusions.json`.

- [x] Extract vouch edges only from verified signed PFT Ledger memo statements.
      Do not consume Nostr events in this MVP, and never consume private
      messages (`python/postfiat_rpc/tasknode_unl_edges.py`).
- [x] Extract one co-work unit per shared Hive project or Team grant from
      replayed pointers, preserving project/grant provenance and applying the
      walk's aggregate cap of three
      (`python/postfiat_rpc/tasknode_unl_edges.py`).
- [x] Extract a funding relation when one wallet was the other's first funder
      or when more than 50% of either wallet's inbound value over the evaluation
      window came from the other
      (`python/postfiat_rpc/tasknode_unl_edges.py`).
- [x] Require a versioned, published exclusion-list fixture for exchange and
      Foundation distribution addresses. Missing, stale, or malformed
      exclusions hold funding extraction; excluded addresses never create a
      funding edge
      (`python/tests/fixtures/tasknode_unl/funding-exclusions.json`).
- [x] Canonicalize and deduplicate evidence without erasing provenance. Test
      forged vouches, private-message-shaped input, repeated projects, Team
      grants, exactly-50% versus greater-than-50% funding, first-funder
      poisoning, exclusions, and conflicting source rows
      (`python/tests/test_tasknode_unl_edges.py`).

### F. Churn and overlap guard

**Targets:** `python/postfiat_rpc/tasknode_unl_churn.py`,
`python/tests/test_tasknode_unl_churn.py`,
`python/tests/fixtures/tasknode_unl/registry-rounds.json`, and
`python/tests/fixtures/tasknode_unl/baseline-list.json`.

- [x] Accept at most one list change per round—one addition or one removal, not
      both—while the list has fewer than 39 validators. At or above 39, continue
      to enforce the supplied existing trust-graph transition budget rather
      than inventing a new budget
      (`python/postfiat_rpc/tasknode_unl_churn.py`).
- [x] Reject a derivation whose registry root is more than one round old, and
      bind the baseline list, current root, source round, and target round into
      the result (`python/postfiat_rpc/tasknode_unl_churn.py`).
- [x] Encode and test the proposal's overlap math, where overlap is intersection
      divided by union. A 20-validator swap gives `19/21 ~= 90.5%` for a node
      one round behind and `18/22 ~= 81.8%` two rounds behind; a swap first
      reaches 95% at `N=39` because `(N-1)/(N+1) >= 0.95`. At `N=20`, a
      single removal gives `19/20 = 95%`, a single addition gives
      `20/21 ~= 95.2%`, and the two-removal lag floor is
      `18/20 = 90%`
      (`python/tests/test_tasknode_unl_churn.py`).
- [x] Treat a newly failed identity condition as a hold first. It becomes
      eligible for a removal candidate only after a complete evaluation window
      and remains subject to the one-change and stale-root guards
      (`python/postfiat_rpc/tasknode_unl_churn.py`).

### G. Fixture-driven shadow derivation runner

**Targets:** `python/postfiat_rpc/tasknode_unl.py`,
`python/postfiat_rpc/tasknode_unl_policy.py`,
`python/tests/test_tasknode_unl.py`,
`python/tests/fixtures/tasknode_unl/expected-shadow-output.json`, and
`python/tests/fixtures/tasknode_unl/expected-shadow-output.md`.

- [x] Wire bindings, work digests, ledger pointers, edge evidence, exclusions,
      the ratified seed list, existing Admission Policy V1 evidence, registry
      roots, and a baseline list into one offline command:
      `PYTHONPATH=python python3 -m postfiat_rpc.tasknode_unl shadow-derive --input-dir python/tests/fixtures/tasknode_unl --output shadow.json`
      (with `derive` and `--fixture-dir` retained as aliases;
      `python/postfiat_rpc/tasknode_unl.py`).
- [x] Derive accountability and graph results, connectivity holds, cluster
      assignments, seat-cap holds, one-wallet/shared-funding control groups, and
      a conservative V1-compatible `rho_score`: `0` only after all required
      independence evidence passes, positive on detected correlation, and
      missing on incomplete or conflicting evidence so the selector holds
      (`python/postfiat_rpc/tasknode_unl_policy.py`).
- [x] Project those results into the existing
      `ValidatorAdmissionEvidencePacket` and
      `ValidatorAdmissionDecision` field shape without changing Admission
      Policy V1. Apply its existing `accountability_score >= 70`,
      `rho_score <= 0`, and no-shared-control-group gates; carry the fixture's
      existing uptime, operator-manifest, key-domain, and
      `cobalt.linkedness_safe` facts unchanged
      (`python/postfiat_rpc/tasknode_unl_policy.py`).
- [x] Emit a canonical JSON shadow result containing the input roots, constants,
      per-candidate calculations, holds/rejections, eligible set, churn-limited
      shadow candidate list, and a baseline diff. Every addition, removal, or
      hold-induced difference must include at least one stable reason code and
      the evidence references that caused it
      (`python/tests/fixtures/tasknode_unl/expected-shadow-output.json`).
- [x] Also support a human-readable Markdown rendering from the verified JSON
      so an operator can understand the candidate list and every difference
      after the CLI works. Both formats must say `SHADOW_ONLY`, no live
      authority, no transaction, and no ratification
      (`python/tests/fixtures/tasknode_unl/expected-shadow-output.md`).
- [ ] After all fixture cases pass, attempt one read-only real-data derivation
      using frozen local exports or bounded read-only PFT Ledger history plus
      the current baseline. Do not access Task Node servers. If signed digests,
      bindings, publishing keys, exclusions, or required V1 facts are absent,
      emit an honest coverage-and-holds report; never synthesize missing data.
- [x] Prove repeated runs over identical inputs are byte-identical. Tampered
      signatures, altered pointer ownership, a stale root, forbidden inputs,
      missing evidence, an over-cap cluster, and a two-change pre-39 delta must
      all fail closed without partial output
      (`python/tests/test_tasknode_unl.py` and the focused A–F tests).

### H. Fresh-context adversarial review

**Targets:** `python/postfiat_rpc/tasknode_unl.py`,
`python/postfiat_rpc/tasknode_unl_schema.py`,
`python/postfiat_rpc/tasknode_unl_accountability.py`,
`python/postfiat_rpc/tasknode_unl_trust_graph.py`,
`python/postfiat_rpc/tasknode_unl_binding.py`,
`python/postfiat_rpc/tasknode_unl_work_digest.py`,
`python/postfiat_rpc/tasknode_unl_edges.py`,
`python/postfiat_rpc/tasknode_unl_churn.py`,
`python/postfiat_rpc/tasknode_unl_policy.py`; their exact test and fixture paths
listed in A–G.

- [ ] At the end of the implementation day, give a fresh-context reviewer the
      source proposal, this plan, the complete diff, and focused test commands.
      The reviewer must not rely on the implementation author's unstated
      assumptions.
- [ ] Require adversarial findings for score arithmetic and window boundaries;
      canonicalization and iteration determinism; graph direction, dangling
      rows, conductance, connectivity, and cluster caps; signature domains and
      key custody; supersession, revocation, rotation, and shared control;
      pointer reconciliation and double counting; exclusion-list and funding
      attacks; stale roots, churn, and overlap; forbidden/private inputs; and
      `SHADOW_ONLY` labeling.
- [ ] Fix every correctness or safety finding, rerun the focused Python suite,
      rerun the byte-identical fixture derivation, and record unresolved
      specification ambiguities as holds rather than silently choosing live
      semantics.
- [ ] Run no workspace-wide or Orchard/Halo2 suite for this Python-only,
      shadow-only slice. A broad suite is reserved for an explicit milestone or
      release gate if later work crosses those boundaries.

## Focused completion gates

- [ ] `PYTHONPATH=python python3 -m pytest python/tests/test_tasknode_unl*.py`
      passes.
- [ ] The canonical fixture derivation is byte-identical across repeated runs
      and matches `expected-shadow-output.json`.
- [ ] The binding CLI emits and verifies a memo payload but exposes no submit
      path and no secret material.
- [ ] The read-only real-data attempt either emits a valid shadow diff or an
      explicit coverage-and-holds report; absence of required evidence is not
      treated as success.
- [ ] JSON and Markdown outputs both carry the complete
      `SHADOW_ONLY`/no-authority boundary.
- [ ] Fresh-context adversarial review is closed or every open finding is named
      as a blocker.
- [ ] Existing Cobalt, validator-registry, governance, Orchard/Halo2, Task Node,
      and website files remain unchanged by implementation except for later
      concise documentation explicitly approved for that milestone.

## Rollout-phase mapping

| Proposal phase | This MVP |
| --- | --- |
| Phase 0 | Implements the published scoring constants, validator-key-to-wallet binding memo builder, signed work-digest format/verifier, and new binding evidence fields in an offline Python reference path. Voluntary live binding and memo submission are not authorized. |
| Phase 1 | Supplies the fixture-driven weekly shadow-list shape, baseline diff, and reasons. One read-only real-data attempt may measure evidence coverage; it is not a production weekly round. |
| Phase 2 | Implements the proposed vouch, co-work, and funding extractors plus the trust walk and cluster controls early so the end-to-end shadow runner can be exercised. It does not collect real vouches or satisfy the three-round exit criterion. |
| Phase 3 | Not started. There is no independent proposer/ratifier qualification, five-percent reviewer replay, promotion decision, Cobalt submission, or registry authority. |

## Out of scope today

- World ID or any other personhood provider.
- Nostr vouch events, NIP-17 messages, or any private-message-derived edge.
- Task Node server, database, worker, badge-routing, task-lifecycle, account, or
  wallet changes.
- On-chain writes, memo submission, wallet spending, live vouch collection,
  Cobalt proposals, ratification, registry mutation, or deployment.
- Changing Admission Policy V1, consensus, execution, storage, RPC, Orchard,
  Halo2, or release behavior.
- Claiming decentralization, one-person-one-seat, production eligibility, or
  correctness of encrypted Task Node review outcomes.
