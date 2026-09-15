# Node startup, release verification, and RPC serving review — 2026-09-15

This is A3 of the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). The review read focused serving and startup paths in `crates/node/src/rpc_cli.rs`, `transport_runtime.rs`, `lifecycle_queries.rs`, and `node_types.rs`, plus the `deployment-manifest-verify` dispatch and its called verification path in `batch_snapshot.rs`. The deployment verifier's signed envelope and locally hashed runtime and service artifacts were traced; unrelated snapshot logic was not audited. The review limits are stated below.

## Findings

### 1. P2 — keep-alive RPC connections omit earlier request events

**Source:** `crates/node/src/rpc_cli.rs:767-812` and `:906-936`.

A keep-alive connection can submit two valid newline-delimited requests, including a finality submission followed by `status`. The worker emits only `last_event` when the connection closes; the earlier submission receives a response but has no event-log row, no entry in the returned request report, and no contribution to the report's method and error counts. An operator interpreting the event stream or summary cannot account for all requests the service actually handled. The `--keep-alive` setting is present in the generated RPC systemd unit.

The minimal repair is to send every completed request event to the receiver as it completes, poll for these events while awaiting the next connection, count closure separately, and bound the number of events one keep-alive connection can retain. Add regressions for two events on one connection, logging before an open socket closes, and closing at the event limit. This changes RPC presentation and telemetry, not consensus rules or signed/hashed bytes.

### 2. P2 — startup publishes ready files before all serving preflights finish

**Source:** `crates/node/src/rpc_cli.rs:659-681` and `crates/node/src/transport_runtime.rs:949-988`.

The RPC server writes `ready: true` before building its health-cache stamps; if metadata for an already-loaded state file cannot be read, startup returns an error after having published a ready file. The validator transport service writes its ready report after bind and prewarm but before setting the listener nonblocking; a nonblocking-configuration error likewise returns with a ready file. A local readiness consumer can observe a positive marker for a process that has not reached its accept loop, until its next startup clears the marker.

The minimal repair is to complete the RPC cache preflight and the transport listener's nonblocking configuration before writing readiness. Regress each ordering boundary by forcing the preflight failure and asserting no positive ready marker is produced. These changes affect service startup behavior only and are not consensus-affecting.

### 3. P3 — runtime identity reporting does not reverify the configured manifest signature

**Source:** `crates/node/src/lifecycle_queries.rs:902-917`.

When `POSTFIAT_DEPLOYMENT_MANIFEST` is present, the status report parses the manifest and hashes its current file bytes, but does not reverify its publisher signature or time window. After the systemd `ExecStartPre` verification has succeeded, a locally replaced manifest with altered signed fields can yield a newly reported deployment-manifest hash in a still-running process even though those bytes were never authorized by that verification. The serving process's prestart check remains a separate gate; this observation concerns the runtime report's evidentiary meaning, not proof of a change to validator consensus state.

A future minimal repair would bind runtime reporting to a separately trusted publisher key and verify the manifest on each report, or explicitly mark unverified current-file identity as such. This P3 is recorded without repair.

## Areas with no findings

- `crates/node/src/rpc_cli.rs:606-658,1049-1145,3693-3849,3883-3997`: reviewed bind-host validation, initial state and spool probing, newline and byte bounds, RPC protocol and method gating, private unique spool creation and cleanup, and child timeout/kill paths had no additional findings.
- `crates/node/src/transport_runtime.rs:899-949,989-1160` and its focused frame handling: reviewed topology membership and local status checks, controlled binding, worker in-flight slots, socket timeouts, active-stream shutdown, and failure propagation had no additional findings.
- `crates/node/src/lifecycle_queries.rs:870-1040` and `crates/node/src/node_types.rs` startup/report option and readiness shapes: reviewed paired runtime-artifact settings, local binding comparison, status fields, and serving option types had no additional findings beyond finding 3.
- `crates/node/src/main_parts/cli_dispatch_parts/group_05.rs:3052-3092` and `crates/node/src/batch_snapshot.rs:2300-2435,2657-2766,2870-2965`: reviewed systemd prestart verification invocation, publisher signature and window checks, canonical binding validation, and comparisons of locally hashed binary, topology, circuits, units, and environment to the signed manifest had no additional findings.

## Review limits and skips

This was a focused review, not a whole-file audit. In the four named large node modules, unrelated asset, wallet, bridge, Orchard, FastPay/FastSwap state-transition, governance, archive/replay, catch-up, and transaction construction paths were not audited. Only the deployment-manifest verifier's dispatch, verification, and generated prestart invocation were traced in the additional call-path files; other snapshot and command operations were not reviewed. The systemd template files outside that generated path supplied no separate A3 review. Previously reviewed A1/A2 and burn 3 crates, excluded crates and files, frozen artifacts, A4/A5, and B were not reviewed. No Task Node or fleet action occurred.

## Repair result

P2 finding 1 is repaired by forwarding each completed request event from a keep-alive worker and draining events during nonblocking accept polling. A separate closure marker decrements the count of active connections. A 64-request limit per connection bounds retained events even if a peer keeps one socket open. Loopback regressions check two `status` requests on one socket, one event logged before its socket closes or another connection arrives, and prompt closure with all 64 events retained at the request limit. Existing accept-budget and stalled-client regressions remain green. The P3 report observation remains recorded without repair.

P2 finding 2 is repaired by moving RPC readiness after both health-cache stamps are built and configuring the validator transport listener as nonblocking inside its bind preflight, before the ready report. The RPC regression initializes a node, removes the optional mempool file so its metadata stamp fails *after* its initial state can be read, and checks startup has no ready marker. The transport regression forces listener-mode configuration failure after bind and checks its prewarm/bind/ready gate never writes a positive marker. The adjacent prewarm-ordering regression passes.

These changes affect RPC telemetry and local serving startup only. **No repair is consensus-affecting**; no live behavior, activation, or deployment is claimed. **Full Rust suite verdict pending** CI.

Post-repair verification:

- `cargo check -p postfiat-node --locked`: passed.
- `cargo test -p postfiat-node rpc_serve_keep_alive_records_each_request_and_closes_one_connection --bin postfiat-node --locked`: 1 passed, including after the per-connection bound.
- `cargo test -p postfiat-node rpc_serve_keep_alive_closes_at_retained_request_limit --bin postfiat-node --locked`: 1 passed.
- `cargo test -p postfiat-node rpc_serve_logs_completed_request_while_keep_alive_socket_is_open --bin postfiat-node --locked`: 1 passed.
- `cargo test -p postfiat-node rpc_serve_health_preflight_failure_keeps_ready_file_absent --bin postfiat-node --locked`: 1 passed.
- `cargo test -p postfiat-node transport_listener_mode_failure_prevents_ready_report --bin postfiat-node --locked`: 1 passed. An initial `--lib` filter selected zero tests because this test belongs to the node binary, so the binary filter was run and passed.
- `cargo test -p postfiat-node rpc_serve_accept_budget_is_exact_at_every_small_boundary --bin postfiat-node --locked`: 1 passed.
- `cargo test -p postfiat-node transport_startup_after_prewarm_blocks_bind_until_prewarm_ready --bin postfiat-node --locked`: 1 passed.
- `cargo test -p postfiat-node rpc_serve_drops_stalled_client_reads_without_blocking_other_connections --bin postfiat-node --locked`: 1 passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

The affected sources are `rpc_cli.rs`, `transport_runtime.rs`, and their focused binary regression modules; no deployment-manifest verification, signed bytes, protocol state, frozen artifacts, excluded crates, or previously reviewed implementations were changed.
