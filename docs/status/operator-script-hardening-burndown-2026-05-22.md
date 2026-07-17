# Operator Script Hardening Burndown

Status: open milestone
Date: 2026-05-22
Source review: `milestones/ambient-code-review-20260522T010605Z-023bbb6ba6/`

## Scope

The Ambient raw inventory contains 287 operator-automation backlog rows in the
new disposition ledger:

- 74 original P0 rows under `scripts/`
- 212 original P1 rows under `scripts/`
- 1 original P1 row under `.github/workflows/`

These rows are not active Rust runtime blockers, but scripts used for live
controlled-testnet operations must be hardened before public or unattended
operations rely on them.

## Current Status

| Workstream | Status | Notes |
| --- | --- | --- |
| Live runtime scripts | Partial | The previously promoted `scripts/ai-job` execution path was hardened in the P0 pass. |
| Remote SSH/testnet scripts | Open | Needs centralized SSH option handling, host-key policy, and remote command construction. |
| Release/provision scripts | Open | Needs private-material input policy, env allowlists, and artifact redaction gates. |
| Docs-host scripts | Partial | Active docs serving defaults were moved to localhost/explicit exposure policy in the P0 pass. |
| CI/docs workflow | Open | Dependency pinning/hash policy remains release hygiene backlog. |

## First Tranche

1. Identify the scripts that are allowed in live controlled-testnet operations.
2. Move shared remote command construction into one shell/Python helper.
3. Reject unknown environment variables in live remote operations.
4. Remove sudo-password stdin patterns from live scripts.
5. Replace `StrictHostKeyChecking=accept-new` in live scripts with an explicit
   known-hosts workflow or a documented one-time enrollment command.
6. Add negative tests for command injection and unsafe env propagation.

## Tracking Contract

The machine-readable row-level source is:

- `docs/status/ambient-finding-disposition-ledger.json`, a manifest whose row
  shards live under `docs/status/ambient-finding-disposition-ledger/`

Rows with `status = "script_backlog"` must be closed one script at a time with
focused evidence. Until then, the accurate claim is:

> Active runtime blockers are cleared; operator automation hardening remains an
> open milestone.
