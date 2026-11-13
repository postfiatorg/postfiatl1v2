# B23 line joined with main and deposit route checked

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-13 UTC

## BLUF

- **The devnet runs the B23 line:** all six validators run the private branch `b23/integration`, revision `abc228eda` of 13 November 00:35Z, at height 1271 with one state root. That line split from main on 2026-09-28, so the running build lacks main's changes since then, and a release from main alone can no longer follow this chain.
- **I joined the two** on the private branch `integration/main-with-b23-20261113` (tip `145426666`). It replays the live chain from genesis to 1271 and ends on the live state root, its wide test run passes, and its proof input inventory check passes, so one release can carry both the B23 work and main. It holds main up to `e7efd6fa1`; today's later main merges are not in it yet.
- **The overnight deposit-route fix:** a correctness reading found it complete. The light client now starts from a pinned header and signer committee, and the live route matches the committed program.
- **Our redelivery fix** (for a validator that stops proposing after a lost acknowledgement) is offered to the B23 line as private PR #28.
- **On main today:** the gateway test repaired, an outside fix merged, the route authority flag for the wallet, the watcher's PFTL snapshot binding, two small follow-ups, and the status pages at 1271.

Primary references: [Current State](../status/chain-state-current.md), [release plan](../plans/active/next-validator-release-plan-20261115.md).

## Current state

**Devnet** (read only, not changed by me; [Current State](../status/chain-state-current.md), recorded in `180417fbb`)

- **Last observed fleet state:** all six at height 1271, one state root `900fedd4…`, empty mempools (`status` at 10:51Z).
- **Deployed binary and lineage:** build `abc228ed` from the private `b23/integration`; manifest `19e74906…`; `deployment_manifest_verified` reads false. No deployment receipt for it is in the repository.
- **Since 1209:** 62 transactions, all accepted, including the B23 market's first sale and fill (1269–1271) and the activation of the route `ethereum-mainnet-usdc-v1` at route epoch 11 (block 1256).
- **Repository HEAD:** `main` at this handoff's commit. **Merged but undeployed:** everything on main below. **Live probe this session:** none beyond the 10:51Z read; nothing was written, signed or restarted.

**Three governed rules the running build does not apply**

- Fees on rejected transactions, the recipient address format and require-memo are switched on by governance from height 1153 ([Current State](../status/chain-state-current.md), the 2026-10-23 entry).
- None of them came into play in blocks 1153–1271, which is why every root still agrees. The joined line applies them.
- So the next rollout should go to all six at once, before a transaction that one of these rules covers.
- Neither the running build nor main alone can replay this chain from genesis; the joined line can.

**The joined line** (private, `integration/main-with-b23-20261113`, tip `145426666`)

- The merge `26ee7a34b` (main `e7efd6fa1` with `abc228eda`) keeps both sides in the five conflicted files.
- `c4b10c605` reads the older 26-field Ethereum ingress values for history, only on a route profile without a beacon anchor; an anchored profile refuses them.
- **Replay:** `verify-blocks` from genesis to 1271 passed and `verify-finalized-checkpoint` at 1271 verified (quorum 5 of 6), on a read-only copy of one validator's data, deleted afterwards.
- **Wide test run:** 0 failures, after one merge-caused test fix (`f50008ba1`). Types, execution, storage, rpc-sdk, bridge and wallet-wasm 952; node library and binaries 1,029; node integration tests 36, the usual ignored ones skipped.
- **Proof input inventory** (111 sources): it pins the pfUSDC ingress program the live route uses (key `0x00d8cbf0…`, ELF `b6a06cb9…`), the four sources the B23 line changed with a note each, and five more statement sources. It records the binding assumptions explicitly and marks the pfETH program "rebuild before its route is used"; that route is not active on the devnet.
- **Still needed before a release from it:** a reproducible rebuild of the ingress program (a machine with Docker), a transport-validator boot on a copy, the release packet, then the all-six rollout.

**The deposit route** (a correctness reading of `abc228eda`)

- The light client starts from the governance-pinned header and committee; the committee is inside the bootstrap commitment the validators sign.
- Rollover goes only through authenticated committees, and the anchor only moves forward.
- The live route's program key equals the committed ELF.
- A credit equals a deposit record proven in the governed vault's own storage, once.
- Four follow-up points for the B23 line, and one note on concurrent deposits, are in the private note `docs/review/client-lane-branch-reviews-20261111/` on `docs/client-lane-branch-reviews-20261111` (`2759b42a7`).

**Private PR #28** against `b23/integration` (branch `b23/redelivery-ack-20261113`, `208ee8687`)

- It brings main's redelivery fix (`569f5f73b`) to the B23 line. Its six new tests fail on `abc228eda` and pass with it. No state, signing or consensus rule changes.
- The joined line already carries the fix through main.

**The blog post** (website PR #29): I left a comment with nine factual corrections, each with a suggested sentence; for example, the post does not say B23 runs on the test network.

**Merged on main today**

- `e7efd6fa1`: the gateway integration test opens a new native session after a restart; `rust-ci` had been red since `e23a6dced`.
- `3c1fe80a6`: outside [PR #202](https://github.com/postfiatorg/postfiatl1v2/pull/202); the monitor warns when any endpoint lacks proof-latency data.
- `68e9472ae` + `9b5af506a`: `navcoin_bridge_routes` rows report the route's authority epoch and whether it matches the newest governed checkpoint committee, which is not update eligibility; the Rust SDK accepts both route report schemas ([RPC methods](../rpc/methods.md)).
- `eb599f736` + `904c0dc87`: the watcher binds its PFTL export pages to one PFTL block, joins carried payouts by digest, refuses a snapshot behind the release view, and warns after repeated refusals ([Genesis external lanes](../architecture/genesis-external-lanes.md)).
- `7fe7e37ef`: a 4 MiB limit on the native-session list files.
- `dc4fbd51a`: the Base Sepolia rehearsal prints the planned release spend and recommends a small relayer surplus.
- `180417fbb`: the status pages at 1271.
- Nothing was sent to Base Sepolia or any public network.

**Not merged:** [`w1/fastpay-inventory-revision-20261113`](https://github.com/postfiatorg/postfiatl1v2/tree/w1/fastpay-inventory-revision-20261113) (`b1c813f8c`)

- `owned_objects` pages carry an inventory revision and the answering node id; the gateway groups validators without them and passes them on.
- A reading found the revision can repeat after a restore from an older snapshot and does not move on legacy applies, so the node side gets a fix round first.
- The wallet does not use the fields yet.

**Task Node** (read at 14:11Z with `corbanu tasknode tasks list`; today's six all show Rewarded, none in verification)

- the route authority epoch: `task_928144a`, and its merge `task_088f020`;
- the watcher snapshot binding: `task_41b9093`, and its fix round `task_947717f`;
- the inventory revision: `task_2859470`;
- the rehearsal sheet and runbook follow-ups: `task_32ad13b`.
- The private-repository work ran without Task Node.

**CI on main** (`gh run list --branch main --limit 24` at 14:10Z; it covers `3c1fe80a6` through `dc4fbd51a`)

- **`docs-build`** and **`pft-lane-export-identity`:** green on every commit.
- **`rust-ci`:** green on `3c1fe80a6`, `180417fbb`, `9b5af506a` and `7fe7e37ef`; running on `904c0dc87` and `dc4fbd51a`.
- **`product-security-ci`:** green on `3c1fe80a6`, `180417fbb` and `9b5af506a`; running on `904c0dc87`, `7fe7e37ef` and `dc4fbd51a`.
- `e7efd6fa1`, just outside that list, is green on all four workflows.

## Next decision or action

Ours first:

1. **The joined line to a release:** the reproducible ingress rebuild, a transport-validator boot on a copy, the release packet; then a rollout to all six at once, which needs a go. The security release candidate (main of 4 November plus its repairs) must now be rebuilt on top of the joined line, because a release without the B23 state can no longer follow this chain.
2. **The deposit route's follow-up points** from the private note, built on the joined line with failing tests first.
3. **The inventory revision:** the node-side fixes (a new incarnation on restore, the revision only where every mutation advances it, no read during a pending ordered commit), a re-read, the merge; then the web wallet orders its coin list by it.
4. **The pfETH entry's binding wording** in the inventory.

Then the client's, each with the date first asked:

- **New 2026-11-13:** could the next B23 commits and rollouts be based on `integration/main-with-b23-20261113`, so the two code lines stay one? PR #28 only matters for a rollout from `b23/integration` before that.
- **New 2026-11-13:** could the corrections on website PR #29 go in before the post goes out?
- **The audit vendor and budget for the token and lock contracts** (2026-10-26 / 2026-11-06).
- **The rollout go, the keyed rehearsal and the signatures for the security release** (2026-11-02/03/04), now on top of the joined line.
- **The wrapped-token path in writing, and the successor's design choices and owner inputs** (2026-11-04 / 2026-11-11).
- **The reserve-proof successor registration** (2026-09-24).
- **Answered** (asked 2026-11-12): the source of the 12 November releases is now on the `b23/*` branches. Could a deployment receipt for the releases since then be added?
- **What gate G2 and the first NAVCoin cycle also wait on:** a governed A666 NAV profile bound to a registered reserve-proof profile with fresh proofs (2026-09-22); then the route advance with the Arc custody row (2026-09-28).
- **From 2026-11-11:** when the NAV observation time rule is activated, and a fresh pull of the six real-data copies for the replay that release still owes; who authorizes route reconciliation records; the rows of the activation inventory that only the owner can settle.
- **The Phase 3 decisions of the design note's section 9**, including who runs the prover and the relay, and the fixed-verifier choice (2026-11-05 / 2026-11-10).
- **The three red CI jobs on the next-release branch**, and our fix branch `security/next-release-ci-fixes-20261106` (2026-11-06).
- **The reachable host of the audit and operations machine** (2026-11-04).
- **The six cached checkpoint votes** (2026-11-04).
- **The old proving service and the faucet key** (2026-11-02).
- **The two issuer signatures** (2026-09-25 / 2026-10-01).
- **The `ETHEREUM_MAINNET_RPC_URL` secret** (2026-09-28).
- **From the 2026-11-07 wallet handoffs** (listed 2026-11-10): review and pin the FastPay committee file in a signed build; switch native-session admission on at the gateway; private admission and pricing-route activation on the live network; which "old Ethereum controller" the handoff means.

## References

- **Today's merges on main:** `e7efd6fa1`, `3c1fe80a6`, `68e9472ae`, `9b5af506a`, `eb599f736`, `904c0dc87`, `7fe7e37ef`, `dc4fbd51a`; the status record `180417fbb`.
- **Status:** [Current State](../status/chain-state-current.md).
- **Plans:** [next validator release plan](../plans/active/next-validator-release-plan-20261115.md).
- **Docs:** [Genesis external lanes](../architecture/genesis-external-lanes.md) (the watcher); [RPC methods](../rpc/methods.md) (`navcoin_bridge_routes`).
- **Public branch:** [`w1/fastpay-inventory-revision-20261113`](https://github.com/postfiatorg/postfiatl1v2/tree/w1/fastpay-inventory-revision-20261113) (`b1c813f8c`).
- **Private repository** (`postfiatorg/postfiatl1v2-private-archive-20260717`): `integration/main-with-b23-20261113` (`145426666`); `b23/redelivery-ack-20261113` (`208ee8687`, PR #28) against `b23/integration` (`abc228eda`); `docs/client-lane-branch-reviews-20261111` (`2759b42a7`); the B23 line's own handoff `docs/plans/HANDOFF-navcoin-metamask-roundtrip.md` on `b23/integration`.
- **Previous handoff:** [2026-11-12: overnight release recorded, and fence status, watcher and delivery fix merged](2026-11-12___dravlic__overnight_release_recorded_and_fence_status_watcher_and_delivery_fix_merged.md).
