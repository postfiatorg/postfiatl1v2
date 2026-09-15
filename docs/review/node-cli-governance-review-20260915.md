# Node command tools and governance agent review — 2026-09-15

This is A5 of the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). Focused command parsing, output interlocks, private-file handling, and dry-run versus live-admission paths were read cold in the named node binaries, governance-agent parts, and the node CLI dispatch module. The brief names `crates/node/src/cli_dispatch.rs`, which does not exist; the actual dispatch module is `crates/node/src/main_parts/cli_dispatch.rs`. This is a focused review, not a whole-file audit.

## Findings

### 1. P2 — repeated shadow command flags can silently select an earlier live-target value

**Source:** `crates/node/src/bin/postfiat_cobalt_shadow.rs:462-477` (command selection at `:30-36`, remote requests at `:240-254,284-303,430-435`).

An operator invokes `propose-rpc` or `commit` with a duplicated `--endpoint` while correcting the target on the same command line. `optional_flag` chooses the first pair and silently ignores the second, so a signed proposal or transcript request is sent to the earlier endpoint while the command can still print a successful response. The same parser accepts trailing positionals and duplicate binding or archive-path flags on local reset and signing commands. This is a command-target ambiguity; it is not evidence of a live fleet action in this review.

The minimal repair is to reject duplicate flags and unpaired positional arguments before dispatch, retain the existing required/optional flag accessors, and add a regression that rejects a repeated endpoint before any remote request. This changes command input validation only and is not consensus-affecting.

### 2. P2 — the FastSwap bootstrap helper overwrites an existing governance payload without an interlock

**Source:** `crates/node/src/bin/fastswap_bootstrap_payload.rs:265-271` (output selection at `:60-85`).

Rerunning the helper against an existing `--output` path truncates and replaces the prior payload, including its committee, activation window and policy, then prints `wrote` as though it created a new artifact. A signing or review workflow that keeps the path stable can unknowingly consume the second payload after the first was inspected. No live signing or activation is implied by the helper itself.

The minimal repair is to write the new payload with exclusive creation, refuse an existing output path without altering its bytes, and add a regression with a pre-existing payload file. This is an operator-file interlock only and is not consensus-affecting.

### 3. P3 — governance implementation reports can treat an unrestricted wildcard as an allowed surface

**Source:** `crates/node/src/governance_agent_parts/implementation_guarded_apply.rs:182-197` (work-item checks at `:124-191`).

If a work item declares `"*"` as an allowed surface, `governance_agent_surface_matches` accepts every touched path. A report can therefore mark `touched_surfaces_authorized` and `verified` true even when an unexpected path was touched; the report checks the submitted work-item fields and does not execute or authorize a live mutation. An operator using the report as a precise scope receipt can miss the breadth of that wildcard.

A future minimal repair would reject unrestricted or ambiguous wildcard patterns during work-item validation and test a work item containing an unexpected touched path. This P3 is recorded without repair.

## Areas with no findings

- `crates/node/src/main_parts/cli_dispatch.rs:350-755`: read top-level command routing, result/error exit behavior and the included dispatch-group boundary; non-RPC failures return exit status 1 and RPC failures print structured errors with exit status 1. The included group files are outside the A5 file list and were not audited.
- `crates/node/src/bin/pftl_swapctl.rs:79-154,245-290,361-571`: read loopback request interlocks, fail-closed HTTP success checking, private-record exclusive write and file permissions, and flag parsing; no additional finding in these paths. `crates/node/src/bin/pftl_swapd.rs:206-264,2240-2334,2527-2714`: read startup key-file and loopback checks, private journal/record permissions and service option parsing; no additional finding in these paths.
- `crates/node/src/bin/postfiat_cobalt_shadow.rs:30-101,111-350,430-524`: read command-specific binding and lineage-reset preconditions, signed remote request assembly, shadow-only listener status and error exits; no additional finding beyond finding 1 in the sampled paths.
- `crates/node/src/bin/fastswap_bootstrap_payload.rs:37-165,206-286` and `fastswap_wallet_payload.rs:14-120,143-255`: read bootstrap committee/policy validation and output handling, wallet-helper quote/window validation and output handling; no additional finding beyond finding 2.
- `crates/node/src/governance_agent_parts/gate_entrypoints.rs:1-317,317-490,764-855`, `implementation_guarded_apply.rs:1-275`, `ruleset_hashing_io.rs:1-190,391-480,605-673`, and `verifier_receipts_adversarial.rs:120-302`: read report failure signaling, no-live-mutation gates, policy candidate caps, bounded input reads and private-marker checks; no additional finding beyond finding 3 in these sampled paths.
- `crates/node/src/bin/asset_orchard_local_service.rs:188-415` and `orchard_frontier_cache_benchmark.rs:33-109,130-202`: read local listener, readiness preflight, loopback command parsing, bounded service input and benchmark argument validation; no finding in these sampled paths. `postfiat_cobalt_benchmark.rs:100-155`, `postfiat_cobalt_decisive_benchmark.rs:163-197,704-758`, and `postfiat_cobalt_liveness_simulation.rs:66-99,1321-1351`: read private-key output mode and simulation-only status exits; no finding in these sampled paths. The E5 live-drill and handoff-rehearsal binaries are three-line wrappers; their called modules are outside the named A5 source list and were not reviewed.

## Review limits and skips

The large bin and governance-agent modules were sampled at the named focus paths, not audited end to end. `crates/node/src/execution_actions.rs` was not reviewed: its state-transition bodies were burn 3 surface A2, and no distinct A5 command-dispatch path was found there. The `cli_dispatch_parts/group_*.rs` files and binaries' called service/protocol modules were not reviewed beyond the allowed A5 command sites. pfUSDC/Arc-specific code in node, excluded crates and files, previously reviewed crates, frozen artifacts, A1–A4, and B were not reviewed or edited. No Task Node or fleet action occurred. The P3 remains recorded without repair.
