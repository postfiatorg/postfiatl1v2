# Ambient P0/P1 Remediation Triage

Status: reviewed remediation triage
Date: 2026-05-22
Scope: Ambient review `20260522T010605Z-023bbb6ba6` against commit `fd80ae5157992e1d12548ff732101d1aa728c08e`

This document is the core-repo working plan for the Ambient P0/P1 output. The
full generated artifact remains at
`milestones/ambient-code-review-20260522T010605Z-023bbb6ba6/`; this file is the
human-reviewed triage layer for controlled-testnet engineering.

## Reviewer Verdict

The raw count, `198` P0 and `677` P1, should be treated as an evidence inventory,
not as `875` independent production exploits. The count is inflated by generated
reports, archived docs, duplicated harness findings, and repeated instances of
the same defect class.

The review still surfaced real launch-relevant work:

- Key material is represented and passed through plain strings/`Vec<u8>` in
  multiple crate and operator surfaces.
- Debug proof/privacy adapters must remain explicitly non-production and must
  not support production privacy claims.
- Some parser and report-writing paths use non-canonical or overly permissive
  mechanisms.
- Several remote-operator scripts mix SSH password automation, sudo prompts,
  generated shell, and evidence redaction in ways that need stricter boundaries.

For controlled transparent testnet, the actionable blocker set is much smaller
than 198, but not zero. Treat crate-level key handling, CLI secret exposure,
debug-proof gating, custom JSON parsing for protocol objects, public bind
defaults, and live-operator credential flows as the first-pass blocker set.

## Raw Count Review

P0 path distribution:

| Path class | Count | Triage read |
| --- | ---: | --- |
| `reports/` | 96 | Mostly generated evidence hygiene; block only if reports are shipped/published with sensitive data. |
| `scripts/` | 74 | Operator/harness risk; block when used for live controlled-testnet operations. |
| `crates/` | 13 | Highest-priority code review queue. |
| `docs/` | 5 | Operator misconfiguration risk. |
| `work_archive/` | 4 | Should not be launch-blocking unless copied into active docs. |
| `docs/` | 2 | Public documentation/ops risk. |
| `python/` | 2 | SDK helper process-argument risk. |
| `systemd/` | 2 | Deployment default risk. |

P1 path distribution:

| Path class | Count | Triage read |
| --- | ---: | --- |
| `reports/` | 377 | Evidence hygiene and redaction policy backlog unless published. |
| `scripts/` | 212 | Operator/harness hardening backlog; promote only live-deploy paths. |
| `crates/` | 30 | Code-level P1 queue. |
| `docs/` | 24 | Documentation/operator safety backlog. |
| `work_archive/` | 18 | Archive hygiene, not launch blocker. |
| Other | 16 | Inspect individually. |

## Spot-Check Findings

The following checks were performed against the current tree, not just the
generated report.

1. Key material lifecycle is a real issue, though the remediation should be
   grouped by API boundary rather than handled as dozens of separate P0s.
   `crates/crypto_provider/src/lib.rs` stores `MlDsa65KeyPair.private_key` as
   `Vec<u8>` and derives `Clone`; `crates/node/src/block_finality.rs` decodes
   validator private keys at current lines 1613, 1897, and 2740; `crates/rpc_sdk/src/main.rs`
   accepts `--master-seed-hex` at current lines 627-635; and
   `python/postfiat_rpc/wallet.py` passes seed material through child process
   argv at current lines 179-195 and 223-235.

2. Debug proof handling is real but claim-scoped. `crates/proofs/src/lib.rs`
   defines `DebugProofSystem` and deterministic `debug_proof_hash` verification.
   This is acceptable only as an explicit debug adapter. It is a blocker for
   any production privacy claim, but not automatically a blocker for a
   transparent-only controlled testnet if all privacy/debug paths are gated and
   documented as non-production.

3. The custom JSON parser finding is valid enough to prioritize. `crates/types/src/lib.rs`
   still has manual `to_json` / `from_json` paths for core structs and
   `extract_json_string` performs delimiter-based string extraction rather than
   using `serde_json`. If these structs are hashed, signed, persisted, or replayed
   across validators, the canonical fix is to replace those manual paths with
   typed serde parsing and regression tests for escaped strings.

4. The P1 `REPORT` path findings in Cobalt examples are real harness hygiene,
   not production-chain P1 by themselves. Example code such as
   `crates/consensus_cobalt/examples/cobalt_adversarial_harness.rs` writes to an
   environment-selected report path. This is acceptable only when the harness
   controls the environment and output root; otherwise it needs containment.

5. The generated plan is coverage-complete but too coarse. It covers all P0/P1
   IDs, but most work items say "apply source recommendations" rather than
   naming exact patches. This document is the implementation triage that should
   drive remediation.

## Severity Policy

Use these buckets before coding:

- **Blocker P0:** active runtime code, release packaging, systemd defaults, live
  remote-deploy scripts, or public operator docs can leak keys, bypass proof or
  signature checks, alter consensus/state safety, or expose public services.
- **P1 before wider launch:** examples, local harnesses, generated evidence,
  docs, and SDK helpers that can mislead operators or become dangerous when used
  with real credentials.
- **Evidence hygiene:** generated reports, old archived plans, placeholder
  packet data, and work archive files. These should be redacted or excluded from
  release artifacts, but they are not equivalent to a live runtime exploit.

## P0 Remediation Plan

| ID | Source generated item | Raw count | Reviewed priority | Required remediation |
| --- | --- | ---: | --- | --- |
| SEC-P0-001 | WI-01: Secure secret and key material lifecycle | 41 | Blocker for live keys | Introduce explicit secret/key wrapper boundaries for ML-DSA private keys and wallet seeds. Avoid `Clone` on secret-owning structs, zeroize decoded key bytes, and forbid accidental serialization/logging of secret fields. |
| SEC-P0-002 | WI-02: Replace placeholder and deterministic cryptographic artifacts | 80 | Split | For active code/docs, remove hardcoded seeds, all-zero digests, dummy keys, and deterministic production defaults. For generated reports, classify as fixture data or regenerate with redacted fixture markers. |
| SEC-P0-003 | WI-03: Gate debug cryptography and proof bypasses | 2 | Privacy blocker | Ensure `DebugProofSystem`, debug nullifiers, and debug privacy state cannot be used by production or production-like binaries. Add explicit feature/config gating and fail-closed tests. |
| SEC-P0-004 | WI-04: Harden SSH, shell, and remote deployment execution | 32 | Blocker for live remote ops | Replace unsafe command-template substitution with argv/structured scripts, require pinned host keys, remove sudo-password stdin flows where possible, and contain credential files outside committed reports. |
| SEC-P0-005 | WI-05: Restrict network exposure | 3 | Deployment blocker | Default docs/RPC/systemd bind addresses must be localhost or explicitly operator-configured; public binds require TLS/auth/reverse-proxy guidance. |
| SEC-P0-006 | WI-06: Redact sensitive operational data from artifacts and reports | 21 | Release artifact blocker | Reports must not publish host/user fingerprints, private material paths, or credential locators unless explicitly classified and access-controlled. |
| SEC-P0-007 | WI-07: Constrain dynamic execution and automation authority | 18 | Operator safety blocker | Remove unbounded automation authority from live scripts; require allowlisted commands, dry-run review, and bounded approval for remote execution. |
| SEC-P0-008 | WI-08: Fix consensus, privacy, and verification correctness | 1 | Protocol blocker if active | Triage the single consensus/privacy finding directly against the touched code and add a replay/regression test before closing it. |

P0 acceptance criteria:

- Every P0 ID is dispositioned as fixed, downgraded with rationale, duplicate,
  generated-artifact hygiene, or false positive.
- Active runtime and live-operator P0s have targeted tests or smoke evidence.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test --workspace --all-targets` pass after code changes.
- Secret scans pass over active source/docs/scripts while excluding
  `milestones/`, historical generated `reports/` when intentionally archived,
  and `work_archive/` unless those artifacts are promoted.
- Controlled-testnet release notes state whether privacy is debug-only,
  alpha-gated, or production-grade.

## P1 Remediation Plan

| ID | Source generated item | Raw count | Reviewed priority | Required remediation |
| --- | --- | ---: | --- | --- |
| SEC-P1-001 | WI-09: Constrain dynamic execution and automation authority | 156 | Operator hardening | Build shared helpers for remote command construction, environment allowlists, and report path containment. |
| SEC-P1-002 | WI-10: Remaining P1 findings | 22 | Triage queue | Split remaining findings by code/doc/generated artifact and either promote to P0, fix as P1, or downgrade with rationale. |
| SEC-P1-003 | WI-11: Secure secret/key lifecycle | 128 | Pre-launch hardening | Finish non-P0 secret hygiene: redaction tests, backup-file permission checks, CLI warnings, and SDK helper secret transport. |
| SEC-P1-004 | WI-12: Redact operational data from artifacts/reports | 243 | Evidence hygiene | Define redaction schema for generated JSON reports, host fingerprints, private paths, and fixture secret labels. Regenerate or quarantine historical reports. |
| SEC-P1-005 | WI-13: Harden SSH/shell/remote deployment | 68 | Operator hardening | Remove `StrictHostKeyChecking=accept-new`, replace password automation where feasible, and test injection-negative payloads. |
| SEC-P1-006 | WI-14: Consensus/privacy/verification correctness | 7 | Protocol review | Add targeted protocol regression tests for each active-code finding before closing. |
| SEC-P1-007 | WI-15: Gate debug cryptography/proof bypasses | 4 | Privacy hardening | Add negative tests proving production/privacy-alpha commands reject debug proof artifacts. |
| SEC-P1-008 | WI-16: Placeholder deterministic crypto artifacts | 46 | Fixture hygiene | Move fixtures under explicit test-only paths, mark fake values as fixtures, and prevent them from entering release/operator artifacts. |
| SEC-P1-009 | WI-17: Restrict network exposure | 3 | Deployment hardening | Add config tests and docs ensuring public service exposure is explicit and authenticated. |

P1 acceptance criteria:

- All P1 IDs are triaged into active-code, operator-script, docs, generated
  artifact, archive, duplicate, or false-positive buckets.
- Every active-code P1 has a targeted unit/integration/property test or a
  written reason why a static check is sufficient.
- All report path writes are rooted under a caller-approved output directory or
  reject traversal/absolute paths where the caller is not trusted.
- Generated evidence uses a stable redaction contract and cannot leak raw
  private keys, master seeds, SSH passwords, or exact host/user fingerprints by
  default.

## Execution Order

1. Build a machine-readable triage ledger for the 875 raw P0/P1 IDs with fields:
   `id`, `file`, `bucket`, `owner`, `status`, `final_severity`, and `rationale`.
2. Fix active-code P0s first: secret/key lifecycle, CLI secret argv, debug proof
   gating, manual JSON parsing, public bind defaults, and live remote-ops command
   construction.
3. Add release-gate scans for active source/docs/scripts and a separate
   generated-artifact redaction gate.
4. Regenerate or quarantine historical reports that contain fixture keys,
   fingerprints, or private paths.
5. Work through P1s after P0 blockers are closed or explicitly downgraded.

## Verification Commands

Minimum commands after each remediation slice:

```bash
git diff --check
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Targeted scans:

```bash
! rg -n 'private_key_hex|master_seed_hex|spending_key_hex|ssh_cred|BEGIN [A-Z ]*PRIVATE KEY|machine_[0-9]+_(password|private_key)' \
  . --glob '!.git/**' --glob '!milestones/**' --glob '!reports/**' --glob '!work_archive/**' --glob '!target/**'

rg -n 'DebugProofSystem|debug_nullifier|debug_proof_hash' crates scripts docs docs
rg -n 'StrictHostKeyChecking=accept-new|sudo -S|pexpect|shell=True|REPORT' scripts crates/consensus_cobalt/examples
```

## Source Artifacts

- Full report: `milestones/ambient-code-review-20260522T010605Z-023bbb6ba6/report.md`
- Generated remediation plan:
  `milestones/ambient-code-review-20260522T010605Z-023bbb6ba6/remediation-plan.md`
- Remediation audit:
  `milestones/.ambient-code-review/runs/20260522T010605Z-023bbb6ba6/remediation-plan-artifacts/audit.json`
