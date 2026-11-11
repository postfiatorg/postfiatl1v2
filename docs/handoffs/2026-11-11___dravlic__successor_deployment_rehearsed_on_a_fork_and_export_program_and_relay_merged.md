# Successor deployment rehearsed on a fork, and export program and relay merged

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-11 UTC

## BLUF

Two things today.

- **The branches pushed on 10 November without a pull request.** I read each of them, and each got a review. What was mine to continue is continued:
  - the successor work for the two wrapped tokens now has review fixes, a deployment command that reads every value back, and a rehearsal of the deployment and of the holder migration on a local fork of Ethereum mainnet (private branches; nothing deployed, no key opened, nothing sent to a public network);
  - the NAV proof watchdog is on main with review fixes;
  - the review notes for all of it are in one place in the private repository.
- **The route back to Base.** The lane export proof program with its reproducible identity, the relay, the local round trip, the Base Sepolia rehearsal command and the holder's export command are merged into main after review. The watcher's export view stays on its branch: its last reading asks for a design step on how it reads Base. Nothing was sent to Base Sepolia.
- **Also on main:** the Z3 selected-state read-back with the one-cycle review packet; cursor paging for `owned_objects` through the node, the gateway, the Python client and the web wallet; the Python client's checks that it signs what was asked; a new precondition for activating the lane export root.
- **The devnet was read, not touched:** all six validators at height 1177 on `vote-records-20261030`.

Primary references: [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md) (P3.1), [release plan](../plans/active/next-validator-release-plan-20261115.md), [Current State](../status/chain-state-current.md).

## Current state

**The 10 November branches, read and reviewed**

One index for everything private, in `postfiatorg/postfiatl1v2-private-archive-20260717`: branch `docs/client-lane-branch-reviews-20261111` (tip `0cd7f7565`), `docs/review/client-lane-branch-reviews-20261111/README.md`, with one page per piece. The private notes carry the content of each review.

- **The successor token and holder migration** (private branches `emergency/s7-successor-token-20261110` at `19a0fcddd` and `emergency/p07-c03-holder-migration-fork-20261110` at `586c7564a`; note `successor-token-and-holder-migration.md`). My work on top is on two private branches:
  - `navcoins/successor-deployment-rehearsal-20261111` (tip `75a574c48`): review fixes with failing tests first, the deployment command `scripts/deploy-navcoin-successor.py`, and a rehearsal of the deployment and the holder migration on a local fork. Its paths: the record `docs/review/navcoin-successor-fork-rehearsal-20261111/`, the runbook `docs/runbooks/navcoin-successor-deployment.md`, a note for the product owner `docs/review/navcoin-successor-review-20261111.md`, and a first draft of a holder notice `docs/drafts/navcoin-successor-holder-notice-draft.md`.
  - `navcoins/nav-time-rule-activation-20261111` (tip `d70f4fa88`): the native NAV observation time rule behind a governed activation height that is unset by default; note `docs/navcoins/nav-observation-time-rule-activation-20261111.md`.
  - Nothing is deployed anywhere. The security release candidate's branch is untouched.
- **The resumable owned-history migration** (private branch `security/s9-history-migration-20261110` at `c575c9490`): reviewed; note `owned-history-migration-resume.md`.
- **The pfUSDC route reconciliation gate** (private branch `pf48/p06-c06-route-reconciliation-gate` at `d1646b571`): reviewed; my fix branch is `pfusdc/route-reconciliation-gate-fixes-20261111` (tip `dbd30525b`); note `pfusdc-route-reconciliation-gate.md`.
- **The sale escrow contract** in the trading repository (`postfiatorg/navstrategies`, branch `pf48/schwab-navcoin-creation-redemption`): a read-only review, nothing changed there; note `sale-escrow.md` in the same private directory.
- **The NAV proof watchdog** (public branch `tools/nav-proof-watch-20261110`), on main as `419a0a439` + `69c6c2e32`. These are new commits that take the tool from the branch at `71aa45d54` and add review fixes:
  - a log that stopped raises an alarm; proof gaps are counted only inside recorded time; terms that could never alarm are refused;
  - a paused or short vault is an alarm whatever input is missing; a truncated log tail is detected;
  - no endpoint, host or path in code, fixtures or docs; the service runs as its own user;
  - its tests (15 → 30) run in CI; a section in [NAVCoins, "Watching that proofs keep arriving"](../navcoins/index.md#watching-that-proofs-keep-arriving) and the [tool README](https://github.com/postfiatorg/postfiatl1v2/blob/main/tools/nav-proof-watch/README.md).
  - The branch's recorded evidence directory is not on main; the branch itself is untouched.
  - Its live read today: height 1177, A666 on NAV epoch 9 and BMNRC on epoch 1 as in October, no observation time on either route.
  - One observation from the same read: the wallet API's TLS certificate is a short-lived one for an IP address, expiring 2026-11-16. Such certificates renew when the renewal job runs, so this is a note to look at once, not a finding.

**The route back to Base, merged on main** (Phase 3 of the ERC-20 bridge; in work, and taking it live is not planned in the current window)

`82d921f90` "Merge the lane export proof program, the relay and the round-trip and Base Sepolia rehearsal commands" (second parent `536c91de7`), `21d96f44a` "Merge the holder's lane export command" (second parent `638de3536`), and `8a0f0149f` (the milestone's P3.1 sub-lines and the release plan rows). The reviews' results are built in.

- **The proof program:** [`crates/pft_lane_proofs`](https://github.com/postfiatorg/postfiatl1v2/tree/main/crates/pft_lane_proofs) and [`programs/pft-lane-export`](https://github.com/postfiatorg/postfiatl1v2/tree/main/programs/pft-lane-export).
  - Committed reproducible ELF, sha256 `e92f20046c20761bb7a26ec47540000a88fad28b51874453437c3dd390f448ab`; verifying key `0x000cef5bfa4586cfb79e255a950ae8c1dbb15c4270a968ad5bfcbe32d0a81414`.
  - The identity workflow `pft-lane-export-identity` rebuilds and compares on every push and pull request. On main it is green on `82d921f90`, `21d96f44a`, `8a0f0149f`, `ce238378b`, `7dc225c55` and `40581d6d5`.
  - Its proof inventory entry makes 8 systems.
  - **No real SP1 proof has been generated with it.** The [program README](https://github.com/postfiatorg/postfiatl1v2/blob/main/programs/pft-lane-export/README.md) and the runbook name one real proof verified against the pinned SP1 verifier as the acceptance gate before any verifier that is meant to last is constructed with this key.
- **The relay** [`scripts/pft-lane-export-relay.py`](https://github.com/postfiatorg/postfiatl1v2/blob/main/scripts/pft-lane-export-relay.py). A placeholder proof is sent only to a loopback Anvil whose SP1 verifier has the pinned code hash of the repository's mock. The decoded public values that will be sent are the ones checked before sending.
- **The local round trip** [`scripts/rehearse-lane-roundtrip.py`](https://github.com/postfiatorg/postfiatl1v2/blob/main/scripts/rehearse-lane-roundtrip.py) ([record](../review/lane-roundtrip-rehearsal-20261110/README.md)). I reran it today through the changed relay: 1,000,000 units locked on a local Anvil → native PFT → exports of 400,000 and 600,000 paid out of the same lock → the lock at zero, with conservation on both sides. A mock SP1 verifier and a local Anvil, with one data directory as the fleet: a local result, not live authority.
- **The Base Sepolia rehearsal command** [`scripts/rehearse-lane-base-sepolia.py`](https://github.com/postfiatorg/postfiatl1v2/blob/main/scripts/rehearse-lane-base-sepolia.py), with the [dry-run runbook](../runbooks/lane-base-sepolia-dry-run.md) and the [fork record](../review/lane-base-sepolia-fork-rehearsal-20261110/README.md). I reran it today through every stage on a local fork only. Nothing was sent to Base Sepolia. Today's review fixes:
  - the in-process chain now outlives the Base finality wait (before, a real run would have lost it after the deployments and the Lock);
  - every keystore's address and every balance is checked before the first send;
  - all constructor values are read back on the real verifier before the Lock is sent;
  - an existing allowance is set to the exact amount, and read back as zero after the Lock;
  - the relayer has a ceiling that bounds what a release can spend;
  - the payout recipient is validated in the plan; the L1 data fee is its own funding line.
  - A second reading of the last five commits this afternoon: the test-network steps up to the proof step are in order as far as the code goes. Two points are open before the first release that carries a real proof: on a resumed run, the relayer whose balance is checked must be the relayer that signs; a balance one wei above the ceiling refuses the release late instead of at the start. A release also needs a prover and the real-proof gate above.
- **The holder's command** `external-lane-export` and `external-lane-export-status`, with the Python helper.
  - Every refusal the command can know from the node's current state happens before signing: the lane's remaining amount with pending releases and exports, the balance, the sender's sequence including a pending later-family transaction, and the mempool limits.
  - A dry run signs nothing and opens no key file.
  - The Python helper refuses a quote that differs from the request, and a fee above a ceiling (default 92 atoms, four times the largest fee an export can be charged today).
- **Hard checks at the merge:** the deployed pfUSDC proof source is byte for byte unchanged; the guest's dependency closure did not change.

**Not merged: the watcher's export view** on [`token/lane-watcher-exports`](https://github.com/postfiatorg/postfiatl1v2/tree/token/lane-watcher-exports) at `e983568b6`

- With `--exports` it checks every finalized payout of the lock on Base against a PFTL export packet, and keeps a record of paid exports in its state file.
- Six readings today. Five of them each found a further case in how the view keeps, rescans or identifies its state, each fixed in the following round. A model-based test now enumerates sequences of node answers; the last reading ran 134,089 sequences on one unchanged network and found none in which a second payout, or a payout without an export, passes without an alarm.
- The fifth reading found a case of another kind. The view reads the lane twice in a run, and only the first read was compared with the network, lane and lock the state file belongs to, so export pages answered by a node of another network were accepted. The fix is on the branch: `5d384038c` compares every export page with the identity the first read validated and refuses a page of another network as `export_identity_mismatch`; `e983568b6` is a wording fix. A sixth reading confirmed both.
- That reading then found the same kind of gap on the Base side, older than today's work. After the one chain-id check at the start of a run, the later single Base answers (the logs, the lock's totals) are not tied to that chain or to a block hash, so one answer from a node of another network could make a payout look backed.
- Closing it is a design step, not one more patch: either every Base read of a run anchored to the validated chain and a block hash, or the stable single-network endpoint stated as an operating assumption. So the view stays on its branch. A small output point goes with it: an export created between the run's two reads is checked but not printed.

**A new precondition for activating the lane export root** (on main, activated nowhere)

- `4ea464d56` "Require an inventory of fixed-key finality programs before the lane export root is activated".
- From the activation height on, every block uses a new block-id form, including blocks with no export. A proof program built before that change knows only the older forms, and a verifier contract that pins such a program's key cannot be given another program. So a route whose verifier pins one could not prove any block finalized after the activation.
- The [activation runbook](../runbooks/external-lane-export-root-activation.md) now starts with an inventory and a rule.
  - In its first table, the Ethereum mainnet pfUSDC and pfETH, the Arc and the Sepolia verifiers predate the change. The rows for the A666 export route and the lane export program say "not established".
  - The rule: no activation while such a route can still owe a withdrawal. A long-lived verifier built from now on should pin a program that accepts the block-id forms of every activation already scheduled.
- Nothing is activated, so nothing is affected today. Private note: `lane-export-root-activation-and-fixed-key-programs.md` in the index directory above.

**Z3 NAVCoin round trip**

- `c20d3e549` + `532160871` + `53333982a` + `370ed18b6`:
  - the selected route's state read back on all six validators at height 1177 and frozen ([`z3-g2-selected-state-20261111.json`](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/z3-g2-selected-state-20261111.json));
  - its identities replayed offline by a test ([`z3_g2_selected_state_replay.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/bridge/tests/z3_g2_selected_state_replay.rs));
  - the G3 tooling re-run on today's main (the dry run resolves all 39 commands and submits nothing);
  - the [one-cycle review packet](../review/z3-cycle1-review-packet-20261111.md).
- Gate G2's last line stays open: everything that exists reads back and replays, but the reserve-proof profile the cycle needs is not on chain until it is registered and bound, and the route advance that then adds the Arc custody row changes the identities once more. See the [Z3 plan, "Status 2026-11-11"](../plans/active/z3-navcoin-roundtrip-plan.md#status-2026-11-11).
- No cycle ran; G4 is not authorized.

**For the Mac wallet** (the 2026-11-07 wallet handoffs)

- **`owned_objects` paging** ([RPC methods, "Owned objects pages"](../rpc/methods.md#owned-objects-pages); [Web wallet](../wallets/web-wallet.md)):
  - `782668ea1`, `030b75f9e`, `3831f9f0d`; after a review `37f3bc03d`, `27cea3dd9`, `4b341f55e`, `6ac9b93f7`, `f8dae2713`, `655ea7216`;
  - for three consumers that treated one page as the whole inventory (the gateway's wallet feed, the web wallet's FastPay inventory, the Python wallet's wrap, unwrap and send): `e9b554cb1`, `4da396c5c`, `d96d70ce6`, `40581d6d5`.
  - What it adds: a cursor, a page bound of 960 KiB, an optional `omit_owner_keys`, and a `view_digest` that binds every page to the object view it was read from. A FastPay apply between two pages now refuses the continuation (`owned_objects_view_changed`) instead of yielding a mixed set.
  - It runs through the node, the Rust SDK's typed request, the gateway, the Python client (`owned_objects_all`) and the web wallet.
  - One behaviour change for an old client: a request for more than about 235 keyed objects gets a shorter page with `truncated: true`.
- **The signed fence status** (the two "terminal proofs"), built and not merged: [`w1/signed-fence-status`](https://github.com/postfiatorg/postfiatl1v2/tree/w1/signed-fence-status) at `8c9cee23c`.
  - `owned_fence_status` returns a status that each recovery-committee validator signs only from certified-block state, under its own signing context. The client check exists in Rust and Python, which share one acceptance-vector file.
  - Two reviews are in and their fixes are on the branch: a proof now stands when enough distinct committee members agree, and a contradicting minority is reported instead of refusing the whole set; the gateway has a per-address and a global budget for this read.
  - The whole workspace builds on the branch, and the node's FastPay, owned-object and signer test groups pass. It merges after one more reading.
  - For the hardened signer (W-1 S7.1): on the two-user signer shape no FastPay vote, acknowledgement or fence status can be signed at all, because those are signed inside rpc-serve from the key file that shape takes away. The signer process would need a design that lets it check certified-state evidence.

**The Python client signs what was asked** (`39389919c` + `4af863566` + `ce238378b` + `7dc225c55`; [Exchange guide, "What the Python client checks before it signs"](../integrate/exchange-guide.md#what-the-python-client-checks-before-it-signs))

- Every signing helper (asset, escrow, NFT, offer, `send_pft`, the latency tool) compares the node's quote with the request before the signer runs. It refuses a difference, a malformed monetary field, or a fee above a ceiling per transaction family.
- `create_offer` and `place_offer` now require `max_fee_atoms`, because an offer's match fee depends on the order book.
- The Rust signer's commands for asset, escrow, NFT and offer transactions still sign from the quote alone; a request-comparing variant is a follow-up.
- A second reading this afternoon confirmed that a changed recipient, asset, amount or fee is refused on every route it read. Four points are not fixed yet:
  - the rule that accepts a field present on one side only when it is null, empty, 0 or false is wider than the operations' real defaults (for an already invalid request, such as an asset with `max_supply: 0`, a node's answer without the field passes); it should become a per-field list;
  - honest empty-list defaults (`issued_payment_v2.memos` and five more fields) are refused;
  - the latency tool does not validate a caller's own fee ceiling;
  - four test fixtures do not use the real operation shapes.
- The same reading on the paging consumers: a partial page never replaces the inventory. Two points are open:
  - the web wallet can publish an older complete view after a newer one when a feed answer and a full reread cross;
  - the Python `wrap` names its new object by comparing two inventories, which goes wrong when another object of the same value arrives in between. This is older than today's change; the object id can be derived from the deposit.

**Deployment state** (see [Current State](../status/chain-state-current.md), recorded on main in `5e21a24c0` "Record the 11 November fleet observation at height 1177")

- **Last observed fleet state:** read only, all six validators at height 1177, one state root, empty mempool. The reads (`status`, `mempool_status` and the route reads) ran at 08:20Z and 10:00:58Z–10:01:18Z.
- **Deployed binary and lineage:** release `vote-records-20261030`, unchanged.
- **Repository HEAD:** `main` at this handoff's commit; the public and private branches are at the tips named above.
- **Merged but undeployed:** everything on main above.
- **Live probe:** the read-only reads above. No deployment, restart, signature or transaction.

**Task Node** (each id read at 14:09Z; all six show Rewarded)

- `task_d6b969b53c21e1e27697e9cc57be8476` (the NAV proof watchdog);
- `task_1bea219a6be29e703fcd8de9b8292686` (`owned_objects` paging);
- `task_801a8fbbdc4e004fe284abcb71bbe8c4` (the Z3 packet);
- `task_450a44d29455428cf2b7d348e7a4216c` (the fence status; Rewarded although its branch is not merged);
- `task_626f3103934d8e2d215460797333945e` (the merge);
- `task_edb171f7645c3481e80918b2ab423871` (the Python client's checks).
- The work in the private repository ran without Task Node.

**Public CI at the time of writing** (main's run list, 14:10Z)

- **`docs-build`, `rust-ci`, `product-security-ci`:** green on every one of today's commits from `69c6c2e32` through `40581d6d5`.
- **`pft-lane-export-identity`:** green on every main commit since the merge, `82d921f90` through `40581d6d5`.
- **On `5e21a24c0`** (status pages only): `docs-build`, `pft-lane-export-identity` and two `product-security-ci` jobs are red in their package-install steps, because the runners could not resolve the Ubuntu package mirrors. They need a rerun. `rust-ci` was still running.

## Next decision or action

Ours first:

1. **The watcher's export view:** anchor every Base read of a run to the validated chain and a block hash (and check whether the release view on main reads Base the same way), and print exports created between the run's two reads. Then a reading, and the merge with a rerun of the local round trip on the merged tree.
2. **The signed fence status:** one more reading, then the merge. Then the remaining wallet asks by value: native-session hardening, and the route authority epoch in `navcoin_bridge_routes`.
3. **The open points of this afternoon's second reading:** the per-field default list and the empty-list defaults in the Python client's quote check, the latency tool's ceiling, the test fixtures; the web wallet's inventory ordering; the Python `wrap` object id.
4. **The Base Sepolia rehearsal command:** bind the release to the checked relayer on a resumed run and settle the ceiling rule, with a reading, before any release that carries a real proof. A real run also needs a prover.
5. **The Rust signer variant** that compares the quote with the request.
6. **The activation runbook's inventory row for `pft-lane-export`** still says "not on main"; the merge at `82d921f90` made that stale.
7. **Read CI on today's last pushes,** and rerun the red runs on `5e21a24c0`.

Then the client's, each with the date first asked.

- **New today (2026-11-11), each detailed in the private notes named above:**
  - the open design choices of the holder migration, and the owner inputs of the successor deployment;
  - when the NAV observation time rule is activated, and a fresh pull of the six real-data copies for the replay that release still owes (the copies of 4 November are no longer on the work server);
  - who authorizes route reconciliation records;
  - the rows of the activation inventory that only the owner can settle (which older verifiers can still owe a withdrawal).
- **The audit vendor and budget for the token and lock contracts** (2026-10-26 / 2026-11-06). The shortlist of three firms and a request text went over on 2026-11-10.
- **What gate G2 and the first NAVCoin cycle wait on:** the reserve-proof successor registration (2026-09-24); a governed A666 NAV profile bound to a registered reserve-proof profile with fresh proofs (2026-09-22); then the route advance with the Arc custody row (2026-09-28).
- **The rollout go, the keyed rehearsal on the validator hosts, the six migration signatures and the migration-trust descriptor** (2026-11-02/03/04).
- **The wrapped-token path in writing** (2026-11-04).
- **The Phase 3 decisions of the design note's section 9**, including who runs the prover and the relay, and the fixed-verifier choice (2026-11-05 / 2026-11-10).
- **The three red CI jobs on the next-release branch**, and our fix branch `security/next-release-ci-fixes-20261106` (2026-11-06).
- **The reachable host of the audit and operations machine** (2026-11-04).
- **The six cached checkpoint votes** (2026-11-04).
- **The old proving service and the faucet key** (2026-11-02).
- **The two issuer signatures** (2026-09-25 / 2026-10-01).
- **The `ETHEREUM_MAINNET_RPC_URL` secret** (2026-09-28).
- **From the 2026-11-07 wallet handoffs** (listed 2026-11-10): review and pin the FastPay committee file in a signed build; switch native-session admission on at the gateway; private admission and pricing-route activation on the live network; which "old Ethereum controller" the handoff means.

## References

- **Phase 3:** [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md) (P3.1); [next validator release plan](../plans/active/next-validator-release-plan-20261115.md); [design note](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md); [Genesis external lanes, "Exports"](../architecture/genesis-external-lanes.md#exports); [export root activation runbook](../runbooks/external-lane-export-root-activation.md); [Base Sepolia dry-run runbook](../runbooks/lane-base-sepolia-dry-run.md); [program README](https://github.com/postfiatorg/postfiatl1v2/blob/main/programs/pft-lane-export/README.md).
- **Z3:** [Z3 plan, "Status 2026-11-11"](../plans/active/z3-navcoin-roundtrip-plan.md#status-2026-11-11); [one-cycle review packet](../review/z3-cycle1-review-packet-20261111.md).
- **NAVCoins:** [NAVCoins](../navcoins/index.md); [NAV proof watchdog README](https://github.com/postfiatorg/postfiatl1v2/blob/main/tools/nav-proof-watch/README.md).
- **Wallet and client:** [RPC methods](../rpc/methods.md) (`owned_objects`); [Web wallet](../wallets/web-wallet.md); [Exchange guide](../integrate/exchange-guide.md).
- **Public branches:** [`token/lane-watcher-exports`](https://github.com/postfiatorg/postfiatl1v2/tree/token/lane-watcher-exports); [`w1/signed-fence-status`](https://github.com/postfiatorg/postfiatl1v2/tree/w1/signed-fence-status).
- **Private repository** (`postfiatorg/postfiatl1v2-private-archive-20260717`): `docs/review/client-lane-branch-reviews-20261111/README.md` on `docs/client-lane-branch-reviews-20261111`; the branches `navcoins/successor-deployment-rehearsal-20261111`, `navcoins/nav-time-rule-activation-20261111` and `pfusdc/route-reconciliation-gate-fixes-20261111`.
- **Status:** [Current State](../status/chain-state-current.md).
- **Previous handoff:** [2026-11-10: lane export header root merged and first local round trip of PFT back to Base](2026-11-10___dravlic__lane_export_header_root_merged_and_first_local_round_trip_of_pft_back_to_base.md).
