# QA campaign log — 2026-09-21

Progress record for the [burn 6 campaign](qa-campaign-20260921-burn6-brief.md).
A1 began at `6ae06356` on clean `burn6-work` in `/tmp/burn6-20260921`,
after `git pull --rebase origin release/combined-devnet-20260915`, within a
25-minute time box beginning at approximately 10:12 UTC.

**Status:** closed after A5's focused review, with the unreviewed ranges
explicitly retained below. Burn 6 now records 15 findings: nine repaired P2s
and six P3s, left unrepaired at closeout. A5 findings `d3608838` and repairs `87c992c3` are pushed;
no A5 repair is consensus-affecting. B's **87.33/100** gate predates A5's four
added inventory rows; the current 141-row inventory was not rescored.
All four repair commits retain a full Rust suite verdict pending.
NAV-03 and SWX-01 require release-tip re-qualification before deployment.
The earlier A1–A4/B narratives, Scores and Final summary are retained as their
historical closeout record; this Status, current totals and A5 section supersede
their statements that A5 was skipped. No Task Node or fleet action. Stop here.

**2026-09-23 update:** all six P3s are repaired in `0b9c6715`, after the
qualified tip; see [September 23 P3 repairs](#september-23-p3-repairs). That
section supersedes the historical P3 statements below. The deployed candidate
is unchanged.

## Campaign state

| Order | Surface | Status | P1 | P2 | P3 | Evidence or fixes |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A1 | Portfolio verification | done | 0 | 0 | 1 | [Review](portfolio-verification-review-20260921.md); findings `21459c8a`; PFV-01 repaired 2026-09-23, `0b9c6715`; repair unit skipped (no P1/P2); no consensus-affecting change |
| A2 | NAV and reserve verification | done | 0 | 3 | 0 | [Review](nav-reserve-verification-review-20260921.md); findings `4bc6d98d`; all P2 repaired in `e9ccdeda`; NAV-03 consensus-affecting: release-tip re-qualification required before deployment |
| A3 | Swap and settlement execution in depth | done | 0 | 1 | 0 | [Review](swap-settlement-execution-review-20260921.md); findings `6d495f43`; SWX-01 repaired in `043b9d69`; consensus-affecting: release-tip re-qualification required before deployment |
| A4 | Bridge workflows | done | 0 | 3 | 3 | [Review](bridge-workflows-review-20260921.md); findings `66a68624`; BRW-01/02/03 repaired in `10bb5655`; BRW-04/05/06 repaired 2026-09-23, `0b9c6715`; no consensus-affecting change |
| A5 | Optional remaining node files | closed — focused review | 0 | 2 | 2 | [Review](node-remaining-review-20260921.md); findings `d3608838`; NOD-01/02 repaired in `87c992c3`; NOD-03/04 repaired 2026-09-23, `0b9c6715`; remaining ranges explicitly unreviewed; no consensus-affecting repair |
| B | Defect inventory and TIH gate | done | — | — | — | [Inventory](defect-inventory-20260910.md) extended to 137 rows in `eba04faf`; first complete gate **87.33/100**; run group `qa-defect-inventory-burn6-20260921`; scored SHA-256 `e6c409eabed23c17fbacec8857d8c6a90d5ecd590a6da5e493c3847b8ba293c0` |

Current finding totals: **0 P1, 9 P2, 6 P3** (A1–A5 within recorded review
limits; all nine P2 repaired; six P3 repaired 2026-09-23 in `0b9c6715`, after
the qualified tip, shipping in the release after next).

The A1–A4 sections retain each unit's scope, decisions and verification at its
closeout. The Status, campaign table and Final summary give the cumulative result.

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

A2 closed after its review, repairs and verification; later units are recorded below.

## Swap and settlement execution review result

The [A3 review](swap-settlement-execution-review-20260921.md) read all 8,797
lines of the three permitted implementation files at `cf12b64f`, after the
required pull on clean `burn6-work`. Work began at 10:46 UTC with a 25-minute
limit. Findings `6d495f43` and repair `043b9d69` were separately gated and
pushed before this log closeout.

| Finding | Result | Consensus impact |
| --- | --- | --- |
| SWX-01 — P2, public redemption omits policy NAV-age limit | Enforce the existing checked policy freshness calculation before pricing or mutation; signed-transaction regressions cover pooled and source-specific settlement | Yes: policy-stale redemptions previously accepted now reject; re-qualify the release tip before deployment |

Reviewed in full: `crates/execution/src/nav_vault_asset_execution.rs` (8,527
lines), `crates/execution/src/pftl_source_settlement.rs` (113), and
`crates/execution/src/vault_bridge_profile_resolution.rs` (157). This includes
the full subscription, entitlement release, redemption, source-custody and
burn-accounting paths. Selected fixtures/assertions in
`market_nav_execution_tests.rs` and selected helpers/imports in
`core_asset_execution_tests.rs` supplied test context; neither whole test file
received an additional correctness review. `lib.rs` and `tests.rs` were
inspected for module placement. No other implementation was reviewed.

The only production file changed is `nav_vault_asset_execution.rs`; it shrinks
by 18 lines by sharing three identical existing freshness checks with the
previously unchecked public redemption path. Public subscription and private
route checks retain their previous semantics. The new 263-line
`swap_settlement_execution_tests.rs` is registered in the short `tests.rs`
include list. No schema, signed encoding, proof/nullifier or private-custody
rule changed. Both source and pooled tests preserve valid redemption while
inbound routes are paused; source issuance is disabled in the source fixture.

All A3 P1/P2 work is complete: **0 P1, 1 P2 repaired, 0 P3**. Final focused
verification: **34 passed, 0 failed, 0 ignored**. Workspace checking passed.
These synthetic/local tests do not establish reserve authenticity, historical
replay or deployment qualification. The full suite is CI's verdict on the
pushed branch; no CI success is claimed. SWX-01 requires release-tip
re-qualification before deployment, in addition to the retained NAV-03
requirement. No qualification or deployment occurred.

A3 closed after its review, repair and verification; later units are recorded below.

## Bridge workflows review result

The [A4 review](bridge-workflows-review-20260921.md) read all 8,624 lines of
the four permitted files at `acfda116`, after the required pull on clean
`burn6-work`. Work began at approximately 11:03 UTC with a 25-minute limit.
Findings `66a68624` and repairs `10bb5655` were separately gated and pushed
before this log closeout.

| Finding | Result | Consensus impact |
| --- | --- | --- |
| BRW-01 — P2, conservation verification trusts cached success | Recompute checked claim/deposit sums and vault identity; reject inconsistent totals and impossible releases | No; local report verification only |
| BRW-02 — P2, deposit receipts contradict selected logs | Require RPC success status, reject explicit supplied-receipt failure, bind enclosing/log hash aliases, reject removed or malformed removal flags | No; unsigned local workflow validation only |
| BRW-03 — P2, conservation reads mix source blocks | Pin every source-state query to one finalized height per chain and reject a changed block hash | No; local source observation policy only |
| BRW-04 — P3, limits follow unbounded file/child reads | Repaired 2026-09-23, `0b9c6715` | No; ships in the release after next |
| BRW-05 — P3, malformed HTTP lengths treated as absent | Repaired 2026-09-23, `0b9c6715` | No; ships in the release after next |
| BRW-06 — P3, V2 recipient offset uses V1 head size | Repaired 2026-09-23, `0b9c6715` | No; ships in the release after next |

Reviewed in full, all under `crates/node/src/`:
`vault_bridge_workflows.rs` (3,918 lines), `vault_bridge_conservation.rs`
(1,755), `ethereum_checkpoint_signing.rs` (2,325), and `pfusdc_tier4.rs` (626),
including in-file tests. Only the first two changed as source. Module/test
registration and test names supplied selection context, without another
implementation review. No permitted file or P1/P2 repair was skipped.

All A4 P1/P2 work is complete: **0 P1, 3 P2 repaired, 3 P3 recorded**.
**13 focused A4 tests passed, 0 failed, 0 ignored**; workspace checking passed.
Each repaired finding has a regression that failed against the original
implementation. The source audit now requires finalized historical RPC reads;
unsupported or unavailable history fails closed. Tests use temporary local
stores and fake cast processes. No real RPC, withdrawal, proof, crash,
historical replay or deployment qualification is established. Global supply
semantics remain delegated; the summary repair validates arithmetic and does
not authenticate a supplied report or its rows. Separate chains and PFTL are
not one atomic snapshot.

**Consensus-affecting A4 repairs: none.** No ledger transition, storage schema,
validator signing/hash encoding, proof or Orchard accounting rule changed.
Existing NAV-03/SWX-01 release-tip re-qualification requirements remain in
force. No qualification or deployment occurred; full-suite verdict belongs
to CI on the pushed branch, with no CI success claimed.

A4 closed after its review, repairs and verification. B completed the inventory
and gate below; optional A5 was not started.

## Remaining node files review result

The [A5 review](node-remaining-review-20260921.md) began at `0e2ae50c` on
clean `burn6-work` after the required pull, at approximately 11:39 UTC with
a 25-minute limit. Findings `d3608838` and repairs `87c992c3` were separately
gated and pushed. This is a focused review, not a whole-file audit.

| Finding | Result | Consensus impact |
| --- | --- | --- |
| NOD-01 — P2, duplicate acknowledgments invent accepted outcomes | Stored outcomes determine counts; missing/conflicting evidence and invalid totals fail closed; three regressions | No; transport reporting only |
| NOD-02 — P2, wallet key output overwrites backup | Resolved key/backup aliases reject before writing, including restore; two regressions | No; local wallet-file interlock only |
| NOD-03 — P3, transport file reads collect unbounded input | Repaired 2026-09-23, `0b9c6715` | No; ships in the release after next |
| NOD-04 — P3, manifests reopen after verification | Repaired 2026-09-23, `0b9c6715` | No; ships in the release after next |

Reviewed ranges at the starting revision, all in `crates/node/src/`:

- `transport_protocol.rs`: production `1–1792` and duplicate-ack fixtures
  `2523–2642`. Other test bodies received selection/context inspection only.
- `block_replay_wallet.rs`: `1–610`, `793–1614`, `1765–2320`, `2664–2815`,
  `3068–3585`. Unreviewed: compatibility tables/root helpers `611–792` and
  `1615–1764`, FastPay replay remainder `2321–2663`, accounting/receipt
  comparison remainder `2816–3067`.
- `rpc_dispatch.rs`: `1–400` and `2500–3102`. Unreviewed: method arms
  `401–2499` and remaining test bodies `3103–3674`; selected test assertions
  and names supplied context only.

Those limits reserve the time box for repair and verification; compiler and
temporary-storage failures also consumed the window. No full-file review or
unreviewed-path safety claim is made. Selected wallet writer/existence helpers,
receipt/option shapes and test registration supplied dependency context only.
No other implementation received a correctness review. All established A5
P1/P2 work is complete; no established repair was skipped.

Only `block_replay_wallet.rs` and `transport_protocol.rs` changed as source.
**36 focused tests passed** and workspace checking passed. The repairs change
neither consensus/state-transition results, replay, storage formats nor signed
or hashed bytes. No new release re-qualification requirement arises from A5;
the existing NAV-03/SWX-01 requirement remains. The wallet check does not claim
concurrent-writer or two-file crash atomicity. Ambiguous receipt histories now
refuse an acknowledgment instead of inventing successful outcomes.

B had already completed. This closeout adds all four NOD- rows and updates
inventory counts to **141 rows: 26 P1, 77 P2, 38 P3**, preserving every prior
finding row verbatim. **B's 87.33/100 gate and recorded hash predate A5**;
no rescore occurred. No Task Node, fleet action, live RPC, spend, installation,
deployment or qualification; git was the only network use. Frozen artifacts,
excluded crates and the protected release checkout were untouched. Full
Rust/Orchard suites, exact archived-chain replay and CI queries were omitted.
The socket test named in Verification was skipped. Stop after this closeout.

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

### A3 decisions

- A4, A5 and B were not started; no inventory edit or TIH scoring. SWX-01's
  consensus classification is in the A3 row pending the separate B unit.
- No permitted implementation file or requested repair was skipped. There is
  no P3 finding to repair. No time or usage limit curtailed the review.
- Not reviewed: outer execution dispatch/authorization/rollback, type
  validation, canonical route hashing/state-root implementations, issued-family
  supply helpers, storage/replay, node/RPC workflows or delegated proof
  implementations. No excluded crate or `programs/` correctness review or
  repair occurred. Exact exclusions are retained in the A3 review document.
- New regressions use signed transactions and synthetic ledger state through
  the existing test admission helper, which bypasses external PFTL proof
  verification. They are state-transition and conservation evidence, not
  reserve-proof, external-event, private-proof or release qualification.
- No local full workspace/Orchard suite, archived-chain replay or CI query.
  The private-route edits only factor out an identical existing age check;
  no private accounting, proof, nullifier or Orchard state rule changed.
  Existing focused PFTL tests exercise the private route-transition helpers.
  Release re-qualification remains required before deployment.
- No Task Node, fleet, live RPC, spend, signup, installation or deployment
  action. Frozen artifacts and the excluded release checkout were untouched.
  All edits/commits used the campaign worktree; git was the only network use.

### A4 decisions

- A5 and B were not started; no inventory edit or TIH scoring. P3 findings
  BRW-04/05/06 remain unfixed as required. No time or usage limit curtailed
  the four-file review or its three repairs.
- Not reviewed: delegated canonical type/supply validation, execution/rollback,
  storage, historical registry replay, finality verification, RPC dispatch,
  or any excluded proof/contract implementation. Specifically, no review or
  repair in `crates/privacy_orchard`, `crates/privacy`, `crates/bridge`,
  `crates/ethereum-contracts`, `crates/pfusdc_proofs`,
  `crates/pftl_uniswap_proofs`, `crates/proofs`, or `programs/`.
- The two `ethereum_checkpoint_signing::tests` tests bind sockets and were
  not invoked under the brief's git/TIH-only network boundary. No Anvil,
  live RPC, fork, full workspace/Orchard suite, archived-chain replay,
  physical crash test or CI query. These skips are not passes or ignored
  Rust test results. Local `cast --help` confirmed the existing block options;
  no generated chain command was executed.
- Legacy manually supplied receipts without status remain permitted as
  unproven input. Actual receipt proof verification, withdrawal consumption
  and replay protection are delegated to unreviewed execution/contracts.
  Source RPC honesty and a coherent PFTL/cross-chain snapshot remain limits.
- No Task Node, fleet action of any kind, spend, signup, install, deployment
  or activation. Frozen artifacts and the protected release checkout were
  untouched. All edits and commits used the campaign worktree; network use
  was git only. No other surface follows A4.

### B decisions

- Only the inventory and this campaign log changed. All 126 prior finding
  rows remain byte-for-byte unchanged; the 11 new rows cover every A1–A4
  finding. New row wording was kept concise before scoring.
- No burn 6 repair was blocked by the release candidate. PFV-01 and
  BRW-04/05/06 remain recorded P3s under the brief, not blocked repairs.
- Optional A5 was not started because this task explicitly ends after B and
  campaign closeout. No source review, repair or Rust test rerun belongs to B.
- No Task Node or fleet action of any kind, live RPC, deployment, activation,
  signup or installation. The protected release checkout, excluded source
  surfaces and frozen artifacts were untouched. Network use was git and
  OpenRouter scoring only; the vault credential stayed in memory.
- One GLM slot exhausted its initial three parse attempts. Only that missing
  slot was resumed with the unchanged harness scorer, model and settings;
  the 14 valid scores were retained. This completed the first full gate,
  without selecting among valid scores or rescoring the document.

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

### A3 verification

Every Cargo test/check below used the exact prefix
`env CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/integrate-20260918-target`.
Compilation of dependencies does not expand the source review or execute
their test suites.

| Command | Final result |
| --- | --- |
| `cargo test -p postfiat-execution burn6_public_redemption --lib --locked` | 2 passed, 0 failed, 0 ignored, 205 filtered |
| `cargo test -p postfiat-execution pftl_ --lib --locked` | 7 passed, 0 failed, 0 ignored, 200 filtered |
| `cargo test -p postfiat-execution vault_bridge --lib --locked` | 21 passed, 0 failed, 0 ignored, 186 filtered |
| `cargo test -p postfiat-execution rotated_route --lib --locked` | 2 passed, 0 failed, 0 ignored, 205 filtered |
| `cargo test -p postfiat-execution ar05_ --lib --locked` | 1 passed, 0 failed, 0 ignored, 206 filtered |
| `cargo test -p postfiat-execution ar11_ --lib --locked` | 1 passed, 0 failed, 0 ignored, 206 filtered |
| `cargo check --workspace --locked` | Passed |
| `cargo fmt --all -- --check` (no environment prefix) | Passed; existing stable-toolchain configuration warnings only |

Unique focused total: **34 passed, 0 failed, 0 ignored**. The existing PFTL
filter also passed **7/7** before adding regressions (198 filtered).
The new two-test filter against the original implementation produced
**1 passed, 1 failed, 0 ignored, 205 filtered**: a packet finalized at height
10 with policy age 5 was accepted at height 16. The failure occurred on the
pooled case before its source-mode loop iteration. Final tests cover both
modes, successful height-15 redemption, first-stale-height rejection with
the entire ledger unchanged, reserve/spread/family-supply conservation, and
nonce replay. These tests use no network or deployment artifact.

Before **each** A3 commit (findings `6d495f43`, repairs `043b9d69` and this
closeout), all three gates passed on the staged changes:
`.venv-docs/bin/mkdocs build --strict`,
`PATH="$PWD/.venv-docs/bin:$PATH" scripts/public-doc-links` (**468 files**),
and `scripts/public-secret-scan` (**tracked-tree**).
`git diff --cached --check` also passed. The new findings/test files were
staged before their secret scans. No Rust test rerun is needed for this
documentation-only closeout.

The initial `git pull --rebase origin release/combined-devnet-20260915` was
already current at `cf12b64f`. Findings and repair commits each completed
`git pull --rebase origin release/combined-devnet-20260915` followed by
`git push origin HEAD:release/combined-devnet-20260915`, with clean trees
between units. This log closeout uses the identical pull/push sequence.
No push to main, full-suite result or CI success is claimed. Stop after A3.

### A4 verification

Every Cargo test/check below used the exact prefix
`env CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/integrate-20260918-target`.
Dependency compilation does not expand the source review or run excluded
test suites. Source-observation regressions use executable local stubs, with
no sockets, validator/RPC access or retained qualification artifacts.

| Command | Final result |
| --- | --- |
| `cargo test -p postfiat-node vault_bridge_workflows::tests --lib --locked` | 4 passed, 0 failed, 0 ignored, 382 filtered |
| `cargo test -p postfiat-node vault_bridge_conservation::tests --lib --locked` | 8 passed, 0 failed, 0 ignored, 378 filtered |
| `cargo test -p postfiat-node pfusdc_tier4::tests --lib --locked` | 1 passed, 0 failed, 0 ignored, 385 filtered |
| `cargo check --workspace --locked` | Passed |
| `cargo fmt --all -- --check` (no environment prefix) | Passed; existing stable-toolchain configuration warnings only |

Unique focused A4 total: **13 passed, 0 failed, 0 ignored**. The workflow
module also passed **3/3**, 380 filtered, before adding regressions. Test
selection used `cargo test -p postfiat-node vault_bridge_ --lib --locked -- --list`
(26 tests listed; none executed by that listing).

The pre-repair command `cargo test -p postfiat-node burn6_ --lib --locked`
produced **3 passed, 3 failed, 0 ignored, 380 filtered**. Its three new A4
regressions failed on an accepted altered source balance, accepted conflicting
block hash, and unpinned state read. The filter also matched three existing
A2 regressions, which passed without additional review or changes; they are
excluded from the 13-test A4 total. Final cases cover altered summary fields,
overflow and impossible releases, valid non-wrapped allocations, failed and
missing-status receipts, contradictory coordinates, removed/malformed flags,
inherited coordinates, pinned source calls and simulated block-hash drift.

Before **each** A4 commit (findings `66a68624`, repairs `10bb5655`, and this
log closeout), the staged changes passed all three gates:
`.venv-docs/bin/mkdocs build --strict`,
`PATH="$PWD/.venv-docs/bin:$PATH" scripts/public-doc-links` (**469 files**),
and `scripts/public-secret-scan` (**tracked-tree**).
`git diff --cached --check` also passed. Findings wording was narrowed before
its commit and all three gates reran on that final staged text. No source
test rerun is needed for the documentation-only closeout.

The initial `git pull --rebase origin release/combined-devnet-20260915` was
already current at `acfda116`. Findings and repairs each completed that pull
followed by `git push origin HEAD:release/combined-devnet-20260915`, with clean
trees between units. This closeout uses the identical pull/push sequence.
No push to main, full-suite result or CI success is claimed. Stop after A4.

### B verification

- Initial `git pull --rebase origin release/combined-devnet-20260915` passed,
  already current at `b322b28b`, on clean `burn6-work` in `/tmp/burn6-20260921`.
- Inventory commit `eba04faf`: all 126 prior rows compared byte-for-byte equal
  to the starting revision. The review-ID audit matched PFV- 1, NAV- 3, SWX- 1
  and BRW- 6, with no missing or duplicate ID; every repair commit is an
  ancestor of the campaign tip. Classification, severity, disposition and
  completeness counts independently sum to **137**.
- Before inventory commit `eba04faf`, the staged changes passed
  `.venv-docs/bin/mkdocs build --strict`,
  `PATH="$PWD/.venv-docs/bin:$PATH" scripts/public-doc-links` (**469 files**),
  `scripts/public-secret-scan` (**tracked-tree**) and
  `git diff --cached --check`. The required pull followed by
  `git push origin HEAD:release/combined-devnet-20260915` succeeded; the tree
  was clean before scoring.
- All fifteen stored scores were checked against the exact prompt and its
  hash, document SHA-256, run group, three model identities, indices 1–5 and
  parsed raw responses. The scored inventory bytes remain unchanged from
  `eba04faf`. Scores and retained record paths are below.
- Before this log closeout commit, the same three gates and staged diff check
  passed again, including the **469-file** link check. This documentation-only
  closeout uses the same pull/push sequence; pushes go only to the release
  branch, with a clean tree at completion.
- No Rust suite, CI query or archived-chain replay ran during B. Retained
  A1–A4 focused counts are **13, 49, 34 and 13 passes** (109 total, including
  the A2 Python tests), with one existing A1 supplied-proof test ignored and
  two A4 socket tests not invoked. These are earlier results, not new runs.
  **Full Rust suite verdict pending** for `e9ccdeda`, `043b9d69` and `10bb5655`.

### A5 verification

Cargo commands used the prefix
`env PATH="/usr/bin:$PATH" CARGO_NET_OFFLINE=true CARGO_TARGET_DIR=/tmp/integrate-20260918-target CARGO_BUILD_JOBS=1`.
Dependency compilation does not constitute another source review or excluded
test-suite run. Fixtures use local temporary files and deterministic test keys.

| Command | Final result |
| --- | --- |
| `cargo test -p postfiat-node block_replay_wallet:: --lib --locked` | 3 passed, 0 failed, 0 ignored, 385 filtered |
| `cargo test -p postfiat-node transport_protocol::transport_cli_tests --bin postfiat-node --locked -- --skip transport_listener_mode_failure_prevents_ready_report` | 30 passed, 0 failed, 0 ignored, 146 filtered |
| `cargo test -p postfiat-node wallet_keygen_restore_round_trips_without_report_secret_leakage --lib --locked` | 1 passed, 0 failed, 0 ignored, 387 filtered |
| `cargo test -p postfiat-node wallet_sign_transfer_emits_submit_ready_redacted_transfer --lib --locked` | 1 passed, 0 failed, 0 ignored, 387 filtered |
| `cargo test -p postfiat-node wallet_test_vector_is_deterministic_and_redacted --lib --locked` | 1 passed, 0 failed, 0 ignored, 387 filtered |
| `cargo check --workspace --locked` | Passed |
| `cargo fmt --all -- --check` (without prefix) | Passed |

Unique total: **36 passed, 0 failed, 0 ignored**. The socket test was explicitly
filtered out, not passed. Before repair,
`cargo test -p postfiat-node --lib --bin postfiat-node --locked --no-fail-fast burn6_nod_`
failed all five new tests at their intended assertions: **0 passed, 2 failed,
386 filtered** in the library; **0 passed, 3 failed, 173 filtered** in the binary.
Earlier compiler-wrapper aborts, linker failures and tmpfs user-quota failures
executed no tests. Existing system `cc` and disk-backed temporary cache storage
resolved those build failures; `/tmp/integrate-20260918-target` now links to
`/var/tmp/burn6-a5-target`. No toolchain installation or repository build-config
change occurred. **Full Rust suite verdict pending** for `87c992c3`.

Before each A5 commit, staged changes passed `.venv-docs/bin/mkdocs build --strict`,
`PATH="$PWD/.venv-docs/bin:$PATH" scripts/public-doc-links` (**470 files**),
`scripts/public-secret-scan` (**tracked-tree**) and `git diff --cached --check`.
The initial pull was current at `0e2ae50c`; findings and repairs each completed
the required pull/rebase and `git push origin HEAD:release/combined-devnet-20260915`
with a clean tree between units. This documentation-only closeout uses the same
gates and pull/push sequence; no Rust test rerun or inventory scoring is needed.
The inventory audit preserves all 137 previous rows and adds exactly four NOD-
IDs; severity, classification, disposition and completeness counts sum to 141.
No push to main, full-suite result, CI pass or deployment qualification is claimed.

## Scores

The exact 137-row inventory bytes from `eba04faf`, SHA-256
`e6c409eabed23c17fbacec8857d8c6a90d5ecd590a6da5e493c3847b8ba293c0`,
received fifteen fresh OpenRouter reviews at temperature 0 with an 8,000-token
response limit: the same three judges and five reviews each as the
[original gate](qa-campaign-20260910.md#final-text-improvement-harness-gate),
[burn 2 gate](qa-campaign-20260910.md#burn-2-final-text-improvement-harness-gate)
and [burn 5 example](qa-campaign-20260916.md#scores). The prompt was
`Rate this document on a scale of 1-100. Output the score and your reasoning.`
The strict-JSON wrapper was verified verbatim against the retained burn 5
prompt. The credential came from vault label `openroutertih` and was passed
in memory; no credential was written to the log or database.

| Judge | Scores (run order) | Average |
| --- | --- | ---: |
| `openai/gpt-6-astra-pro` | 86, 86, 86, 85, 86 | 85.80 |
| `anthropic/claude-fable-5.1` | 84, 87, 87, 86, 87 | 86.20 |
| `z-ai/glm-5.3` | 90, 90, 91, 88, 91 | 90.00 |
| **All fifteen** | — | **87.33** |

Run group: `qa-defect-inventory-burn6-20260921`. The first complete full gate
met the **86/100** stop condition; **no wording rewrite or rescore** followed.
The harness ran `score` with
`--gate full --runs 5 --force --temperature 0 --max-tokens 8000 --concurrency 15`,
the explicit prompt above and the named run group. GLM run 1 succeeded on its
third built-in attempt. GLM run 4 exhausted three invalid-output attempts,
ending the CLI with 14 valid records. Only run 4 was resumed through the
unchanged `run_score_task`, with the same prompt, document, temperature,
response limit and model; the resumed call succeeded on its first attempt.
No valid score was replaced.

All fifteen SQLite records were verified against the full prompt, prompt hash,
document hash, group, model, index and parsed response score. The external
records are `/home/postfiatchad/pastedocs/.qa-campaign-defect-inventory-burn6-20260921/score.log`
and `scores.sqlite3` in that directory; they are not committed to Git.

## Final summary

| Surface | P1 | P2 | P3 | Findings commit | Repair commit |
| --- | ---: | ---: | ---: | --- | --- |
| A1 — Portfolio verification | 0 | 0 | 1 | `21459c8a` | None; no P1/P2 |
| A2 — NAV and reserve verification | 0 | 3 | 0 | `4bc6d98d` | `e9ccdeda` |
| A3 — Swap and settlement execution | 0 | 1 | 0 | `6d495f43` | `043b9d69` |
| A4 — Bridge workflows | 0 | 3 | 3 | `66a68624` | `10bb5655` |
| **A1–A4 total** | **0** | **7** | **4** | — | — |

All seven P2s are repaired in source. PFV-01 and BRW-04/05/06 remain recorded
without repair as required for P3. **No burn 6 repair is blocked by the release
candidate.** Optional A5 was not reviewed and contributes no finding rows.
**Full Rust suite verdict pending** for all three repair commits: `e9ccdeda`,
`043b9d69` and `10bb5655`. No CI success or deployment qualification is claimed.

**Consensus-affecting repairs:**

- **NAV-03 — `e9ccdeda`:** rejected local bridge transitions no longer persist
  reserve mutation when receipt history is invalid or would exceed its bounds.
  This is consensus-affecting under the brief's state-transition-result rule;
  consensus execution rules and the successful JSON encoding are unchanged.
- **SWX-01 — `043b9d69`:** policy-stale public redemptions previously accepted
  now reject before pricing or mutation, for pooled and source-specific paths.

Both require **release-tip re-qualification before deployment**. NAV-01/02
and BRW-01/02/03 are not consensus-affecting. No re-qualification, deployment
or activation occurred.

Remaining risks and limits:

- PFV-01 leaves nine collection context mismatches without individual negative
  regressions; no invalid-proof acceptance was established. Excluded guest
  holdings/weight/amount arithmetic and freshness remain unqualified.
- BRW-04 retains unbounded file/child reads and missing deadlines; BRW-05
  retains ambiguous HTTP length handling; BRW-06 retains the V1/V2 recipient
  offset discrepancy. No checkpoint-signature or receipt-proof bypass was shown.
- Reserve completeness, proof/overlay disjointness, source freshness and price
  trust depend on unreviewed proof/source machinery. Synthetic fixtures do not
  authenticate reserves. Cross-file crash/I/O atomicity and concurrent writers
  remain outside NAV-03's guarantee.
- Bridge reports establish arithmetic consistency, not observation authenticity
  or delegated supply semantics. Legacy supplied receipts without status remain
  unproven; RPC honesty, historical availability and a coherent PFTL/cross-chain
  snapshot remain limits. Withdrawal proof/replay guarantees are delegated.
- Full-suite CI, exact archived-chain replay and release qualification remain
  pending. The ignored supplied-proof fixture and skipped checkpoint socket
  tests are not passes. No deployment readiness is inferred from focused tests.

The inventory adds **PFV- 1, NAV- 3, SWX- 1 and BRW- 6** in `eba04faf`, for
**137 rows: 26 P1, 75 P2 and 36 P3**, with every prior row unchanged.
Its first complete gate passed at **87.33/100**, run group
`qa-defect-inventory-burn6-20260921`, scored file SHA-256
`e6c409eabed23c17fbacec8857d8c6a90d5ecd590a6da5e493c3847b8ba293c0`.
One invalid-output slot was resumed; no document rewrite or rescore occurred.
The inventory and this log closeout are separately gated and pushed to
`release/combined-devnet-20260915`. No Task Node or fleet action occurred;
the protected release checkout was untouched. Stop after this closeout.

## September 23 P3 repairs

The six burn 6 P3s were repaired on 2026-09-23 in `0b9c6715` from a
detached worktree of `release/combined-devnet-20260915` at `129d70ff`.
**The deployed candidate is unchanged.** The deployment candidate is pinned to
the qualified executable `e7bb1afa…`, built from `1a0989ad`
(`deployments/release-repair-20260922/`, `deployments/combined-devnet-20260923/`).
These commits land after that tip, do not change what is deployed, and ship in
the release after next.

| Finding | Repair | Regression |
| --- | --- | --- |
| PFV-01 | Test-only: each of the nine digests and the snapshot count is mismatched individually, with its specific error | `collection_context_binds_every_public_value` |
| BRW-04 | Proof and lineage files are capped on the opened handle; each `cast` child has bounded stdout, stderr and a 120 s deadline, and is killed and reaped on overflow or timeout | `bounded_file_reads_cap_the_open_handle`, `bounded_cast_output_caps_pipes_and_duration` |
| BRW-05 | Each `Content-Length` is parsed strictly; malformed, empty or duplicate lengths are rejected | `rpc_content_length_framing_is_strict` |
| BRW-06 | The recipient decoder takes the selected event version's head length | `v2_deposit_log_recipient_offset_uses_v2_head` |
| NOD-03 | Topology, registry and key reads (8 MiB) and payload reads (frame cap) are capped on the opened handle | `transport_local_file_reads_are_bounded_on_the_open_handle` |
| NOD-04 | `verify_governance_genesis_bundle_snapshot` returns the report and manifests from one verified read; `rpc --method manifests` no longer rereads the file | `governance_genesis_bundle_binds_registry_and_operator_manifests` |

No repair is consensus-affecting: the changes cover local operator tooling,
RPC reporting and tests only. All six findings are repaired; none remains open.

Verification at `0b9c6715` (`-j 2`, `RUST_TEST_THREADS=2`):

- `cargo check -p postfiat-execution -p postfiat-node`: pass.
- Focused tests, all passing: execution `yolo_collection_verifier_tests` 2;
  node library `vault_bridge` 30 (2 ignored, pre-existing),
  `ethereum_checkpoint_signing::` 3, `governance_genesis_bundle` 2; node
  binary `transport_protocol::` 32.
- `cargo fmt --all -- --check`: pass. `cargo clippy -p postfiat-execution
  -p postfiat-node --lib --bins --tests`: no warnings.
- `mkdocs build --strict`, `scripts/public-doc-links` and
  `scripts/public-secret-scan`: pass.

No Orchard boundary is crossed, so no Orchard, workspace or archived-chain
replay run applies. No Task Node, fleet or deployment action occurred.
