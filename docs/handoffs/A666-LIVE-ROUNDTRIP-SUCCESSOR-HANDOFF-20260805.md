# A666 LIVE ROUND-TRIP — SUCCESSOR HANDOFF — 2026-08-05

> **READ THIS FIRST**
>
> The human's actual objective is a real live-money round trip using the already-deployed literal A666 route:
>
> **USDC → pfUSDC → A666/wA666 → Uniswap → return to PFTL → redeem to pfUSDC → bridge back to external USDC.**
>
> Literal A666 already exists. The user currently holds **103.000000 wA666** in the Ethereum wallet below. Do not rebuild, reset, replace, or provision a different chain merely to prove that A666 exists.
>
> This predecessor made a serious error: it confused a newer local six-validator lightning test fleet with the existing A666 fleet and spent hours following stale profile/preflight failures on the wrong environment. The successor must begin from the live Ethereum balance and the existing A666 deployment/receipt evidence below.

---

## 1. Human directive and communication requirement

The user wants the existing live A666 flow completed end to end. They are angry because the predecessor repeatedly described the already-completed A666 export as if it had never happened.

Required communication style:

- Speak in plain English.
- Lead with the live result or exact blocker.
- Never say A666 was never deployed or never exported.
- Never call a mocked five-leg test a live demo.
- Never ask the user to locate internal key files.
- Ground every claim in an on-chain read, a signed receipt, a current process probe, or an exact file path.
- No long speculative network-recovery detours before answering the pending question.

All earlier live-fire approvals should be treated as **superseded by this handoff request**. The five packets remain HELD. Obtain fresh approval only after a current, correct A666-route preflight is presented.

---

## 2. Facts already verified on 2026-08-05

### 2.1 Current Ethereum wA666 balance

Wallet:

`0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0`

wA666 token:

`0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5`

Live Ethereum reads returned:

- raw balance: `103000000`
- decimals: `6`
- human balance: **103.000000 wA666**

Three independent Ethereum RPCs agreed at blocks 25,691,659–25,691,660:

- `https://ethereum-rpc.publicnode.com`
- `https://1rpc.io/eth`
- `https://eth.drpc.org`

Re-run:

```bash
TOKEN=0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5
OWNER=0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0
cast call "$TOKEN" 'balanceOf(address)(uint256)' "$OWNER" \
  --rpc-url https://ethereum-rpc.publicnode.com
cast call "$TOKEN" 'decimals()(uint8)' \
  --rpc-url https://ethereum-rpc.publicnode.com
```

### 2.2 Literal deployed A666 route

Route ID:

`pftl-a666-ethereum-wA666-usdc-v1`

PFTL native A666 asset ID:

`521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c`

pfUSDC settlement asset ID:

`02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b`

Ethereum contracts:

- controller / settlement adapter: `0x9A0262C0572fb4DB08765408eB225E207F40c3d9`
- wrapped token: `0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5`
- verifier: `0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A`
- Uniswap universal router: `0x66a9893cC07D91D95644AEDD05D03f95E1dBA8Af`
- Uniswap pool/path identifier:
  `0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98`

Canonical config:

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/deployments/a666-mainnet-20260727/09-production-route-config.json`

Current fire-profile route binding:

`/home/postfiat/repos/StakeHub-repeat-demo/data/wallet-demo-a666-mainnet-fire.json`

### 2.3 Existing A666 PFTL fleet evidence — use this, not the lightning sixval fleet

Canonical baseline:

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/baseline/a666-state-inventory.json`

That baseline records:

- PFTL RPC ports: **39650–39655**
- 6/6 validators identical
- observed height: 776
- A666 found and active
- A666 route live: true
- A666 route paused: false
- route epoch: 6
- latest finalized NAV epoch: 5
- wrapped token matches `0xeE4C...9bE5`
- bridge supply invariant holds
- Ethereum spendable supply at the snapshot: 9,000,000 atoms
- PFTL spendable A666 supply at the snapshot: 99,000,000 atoms

The original production route was signed for:

- chain ID: `postfiat-wan-devnet-2`
- genesis:
  `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9`

Evidence:

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/deployments/a666-mainnet-20260727/09-production-route-init/finality.request.json`

### 2.4 Prior literal-A666 export receipt

Opening export packet:

`2385ff333b16f4dac45b2845313d0b34ff6ca28a052dba727ebbe5fae4707c23722c192d8e5258c2b262b6f78f1d97a3`

PFTL export receipt hash:

`b7bfa196b87c79dde837e8de6026b5150059fa7ace8f11fc44bfd5f01f6e77f6896ac5716d7801d9804bf3a7246265c6`

Receipt facts:

- transition: `export_debit`
- route: literal A666 route
- PFTL owner:
  `pfab9b9228942e5c529633a13aa271d5297bec6353`
- amount: `31,386,197,455` atoms
- PFTL block height: 348

Evidence:

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/deployments/a666-mainnet-20260727/13-opening-export-proof/receipt-witness.json`

A separate archived signed export transaction sends 1,000,000 atoms to the Ethereum wallet `0x1455...8c0` on the same literal route. It lives inside the signed batch archives under:

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/deployments/a666-mainnet-20260727/`

The successor should locate the corresponding Ethereum mint/transfer transaction hash from logs or chain events. This predecessor did not finish that lookup. Do not claim a specific Ethereum transaction hash until verified.

---

## 3. The wrong path taken by this predecessor

The predecessor followed:

`/tmp/pftl/mainnet-grad-846ed67/sixval/`

That fleet is:

- chain ID: `local-postfiat-lightning-navcoin-demo`
- genesis: `c9923b5a...`
- debug binary revision: `ae3c53c9`
- RPC ports: 30660–30665
- height around 34 during the audit
- launched as bare processes
- missing deployment environment
- missing an active pfUSDC NAV profile

This fleet is not proof that the existing literal A666 route disappeared. It is a separate local fleet.

Do not:

- reset this fleet;
- migrate StakeHub onto it;
- provision pfUSDC/A666 on it;
- relaunch it merely to satisfy the old StakeHub devnet profile;
- use its failures as evidence that the deployed A666 route never worked.

All proposed sixval relaunch and on-chain provisioning actions were stopped before execution.

---

## 4. Current repository state

### 4.1 A666 worktree

Path:

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724`

Branch:

`feature/pnok-private-fix`

Current HEAD at handoff creation:

`523d179 test(a666): record topology-aligned restage and full preflight`

Tracked tree was clean when checked. The branch was 34 commits ahead of origin. There are approximately 9,078 protected untracked deployment/evidence paths.

Rules:

- never `git add .`
- never bulk clean
- never delete untracked deployment/evidence paths
- never use destructive checkout/reset commands
- add only exact owned paths

### 4.2 StakeHub

Path:

`/home/postfiat/repos/StakeHub-repeat-demo`

Branch:

`pft-cli-wallet-20260721`

Current HEAD at handoff creation:

`8424e2b Revert "feat(profile): align topology_file with signed fleet manifest"`

The worktree contains many pre-existing modified and untracked files under `pft_wallet/`, `stakehub/`, `tests/`, `zk/`, egg-info, and related paths. They predate this handoff campaign.

Do not clean, revert, or commit unrelated StakeHub worktree changes.

---

## 5. Useful code delivered during this campaign

These commits are real and tested, even though live fire never happened:

- `2374778` — split raw profile file hash from canonical profile identity
- `6e7a67a` — packet-gated live-loop runner
- `dc0938a` — governed subscription adapter
- `66c6de5` — export/return adapters
- `215c2eb` — real bridge-in adapter using existing StakeHub ingress
- `813c226` — profile-driven bridge-out and real redeem adapter
- `1f67d08` / `d5f5e82` — wired dispatch and registry-driven end-to-end tests
- `f695a0c` — encrypted deployment-signing custody with recovery tests

Important existing StakeHub surfaces:

- bridge in:
  `stakehub/private_swap_ingress.py::submit_pftl_bridge_in`
- bridge out:
  `stakehub/private_swap_orchestration.py::submit_pftl_bridge_out`
- runner:
  `stakehub/wallet_demo_live_loop.py`
- leg adapters:
  `stakehub/live_loop_leg1_bridge_in.py`
  `stakehub/live_loop_leg2_governed_subscription.py`
  `stakehub/live_loop_leg3_export.py`
  `stakehub/live_loop_leg4_return.py`
  `stakehub/live_loop_leg5_redeem.py`

Latest relevant qualification numbers before the scope error was discovered:

- 132 Python tests passed
- recovery manifest `--run`: PASS
- 13 Rust execution unit tests passed
- wrong packet hash rejected 5/5

These are code qualifications. They are not proof of a live transaction.

---

## 6. Five v3 packets remain HELD

Directory:

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/live-demo/`

Current v3 files and hashes:

1. `leg1-a666-bridge-in-held.json`
   `04a2c5c88401f807daa405ff67e6e3be6f17b0c576d8290317876e37c9d840c5`
2. `leg2-a666-governed-subscription-held.json`
   `192ad79dc3b2661885aa33da2ecadc34ed2076eae26c1a2e52820becdeaec1fd`
3. `leg3-a666-mainnet-export-held.json`
   `9e6a195253b1722e49126063db5ab265786019074b54ae8ddabfbdc2555f1b3d`
4. `leg4-a666-return-burn-import-held.json`
   `4f3e5648050ec7a0440120cc0ff8c1d9ec059a62f41850488e373c80102a8890`
5. `leg5-a666-redeem-conservation-held.json`
   `80fb313e4ae880f6ef140520281c90c17cff9fc193cb51379c41f53c52ab55d7`

All are:

- packet version 3
- `classification: HELD`
- `wiring_status: WIRED`
- `self_executable: false`
- principal: 10,000,000 atoms

Do not fire them merely because they passed mocked tests. Revalidate them against the correct A666 fleet, current balances, current receipts, and exact command surfaces. Obtain fresh human approval.

---

## 7. Live balances and spend state seen during the campaign

Point-in-time values; refresh before any fire:

- Ethereum USDC: `84.161443`
- Ethereum ETH: approximately `0.289781906`
- Arbitrum USDC: `0.000000`
- Arbitrum ETH: approximately `0.005556809`
- spend ledger: `501.024845 USDC`
- cap: `530.000000 USDC`
- proposed 10-USDC leg projection:
  `501.024845 + 10.000000 = 511.024845`
- projected headroom:
  `18.975155 USDC`
- Ethereum wA666 wallet balance:
  **103.000000 wA666**

Never raise or bypass the cap.

---

## 8. Deployment-signing key work — banked but inactive

The old ML-DSA deployment publisher private key was unrecoverable. A replacement key was generated and placed under StakeHub-managed encrypted custody.

Public identity of the replacement publisher:

`pfaf3b9a1593427e656aa1e03ca641ca7842a6f0e7`

Primary custody locations, paths only:

- `/home/postfiat/.stakehub/deployment-signing/`
- passphrase location:
  `/home/postfiat/.pft/a666-live-demo/deployment/keys/vault-passphrase`
- recovery copy under:
  `/mnt/HC_Volume_106212907`

The backup/recovery drill passed. True off-host backup remained marked false.

A fresh manifest was signed for the unrelated lightning sixval fleet. StakeHub profile changes were subsequently reverted. Do not activate that manifest for the real A666 route.

Relevant commits:

- StakeHub custody code: `f695a0c`
- A666 rotation evidence: `cec3199`
- StakeHub profile rotation attempt: `213ed5e`
- revert: `1177557`
- topology attempt: `6753086`
- revert: `8424e2b`

No private key or passphrase value belongs in a report, prompt, commit, shell history, or transcript.

---

## 9. Current runtime state after rollback

Last verified rollback state:

- five StakeHub units active
- dashboard 8787: original snapshot state
- bfinal 8788: original snapshot state
- warm prover 18793: down
- 18792, 8787, 8788, and 8080 listening
- no live-money command invoked
- no signing action invoked
- no transaction submitted
- no balance mutation caused by this campaign

Snapshot and rollback evidence:

- snapshot commit: `fb022ba`
- first restage result: `ffe637a`
- rollback: `aba3baa`
- later rollback evidence: `5092f5f`, `6d0690a`, `523d179`

Do not blindly reuse the snapshot runtime as the target configuration. First identify which current StakeHub service/profile is actually meant to drive the existing A666 fleet on ports 39650–39655.

---

## 10. First 30 minutes for the successor

### Step 1 — verify the served and on-chain facts

Re-run the Ethereum wA666 balance and contract-code reads.

Verify:

- token code exists at `0xeE4C...9bE5`
- controller code exists at `0x9A02...c3d9`
- verifier code exists at `0xb79F...F96A`
- wallet balance remains 103.000000 wA666
- current Uniswap position and pool balances, read-only

### Step 2 — identify the correct live A666 PFTL fleet

Start from:

`docs/evidence/a666-public-reserve-product-20260803/baseline/a666-state-inventory.json`

Probe ports 39650–39655 using the sanctioned read-only RPC method.

Require:

- 6/6 same chain ID
- 6/6 same genesis
- 6/6 same height/tip/state root
- A666 asset exists
- A666 route is live and unpaused
- supply invariant holds
- current route epoch and finalized NAV epoch known

Do not substitute ports 30660–30665.

### Step 3 — reconstruct the prior successful A666 path

Read in this order:

1. `deployments/a666-mainnet-20260727/09-production-route-config.json`
2. `deployments/a666-mainnet-20260727/09-production-route-init/`
3. `deployments/a666-mainnet-20260727/10-production-route-activate/`
4. `deployments/a666-mainnet-20260727/11-opening-inventory-export/`
5. `deployments/a666-mainnet-20260727/11b-opening-inventory-export/`
6. `deployments/a666-mainnet-20260727/13-opening-export-proof/`
7. baseline A666 state inventory
8. current Ethereum events for the wrapped token/controller

Produce a receipt map:

- PFTL export packet/receipt
- Ethereum mint/claim transaction
- Uniswap swap/liquidity transaction
- any prior return-burn/import receipt
- current wallet/token balances

### Step 4 — reconcile StakeHub to the correct existing environment

Determine the exact profile, binary, validator endpoints, and service set that drove the prior A666 route.

Do not:

- migrate to the lightning sixval fleet;
- reset a devnet;
- generate a new A666 asset;
- deploy replacement Ethereum contracts;
- overwrite existing route IDs;
- activate the lightning sixval signed manifest.

### Step 5 — prepare one fresh live packet set

Only after Steps 1–4 are proven:

- refresh balances and gas
- refresh NAV/proof and route status
- verify replay registry
- verify services and warm prover
- bind exact correct fleet/profile
- re-hash current packet commands
- produce separate HELD packets per irreversible leg
- present exact expected before/after arithmetic

---

## 11. Required live sequence

The human's requested sequence is:

1. external USDC → PFTL pfUSDC
2. pfUSDC → literal A666 at certified NAV
3. A666 export → Ethereum wA666
4. Uniswap interaction using wA666
5. Ethereum return burn → certified PFTL A666 import
6. A666 redeem → pfUSDC
7. pfUSDC → external USDC
8. final atom-exact conservation and PnL report

One leg at a time.

For every leg:

- exact packet hash confirmed
- exact command captured
- one invocation
- finalized receipt
- expected balance delta
- replay entry
- committed evidence
- then and only then the next leg

Any deviation: STOP-no-retry; hold all remaining legs.

---

## 12. What “done” means

Done requires evidence of a real live loop, not passing tests:

- live USDC debited from the approved source
- pfUSDC credited on the correct existing PFTL fleet
- literal A666 minted/allocated
- wA666 minted/exported on Ethereum
- actual Uniswap receipt
- wA666 burned/returned
- A666 credited back on PFTL
- A666 redeemed to pfUSDC
- pfUSDC bridged to external USDC
- final balances reconciled
- all transaction/receipt hashes recorded
- spend ledger updated
- final PnL produced
- no replay, cap, supply, or custody invariant violated

---

## 13. Verified closed — do not re-report

- Literal A666 route exists.
- wA666 contract exists.
- The user holds 103.000000 wA666.
- A prior literal-A666 PFTL export receipt exists.
- No devnet reset is required merely to establish that A666 exists.
- The local lightning sixval fleet is a different environment.
- This predecessor moved zero funds.
- The replacement deployment publisher key is stored under StakeHub custody but is not active for the existing A666 route.
- All sixval relaunch/provisioning proposals are stopped.
- Five v3 packet files exist but remain HELD.

---

## 14. Primary evidence paths

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/handoffs/A666-RECOVERY-AND-LIVE-DEMO-HANDOFF-20260805.md`

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/baseline/a666-state-inventory.json`

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/deployments/a666-mainnet-20260727/`

`/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/live-demo/`

`/home/postfiat/repos/StakeHub-repeat-demo/data/wallet-demo-a666-mainnet-fire.json`

`/home/postfiat/repos/StakeHub-repeat-demo/stakehub/wallet_demo_live_loop.py`

`/home/postfiat/repos/StakeHub-repeat-demo/stakehub/private_swap_ingress.py`

`/home/postfiat/repos/StakeHub-repeat-demo/stakehub/private_swap_orchestration.py`

---

## 15. Final predecessor statement

The predecessor's central error was failing to begin with the live Ethereum wallet and the existing A666 deployment receipts. That error caused irrelevant work on a separate local fleet.

The successor should distrust narrative continuity and verify the exact live route first. The user wants execution against the already-existing A666 system, not another recovery program.
