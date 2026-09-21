# Deploy prepared and NAVCoin audit

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-21 UTC

## BLUF

The qualified combined release `a5b1e757` (CI fully green) is
[prepared for deployment](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/README.md)
up to the signatures this server cannot produce: the deployment manifest and
canary backup need the deployment publisher key, which is not on this server.
The requested NAVCoin audit became [burn 6](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/qa-campaign-20260921.md):
including optional A5, 0 P1, 9 P2 and 6 P3; every P2 is repaired. NAV-03 and
SWX-01 change state-transition results, so this deployment stays pinned to
`a5b1e757`; burn 6 repairs go into the next release after re-qualification.
The reachable-history secret-scan blocker is cleared: all seven findings were
test fixtures. Nothing was deployed; the fleet still runs
`a666-source-route-20260907`, subject to the observation boundary below.

## Current state

- **Deployment preparation:** `9cc1fce8` added release
  `combined-devnet-20260921`, qualified tip `a5b1e757`, executable SHA-256
  `051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d`.
  Build source and expected RPC build revision are `03e422a7`.
  The [packet](https://github.com/postfiatorg/postfiatl1v2/tree/release/combined-devnet-20260915/deployments/combined-devnet-20260921)
  contains all six validators' generated inputs under
  `rootfs/etc/postfiat/releases/combined-devnet-20260921/`: topology, swap
  and private-egress metadata, bindings and environment files, mirroring
  validator-0's read-only layout observation. The
  [unsigned manifest input](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/manifest-input.unsigned.json),
  [local checker](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/local-preflight.py)
  and [PASS result](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/local-preflight.json)
  cover local inputs, canonical regeneration and bindings only.
  [PREFLIGHT.md](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/PREFLIGHT.md)
  retains the blocked signed-manifest verification, fresh fleet preflight
  and signed canary backup. [SIGNING.md](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/SIGNING.md)
  gives the exact `sign-manifest.py` invocation, the one-year approval window
  mirroring the current manifest, and backup-signing commands. No key
  material is in the directory; fleet access was read-only.

- **Burn 6:** The other lane's NAVCoin audit request is governed by the
  [brief](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/qa-campaign-20260921-burn6-brief.md)
  (`6ae06356`). The [campaign log](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/qa-campaign-20260921.md)
  indexes each review's file, line, condition, observed/expected behaviour
  and suggested change. Counts below are P1 / P2 / P3; all P2s are repaired.

| Review and scope | Findings | Result |
| --- | --- | --- |
| [A1 portfolio verification](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/portfolio-verification-review-20260921.md): 1,356 lines; yolo target/collection verifiers, target queries, collection public values and receipt tests | 0 / 0 / 1 | No P1/P2 in the reviewed verification code. PFV-01: collection regression covers only attestation mismatch. Findings `21459c8a`. |
| [A2 NAV and reserve verification](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/nav-reserve-verification-review-20260921.md): 4,022 lines; NAV protocol/public values, `market_bridge.rs`, proof-status tests, live-NAV-mark and route-epoch builders | 0 / 3 / 0 | NAV-01: duplicate status rows inflated subscription overlay. NAV-02: packet input could replace the reserve-submit tag. NAV-03: invalid local receipt history was checked after reserve mutation; consensus-affecting under the brief's rule. Findings `4bc6d98d`; repairs `e9ccdeda`. |
| [A3 swap and settlement execution](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/swap-settlement-execution-review-20260921.md): 8,797 lines; NAV vault execution, PFTL source settlement and bridge profile resolution | 0 / 1 / 0 | SWX-01: public redemption omitted the policy NAV-age limit; consensus-affecting. Findings `6d495f43`; repair `043b9d69`. |
| [A4 bridge workflows](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/bridge-workflows-review-20260921.md): 8,624 lines; bridge workflows/conservation, Ethereum checkpoint signing and pfUSDC tier 4 | 0 / 3 / 3 | BRW-01: conservation trusted cached success fields. BRW-02: deposit receipts could contradict selected logs. BRW-03: observations mixed source blocks. Repairs are not consensus-affecting; BRW-04/05/06 remain P3. Findings `66a68624`; repairs `10bb5655`. |
| [A5 optional remaining node files](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/node-remaining-review-20260921.md): focused ranges of `block_replay_wallet.rs`, `rpc_dispatch.rs`, `transport_protocol.rs`, skipped by earlier burns | 0 / 2 / 2 | NOD-01: duplicate batch acknowledgments reported rejected receipts as accepted. NOD-02: wallet output aliases could overwrite recovery backup. Both repaired, neither consensus-affecting; NOD-03/04 remain P3. Findings `d3608838`; repairs `87c992c3`; close `053e5210`. |

- **Inventory and verification:** `eba04faf` added 11 rows to the
  [inventory](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/defect-inventory-20260910.md):
  137 rows, 26 P1 / 75 P2 / 36 P3, 91 fixed. The first complete gate scored
  **87.33/100**, run group `qa-defect-inventory-burn6-20260921`, scored file
  SHA-256 `e6c409eabed23c17fbacec8857d8c6a90d5ecd590a6da5e493c3847b8ba293c0`
  ([Scores](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/qa-campaign-20260921.md#scores)).
  A1–A4/B closed in `0e2ae50c`; A5 subsequently appended four NOD- rows,
  bringing the inventory to **141 without a rescore**. The 87.33 gate predates
  A5. [Verification](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/qa-campaign-20260921.md#verification)
  records focused tests by surface; the full suite is CI's verdict on the
  pushed branch. Local tests and offline qualification establish no live
  authority or deployment.

- **History secret scan:** The [disposition](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/history-secret-scan-disposition-20260921.md)
  classifies all seven findings in `wallet-extension/lib/security-regression.test.mjs`
  and six `wallet-web/scripts/*.mjs` UX scripts as temporary test-wallet,
  browser-vault or synthetic passphrases; none is a credential.
  [The scanner](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/scripts/public-secret-scan) allow-lists exact
  `(rule, path)` pairs for history scanning, with regression coverage.
  Tracked-tree and history scans report zero findings on main `14127221`
  and release `b1201bc9`. No history was rewritten.

- **Repository and CI:** Before this handoff, checkout `main` is at
  `14127221` (from `01eb47a0`); the initial pull was already up to date.
  `release/combined-devnet-20260915` moved from `a5b1e757` to `b1201bc9`
  today, adding deployment preparation, burn 6 and the scan allowlist.
  All combined-release and burn 6 work remains undeployed. At
  `2026-09-21T12:21:59Z`, `gh run list` showed:

| Tip | CI at capture time |
| --- | --- |
| Qualified deployment `a5b1e757` | All completed successfully: [rust-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35347335821), [product-security-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35347335859), [arc-proof-identities](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35347335926), [docs-build](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35347335689). |
| Release `b1201bc9` | [docs-build](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35598206802) passed; [rust-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35598206892), [product-security-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35598206795), [arc-proof-identities](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35598206896) in progress. |
| Main `14127221` | [docs-build](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35598049672) passed; [rust-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35598049554), [product-security-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35598049566) in progress. |

- **Fleet boundary:** The last all-six observation in the
  [September 17 preflight](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/status/z3-preflight-20260917.md)
  ran `2026-09-17T11:16:49.908219Z`–`11:32:07.540468Z`: height 1020,
  empty mempools, all 12 validator/RPC processes on
  `a666-source-route-20260907`, binary `57b0f4d1…634eec83`, reported build
  revision `707e006f`. [Current State](../status/chain-state-current.md)
  retains the canonical deployment baseline. Today's
  [validator-0 layout read](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/observed/layout.json)
  at `2026-09-21T09:41:02.187202Z` was restricted to release files and unit
  definitions; it was not a fresh fleet-status probe. No live probe was
  performed in this handoff session. Devnet was not touched: no fleet
  mutation, deployment or restart. No Task Node action.

- **Z3:** [Unchanged since September 17](2026-09-17___dravlic__arc_test_wallet_and_z3_offline_gates.md):
  offline gates done, wallet funded with 20 test USDC. The first integrated
  live cycle follows deployment. This lane chose the current-v2 Arc pair,
  a cap of 2,000,000 USDC atoms, and the
  [proposed latency bounds](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/z3-g5-failure-rehearsal-20260917.md).

## Next decision or action

### This lane's decisions

1. As soon as the signed manifest and signed canary backup exist, deploy per
   [DEPLOY-SHEET.md](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/DEPLOY-SHEET.md):
   fresh read-only observation and preflight, signed-backup verification,
   `apply-next` to canary validator-1, then 0, 2, 3, 4, 5 individually, with
   health and six-node convergence after each. Follow the runbook's stop
   conditions and [rollback-one.sh](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/rollback-one.sh)
   on the first failed check; finish with a fresh observation and canonical
   status record. Deployment is pinned to qualified tip `a5b1e757` and
   executable `051ad12c…bc7d`, with no burn 6 changes.
2. Run the first Z3 integrated cycle on the new release, using the pair,
   cap and latency bounds already chosen.
3. Re-qualify the tip carrying NAV-03, SWX-01 and later burn 6 commits for
   the release after this one. Follow up the four A1–A4 P3s (PFV-01 and
   BRW-04/05/06); A5's NOD-03/04 remain recorded as two additional P3s.

### Only the other lane can provide

| Item | First asked (UTC) |
| --- | --- |
| Sign the `combined-devnet-20260921` deployment manifest and canary backup with the deployment publisher key per [SIGNING.md](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/SIGNING.md), or hand the key to this lane. | 2026-09-21 |
| Height-915 quarantine archive and height-924 custodian authorization. | 2026-08-30 |
| AI-governance decision for Gate Zero Z2. | 2026-09-03 |
| Whitepaper abstract-rule decision. | 2026-09-10 |
| StakeHub PR #8 findings decision: three P1 items remain unanswered while live funds are planned there. | 2026-09-07 |
| Decisions or a named live environment for the twelve inventory rows. | 2026-09-10 |
| Who restarted every service on all six validators on 2026-09-11. | 2026-09-14 |
| Task Node instruction for this lane. | 2026-08-27 |

## References

- [Safe validator rollout](../runbooks/safe-validator-rollout.md) and
  [signed deployment manifest](../runbooks/signed-deployment-manifest.md).
- [Previous handoff](2026-09-18___dravlic__combined_release_merged_repaired_and_qualified.md)
  and [other lane's last handoff](https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/handoffs/2026-09-16___codex__combined_release_check_kickoff_and_private_funding.md).
- Release preparation and A1:
  [9cc1fce8](https://github.com/postfiatorg/postfiatl1v2/commit/9cc1fce8),
  [6ae06356](https://github.com/postfiatorg/postfiatl1v2/commit/6ae06356),
  [21459c8a](https://github.com/postfiatorg/postfiatl1v2/commit/21459c8a),
  [b82052ce](https://github.com/postfiatorg/postfiatl1v2/commit/b82052ce).
- A2–A4 findings, repairs and closeouts:
  [4bc6d98d](https://github.com/postfiatorg/postfiatl1v2/commit/4bc6d98d),
  [e9ccdeda](https://github.com/postfiatorg/postfiatl1v2/commit/e9ccdeda),
  [cf12b64f](https://github.com/postfiatorg/postfiatl1v2/commit/cf12b64f),
  [6d495f43](https://github.com/postfiatorg/postfiatl1v2/commit/6d495f43),
  [043b9d69](https://github.com/postfiatorg/postfiatl1v2/commit/043b9d69),
  [acfda116](https://github.com/postfiatorg/postfiatl1v2/commit/acfda116),
  [66a68624](https://github.com/postfiatorg/postfiatl1v2/commit/66a68624),
  [10bb5655](https://github.com/postfiatorg/postfiatl1v2/commit/10bb5655),
  [b322b28b](https://github.com/postfiatorg/postfiatl1v2/commit/b322b28b).
- Inventory, campaign close and A5:
  [eba04faf](https://github.com/postfiatorg/postfiatl1v2/commit/eba04faf),
  [0e2ae50c](https://github.com/postfiatorg/postfiatl1v2/commit/0e2ae50c),
  [d3608838](https://github.com/postfiatorg/postfiatl1v2/commit/d3608838),
  [87c992c3](https://github.com/postfiatorg/postfiatl1v2/commit/87c992c3),
  [053e5210](https://github.com/postfiatorg/postfiatl1v2/commit/053e5210).
- Scan allowlist: [14127221 on main](https://github.com/postfiatorg/postfiatl1v2/commit/14127221)
  and [b1201bc9 on the release branch](https://github.com/postfiatorg/postfiatl1v2/commit/b1201bc9).

## End of session

The [Z3 dry-run record](../status/z3-dry-run-20260921.md) captures the release-build
first run (`77314538`), tooling fix (`d825b4fb` on main, `048d23df` on the
release branch), and second run (`61bcbaba`). The first run stopped before
printing any command because manifest validation rejected cycle 0; row 5 also
carried a flag the node does not accept. Both were fixed with tests: cycle 0
is accepted only on the explicit dry-run path, and the skeleton is marked
`dry_run` and not counted as a live cycle. The second run printed and resolved
**39/39 commands** and wrote the skeleton. **Dry-run tooling: PASS.**

The live cycle remains **BLOCKED** on the record's
[Remaining live-cycle preconditions](../status/z3-dry-run-20260921.md#remaining-live-cycle-preconditions):
17 execution inputs still unknown (current primary policy, anchor code hash,
NAV program and source-manifest binding, PFTL signer roles, predecessor
artifacts), signer bindings, qualified deployment and prover provenance,
fresh state read-backs, G5 monitoring, and live authorization.

**Next shift — this lane's plan:** after deployment of
`combined-devnet-20260921`, insert **cycle-1 preparation** before the first
live cycle: collect the 17 inputs by read-back on the deployed release and
pin prover provenance. Only then proceed to the first authorized live cycle
with the remaining preconditions satisfied.

Branch tips before this append: main **`61bcbaba`**;
`release/combined-devnet-20260915` **`048d23df`**.
At write time (`2026-09-21T12:57:17Z`), `gh run list` showed:

| Tip | CI at capture time |
| --- | --- |
| Main `61bcbaba` | [docs-build](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35602039425) passed; [rust-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35602039340) and [product-security-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35602039605) in progress. |
| Release `048d23df` | [docs-build](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35602058863) passed; [rust-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35602058991), [product-security-ci](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35602058785), and [arc-proof-identities](https://github.com/postfiatorg/postfiatl1v2/actions/runs/35602058661) in progress. |

No fleet action, no signature, no transaction, and no Task Node action in this
session.
