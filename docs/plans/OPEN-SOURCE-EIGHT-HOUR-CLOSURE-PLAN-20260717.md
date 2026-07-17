# Open-Source Productionization: Eight-Hour Closure Sprint

**Date:** 2026-07-17
**Branch:** `open-source-productionization-20260716`
**Controlling plan:** `docs/plans/P0-COMPLETE-REMEDIATION-PLAN-20260716.md`
**Finding/evidence register:** `docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md`
**Objective:** finish every internally actionable P0/P1, freeze one exact source-publication candidate, and prove that candidate through the complete release gates within eight hours.

> **Scope truth:** eight hours can close the controlled-testnet/source-publication
> phase. It cannot honestly complete independent specialist audits, production HSM
> deployment, multi-region independent-control drills, or long-duration production
> scale soaks. Those remain fail-closed **real-value launch gates** and must not be
> represented as complete. The sole known external blocker to public source
> publication is the private provider-owner record proving the captured credential
> was revoked or decommissioned. No engineer may fabricate or waive that record.

## 1. Eight-hour definition of done

The sprint is complete only when all of the following are true:

- [x] `P0-CONSENSUS-01` shipping automatic view recovery is reviewed, committed,
  and passes the real four- and six-validator product-path regressions.
- [x] `P1-FASTPAY-01` completed-response replay has a bounded durable tombstone,
  real lost-response/restart/compaction/conflict regressions, and a separable commit.
- [x] No other internally actionable P0/P1 is open in the closure table.
- [x] One clean commit and tree are frozen as the release candidate; no test is
  attached to a moving working tree.
- [x] Formatting, workspace check, strict Clippy, complete workspace tests,
  wallet/proxy tests and build, Foundry tests, docs/claims, dependency/provenance,
  deterministic SBOM, replay/recovery, deterministic n=4/n=6, isolated rolling
  upgrade, and fresh-wallet flow are green on that exact tree.
- [x] A clean, non-shallow, sanitized staging repository contains only the intended
  public refs and has zero tracked-tree or reachable-history secret findings.
- [x] A second clean clone reproduces the exact commit, tree, refs, file count,
  scanners, tests, SBOM and artifact hashes.
- [x] The closure table maps every P0/P1 to reproduction, fix commit, regression,
  integrated evidence, claim change, and residual risk with no ownerless row.
- [x] If the provider-owner revocation record exists, the strict publication gate
  passes. If it does not, the final state is **CODE AND STAGING COMPLETE / PUBLICATION
  BLOCKED ONLY ON P0-SECRET-01 PROVIDER RECORD**; the repository is not published.

This sprint does not deploy to a real-value network, rotate production keys, publish
the repository, or reset the shared devnet.

## 2. Current critical-path inventory

| Work | Current state | Remaining work | Budget |
|---|---|---|---:|
| P0 automatic view recovery | Closed in `09125687`; real n=4/n=6 shipping-path recovery, timeout durability and routing are green | None for source publication | complete |
| P1 FastPay response replay | Closed in `77e4a3c7`; bounded durable replay survives loss/restart/compaction/conflict | None for source publication | complete |
| Candidate freeze | Final closure tree is committed and identified in the external acceptance manifest | No source edits after freeze | complete |
| Complete candidate battery | Exact-tree complete workspace, release Orchard, FastSwap, atomic swap, Anvil, wallet/proxy, Foundry, docs, dependency and scanner gates pass | Real-value-only gates remain separately classified | complete |
| Publication staging | Exact one-commit public staging, second clone and GitHub clone have intended refs and zero current/history secret findings | None | complete |
| Provider credential record | Provider owner confirmed destruction; private mode-0600 record passes the verifier | None | complete |
| Closure reconciliation | All 35 rows name real commits/evidence/residual risk; every unchecked checklist item is classified | None internally actionable | complete |

## 3. Execution schedule

### T+00:00–00:30 — close the last internal P0

1. Review only the existing automatic view-recovery diff for:
   - exact activation/version boundary;
   - durable timeout-vote persistence before signing;
   - distinct-validator quorum counting;
   - deterministic later-view proposer routing;
   - bounded timeout/view escalation;
   - terminal success classification for an already-applied batch;
   - no weakening of receipt, state-root, or signature checks.
2. Run the focused four- and six-validator shipping finality regressions and the
   wallet-proxy proposer/auth regressions.
3. Commit the implementation as one separable P0 commit.
4. Update `P0-CONSENSUS-01` with command, result, commit and evidence path.

**Stop condition:** any conflicting commit, double-sign, non-durable vote, or
unbounded retry is a real P0 RED. Do not freeze a candidate around it.

### T+00:30–02:00 — close the last internal P1

1. First add a failing test at the real proxy/outbox boundary:
   - complete a certificate with at least `n-f` distinct signed apply acks;
   - lose the client response;
   - restart the proxy;
   - replay the byte-identical certificate;
   - require the same authenticated terminal response without reapplying money.
2. Implement a versioned bounded durable completed record containing:
   - certificate ID and full certificate digest;
   - chain/genesis/protocol/committee domain;
   - method and exact order/effect digest;
   - terminal result/receipt digest;
   - the distinct signed durable apply acknowledgements needed to reconstruct
     product finality;
   - creation/completion time and explicit retention version.
3. Persist atomically before acknowledging completion. Retain a strict count,
   byte and age bound; compaction must preserve the high-water/conflict boundary.
4. Prove:
   - identical replay before and after restart;
   - response loss at every persistence boundary;
   - duplicate acknowledgement deduplication by validator ID;
   - conflicting certificate/order/domain rejection;
   - tampered tombstone fail-closed behavior;
   - bounded compaction and deterministic recovery;
   - no second debit/apply on replay.
5. Run the FastPay proxy tests plus affected node/execution recovery tests, then
   commit this P1 separately and update the evidence register.

**Stop condition:** a replay that can mutate again, accept a conflicting digest,
or return finality without `n-f` valid distinct acknowledgements is a correctness RED.

### T+02:00–02:20 — freeze RC1

1. Finish evidence-only updates for the two commits.
2. Require `git diff --check`, format, affected tests, and a clean worktree.
3. Record:
   - `git rev-parse HEAD`;
   - `git rev-parse HEAD^{tree}`;
   - Rust/Node/npm/Forge versions;
   - lockfile, proof-source, scanner and release-script hashes.
4. Declare that exact commit/tree `RC1`. No source or test edits occur while its
   final battery runs. A genuine RED creates RC2; it is never patched in place.

### T+02:20–06:20 — run the immutable-candidate gates in parallel

Start the expensive complete workspace test immediately. Use separate output files
and preserve every exit code. Run only non-mutating checks concurrently.

**Lane A — longest Rust/proof gate**

```text
cargo test --workspace --all-targets --locked
```

This includes the long Orchard/proof cases and is run exactly once after source
freeze. Do not spend hours completing it on a tree that will subsequently change.

**Lane B — compiler, supply chain, docs and public-tree gates**

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo audit
cargo deny check
./scripts/generate-cargo-sbom /tmp/postfiat-sbom-1.cdx.json
./scripts/generate-cargo-sbom /tmp/postfiat-sbom-2.cdx.json
cmp /tmp/postfiat-sbom-1.cdx.json /tmp/postfiat-sbom-2.cdx.json
./scripts/docs-site-build                  # internally runs mkdocs build --strict
./scripts/docs-site-redaction-check
./scripts/public-secret-scan
```

Also run wallet-web and wallet-proxy test/build/audit gates, the proof-source/
public-input inventory verifier, license/provenance checks, link/claim-boundary
checks, and the publication-verifier regression suite.

**Lane C — protocol integration gates**

- deterministic/adversarial n=4 and n=6 consensus view recovery;
- FastPay recovery, committee rotation, crash matrix, replay tombstone and WAN
  product-response tests;
- activation replay, snapshot restore, lagging-node catch-up, crash-prefix and
  rollback-before-activation tests;
- isolated six-node rolling upgrade/activation drill;
- complete fresh-wallet flow with accepted receipt codes and conservation;
- offline Foundry plus pinned official-mainnet-fork suite.

The live shared devnet is not required for these gates. Use isolated temporary
directories, ports, keys and Anvil instances. No destructive reset is scheduled.

### T+06:20–07:20 — construct and independently reproduce public staging

1. Export only the exact reviewed RC tree and intended public refs into a fresh,
   non-shallow staging repository. Do not carry the contaminated private ref set.
2. Run tracked-tree and complete reachable-history scans. Both must report zero.
3. Run `scripts/verify-publication-candidate` with:
   - the exact reviewed tree;
   - an exact ref allowlist;
   - the private mode-0600 provider record outside the repository.
4. Clone the staging repository into a second clean location and repeat the exact
   verifier, build metadata, scanner and SBOM checks.
5. Record commit, tree, ref set, file count, scanner versions, SBOM hash, artifact
   hashes and both clone locations in the evidence manifest.

If the provider record is absent, steps 1, 2, 4 and the non-provider portions of
step 5 still run. Record the expected fail-closed verifier result; do not publish.

### T+07:20–08:00 — reconcile and seal

1. Rerun the STEP 1 inventory/scanner generators against the exact staging tree to
   detect newly reachable RPCs, transaction families, state fields, custody lanes,
   proof inputs, dependencies, claims, artifacts or secrets.
2. Update the closure table so every P0/P1 names its fix commit and exact-candidate
   evidence. Replace stale `working tree`/`global gates pending` labels.
3. Classify every remaining unchecked checklist line into exactly one category:
   - source-publication blocker;
   - real-value launch gate;
   - future hardening/P2.
4. Write the final acceptance manifest with hashes and command results.
5. State one of only two honest outcomes:
   - **SOURCE-PUBLICATION READY:** all internal gates and the provider record pass; or
   - **SOURCE-PUBLICATION BLOCKED ON ONE EXTERNAL ACTION:** code, staging and tests
     pass, but the named provider record is absent or invalid.

## 4. Failure and restart policy

- Correctness, safety, secret, history, replay, migration or conservation RED:
  stop candidate certification, preserve evidence, fix in a new commit, declare RC2,
  rerun the affected gate and the complete final battery.
- Test-environment failure with no product defect: preserve the raw result, correct
  only the harness in a separable commit, and repeat the affected gate.
- Performance-only RED outside an explicit acceptance threshold: record it; do not
  weaken a safety or correctness gate.
- Never delete or skip a test, loosen quorum, bypass receipt-code checks, accept an
  arbitrary ref/tree, or waive the provider record.
- Do not reset or mutate the shared devnet during this sprint. If a later deployment
  requires reset, batch all compatible protocol changes into one separately approved
  release operation with pre-reset snapshot and rollback evidence.

## 5. What remains after this eight-hour sprint

These are not hidden source-publication blockers. They are explicit prerequisites
before representing the system as safe for real customer value or decentralized
production:

- production HSM/remote-signer custody, rotation, backup and recovery;
- indexed transactional storage and production-scale growth/failure tests;
- authenticated production edge deployment and independent operations;
- specialist cryptography/circuit, consensus, bridge/contract and wallet audits;
- multi-region fault drills without shared control planes;
- reproducible independently built and signed binaries, images, manifests and SBOMs;
- governed launch ceremony and explicit residual-risk acceptance.

The public repository, README, whitepaper and security policy must continue to say
that these real-value gates are open. Source visibility is not a production-safety
claim.

## 6. Closure outcome

**Outcome:** **SOURCE-PUBLICATION READY AND PUBLISHED FROM SANITIZED HISTORY.**

The exact commit, tree, toolchains, command results, log hashes, SBOM hash,
artifact hashes, intended refs, file count and both staging-clone locations are
sealed in the release-owner
`open_source_publication_candidate_20260717/ACCEPTANCE.json` manifest outside
the public tree. The sanitized one-commit history is published at
`https://github.com/postfiatorg/postfiatl1v2`; the contaminated development
history remains private under a separately named archive repository.

The immutable run invalidated nine candidates rather than waiving their REDs:

1. RC1: runtime scanner scanned its own malicious fixture.
2. RC2: crypto inventory truncated production code after an inline test module.
3. RC3: local SBOM identities embedded absolute checkout paths.
4. RC4: proxy tests depended on an untracked generated WASM directory.
5. RC5: a concurrently inherited FastSwap lock descriptor could outlive the
   logical store owner and block immediate restart. Commit `b49c85e4` adds an
   explicit owner-drop unlock and a deterministic inherited-descriptor
   regression while preserving fail-closed concurrent-open exclusion.
6. RC6: the clean clone exposed one execution compatibility regression that
   read a legacy NAV batch from an ignored operator-report directory. Commit
   `929b0f40` points the test at the byte-equivalent canonical catch-up batch
   already tracked under node testdata; no fixture or runtime code was added.
7. RC7: the final reconciliation search found stale narrative labels claiming
   already-green candidate or hosted-review gates were still pending. The audit
   now names the exact-candidate evidence and keeps only genuinely open
   real-value/future work; hosted external review is not a source-publication gate.
8. RC8: the second clean clone exposed another replay test dependency on an
   ignored operator-report fixture, while the source workspace exposed a
   load-sensitive sleep threshold in the quorum-early unit regression. Commit
   `d5740cdc` promotes the required public signed legacy batch into canonical
   testdata and replaces the timing race with a deterministic gated-sixth-response
   proof. Runtime protocol behavior and release performance thresholds are unchanged.
9. RC9: the final completion audit found the controlling P0 plan still showed
   already-proven internal replay, simulation, staging and clean-history work as
   unchecked. Its short workspace runs were interrupted before the proof-heavy
   tail. The controlling plan now leaves only the provider-owner terminal action
   and its private record open; external review remains a real-value gate, not a
   source-publication dependency.

The corrected final candidate repeats the complete battery. The remaining
unchecked checklist lines are explicitly labeled either `REAL-VALUE LAUNCH`,
`FUTURE HARDENING / P2`, or the single `SOURCE-PUBLICATION BLOCKER`; none is an
unclassified discovery placeholder.
