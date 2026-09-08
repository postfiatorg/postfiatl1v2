# Task Node UNL amendment V2 implementation journal

**Status:** Completed 2026-09-08 — sections A–E and their focused gates pass; V2 remains `SHADOW_ONLY` and is not promoted.

**Completion note:** Section A landed in `a710f96c`, section B in `1306ac26`,
section C in `41f6ae1a`, and sections D and E in this completion commit (the
containing commit's exact hash is recorded by Git). The paired gate verdict is
`PASS_SHADOW_ONLY` with 14/14 honest controls admissible. Task Node lifecycle
was skipped by operator decision 2026-09-08 after generation requests
`req_cf56f893…` and `req_6948d197…` failed with
`taskgen_provider_output_invalid`. Completion records the five implemented
sections, not live promotion.

**Documentation task:** `task_fbdc96a15de43ad360a81c25173d8d5b`.
**Locked source:** [UNL amendment V2](../../governance/tasknode-unl-amendment-v2-20260907.md), SHA-256 `a74bdf603c70ebc3432499b555b470d87da93e13c54424b3687f834bf02f892f`.
**Research evidence:** [first full gate: 90.53/100](../../review/tasknode-unl-amendment-v2-lock-20260907.md), research task `task_c26d27a9e112a7ad1c27682b95f0790e`.

This journal changes no admission rule. Preserve the locked amendment, original
blog, V1 MVP and its committed attack outputs. V2 is a separate, explicitly
selected, `SHADOW_ONLY` policy. No submit, ratify, deploy or fund operation
belongs to this milestone. Sections A–E's V2 evidence, policy, gate, CLI and
human-report files now exist. Existing V1 references identify boundaries, not
permission to rewrite V1.

## A. Versioned evidence contract — amendment §§2–3, 6

**Implementation task:** `task_aece75f855b2633598b89dffd266a024` (accepted 2026-09-08).

Existing references: `python/postfiat_rpc/tasknode_unl_schema.py`,
`tasknode_unl_binding.py`, `tasknode_unl_edges.py`, `tasknode_unl_work_digest.py`.
Implementation surface: separate `tasknode_unl_v2_schema.py` and
`tasknode_unl_v2_evidence.py`, with tests under `python/tests/`.

- [x] Freeze closed, bounded V2 schemas, domain-separated canonical statements, signature/custody contract, policy/input/registry roots and deterministic golden vectors. Reject unknown versions and mismatched commitments. (`python/postfiat_rpc/tasknode_unl_v2_schema.py`; `tasknode_unl_v2_evidence.py`; `python/tests/fixtures/tasknode_unl_v2/evidence-golden.json`)
- [x] Implement public control epochs for authorized transfer, binding/key replacement and recovery; untrusted accusations or transfers cannot reset them. (`ControlEpoch`, `control_epoch_from_event` in `tasknode_unl_v2_evidence.py`)
- [x] Require a full fresh 180-day window for additions, including post-event score evidence, renewed vouches and post-event co-work. Incumbent rotations report continuity holds, not automatic eviction. (`assess_fresh_window`, `ContinuityAssessment` in `tasknode_unl_v2_evidence.py`)
- [x] Keep unchanged-key account sales explicitly undetectable; never synthesize personhood or “sale detected” evidence. (`EVIDENCE_LIMITATIONS` in `tasknode_unl_v2_schema.py`; `EvidenceSnapshotResult` in `tasknode_unl_v2_evidence.py`)
- [x] Separate audit-only funding observations from bilateral accepted vouches/co-work and all-member-signed control declarations. Bind each acknowledgement to exact accounts, control epochs, statement and effective/expiry windows. (`V2_FUNDING_OBSERVATION_SCHEMA`, `V2_BILATERAL_RECORD_SCHEMA`, `V2_CONTROL_DECLARATION_SCHEMA`; `verify_evidence_snapshot`)
- [x] Implement boundary-only updates, duplicate credit suppression, next-window trust revocation and a full-window delay after jointly signed control-group dissolution. Malformed individual declarations cannot poison a valid whole snapshot. (`_active_relations`, `_active_declarations`, `RecordRejection`; `python/tests/test_tasknode_unl_v2_evidence.py`)

## B. Graph and admission state — amendment §§3–4, 6

**Implementation task:** `task_91a089bd14dedf6b6fb681fae7e49561` (accepted 2026-09-08).

Existing references: `tasknode_unl_trust_graph.py`, `tasknode_unl_policy.py`,
`tasknode_unl_accountability.py`, `tasknode_unl_churn.py` under the same package;
`crates/consensus_cobalt/src/validator_admission_policy.rs` is a **read-only live-authority reference**.
Implementation surface: separate `tasknode_unl_v2_policy.py` and tests.

- [x] Preserve exact rational arithmetic, canonical ordering and the locked numerical constants; use no funding-derived positive mass or unilateral funding veto in V2. (`admission_policy_document`, `_credits`, `_walk` in `python/postfiat_rpc/tasknode_unl_v2_policy.py`)
- [x] Freeze graph, partition, N and non-Foundation seeds per window; recount current seats after every conceptual round. New validators become seeds only at the next boundary. (`freeze_admission_window`, `recount_limit_states`, `advance_shadow_round`)
- [x] Enforce both the exact social cap `max(2,N/10)` and declared-control-group one-seat restriction for additions; do not round fractional limits upward. (`social_seat_limit`, `permitted_integer_seats`, `_control_groups`)
- [x] Distinguish CLEAR, SATURATED and EXISTING_BREACH, including excess seats and causative evidence. Block affected additions, not every unrelated cluster. (`LimitState`, `_limit_state`, `evaluate_admission_round`)
- [x] Preserve and report incumbent breaches instead of silently deleting seats. Model full-window review, persistent unresolved state, existing authorized churn and one-round old-root overlap; do not invent an eviction selector or correction override. (`prior_breaches_from_report`, `RegistryRoundState`, `ShadowAdmissionReport`)
- [x] Bind every candidate reason and unresolved limit into canonical shadow reports. Empty eligible seeds or invalid snapshot commitments produce no proposal. (`CandidateDecision`, `ShadowAdmissionReport.canonical_bytes`, `freeze_admission_window`; `python/tests/fixtures/tasknode_unl_v2/policy-golden.json`)

## C. Paired adversarial and liveness gate — amendment §§1, 5

**Implementation task:** Task Node lifecycle skipped by operator decision 2026-09-08; generation requests req_cf56f893… and req_6948d197… failed with taskgen_provider_output_invalid.

Existing baseline: `benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/run_simulation.py`,
its manifest/results and `python/tests/test_tasknode_unl_*.py`.
Implementation surface: `benchmarks/ai-governance/tasknode-unl-v2-gate-20260908/`
and `python/tests/fixtures/tasknode_unl_v2/gate-golden.json`.

- [x] Preregister and hash generators, budgets, labels, parameter grid and metrics before V2 trials. Reproduce V1 bytes unchanged; report V2 separately. (`preregistration.json`, `preregistration-extension.json`, `PREREGISTRATION*.sha256`; `load_and_verify_preregistration`, `verify_frozen_v1` in `run_gate.py`)
- [x] Test quiet-window dust and larger unsolicited inflows for identical target mass, cluster, group and result; retain the observations for audit. (`run_unsolicited_funding`; `outputs/attack-audit.json`)
- [x] Test declared versus hidden control changes, renewal/recovery, accepted-bridge attacks, undeclared common funding, revocation and cap-merging grief. (`run_control_change`, `run_renewal_recovery`, `run_accepted_bridge`, `run_undeclared_common_funding`, `run_revocation`, `run_cap_merging_grief`)
- [x] Require all fourteen original honest controls to remain admissible in the paired consent-complete fixture; report failures rather than adjusting the gate. (`run_honest_gate`; `outputs/results.json`)
- [x] Sweep acknowledgement availability at 100/75/50/0%; repeat the original low/base/high grid and all 27 damping/steps/floor combinations, reporting honest and attacker outcomes together. (`run_acknowledgement_sweep`, `run_sensitivity`, `run_cartesian_grid`; `outputs/*.csv`)
- [x] Run at least thirty published seeded topology variants and at least three changing windows; cover exact floor boundaries, dangling nodes, no seeds, duplicates, rotations, removals and persistent breaches. (`build_variable_topology_scenario`, `run_topology_windows`; `outputs/topology-windows.csv`)
- [x] Report honest admission/waiting, zero-admission windows, best attacker seats within budget, unsolicited denials, concentration and breach duration. Require two-run and reordered-input byte determinism. (`build_results`, `run_experiment`; `outputs/determinism.json`, `outputs/output-manifest.json`)

## D. Human-operable Python CLI — amendment §6

Existing CLI/report references: `python/postfiat_rpc/tasknode_unl.py`
(`build_parser`, `main`, `_emit_shadow`, `_emit_shadow_diff`) and
`tasknode_unl_policy.py` (`render_shadow_markdown`).
Implemented surface: separate `python -m postfiat_rpc.tasknode_unl_v2` interface.

- [x] Request and accept the implementation/CLI Task Node work before construction. Task Node lifecycle skipped by operator decision 2026-09-08; explicit V2 selection, bounded validation and read-only side-by-side derivation are implemented in `build_parser`, `_parse_admission_input` and `derive_v2_cli_report` in `python/postfiat_rpc/tasknode_unl_v2.py`.
- [x] Show SHADOW_ONLY, HOLD_CONTINUITY, admission denials, EXISTING_BREACH and undetectable-sale/control limits separately; provide machine-readable output plus concise human explanations. (`derive_v2_cli_report`, `render_v2_markdown`; `python/tests/fixtures/tasknode_unl_v2/cli-report.md`)
- [x] Verify the actual CLI against the same fixtures, malformed input cases and deterministic outputs before proceeding to the user interface. (`python/tests/test_tasknode_unl_v2_cli.py`; `evidence-golden.json`; `cli-admission-input.json`)

## E. User-facing report and documentation — amendment §6

Existing UX baseline: CLI-generated human Markdown report above.
Implemented surface: a read-only Markdown report interface consuming and
root-verifying that CLI output.

- [x] Request and accept the interface task after the CLI works. Task Node lifecycle skipped by operator decision 2026-09-08; the root-verified report preserves reason/root/version visibility and uses `SHADOW_ONLY_WITH_UNRESOLVED_IDENTITY_LIMITS`, never an all-green badge. (`render_v2_markdown`; `python/tests/fixtures/tasknode_unl_v2/cli-report.md`)
- [x] Test the human workflow and report-to-CLI consistency. Document working functionality in the existing docs only after both CLI and interface work. (`TestV2ActualCli` in `python/tests/test_tasknode_unl_v2_cli.py`; `docs/governance/tasknode-unl-v2-shadow-report.md`)
- [x] Retire this journal into completed plans only after implementation, CLI, user surface and evidence gates genuinely pass; research lock alone completes none of them. (`docs/plans/completed/tasknode-unl-amendment-v2-milestone.md`)

## Explicit activation boundary — amendment §§4–6

Representative live liveness targets, acceptable hidden-control risk, reviewed
acknowledgement UX/custody, final schemas and a safe cap-repair/removal policy
remain separate approvals. Synthetic passes and model scores authorize none of
these. Do not alter registry signatures, Cobalt ratification, consensus,
Foundation score provenance or token economics as an incidental V2 change.

## Journal

- [x] 2026-09-07: verified the locked research hash and drafted this documentation-only journal. V1 source and original blog are unchanged. All implementation/CLI/UI boxes remain unchecked.
- [x] 2026-09-08: accepted Task Node task `task_aece75f855b2633598b89dffd266a024` and completed section A in `tasknode_unl_v2_schema.py`, `tasknode_unl_v2_evidence.py` and the V2 fixture/tests. The combined V1/V2 selection passed 123 tests and strict MkDocs passed.
- [x] 2026-09-08: accepted Task Node task `task_91a089bd14dedf6b6fb681fae7e49561` and completed section B in `tasknode_unl_v2_policy.py` and its golden fixture/tests. V1 passed 107 tests/36 subtests, V2 passed 32 tests/7 subtests and the combined selection passed 139 tests/43 subtests; strict MkDocs passed. Sections C–E remain unchecked.
- [x] 2026-09-08: completed the separately preregistered Section C offline gate after the operator skipped Task Node because both generation requests failed. The frozen V1 artifacts remained byte-identical, the paired honest fixture admitted 14/14 controls, all 30 seeds ran over three windows, and normal/reordered V2 result bytes matched. Sections D–E remain unchecked.
- [x] 2026-09-08: completed sections D and E with the explicit read-only `tasknode_unl_v2` derive/render CLI, a root-verified human Markdown report, fixture/malformed/determinism tests and user documentation. All A–E boxes are complete; the milestone is retired without promoting V2.
