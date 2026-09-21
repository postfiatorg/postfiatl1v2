# QA campaign log — 2026-09-21

Progress record for the [burn 6 campaign](qa-campaign-20260921-burn6-brief.md).
A1 began at `6ae06356` on clean `burn6-work` in `/tmp/burn6-20260921`,
after `git pull --rebase origin release/combined-devnet-20260915`, within a
25-minute time box beginning at approximately 10:12 UTC.

**Status:** A1 closed within its recorded limits. One P3 is recorded without
repair; no P1/P2 finding and no repair unit. All 13 focused tests passed, with
one existing opt-in test ignored. A2–A5 and B remain pending and were not
started. No Task Node or fleet action. Work stops after this A1 closeout.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Portfolio verification | done | 0 | 0 | 1 | [Review](portfolio-verification-review-20260921.md); findings `21459c8a`; PFV-01 recorded without repair; repair unit skipped (no P1/P2); no consensus-affecting change |
| A2 | NAV and reserve verification | pending | — | — | — | Not started |
| A3 | Swap and settlement execution in depth | pending | — | — | — | Not started |
| A4 | Bridge workflows | pending | — | — | — | Not started |
| A5 | Optional remaining node files | pending | — | — | — | Not started |
| B | Defect inventory and TIH gate | pending | — | — | — | Not started |

Current finding totals: **0 P1, 0 P2, 1 P3** (A1 only).

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

Pending; not started.

## Swap and settlement execution review result

Pending; not started.

## Bridge workflows review result

Pending; not started.

## Remaining node files review result

Pending; not started.

## Skips and boundary decisions

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

## Verification

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
