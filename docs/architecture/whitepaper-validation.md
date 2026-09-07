# Whitepaper audit validation

Audit source: `d351353e57b295368450a57866ace17b5e1ce6ad`. All Rust evidence below
uses unchanged runtime code from that baseline in the isolated audit worktree.
Changes are limited to the Python CLI, documentation and the Python CI checkout's
history requirement.

## Specification gate

The research specification at `docs/specs/whitepaper-alignment-audit-20260907.md`
passed its first full selected-model text gate: GPT 92.00, Fable 81.60,
GLM 87.40, five scores each, **87.00/100 overall**. It was locked immediately
without rewrite/rescore. SHA-256:
`4b683479a1d82fd1ef387797a5cf5bd751b53f75063b8985ec15441441f914c0`.
The separate record is `docs/review/whitepaper-audit-lock-20260907.md`.
This is a text-quality gate, not protocol correctness evidence.

## Focused runtime checks

The shared target cache was selected with
`CARGO_TARGET_DIR=/home/postfiatchad/repos/postfiatl1v2/target`; source files came
from the isolated audit worktree. Commands below use the pinned lockfile.

| Command | Observed result | Scope |
| --- | --- | --- |
| `cargo test -p postfiat-consensus-cobalt --lib --locked validator_admission_policy` | **5 passed**, 0 failed/ignored | Clean candidate, shared-control rejection, missing domain, conflicting evidence and unknown model citation. |
| `cargo test -p postfiat-consensus-cobalt --lib --locked cobalt_cover_extractor` | **7 passed**, 0 failed/ignored | Derived cover, deduplication, inactive/oversized/budget rejection, omission detection and unsafe intersection. |
| `cargo test -p postfiat-ordering-fast --lib --locked consensus_v2` | **8 passed**, 0 failed/ignored | Exact roots/domains, typed timeout ancestry, durable lock/precommit rule, n=4/n=6 quorum and adversarial/restart models. |
| `cargo test -p postfiat-node --lib --locked cobalt_handoff::tests` | **10 passed**, 0 failed/ignored | Scope exclusivity, distinct signatures, replay/rollback, protocol-decision tamper rejection, exact live batch consumer and certificate sizing regression. |

Total: **30 focused Rust tests passed**. Other retained tests referenced in the
inventory were inspected as evidence anchors but not all rerun. No workspace,
long Orchard, live validator, external bridge, hardware prover or new performance
campaign was run: the patch changes neither those runtime boundaries nor their
cryptography. These results are bounded regression evidence, not universal
safety/liveness proofs.

## Documentation and CLI checks

- `PYTHONPATH=python python3 -m unittest discover -s python/tests -p test_whitepaper_audit.py -v`: **12 tests passed** for schema/pin/coverage/anchor failures, invalid paths, section-filter JSON, and stale table/download detection.
- Prerequisite `scripts/docs-site-build` with the existing docs virtual environment: **passed strict build**. Existing excluded-document INFO notices were retained.
- `PYTHONPATH=python python3 -m postfiat_rpc.whitepaper_audit --check`: **passed**, 73 claims, 49 original sections/headings, E1–E8, 192 evidence references across 71 files, exact generated table and whitepaper download.
- `scripts/test-whitepaper-implementation-boundaries`: **passed**; no invented ML-DSA shielded outer-envelope claim.
- `scripts/test-public-doc-links`: **passed** its regression.
- `scripts/public-doc-links`: **passed**, 357 Markdown files. The docs virtual environment supplied its existing Markdown dependency.
- Final `scripts/docs-site-build`: **passed**, including redaction check and strict MkDocs build. The first full-patch build caught a new relative link from the operational page to an out-of-site benchmark; replacing it with the pinned GitHub artifact link resolved the warning. Existing excluded-document INFO notices remain.
- `cargo fmt --all -- --check`: **passed**; existing stable-toolchain warnings about nightly-only configuration were informational. No Rust source was changed.
- `git diff --check`: **passed**.
- Built-HTML inspection found all four audit pages and exactly 73 claim rows; the table's links resolve to the declared source commit and anchors.
- Headless Chromium inspection confirmed readable overview/navigation at 1440×1100 and 390×844, and a readable alignment table at 1440×1400. Local captures were retained outside Git. The installed full Chromium binary lacked shared libraries; the installed headless shell worked with the existing temporary browser-library directory. This was a local documentation preview, not a deployed-site check.

At the original audit delivery (`f2db323b`), the corrected canonical whitepaper
and downloadable copy shared SHA-256
`a905c4de38c9dc9540c9ca49d0281a5a64ff547cb7368d84ebf7be006a92bd4a`.
The original paper identity remains in the inventory; it is not overwritten by
the corrected-text identity.

## Hosted CI follow-up

The first PR run passed `docs-build` but exposed the Python job's shallow Git
checkout: eight inventory tests could not read the pinned baseline. The workflow
now fetches full history for `python-sdk`, matching the CLI's documented history
requirement. The other 446 Python tests passed and three were skipped in that
initial run. The local 12-test audit suite remains green with the baseline present.
The repeat hosted `python-sdk` job at `69015d31` passed: **454 passed, 3 skipped**
in the Python suite, followed by 6 remote-sync and 4 remote-finality operations
regressions. `docs-build` passed again.

The same run's `public-tree-hygiene` job failed the existing proof inventory:
`scripts/check-nav-reserve-proof-fuzz-smoke` has source digest
`e0c814a53c3b81f7b72c6ca3c7a71d739ed0efae62505fd9effba15326acb4b7`, while the
inventory expects `d9bae0c68f8448eba9c99092546a7780026b277901980bec56730b5d66418d79`.
Those source/inventory files are unchanged by this audit. This PR does not claim
all hosted checks passed; that baseline gate needs its separate reviewed repair.

## Evidence interpretation

The inventory covers 73 material claims and every original numbered
section/subsection, Abstract, Appendix A entry E1–E8 and References. Each row has
a unique original-text locator and file/anchor references resolved at the full
baseline commit. Its structural checker cannot decide whether a cited test proves
a claim; that remains review judgment.

Original E1/E2/E3/E5/E6/E7 measurement packets were not fully available in the
pinned public tree. Retained summaries and hashes were identified, without
claiming that missing raw results were reproduced. E5/E6/E7 profile summaries
must not be conflated across different experiments.

The August 31 storage deployment receipt was read as retained evidence, not
reproduced or independently attested. No signing, fleet mutation, runtime
configuration change or value movement occurred. The original main checkout's
unrelated work was excluded from this branch.

Task Node reported both the audit and specification/milestone tasks **Rewarded**
on September 7, 2026 after their initial evidence and verification responses.
The [completed milestone](../plans/completed/whitepaper-audit-milestone.md) records
retirement. The draft PR remains a review artifact; reward does not imply a merge,
release, protocol proof or closure of the gap backlog.
