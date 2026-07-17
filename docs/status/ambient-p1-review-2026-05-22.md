# Ambient P1 Review

Status: reviewed; active-code remediation substantially complete in current working tree
Date: 2026-05-22
Scope: Ambient review `20260522T010605Z-023bbb6ba6` against commit `fd80ae5157992e1d12548ff732101d1aa728c08e`
Current tree: P0 remediation changes are present but uncommitted

## Review Verdict

The raw P1 count is `677`. As with the P0 inventory, this is an evidence
inventory, not `677` equally urgent live-runtime defects.

The P1 set is still real work. It should not block the current controlled
transparent testnet after the active-code P0 fixes, but several P1 classes are
pre-public-launch blockers:

- runtime resource exhaustion and local data-corruption risks,
- plaintext wallet, Orchard, and validator secret-file formats,
- unauthenticated/plaintext transport examples and testnet transport listeners,
- canonical serialization gaps for consensus/proof/privacy hashes,
- privacy accounting gaps that are acceptable only under controlled-testnet
  claims,
- remote-ops script authority, redaction, and credential-handling backlog,
- historical/generated evidence hygiene.

Safe claim now:

> Active-code P1s needed for controlled-testnet hardening have been
> substantially remediated in the current working tree. Raw P1 inventory
> closure remains open.

Unsafe claim:

> P1s are closed or launch-irrelevant.

## Raw P1 Inventory

| Path class | Count | Review read |
| --- | ---: | --- |
| `reports/` | 377 | Mostly generated evidence hygiene, contradictory historical gate reports, topology/funding-claim evidence, and redaction backlog. Not active runtime unless published as current evidence. |
| `scripts/` | 212 | Real operator hardening backlog. Promote scripts used for live remote operation, release packaging, key handling, or public claims. |
| `crates/` | 30 | Highest-priority active-code queue. Mix of runtime issues, examples, and controlled-testnet/debug-only protocol debt. |
| `docs/` | 24 | Operator safety and public-claims wording backlog. |
| `work_archive/` | 18 | Archive hygiene unless reused in active docs. |
| `docs/` | 6 | Hosted docs/auth/redaction/operator guidance backlog. |
| `python/` | 4 | SDK transport and temp-file permissions backlog. |
| Other root files | 6 | `README.md`, `roadmap.md`, `mkdocs.yml`, `.github`, and `Cargo.lock` review individually. |

## Highest-Priority Active-Code Findings

1. `P1-4`: bridge replay cache is unbounded.

   Current code stores replay keys in `BridgeState.replay_cache: Vec<String>`
   and appends one entry per accepted transfer. This is a real valid-traffic DoS
   path if bridge traffic is enabled beyond controlled testing.

   Files: `crates/types/src/lib.rs`, `crates/bridge/src/lib.rs`

   Required fix: make replay retention epoch-aware and bounded, and use an
   indexed membership structure rather than a linear vector scan. Add a
   regression that old epochs prune while same-epoch replay still fails.

2. `P1-32`: storage atomic-write temp path is predictable.

   `temp_write_path` derives the temp name from the target filename and process
   ID. This leaves a local symlink/data-corruption surface around state writes.

   File: `crates/storage/src/lib.rs`

   Required fix: use secure create-new temp files under the target parent
   directory, preferably `tempfile::NamedTempFile::new_in`, and persist/rename
   only after fsync. Add a symlink/preexisting-temp negative test.

3. `P1-19`, `P1-20`, `P1-21`: key-file types serialize private material in
   plaintext.

   `OrchardWalletKeyFile`, `WalletBackupFile`, and `ValidatorKeyRecord` still
   derive `Serialize`/`Deserialize` with plaintext `spending_key_hex`,
   `master_seed_hex`, and `private_key_hex` fields. The P0 pass improved CLI
   argv and zeroizing boundaries, but these file formats remain plaintext secret
   formats.

   File: `crates/node/src/node_types.rs`

   Required fix: explicitly scope these as controlled local private-material
   files, enforce permissions everywhere they are read or written, and plan an
   encrypted-at-rest format before public/custodial use. Do not let these types
   appear in reports or public artifacts.

4. `P1-22`, `P1-23`: testnet transport accepts unauthenticated TCP and uses
   blocking per-connection I/O.

   The transport listeners authenticate after accepting and reading application
   data. Some paths are bounded by `max_peers`, `max_batches`, or
   `max_connections`, but the pre-auth read surface and blocking model are still
   not suitable for untrusted/public exposure.

   File: `crates/node/src/transport_cli.rs`

   Required fix: keep localhost/private-network-only for controlled testnet.
   Before public exposure, add an authenticated channel such as mTLS/Noise,
   resource limits before application parsing, and bounded async or worker-pool
   concurrency.

5. `P1-24`, `P1-25`, `P1-29`: consensus/proof/privacy hashes rely on
   `serde_json::to_vec`.

   Current code hashes derived JSON serialization in ordering, debug proof
   statements, and debug shielded commitments. This is probably stable for the
   current Rust-only structs that do not use maps, but it is not a protocol
   serialization contract.

   Files: `crates/ordering_fast/src/lib.rs`, `crates/proofs/src/lib.rs`,
   `crates/privacy/src/lib.rs`

   Required fix: define a canonical protocol hash encoding for signed/verified
   objects before cross-implementation consensus or production privacy claims.
   Add golden vectors.

6. `P1-26`, `P1-27`, `P1-28`: privacy correctness/accounting issues remain.

   The debug shielded state derives note position from vector length, Orchard
   authorization hashing uses a local field-concatenation scheme, and Orchard
   output scanning silently skips non-matching/decryption-failed outputs. These
   are acceptable only under controlled-testnet/debug or audited-production
   boundaries.

   Files: `crates/privacy/src/lib.rs`, `crates/privacy_orchard/src/verify.rs`

   Required fix: use monotonic note counters for debug state, document and
   golden-vector the Orchard auth hash encoding, and return scan accounting that
   distinguishes decrypted, non-matching, and malformed/corrupt outputs.

7. `P1-18`: history archive import has a verify/read TOCTOU gap.

   `import_history_archive_window` verifies `options.bundle_file`, then reads
   it again for import. A local attacker who can modify the bundle path between
   the two reads can race the verified contents.

   File: `crates/node/src/history.rs`

   Required fix: make verification return the verified bundle or read the bytes
   once and pass the same bytes to verification and import.

8. `P1-16`, `P1-33`: parsing/serialization edges remain after P0 fixes.

   IPv6 validation still uses a character check instead of `std::net::Ipv6Addr`.
   The P0 pass fixed `Genesis` and `NodeState` JSON parsing, but manual
   `to_json` formatting still exists for other status/report types.

   Files: `crates/network/src/lib.rs`, `crates/types/src/lib.rs`

   Required fix: use standard IP parsers and move remaining manual JSON output
   for structured types to serde serialization.

## Lower-Priority Or Scoped Crate Findings

| Finding | Review status |
| --- | --- |
| `P1-15` deterministic ML-DSA seed length | Likely stale or overbroad because the current API type-checks with `&[u8; 32]`. Keep open until a conformance test or upstream API citation proves the seed length contract. |
| `P1-17` private key hex retained in governance | Partially remediated by zeroizing decoded bytes. Still true that deserialized key-file structs hold plaintext `String` fields until drop. Track with the plaintext key-file format work. |
| `P1-5` through `P1-13` Cobalt example `REPORT` writes | Example/harness hardening. Not live runtime, but should be fixed with a shared safe-report-path helper before using these examples in CI or release gates. |
| `P1-14`, `P1-31` plaintext TCP examples | Example-only if kept loopback/private. Must be loudly non-production or upgraded before recommending for real wallet/validator use. |
| `P1-30` wallet backup example path read | Example-only pattern. Keep out of production SDK paths or validate/canonicalize if promoted. |

## Python And Docs P1s

Python P1s are not P0 after the secret argv fix, but they remain real hardening
items:

- `python/postfiat_rpc/client.py` uses plaintext TCP RPC. This is acceptable
  for local/private controlled testnet only.
- `python/postfiat_rpc/wallet.py` creates default temp work directories and
  writes JSON outputs without explicit private permissions. Some outputs are
  signed transactions or reports, but wallet-adjacent files should default to
  private mode.
- `python/postfiat_rpc/__main__.py` uses check logic that can miss escaped
  sensitive values in JSON-shaped output.

Docs/docs-site P1s mostly require wording and deployment guardrails:

- The docs server now defaults to localhost after the P0 pass, but Basic Auth
  over plain HTTP is still only safe behind SSH forwarding or TLS termination.
- Redaction docs and evidence guidance should describe structural JSON redaction
  rather than grep-only redaction.
- Topology and funding-independence reports should be framed as public-launch
  evidence gates, not controlled-testnet code blockers.

## Script P1 Themes

The `scripts/` P1 inventory is too large to close as one patch. Treat it as a
separate operator-tooling hardening milestone.

Promote these script classes first:

- scripts that handle private keys, master seeds, SSH credentials, sudo
  passwords, or credential files,
- scripts that run remote commands or generate remote shell/systemd files,
- scripts that package release artifacts or evidence for publication,
- scripts that make public launch/readiness claims,
- scripts with destructive filesystem operations such as `rm -rf`,
- scripts that perform redaction or secret scans.

Do not spend early time on one-off devnet smoke scripts unless they are copied
into live operator runbooks.

## Work-Item Status

| ID | Status | Review |
| --- | --- | --- |
| `SEC-P1-001` / `WI-09` | Open | Dynamic execution and automation authority. Real script hardening backlog. |
| `SEC-P1-002` / `WI-10` | Open | Mixed remaining P1s need ledger classification. |
| `SEC-P1-003` / `WI-11` | Open | Secret/key lifecycle beyond P0 remains, especially plaintext key-file formats and SDK temp files. |
| `SEC-P1-004` / `WI-12` | Open | Generated report/artifact redaction is still the largest raw P1 bucket. |
| `SEC-P1-005` / `WI-13` | Open | SSH/shell/remote deployment hardening remains broad script work. |
| `SEC-P1-006` / `WI-14` | Open | Consensus/privacy/verification correctness needs targeted fixes and golden tests. |
| `SEC-P1-007` / `WI-15` | Partially reduced | P0 debug proof construction was gated, but docs and residual debug/privacy claims still need cleanup. |
| `SEC-P1-008` / `WI-16` | Open | Placeholder/deterministic artifacts remain mostly fixtures, reports, docs, and smoke scripts. |
| `SEC-P1-009` / `WI-17` | Partially reduced | Defaults were made safer in P0 work; TLS/auth/public exposure guidance remains. |

## Recommended Execution Order

1. Fix the active-code safety issues: bridge replay pruning, secure atomic
   writes, history archive single-read import, IPv6 parsing, remaining manual
   JSON output.
2. Fence plaintext secret-file formats with strict permissions and explicit
   private-material type boundaries; plan encrypted formats separately.
3. Define canonical hash encodings and golden vectors for consensus/proof/privacy
   objects before cross-implementation claims.
4. Add privacy accounting fixes for debug notes and Orchard scan outcomes.
5. Build a shared safe report-output helper for Cobalt examples and scripts.
6. Run a script hardening milestone for remote execution, SSH, sudo, redaction,
   and destructive path handling.
7. Create a generated-artifact redaction/quarantine ledger for `reports/`,
   `work_archive/`, and stale public-claims evidence.

## Verification Performed For This Review

- Recounted P1 path distribution from
  `milestones/ambient-code-review-20260522T010605Z-023bbb6ba6/report.json`.
- Enumerated the 30 crate-level P1 findings with generated `P1-N` IDs.
- Spot-checked current source around the active-code findings after the P0
  remediation changes.
- No P1 remediation patches or tests were run as part of this review.
