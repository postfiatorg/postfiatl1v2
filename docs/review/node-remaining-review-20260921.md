# Remaining node files correctness review — 2026-09-21

A5 of the [burn 6 campaign](qa-campaign-20260921-burn6-brief.md), starting
at `0e2ae50c` after the required release-branch pull on clean `burn6-work`
in `/tmp/burn6-20260921`. The 25-minute window began at approximately
11:39 UTC. This is a focused review of the three named files, not a
whole-file or end-to-end audit. References describe the pre-repair revision.

## Findings

### NOD-01. P2 — duplicate batch acknowledgments report rejected receipts as accepted

**Source:** `crates/node/src/transport_protocol.rs:1490-1572`, particularly
`:1563-1565`; acknowledgment validation at `:1018-1065`.

**Condition:** a certified batch containing a rejected transaction is delivered
again. **Observed:** the duplicate path uses the number of block receipt IDs
as `accepted_count`, sets `rejected_count` to zero, and never reads the stored
receipt outcomes. The initial acknowledgment counts actual outcomes. A retry
therefore changes the reported result without changing the committed batch.
The receiver also accepts inconsistent outcome-count sums.
**Expected:** count the referenced persisted outcomes, fail closed when they
are missing or ambiguous, and validate the checked count sum on receipt.

**Suggested minimal change:** derive duplicate counts from stored receipts;
regress rejected/mixed outcomes, unavailable or conflicting receipt evidence,
and malformed acknowledgment totals. **Consensus-affecting: no**; transport
reporting only, with no change to committed results, encodings or signatures.

### NOD-02. P2 — wallet outputs can overwrite their recovery backup

**Source:** `crates/node/src/block_replay_wallet.rs:3072-3121`.

**Condition:** key generation names the same fresh destination for its key
and backup, or restore uses its backup as the output with overwrite enabled.
**Observed:** independent existence checks pass; the key write replaces the
backup and the operation returns success with an unusable backup path.
**Expected:** reject aliased key/backup destinations before writing either.

**Suggested minimal change:** compare resolved destinations in both entry
points before mutation; regress identical and normalized paths, a symlinked
parent alias, successful distinct outputs and restore rejection preserving
the original backup. **Consensus-affecting: no**; local wallet-file interlock
only. Existing key and backup formats remain unchanged.

### NOD-03. P3 — local transport file reads are not bounded during collection

**Source:** `crates/node/src/transport_protocol.rs:15-29` and `:384-407`.

**Condition:** a topology/registry/key JSON file is oversized, or a payload
grows after its metadata-size check. **Observed:** `read_to_string` collects
the entire file; the generic readers have no byte cap and the payload limit
is checked only before opening. **Expected:** enforce appropriate limits
while reading the opened handle. **Suggested minimal change:** bounded reads
with an extra-byte overflow check. This is local file/resource exposure;
network frame reads have their own caps. **Repaired 2026-09-23 in `0b9c6715`:**
reads are capped on the opened handle.
Repairs land after the qualified tip and ship in the release after next.

### NOD-04. P3 — manifest reporting reopens the file after verification

**Source:** `crates/node/src/rpc_dispatch.rs:2707-2753`.

**Condition:** the local governance bundle is replaced between verification
and the following file read. **Observed:** the response combines the earlier
verified hash/count with `operator_manifests` parsed from later bytes.
**Expected:** reported manifests and verification metadata describe the same
snapshot. **Suggested minimal change:** obtain both from one verified input
snapshot or reject a changed file before returning. No signature bypass or
governance mutation is established. **Repaired 2026-09-23 in `0b9c6715`:** one
verified read supplies both.
Repairs land after the qualified tip and ship in the release after next.

## Areas with no additional findings

- Transport domain, route, payload and authentication binding; signed health
  nonce binding; proposal-parent interlocks; supported batch kinds; vote phase
  shape; bounded line reads; private bind policy and synchronous send-result
  accounting were examined. Delegated finality/crypto verification was not
  reviewed. An acknowledgment alone does not establish transaction success.
- Block/certificate ordering, archived payload linkage, persisted receipt
  multiplicity, registry-update prefix/lineage handling and the main replay
  loop's receipt/root comparisons were examined at their call sites. No
  additional confirmed defect; historical compatibility is not independently
  qualified by this review.
- Wallet signing binds caller-provided domain, operation, fee and sequence,
  checks expected source where exposed and verifies its generated signature.
  RPC aliases, request-file flag conversion and action-spool helpers were
  inspected; no additional confirmed defect beyond the file-snapshot issue.

## Review limits and skips

Source ranges reviewed: transport production `1-1792` and duplicate-ack test
fixtures `2523-2642`; block replay/wallet `1-610`, `793-1614`, `1765-2320`,
`2664-2815` and `3068-3585`; RPC dispatch `1-400` and `2500-3102`.
The remaining RPC method arms, replay compatibility tables/root helpers,
FastPay replay/accounting remainder and other in-file tests are unreviewed.
These explicit limits reserve time for repairs, regressions and pushed
closeout within the time box. No full-file coverage is claimed.

Only the three named files received a correctness review. The wallet output
writers/existence helper in `storage_commit.rs`, receipt/option type shapes,
and test registration/names supplied dependency context, not another review.
Called execution, storage, signing, finality and proof implementations remain
unreviewed. No excluded crate, `programs/`, other campaign surface, frozen
artifact or protected release checkout is reviewed or changed.

No Task Node, fleet action, live RPC, spend, installation or deployment.
Network use is git only. Socket tests, full Rust/Orchard suites, archived-chain
replay and CI queries are omitted. Repairs do not alter replay, shielded
execution, proof verification, native accounting or state commitments.
P3 findings remained unfixed at this review (repaired 2026-09-23).
B already ran; A5 inventory rows will be appended
without rescoring, and B's retained score predates this addition.

## Repair result

Findings were separately gated and pushed as `d3608838` before repairs.
Only `transport_protocol.rs` and `block_replay_wallet.rs` change as source.

- **NOD-01:** duplicate acknowledgments count the selected receipt IDs using
  persisted outcomes. Missing or conflicting outcomes return an error;
  repeated identical evidence is not counted as another transaction. Receiver
  validation requires the checked accepted/rejected sum to equal receipt count.
  Three regressions cover mixed/all-rejected batches at current and later
  heights, unrelated and duplicate evidence, missing/conflicting outcomes,
  invalid totals and overflow. **Consensus-affecting: no.**
- **NOD-02:** key generation and restore compare resolved output destinations
  before writing. Existing ancestors resolve directory symlinks; distinct new
  directories remain supported. Two regressions cover identical, normalized
  and symlink-parent aliases, preservation of the restore backup, and valid
  distinct outputs. **Consensus-affecting: no.** This is a preflight interlock,
  not a guarantee against concurrent filesystem replacement or two-file crashes.
- **NOD-03/04:** repaired 2026-09-23 in `0b9c6715`, after the qualified tip; they
  ship in the release after next. **Consensus-affecting: no.**

All five new regressions failed against the original implementation:
`cargo test -p postfiat-node --lib --bin postfiat-node --locked --no-fail-fast burn6_nod_`
reported **0 passed, 2 failed** in the library (386 filtered) and **0 passed,
3 failed** in the binary (173 filtered). These were the intended assertions,
not fixture or compilation errors. Earlier linking/quota failures ran no tests.

Final commands used
`env PATH="/usr/bin:$PATH" CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/integrate-20260918-target CARGO_BUILD_JOBS=1`:

| Command | Result |
| --- | --- |
| `cargo test -p postfiat-node block_replay_wallet:: --lib --locked` | 3 passed; 385 filtered |
| `cargo test -p postfiat-node transport_protocol::transport_cli_tests --bin postfiat-node --locked -- --skip transport_listener_mode_failure_prevents_ready_report` | 30 passed; 146 filtered |
| `cargo test -p postfiat-node wallet_keygen_restore_round_trips_without_report_secret_leakage --lib --locked` | 1 passed; 387 filtered |
| `cargo test -p postfiat-node wallet_sign_transfer_emits_submit_ready_redacted_transfer --lib --locked` | 1 passed; 387 filtered |
| `cargo test -p postfiat-node wallet_test_vector_is_deterministic_and_redacted --lib --locked` | 1 passed; 387 filtered |
| `cargo check --workspace --locked` | Passed |

**36 unique focused tests passed, 0 failed, 0 ignored.** The named socket
test was explicitly skipped, not passed. The other transport fixtures use
local files, in-memory simulation and injected closures; no fleet or network
test ran. `cargo fmt --all -- --check` and `git diff --check` passed.
The system compiler and disk-backed temporary build cache avoided the local
compiler-wrapper aborts and tmpfs user quota; no toolchain was installed.
The original target path now links to `/var/tmp/burn6-a5-target`.
**Full Rust suite verdict pending.** No A5 repair requires release-tip
re-qualification; the earlier NAV-03/SWX-01 requirement remains in force.
