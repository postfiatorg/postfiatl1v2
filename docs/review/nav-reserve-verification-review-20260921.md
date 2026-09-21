# NAV and reserve verification review — 2026-09-21

This is A2 of the [burn 6 campaign](qa-campaign-20260921-burn6-brief.md),
reviewed at `b82052ce` on `burn6-work` in `/tmp/burn6-20260921` after the
required release-branch pull. References describe the pre-repair revision.
All 4,022 lines of the six permitted files were read.

| File | Lines |
| --- | ---: |
| `crates/nav_reserve_protocol/src/lib.rs` | 153 |
| `crates/types/src/nav_reserve_public_values.rs` | 393 |
| `crates/node/src/market_bridge.rs` | 2,628 |
| `crates/node/src/tests/nav_reserve_proof_status_tests.rs` | 143 |
| `scripts/a666-build-live-nav-mark-ops.py` | 476 |
| `scripts/a666-build-route-epoch-advance.py` | 229 |

Focus: stale, partial or duplicated reserve accounting; proof asset/epoch/route
binding; route ordering and replay; same-cycle double counting; and price-input
trust. These files expose codecs, host constructions, status and operation
builders, not the complete reserve verifier or external-source valuation policy.

## Findings

### NAV-01. P2 — duplicate status rows inflate the subscription overlay

**Source:** `scripts/a666-build-live-nav-mark-ops.py:69-107`, with allocation
accumulation at `:137-139`.

**Condition:** the supplied vault snapshot contains the same bucket or active
subscription allocation twice. The primary-market reserve exceeds one copy of
the bucket's outstanding balance.

**Observed behaviour:** bucket/receipt dictionaries silently overwrite repeated
IDs, while backing is summed from the original bucket array and allocations
are accumulated without unique-ID checks. A local reproduction with one
10-atom bucket and a 20-atom route reserve produces 1,000 NAV valuation units;
repeating that bucket produces 2,000. Repeating an eligible allocation also
counts the same reserve twice. The builder can forward this inflated overlay
to packet preparation. This establishes a builder accounting error, not
acceptance of an invalid proof by consensus.

**Expected behaviour:** each bucket, receipt and allocation identity occurs
once; ambiguous snapshots fail before overlay construction or packet output.

**Suggested minimal change:** reject duplicate IDs in all three collections
before indexing or summing. Test duplicate buckets, allocations and receipts,
including conflicting duplicate rows, and retain unique-row accounting.
This changes local unsigned-input admission only; valid overlay hashes and
validator rules remain unchanged. **Not consensus-affecting.**

### NAV-02. P2 — packet input can replace the reserve-submit operation tag

**Source:** `scripts/a666-build-live-nav-mark-ops.py:249-283` and `:415`.

**Condition:** a supplied packet has all fields accepted by `validate_packet`
and an `operation` field other than `nav_reserve_submit`, such as
`nav_epoch_finalize`.

**Observed behaviour:** validation ignores the tag. The dictionary expansion
`{"operation": "nav_reserve_submit", **packet}` lets the input replace it.
The output named `01-reserve-submit.ops.json` then requests a different
operation using the reserve-operator source/key path, followed by the normal
issuer finalization request. The local reproduction accepted the packet and
emitted `nav_epoch_finalize`. No signing or chain acceptance is claimed.

**Expected behaviour:** packet input can only generate a reserve submission;
a mismatched tag fails before any operation file is written. Both untagged
packet bodies and explicitly tagged reserve-submit bodies remain supported.

**Suggested minimal change:** validate the optional tag and assign the fixed
tag after expansion as defense in depth. Add a regression through the CLI
that checks rejection, absence of operation files and both supported forms.
**Not consensus-affecting:** unsigned builder validation only; no validator
rule, state interpretation, storage format or valid signed encoding changes.

### NAV-03. P2 — invalid local receipt history is checked after reserve mutation

**Source:** `crates/node/src/market_bridge.rs:2374-2394`, `:2397-2417` and
`:2420-2459`.

**Condition:** a local bridge primary subscription has a valid ledger/request,
but the existing transition-receipt file is malformed or otherwise invalid;
alternatively, an accepted append would exceed the local history limit.

**Observed behaviour:** `pftl_uniswap_apply_transition` writes the changed
ledger before reading/validating receipt history. A receipt-validation error
therefore rejects the command after persisting its reserve/supply/nonce
change, without its replay receipt. Separately, append validates only the old
history length, so a full history accepts one extra receipt that subsequent
reads reject. These are deterministic local validation/publication errors;
this path uses the legacy local bridge ledger, not the consensus block ledger.

**Expected behaviour:** known-invalid or full history rejects without changing
the local ledger or receipts; the last permitted receipt remains readable.

**Suggested minimal change:** prepare and validate the prospective receipt
history and serialize the report before either persistent write; only then
publish the existing ledger and receipt files. Regress malformed history,
duplicate history and the exact capacity boundary, checking unchanged file
bytes on rejection. This narrow change does not provide crash atomicity
between two successful-file publications; that storage-design boundary is
explicitly outside the claimed repair.

**Consensus-affecting under the brief's state-transition-result rule:** a
rejected local transition will cease leaving a persisted reserve mutation.
No consensus execution or encoding change is proposed, but the release tip
must be **re-qualified before deployment** under the campaign's conservative
classification. No deployment is part of A2.

There are **0 P1, 3 P2 and 0 P3** findings.

## Areas with no findings

- `nav_reserve_protocol/src/lib.rs:1-153`: validates the public values and
  overlay-root encoding, rejects zero overlay and checked-add overflow, and
  hashes the entire encoded public values with separate domains. The stable
  vector and bounds test passes. The helper does not authenticate overlay
  provenance or establish disjointness from the proof's reserve perimeter.
- `nav_reserve_public_values.rs:1-393`: exact 584-byte framing, magic/version,
  canonical hex, checked interval/count/value arithmetic, gross less
  liabilities, trust-bucket totals, matching encode/decode order and bounded
  reader. Five focused tests pass. Epoch is nonzero but freshness relative to
  chain height, expected asset/profile/genesis and source policy are consumer
  responsibilities, not codec guarantees.
- `market_bridge.rs:1-461`: market-operation builders select by asset, epoch,
  finalized state and current packet hash, bind reserve/supply/evidence roots,
  validate policy inputs and invoke replay recomputation before publication.
  External prices and recomputation are delegated; a caller-supplied input
  file is not independent market-price evidence.
- `market_bridge.rs:463-1768`: checked supply/bucket/allocation accounting,
  asset/family filtering, governed route selection, route/packet lookup and
  separate outstanding export claims were examined. Ingress preflight mutates
  an in-memory ledger and explicitly reports simulated readiness. Status is
  not proof verification, and separately loaded files are not established
  here as a single atomic chain snapshot.
- `market_bridge.rs:1770-2628`: reserve-backed redemption search is monotone
  over the delegated settlement conversion; local replay filters by route;
  launch config binds chain/assets/controller/config digest; local route and
  receipt identity duplicates are rejected on read. NAV-03 identifies the
  publication-order exception. Called bridge transitions and cryptographic
  finality are outside this review.
- `nav_reserve_proof_status_tests.rs:1-143`: tests persistence, exact asset
  lookup, descending 16-packet bound, explicit trust buckets and omission of
  raw proof bytes. The fixture deliberately writes synthetic proof bytes to
  storage; its passing result does not establish proof acceptance.
- `a666-build-live-nav-mark-ops.py:184-476`: pins route/assets/profile, requires
  paused live-value mode and the supply invariant in the derived-packet path,
  checks newer packet epoch and evidence size bounds, and excludes reported
  primary reserve beyond the computed backing. NAV-01/NAV-02 are exceptions
  above. Proof bytes are not cryptographically verified by this Python file.
- `a666-build-route-epoch-advance.py:1-229`: checks paused route and empty order
  state, exact prior pricing epoch, strictly newer NAV epoch, asset/profile/
  policy/program identities, packet-hash syntax and expiry; advances route
  and policy epochs once. Larger NAV epoch gaps are intentionally supported.
  Optional identity validation is delegated. The manifest is a construction
  artifact, not evidence that its reserve submission/finalization succeeded.

## Review limits and skips

Only the six listed production/test files are the A2 review surface. The two
adjacent Python builder test files supplied fixtures and focused regression
coverage. Selected config/subscription fixtures in
`crates/node/src/tests/pftl_uniswap_bridge_rpc_tests.rs` supplied local test
construction context; that whole file was not reviewed as another surface.
Repository guidance, NAV documentation, the burn 5 review format and required
gate scripts supplied context. Searches for module wiring and helper names
do not constitute review of their implementations.

Not reviewed: `crates/execution/src/nav_sp1_verifier.rs`, reserve submission
and finalization execution, subscription overlay recomputation, route-epoch
execution, shared type/state/storage/finality implementations, optional
`scripts/a666-pfusdc-reserve-demo.py` identity validators, or the
`tools/nav-reserve-proof` packet builder, guest and source-adapter policy.
Thus private reserve completeness, disjoint proof/overlay holdings,
source-time freshness, price manipulation by a source operator and consensus
rejection of a wrong-epoch/route/asset proof remain delegated and unqualified
by this review. No malformed-reserve witness was proved.

All explicitly excluded crates, `programs/`, other campaign surfaces and unit
B remain untouched. No inventory update/scoring, Task Node action, fleet
operation, live RPC, deployment, spend or install occurred. Local fixtures
use temporary directories and dummy key-file contents; no real keys or
signatures are used by the Python regressions. No external proof CLI is run
except the fixture stub. Network use is git only.

Cross-file crash recovery, concurrent local writers and exact archived-chain
replay are not established. The full Rust/Orchard suites and CI status queries
are deferred; full-suite verdict belongs to CI on the pushed branch. Exact
repair, test and gate results are recorded in the
[campaign log](qa-campaign-20260921.md#verification).
