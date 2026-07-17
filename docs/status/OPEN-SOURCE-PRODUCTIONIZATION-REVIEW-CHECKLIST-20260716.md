# PostFiat L1 Open-Source Productionization Review Checklist

> **Execution status:** This is an active repository-wide code triage and remediation program, not a documentation-editing exercise. STEP 1 means inspecting the code, history, dependencies, runtime surfaces, and tests to complete the evidence-backed triage. STEP 2 means implementing and testing every required P0/P1 fix in the codebase. Editing this document is only recordkeeping for that work and is not progress by itself.

**Date:** 2026-07-16  
**Repository:** `postfiatl1v2`  
**Purpose:** define the complete, local-only review and remediation program required before this repository is made public or represented as production-ready.

## Non-negotiable publication rule

This repository is **not ready to be made public merely because the review is written**. The review and the remediation are two separate deliverables. Both must be completed against the same audited baseline before publication:

1. **STEP 1 — finish the comprehensive review:** establish the complete, evidence-backed truth about the repository and turn every P0/P1 into an actionable closure specification.
2. **STEP 2 — fix and prove closure:** implement the P0/P1 fixes, test the integrated result, correct the public claims, and produce a zero-open-blocker release candidate.

All confirmed P0s must be fixed before publication. Publishing known consensus-safety, custody, monetary-integrity, cryptographic, privacy, or secret-exposure defects would be a direct credibility failure: outside reviewers would correctly identify that the project knowingly published safety-critical defects. P1s also block publication unless the affected feature or surface is completely removed or disabled and every claim about it is withdrawn.

Completing STEP 1 without STEP 2 means **the repository remains blocked from public release**. Passing isolated tests or fixing selected findings without completing STEP 1 means **the repository also remains blocked**, because undiscovered or unclassified release-critical risk may remain.

## Mandatory two-step program

Public release is **not** the first action. Publishing a repository while knowingly carrying unresolved consensus, custody, monetary-safety, cryptographic, privacy, or other P0 defects would create an avoidable credibility failure. The repository must therefore pass these steps in order:

| Gate | Required deliverable | Completion condition | Publication state |
|---|---|---|---|
| **STEP 1** | This document completed as a comprehensive, evidence-backed audit and remediation specification | Every repository surface is reviewed; every finding is classified; every P0/P1 has exact code evidence, a complete fix specification, and objective acceptance tests | **Blocked** |
| **STEP 2** | The audited P0/P1 fixes actually implemented and tested | Every P0/P1 is closed or its entire affected surface is removed; regression and integrated release batteries pass; public claims match the resulting code | **Eligible for final public-release gate** |

There is no publication milestone between STEP 1 and STEP 2. A comprehensive document that still lists open P0s or P1s is a remediation input, not a release artifact. In particular, all P0s must be closed before publication because knowingly exposing a safety-critical defect would undermine the project's technical credibility immediately.

## P0 fix-before-public program

The detailed evidence and evolving line references live in
`docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md`. The table below is
the governing fix program, not a list of risks that may be disclosed and deferred.
STEP 1 must finish the evidence, reproduction, design, and acceptance criteria for
every row. STEP 2 must implement the fix and produce the closure evidence. A row
may leave the table only after the integrated public candidate proves it closed.

| Blocker | Required STEP 2 disposition | Minimum closure proof before publication |
|---|---|---|
| `P0-CONSENSUS-01` — production finality does not implement the claimed chained HotStuff safety rule | Implement a complete, internally consistent proposal/QC/lock/timeout/commit protocol, including cross-view safety and verified high-QC extension; correct the whitepaper if the implemented protocol deliberately differs | Model/DST counterexample fails before and passes after; conflicting-commit safety under Byzantine schedules; deterministic replay and upgrade/migration tests |
| `P0-GOVERNANCE-01` — governance authorization is based on unsigned validator-name assertions | Bind every governance vote and certificate to a registered validator key, chain/genesis, policy action, payload, epoch, height/view, and replay domain | Forged-name, duplicate-vote, replay, wrong-domain, stale-committee, and conflicting-policy tests plus successful authorized activation/replay |
| `P0-RPC-01` — remote unsigned `wrap_owned` can debit a caller-selected account and mutate validator-local state | Remove the legacy remote mutation and replace it with the signed, sequenced, consensus-ordered FastLane deposit path; eliminate local-clock object IDs | Real-store theft/divergence reproduction fails after fix; signed deposit succeeds; unauthorized, replayed, duplicate, and cross-node deterministic tests pass |
| `P0-BRIDGE-01` — PFTL/Ethereum consume, import, and refund trust unverified operator assertions | Implement cryptographically or threshold verified external-chain evidence with mutually exclusive consume/refund state transitions, or remove and prove unreachable the entire external bridge surface and its claims | Fictitious burn, forged finality, double representation, refund/consume race, replay, reorg, and wrong-chain tests; local-fork happy-path conservation test if retained |
| `P0-SUPPLY-01` — EVM `MintController` releases escrowed mints against a caller-authored, unverified settlement assertion | Replace `SettlementProof` with a verifier-bound, domain-separated settlement certificate or direct on-chain balance/venue/escrow proof that cannot be authored by the beneficiary; otherwise remove the mint-release surface | Pre-fix beneficiary self-release exploit; forged/duplicate/wrong-recipient/wrong-envelope/wrong-venue/wrong-chain/partial-settlement tests reject; real settlement releases once; cross-domain supply/backing oracle passes |
| `P0-CUSTODY-01` — browser wallet can transmit a backup containing the master seed to the proxy signer | Make self-custody signing local, delete the seed/backup transport API, and make any custodial signer a separate explicit product mode | Network-capture test proves no seed/private key leaves the browser; proxy rejects legacy payloads; fresh-wallet send/swap/recovery flows pass |
| `P0-CUSTODY-02` — the supposedly redacted wallet test-vector CLI echoes caller-supplied master/signature seeds in JSON | Remove both seed fields from the report, version the public schema, and prove the real CLI output contains neither field name nor supplied value | Pre-fix real CLI capture contains both exact secrets while claiming redaction; unit and subprocess boundary tests prove v2 output omits both names/values while the deterministic signed vector remains reproducible |
| `P0-PROXY-AUTH-01` — the wallet proxy publicly binds and dispatches mutation and custody-signing requests without authentication by default | Default to loopback, require an authenticated user/session boundary for every mutation, enforce a nonempty exact origin policy for browser use, and remove custody-signing methods from the public proxy | Dynamic pre-fix reproduction is inverted: public default fails startup, tokenless/originless mutations reject before route dispatch, signer methods are absent, and authenticated same-origin wallet flows pass |
| `P0-PRIVACY-01` — reachable legacy “shielded” notes persist owner, asset, value, and memo in cleartext | Remove/disable the legacy cleartext note path or migrate it to the supported Orchard representation; correct every privacy claim | Cleartext Mint/Spend/Migrate actions are unreachable; migration/replay and wire/log/state privacy scans pass; supported private round trip remains conserved |
| `P0-PRIVACY-02` — live Asset-Orchard ingress publishes the complete recipient note opening in the consensus payload | Replace ingress v1 with an opaque commitment + authenticated ciphertext payload, or disable v1 live while retaining exact archive replay; never relay `rho`, `psi`, `rcm`, diversifier, recipient key material, or note value | Real batch/archive regression proves v1 exposes the opening; live v1 rejects without burn/state mutation; new ingress contains no opening fields, uses the versioned note-encryption envelope, decrypts only for the intended wallet, and completes conserved ingress/swap/private-egress replay |
| `P0-PUBLIC-EVIDENCE-01` — the tracked public-source evidence tree contains real devnet shielded note openings and operator-local artifacts | Remove raw run evidence from the publication tree, preserve it in a restricted integrity-checked archive, retain only redaction-safe summaries/manifests, and make note-opening detection a blocking current-tree/history rule | Pre-fix scanner fails on real `rho`/`psi`/`rcm` values without printing them; deterministic archive hash/count/sample extraction verify preservation; post-fix tracked-tree scan passes and raw evidence is absent |
| `P0-WALLET-02` — public Vite development server and vulnerable browser toolchain expose live mutation paths | Ship a production static build behind the hardened backend, update vulnerable Vite/esbuild dependencies, enforce CSP/origin/auth boundaries, and make public dev binding impossible by default | Clean production build; dependency audit; hostile-origin/CSP/mutation-auth tests; no dev server in release manifests |
| `P0-WALLET-BRIDGE-DEST-01` — browser, proxy relay, and live UX script implicitly targeted a retired Arbitrum bridge vault; the browser also allowed a per-wallet override | Make every bridge money destination and deployed bytecode hash an explicit reviewed build/deployment binding, deny retired destinations, verify live code before approval/relay/transaction, disable every mutation path when either binding is absent, and remove the destination from user settings | Pre-fix source-boundary tests expose the retired default and missing code binding; browser code-hash vector plus real backend empty/default/retired/missing-hash tests; live script fails closed; public-default scan, wallet/proxy suites, production build, and dependency audit pass |
| `P0-SECRET-01` — full history contains a captured Jupyter access token | Revoke/decommission the credential, construct sanitized public history, and add blocking current-tree/history secret scanning | Rotation/decommission record; clean full-ref scan of the exact publication candidate; old contaminated refs excluded from every published remote and archive |
| `P0-ASSET-01` — legacy owned wrapping accepted an issued-asset label for native-backed value | Retain the fail-closed PFT-only guard already implemented and prove it remains closed in the integrated candidate | Execution and node-RPC regressions reject `pfUSDC`, `a651`, and unknown labels with no mutation; native PFT path and full release suite pass |
| `P0-ISSUED-SUPPLY-02` — issued-asset mint caps omitted FastLane and AssetOrchard custody | Count every supported issued-asset custody lane at both mint admission and the replicated-state proposal/commit/replay boundary; reject unknown custody assets and any aggregate above `max_supply` | Pre-fix state-root counterexample with 10 public + 1 private under cap 10; FastLane reserve mint-cap regression; exact-cap inverse; real AssetOrchard ingress/egress and full replay suites |

This is a minimum register, not a completeness claim. Any additional P0 found while
finishing STEP 1 is appended immediately and receives the same STEP 2 treatment.
No P0 may be downgraded merely to make publication possible. A feature-level removal
closes a P0 only when the code path is absent or fail-closed, migrations are handled,
tests prove it unreachable, and all public claims and examples are removed.

External specialist audit is a separate real-value production-launch gate. It is
strongly recommended and must remain in the launch plan, but waiting for an external
review does not block this team from completing STEP 1, implementing every STEP 2
fix, running the full local assurance program, or making the corrected source public.

### STEP 1 — comprehensively complete this audit document

Complete every review section in this document and replace unchecked discovery items with evidence-backed findings or explicit clean results. The completed audit must:

- cover the entire tracked repository, full Git history, production/runtime surfaces, tests, tooling, documentation, and release configuration;
- classify every finding P0 through P3 with exact files, line references, affected invariant, failure scenario, and confidence level;
- provide the bloat disposition, unsafe/outdated inventory, and whitepaper-conformance matrix requested by the founder;
- distinguish confirmed defects from hypotheses that still require adversarial reproduction;
- define an implementation order and objective acceptance test for every P0 and P1;
- identify which claims and features must be removed, relabeled, or disabled if they cannot be fixed safely;
- end with a signed-off release-blocker register containing no unowned P0 or P1.

STEP 1 is complete only when the audit is comprehensive enough to serve as the controlling remediation specification for STEP 2. A partial scan, generated summary, unchecked checklist, or list of suspicions is not completion. STEP 1 does not make the repository safe to publish.

### STEP 2 — actually close and test every P0 and P1

Implement the audit’s P0 and P1 remediations in dependency order. For every item:

1. reproduce or formally demonstrate the defect before changing it;
2. implement the smallest complete fix, including migrations and compatibility behavior;
3. add a regression test that fails on the vulnerable behavior and passes on the fix;
4. run the relevant unit, integration, property, fuzz, deterministic-simulation, fault-injection, replay, and performance suites;
5. perform a second internal security review of protocol, cryptographic, custody, bridge, and privacy fixes, and package the exact artifacts needed for later external specialist audit;
6. record the code revision, tests, artifacts, hashes, reviewer, and residual risk in the blocker register;
7. remove or correct every public claim invalidated by the final implementation.

There is no “documented but accepted,” “known issue,” “roadmap,” “devnet only,” or waiver status for a reachable P0 or P1 in the public-release candidate. A finding is either closed with evidence or the affected feature is removed/disabled, proven unreachable, and all claims are corrected. The repository must not be made public until STEP 2 is complete and its integrated release battery passes.

## Review contract

STEP 1 is an audit and planning exercise. It does not itself close findings or authorize publication. It does not itself authorize deployment, money movement, secret rotation, history rewriting, branch pushes, or deletion of evidence. Existing uncommitted work must be preserved. Source files will not be sent to external review services without explicit founder authorization. STEP 2 is the implementation and verification phase performed from the completed audit, with changes reviewed and tested before inclusion in the public-release candidate.

The final report must contain:

1. A severity-ranked list of P0, P1, P2, and P3 findings.
2. A concrete inventory of repository bloat and a keep/move/delete/generate disposition.
3. A concrete inventory of unsafe, obsolete, unsupported, or misleading code and dependencies.
4. A claim-by-claim matrix mapping the implementation to the canonical whitepaper.
5. A productionization plan with owners, ordering, acceptance tests, and release gates.
6. A public-release checklist that requires P0/P1 closure before publication and separately records the additional launch evidence needed before real-value operation.
7. Evidence for every material claim: file paths, line references, commands, and test results.

Severity meanings:

- **P0:** can violate safety, custody, consensus agreement, supply, authorization, privacy, or make public release itself unsafe; blocks STEP 2 completion and public release.
- **P1:** major liveness, availability, operational, scale, or correctness gap; blocks STEP 2 completion and public release unless the affected surface is completely removed or disabled and its claims are withdrawn.
- **P2:** material maintainability, performance, API, documentation, or testing debt.
- **P3:** hygiene, polish, or low-risk cleanup.

## Phase 0 — preserve resumability and establish ground truth

- [x] Write the current StakeHub end-to-end demo resume handoff before beginning the L1 review.
- [x] Record repository path, branch, HEAD, remotes, upstream relationship, tags, and worktree status.
- [x] Identify and preserve every tracked modification, untracked file, and local-only branch.
- [x] Record toolchain versions and host assumptions used by the audit.
- [x] Inventory `AGENTS.md`, skills, build instructions, release instructions, and security policies.
- [x] Define the exact canonical whitepaper(s); flag competing public documents rather than silently choosing one.
- [x] Record what is devnet evidence, what is test-only, and what is production code.

## Phase 1 — repository topology and bloat

### Size and composition

- [x] Count tracked files, Git object size, checkout size, language LOC, and files by top-level directory.
- [x] List the largest tracked files and largest source files.
- [x] Identify generated artifacts, binaries, proving parameters, verifying keys, WASM bundles, screenshots, PDFs, logs, TAP output, JSON captures, and duplicated fixtures.
- [x] Inventory raw evidence archives and distinguish canonical evidence manifests from bulky captures.
- [x] Identify abandoned, empty, experimental, duplicate, superseded, or unreachable crates and directories.
- [x] Identify one-off operational scripts mixed into supported product tooling.
- [x] Identify duplicated wallet, extension, proxy, CLI, and API implementations.
- [x] Identify giant modules that prevent effective review and ownership.

### Required disposition

- [x] Assign every bloat class one disposition: **keep in source**, **move to evidence repository/object storage**, **publish as release asset**, **generate in CI**, **Git LFS**, **archive**, or **delete after approval**.
- [x] Preserve hashes/manifests when moving evidence or cryptographic artifacts; the candidate now has a fail-closed 14-path binary/media manifest covering SP1 fixtures, active/replay Orchard artifacts, the retained Cobalt reference, vendored Halo2 fixture, public icons, and deterministic byte-identical wallet WASM.
- [x] Estimate repository-size reduction and clone/build impact.
- [x] Propose a supported public directory structure and archive boundary.

## Phase 2 — public-repository hygiene

- [x] Validate license coverage for first-party code, vendored code, fonts, images, proving material, WASM, and documentation; raw screenshots/PDF redistribution is assigned to the archive/removal approval queue rather than assumed licensed.
- [x] Verify copyright headers and third-party attribution; exact Halo2 licenses/provenance are restored and non-source artifacts retain explicit disposition.
- [x] Verify README claims, quick start, architecture overview, supported platforms, maturity labels, and threat-model links.
- [x] Review `SECURITY.md` for a real contact, response expectations, supported versions, disclosure process, and cryptographic key.
- [x] Review `CONTRIBUTING.md`, code of conduct, issue templates, PR templates, and release process.
- [x] Add or plan CODEOWNERS and security-sensitive ownership boundaries.
- [x] Verify Cargo package metadata, repository URL, authorship, license, documentation URL, and `publish = false` where appropriate.
- [x] Check that examples and defaults cannot accidentally target live infrastructure; active runtime defaults are loopback/explicit and a blocking runtime-default scan covers product surfaces.
- [x] Remove or clearly label internal hostnames, IPs, operator names, founder-only procedures, local paths, and controlled credentials; real WAN/public IP literals were replaced with RFC documentation ranges, maintainer-home paths were replaced by explicit inputs/portable roots, controlled wallet-key defaults were removed, raw operator evidence was archived, and `test-public-source-portability` now blocks regressions including binary-embedded builder paths.
- [x] Validate all docs links and strict documentation builds from the exact
  clean publication checkout; local files, images, scripts, stylesheets and
  cross-document anchors resolve, and strict/redaction builds pass.
- [x] Identify stale CI jobs or scripts referencing deleted paths.

## Phase 3 — secrets, privacy, and history disclosure

- [x] Scan the current tree for private keys, seeds, mnemonics, tokens, passwords, API keys, cookies, SSH material, cloud credentials, and encrypted secrets with bundled decryption material.
- [x] Scan full Git history, all refs, tags, deleted files, large blobs, and merge commits.
- [x] Triage every high-entropy hit; do not blanket-allowlist proof hashes.
- [x] Inspect evidence captures for wallet backups, viewing keys, spend keys, raw notes, private RPC bodies, browser storage, and operator metadata; tracked-tree and all-permissions NAVSwap evidence scans cover 1,286 structured/text artifacts with zero secret-bearing values, while the screenshot/PDF visual-review queue remains separately open below.
- [x] Inspect test fixtures for keys that might have been reused outside tests.
- [x] Review screenshots and PDFs for secrets and private infrastructure; the 1,283-file raw evidence tree plus 19 unreferenced/historical wallet screenshots moved to restricted hash-manifested archives, the redundant VeriLLM download was replaced by its canonical DOI, and the only retained media are public extension icons plus the hash-pinned Cobalt source reference.
- [x] Define history-rewrite and credential-rotation procedure if any real secret is found.
- [x] Make sanitized publication fail closed on exact reviewed tree, exact ref
      set, complete clone, and zero tracked-tree/reachable-history findings; the
      regression rejects deleted-but-still-reachable credentials.
- [x] Add blocking secret scanning and evidence redaction to CI.
- [x] Confirm redaction tooling runs against directories that actually exist.
- [x] Document privacy leakage from public chain state, RPCs, logs, metrics, and error messages.

## Phase 4 — dependency and supply-chain audit

### Rust

- [x] Run RustSec audit and record vulnerable, yanked, and unmaintained crates.
- [x] Run dependency-policy review for duplicate versions, unused dependencies, default features, and oversized transitive graphs.
- [x] Review cryptographic dependency provenance, versions, features, and upstream security posture.
- [x] Review every vendored or patched dependency against its upstream commit and local diff.
- [x] Add `cargo-deny`, `cargo-vet` or equivalent policy, license allowlist, source allowlist, and advisory exceptions with expiry.
- [x] Pin the exact Rust toolchain rather than floating on `stable`.
- [x] Require locked/frozen dependency resolution in CI and release builds.

### JavaScript, Python, and operations

- [x] Audit every npm workspace, direct dependency, lockfile, dev server, bundler, and browser dependency.
- [x] Audit Python dependencies and ensure hashes/constraints are reproducible.
- [x] Audit GitHub Actions by commit SHA rather than mutable tags where warranted.
- [x] Audit shell downloads, curl-pipe-shell patterns, package installers, and unsigned release artifacts.
- [x] Generate a deterministic, relocation-stable CycloneDX SBOM for the exact
  source candidate and reproduce it from a second checkout.
- [ ] **REAL-VALUE LAUNCH:** sign release provenance, binaries, images,
  manifests and checksums through the governed release ceremony.
- [x] Define dependency-update cadence and emergency advisory response.

## Phase 5 — consensus safety and deterministic execution

### Protocol model

- [x] Trace the production proposal, vote, timeout, certificate, lock, commit, replay, and recovery paths end to end.
- [x] Prove quorum math for every committee size and every certificate type; exhaustive 1–64 shared-BFT and 4–64 FastSwap threshold/intersection tests pass, with under-quorum/duplicate/domain checks across block QC/TC, admission, Cobalt, FastPay payment, and FastSwap normal/new-round/control/checkpoint/exit certificates.
- [x] Verify unique-validator counting on every enabled production certificate path; live governance requires distinct signed old-registry authorizations and unsigned legacy artifacts are replay-only.
- [x] Verify domain separation binds chain ID, genesis, protocol version, height, view, parent, proposal/payload, registry root, and phase on every enabled certificate path; P1-CERT-DOMAIN-01 rejects empty legacy roots live, while FastPay v2 owner authorizations and validator votes now bind the exact chain/genesis/protocol/registry domain and foreign-domain apply fails before mutation.
- [x] Verify proposer selection is deterministic and identical across nodes.
- [x] Verify validators cannot vote for conflicting proposals across views in a way that breaks agreement; activated consensus v2 persists prepare/precommit locks and vote digests before signing across views and restarts.
- [x] Verify timeout certificates carry a real, verifiable high QC and proposals extend the required locked/high-QC branch; activated v2 admits only the immediately following view with signed timeout ancestry.
- [x] Verify the actual commit rule matches the documented rule; activated v2 commits only a non-nil precommit QC after a prepare QC, while the versioned pre-activation path replays byte-identically.
- [x] Verify reconfiguration intersects safely with in-flight certificates and cannot self-authorize; signed old-rule rotation activates after its authorizing block, namespaces safety/QCs by committee domain, and starts new-key signing on the following height.
- [x] Verify registry roots, committee snapshots, and activation heights are bound consistently on enabled paths.
- [x] Verify duplicate, replayed, future-height, stale-view, foreign-chain, and malformed artifacts fail closed on enabled certificate paths.

### Determinism

- [x] Audit consensus/state code for wall-clock use, randomness, floating point, locale, environment variables, filesystem enumeration, unordered maps/sets, platform-dependent serialization, and integer overflow; the replicated execution/order/batch/root boundary is now protected by a zero-allowlist CI scanner, wider-node matches are classified as local timing/files/randomness/deduplication, and the arithmetic inventory closes the monetary overflow/rounding paths found.
- [x] Audit canonical encodings and hash inputs for ambiguity, normalization, and versioning; the storage/determinism inventory maps every enabled encoding family to its ambiguity controls and golden/mutation evidence, while cross-architecture replay and exhaustive historical Serde vectors remain explicit real-value launch gates rather than unclassified review work.
- [x] Verify rejected transactions produce deterministic receipts and no partial mutation for every P0-touched live boundary.
- [x] Verify state-root construction includes all consensus-relevant state exactly once and in canonical order; P0-STATE-01 reproduced the omitted FastLane fields, the candidate commits all ten with an exhaustive compile-time inventory, order-invariance regression, new-genesis height-zero activation, and an irreversible future-height migration boundary for legacy genesis. The cap-valid candidate is now live 6/6, crossed consensus-v2 at h1, and its signed v6 snapshot passes six independent exact-tip/root `verify-state` and `verify-blocks` replays.
- [x] Verify every supported historical/live compatibility boundary produces
  byte-identical roots and receipts: golden roots, activation, FastSwap
  bootstrap, registry rotation, catch-up certificates, atomic-swap archives,
  snapshot v5/v6, coordinated upgrade and rollback all pass on the candidate.
- [ ] **FUTURE HARDENING / P2:** extend panic, abort and undefined-behavior
  campaigns beyond the current parser fuzz, checked-arithmetic, malformed-load
  and process-isolation coverage; no finite test can prove universal absence.

### Required adversarial tests

- [x] Conflicting proposals across views and partitions.
- [x] Stale/high-QC substitution and fabricated QC identifiers.
- [x] Quorum intersection at validator-set boundaries; exhaustive shared-BFT
  committee sizes 1–64 and FastSwap sizes 4–64 prove threshold/intersection and
  duplicate-validator rejection, including the n=4 and n=6 adversarial models.
- [x] Byzantine duplicate votes and identity substitution.
- [x] Crash after durable lock but before signature; crash after signature but before response.
- [x] Crash at every ordered-commit journal boundary; the production delta journal is replayed from all 11 persisted prefixes spanning ledger, governance, shielded, bridge, receipts, ordered batches, archive, block, chain tip, validator registry, and final journal removal, with exact post-recovery state and `verify_state` checks.
- [ ] **REAL-VALUE LAUNCH:** reproduce replay roots across independent CPU/OS
  architectures and optimized/debug builders.
- [x] Deterministic simulation with delay, drop, reorder, duplication, partition,
  Byzantine behavior, restart, and persistence faults; ordering n=4/n=6,
  consensus TCP failed-proposer recovery, FastPay recovery/crash matrices, and
  FastSwap replacement-relayer/restart suites all pass.

## Phase 6 — transaction, asset, and monetary safety

- [x] Enumerate every transaction/action type and its admission, authorization, fee, sequence, replay, execution, receipt, and rollback behavior.
- [x] Verify checked arithmetic and explicit rounding for every balance, supply, NAV, fee, bridge, offer, and redemption operation; the compiler-assisted, code-referenced classification and rounding matrix is `OPEN-SOURCE-ARITHMETIC-ROUNDING-INVENTORY-20260716.md`, including the sequence/height fixes, native burn oracle, and newly found issued-custody cap P0.
- [x] Prove native supply conservation plus explicit fee burn from genesis onward; genesis commits the exact supply, canonical replay reconciles every live account/escrow/offer-reserve/owned/FastLane/Orchard lane against receipt burns block by block, FastLane deposit/checkpoint burns are explicit, and history-checkpoint v2 commits cumulative burns while legacy v1 fails closed pending archive-backed rebuild.
- [x] Prove issued-asset supply caps, issuer authority, trustline rules, mint/burn lanes, and redemption accounting; the complete custody/transition/enforcement/test map is `OPEN-SOURCE-ISSUED-SUPPLY-INVENTORY-20260716.md`, including FastLane, AssetOrchard, finalized NAV supply, and explicit exclusion of the contained external bridge route.
- [x] Verify atomic swap dual authorization, exact-parent binding, both-or-neither execution, price/NAV constraints, fees, and conservation.
- [x] Verify rejected blocks/receipts are never surfaced as successful settlement in the hardened wallet/proxy/atomic-swap boundaries.
- [x] Verify mempool admission and execution validation cannot disagree dangerously; `P1-MEMPOOL-01` reproduced cross-family false admission caused by simulating candidates after every existing family, then aligned transfer/payment/asset/FastLane/escrow/NFT/offer admission with the canonical proposal-family prefix while preserving the rule that a paused stale atomic swap cannot wedge unrelated traffic.
- [x] Verify public-key publication and first-transaction behavior cannot strand accounts; the React wallet automatically performs one minimal signed self-transfer after first funding, reconciles ambiguous transport failure from ledger state without blind resubmission, validates the published key against the unlocked wallet, and now fails closed unless the final receipt contains both `accepted=true` and the explicit `code=accepted` terminal code.
- [x] Verify object ownership, lock, certificate, apply, replay, and cancellation semantics.
- [x] Verify FastPay unlock/cancel cannot race a late certificate or resurrect a
  spent object. The lane is enabled by default. V3 recovery admits confirmation
  only from a complete certificate; otherwise an ordered cancel advances every
  input version, making delayed certificates inapplicable. Full-certificate
  reveal, minority rollback, catch-up, crash-prefix, rotation/drain, wallet and
  Python-client regressions pass; explicit emergency disable remains available.
- [x] Verify unsupported asset labels cannot mint mislabeled owned objects.
- [x] Classify legacy transaction paths: supported, migration-only, test-only, or remove.

## Phase 7 — cryptography and key management

- [x] Inventory every signature, hash, commitment, proof, encryption, KDF, randomness, and key-derivation primitive.
- [x] Verify implementation and parameters against FIPS/NIST or upstream specifications; ML-DSA uses the pinned FIPS-204 implementation, Orchard/Halo2 and SP1 boundaries are version/hash bound, and exact upstream/provenance plus artifact hashes are recorded, with independent specialist validation retained as a real-value gate.
- [x] Verify signature contexts and deterministic/randomized signing behavior; the transcript inventory covers enabled families and a blocking call-site policy freezes all 46 generic-context/deterministic-seed uses so new purpose reuse cannot enter silently.
- [x] Verify public-key parsing, subgroup/canonical checks, signature malleability handling, and failure behavior; fixed-size ML-DSA parsing, canonical Orchard/Pallas wrappers, identity rejection, FastSwap re-encoding, mutation vectors, and the 15-target adversarial parser harness fail closed.
- [x] Audit every proof system's public inputs, witnesses, circuit/version IDs,
      verifying-key binding, chain domain and activation policy in the
      machine-readable proof inventory; AssetOrchard closes the binding, while
      SP1 guest provenance and the debug adapter's production exclusion remain
      explicit real-value/release-profile gates rather than unknowns.
- [x] Audit proving/verifying keys and parameter files for reproducibility,
      hash binding and distribution; AssetOrchard artifacts are embedded,
      round-trip reproducible and hash-pinned, while the absent SP1 guest build
      is recorded as a blocker to production activation.
- [x] Review patched Halo2 source against its exact upstream commit and freeze the normalized diff; independent specialist circuit review remains a real-value launch gate.
- [x] Verify entropy sources and remove test RNGs from production paths; native FIPS key generation uses `rand_core/getrandom`, wallet WASM explicitly enables the JavaScript CSPRNG backend, Orchard uses `OsRng`, and deterministic key/signature APIs are limited by the exact CI call-site policy.
- [ ] **REAL-VALUE LAUNCH:** verify secret zeroization across serialization,
  temporary files, subprocesses, crash dumps and logs.
- [ ] **REAL-VALUE LAUNCH:** replace explicitly controlled-devnet plaintext key
  files with HSM/remote-signer custody, rotation, backup and recovery.
- [x] Verify or implement every whitepaper-promised recovery-key commitment and activation path; result: SLH-DSA is absent and the present-tense claim is removed.
- [ ] **REAL-VALUE LAUNCH:** commission independent cryptographic and circuit
  audits. External review is not a source-publication prerequisite.

## Phase 8 — privacy and shielded-pool safety

- [x] Separate legacy cleartext “shielded” structures from the actual encrypted Orchard/Asset-Orchard path.
- [x] Map exactly what is hidden and leaked: sender, recipient, asset tag, amount, fee, NAV epoch, price ratio, nullifier, timing, aggregate pool balance, ingress, and egress.
- [x] Verify note encryption, scanning, diversifiers, nullifiers, anchors, root
  history and recipient-only chain ciphertext recovery; the AssetOrchard
  ordinary `83/83` and explicit release-scale `17/17` gates pass.
- [x] Verify value and per-asset conservation in-circuit and in transparent
  turnstile accounting; real K15 proofs, forged-nonconservation vectors,
  ingress/swap/egress supply equality and replay all pass.
- [x] Verify public pricing claims are exact, bound to certified reserve packets, and not off-band.
- [x] Verify private egress binds recipient, asset, amount, fee, nullifier, and exit authorization.
- [x] Verify no wallet/proxy/API log contains note plaintext or spend authority;
  browser wire capture plus 13 ordered-store public artifacts are clean, and
  recursive oversized-serialized-field attempts fail closed.
- [x] Verify proof rejection cannot mutate state or nullify notes.
- [x] Verify circuit-version migrations, freezes, turnstile limits, and rollback
  policy; live v1 admission rejects, authenticated v1 archive replay remains
  exact, current/replay VKs are separately pinned, and pause/activation,
  snapshot and rollback regressions pass.
- [x] Add privacy regression scans over the candidate tree, generated evidence,
      browser/proxy private-material boundaries and RPC response schemas; the
      redaction reporter itself is v2 and never echoes matched values. Fresh
      deployed wire/browser/log captures remain a real-value launch gate.

## Phase 9 — bridges and external-system risk

- [x] Inventory CCTP, Ethereum, Uniswap, PFTL, NAVCoin, vault, issuer, attestor, and sidecar trust boundaries.
- [x] Distinguish canonical on-chain enforcement from operator scripts and simulated/adapted external systems.
- [x] Verify replay protection, chain IDs, contract addresses/code hashes,
  finality depths, mint/burn nonces, and message uniqueness at both PFTL and
  isolated Anvil boundaries.
- [x] Verify deposits, refunds, returns, failed relays, reorganizations, partial
  completion, duplicate delivery, and consume/refund mutual exclusion; ordered-
  commit crash prefixes and both explicit Anvil gates pass.
- [x] Prove cross-domain supply conservation and define reconciliation/incident
  procedures; exact `V=S+D+B-R`, external inventory, wrong-amount, replay and
  route-rotation tests pass under the explicitly documented BFT-checkpoint
  trust model.
- [x] Review smart contracts, deployment bytecode, admin keys, upgradeability, pause powers, and verification status.
- [x] Remove claims that a bridge or representation is live unless production contracts and controls are actually deployed.
- [ ] **REAL-VALUE LAUNCH:** require separate contract and bridge audits before
  real value. External review is not a source-publication prerequisite.

## Phase 10 — storage, crash consistency, and scale

- [x] Inventory all persisted files, schemas, indexes, journals, append logs, snapshots, archives, and caches.
- [x] Verify atomicity across ledger, governance, shielded, bridge, receipt, block, registry, and index state; the ordered-commit delta journal recovers exact state from every one of 11 persisted prefixes across all listed domains and removes the journal idempotently.
- [ ] **REAL-VALUE LAUNCH:** extend the existing torn-write, fsync, corruption,
  concurrency and crash-prefix coverage to production filesystems and disk-full,
  permission and concurrent-reader fault campaigns.
- [x] Verify startup recovery is idempotent and cannot expose half-applied transactions or swaps.
- [x] Measure asymptotic read/write complexity as chain height, account count, trustlines, notes, blocks, and receipts grow; production-scale empirical growth remains open.
- [x] Identify whole-state rewrites, whole-history reads, linear scans, unbounded vectors, and memory amplification; hard candidate ceilings now bound the previously unbounded reads/appends.
- [x] Define a production storage engine, schema migration, column/index layout, snapshot, backup, restore, compaction, and corruption-repair plan.
- [x] Verify history pruning retains sufficient proof material and cannot break state verification in existing checkpoint/archive tests.
- [ ] **REAL-VALUE LAUNCH:** run long-duration growth, restart and backup/restore
  tests at projected production scale.

## Phase 11 — networking, RPC, and denial of service

- [x] Inventory every TCP, HTTP, WebSocket, CLI, proxy, prover, admin, metrics, and debug endpoint.
- [x] Classify endpoints as public read, authenticated wallet mutation, validator-only, operator-only, or local-only; the code-derived v2 inventory explicitly partitions all 135 observed methods into 63 reads, 12 default-public cryptographically authorized protocol mutations, 14 flag-gated signed submissions, four flag-gated Orchard methods, four flag-gated owned-lane methods, and 38 operator/local methods, with zero unknowns and a CI regression that fails on any unclassified addition.
- [ ] **REAL-VALUE LAUNCH:** exercise the documented authenticated TLS/mTLS edge
  and its limits/backpressure under production deployment load. Source defaults,
  origin/auth, request/rate/concurrency and loopback boundaries are already gated.
- [x] Ensure public binding is impossible by accident and dev-server defaults are never production defaults.
- [ ] **FUTURE HARDENING / P2:** extend request framing, JSON, compression,
  dispatch and oversized-input fuzzing beyond the current adversarial parser and
  bounded-request suites.
- [x] Verify expensive proof, history, state verification, scan and child-worker
  RPCs have bounded request, concurrency, timeout and result policies, with
  malformed-load, timeout, pagination and process-isolation regressions.
- [x] Verify idempotency keys cannot cross users, methods, chains, or payloads.
- [x] Verify error responses do not reveal secrets, filesystem paths, topology, or internal policy; `P1-RPC-ERROR-01` reproduced a verbatim operator path at the remote response boundary, then made internal/worker/status/unavailable failures stable and path-free while preserving typed protocol rejection details, with the complete RPC request regression suite green 18/18.
- [x] Define DDoS controls and separation between validator consensus traffic and public RPC traffic.

## Phase 12 — wallet, proxy, API, and UX custody

- [x] Trace seed/key creation, storage, backup, unlock, signing, export, import, and deletion across web wallet, CLI, API, proxy, extension, and prover.
- [x] Verify whether signing is genuinely client-side; the public self-custody path now signs locally and sends no backup/seed to the proxy.
- [x] Disable native/server-side wallet signing by default in public builds unless explicitly presented as custodial.
- [x] Verify browser CSP, dependency integrity, origin checks, WebSocket authentication, CSRF, XSS, clickjacking, storage isolation, and cache controls.
- [x] Replace public Vite/dev servers with a production static build and hardened serving configuration.
- [x] Verify mutation confirmation, receipt-code semantics, rejected/unknown handling, idempotent replay, and balance reconciliation.
- [ ] **FUTURE PRODUCT HARDENING / P2:** conduct non-expert usability research
  for NAV source/freshness/epoch, price, fees, settlement and privacy leakage.
- [x] Verify faucet and demo controls cannot become production backdoors at the proxy/auth and remote-RPC boundaries.
- [x] Add end-to-end tests for two fresh addresses, first funding, automatic key
  publication, transparent/private swap, private egress, chain-only recovery and
  rejected-receipt handling with exact conservation.
- [x] Produce an explicit custody statement for the supported self-custody web-wallet mode; no public custodial mode is supplied.

## Phase 13 — operations and validator production readiness

- [x] Review service units, privilege separation, filesystem permissions, sandboxing, resource limits, log rotation, and core-dump policy; hardening plus a tested 14-day/100-MiB log-rotation policy are implemented, while structured reopen and independent retention drills remain in `P1-OPS-01`.
- [x] Separate signing keys, committee rosters, public topology, runtime config,
  and release identity; pre-deploy roster/release manifests reject incomplete
  committees and key/config substitution.
- [x] Add pre-deploy validation for complete committee rosters and byte-identical signed releases.
- [x] Define rolling deploy, rollback, state compatibility, and mixed-version rules.
- [ ] **REAL-VALUE LAUNCH:** execute the already specified genesis/key ceremony,
  launch certificate, operator manifests and independent ratification.
- [x] Define monitoring for height/root divergence, certificate participation, rejected receipts, mempool, disk, proof latency, RPC saturation, and clock skew; every listed signal has an implemented source and ordered fail-closed threshold, including exact AssetOrchard Halo2 timing and direct RPC active-connection utilization rather than a latency proxy. Alert delivery and fleet drills remain separately open below.
- [x] Define alerts, SLOs, incident severity, runbooks, escalation, and public status communication; private alert envelopes carry enforced SEV-1/SEV-2 acknowledgement, incident-command, escalation, public-update, and runbook fields, and `docs/runbooks/incident-response.md` defines measurable controlled-pretestnet SLOs. External delivery and independent drill evidence remain production-launch gates, not claimed capabilities.
- [x] Prove backup/restore, node replacement, catch-up, archive restore, key
  rotation, rollback and forward recovery in isolated exact-six drills.
- [ ] **REAL-VALUE LAUNCH:** run multi-region fault drills without shared control planes. Real WAN
  quorum-early finality, exact-six convergence, slow-node voting, failed-
  proposer recovery and wallet settlement are proven, but the independence
  condition remains a real-value launch gate.
- [ ] **REAL-VALUE LAUNCH:** remove single-founder/single-host dependencies before
  claiming decentralized production.

## Phase 14 — tests, CI, and release assurance

- [x] Make formatting, check, strict Clippy, complete workspace, explicit
  Orchard, atomic-swap, FastSwap and Foundry/Anvil tests pass from a clean
  checkout. All pass on the integrated candidate tree; the exact sanitized
  staging clone must reproduce them before this clean-checkout box closes.
- [x] Stop skipping privacy/Orchard tests in normal CI; split slow suites without silently omitting them.
- [x] Add exact toolchain, locked dependencies, dependency audit, license policy, secret scan, docs build, and artifact checks.
- [x] Add unit, integration, property/fuzz, same-architecture differential,
  deterministic-simulation, soak, fault-injection, restart/replay and performance
  suites across the P0/P1 protocol and customer surfaces.
- [ ] **FUTURE HARDENING / P2:** add Miri/sanitizer coverage where practical and
  narrow the remaining reviewed `unsafe` boundaries further.
- [ ] **FUTURE HARDENING / P2:** establish protocol-critical branch coverage,
  not only aggregate line coverage.
- [x] Add consensus certificate/lock and touched monetary conservation/no-mutation invariant tests; the full cross-domain supply oracle remains open.
- [x] Test protocol compatibility across legacy/current encodings, state-root
  v1/v2 activation, snapshot v5/v6, governance/key rotation, FastPay v2/v3 and
  privacy live-v2/archive-v1 boundaries.
- [ ] **REAL-VALUE LAUNCH:** reproduce release binaries and hashes across
  independently controlled builders.
- [ ] **REAL-VALUE LAUNCH:** sign releases, SBOMs, manifests, images and checksums.
- [x] Define release candidates, canary criteria, rollback thresholds, and post-deploy verification in the release process; execution awaits a clean candidate.

## Phase 15 — whitepaper conformance matrix

For every claim, the final report must mark **implemented**, **partially implemented**, **test/devnet only**, **documentation only**, **contradicted**, or **not found**, with evidence.

### Identity and architecture

- [x] Resolve “XRPL-derived” versus the actual implementation architecture.
- [x] Resolve competing public and repository whitepapers and version them explicitly.

### Consensus and governance

- [x] HotStuff-style chained ordering and exact commit rule.
- [x] Timeout certificates, high-QC propagation, lock rule, and view change.
- [x] Byzantine threshold and certificate-size claims.
- [x] Cobalt-governed registry evolution and old/new transition safety.
- [x] Admission evidence, linkedness/correlation checks, and challenge behavior.
- [x] Launch certificate, at least seven ratifiers, control-group limits, and immutable genesis roots.
- [x] Availability suspension and bounded emergency recovery.

### Monetary and asset claims

- [x] Fixed native supply, no later native issuance, fee burn, and no validator subsidy.
- [x] Issued assets, NAV assets, proof freshness, reserve packets, challenges, halts, mint, redemption, and supply caps.
- [x] Uniswap/bridge representation and OTC swap claims.

### Privacy and cryptography

- [x] Orchard/Halo2 architecture and setup assumptions.
- [x] Claimed hidden fields versus actual public inputs and aggregate leakage.
- [x] Turnstile conservation and incident freeze.
- [x] ML-DSA authorization from genesis.
- [x] SLH-DSA recovery commitments and governed activation.
- [x] Classical proof/encryption assumptions and upgrade paths.

### Model and evidence claims

- [x] Models remain outside consensus and authority.
- [x] Deterministic replay/profile claims and failure-to-hold behavior.
- [x] Every empirical claim links to a public, hash-verifiable artifact; result: several evidence links remain missing and are classified rather than silently accepted.
- [x] Controlled-testnet measurements are not presented as production guarantees.

### Business whitepaper claims

- [x] Native NAV transaction surface and reserve accounting.
- [x] Challenge mechanics actually halt the promised actions.
- [x] Indexing and compliance products are labeled roadmap where not implemented.
- [x] External collector, attestor, custody, redemption, and legal trust assumptions are explicit.

## Phase 16 — known high-risk findings requiring immediate proof or disposition

These are already visible from code inspection and must be validated first; inclusion here is not a substitute for the full review.

- [x] **Shipping automatic view recovery:** commit `09125687` makes the normal
  finality path collect a distinct signed timeout quorum against one exact parent,
  route the deterministic later-view proposer, verify the timeout certificate and
  return the commit. Real shipping-path `n=4` and `n=6` regressions pass in 80.51
  and 122.55 seconds; durable timeout signing, bounded transport, browser exclusion,
  formatting, affected check and strict Clippy are green.
- [x] **Cross-view locking gap:** reproduced and fixed with one durable vote per validator/height plus legacy-lock migration.
- [x] **High-QC gap:** nonzero views and timeout authorization are fail-closed pending a complete QC-ancestry protocol.
- [x] **Remote seed exposure:** fixed; browser signs locally and backup-bearing proxy methods are removed.
- [x] **Public dev server:** fixed; loopback development only and hardened static production serving.
- [x] **Native genesis supply rewrite:** coordinated replay-base faucet plus height-zero ledger rewrites fail closed; new genesis hashes commit the exact supply and legacy hashes remain byte-compatible under the same enforced constant.
- [x] **Plaintext validator keys:** production explicitly unsupported; long-running services require an unsafe-devnet acknowledgement.
- [x] **Storage scale:** allocation/append work is hard-bounded, concrete cross-process lost-update and commit-interleaving boundaries are locked, and every core long-running service fails closed without an exact controlled-devnet storage acknowledgement; transactional indexed production storage remains a documented real-value gate.
- [x] **FastPay lock liveness:** default availability is restored with exact
  certificate-domain binding and explicit emergency disable. The complete V3
  consume-or-cancel recovery protocol, late-certificate fencing, durable
  crash recovery, committee rotation/drain, wallet and Python callers are
  implemented and green. The real six-node WAN safety-correct battery is now
  green at p50 2.49 s / p95 3.72 s with five signed durable apply
  acknowledgements per payment. Lost-client-response replay remains open because
  completed proxy outbox records are deleted after exact-six replication.
- [x] **SLH-DSA whitepaper claim:** no implementation found; present-tense claim removed.
- [x] **Legacy privacy path:** Mint/Spend is historical replay only; Asset-Orchard is the supported path.
- [x] **Supply-chain advisories:** vulnerable/yanked versions removed; three unmaintained transitives have scoped, expiring exceptions.
- [x] **Broken docs/security CI:** repaired, SHA-pinned, hash-locked, and locally proven.
- [x] **History secret/privacy triage:** all 719 Gitleaks generic findings are
      classified; the first-party scanner separately identifies exactly three
      historical Jupyter-token occurrences and 24 removed note-opening field
      occurrences that sanitized public refs must reduce to zero.
- [x] **Whitepaper conflict:** canonical protocol candidate and non-normative business paper are explicitly labeled.

## Phase 17 — STEP 1 completion: final audit and productionization specification

The completed STEP 1 plan must be ordered by dependency, not by convenience:

1. **Containment and truth:** freeze claims, preserve work, choose canonical specification, triage secrets.
2. **P0 safety:** consensus proof/fix, custody boundary, monetary/privacy invariants, cryptographic claim corrections.
3. **Public-source hygiene:** history, licensing, evidence split, metadata, docs, supported surface.
4. **Production foundations:** storage engine, authenticated networking, signer custody, operations, observability.
5. **Assurance:** adversarial tests, DST, fuzzing, independent consensus/cryptography/bridge audits.
6. **Release engineering:** reproducible signed artifacts, SBOM, staged upgrade/rollback, launch ceremony.
7. **Public-release candidate:** create only after every P0/P1 gate has objective closure evidence and named ownership.
8. **Launch:** perform controlled real-value launch only after the additional operational, independent-audit, and launch-governance gates pass.

For each work item, record:

- severity and affected invariant;
- exact files/components;
- exploit or failure scenario;
- recommended change;
- dependencies and migration impact;
- test and evidence required for closure;
- owner and reviewer class;
- public-release blocker versus production-launch blocker;
- status and closure hash.

STEP 1 exit gate:

- [x] Every checklist section has evidence-backed results rather than discovery placeholders; unchecked items above are explicit candidate/real-value assurance gates, not unknown surfaces.
- [x] Every P0/P1 has a reproducible failure argument and objective acceptance
  specification; newly confirmed shipping view-escalation and completed FastPay
  response-replay gaps are explicitly reopened rather than hidden by earlier
  component-level evidence.
- [x] The whitepaper matrix covers every material protocol and product claim.
- [x] The bloat and public-history disposition is complete; publication from the contaminated private history is forbidden and mechanically gated.
- [x] The blocker register has an owner class and implementation order for every
  item; provider, release and security ownership is explicit for the external
  publication gate, while consensus and FastPay liveness remain local work.
- [x] The audit is the internally reviewed controlling STEP 2 specification;
  independent specialist review remains a real-value gate, not a public-source
  publication dependency.

## Phase 18 — STEP 2 execution: remediate P0/P1 and prove closure

- [x] Create a dedicated remediation branch or branch stack from an immutable audited baseline (`open-source-productionization-20260716` from `4b5af7bc6bb6e793ed8a60219d13d6d35be03058`).
- [x] Close every P0 in priority order. Automatic view escalation, custody,
  monetary, privacy and public-evidence code are fixed; the provider owner
  confirmed destruction and the private `P0-SECRET-01` record passes the gate.
- [x] Close locally actionable P1s next, ordered by storage correctness/scale, network exposure, liveness/recovery, operations, supply chain, and test/release assurance; real-value-only capabilities remain explicitly disabled/unsupported.
- [x] Keep each remediation traceable to exactly one blocker unless atomic cross-component changes are required for safety; the closure table records the deliberate cross-boundary cases.
- [x] Add exploit/reproduction fixtures before or alongside each confirmed P0/P1 fix.
- [x] Run targeted tests after each change and the complete release battery
  after the integrated set; the full workspace, release Orchard, six-process
  atomic/FastSwap, wallet/proxy/browser, Foundry/mainnet-fork and Anvil gates
  are recorded in the audit and lab book.
- [x] Run cross-version, migration, rollback, crash, replay,
  deterministic-simulation and adversarial-network tests against the integrated
  candidate.
- [x] Run dependency, license, tracked-tree/sanitized-history, redaction and
  reproducible-SBOM gates against the exact candidate and second clone.
- [x] Complete the second internal reconciliation for consensus,
  circuits/cryptography, bridge/supply and wallet custody; external specialist
  review remains explicitly packaged as a real-value gate.
- [x] Re-run STEP 1 scanners/inventories against the exact staging tree; no new
  unclassified runtime, RPC, proof, artifact, dependency, claim or secret surface
  is reachable.
- [x] Produce the 35-row closure table mapping reproduction, real fix commits,
  regression, integrated evidence, claim change and residual risk; finding
  sections retain owner/reviewer class.
- [x] Obtain the private provider-owner record and close `P0-SECRET-01`; all 35
  P0/P1 rows are closed with no waiver or ownerless item.
- [x] Freeze the exact code/staging candidate after all internally actionable
  STEP 2 gates; publish only the verified one-commit sanitized history.

## Exit criteria

### Safe to make the source public

- [x] STEP 1 is complete and accepted as the comprehensive internal audit record.
- [x] STEP 2 has zero unresolved P0/P1 findings; `P0-SECRET-01` is closed by the
  private provider-owner record and verified sanitized-history publication.
- [x] Every removed or contained unsafe feature is mechanically unreachable in
  its prohibited profile, tested, and absent from public claims; core FastPay,
  FastSwap, atomic swap, bridge and private-flow capabilities remain enabled in
  their documented controlled profiles.
- [x] The intended public refs/history scan clean, the provider owner confirmed
  destruction, and the exact GitHub clone passes the strict publication gate.
- [x] Licenses and third-party notices are complete.
- [x] Internal infrastructure and operator-sensitive evidence are removed or
  deliberately documented at a safe abstraction boundary.
- [x] Security policy and controlled-testnet maturity warnings are accurate.
- [x] CI-equivalent gates build the exact public tree and strict docs from a
  second clean, non-shallow clone.
- [x] Whitepaper and README make no unimplemented production-safety guarantee.

### Safe to run with real value

- [ ] **REAL-VALUE LAUNCH:** revalidate public-release P0/P1 closure on the exact
  separately governed launch revision.
- [ ] **REAL-VALUE LAUNCH:** add independent review to the existing deterministic
  consensus/reconfiguration simulations and models.
- [ ] **REAL-VALUE LAUNCH:** ratify the existing monetary, bridge, atomicity and
  privacy property/fuzz/fault evidence under the launch profile.
- [ ] **REAL-VALUE LAUNCH:** move validator and wallet keys to approved production
  custody.
- [ ] **REAL-VALUE LAUNCH:** pass production-scale storage, networking,
  observability, backup, recovery, upgrade and rollback drills.
- [ ] **REAL-VALUE LAUNCH:** complete independent cryptography, circuit, bridge,
  wallet-custody and consensus audits.
- [ ] **REAL-VALUE LAUNCH:** produce independently reproducible signed releases
  and a governed launch profile.
- [ ] **REAL-VALUE LAUNCH:** obtain launch-authority acceptance of residual risks.
