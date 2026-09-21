# Combined release merged, repaired, and qualified

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-18 UTC

**Update:** The [September 21 handoff](2026-09-21___dravlic__deploy_prepared_and_navcoin_audit.md)
records green CI for `a5b1e757`, deployment preparation awaiting publisher
signatures, burn 6 repairs reserved for the next release, and the cleared
history secret-scan blocker. Nothing has been deployed.

## BLUF

This lane merged main into the combined release candidate (draft PR #41),
repaired both P1 review findings, and merged [PR #42](https://github.com/postfiatorg/postfiatl1v2/pull/42)
into `release/combined-devnet-20260915`. The [qualification packet](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/release-repair-20260918/README.md)
records passing disposable-copy history, rotation, and pre-activation rollback
checks. The initial Clippy and reproducible-build failures were closed the same
day; both clean builds of `a5b1e757` now match. Deployment to the six validators
is next, subject to the remaining gates, its own explicit go, and a written
rollback path. Nothing was deployed today; the live fleet was untouched.

## Current state

- **Repository and integration:** This checkout is `main` at `1a7f7bf1` before
  this handoff; `git pull --rebase origin main` was already up to date.
  On `integrate/main-into-combined-20260918`, merge `4d0a0b32` contains both
  main `1a7f7bf1` and candidate `15126ac3`: 36 main commits covering burn-four
  and burn-five repairs, the P3 sweep, transport timing-test fix, RPC probe,
  and Z3 tooling. The sole conflict was in
  [rpc_cli.rs](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/crates/node/src/rpc_cli.rs).
  Main's [rpc_serve_runtime.rs extraction](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/crates/node/src/rpc_serve_runtime.rs)
  was kept, preserving the candidate's StatusReport cache, initial local-status
  seed, data-directory and health-cache helper parameters, report-age expiry
  and refresh, and publish-on-success timing; each is named in the merge body.
  Recorded merge checks passed: RPC serve/transport 13; vote locks 18
  (1 ignored); types 147; execution 202; ordering 35; FastPay 19 + 1 + 4;
  Python 611 (3 skipped, 103 subtests); proof public-input inventory and three
  documentation gates.

- **P1 repairs:** Both are consensus-affecting, source-only changes, recorded
  by `b8b8145d` in the [updated review](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/pr41-release-candidate-review-20260916.md).
  `6849d976` requires V2 authorization for new Arc activations, binding a
  canonical digest of bootstrap trust state (initial validator set and
  checkpoint); activations differing only in trust state are no longer both
  accepted. `397b4f53` shares certificate count/shape validation between
  FastPay acceptance and V2 commitment encoding: accepted certificates fit
  the commitment bound, and unknown voters are rejected. Regression checks
  passed: types 149; execution 205; Arc types 5; vault-bridge governed route
  15 (2 Anvil tests remain ignored); Cobalt authority kinds 1; FastPay recovery
  types 9; owned-transfer recovery 11; node FastPay 19; pfUSDC proofs 1;
  cargo check, formatting, and three documentation gates. Finding 3 (P2)
  remains open.

- **Release merge:** This lane opened and merged PR #42
  (`integrate/main-into-combined-20260918` → `release/combined-devnet-20260915`,
  +8,665/−921) with a merge commit at `2026-09-18T10:39Z`. Resulting release
  tip `d224c0be` contains candidate `15126ac3`, main `1a7f7bf1`, and both
  repairs. Subsequent qualification and repair commits brought the release
  tip to `a5b1e757`.

- **Local qualification:** The packet (`10b7b203`, `e49a2390`) uses source
  `d224c0be` for history/service checks and the September 16 packet's method.
  PASS: six original full-history checks at height 1020, six saved-V2 checks
  at 1021, six fresh-rotation replays at 1021, local governed rotation in
  both startup orders with six accepted receipts, convergence and restart,
  and pre-activation rollback (six old-binary checkpoint/restart checks plus
  new-binary full replay). Workspace check, formatting, proof-input inventory,
  FastPay types/execution suites, live-replay supply, three documentation
  gates, and checksums passed. The full workspace test suite passed in CI
  for the source tip; node-fastpay and warm-latency were deferred to CI.
  This is disposable local-copy qualification, not live authority. Copies
  and logs remain under `~/.cache/release-repair-20260918` on the server;
  the memory rule held: one copy at a time, two workers, limits.

- **Initial failures closed:** `03e422a7` fixes the test-only Clippy error at
  [cobalt_handoff.rs:1466](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/crates/node/src/cobalt_handoff.rs#L1466);
  workspace Clippy and all 13 Cobalt handoff tests pass. Two clean release
  executables initially differed at byte 8703: the ambient Zig linker
  embedded a build-specific RUNPATH string in `.dynstr`. Using the September
  16 build method, both clean builds of `a5b1e757` match at SHA-256
  `051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d`.
  The packet's README row is PASS;
  [node-builds.json](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/release-repair-20260918/node-builds.json)
  retains the superseded attempt, and
  [SHA256SUMS](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/release-repair-20260918/SHA256SUMS)
  was regenerated.

- **CI and remaining deployment gates:** At `2026-09-18T13:04Z`,
  `gh run list` for `d224c0be` showed
  [product-security-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35335763524),
  [arc-proof-identities](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35335763574),
  and [docs-build](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35335763440)
  completed successfully; [rust-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35335763469)
  completed with failure overall, despite the recorded workspace-test pass.
  For `a5b1e757`, [docs-build](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35347335689)
  passed; [rust-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35347335821),
  [product-security-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35347335859),
  and [arc-proof-identities](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35347335926)
  were in progress. Still required: fresh green CI on `a5b1e757`, closure of
  node-fastpay and warm-latency, classification as test-only of seven
  reachable-history secret-scan findings in old wallet test files, and the
  unavailable pinned testnet archive.

- **Fleet and activation boundary:** The last fleet observation in the
  [September 17 preflight](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/status/z3-preflight-20260917.md) ran from
  `2026-09-17T11:16:49.908219Z` through `11:32:07.540468Z`: all six agreed at
  height 1020 with empty mempools, and all 12 validator/RPC processes ran
  `a666-source-route-20260907`, binary `57b0f4d1…634eec83`, reporting build
  revision `707e006f`. See [Current State](../status/chain-state-current.md)
  for the deployment baseline. The nine consensus-affecting repairs in the
  [September 16 handoff](2026-09-16___dravlic__flaky_tests_p3_sweep_candidate_review_and_burn_five.md)
  and today's two P1 repairs are now on the release branch, source-only and
  not activated. No fresh live probe, fleet mutation, deployment, or restart
  occurred in this session. No Task Node action.

- **Z3:** Unchanged since [September 17](2026-09-17___dravlic__arc_test_wallet_and_z3_offline_gates.md):
  offline gates done, wallet funded, no integrated live cycle yet. This lane
  decided to run the first live cycle after deployment, so the sustained
  window counts on one binary, using the current-v2 Arc pair used on
  September 2, a per-cycle cap of 2,000,000 USDC atoms, and the latency bounds
  proposed in the [G5 rehearsal review](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/z3-g5-failure-rehearsal-20260917.md).

## Next decision or action

### This lane's decisions

1. Close the remaining deployment gates above, obtain an explicit deployment
   go, and take a fresh read-only fleet observation. Deploy the qualified
   combined release to all six controlled-devnet validators with pinned
   source, binary, data, activation, and rollback identities and the written
   rollback path from the qualification packet. Then take a fresh observation
   and update the canonical status record.
2. Run the first Z3 integrated cycle on the new release (G4), then the
   sustained window, using the pair, cap, and latency bounds chosen above.
3. Address the remaining candidate P2: the Arc finality cursor rejects
   independent deposits at one height; see the updated review's finding 3.

### Only the other lane can provide

| Item | First asked (UTC) |
| --- | --- |
| Height-915 quarantine archive and height-924 custodian authorization; block offline storage qualification A3–A5. | 2026-08-30 |
| AI-governance decision for Gate Zero Z2. | 2026-09-03 |
| Whitepaper abstract-rule decision. | 2026-09-10 |
| StakeHub PR #8 findings decision. | 2026-09-07 |
| Decisions or named live environments for the twelve inventory rows. | 2026-09-10 |
| Who restarted every service on all six validators on September 11, 06:01–06:47Z. | 2026-09-14 |
| Task Node instruction for this lane. | 2026-08-27 |

## References

- [PR #42](https://github.com/postfiatorg/postfiatl1v2/pull/42),
  [draft PR #41](https://github.com/postfiatorg/postfiatl1v2/pull/41),
  [integration branch](https://github.com/postfiatorg/postfiatl1v2/tree/integrate/main-into-combined-20260918),
  [qualification packet directory](https://github.com/postfiatorg/postfiatl1v2/tree/release/combined-devnet-20260915/deployments/release-repair-20260918).
- [Other lane's last handoff](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/handoffs/2026-09-16___codex__combined_release_check_kickoff_and_private_funding.md).
- Integration and repairs:
  [4d0a0b32](https://github.com/postfiatorg/postfiatl1v2/commit/4d0a0b32),
  [6849d976](https://github.com/postfiatorg/postfiatl1v2/commit/6849d976),
  [397b4f53](https://github.com/postfiatorg/postfiatl1v2/commit/397b4f53),
  [b8b8145d](https://github.com/postfiatorg/postfiatl1v2/commit/b8b8145d),
  [d224c0be](https://github.com/postfiatorg/postfiatl1v2/commit/d224c0be).
- Qualification and closure:
  [10b7b203](https://github.com/postfiatorg/postfiatl1v2/commit/10b7b203),
  [e49a2390](https://github.com/postfiatorg/postfiatl1v2/commit/e49a2390),
  [03e422a7](https://github.com/postfiatorg/postfiatl1v2/commit/03e422a7),
  [a5b1e757](https://github.com/postfiatorg/postfiatl1v2/commit/a5b1e757).
