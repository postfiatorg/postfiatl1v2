# Overnight release recorded, and fence status, watcher and delivery fix merged

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-12 UTC

## BLUF

- **The devnet:** last night four releases were rolled to all six validators. The running one is `r2-1-activation-bound-20261112` (source revision `5d13c775e408…`), restarted 01:53–01:56Z. I recorded the fleet at height 1209 on the status pages (`2f35de745`). The four revisions are not on a branch of either repository yet.
- **Six changes landed on main after a correctness review:** the idempotent acknowledgement of a redelivered certified block (`569f5f73b`); the validator-signed FastPay fence status (`c1a8a7f7a`); the watcher's export view with every Base read bound to one anchor block (`32c7db71b`; the local round trip passes on the merged tree); the typed quote check and the wrap object id from the signed deposit (`8fd2355fd`); the native-session hardening (`e23a6dced`); the Base Sepolia rehearsal's relayer binding and release budget (`2c50a17b9`).
- **The lane export program's committed identity** was rebuilt after the fence status merge grew `crates/types` (`5be9d42ab`). Nothing was deployed with the old key.
- **Private:** the successor's route settings exist on the private branch. Notes for the product owner (the overnight fleet read in full, and one more check) are in the private repository, `docs/client-lane-branch-reviews-20261111` (`ade6d1da1`).

Primary references: [Current State](../status/chain-state-current.md), [release plan](../plans/active/next-validator-release-plan-20261115.md).

## Current state

**Devnet** (read only, not changed; [Current State](../status/chain-state-current.md), recorded in `2f35de745`)

- **Last observed fleet state:** all six at height 1209, one state root `e101f848…`, view 0 at every height 1178–1209, empty mempools (`status` at 10:50Z; host and chain reads 11:06–11:30Z).
- **Deployed binary and lineage:** the binary, the units and the environment files match the signed manifest on all six. `deployment_manifest_verified` reads false because this build does not write the verified-deployment record.
- No deployment receipt is in the repository for the four releases (`r2-1-beacon-anchor`, `r2-1-anchor-bound`, `r2-1-kind-bound`, `r2-1-activation-bound`). Their source revisions are not on any branch we hold.
- **Repository HEAD:** `main` at this handoff's commit. **Merged but undeployed:** everything on main below. **Live probe this session:** none.

**The redelivery fix** (`569f5f73b`; [release plan](../plans/active/next-validator-release-plan-20261115.md), "For the release after")

- Before: the target treated a redelivery as already applied only when the old JSON store's error text said so. The transactional store answers `storage_duplicate_record … already exists`, so one lost acknowledgement left a sender's job pending and that node stopped proposing.
- Now a block held with the same content is acknowledged `already_applied`: certificate fields, the consensus-v2 commit, the FastPay effects, chain, genesis, protocol version and certificate schema. Different content is `certified_send_duplicate_conflict` (`crates/node/src/transport_protocol.rs`).
- It rides along with the next release cut from main. Until then the hosts' own workaround is still needed.
- The second report from the same hosts (account history failing on a repeated governance receipt id) is handled on main since `d9260e937`. The running build does not carry it.

**The fence status** (`c1a8a7f7a`; the second chain-side ask of the 2026-11-07 wallet handoffs; [Web wallet](../wallets/web-wallet.md))

- Validator-held FastPay committee keys sign `FastPayFenceStatusV2` from certified-block state under the context `postfiat-l1-v2/fastpay-fence-status/v2`.
- A quorum of distinct members with the same decision, operation and fence digest proves an answer. A "not terminal" status contradicts a terminal quorum only at or above the (f+1)-th smallest bound of its signers. A terminal status contradicts a "not terminal" quorum only at or below the (f+1)-th largest observed height.
- Rust and Python share one acceptance-vector file with 68 cases (`crates/types/test-vectors/fastpay_fence_status_acceptance_v2.json`).
- It signs a new message, so the release plan says it **needs its own qualification**.

**The watcher** (`32c7db71b`; [Genesis external lanes, "The export view"](../architecture/genesis-external-lanes.md#the-export-view))

- Every Base read of a run is bound to one finalized anchor block by hash. A log's block, and a differing source block, must also be the canonical block at its number.
- The Base endpoint must support EIP-1898 by-hash reads with `requireCanonical`, and must be archive-capable.
- Stated assumption: the PFTL endpoint is a node of the validated fleet.

**The wallet**

- The fixes A1–A4 and B2 are on main in `8fd2355fd`.
- Not merged: the web wallet's coin-list ordering, on [`wallet/second-reading-fixes-20261112`](https://github.com/postfiatorg/postfiatl1v2/tree/wallet/second-reading-fixes-20261112) (tip `8a799d2a0`).
  - Three rounds of a conservative rule (publish only confirmed reads; the feed view as a constraint; a memory of replaced views) each closed the cases found before and opened new ones. A bounded memory forgets, and an honest inventory can recur (empty, then a deposit, then empty again).
  - The branch's design note, [`fastpay-inventory-ordering.md`](https://github.com/postfiatorg/postfiatl1v2/blob/wallet/second-reading-fixes-20261112/docs/wallets/fastpay-inventory-ordering.md), describes the real fix: the node returns an inventory revision with each answer, the gateway passes on each answering validator's revision, and the wallet compares answers only by node and revision.

**Also merged**

- **The native-session hardening for the Mac wallet's gateway** (`e23a6dced`):
  - the allowlist and a denylist re-read and hashed on change, strict entries, revoked installs dropped at once, a live-install cap;
  - a boot generation in each token, so a gateway restart asks every wallet to reopen;
  - one audit line per open without credentials, and an info route.
  - Switching native sessions on at the live gateway stays the operator's.
- **The Base Sepolia rehearsal's relayer binding and release budget** (`2c50a17b9`). Nothing was sent to Base Sepolia.
- **The lane export program identity** (`5be9d42ab`): ELF `8ac7696a…`, key `0x00d5ebeb…`. A new `releases.json` entry; the old one is kept.

**Private** (`postfiatorg/postfiatl1v2-private-archive-20260717`)

- `navcoins/successor-deployment-rehearsal-20261111` (tip `99ce71059`) now has the `routes` stage and `routes --check`.
- All five prepared pieces of the successor work exist. Switching a route is code work in the node and the wallet after the token path is chosen.

**Task Node** (read at 14:12Z with `corbanu tasknode tasks list`; all twelve show Rewarded, none in verification)

- the watcher anchor: `task_4c7bad6`, and its canonical-membership round `task_fe36b70`;
- the redelivery fix `task_c840deb` and its follow-up `task_874fe4b`;
- the fence status's two rounds: `task_9ff9ff2`, `task_73c2630`;
- the wallet fixes: `task_348b3cb`;
- the coin-list rule: `task_f5bec13`, `task_8b4d67f`;
- the native-session hardening `task_04ddbb1` and its fix round `task_3d115ee`;
- the relayer binding and budget: `task_0c6f24f`.
- The route checks and the rest of the private-repository work ran without Task Node.

**CI on main** (run list read at 14:11Z, today's commits `2f35de745` through `c786f41f0`)

- **`docs-build`:** green on every commit.
- **`pft-lane-export-identity`:** green on `2f35de745` and `8fd2355fd`. Red on `c1a8a7f7a`, `32c7db71b`, `2181cc42c`, `569f5f73b` and `26a333102`, the commits between the fence status merge and the identity rebuild. Running on `5be9d42ab` and after.
- **`product-security-ci`:** green on `2f35de745` and `8fd2355fd`; running on the later commits.
- **`rust-ci`:** green on `2f35de745`; running on `8fd2355fd` and after.
- Yesterday's red runs on `5e21a24c0` (the runners' package-mirror name lookup) passed on rerun.

## Next decision or action

Ours first:

1. **The coin-list ordering:** build the node-side inventory revision of the design note instead of a fourth interim rule. The node already journals an apply sequence per FastPay effect; no read returns it yet. Then the wallet orders by it.
2. **Small follow-ups of today's readings:** a size limit on the native-session list files; the rehearsal runbook's funding wording and the planned spend in the deploy sheet.
3. **The lane export program:** a narrow proof-types crate, so unrelated changes in `crates/types` no longer move its identity, and the identity workflow as a gate before merges that touch `crates/types`.
4. **The watcher:** bind export pages to an independently confirmed PFTL snapshot, and join a carried payment to its export by digest, not by nonce alone.
5. **The fence status's `inconsistent_bounds`:** a neutral name. The note that an entry can be an honest, tighter value is on main since `26a333102`.

Then the client's, each with the date first asked:

- **New 2026-11-12:** could the source of the four releases rolled to the devnet last night (`r2-1-beacon-anchor`, `r2-1-anchor-bound`, `r2-1-kind-bound`, `r2-1-activation-bound`; running revision `5d13c775e408…`) be pushed to a branch of the private repository, with a deployment receipt? Then what runs can be read, and the status page's repository row updated.
- **The audit vendor and budget for the token and lock contracts** (2026-10-26 / 2026-11-06). The shortlist of three firms and a request text went over on 2026-11-10.
- **The rollout go, the keyed rehearsal on the validator hosts, the six migration signatures and the migration-trust descriptor for the security release** (2026-11-02/03/04), to be re-read against what now runs.
- **The wrapped-token path in writing, and the successor's design choices and owner inputs** (2026-11-04 / 2026-11-11).
- **What gate G2 and the first NAVCoin cycle wait on:** the reserve-proof successor registration (2026-09-24); a governed A666 NAV profile bound to a registered reserve-proof profile with fresh proofs (2026-09-22); then the route advance with the Arc custody row (2026-09-28).
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

- **Today's merges on main:** `569f5f73b`, `c1a8a7f7a`, `32c7db71b`, `8fd2355fd`, `e23a6dced`, `2c50a17b9`; the identity rebuild `5be9d42ab`; the status record `2f35de745`.
- **Status:** [Current State](../status/chain-state-current.md).
- **Plans:** [next validator release plan](../plans/active/next-validator-release-plan-20261115.md).
- **Docs:** [Genesis external lanes](../architecture/genesis-external-lanes.md) (the watcher); [Web wallet](../wallets/web-wallet.md) (the fence status, native sessions); [lane export program README](https://github.com/postfiatorg/postfiatl1v2/blob/main/programs/pft-lane-export/README.md).
- **Public branch:** [`wallet/second-reading-fixes-20261112`](https://github.com/postfiatorg/postfiatl1v2/tree/wallet/second-reading-fixes-20261112) (`8a799d2a0`).
- **Private repository** (`postfiatorg/postfiatl1v2-private-archive-20260717`): `docs/client-lane-branch-reviews-20261111` (`ade6d1da1`); `navcoins/successor-deployment-rehearsal-20261111` (`99ce71059`).
- **Previous handoff:** [2026-11-11: successor deployment rehearsed on a fork, and export program and relay merged](2026-11-11___dravlic__successor_deployment_rehearsed_on_a_fork_and_export_program_and_relay_merged.md).
