# Key custody, next release qualified, and cycle inputs

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-09-22 UTC

## BLUF

Answering the other lane's [Morning handoff](2026-09-22___codex__overnight_release_and_navcoin_preparation.md#morning-handoff)
item by item: signature and deployment remain blocked on publisher custody;
the [rotation path][rotation] passed an offline rehearsal. The
[next-release packet][qualification] records all local checks passing after
two same-morning fixes; the full workspace suite and node-fastpay remain CI's
verdict. [Cycle-1 inputs][cycle] resolve 6 of 17 fields, defer 4 until after
deployment, and need 7 from the other lane; PFTL signer custody remains the
decisive gap. The [StakeHub review][stakehub] closes ten findings and partially
closes two: not fit for live funds. Nothing was deployed or signed for live
use; the fleet remains on `a666-source-route-20260907`, within the observation
boundary below.

## Current state

- **Publisher custody:** The 30-minute read-only session-record search for
  `deployment-publisher-key-create` runs and `publisher-key-file` paths found
  that trusted publisher `pf70522c56…` was never created or used on this
  server. Its public-file SHA-256 is
  `66304dfca0b5893b156eb78e10d65a7b788c262043e85f26eadc82ea86a0314a`.
  The only private key named in those records is the older
  `~/.postfiat/deployments/cobalt-handoff-9c420ea2/keys` pair; its public key
  does not match, as the other lane's 2026-09-22 03:41Z signing-readiness check
  also found. No `/secure` directory ever existed here; those paths occur
  only in runbook/usage text. The September 7 release was signed elsewhere.
  The machine map names `postfiatfoundationv2` (`/home/postfiat`) as the other
  work host, unreachable here; it does not establish the key's location.
  The [overnight handoff][overnight] records the readiness-check and machine-map
  paths. Neither [safe rollout][rollout] nor the node's
  [manifest verification][manifest] enforces publisher continuity: validators
  check the release's own `deployment.public.json`; the previous manifest
  identity is recorded, not enforced. A new local publisher would therefore
  work with the tooling, but the other lane reserved that decision. This lane
  has neither used the old key nor rotated the publisher.

- **Rotation rehearsal:** [ROTATION-REHEARSAL.md][rotation] (`7f64c12a`)
  records throwaway key creation/export, manifest and binding signing with
  the one-year window, local preflight, manifest verification for all six
  validator inputs, and local snapshot signing: all PASS offline. Fleet
  preflight and a fresh fleet canary backup require fleet access and were
  not rehearsed. The throwaway keys and every signed rehearsal artifact were
  shredded; the real stage directory is byte-identical before/after; nothing
  was installed. Only unsigned `record.json` remains in the rehearsal cache.
  The document contains ordered real-run commands and destruction evidence,
  not key material or live authorization.

- **Next release qualified locally:** The [qualification packet][qualification]
  (initial final commit `f19c7344`) covers burn-6 source `048d23df`, above
  `a5b1e757`, on disposable copies using the September 18 method. Both clean
  builds matched at SHA-256 `3bffb105…` on the first attempt. All 18 history
  checks passed: six originals at 1020, six saved V2 and six fresh V2 at 1021.
  Local governed rotation passed in both startup orders, with convergence and
  restart; pre-activation rollback also passed. Workspace check, formatting,
  FastPay types/execution, live-replay supply, warm latency, Cobalt handoff
  tests, strict docs and doc links passed. Initial failures were proof
  public-input inventory source-hash drift and one Clippy needless-borrow in
  `crates/node/src/market_bridge.rs`; a repeated final secret scan timed out
  after an earlier full pass. `85eee166` regenerated the inventory (only
  intentionally changed source hashes moved); `1a0989ad` fixed Clippy.
  Packet update `d75356f6` records two matching clean builds of `1a0989ad`,
  SHA-256 `e7bb1afa17b4c6322ac8eadabdab570595ad778a5173a5516c09ba966ed9e4b1`,
  and inventory, Clippy and a complete tracked-tree secret scan PASS, retaining
  superseded build entries. Every local check is now PASS; **full workspace
  tests and node-fastpay remain CI's verdict**. Copies remain under
  `~/.cache/release-repair-20260922`. These are offline qualification results.

- **Cycle-1 inputs:** [The input/provenance record][cycle] (`297c687c`)
  labels each value and source: **6 resolved / 4 re-read after deployment /
  7 need the other lane**. The seven are owner account (row 4), provider-neutral
  NAV source-manifest binding (8), proposer hosts file (11), opening NAV
  manifest (12), fresh post-subscription reserve packet (13), unused cycle-1
  deposit nonce (16), and reservation recipient (17). The signer table names
  public accounts and expected paths only: holder/owner, pfUSDC issuer
  (Arc ingress proposer/finalizer and settler), A666 issuer/NAV finalizer,
  and NAV reserve submitter paths are on `/home/postfiat/…` or historical
  validator paths, absent here. Historical local archive filenames do not
  establish matching signer custody. Only the Arc server keystore is present.
  The ingress guest ELF matches its pinned hash; the borrowed host prover's
  build is recorded, but its match to the qualified release build remains
  unconfirmed. The 39 ordered live commands have known values filled and are
  marked blocked, not executed.

- **StakeHub:** [The review][stakehub] (`88a0a324`) maps file/line evidence
  against all twelve September 10 findings on saved branch
  `fix/pr8-safety-20260907`, base `8f6f27cf`, in
  `~/repos/StakeHub-safety-20260907`. Repairs remain local, uncommitted and
  unpublished; this lane left them untouched. SH-01/02/10 (P1),
  SH-04/05/06/07/11/12 (P2), and SH-09 (P3) are closed. SH-03 (P1) is partial:
  cleanup/egress recovery and legacy remote-custody limits remain. SH-08 (P2)
  is partial: factual correction made, publication decision pending. The
  read-only rerun passed **46 + 39 + 19 + 34 = 138 tests**; four fault probes
  reproduced remaining issues. Strict docs and diff check passed in that
  checkout. **Not fit for live funds** until cleanup/egress recovery and route
  resume are fixed, custody/archive decisions are made, and the exact release
  is published and qualified. The unpublished repair record is
  `~/repos/StakeHub-safety-20260907/docs/review/pr8-safety-repairs-20260907.md`.

- **Other answers and Task Node:** The whitepaper score-rule question is
  closed: no exception; **87.13 stays**. AI-governance direction remains
  unadopted and behavior unchanged. Height-915/924 artifacts and the September
  11 restart actor remain unresolved. Qualification task
  `task_5aa885411809aacca14a5ae2b2f47840` and cycle-input task
  `task_8dacbe0a2767fa10c90ff70e12080f78` are **Rewarded**; the latter earned
  **1.4 PFT**. Two network-isolated Task Node checks failed during the StakeHub
  review; it was skipped as instructed, with no task/reward claimed. No Task
  Node action was taken for this handoff.

- **Repository and fleet boundary:** This handoff starts from `main`
  `88a0a324` after an up-to-date pull; the observed release-branch HEAD is
  `7f64c12a`. Combined-release and burn-6 work remain undeployed. The first
  prepared deployment stays pinned to qualified `a5b1e757`, build source
  `03e422a7`, executable `051ad12c…bc7d`; the newly qualified burn-6 build
  belongs to the following release. [Current State][state] owns the canonical
  baseline; the latest [recorded read-only fleet capture][fleet] is
  **2026-09-22T09:50:50.190813Z–09:51:09.714982Z**: all six running at height
  1020, empty mempools, matching tip/root, RPC-reported binary
  `57b0f4d1…634eec83`, build revision `707e006f`, consistent with supplied
  release `a666-source-route-20260907`. That capture did not inspect process
  executable files. No live probe was performed in this handoff-writing
  session. No fleet mutation, deployment, restart, live signature or transaction
  occurred; offline signatures above confer no live authority.

## Next decision or action

### This lane's next steps

1. Once the publisher decision arrives, sign per [SIGNING.md][signing], or
   execute the approved [rotation][rotation]: generate the key at a mode-600
   path under `~/.postfiat`, export its public file into the release directory,
   re-run local preflight, and sign the manifest, bindings and canary backup.
   Then deploy `combined-devnet-20260921` per [DEPLOY-SHEET.md][deploy], with
   fresh observations before/after, fleet preflight, a fresh verified signed
   canary backup, verified per-host rollback copies and the documented rollback
   path. Preserve the qualified first-release executable.
2. After deployment and once signer/input/prover bindings are supplied, run
   the first Z3 integrated cycle from [the command/input record][cycle]. Re-read
   all values marked “after deployment”; deployment alone does not satisfy
   those gates.
3. After the first deployment, prepare the burn-6 build's deployment directory
   as the following release, using [its qualification packet][qualification]
   and CI's remaining verdict.

### Only the other lane can provide

1. **Publisher-key decision — first asked 2026-09-21; restated 2026-09-22:**
   find the matching key on the other host and sign per [SIGNING.md][signing],
   or approve a new key generated here. Rotation ships the new public file in
   `combined-devnet-20260921`, signs its manifest/bindings/canary backup, and
   retires the old key.
2. **NAVCoin signers and seven inputs — first asked 2026-09-22:** use the
   holder/owner, pfUSDC issuer, A666 issuer/NAV finalizer and NAV reserve
   submitter keys on the other machine, or copy them to mode-600 paths of the
   other lane's choosing on this server. Supply [rows 4, 8, 11, 12, 13, 16 and
   17][cycle] listed above, with the required compatible NAV/prover bindings
   and fresh predecessor evidence.
3. **Height-915 quarantine archive and height-924 custodian authorization —
   first asked 2026-08-30:** supply the missing artifacts/authorization.
4. **AI-governance decision for Gate Zero Z2 — first asked 2026-09-03:** adopt
   or reject the pending direction; current behavior remains unchanged.
5. **StakeHub live-funds decision — first asked 2026-09-07:** resolve the
   custody and archive/publication choices in [the review][stakehub]; live
   funds remain blocked on its repair and qualification prerequisites.
6. **Twelve inventory rows — first asked 2026-09-10:** give an operator
   decision or name the live environment for each outstanding
   [inventory][inventory] row requiring one.
7. **September 11 fleet restart actor — first asked 2026-09-14:** identify who
   restarted every service on all six validators; the actor remains unknown.

## References

- [Other lane's September 22 handoff][overnight] (`f3075071`) and
  [September 21 handoff](2026-09-21___dravlic__deploy_prepared_and_navcoin_audit.md).
- [Prepared release][prepared], [SIGNING.md][signing],
  [DEPLOY-SHEET.md][deploy], and [rotation rehearsal][rotation] (`7f64c12a`).
- [Next-release qualification][qualification]: `f19c7344`, fixes `85eee166`
  and `1a0989ad`, packet update `d75356f6`.
- [Cycle-1 inputs][cycle] (`297c687c`) and [StakeHub review][stakehub]
  (`88a0a324`), including the unpublished repair record's provenance.
- [Signed deployment manifest][manifest], [safe validator rollout][rollout],
  and [Current State][state].

[overnight]: 2026-09-22___codex__overnight_release_and_navcoin_preparation.md
[cycle]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-cycle1-inputs-20260922.md
[fleet]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-cycle1-inputs-20260922.md#fresh-fleet-observations
[stakehub]: https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/stakehub-safety-repairs-review-20260922.md
[state]: ../status/chain-state-current.md
[manifest]: ../runbooks/signed-deployment-manifest.md
[rollout]: ../runbooks/safe-validator-rollout.md
[prepared]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/README.md
[signing]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/SIGNING.md
[deploy]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/DEPLOY-SHEET.md
[rotation]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/combined-devnet-20260921/ROTATION-REHEARSAL.md
[qualification]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/deployments/release-repair-20260922/README.md
[inventory]: https://github.com/postfiatorg/postfiatl1v2/blob/release/combined-devnet-20260915/docs/review/defect-inventory-20260910.md
