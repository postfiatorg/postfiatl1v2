# Ambient P1 Remediation Plan

Status: active-code remediation substantially complete; raw inventory still open
Date: 2026-05-22
Inputs:

- `docs/status/ambient-p1-review-2026-05-22.md`
- `docs/status/ambient-p0-remediation-status-2026-05-22.md`
- `milestones/ambient-code-review-20260522T010605Z-023bbb6ba6/report.json`

## Goal

Clear the launch-relevant P1 backlog without pretending that all `677` raw P1
records are the same kind of work.

The plan separates:

- active runtime/source-code P1s,
- controlled-testnet versus public-launch requirements,
- examples and SDK ergonomics,
- operator scripts and remote automation,
- generated reports, archived evidence, and public-claims hygiene.

## Non-Goals

- Do not re-open P0 remediation unless a P1 fix reveals a missed P0.
- Do not claim "all P1s closed" until every raw P1 ID has a ledger
  disposition.
- Do not weaken consensus, proof, privacy, or validator safety to make tests
  pass.
- Do not turn controlled-testnet-only debug/privacy code into production claims.

## Completion Definitions

Active-code P1s are cleared when:

- every crate-level active runtime finding is fixed, downgraded with rationale,
  or explicitly scoped to example/debug-only use,
- targeted regression tests cover each behavioral fix,
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test --workspace --all-targets` pass,
- focused scans for public bind defaults, plaintext secret argv, unsafe report
  paths, and generated artifact leakage pass.

Raw P1 inventory is closed only when:

- every raw `P1-N` finding has a ledger row,
- every row has one of: fixed, duplicate, generated-artifact hygiene,
  archive-only, example-only, script backlog, downgraded with rationale, or
  false positive with proof,
- generated reports and archives are redacted, quarantined, or explicitly
  excluded from release artifacts.

## Phase 1: Active-Code Safety Fixes

These should be done first. They are the highest signal and smallest blast
radius compared with the script/report inventory.

### 1.1 Bridge Replay Cache Bound

Findings: `P1-4`

Files:

- `crates/types/src/lib.rs`
- `crates/bridge/src/lib.rs`

Problem:

- `BridgeState.replay_cache: Vec<String>` grows forever and is searched
  linearly.

Implementation plan:

- Add an epoch-aware replay structure while preserving deterministic serialized
  state.
- Keep replay entries for the current witness epoch and a configured trailing
  window.
- Reject same-domain/same-epoch/same-witness replay exactly as today.
- Prune older epochs only after successful transfer application boundaries, not
  during failed validation.
- Prefer deterministic containers and stable ordering for serialized state.

Tests:

- same witness in same epoch still fails,
- same witness in a later epoch remains distinct,
- old epochs are pruned after the retention window,
- replay pruning does not alter transfer sequence or domain caps.

Verification:

```bash
cargo test -p postfiat-bridge replay
cargo test -p postfiat-types bridge
```

### 1.2 Secure Atomic Writes

Findings: `P1-32`

File:

- `crates/storage/src/lib.rs`

Problem:

- `temp_write_path` uses a predictable `.{file}.{pid}.tmp` path.

Implementation plan:

- Avoid adding a new dependency unless there is a stronger reason.
- Use `OpenOptions::new().write(true).create_new(true)` for a randomized or
  nonce-suffixed temp path in the same parent directory.
- Retry on `AlreadyExists`.
- Keep `write_all`, file `sync_all`, atomic rename, and parent-directory sync.
- Remove the temp file on failures.

Tests:

- preexisting temp path does not get overwritten,
- symlink-like or existing temp collision fails/retries safely,
- successful write still persists valid JSON,
- failed serialization does not corrupt the target file.

Verification:

```bash
cargo test -p postfiat-storage atomic
```

### 1.3 History Archive Single-Read Import

Findings: `P1-18`

File:

- `crates/node/src/history.rs`

Problem:

- `import_history_archive_window` verifies `options.bundle_file`, then reads it
  again for import.

Implementation plan:

- Refactor verification so it returns both the verify report and the verified
  `HistoryArchiveWindowBundle`, or add an internal helper that reads bytes once
  and passes those bytes to both verification and import.
- Keep the public verify report shape stable unless callers require a change.
- Ensure the imported archive file path is derived from the verified bundle.

Tests:

- import uses the same verified bundle data,
- malformed bundle hash still fails,
- local genesis/domain mismatch still fails,
- duplicate bundle import behavior remains unchanged.

Verification:

```bash
cargo test -p postfiat-node history_archive
```

### 1.4 Strict Host And JSON Output Parsing

Findings: `P1-16`, `P1-33`

Files:

- `crates/network/src/lib.rs`
- `crates/types/src/lib.rs`

Problem:

- IPv6 detection is character-based rather than structural.
- Some structured outputs still use manual JSON string formatting.

Implementation plan:

- Replace IPv4/IPv6 detection with `std::net::{Ipv4Addr, Ipv6Addr}` parsing.
- Keep DNS host validation separate and reject multiaddr separators and control
  characters.
- Move remaining manually formatted structured JSON outputs to serde-backed
  serialization where the type already has or can safely derive `Serialize`.

Tests:

- reject malformed IPv6 values like `:::`, `1::2::3`, and empty segments where
  invalid,
- preserve valid IPv4, IPv6, and DNS host multiaddr output,
- serialize strings with control characters through serde escaping,
- preserve existing output schemas.

Verification:

```bash
cargo test -p postfiat-network
cargo test -p postfiat-types json
```

## Phase 2: Secret-File And SDK Hardening

### 2.1 Plaintext Private-Material File Boundaries

Findings: `P1-19`, `P1-20`, `P1-21`, related `P1-17`

File:

- `crates/node/src/node_types.rs`

Problem:

- `OrchardWalletKeyFile`, `WalletBackupFile`, and `ValidatorKeyRecord`
  serialize private material in plaintext.

Implementation plan:

- Keep compatibility for controlled local testnet files, but rename or document
  the types as private-material file formats where possible.
- Enforce `0600` file permissions on writes and reads for every path that
  contains `spending_key_hex`, `master_seed_hex`, or `private_key_hex`.
- Ensure reports never serialize these structs directly.
- Add redacted report types where reports need to mention key files.
- Track encrypted-at-rest formats as the public/custodial follow-up, not as a
  quick compatibility-breaking patch.

Tests:

- private-material writes create `0600` files on Unix,
- reads reject group/world-readable files on Unix,
- reports include paths and redaction flags, not raw private fields,
- existing controlled-testnet fixtures still load when permissions are safe.

Verification:

```bash
cargo test -p postfiat-node wallet_key
cargo test -p postfiat-node validator_key
cargo test -p postfiat-node orchard_key
```

### 2.2 Python SDK Temporary File Permissions

Findings: `P1-67`, `P1-68`

File:

- `python/postfiat_rpc/wallet.py`

Problem:

- Default temp work directories and JSON writes do not explicitly lock down
  permissions.

Implementation plan:

- Ensure SDK-created work directories are `0700`.
- Write wallet-adjacent JSON files through a helper that creates files with
  `0600` where the output may contain signed transactions, notes, seeds, or
  wallet-derived material.
- Keep public read/report outputs readable only when explicitly caller-owned.

Tests:

- unit test `_work_dir(None)` permissions on Unix,
- unit test `_write_json` private mode on Unix,
- no regression to existing wallet helper APIs.

Verification:

```bash
python -m pytest python
```

If there is no existing pytest setup, add narrow unit tests or a script-level
smoke command instead of inventing a large test framework.

## Phase 3: Protocol Encoding And Privacy Accounting

### 3.1 Canonical Hash Encoding

Findings: `P1-24`, `P1-25`, `P1-29`

Files:

- `crates/ordering_fast/src/lib.rs`
- `crates/proofs/src/lib.rs`
- `crates/privacy/src/lib.rs`

Problem:

- Consensus/proof/privacy hashes use `serde_json::to_vec`.

Implementation plan:

- Define a small local canonical encoding helper for current structs, or choose
  a workspace-wide deterministic encoding only after checking dependency impact.
- Preserve existing hash domains.
- Add golden vectors for proposal IDs, vote IDs, certificate IDs, proof
  statements, and debug shielded IDs.
- If changing existing IDs would break fixtures, version the hash domain or
  gate the migration clearly.

Tests:

- golden vector tests for each hash category,
- field-order and duplicate-input tests where applicable,
- reject empty or duplicate public proof inputs remains covered.

Verification:

```bash
cargo test -p postfiat-ordering-fast
cargo test -p postfiat-proofs
cargo test -p postfiat-privacy
```

### 3.2 Privacy State And Orchard Scan Accounting

Findings: `P1-26`, `P1-27`, `P1-28`

Files:

- `crates/privacy/src/lib.rs`
- `crates/privacy_orchard/src/verify.rs`

Problem:

- Debug notes derive position from vector length.
- Orchard authorization hash construction is local and needs golden-vector
  treatment.
- Orchard output scanning skips non-matching/decryption-failed outputs without
  accounting.

Implementation plan:

- Add a monotonic debug note counter to `ShieldedState` if compatible with
  persisted state; otherwise introduce a versioned migration/default.
- Keep current Orchard authorizing hash if it is intentionally domain-separated,
  but document the encoding and add golden vectors for the byte payload and
  final hash.
- Change scan return shape or add a new API that reports counts for decrypted,
  non-matching, malformed, and total outputs without breaking existing callers.

Tests:

- removed or nullified notes cannot reuse a previous note position,
- Orchard authorizing hash golden vectors cover fee and external binding hash,
- scanner distinguishes no-match from malformed/corrupt ciphertext.

Verification:

```bash
cargo test -p postfiat-privacy
cargo test -p postfiat-privacy-orchard
```

## Phase 4: Transport And Network Exposure

Findings: `P1-14`, `P1-22`, `P1-23`, `P1-31`, `P1-66`, `P1-468`, `WI-17`

Files:

- `crates/node/src/transport_cli.rs`
- `crates/consensus_cobalt/examples/rbc_nonuniform_tcp_drill.rs`
- `crates/rpc_sdk/examples/tcp_wallet_flow.rs`
- `python/postfiat_rpc/client.py`
- `scripts/docs-site-basic-auth-server`
- relevant docs and docs-site pages

Implementation plan:

- Short term: make all plaintext transports explicitly localhost/private-network
  controlled-testnet only in CLI help, docs, and examples.
- Add guards where feasible to reject public bind defaults unless an explicit
  unsafe/controlled flag is set.
- Add bounded read limits and pre-parse resource limits for transport listeners.
- Public-launch follow-up: design and implement authenticated transport
  (mTLS/Noise or equivalent) before exposing validator or wallet traffic to
  untrusted networks.
- Docs Basic Auth remains acceptable only behind SSH forwarding or TLS
  termination; do not present it as standalone internet security.

Tests:

- public bind requires explicit opt-in,
- localhost/private bind remains compatible,
- oversized pre-auth payload fails before expensive parsing,
- docs examples show localhost or reverse proxy/TLS.

Verification:

```bash
cargo test -p postfiat-node transport
cargo test -p postfiat-rpc-sdk
```

## Phase 5: Example And Report Output Hardening

Findings: `P1-5` through `P1-13`, `P1-30`, and related script report-path P1s

Files:

- `crates/consensus_cobalt/examples/*.rs`
- `crates/rpc_sdk/examples/tcp_wallet_flow.rs`
- selected report-generating scripts

Implementation plan:

- Add a shared safe report-output helper for Rust examples:
  - reject absolute paths unless explicitly allowed,
  - reject `..` components,
  - require output under a caller-approved report root,
  - create parent directories safely,
  - write with restricted permissions where contents are sensitive.
- Redact trust graph roots or mark them as internal evidence unless the report is
  meant for publication.
- Keep example binaries out of production release packaging unless their output
  paths and transport warnings are safe.

Tests:

- malicious `REPORT=../../...` is rejected,
- absolute report paths are rejected by default,
- valid report path under the approved root succeeds,
- trust graph/internal fields are omitted or classified.

Verification:

```bash
cargo test -p postfiat-consensus-cobalt
```

## Phase 6: Operator Script Hardening

Findings: most `scripts/` P1s, especially `WI-09`, `WI-13`, and script parts of
`WI-11`, `WI-12`, `WI-16`, `WI-17`

Priority order:

1. scripts handling private keys, master seeds, SSH credentials, sudo passwords,
   or credential files,
2. scripts that run remote commands or generate remote shell/systemd files,
3. scripts that package release artifacts or evidence for publication,
4. scripts that make public launch/readiness claims,
5. scripts with destructive filesystem operations such as `rm -rf`,
6. scripts that perform redaction or secret scans.

Implementation plan:

- Create shared shell/Python helpers for:
  - validated paths under approved roots,
  - safe JSON writing,
  - structural redaction of JSON artifacts,
  - command argv arrays instead of shell strings,
  - pinned SSH host-key policy,
  - private temp directories and cleanup traps.
- Fix live/release/remote scripts before devnet-only smoke scripts.
- Replace grep-only secret detection with structural JSON traversal where
  artifacts are JSON.
- Add dry-run and negative-injection smoke tests for remote command builders.

Acceptance:

- every promoted live operator script has a negative injection test,
- every script writing private material uses private dirs/files,
- every public evidence pack runs a structural redaction check,
- no release/readiness script depends on stale generated reports as current
  evidence.

Verification:

```bash
shellcheck scripts/<changed-script>
git diff --check
```

Use `shellcheck` only if it is installed; otherwise record that it was not
available and run the relevant script smoke tests.

## Phase 7: Generated Artifact And Claims Ledger

Findings: most `reports/`, `work_archive/`, docs, and docs-site P1s

Implementation plan:

- Create `docs/status/ambient-p1-ledger-2026-05-22.md` or a machine-readable
  JSON/CSV ledger.
- Preserve every raw `P1-N` ID from Ambient.
- Assign each row:
  - `bucket`,
  - `status`,
  - `final_severity`,
  - `rationale`,
  - `fix_commit` or `evidence_path`.
- Quarantine or redact generated reports that contain sensitive paths,
  fingerprints, topology internals, fixture secrets, or stale failed-gate
  conclusions.
- Update public docs so controlled-testnet claims are separated from
  public-launch topology/privacy/transport claims.

Acceptance:

- every raw P1 ID has a disposition,
- generated artifacts are either safe to publish or excluded from release,
- docs no longer cite stale failed/contradictory evidence as current launch
  truth,
- final P1 closeout document distinguishes active-code closure from raw-inventory
  closure.

## Suggested Commit Slices

1. `p1-storage-history-network-types`: atomic writes, history single-read import,
   IP parsing, remaining serde JSON cleanup.
2. `p1-bridge-replay`: bounded bridge replay cache and tests.
3. `p1-private-material-files`: key-file permissions, redacted report surfaces,
   Python SDK temp permissions.
4. `p1-canonical-hashes`: canonical hash helpers and golden vectors.
5. `p1-privacy-accounting`: debug note counter, Orchard scan accounting,
   Orchard hash vectors.
6. `p1-transport-scope`: transport exposure guards, docs, pre-auth limits.
7. `p1-example-report-paths`: Cobalt example/report helper and tests.
8. `p1-operator-scripts-wave1`: live/release/remote script hardening.
9. `p1-ledger-artifact-redaction`: raw ID ledger and generated artifact policy.

## Global Verification Gate

Run after each code-heavy slice:

```bash
git diff --check
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
```

Run after docs/scripts/artifact slices:

```bash
git diff --check
rg -n '0\.0\.0\.0|POSTFIAT_DOCS_PASSWORD=replace|StrictHostKeyChecking=accept-new|sudo -S|bash -c' \
  scripts docs docs systemd crates python -g '!target'
rg -n 'private_key_hex|master_seed_hex|spending_key_hex|BEGIN [A-Z ]*PRIVATE KEY' \
  scripts docs docs systemd crates python -g '!target'
```

Interpret scans by context. A positive hit can be acceptable only if it is a
test fixture, private-material type definition, detection guard, or explicitly
documented controlled-only example.

## Current Status

- P0 active-code blockers: cleared in current working tree.
- P1 review: complete.
- P1 remediation: Phase 1 complete; Phase 2 and the active-code portions of
  Phase 3 and Phase 4 are substantially complete in current working tree.
  Completed items:
  - bounded bridge replay cache while preserving historical replay rejection,
  - secure create-new atomic storage temp files,
  - single-read verified history archive import helper,
  - structural IPv4/IPv6 host parsing for network multiaddrs,
  - serde-backed `StatusReport` JSON output for strict escaping,
  - private-permission enforcement for local private-material files,
  - Python SDK private temp/output file permissions,
  - explicit length-delimited hash encodings and golden vectors for HotStuff,
    proof statements, and debug shielded commitments,
  - monotonic debug shielded note positions,
  - Orchard scan accounting for decrypted, non-matching, and malformed outputs,
  - public/wildcard/DNS transport bind opt-in guard for plaintext transport
    listeners.
- Raw P1 ledger: not created.
- Remaining P1 work: example report-path hardening, operator-script
  hardening, generated artifact/public-claims ledger, encrypted-at-rest key
  format design, authenticated public transport, and Orchard authorizing hash
  public vector/spec work remain open.
