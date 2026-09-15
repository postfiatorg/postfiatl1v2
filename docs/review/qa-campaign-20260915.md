# QA campaign log — 2026-09-15

This is the progress record for the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). Work began on clean `main` at `d4037d14`. This campaign reviews source and makes no deployment or live activation.

**Status:** Closed. A1, A2, A3, A4, A5, and B are done with the review limits below; the first compliant full inventory gate passed at 88.00/100. No live activation or fleet action.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Block finality, consensus artifacts, and signing | done | 0 | 2 | 1 | [Review](finality-consensus-review-20260915.md); findings `abb42827`; repairs `6ec35092` (consensus-affecting; full Rust suite verdict pending) |
| A2 | Canonical types and state commitment | done | 0 | 2 | 1 | [Review](types-state-commitment-review-20260915.md); findings `f5c8e88a`; repairs `0a1216c3` (consensus-affecting; full Rust suite verdict pending) |
| A3 | Node startup, release verification, and RPC serving | done | 0 | 2 | 1 | [Review](node-serving-review-20260915.md); findings `910030ce`; repairs `bccd5b5f` (not consensus-affecting; full Rust suite verdict pending) |
| A4 | Live shadow and swap services | done | 0 | 2 | 1 | [Review](shadow-swap-services-review-20260915.md); findings `b2df0ae1`; repairs `33c8ce34` (consensus-affecting; full Rust suite verdict pending) |
| A5 | Node command tools and governance agent | done | 0 | 2 | 1 | [Review](node-cli-governance-review-20260915.md); findings `face08c2`; repairs `eb4c2afd` (not consensus-affecting; full Rust suite verdict pending) |
| B | Defect inventory and TIH gate | done | — | — | — | [Inventory](defect-inventory-20260910.md) extended to 104 rows in `ec9cc1a0`; first compliant full gate 88.00/100; run group `qa-defect-inventory-burn4-20260915`; scored SHA-256 `795542e3964aef09681bd69dc8f31317726a66997105f0156e5781f8496d9298` |

Current finding totals: **0 P1, 10 P2, 5 P3** (A1, A2, A3, A4, and A5).

## Block finality, consensus artifacts, and signing review result

The [A1 review](finality-consensus-review-20260915.md) records **0 P1, 2 P2, 1 P3**. The P2s are maximum-view timeout successor overflow and missing durable proposal-lock reservation before a proposer returns its signature. Repair `6ec35092` uses a checked successor and the existing durable lock; both focused regressions pass. Both changes tighten artifact or signing admissibility and are **consensus-affecting**, source-only, and not live or deployed; **full Rust suite verdict pending**. The P3 records independently selected duplicate receipts and block links in the unaudited finality query and remains unfixed.

The A1 review examined focused paths within exactly five source files: `crates/node/src/block_finality.rs` (finality query, block proposal signing, vote and certificate aggregation, timeout and equivocation handling); `crates/node/src/consensus_artifacts.rs` (block vote/certificate and timeout validation, signed governance authorization); `crates/ordering_fast/src/lib.rs` (shared quorum and legacy certificate verification); `crates/ordering_fast/src/consensus_v2.rs` (typed proposal/vote/QC/TC verification and safety authorization); and `crates/crypto_provider/src/lib.rs` (ML-DSA signing, context verification, and hashing). This was a focused review of A1 behavior, not a whole-file audit. Sections that were not read for the A1 focus are detailed in Skips. No A2–A5 file or B inventory was reviewed.

## Canonical types and state commitment review result

The [A2 review](types-state-commitment-review-20260915.md) records **0 P1, 2 P2, 1 P3**. The P2s are a FastPay recovery committee identity that omits its new-order admission window and a retained recovery certificate absent from the reveal state commitment; the same undercommitment in confirmed version fences was repaired with the shared canonical certificate encoder. Repair `0a1216c3` binds both admission heights in the committee root and length-prefixes retained canonical certificate bytes in recovery state commitments. The two type-module regressions and the adjacent version-fence regression pass. The P3 notes a long domain label length narrowed by the public genesis digest helper; constant-label genesis-registry callers are unaffected, and the P3 remains recorded without repair.

Both P2 repairs change hashed committee identities or state-root bytes and are **consensus-affecting**, source-only, and not live or deployed; **full Rust suite verdict pending**. Archived replay and activation qualification are still required before any release-lineage use.

The A2 review read focused serialization, hashing, arithmetic, ordering, and version paths in `crates/types/src/consensus_v2_types.rs`, `core_chain.rs`, `ledger_assets.rs`, `genesis_registry.rs`, `fastswap_types.rs`, `account_owned_asset_types.rs`, `market_nav_asset_types.rs`, `fx_fix_types.rs`, `nav_reserve_public_values.rs`, `shielded_bridge_governance.rs`, `fastpay_recovery_types.rs`, and `crates/node/src/state_commitment.rs`. Large modules were sampled around the A2 focus, not audited in full. The pfUSDC and Ethereum bridge type files provided read-only schema context. The node root-test fixture was adjusted only to exercise a canonical retained certificate; it was not reviewed as a separate surface. No A3–A5 file or B inventory was reviewed during A2. Those units remained pending at A2 closeout.

## Node startup, release verification, and RPC serving review result

The [A3 review](node-serving-review-20260915.md) records **0 P1, 2 P2, 1 P3**. P2 findings cover lost earlier request events on RPC keep-alive sockets and ready markers published before serving preflights finish. Repair `bccd5b5f` records each completed request while the socket is open, drains events during nonblocking accept polling, releases each connection slot on a separate closure marker, and limits retained requests to 64 per socket. It also writes RPC readiness only after health-cache stamps pass and transport readiness only after the bound listener has been set nonblocking. The P3 records that runtime status hashes the configured manifest without rechecking its publisher signature or time window after the systemd prestart verifier; it remains recorded without repair.

The A3 repair changes only RPC serving, telemetry, and startup readiness. **No repair is consensus-affecting**; this is source-only, with no live activation or deployment. **Full Rust suite verdict pending** CI. Eight focused or adjacent node-binary tests passed; the initial transport regression `--lib` filter selected zero tests, so that regression was run with `--bin postfiat-node` and passed.

Focused serving/startup paths in `crates/node/src/rpc_cli.rs`, `transport_runtime.rs`, `lifecycle_queries.rs`, and `node_types.rs` were read, along with the `deployment-manifest-verify` subcommand in `main_parts/cli_dispatch_parts/group_05.rs` and its signed-envelope and locally hashed service/runtime artifact checks in `batch_snapshot.rs`. The generated systemd prestart invocation was traced within that verifier call path. These were focused reads, not whole-file audits; `node_types_snapshot_deployment.rs` was schema context only. The `main_parts/tests/rpc_serve_request_tests.rs` and `transport_protocol.rs` changes provide minimal serving regressions and were not reviewed as separate surfaces. The unrelated sections, excluded/previously reviewed crates and files, A4/A5, and B remain unreviewed as stated in Skips.

## Live shadow and swap services review result

The [A4 review](shadow-swap-services-review-20260915.md) records **0 P1, 2 P2, 1 P3**. The P2 findings cover a shadow peer replay watermark lowered by draining different rounds out of peer-sequence order, and issuer asset-control prepare signing another round-zero vote after an operation advances to a later recovery round. Repair `33c8ce34` keeps the highest peer sequence during queue draining and applies the ordinary swap's terminal-status and later-round guard to issuer asset control. The two regressions pass, as do the adjacent focused shadow and issuer-control tests. The P3 records canonical FastSwap refresh writing local WAL/state before a changed-tip error; it remains unfixed.

The shadow fix changes the hashed advisory shadow state-transition result, and the issuer fix tightens validator FastSwap vote admissibility. Both are **consensus-affecting** under the brief, source-only, and not live or deployed; **full Rust suite verdict pending** CI. No local full Rust workspace or Orchard/Halo2 suite was run: these fixes do not cross an Orchard boundary.

Focused authority, peer transport, queued-message, history/restart, signature, and bound paths in `crates/node/src/cobalt_shadow.rs` were reviewed; focused canonical controls, vote/interlock, FastSwap asset-control, certificate, and restart paths in `crates/node/src/fastswap_service.rs` were reviewed. The service files' drill and extensive test/latency fixture bodies were not audited as shipped service paths. `crates/node/src/batch_snapshot.rs` was not reviewed for A4 because neither named service directly uses it. `cobalt_shadow_tests.rs` was changed only for a focused regression, not reviewed as a separate surface. No A1–A3 implementation, A5 command path, B inventory, excluded or already-reviewed crates, or frozen artifact was reviewed or edited.

## Node command tools and governance agent review result

The [A5 review](node-cli-governance-review-20260915.md) records **0 P1, 2 P2, 1 P3**. P2 findings cover duplicated shadow command flags choosing an earlier remote target and the FastSwap bootstrap helper silently replacing an existing governance payload. Repair `eb4c2afd` rejects repeated or unpaired shadow flags before dispatch and exclusively creates bootstrap output, leaving existing bytes unchanged. Both regressions pass (1 each). The P3 records that a governance implementation work item can allow every touched path with an unrestricted wildcard; it remains recorded without repair.

Both fixes change operator command input validation or file creation only. **No A5 repair is consensus-affecting**; they are source-only, with no live activation or deployment; **full Rust suite verdict pending** CI. The full workspace and long Orchard/Halo2 suites were not run locally: neither change crosses an Orchard boundary.

Focused command parsing, key-file and private-record handling, listener/dry-run status, output and error paths in the twelve binaries under `crates/node/src/bin/` were sampled, including two three-line wrappers whose called modules were not followed. Focused dry-run/no-live-mutation gates, report validation and work-item scope paths in four production files under `crates/node/src/governance_agent_parts/` were sampled; `tests.rs` was not reviewed as shipped behavior. The top-level router and failure exit in `crates/node/src/main_parts/cli_dispatch.rs` were read: the brief's `crates/node/src/cli_dispatch.rs` path does not exist. `crates/node/src/execution_actions.rs` was not reviewed because its state-transition bodies were burn 3 A2 and no distinct A5 command-dispatch path was found there. Included CLI dispatch groups, called service/protocol modules, pfUSDC/Arc-specific node code, excluded and previously reviewed crates and files, frozen artifacts, A1–A4 implementation and B inventory were not reviewed or edited for A5.

## Skips and boundary decisions

- No Task Node, fleet, host, chain, deployment, spend, or signup action occurred; pushes were only to this repository's `origin main`.
- `crates/node/src/block_finality.rs`: account-transaction indexing and presentation (lines 163–2216) and remaining batch simulation/archive helpers were not audited. `crates/node/src/consensus_artifacts.rs`: unrelated shielded, bridge, pfUSDC/Arc, owned-object, operator-manifest, snapshot, and batch-action helpers were not audited. Full artifact reload across non-A1 node store modules was not audited; the repair regression checked durable proposal-lock reload using a fresh store instance.
- `crates/ordering_fast/src/lib.rs`: previously reviewed simulation/legacy ordering model, admission receipts, and omission evidence were not re-reviewed. `crates/ordering_fast/src/consensus_v2.rs` and `crates/crypto_provider/src/lib.rs`: their A1 signing and verification paths were read; no A1 file was excluded in full. The ordering v2 test file and node consensus-history test file were used only as regression fixtures, not as new review surfaces; the latter was minimally changed to preserve an externally signed equivocation case.
- Excluded and previously reviewed crates and files, frozen artifacts, and A2–A5 and B were not reviewed or edited during A1. The A1 P3 hot-path observation was recorded without repair. The full workspace and long Orchard/Halo2 suites were not run for A1: its changes touch signing and artifact admission and do not cross an Orchard boundary; the full Rust suite remains CI's verdict.
- For A2, the large account-owned, market/NAV, FastSwap, governance/shielded, and state-commitment modules were reviewed at the named focus paths, not line by line. Bridge-specific, pfUSDC/Arc, excluded Orchard/proof, and historical-exception commitment bodies were not audited; the pfUSDC and Ethereum type files were schema context only. Already-reviewed storage, execution, governance/Cobalt, network, mempool, wallet, RPC SDK, and Python sources were not re-reviewed. A3–A5 and B were not started. The A2 P3 was recorded without repair. No Orchard boundary was changed, so neither the long Orchard/Halo2 suite nor the full workspace suite was run locally; the full Rust suite remains CI's verdict.
- For A3, unrelated command, wallet, bridge, pfUSDC/Arc, Orchard, governance, archive/replay, and transaction paths in its large node files and snapshot verifier's surrounding snapshot operations were not audited. Deployment-manifest dispatch and its called verifier, generated prestart invocation, and manifest option/schema context alone were traced outside the four named files. No A1/A2 or burn 3 surface was re-reviewed; excluded files, frozen artifacts, A4/A5, and B were untouched. The A3 P3 remains unfixed. An initial `cargo test -p postfiat-node transport_listener_mode_failure_prevents_ready_report --lib --locked` ran zero selected tests because the regression belongs to the node binary; its exact binary-filtered rerun passed. No full workspace or Orchard/Halo2 suite ran for this RPC/startup-only change; the full Rust suite is CI's verdict. No Task Node, fleet, live chain, spend, or signup action occurred.

- For A4, `batch_snapshot.rs` was skipped because these services do not directly call it. The extensive drill/fixture and performance-test bodies in A4's two large service files were not reviewed as shipped service paths; A5 and B were not started. The A4 P3 is recorded without repair under the P3 rule. No release-lineage replay or live confirmation was run: the repairs are source-only; no fleet action was authorized. No full workspace or Orchard/Halo2 suite was run locally because no Orchard boundary changed; the full Rust suite verdict remains CI's. No Task Node, fleet, host, chain, spend, signup, or frozen-artifact action occurred.

- For A5, the named `crates/node/src/cli_dispatch.rs` was absent; the actual `main_parts/cli_dispatch.rs` router was reviewed at the A5 focus. Its included dispatch groups and the two three-line binaries' called modules were skipped because they are outside the A5 file list. `execution_actions.rs` was skipped because no distinct command-dispatch path was found outside the previously reviewed state transitions. Governance-agent `tests.rs`, large bin service internals, governance fixture/performance bodies and unaffected modules were not audited as shipped command paths. The A5 P3 is recorded without repair. Excluded, already-reviewed and frozen sources, plus B, were untouched; no full workspace or Orchard/Halo2 suite ran locally because neither A5 repair crosses those boundaries. No Task Node, fleet, host, chain, spend or signup action occurred; the three A5 pushes were only to this repository's `origin main`.

- For B, an initial harness dispatch accidentally used the shorter `--simple-prompt` setting. It was terminated and excluded from the gate; its external log is retained separately. A fresh full run used the recorded exact prompt, three judges, five reviews per judge, temperature 0 and an 8,000-token response limit. The first compliant average passed, so no wording rewrite or rescore was performed.

## Verification

- `cargo check -p postfiat-node -p postfiat-ordering-fast --locked`: passed.
- `cargo test -p postfiat-ordering-fast consensus_v2_proposal_rejects_exhausted_timeout_view_without_overflow --locked`: 1 passed.
- `cargo test -p postfiat-ordering-fast consensus_v2::tests --locked`: 12 passed.
- `cargo test -p postfiat-node proposer_signature_reserves_durable_proposal_lock_before_returning --lib --locked`: 1 passed.
- `cargo test -p postfiat-node bridge_exit_root_activation_tests --lib --locked`: 3 passed.
- `cargo test -p postfiat-node block_vote --lib --locked`: 3 passed.
- `cargo test -p postfiat-node block_proposal --lib --locked`: initial run 2 passed, 1 failed because the evidence fixture used the newly locked signer twice; after changing that fixture to provide a test-key signed external artifact, the rerun passed 3 tests.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and `scripts/public-secret-scan`: passed before each A1 commit (findings, repairs, and closeout). No full Rust suite was run locally; **full Rust suite verdict pending** CI.

- `cargo check -p postfiat-types -p postfiat-node --locked`: passed (A2).
- `cargo test -p postfiat-types fastpay_recovery_type_tests --lib --locked`: 5 passed (A2, including the committee, reveal, and confirmed-fence regressions).
- `cargo test -p postfiat-node replicated_state_root_commits_every_fastlane_ledger_field --lib --locked`: initial run 0 passed, 1 failed because its old retained-certificate fixture had no votes; after adding one ordered vote, rerun 1 passed (A2).
- `cargo test -p postfiat-node fastpay_recovery_bootstrap_is_signed_future_activated_and_tamper_atomic --lib --locked`: 1 passed (A2).
- `cargo test -p postfiat-node ordered_fastpay_recovery_cancels_partial_and_withheld_certificates_and_replays --lib --locked`: 1 passed (A2).
- `cargo fmt --all -- --check` and `git diff --check`: passed (A2).
- `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and `scripts/public-secret-scan`: passed before the A2 findings and repairs commits; A2 closeout gates passed before its commit. **Full Rust suite verdict pending** CI.

- `cargo check -p postfiat-node --locked`: passed (A3).
- `cargo test -p postfiat-node rpc_serve_keep_alive_records_each_request_and_closes_one_connection --bin postfiat-node --locked`: 1 passed (A3).
- `cargo test -p postfiat-node rpc_serve_logs_completed_request_while_keep_alive_socket_is_open --bin postfiat-node --locked`: 1 passed (A3).
- `cargo test -p postfiat-node rpc_serve_keep_alive_closes_at_retained_request_limit --bin postfiat-node --locked`: 1 passed (A3).
- `cargo test -p postfiat-node rpc_serve_health_preflight_failure_keeps_ready_file_absent --bin postfiat-node --locked`: 1 passed (A3).
- `cargo test -p postfiat-node transport_listener_mode_failure_prevents_ready_report --lib --locked`: 0 selected, 0 passed (A3; incorrect filter). `cargo test -p postfiat-node transport_listener_mode_failure_prevents_ready_report --bin postfiat-node --locked`: 1 passed (A3).
- `cargo test -p postfiat-node rpc_serve_accept_budget_is_exact_at_every_small_boundary --bin postfiat-node --locked`: 1 passed (A3).
- `cargo test -p postfiat-node transport_startup_after_prewarm_blocks_bind_until_prewarm_ready --bin postfiat-node --locked`: 1 passed (A3).
- `cargo test -p postfiat-node rpc_serve_drops_stalled_client_reads_without_blocking_other_connections --bin postfiat-node --locked`: 1 passed (A3). Eight selected node-binary tests passed in total.
- `cargo fmt --all -- --check` and `git diff --check`: passed (A3).
- `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and `scripts/public-secret-scan`: passed before the A3 findings and repairs commits; A3 closeout gates passed before its commit. **Full Rust suite verdict pending** CI.

- `cargo check -p postfiat-node --locked`: passed (A4).
- `cargo test -p postfiat-node queued_shadow_round_order_cannot_lower_peer_sequence_after_restart --lib --locked`: 1 passed (A4).
- `cargo test -p postfiat-node cobalt_shadow::tests --lib --locked`: 13 passed (A4, including the separately run shadow regression).
- `cargo test -p postfiat-node issuer_asset_control_round_zero_rejects_later_recovery_vote --bin postfiat-node --locked`: 1 passed (A4).
- `cargo test -p postfiat-node issuer_freeze_unfreeze_and_clawback_use_the_swap_lock_domain --bin postfiat-node --locked`: 1 passed (A4). Fifteen distinct focused node tests passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed (A4).
- `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and `scripts/public-secret-scan`: passed before A4 findings and repairs commits; A4 closeout gates passed before its commit. **Full Rust suite verdict pending** CI.

- `cargo check -p postfiat-node --locked`: passed (A5).
- `cargo test -p postfiat-node repeated_remote_target_is_rejected_before_shadow_request --bin postfiat-cobalt-shadow --locked`: 1 passed (A5).
- `cargo test -p postfiat-node bootstrap_output_refuses_existing_governance_payload_without_changing_it --bin fastswap_bootstrap_payload --locked`: 1 passed (A5). Two distinct focused node-binary tests passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed (A5).
- `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and `scripts/public-secret-scan`: passed before A5 findings and repairs commits; A5 closeout gates passed before its commit. **Full Rust suite verdict pending** CI.

- B changed only documentation. All 89 earlier inventory rows were verified unchanged; 15 new IDs and 104 unique rows were counted. Severity counts (25 P1, 54 P2, 25 P3), classification counts (77 reproduced defects, 22 evidence gaps, one economic assumption, four proposed capabilities), and disposition totals were reconciled. The inventory file SHA-256 and exact prompt were checked against all 15 fresh SQLite score records in the compliant run group.
- `.venv-docs/bin/mkdocs build --strict`, `scripts/public-doc-links`, and `scripts/public-secret-scan`: passed before the B inventory commit and closeout commit. `git diff --check` passed. No Rust, Python, workspace, or Orchard suite was rerun for B: the B changes are documentation only. The full Rust CI verdict for A1–A5 repairs remains pending.

## Scores

The exact 104-row inventory bytes, SHA-256
`795542e3964aef09681bd69dc8f31317726a66997105f0156e5781f8496d9298`,
received fifteen fresh OpenRouter reviews at temperature 0 with an 8,000-token
response limit. The prompt was
`Rate this document on a scale of 1-100. Output the score and your reasoning.`
The credential came from vault label `openroutertih` and was passed to the
harness in memory.

| Judge | Scores | Average |
| --- | --- | ---: |
| `openai/gpt-6-astra-pro` | 89, 88, 88, 89, 88 | 88.40 |
| `anthropic/claude-fable-5.1` | 87, 86, 82, 89, 89 | 86.60 |
| `z-ai/glm-5.3` | 90, 88, 88, 91, 88 | 89.00 |
| **All fifteen** | — | **88.00** |

Run group: `qa-defect-inventory-burn4-20260915`. The first compliant full
score exceeded the 86/100 stop condition; no inventory rewrite or rescore was
performed. All fifteen SQLite records match the prompt, group and scored file
SHA-256. The external score log and SQLite record are under
`/home/postfiatchad/pastedocs/.qa-campaign-defect-inventory-burn4-20260915/`.
An interrupted wrong-prompt attempt is isolated there and was not counted.

## Final summary

| Surface | P1 | P2 | P3 | Findings commit | Repair commit |
| --- | ---: | ---: | ---: | --- | --- |
| A1 — Finality, consensus artifacts, signing | 0 | 2 | 1 | `abb42827` | `6ec35092` |
| A2 — Canonical types, state commitment | 0 | 2 | 1 | `f5c8e88a` | `0a1216c3` |
| A3 — Startup, release verification, RPC | 0 | 2 | 1 | `910030ce` | `bccd5b5f` |
| A4 — Shadow and swap services | 0 | 2 | 1 | `b2df0ae1` | `33c8ce34` |
| A5 — Node tools, governance agent | 0 | 2 | 1 | `face08c2` | `eb4c2afd` |
| **A1–A5 total** | **0** | **10** | **5** | — | — |

B added three rows per surface: FIN-, TYP-, SRV-, SHD-, and CLI-. The consolidated
inventory has **104 rows**: 25 P1, 54 P2, and 25 P3; 58 fixed, 16
dispositioned, five reproduced and retained from prior campaigns, thirteen
burn 3/4 P3 recorded without repair, six needing a live environment, six
needing an operator decision, and zero bare open. The first compliant full
Text Improvement Harness gate scored **88.00/100**, run group
`qa-defect-inventory-burn4-20260915`, scored file SHA-256
`795542e3964aef09681bd69dc8f31317726a66997105f0156e5781f8496d9298`.
No rewrite or rescore was needed; the inventory extension is `ec9cc1a0`.

Consensus-affecting repairs, all source-only and not activated or deployed:

- Checked maximum-view timeout successor in proposal verification — `6ec35092`.
- Durable proposal-hash lock before the proposer returns a block signature — `6ec35092`.
- FastPay recovery committee root binding both admission heights — `0a1216c3`.
- Canonical retained-certificate bytes bound to recovery reveal and confirmed
  version-fence state commitments — `0a1216c3`.
- Highest sender sequence retained across round-ordered shadow queue draining,
  changing hashed advisory shadow state — `33c8ce34`.
- Issuer asset-control prepare refusing a stale round-zero FastSwap vote after
  later recovery progress or terminal status — `33c8ce34`.

The **full Rust suite verdict is pending** CI for every repair commit:
`6ec35092`, `0a1216c3`, `bccd5b5f`, `33c8ce34`, and `eb4c2afd`.
Archived-chain replay and activation qualification remain necessary before
release-lineage use of the FastPay committee and state-root changes.

The five burn 4 P3 risks remain recorded without repair: unaudited finality
queries can pair duplicate receipt and block records; an external long-label
caller can collide genesis-digest framing; runtime status does not reverify a
replaced deployment manifest; a changed FastSwap canonical tip can leave local
WAL/state writes before refresh fails; and an unrestricted governance
work-item wildcard can overstate report scope. Review limits and other
campaign skips are recorded above. No fleet, live-chain, deployment,
activation, Task Node, or frozen-artifact action occurred.

A1 through A5 and B are closed. The inventory and closeout commits are pushed
to origin `main`.

## Burn 3 and 4 fuzz and property coverage

Bounded deterministic Rust cases added over the repaired source; no protocol
behavior changed. Each named test also runs with its owning module regressions.

| Repair | Property test (generated cases) |
| --- | --- |
| Burn 3 `69e1f1ce` | `torn_wal_suffix_property_truncates_before_the_next_durable_append` (3 torn lengths); `snapshot_import_mutated_manifest_property_does_not_publish_destination` (8 invalid genesis values, 1 truncated manifest) |
| Burn 3 `e95efbdf` | `owned_object_capacity_property_rejects_growth_across_the_limit` (5,265 cap/consume/create combinations, 1 arithmetic overflow); `supplied_proposal_receipt_id_property_rejects_every_duplicate_position` (17 lengths, 120 duplicate positions, 17 reorderings); `state_file_size_property_rejects_overflow_and_counts_the_trailer` (387 limit edges, 1 overflow, 4 small values) |
| Burn 3 `c9a61fcd` | `cobalt_quorum_overlap_property_matches_minimum_possible_quorum_intersection` (128 combinations); `full_knowledge_checkpoint_candidate_binding_property_gates_dabc_activation` (4 signed wrong-candidate checkpoints) |
| Burn 3 `f2dea308` | `validator_serving_summary_property_caps_retention_on_replayed_rejections` (7 rejection-series sizes, 5,137 attempted summaries); `validator_worker_permit_property_limits_in_flight_connections` (5 slot capacities, 31 permits); `batch_serve_rejection_budget_property_stays_bounded_at_zero_and_overflow` (1,028 batch sizes) |
| Burn 4 `6ec35092` | `consensus_v2_max_view_timeout_property_rejects_all_successor_proposals` (4 candidate views); `proposer_durable_lock_property_rejects_conflicting_payloads_after_signing` (8 conflicting hashes) |
| Burn 4 `0a1216c3` | `recovery_committee_admission_window_property_changes_root_and_rejects_stale_root` (64 height mutations, 2 invalid windows); `recovery_reveal_certificate_mutation_property_changes_committed_bytes` (128 owner/vote signature mutations); `fastpay-recovery-committee-window` harness target (256 JSON mutations, 1 valid seed, 3 invalid/max windows); adjacent confirmed-version-fence regression remains in the same module |
| Burn 4 `bccd5b5f` | `rpc_serve_health_stamp_property_requires_each_preflight_file` (4 missing files, 4 restores); existing keep-alive regression expanded to 8 ordered requests and 8 event-log records |
| Burn 4 `33c8ce34` | `queued_shadow_reordered_sequences_property_preserves_maximum_after_restart` (4 out-of-order queued rounds, 3 stale signed replays); issuer round-zero regression expanded to 8 later recovery rounds and terminal cancellation |
| Burn 4 `eb4c2afd` | `shadow_flag_property_rejects_repetition_and_truncated_values` (12 flag pairs with 4 truncation points and invalid variants); `bootstrap_payload_existing_file_property_preserves_all_bytes` (6 existing sizes, 1 new-file retry) |

The `fastpay-recovery-committee-window` 260-case corpus parsed one case and
rejected 259 at parse; only the parsed case reached the invariant check, with
zero invariant failures. This is the limit of the fuzz-harness evidence.

Skip: burn 3 `c2724977` changes Python operational CLIs only; this unit is
Rust-only. No bridge, Orchard, proof, or program crate was edited or tested.
