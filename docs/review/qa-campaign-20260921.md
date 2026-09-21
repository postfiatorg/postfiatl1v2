# QA campaign log — 2026-09-21

Progress record for the [burn 6 campaign](qa-campaign-20260921-burn6-brief.md).
A1 began at `6ae06356` on clean `burn6-work` in `/tmp/burn6-20260921`,
after `git pull --rebase origin release/combined-devnet-20260915`, within a
25-minute time box beginning at approximately 10:12 UTC.

**Status:** A1 and A2 closed within their recorded limits. A2 found and repaired
three P2 findings; its 49 focused tests passed. A1's one P3 remains recorded.
NAV-03 is consensus-affecting under the brief's state-transition-result rule:
the release tip requires re-qualification before deployment. A3–A5 and B remain
pending and were not started. No Task Node or fleet action. Stop after A2.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Portfolio verification | done | 0 | 0 | 1 | [Review](portfolio-verification-review-20260921.md); findings `21459c8a`; PFV-01 recorded without repair; repair unit skipped (no P1/P2); no consensus-affecting change |
| A2 | NAV and reserve verification | done | 0 | 3 | 0 | [Review](nav-reserve-verification-review-20260921.md); findings `4bc6d98d`; all P2 repaired in `e9ccdeda`; NAV-03 consensus-affecting: release-tip re-qualification required before deployment |
| A3 | Swap and settlement execution in depth | pending | — | — | — | Not started |
| A4 | Bridge workflows | pending | — | — | — | Not started |
| A5 | Optional remaining node files | pending | — | — | — | Not started |
| B | Defect inventory and TIH gate | pending | — | — | — | Not started |

Current finding totals: **0 P1, 3 P2, 1 P3** (A1–A2; all three P2 repaired,
one P3 recorded without repair).

## Portfolio verification review result

The [A1 review](portfolio-verification-review-20260921.md) read all 1,356 lines
of the six permitted source/test files. PFV-01 is a P3 coverage gap: the
collection binding test mutates only the attestation digest, leaving eight
other digest mismatches and the count mismatch unexercised. No P1/P2 defect
was established. The repair unit is skipped; no source or test is changed.
No consensus-affecting repair occurred. Holdings, weight sums and target
arithmetic remain delegated to unreviewed guest/proof tooling outside A1.

Reviewed in full: `crates/execution/src/yolo_target_verifier.rs` (153 lines),
`crates/execution/src/yolo_collection_verifier.rs` (205),
`crates/execution/src/yolo_target_execution_tests.rs` (340),
`crates/node/src/yolo_target_queries.rs` (44),
`crates/types/src/yolo_collection_public_values.rs` (225), and
`crates/node/src/tests/yolo_target_receipt_tests.rs` (389), at `6ae06356`.
No other source implementation was reviewed for correctness.

Findings and the initial log were committed and pushed as `21459c8a` before
this closeout. The execution tests passed real-proof mutation, activation,
duplicate and replay checks; the codec tests passed; the node tests passed
query/state-commitment checks and both synthetic/AWS-compatible local
four-store certificate, reopening and replay cases. Total: **13 passed,
0 failed, 1 ignored**. These results do not establish private witness
arithmetic, live-source freshness or deployment qualification.

## NAV and reserve verification review result

The [A2 review](nav-reserve-verification-review-20260921.md) read all 4,022 lines
of the six permitted files at `b82052ce`, after the required pull on clean
`burn6-work`. Work began at approximately 10:25 UTC with a 25-minute limit.
Findings `4bc6d98d` and repairs `e9ccdeda` were separately gated and pushed.

| Finding | Result | Consensus impact |
| --- | --- | --- |
| NAV-01 — P2, duplicate vault rows inflate overlay | Repeated bucket/receipt/allocation IDs rejected before summing; regression coverage added | No; local unsigned builder inputs only |
| NAV-02 — P2, packet overrides reserve-submit operation | Optional tag validated; fixed tag assigned last; both input paths covered | No; local unsigned builder inputs only |
| NAV-03 — P2, invalid receipt history follows persisted reserve mutation | Existing and prospective history validated before publication, including row and 8 MiB byte limits; rejection preserves both files | Yes under the brief's state-transition-result rule; re-qualify release tip before deployment |

Reviewed in full: `crates/nav_reserve_protocol/src/lib.rs` (153 lines),
`crates/types/src/nav_reserve_public_values.rs` (393),
`crates/node/src/market_bridge.rs` (2,628),
`crates/node/src/tests/nav_reserve_proof_status_tests.rs` (143),
`scripts/a666-build-live-nav-mark-ops.py` (476), and
`scripts/a666-build-route-epoch-advance.py` (229).
The adjacent two Python builder tests were read for coverage; selected local
config/subscription fixtures in `pftl_uniswap_bridge_rpc_tests.rs` supplied
test construction context, without a correctness review of that whole file.

Only `market_bridge.rs`, the live-NAV builder and its Python test changed as
source/test files. NAV-03 preserves the existing JSON encoding and successful
transition semantics; its conservative campaign classification covers changed
rejected-transition persistence. Cross-file crash/I/O atomicity and concurrent
writers remain outside the repair guarantee. No release qualification or
deployment occurred. All three P2 findings are repaired; no P1/P3 was found in
A2. **49 focused tests passed, none failed or ignored.** Full-suite verdict
remains CI's responsibility on the pushed branch; no CI success is claimed.

A2 closes here; A3–A5 and B are pending. The retained A1 narrative and the
existing A1-only Final summary below are historical; this section and the
top Status give the current campaign disposition.

## Swap and settlement execution review result

Pending; not started.

## Bridge workflows review result

Pending; not started.

## Remaining node files review result

Pending; not started.

## Skips and boundary decisions

### A1 record retained

- A2–A5 and B are outside this task; no inventory update or scoring.
- PFV-01 remains unfixed as required for P3. There is no P1/P2 repair unit.
- Shared verifier/type/storage/finality implementations and excluded guest
  calculations were not reviewed. The findings document names these limits.
- Specifically unreviewed: `nav_sp1_verifier.rs`, target public-value and
  receipt type implementations, outer execution dispatch, storage,
  state-commitment and `tx_finality` implementations, and the target/collection/
  Nitro guest-proof tooling named in the review. No holdings/weight/amount
  witness was constructed or proved; overflow/rounding in the excluded target
  calculation remains unqualified by A1.
- No Task Node or fleet action. No deployment, activation, spends or installs.
- The excluded release checkout, frozen artifacts and excluded source crates
  remain outside this review. Network use is limited to git.
- The existing opt-in supplied-public-proof test remains ignored because its
  independent external fixture is not part of this task. The two committed
  public fixture tests ran normally in temporary local stores, with no sockets
  or live validator/RPC access and no retained qualification-report output.
- `cargo check --workspace --locked` and repair regression additions were
  skipped with the repair unit: there is no P1/P2 finding and no source change.
  No local full workspace/Orchard suite, archived-chain replay or CI query ran;
  the full Rust suite verdict remains pending CI on the pushed branch.
- No time or usage limit curtailed the six-file review or focused verification.

### A2 decisions

- A3–A5 and B were not started. No inventory edit or TIH scoring; NAV-03's
  classification is recorded in the A2 row pending the separate inventory unit.
- Not reviewed: `nav_sp1_verifier.rs`, reserve submit/finalize and overlay
  recomputation execution, route-epoch execution, shared type/storage/finality
  implementations, the optional `a666-pfusdc-reserve-demo.py` identity helper,
  or `tools/nav-reserve-proof` packet/guest/source-policy implementations.
  No excluded crate or `programs/` correctness review or repair occurred.
- Consequently source freshness/completeness, proof/overlay disjointness,
  independently trustworthy prices and consensus rejection of wrong-context
  proofs remain delegated. No malformed witness was proved. Local status
  fixtures and operation manifests are not proof or finality evidence.
- No full Rust/Orchard suite, exact archived-chain replay, crash-injection test
  or CI query. This local history/builder repair does not cross an Orchard
  accounting or proof boundary. Re-qualification remains required for NAV-03
  before deployment; the current work makes no qualification claim.
- No Task Node, fleet, live RPC, spend, signup, install or deployment action.
  Frozen artifacts and the explicitly excluded release checkout were untouched;
  network use was git only. Tests used local temporary fixtures.
- Tool-path correction: the first documentation write resolved in the ordinary
  development checkout rather than the campaign worktree. The new draft was
  immediately moved into the campaign worktree, and the ordinary checkout was
  verified clean before the findings commit. All commits and pushes used the
  campaign worktree and the required release ref; nothing was pushed to main.
- No time/usage limit curtailed the six-file review or its three repairs. No
  requested repair was skipped. Stop after the separately gated log closeout.

## Verification

### A1 record retained

- Initial `git pull --rebase origin release/combined-devnet-20260915`: passed;
  already up to date at `6ae06356`, with a clean tree.
- Cargo uses `CARGO_NET_OFFLINE=true` and
  `CARGO_TARGET_DIR=/tmp/integrate-20260918-target`; dependencies compile
  normally without expanding the source review or running their test suites.
- `cargo test -p postfiat-execution yolo_ --lib --locked`: **7 passed, 0 failed,
  0 ignored, 198 filtered out**, including the real Groth16 mutation tests.
- `cargo test -p postfiat-types yolo_collection_public_values_tests --lib --locked`:
  **2 passed, 0 failed, 0 ignored, 147 filtered out**.
- `cargo test -p postfiat-node yolo_ --lib --locked`: **4 passed, 0 failed,
  1 ignored, 375 filtered out**. The full invocation was
  `env -u YOLO_QUALIFICATION_REPORT -u YOLO_QUALIFICATION_KEEP_DIRECTORY -u YOLO_QUALIFICATION_PROOF_FIXTURE CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/integrate-20260918-target cargo test -p postfiat-node yolo_ --lib --locked`.
- Unique focused total: **13 passed, 0 failed, 1 ignored**. No pre-repair
  failing run or new regression is claimed; source and tests are unchanged.
- Before findings commit `21459c8a`: `.venv-docs/bin/mkdocs build --strict`,
  `scripts/public-doc-links` (**466 files**) and `scripts/public-secret-scan`
  (**tracked-tree**) all passed. Both new documents were staged before the
  secret scan; `git diff --cached --check` passed.
- Before this closeout commit: the same three gates passed again, including
  the **466-file** link check and tracked-tree secret scan with the updated
  log staged. `git diff --cached --check` passed. The link command used
  `PATH="$PWD/.venv-docs/bin:$PATH" scripts/public-doc-links` in both units.
- Findings commit `21459c8a` was followed by
  `git pull --rebase origin release/combined-devnet-20260915` and
  `git push origin HEAD:release/combined-devnet-20260915`; both succeeded,
  and the tree was clean before closeout. This documentation-only closeout
  follows the same pull/push sequence and reruns no Rust tests. No push to
  `main` occurs. No full Rust suite or CI success is claimed.

### A2 verification

Every Cargo test/check below used the exact prefix
`env CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/integrate-20260918-target`.
Dependencies compiled normally; this does not expand the source review or run
their excluded test suites. Python tests use stub proof tooling and dummy
key-file contents, with no signing or network access.

| Command | Final result |
| --- | --- |
| `cargo test -p postfiat-nav-reserve-protocol --lib --locked` | 1 passed, 0 failed, 0 ignored, 0 filtered |
| `cargo test -p postfiat-types nav_reserve_public_values_tests --lib --locked` | 5 passed, 0 failed, 0 ignored, 144 filtered |
| `cargo test -p postfiat-node nav_reserve_proof_status --lib --locked` | 1 passed, 0 failed, 0 ignored, 379 filtered (before adding regressions) |
| `cargo test -p postfiat-node navcoin_bridge --lib --locked` | 14 passed, 0 failed, 0 ignored, 369 filtered; includes three new regressions |
| `python3 scripts/test-a666-build-live-nav-mark-ops.py` | 16 passed; includes seven added test methods |
| `python3 scripts/test-a666-build-route-epoch-advance.py` | 12 passed |
| `cargo check --workspace --locked` | Passed |
| `cargo fmt --all -- --check` (no environment prefix) | Passed after formatting the three changed Rust hunks |
| `scripts/test-proof-public-input-inventory` | Passed: 7 systems, 150 public fields, 94 source hashes |

Unique focused total: **49 passed, 0 failed, 0 ignored**. The unchanged
protocol/codec/status/route tests ran during review; touched-module tests ran
after repairs. Baseline live-NAV tests passed 9/9 before adding regressions.
Against the original builder, the expanded 15-method suite had 11 passes and
four failing methods (seven failing assertions including subtests), proving
NAV-01/NAV-02. The final suite adds a supplied-packet-path case as well.

`cargo test -p postfiat-node market_bridge::burn6_tests --lib --locked`, with
the same environment prefix, initially failed two tests; the capacity fixture
hit the existing 8 MiB reader cap before reaching its intended row bound. After
correcting the fixture, the original implementation produced **0 passed,
2 failed, 0 ignored, 380 filtered** for the intended defects: persisted ledger
on rejection and accepted oversized history. Final node tests prove malformed
and duplicate-history rejection, a readable last append, rejection before the
next byte-limit overflow, the independent row cap and unchanged JSON encoding.
The capacity fixture uses distinct standalone receipts to isolate admission;
it is not chained replay or release qualification evidence.

Before **each** A2 commit (findings `4bc6d98d`, repairs `e9ccdeda` and this
closeout), all three gates passed on the staged changes:
`.venv-docs/bin/mkdocs build --strict`,
`PATH="$PWD/.venv-docs/bin:$PATH" scripts/public-doc-links` (**467 files**),
and `scripts/public-secret-scan` (**tracked-tree**).
`git diff --cached --check` also passed. The first draft-location correction
was followed by a fresh set of findings gates; only those runs count here.

The initial `git pull --rebase origin release/combined-devnet-20260915` was
up to date at `b82052ce`. Findings and repair commits each completed
`git pull --rebase origin release/combined-devnet-20260915` followed by
`git push origin HEAD:release/combined-devnet-20260915`, with a clean tree
between units. This log-only closeout uses the identical pull/push sequence.
No full-suite or CI pass is claimed; no other surface follows A2.

## Scores

Not run; inventory scoring belongs to B, which is not started.

## Final summary

| Surface | P1 | P2 | P3 | Findings commit | Repair commit |
| --- | ---: | ---: | ---: | --- | --- |
| A1 — Portfolio verification | 0 | 0 | 1 | `21459c8a` | None; no P1/P2 |

PFV-01 remains recorded without repair. **Consensus-affecting repairs: none.**
No release-tip re-qualification is required by an A1 repair because no repair
was made; no deployment or qualification claim is introduced. Any future
consensus-affecting repair on this release branch requires re-qualification
of the release tip before deployment, as the brief requires.

Remaining limits: excluded guest holdings/weight/amount arithmetic and
freshness policy; delegated cryptographic/type/storage/finality implementations;
the ignored supplied-proof case; and the pending full-suite CI verdict.
Findings and closeout are separately gated and pushed; only the two A1 review
documents change. A2–A5 and B remain pending. Stop after the A1 closeout.
