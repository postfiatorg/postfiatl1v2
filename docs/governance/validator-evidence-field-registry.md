# Validator Evidence Field Registry

Status: initial registry for DGA validator evidence rules.
Date: 2026-05-23

This registry defines the validator evidence fields that Qwen-generated
governance rules may reference.

The purpose is narrow: make the evidence universe explicit before packet
schema implementation. A generated governance rule is valid only if every
evidence reference appears in this registry or in a later governed revision.

## Rule

Qwen may not invent evidence fields.

Every DGA rule that reads validator evidence must cite a registered field path,
minimum provenance, freshness requirement, missing-evidence behavior,
conflict behavior, and action bound.

## Field Path Model

Rules evaluate one validator at a time unless explicitly marked as packet or
cohort scope.

| Prefix | Scope | Meaning |
| --- | --- | --- |
| `packet.*` | Whole packet | Collection window, schema, collector, packet roots. |
| `validator.*` | One validator | Identity, registry state, performance, Cobalt history, topology, quality flags. |
| `cohort.*` | Validator set | Concentration and diversity calculations across validators. |

## Provenance Levels

| Provenance | Strength | Meaning |
| --- | --- | --- |
| `chain_derived` | Strong | Computed from finalized chain, ordered batches, receipts, or Cobalt history. |
| `registry_derived` | Strong | Computed from active or historical validator registry state. |
| `operator_signed` | Medium | Signed by a validator, governance, or operator accountability key. |
| `collector_observed` | Medium | Observed by an approved PostFiat collector and committed by hash. |
| `network_observed` | Medium | Observed through node, peer, RPC, or transport behavior. |
| `self_asserted` | Weak | Supplied by the operator without independent observation. |
| `third_party_attested` | Medium or strong | Supplied by an approved external attestation source. |

Rules may require a minimum provenance. A rule may accept weaker provenance
only if its action bound is correspondingly weaker.

## Freshness Classes

| Freshness | Default Window | Typical Use |
| --- | --- | --- |
| `same_packet` | Must be collected inside this packet window. | Packet integrity, redaction status. |
| `recent_24h` | 24 hours. | URL reachability, RPC reachability, peer reachability. |
| `recent_7d` | 7 days. | Uptime, latency, missed votes, missed proposals. |
| `recent_30d` | 30 days. | Operator manifests, domain claims, topology labels. |
| `epoch_bound` | Valid for a registry or governance epoch. | Registry status, Cobalt amendment history. |
| `historical` | No freshness expiry, but must be bounded by height. | Prior registry lifecycle events, rollback records. |

The schema implementation may refine these windows, but generated rules must
declare which class they require.

## Missing And Conflict Semantics

| State | Default Behavior |
| --- | --- |
| Missing optional field | Neutral. Do not penalize or admit solely because it is missing. |
| Missing required field | Hold or no-op. Do not admit. |
| Stale field | No-op, hold, or reduced confidence according to the registered behavior. |
| Conflicting field | Hold or no-op. Do not admit. |
| Redacted field | Use only registered summary fields and raw observation hashes. |
| Forbidden/private field | Not visible to Qwen or DGA policy. |

Identity fields are especially sensitive. A missing URL or domain proof is
neutral unless a governed ruleset explicitly makes that proof required for the
specific action being evaluated.

## Action Bounds

| Bound | Meaning |
| --- | --- |
| `informational_only` | May appear in reports, but cannot drive registry action. |
| `score_adjustment` | May contribute to a bounded score or priority. |
| `admission_gate` | May gate an admit candidate if all required evidence is fresh and non-conflicting. |
| `hold_only` | May open a hold, but cannot remove or suspend. |
| `suspend_candidate` | May support a bounded suspension candidate with rollback evidence. |
| `remove_candidate` | May support removal only if a later governed ruleset explicitly allows it. |
| `no_action` | Must not affect action selection. |

The initial registry should prefer `informational_only`, `score_adjustment`,
`admission_gate`, and `hold_only`. Removal remains intentionally conservative.

## Packet Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `packet.schema` | enum | collector_observed | same_packet | reject packet | reject packet | no_action |
| `packet.packet_id` | string | collector_observed | same_packet | reject packet | reject packet | no_action |
| `packet.collection_window.from_height` | integer | chain_derived | same_packet | reject packet | reject packet | no_action |
| `packet.collection_window.to_height` | integer | chain_derived | same_packet | reject packet | reject packet | no_action |
| `packet.collection_window.from_time` | timestamp | collector_observed | same_packet | hold | hold | informational_only |
| `packet.collection_window.to_time` | timestamp | collector_observed | same_packet | hold | hold | informational_only |
| `packet.collector.name` | string | collector_observed | same_packet | reject packet | hold | informational_only |
| `packet.collector.version` | semver string | collector_observed | same_packet | reject packet | hold | informational_only |
| `packet.collector.config_hash` | hash | collector_observed | same_packet | reject packet | hold | informational_only |
| `packet.raw_observation_root` | hash | collector_observed | same_packet | hold | hold | informational_only |
| `packet.redaction.private_fields_removed` | boolean | collector_observed | same_packet | reject packet | reject packet | no_action |
| `packet.redaction.secret_scan_passed` | boolean | collector_observed | same_packet | reject packet | reject packet | no_action |

Packet fields mostly validate the evidence object itself. They should not
directly score validators.

## Registry Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `validator.registry.validator_id` | string | registry_derived | epoch_bound | reject validator entry | reject validator entry | no_action |
| `validator.registry.public_key_hash` | hash | registry_derived | epoch_bound | reject validator entry | reject validator entry | no_action |
| `validator.registry.active_status` | enum | registry_derived | epoch_bound | hold | hold | admission_gate |
| `validator.registry.registry_root` | hash | registry_derived | epoch_bound | reject packet | reject packet | no_action |
| `validator.registry.governance_epoch` | integer | registry_derived | epoch_bound | hold | hold | informational_only |
| `validator.registry.activation_height` | integer or null | registry_derived | historical | neutral | hold | informational_only |
| `validator.registry.suspension_height` | integer or null | registry_derived | historical | neutral | hold | hold_only |
| `validator.registry.removal_height` | integer or null | registry_derived | historical | neutral | hold | hold_only |
| `validator.registry.key_rotation_count` | integer | registry_derived | historical | neutral | hold | score_adjustment |

Registry-derived fields are strong evidence for current status. They do not
prove identity or operator quality by themselves.

## Operator Manifest Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `validator.operator_manifest.manifest_hash` | hash | operator_signed | recent_30d | neutral | hold | score_adjustment |
| `validator.operator_manifest.signing_key_hash` | hash | operator_signed | recent_30d | neutral | hold | informational_only |
| `validator.operator_manifest.signature_valid` | boolean | operator_signed | recent_30d | neutral | hold | admission_gate |
| `validator.operator_manifest.signed_at` | timestamp | operator_signed | recent_30d | neutral | hold | informational_only |
| `validator.operator_manifest.expires_at` | timestamp | operator_signed | recent_30d | stale | hold | admission_gate |
| `validator.operator_manifest.claims_hash` | hash | operator_signed | recent_30d | neutral | hold | informational_only |

Missing operator manifest evidence is neutral for controlled-testnet work unless
a later governed ruleset makes it mandatory for public launch claims.

## Identity And URL Proof Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `validator.identity.declared_domain` | domain | operator_signed | recent_30d | neutral | hold | score_adjustment |
| `validator.identity.declared_url` | URL | operator_signed | recent_30d | neutral | hold | score_adjustment |
| `validator.identity.url_fetch.status` | enum | collector_observed | recent_24h | neutral | hold | score_adjustment |
| `validator.identity.url_fetch.http_status` | integer or null | collector_observed | recent_24h | neutral | hold | informational_only |
| `validator.identity.url_fetch.content_hash` | hash or null | collector_observed | recent_24h | neutral | hold | score_adjustment |
| `validator.identity.url_fetch.observed_at` | timestamp | collector_observed | recent_24h | neutral | hold | informational_only |
| `validator.identity.url_fetch.failure_reason` | enum or null | collector_observed | recent_24h | neutral | hold | informational_only |
| `validator.identity.key_domain_binding.status` | enum | operator_signed and collector_observed | recent_30d | neutral | hold | admission_gate |
| `validator.identity.key_domain_binding.method` | enum | operator_signed and collector_observed | recent_30d | neutral | hold | informational_only |
| `validator.identity.key_domain_binding.proof_hash` | hash or null | operator_signed and collector_observed | recent_30d | neutral | hold | score_adjustment |

URL proof is an accountability signal, not a universal validator-quality
signal. Rules may reward strong fresh proof or require it for a specific public
launch path, but missing URL proof defaults to neutral.

## Network Performance Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `validator.performance.uptime_window_bps` | integer 0..10000 | network_observed | recent_7d | neutral | hold | score_adjustment |
| `validator.performance.rpc_reachable_bps` | integer 0..10000 | network_observed | recent_24h | neutral | hold | score_adjustment |
| `validator.performance.peer_reachable_bps` | integer 0..10000 | network_observed | recent_24h | neutral | hold | score_adjustment |
| `validator.performance.missed_vote_count` | integer | chain_derived or network_observed | recent_7d | neutral | hold | score_adjustment |
| `validator.performance.missed_proposal_count` | integer | chain_derived or network_observed | recent_7d | neutral | hold | score_adjustment |
| `validator.performance.late_vote_count` | integer | chain_derived or network_observed | recent_7d | neutral | hold | score_adjustment |
| `validator.performance.malformed_message_count` | integer | network_observed | recent_7d | neutral | hold | hold_only |
| `validator.performance.equivocation_event_count` | integer | chain_derived | historical | neutral | hold | suspend_candidate |
| `validator.performance.restart_event_count` | integer | network_observed | recent_7d | neutral | hold | score_adjustment |
| `validator.performance.p95_latency_ms` | integer | network_observed | recent_7d | neutral | hold | score_adjustment |

Performance fields can contribute to score or hold decisions. Suspension based
on performance requires a separate governed threshold and rollback path.

## Cobalt Participation Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `validator.cobalt.amendment_vote_count` | integer | chain_derived | epoch_bound | neutral | hold | score_adjustment |
| `validator.cobalt.missed_amendment_vote_count` | integer | chain_derived | epoch_bound | neutral | hold | score_adjustment |
| `validator.cobalt.dabc_participation_count` | integer | chain_derived | epoch_bound | neutral | hold | score_adjustment |
| `validator.cobalt.stale_evidence_rejection_count` | integer | chain_derived | historical | neutral | hold | hold_only |
| `validator.cobalt.rollback_event_count` | integer | chain_derived | historical | neutral | hold | hold_only |
| `validator.cobalt.supersession_event_count` | integer | chain_derived | historical | neutral | hold | informational_only |
| `validator.cobalt.trust_view_mismatch_count` | integer | chain_derived | epoch_bound | neutral | hold | hold_only |
| `validator.cobalt.last_participation_height` | integer or null | chain_derived | epoch_bound | neutral | hold | score_adjustment |

Cobalt fields are strong when derived from ordered governance history. They are
appropriate for governance participation rules because they are replayable.

## Governance Lifecycle Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `validator.governance.admit_event_count` | integer | chain_derived | historical | neutral | hold | informational_only |
| `validator.governance.hold_event_count` | integer | chain_derived | historical | neutral | hold | score_adjustment |
| `validator.governance.suspend_event_count` | integer | chain_derived | historical | neutral | hold | hold_only |
| `validator.governance.reactivate_event_count` | integer | chain_derived | historical | neutral | hold | informational_only |
| `validator.governance.remove_event_count` | integer | chain_derived | historical | neutral | hold | hold_only |
| `validator.governance.key_rotation_event_count` | integer | chain_derived | historical | neutral | hold | score_adjustment |
| `validator.governance.last_lifecycle_event_height` | integer or null | chain_derived | historical | neutral | hold | informational_only |
| `validator.governance.open_remediation_hold` | boolean | chain_derived | epoch_bound | neutral | hold | hold_only |

Lifecycle fields describe history. They should not create permanent stigma
without an explicit freshness or decay rule.

## Admission Policy Fields

These fields are consumed by the deterministic validator-admission policy v1.
They are not free-form model output. Qwen may classify bounded qualitative
evidence, but the selector only consumes typed fields with source hashes.

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `validator.admission.accountability_score` | integer 0..100 | derived from registered identity, manifest, contact, and revocation fields | same_packet | hold | hold | admission_gate |
| `validator.admission.rho_score` | integer 0..100 | derived from cohort topology and control-group evidence | same_packet | hold | hold | admission_gate |
| `validator.cobalt.linkedness_safe` | boolean | registry_derived | epoch_bound | hold | hold | admission_gate |
| `validator.model.operator_independence_classification` | enum | model replay certificate over registered evidence | same_packet | hold | hold | hold_only |

The initial controlled-testnet policy uses
`validator.admission.accountability_score >= 70`,
`validator.performance.uptime_window_bps >= 9950`, and
`validator.admission.rho_score <= 0`. Missing or conflicting required fields
hold. Values below floors or above caps reject. Model output cannot admit a
validator by itself.

## Topology And Diversity Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `validator.topology.host_group` | string | operator_signed or self_asserted | recent_30d | neutral | hold | score_adjustment |
| `validator.topology.operator_group` | string | operator_signed or self_asserted | recent_30d | neutral | hold | score_adjustment |
| `validator.topology.operator_host_group` | string | operator_signed or self_asserted | recent_30d | neutral | hold | score_adjustment |
| `validator.topology.cloud_provider_group` | string | operator_signed or self_asserted | recent_30d | neutral | hold | score_adjustment |
| `validator.topology.region_group` | string | operator_signed or self_asserted | recent_30d | neutral | hold | score_adjustment |
| `validator.topology.jurisdiction_group` | string | operator_signed, self_asserted, or third_party_attested | recent_30d | neutral | hold | score_adjustment |
| `validator.topology.legal_domain_group` | string | operator_signed, self_asserted, or third_party_attested | recent_30d | neutral | hold | score_adjustment |
| `validator.topology.funding_source_group` | string | operator_signed, self_asserted, or third_party_attested | recent_30d | neutral | hold | score_adjustment |
| `validator.topology.release_manager_group` | string | operator_signed or collector_observed | recent_30d | hold | hold | admission_gate |
| `validator.topology.key_management_group` | string | operator_signed or third_party_attested | recent_30d | hold | hold | admission_gate |

Topology fields are diversity signals. Self-asserted labels are useful for
controlled testnet planning, but not definitive public decentralization proof.

## Cohort Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `cohort.topology.host_group_count` | integer | derived from validator topology | same_packet | neutral | hold | admission_gate |
| `cohort.topology.operator_group_count` | integer | derived from validator topology | same_packet | neutral | hold | admission_gate |
| `cohort.topology.operator_host_group_count` | integer | derived from validator topology | same_packet | neutral | hold | admission_gate |
| `cohort.topology.cloud_provider_group_count` | integer | derived from validator topology | same_packet | neutral | hold | score_adjustment |
| `cohort.topology.region_group_count` | integer | derived from validator topology | same_packet | neutral | hold | score_adjustment |
| `cohort.topology.jurisdiction_group_count` | integer | derived from validator topology | same_packet | neutral | hold | score_adjustment |
| `cohort.registry.active_validator_count` | integer | registry_derived | epoch_bound | reject packet | reject packet | no_action |
| `cohort.registry.pending_validator_count` | integer | registry_derived | epoch_bound | neutral | hold | informational_only |

Cohort fields must be computed deterministically from packet contents. Rules
must not compute diversity from private labels or unregistered fields.

## Evidence Quality Fields

| Field Path | Type | Provenance | Freshness | Missing | Conflict | Allowed DGA Usage |
| --- | --- | --- | --- | --- | --- | --- |
| `validator.evidence_quality.field_conflict_count` | integer | collector_observed | same_packet | neutral | hold | hold_only |
| `validator.evidence_quality.stale_field_count` | integer | collector_observed | same_packet | neutral | hold | score_adjustment |
| `validator.evidence_quality.missing_required_field_count` | integer | collector_observed | same_packet | neutral | hold | hold_only |
| `validator.evidence_quality.redacted_field_count` | integer | collector_observed | same_packet | neutral | hold | informational_only |
| `validator.evidence_quality.private_field_rejected_count` | integer | collector_observed | same_packet | neutral | hold | hold_only |
| `validator.evidence_quality.raw_observation_hash_count` | integer | collector_observed | same_packet | neutral | hold | informational_only |

Evidence-quality fields are meta-signals. They can open a hold, but they
should not independently trigger removal.

## Forbidden Rule Inputs

Rules must not reference:

- social-media reputation;
- private KYC status;
- private messages;
- uncollected web search results;
- unbounded internet browsing;
- unredacted operator secrets;
- raw IP geolocation as proof of legal jurisdiction;
- human labels not present in the packet;
- fields from another packet unless a packet-root lineage proves inclusion.

## First Schema Implications

The first schema slice now lives in
[Validator Evidence Packet Schema](validator-evidence-packet-schema.md), backed
by `docs/governance/agent/validator_evidence_packet_schema.json`.

It encodes:

- registered field paths as an enum or registry file;
- provenance as a closed enum;
- freshness class as a closed enum;
- missing behavior as a closed enum;
- conflict behavior as a closed enum;
- action bound as a closed enum;
- per-field source hashes or raw observation roots;
- deterministic ordering for validator entries and field summaries.

The rule-binding slice now makes `GovernanceRuleset` decisions cite these
closed evidence fields directly. See
Validator Evidence Ruleset Binding.

## Open Review Questions

1. Should `validator.identity.key_domain_binding.status=proved` require the
   validator consensus key, a governance key, or an operator accountability key?
2. Should `validator.performance.equivocation_event_count` ever support direct
   suspension, or should it only open a hold before a separate Cobalt action?
3. Which topology fields are acceptable as self-asserted for controlled testnet
   but insufficient for public decentralization claims?
4. Should URL proof be a public-launch requirement or remain optional evidence?
5. How should multiple collectors produce a single conflict summary without
   leaking private network topology?
