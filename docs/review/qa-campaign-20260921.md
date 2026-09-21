# QA campaign log — 2026-09-21

Progress record for the [burn 6 campaign](qa-campaign-20260921-burn6-brief.md).
A1 began at `6ae06356` on clean `burn6-work` in `/tmp/burn6-20260921`,
after `git pull --rebase origin release/combined-devnet-20260915`, within a
25-minute time box beginning at approximately 10:12 UTC.

**Status:** A1 findings complete; focused verification and closeout in progress.
A2–A5 and B are pending and not started.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Portfolio verification | findings complete; verification in progress | 0 | 0 | 1 | [Review](portfolio-verification-review-20260921.md); PFV-01 recorded without repair; no P1/P2 repair unit |
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
- No Task Node or fleet action. No deployment, activation, spends or installs.
- The excluded release checkout, frozen artifacts and excluded source crates
  remain outside this review. Network use is limited to git.

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
- `cargo test -p postfiat-node yolo_ --lib --locked`: in progress. Optional
  `YOLO_QUALIFICATION_REPORT`, `YOLO_QUALIFICATION_KEEP_DIRECTORY` and
  `YOLO_QUALIFICATION_PROOF_FIXTURE` variables are unset for this command.
- Findings, repairs and closeout will each run the three required gates before
  any applicable separate commit, then pull with rebase and push only with
  `git push origin HEAD:release/combined-devnet-20260915`.

## Scores

Not run; inventory scoring belongs to B, which is not started.

## Final summary

A1 recorded one P3 and no P1/P2; verification and closeout remain in progress.
No repair commit is required. A2–A5 and B have not been started.
