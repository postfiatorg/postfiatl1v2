# Node startup, release verification, and RPC serving review — 2026-09-15

This is A3 of the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). The review read focused serving and startup paths in `crates/node/src/rpc_cli.rs`, `transport_runtime.rs`, `lifecycle_queries.rs`, and `node_types.rs`, plus the `deployment-manifest-verify` dispatch and its called verification path in `batch_snapshot.rs`. The deployment verifier's signed envelope and locally hashed runtime and service artifacts were traced; unrelated snapshot logic was not audited. The review limits are stated below.

## Findings

### 1. P2 — keep-alive RPC connections omit earlier request events

**Source:** `crates/node/src/rpc_cli.rs:767-812` and `:906-936`.

A keep-alive connection can submit two valid newline-delimited requests, including a finality submission followed by `status`. The worker emits only `last_event` when the connection closes; the earlier submission receives a response but has no event-log row, no entry in the returned request report, and no contribution to the report's method and error counts. An operator interpreting the event stream or summary cannot account for all requests the service actually handled. The `--keep-alive` setting is present in the generated RPC systemd unit.

The minimal repair is to send every completed request event to the receiver, while counting a connection as closed only on its final event. Add a regression that checks two events on one connection are retained and only its final event releases the active-connection slot. This changes RPC presentation and telemetry, not consensus rules or signed/hashed bytes.

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
