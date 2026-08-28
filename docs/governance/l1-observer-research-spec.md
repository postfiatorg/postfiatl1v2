# L1 Observer Service Research Specification

**Status:** Text Improvement Harness full gate passed on 2026-08-28 — average 89.40/100 (GPT 91.60, Fable 86.60, GLM 90.00; five runs per lane; run group `l1-observer-research-spec`); scored content SHA-256 `3041bdba3416da3a5a7d255b9c9c2c9cc89c5477dfd50a176d8cc1b7b165cb6e`; Task Node lock pending the operator's decision

**Date:** 2026-08-28

**Author:** Domagoj Ravlić (dravlic)

**Decision owner:** Post Fiat

**Related:** [Dynamic UNL L1 Evidence-Source Decision Note](dynamic-unl-l1-evidence-source-note.md), [Validator Evaluator Alternatives Decision Note](validator-evaluator-alternatives-note.md), [Dynamic UNL Proposal Source Research Specification](dynamic-unl-proposal-source-research-spec.md), and [Deferred Dynamic UNL Proposal Source Milestone](../deferred-plans/dynamic-unl-proposal-source-milestone.md)

## Plain-English directive

Build and test the smallest read-only L1 observer that can freeze scoring-window evidence. The observer sits beside a Post Fiat node. It records signed Consensus v2 round records. It records Cobalt certificates and ratification events. It records validator uptime, liveness, topology, and membership observations. It preserves enough raw data to reproduce every emitted field. It attributes validator records through the active ML-DSA-65 registry key. It emits one versioned L1 evidence profile. Independent observers must be able to merge their views deterministically. The observer grants no authority. It mutates no node, registry, Cobalt, consensus, or scoring state. Every packet and derived output is a SHADOW_ONLY input. This specification decides whether that evidence path is fit for shadow research. It does not authorize production selection, admission, removal, or quorum changes.

## Claims, evidence, and gaps

| Claim | Evidence now | Gap to close |
|---|---|---|
| Consensus v2 has signed round evidence. | ConsensusV2Proposal, ConsensusV2Vote, ConsensusV2TimeoutVote, quorum certificates, timeout certificates, and ConsensusV2Commit carry bound round data. | No observer freezes all relevant records across a scoring window. |
| Finalized history is replayable. | A commit binds the proposal, prior certificates, timeout certificate, prepare QC, and precommit QC. | Non-final and conflicting records can be lost without a read-only event export. |
| Validator attribution is verifiable. | Validator registry entries bind validator IDs to algorithm IDs and public keys. Consensus verification uses the committee epoch and registry view. | Evidence packets do not yet freeze the exact registry root and key interval for each record. |
| Cobalt has signed governance evidence. | Cobalt authorization records, certificates, protocol transcripts, and validator update records carry ML-DSA-bound evidence. | There is no scoring-window export of participation and ratification events. |
| Registry changes are durable governance facts. | ValidatorRegistryUpdateRecord and Cobalt validator update certificates bind activation, supersession, rollback, and registry data. | Membership intervals are not normalized for the scoring pipeline. |
| Liveness is observable. | A node exposes finalized progress and peer or RPC reachability from its local vantage. | Liveness is not a globally signed chain fact and needs explicit observer provenance and denominators. |
| Topology is partially observable. | NetworkTopology and peer handshakes expose configured membership and observed connections. | Configured topology is not proof of public infrastructure, operator, geography, or independence. |
| The scoring pipeline is deterministic after normalization. | DeterministicFinalScore uses pinned integer arithmetic over five normalized subscores. | No L1 evidence adapter supplies those normalized inputs. |
| The sidecar survives restarts. | It persists input, commitment, salt, reveal state, and chain cursor before acting. | L1 evidence capture needs the same persist-before-advance discipline without commit authority. |
| Several observers can expose conflicts. | The Phase 3 charter requires independently administered, signed, content-addressed packages and deterministic union. | No L1 observer federation rule or controlled devnet result exists. |
| A retained gap is material. | A scoring result depends on complete counts and declared missingness. | Current history availability is not guaranteed for the longest scoring window. |

The main unknown is not whether records exist. The main unknown is whether independent services can retain, verify, normalize, and replay them without becoming an authority.

## Decision question

Can a read-only, federated L1 observer produce complete and reproducible SHADOW_ONLY evidence for one full scoring window? The answer must cover four properties:

1. Every derived field traces to retained raw records.
2. Every validator attribution traces to the correct registry key interval.
3. Independent observers converge or expose their differences.
4. The existing scoring pipeline replays deterministically from the emitted profile.

## Scope

### In scope

- Signed Consensus v2 proposals.
- Signed prepare and precommit votes.
- Signed timeout votes.
- Quorum certificates and timeout certificates.
- Final ConsensusV2Commit records.
- Invalid, duplicate, late, and conflicting signed round records seen by an observer.
- Signed Cobalt protocol messages available to the node.
- CobaltCertificate and NonUniformGovernanceCertificate records.
- Ratified Cobalt governance records.
- Activation, supersession, rollback, and validator update events.
- Validator registry roots, epochs, entries, and key rotations.
- Membership activation and removal intervals.
- Finalized-height progress.
- Expected and observed validator participation counts.
- Read-only RPC and peer reachability probes.
- Observed peer and topology changes.
- Configured topology as a separate, labeled input.
- Raw-record retention across the longest active scoring window.
- Observer identity, ownership, signatures, and federation.
- Restart, catch-up, gap, and incomplete-window behavior.
- A versioned L1 evidence-field contract.
- A Python CLI and a read-only human interface.
- Feeding the profile into the existing shadow scoring path.

### Out of scope

- Changing Consensus v2 safety or liveness rules.
- Changing Cobalt nomination, ratification, or activation rules.
- Changing validator admission or removal.
- Changing the validator registry.
- Changing quorum, threshold, or committee membership.
- Replacing DeterministicFinalScore.
- Giving an observer a validator signing key.
- Giving an observer a governance signing key.
- Treating configured topology as verified operator independence.
- Discovering records that every observer and every retained node omitted.
- Resolving legal ownership, funding, geography, or identity from network traffic alone.
- Publishing raw IP addresses or private peer data to model inputs.
- Creating an authority-bearing score or UNL proposal.
- Live commit or reveal submission.
- Production rollout.

## Responsibility split

| Surface | Existing or proposed module | Responsibility |
|---|---|---|
| Consensus wire records | crates/types/src/consensus_v2_types.rs | Defines proposals, votes, timeout votes, certificates, commits, domains, rounds, phases, and signatures. |
| Consensus verification | crates/consensus/src and crates/node/src/consensus_v2_store.rs | Verifies domains, signatures, committee membership, and certificate thresholds. |
| Cobalt records | crates/consensus_cobalt/src/core_types.rs | Defines Cobalt proposals, votes, certificates, memberships, and decision bindings. |
| Registry and governance records | crates/types/src/shielded_bridge_governance.rs | Defines registry entries, signed authorizations, update records, and lifecycle events. |
| Network observations | crates/network/src/lib.rs and crates/node/src/transport_protocol.rs | Supplies configured topology and locally observed peer state. |
| Evidence record types | new: crates/types/src/validator_evidence_round.rs | Defines the canonical raw-record envelope, window manifest, field contract, and conflict proofs. |
| Read-only event export | new: crates/node/src/validator_evidence_export.rs | Exposes verified and rejected observation records without a write method. |
| Observer service | new: crates/node/src/validator_evidence_observer.rs | Captures, verifies, stores, checkpoints, catches up, and emits signed manifests. |
| Observer storage | new: crates/node/src/validator_evidence_store.rs | Stores append-only raw records, canonical indexes, cursors, gaps, and window roots. |
| Federation | new: crates/node/src/validator_evidence_federation.rs | Verifies observer manifests and builds a deterministic union with difference reports. |
| L1 scoring adapter | new: dynamic-unl-scoring/scoring_service/sources/postfiat_l1.py | Maps the L1 contract into the existing evidence profile without changing score arithmetic. |
| Sidecar input verification | validator-scoring-sidecar/src/validator_scoring_sidecar/input_package.py and verification.py | Verifies the frozen input package and preserves its hash through score, commit, and reveal research. |
| Human tooling | new: python/postfiat_rpc/l1_observer.py | Verifies, compares, replays, and explains packets. |

The module paths marked new do not exist. They are proposed ownership boundaries, not implementation claims.

## Shared invariants

### I1. Read-only authority boundary

The service has no node write RPC. It holds no validator or governance private key. It cannot submit transactions, votes, proposals, or registry updates. Its own key signs only observer manifests and observation claims. Every output declares SHADOW_ONLY. Consumers must reject a missing or different authority mode.

### I2. Domain-bound source records

Every consensus record retains its original chain, genesis, protocol, committee epoch, height, view, phase, parent, payload, and state bindings. The observer does not replace validator signatures. It verifies source signatures against the registry view active for that record. The profile pins ML-DSA-65 as the accepted validator attribution algorithm for this experiment. Unsupported algorithm IDs remain raw evidence but make the derived validator field ineligible.

### I3. Raw before derived

The observer persists a raw record before advancing its durable cursor. Derived counters refer to canonical raw record IDs. Raw bytes remain available for replay. Normalization never erases invalid, duplicate, late, or conflicting records. Redaction creates a derived publication view. It does not change the retained raw archive.

### I4. Canonical identity and hashes

The profile defines canonical encoding before hashing. A raw record ID commits to its type, source domain, canonical bytes, and source signature. A window root commits to the ordered record IDs, gap ledger, registry snapshots, and observer manifest. All hashes include an algorithm identifier. No JSON map iteration order may affect a hash.

### I5. Exact window boundary

Each packet names a profile version and scoring-window ID. It binds inclusive start and end finalized heights. It also records first-seen and last-seen UTC times. Heights decide chain inclusion. Times describe liveness observations and current 1-hour, 24-hour, and 30-day profile horizons. The packet records the expected height set and every known gap.

### I6. Retention

Raw records remain available from the start of the longest active scoring horizon. For the current profile, that horizon is 30 days. Records remain until the window is sealed and its verification and challenge grace has expired. The grace duration is a governed profile value. No record referenced by an unexpired manifest may be pruned. Operators may retain longer. A catch-up gap older than available history yields INCOMPLETE_WINDOW. The observer must not clamp the cursor forward and claim completeness.

### I7. Membership intervals

Each validator row binds validator ID, registry root, committee epoch, algorithm ID, public-key hash, and active height interval. Key rotations create a new attribution interval. Activation, removal, supersession, and rollback stay explicit. Counts use the membership interval active at the relevant height and view.

### I8. Observational fields stay observational

Liveness and topology fields name the observer, vantage, method, numerator, denominator, and interval. Configured membership is separate from observed connectivity. An IP address, host label, cloud, geography, or operator claim is not promoted to verified identity without its declared evidence. Private addresses are retained under operator policy and omitted or salted in published packets.

### I9. No silent missingness

Missing records do not become zero misconduct. Every field has PRESENT, MISSING, CONFLICTING, INCOMPLETE_WINDOW, or NOT_APPLICABLE state. Every PRESENT field carries source record references. Score adapters fail closed when a required field is not PRESENT.

### I10. Deterministic federation

The federation verifies source records before union. Byte-identical valid records collapse to one record with an observer provenance set. The order is canonical by height, view, phase, record type, validator ID, and record ID. Two valid records for the same signed slot with different digests form an equivocation proof. A record present in the valid union but absent from one eligible observer forms an omission report for that observer. Observer-specific probes remain separate observations. No majority vote changes a signed source fact.

### I11. Independent administration

One observer is not enough. It can omit, delay, corrupt, or selectively publish records without an external comparison. The experiment uses at least three independently administered observers. They use separate credentials, storage, and publication paths. The exact production independence threshold is not decided here.

### I12. Replay

Given raw records, profile version, registry history, and window bounds, a clean verifier reproduces every derived field and root. Given the normalized profile and pinned score code, a clean scorer reproduces the same final integer scores and selected shadow set. Replay does not call a live node.

### I13. Stable failure

Restart is at-least-once. Duplicate delivery is harmless. Cursor advancement follows durable raw storage. Unknown schema, broken signature, broken hash, missing registry view, or retained-history gap fails the affected field or window closed. The failure is published as evidence.

## L1 evidence-field contract

The first profile is postfiat_l1_evidence_v1. The contract has six layers.

### Window envelope

- schema_version, source_profile, and authority_mode equal to SHADOW_ONLY
- network_id, chain_id, genesis_hash, and protocol_version
- scoring_window_id, start_height, end_height, start_block_id, and end_block_id
- observed_start_utc, observed_end_utc, and longest_horizon_seconds
- verification_grace_seconds and observer_manifest_refs
- raw_record_root, registry_history_root, gap_ledger_root, and derived_fields_root

### Observer identity

- observer_id, observer_public_key, and observer_signature_algorithm
- operator_id, administration_id, infrastructure_id, and credential_id
- software_release, configuration_hash, and capture_vantage
- signed_manifest_hash

These slots make independence review possible. They do not prove independence by themselves.

### Validator attribution

- validator_id, committee_epoch, registry_root, and registry_entry_hash
- validator_algorithm_id and validator_public_key_hash
- active_from_height and active_through_height
- activation_record_ref and removal_or_rotation_record_ref
- attribution_state

### Consensus and Cobalt fields

- consensus_expected_prepare_count and consensus_signed_prepare_count
- consensus_expected_precommit_count and consensus_signed_precommit_count
- consensus_expected_proposal_count and consensus_signed_proposal_count
- consensus_timeout_vote_count and consensus_late_vote_count
- consensus_invalid_record_count and consensus_equivocation_count
- consensus_finalized_commit_refs
- cobalt_eligible_event_count, cobalt_signed_event_count, and cobalt_missed_event_count
- cobalt_certificate_refs and cobalt_ratification_refs
- cobalt_activation_refs, cobalt_supersession_refs, and cobalt_rollback_refs

Every count carries its denominator rule and raw record references.

### Liveness, topology, and change fields

- finalized_height_progress_numerator and finalized_height_progress_denominator
- rpc_probe_success_count and rpc_probe_total_count
- peer_probe_success_count and peer_probe_total_count
- restart_observation_count and observation_interval_seconds
- configured_topology_hash and observed_peer_set_hash
- observed_peer_change_refs and registry_membership_change_refs
- topology_claim_refs and vantage_count

Ratios use integer basis points only after numerator and denominator are frozen.

### Quality and federation fields

- field_state and source_record_refs
- reporting_observer_ids and missing_observer_ids
- conflict_record_refs, omission_report_refs, and equivocation_proof_refs
- first_seen_height, first_seen_utc, and last_seen_utc
- canonicalization_version and replay_tool_version

## Mapping to the existing scoring profile

The L1 adapter produces the existing five model inputs. Consensus agreement windows feed the consensus subscore evidence. Finalized progress and reachability feed reliability evidence. Observed protocol and release status feed software evidence. Topology claims feed diversity evidence only with provenance and quality state. Registry identity and key continuity feed identity evidence. The adapter must preserve the current 1-hour, 24-hour, and 30-day window labels. It must preserve raw numerators, denominators, missingness, and conflict flags. It may not infer a favorable value from missing data. DeterministicFinalScore remains unchanged. The observer does not calculate or endorse a final score.

## Operational shape

The service runs as an unprivileged process beside a node. It may use a local read-only socket or authenticated read-only RPC. Its required inputs are:

1. Finalized block and commit history.
2. A read-only stream of verified and rejected Consensus v2 round records.
3. A read-only stream of Cobalt protocol and ratification records.
4. Validator registry snapshots and update history.
5. Finalized-height and local reachability observations.
6. Configured and observed topology events, labeled separately.

The node needs a new read-only export for records not preserved in finalized commits. The export has no request that changes node state. The observer keeps its own append-only raw archive. It keeps canonical indexes by height, view, phase, validator, and record ID. It checkpoints the last fully persisted source position. On restart, it replays from or before that checkpoint. It deduplicates by canonical record ID. It catches up from retained node history. It seals no window until catch-up reaches its end height. If required history was pruned, it emits INCOMPLETE_WINDOW and the exact missing range. It does not borrow an unverified peer packet to hide the gap.

## Identity and ownership slots

| Slot | Research status | Meaning |
|---|---|---|
| L1 observer service owner | Unassigned | Owns capture, storage, restart, and release safety. |
| Evidence contract owner | Unassigned | Owns schema versioning and canonical encoding. |
| Federation policy owner | Unassigned | Owns eligible observer and independence rules. |
| Observer operator identity | Per deployment | Signs manifests with a non-validator key. |
| Archive custodian | Per deployment | Owns retention and recovery evidence. |
| Node export owner | Post Fiat assignment required | Owns the read-only node boundary. |
| L1 scoring adapter owner | Unassigned | Owns deterministic normalization only. |
| Sidecar integration owner | Unassigned | Owns input verification and replay plumbing. |
| Python CLI owner | Unassigned | Owns human verification workflows. |
| Read-only UI owner | Unassigned | Owns public explanation without control paths. |
| Decision owner | Post Fiat | Decides the research gate. |

No implementation milestone may hide an unassigned slot. The milestone must record the named owner before production work for that slot.

## Experiments

### E1. Evidence contract and raw replay

Implement a minimal canonical contract fixture. Capture signed consensus records, Cobalt records, registry history, liveness samples, and topology observations. Include valid, invalid, duplicate, late, missing, and conflicting cases. Build the derived packet from retained raw records. Delete the derived packet. Rebuild it in a clean verifier from raw records only. Compare every field and root.

**Required result:** Two clean replays produce byte-identical derived packets and roots. Every PRESENT field resolves to retained raw records. Invalid attribution, missing registry history, or a raw gap fails closed.

### E2. Multi-observer convergence

Run at least three independently administered observers on the controlled devnet. All observers remain read-only. Exercise normal delivery, delay, loss, partition, selective omission, duplicate delivery, and signed equivocation. Merge manifests with the deterministic federation rule. Compare union roots and difference reports across clean federation implementations. Record the limits when all observers miss the same event.

**Required result:** Valid shared records converge to the same canonical union. Selective omission names the missing observer and record IDs. Signed equivocation retains both records and produces a proof. Probe disagreement remains attributed to each vantage. No observer output changes L1 state.

### E3. Window retention, restart, and catch-up

Exercise the longest active scoring horizon. The current fixture covers a 30-day window and the configured verification grace. Restart observers before, during, and after catch-up. Replay duplicate pages and interrupted writes. Catch up from retained finalized and round history. Then remove a required historical range in a negative fixture. Measure retained bytes, catch-up duration, index rebuild duration, and peak backlog.

**Required result:** Complete history yields the same raw root before and after every restart. Cursor advancement never precedes durable raw storage. Duplicate delivery changes no count. Missing retained history yields a stable INCOMPLETE_WINDOW with an exact gap and no score-eligible packet.

### E4. L1 profile and deterministic score replay

Map a sealed observer federation packet to postfiat_l1_evidence_v1. Feed it through the existing dynamic scoring input path. Run the pinned model-input construction and DeterministicFinalScore replay. Run the same fixture through the sidecar input verifier. Repeat on clean machines or isolated environments. Exercise source-profile mismatch, altered roots, missing fields, and conflicting fields. Do not submit a live commit or reveal.

**Required result:** Identical L1 packets produce identical normalized inputs, integer scores, selected shadow sets, and output hashes. Profile or root mismatch is rejected. Required missing or conflicting fields fail closed. Every artifact remains SHADOW_ONLY.

## Gates

### ADOPT FOR SHADOW

Choose ADOPT FOR SHADOW only when all four experiments meet their Required result. The gate permits an L1 evidence profile as a shadow scoring input. It does not permit authority-bearing proposals or validator changes. All owners needed for the next milestone must be named.

### REMEDIATE

Choose REMEDIATE when failures are bounded and explainable. Examples include an incomplete field map, a recoverable index bug, an explicit retention shortfall, or a deterministic canonicalization mismatch. Record the failed invariant, owner, repair, and repeat experiment. Do not widen authority while remediating.

### REJECT

Choose REJECT for any unbounded or safety-relevant failure. Examples include accepting a forged source record, losing raw lineage, hiding a history gap, nondeterministic replay, undetected signed equivocation, silent selective omission, observer-controlled node mutation, or an output that can activate authority. Rejection leaves Option C on its existing non-L1 shadow source.

## Required evidence packet

The gate packet must contain:

- The exact source revision and dirty-state report.
- The exact reference-clone revisions used for compatibility review.
- The profile schema and canonicalization rules.
- The raw fixture manifest and hashes.
- The registry history fixture and root.
- The observer identities and declared independence slots.
- Signed observer manifests.
- Raw record, gap ledger, registry, and derived roots.
- Per-field provenance reports.
- Federation union and difference reports.
- Omission reports and equivocation proofs.
- Restart and catch-up logs.
- Retention size and duration measurements.
- Negative-fixture results.
- Clean replay commands and outputs.
- Normalized scoring inputs and hashes.
- DeterministicFinalScore outputs and hashes.
- Sidecar verification output.
- CLI output.
- Read-only UI captures.
- A statement that no live commit, reveal, registry change, or authority action occurred.
- The ADOPT FOR SHADOW, REMEDIATE, or REJECT decision.

Large raw archives may be content-addressed outside Git. The packet must include durable locations and hashes. Logs must not contain private keys, credentials, or unredacted private topology.

## Human interfaces

The Python CLI is required first. The proposed entry point is python -m postfiat_rpc.l1_observer. It must support:

- verify for signatures, roots, windows, and registry attribution;
- inspect for a concise window and validator summary;
- trace for one derived field back to raw records;
- compare for two observer manifests or federation packets;
- replay for rebuilding a derived packet offline;
- gaps for missing ranges and incomplete fields; and
- export for a redacted SHADOW_ONLY scoring packet.

The CLI defaults to read-only local files. It refuses an output whose authority mode is not SHADOW_ONLY.

After the CLI works, provide a read-only user-facing interface. The interface shows window bounds, freshness, completeness, observer provenance, registry intervals, field states, omissions, equivocations, and replay hashes. It exposes no vote, proposal, transaction, registry, commit, or reveal control. Its network methods are GET or equivalent read-only queries.

## Required publication

Publish a concise research result under docs/governance. Publish the versioned evidence-field contract. Publish the canonicalization and hash rules. Publish redacted fixtures for all experiment classes. Publish the federation merge rule. Publish the retention and failure policy. Publish replay instructions for the CLI. Publish the selected gate and unresolved gaps. If adopted for shadow, update the L1 evidence-source decision note. Then create the milestone document through the mandated Task Node lifecycle. That action remains pending the operator's separate decision.

## Decisions recorded by this specification

1. The first L1-native component is a read-only observer.
2. Finalized and non-final signed records are retained when available.
3. Validator attribution uses the registry view active for the signed record.
4. ML-DSA-65 is the validator attribution profile for this experiment.
5. Observer signatures attest observation and publication, not consensus truth.
6. Liveness and topology remain vantage-bound evidence.
7. One observer is insufficient.
8. The experiment begins with at least three independently administered observers.
9. Federation is a deterministic verified union, not majority rewriting.
10. Omission is measured against the valid union.
11. Equivocation retains all valid conflicting signed records.
12. Records missed by every observer remain an explicit limitation.
13. The current longest scoring horizon is 30 days.
14. Retention also covers the configured verification grace.
15. A pruned catch-up gap makes the window incomplete.
16. DeterministicFinalScore remains unchanged.
17. All observer and scoring outputs remain SHADOW_ONLY.
18. This specification grants no authority and changes no protocol state.

## Work sequence

- [ ] Score this exact research specification with the Text Improvement Harness full gate.
- [ ] Lock it immediately if the first compliant average is at least 86/100.
- [ ] If below 86/100, use the harness critiques in one direct OpenRouter rewrite pass.
- [ ] Re-score only the rewritten content, with no more than two improvement loops.
- [ ] Record the final score and scored-content SHA-256 in the Status line.
- [ ] Await the operator's decision on the Task Node lock.
- [ ] Request one substantial Task Node milestone only after that decision.
- [ ] Assign every required ownership slot.
- [ ] Implement the CLI before the read-only UI.
- [ ] Run E1 through E4 in order.
- [ ] Publish the evidence packet and gate decision.
- [ ] If adopted, keep the source SHADOW_ONLY.
- [ ] Retire the milestone only after the CLI, UI, and concise documentation work.
