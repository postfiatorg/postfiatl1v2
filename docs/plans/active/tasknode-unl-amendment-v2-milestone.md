# Task Node UNL amendment V2 implementation journal

**Status:** Section A implemented under the accepted Task Node item below. Sections B–E remain **future Task Node-governed work, not completed functionality**; V2 remains `SHADOW_ONLY`.

**Documentation task:** `task_fbdc96a15de43ad360a81c25173d8d5b`.
**Locked source:** [UNL amendment V2](../../governance/tasknode-unl-amendment-v2-20260907.md), SHA-256 `a74bdf603c70ebc3432499b555b470d87da93e13c54424b3687f834bf02f892f`.
**Research evidence:** [first full gate: 90.53/100](../../review/tasknode-unl-amendment-v2-lock-20260907.md), research task `task_c26d27a9e112a7ad1c27682b95f0790e`.

This journal changes no admission rule. Preserve the locked amendment, original
blog, V1 MVP and its committed attack outputs. V2 is a separate, explicitly
selected, `SHADOW_ONLY` policy. No submit, ratify, deploy or fund operation
belongs to this milestone. Section A's V2 evidence files now exist; later
proposed filenames do not. Existing V1 references identify boundaries, not
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

Existing references: `tasknode_unl_trust_graph.py`, `tasknode_unl_policy.py`,
`tasknode_unl_accountability.py`, `tasknode_unl_churn.py` under the same package;
`crates/consensus_cobalt/src/validator_admission_policy.rs` is a **read-only live-authority reference**.
Future implementation surface: separate `tasknode_unl_v2_policy.py` and tests.

- [ ] Preserve exact rational arithmetic, canonical ordering and the locked numerical constants; use no funding-derived positive mass or unilateral funding veto in V2.
- [ ] Freeze graph, partition, N and non-Foundation seeds per window; recount current seats after every conceptual round. New validators become seeds only at the next boundary.
- [ ] Enforce both the exact social cap `max(2,N/10)` and declared-control-group one-seat restriction for additions; do not round fractional limits upward.
- [ ] Distinguish CLEAR, SATURATED and EXISTING_BREACH, including excess seats and causative evidence. Block affected additions, not every unrelated cluster.
- [ ] Preserve and report incumbent breaches instead of silently deleting seats. Model full-window review, persistent unresolved state, existing authorized churn and one-round old-root overlap; do not invent an eviction selector or correction override.
- [ ] Bind every candidate reason and unresolved limit into canonical shadow reports. Empty eligible seeds or invalid snapshot commitments produce no proposal.

## C. Paired adversarial and liveness gate — amendment §§1, 5

Existing baseline: `benchmarks/ai-governance/tasknode-unl-attack-simulation-20260907/run_simulation.py`,
its manifest/results and `python/tests/test_tasknode_unl_*.py`.
Future implementation surface: a separate V2 experiment directory and fixtures.

- [ ] Preregister and hash generators, budgets, labels, parameter grid and metrics before V2 trials. Reproduce V1 bytes unchanged; report V2 separately.
- [ ] Test quiet-window dust and larger unsolicited inflows for identical target mass, cluster, group and result; retain the observations for audit.
- [ ] Test declared versus hidden control changes, renewal/recovery, accepted-bridge attacks, undeclared common funding, revocation and cap-merging grief.
- [ ] Require all fourteen original honest controls to remain admissible in the paired consent-complete fixture; report failures rather than adjusting the gate.
- [ ] Sweep acknowledgement availability at 100/75/50/0%; repeat the original low/base/high grid and all 27 damping/steps/floor combinations, reporting honest and attacker outcomes together.
- [ ] Run at least thirty published seeded topology variants and at least three changing windows; cover exact floor boundaries, dangling nodes, no seeds, duplicates, rotations, removals and persistent breaches.
- [ ] Report honest admission/waiting, zero-admission windows, best attacker seats within budget, unsolicited denials, concentration and breach duration. Require two-run and reordered-input byte determinism.

## D. Human-operable Python CLI — amendment §6

Existing CLI/report references: `python/postfiat_rpc/tasknode_unl.py`
(`build_parser`, `main`, `_emit_shadow`, `_emit_shadow_diff`) and
`tasknode_unl_policy.py` (`render_shadow_markdown`).
Future surface: separate `python -m postfiat_rpc.tasknode_unl_v2` interface.

- [ ] Request and accept the implementation/CLI Task Node work before construction. Expose explicit V2 selection, bounded input validation, derivation and side-by-side V1/V2 explanation without submission or live mutation.
- [ ] Show SHADOW_ONLY, HOLD_CONTINUITY, admission denials, EXISTING_BREACH and undetectable-sale/control limits separately; provide machine-readable output plus concise human explanations.
- [ ] Verify the actual CLI against the same fixtures, malformed input cases and deterministic outputs before proceeding to the user interface.

## E. User-facing report and documentation — amendment §6

Existing UX baseline: CLI-generated human Markdown report above.
Future surface: a separately Task Node-scoped read-only report interface consuming
that CLI output; concrete UI ownership is assigned in that generated task, not assumed here.

- [ ] Request and accept the interface task after the CLI works. Preserve reason/root/version visibility and avoid an all-green badge for unresolved identity or cap risk.
- [ ] Test the human workflow and report-to-CLI consistency. Document working functionality in the existing docs only after both CLI and interface work.
- [ ] Retire this journal into completed plans only after implementation, CLI, user surface and evidence gates genuinely pass; research lock alone completes none of them.

## Explicit activation boundary — amendment §§4–6

Representative live liveness targets, acceptable hidden-control risk, reviewed
acknowledgement UX/custody, final schemas and a safe cap-repair/removal policy
remain separate approvals. Synthetic passes and model scores authorize none of
these. Do not alter registry signatures, Cobalt ratification, consensus,
Foundation score provenance or token economics as an incidental V2 change.

## Journal

- [x] 2026-09-07: verified the locked research hash and drafted this documentation-only journal. V1 source and original blog are unchanged. All implementation/CLI/UI boxes remain unchecked.
- [x] 2026-09-08: accepted Task Node task `task_aece75f855b2633598b89dffd266a024` and completed section A in `tasknode_unl_v2_schema.py`, `tasknode_unl_v2_evidence.py` and the V2 fixture/tests. The combined V1/V2 selection passed 123 tests and strict MkDocs passed; sections B–E remain unchecked.
