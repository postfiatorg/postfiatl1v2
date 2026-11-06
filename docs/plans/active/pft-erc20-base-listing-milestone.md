# PFT as an ERC-20 on Base: the Coinbase listing path

**Status:** Active. Priority one for this lane from 2026-10-26. Authorizes no Base mainnet deployment.
**Date:** 2026-10-26
**Source paper:** [ERC-20 options paper](../../listings/coinbase/pft-erc20-representation-options-20261026.md), Text Improvement Harness full gate 85.67 on 2026-10-26 after four passes, 82.60 on 2026-10-28 as it stood and 84.00 after one more pass. Locked as the research specification on 2026-11-03 at 87.27 (M1).
**Basis:** [Coinbase integration plan](../../listings/coinbase/integration-plan.md), interim path (`integration-plan.md:71-74`, `:719-739`)

## BLUF

PFT goes onto Coinbase first as a fixed-supply ERC-20 on Base, with six
decimals, the unit of native PFT. A 1:1 bridge into native PFT opens at
PFTL's mainnet genesis and never closes. The contract, its tests and the
deploy script are on main (`ad5ba32f`). Base mainnet needs three things from
the other lane: the token document with the four decisions, the audit, and
the Safe. Phase 2 waits for the mainnet genesis plan. The estimates are mine,
optimistic and without audit time: about two days for the token plus the
audit, one to two weeks for the genesis bridge, and two weeks for the reverse
bridge.

Owners are marked *(this lane)*, *(other lane)* or *(Coinbase)*.

## Phase 1 — the token on Base

- [x] P1.1 *(this lane)* The contract, its tests and the deploy script (`ad5ba32f`).
  - `crates/pft-token-contracts/src/PFTToken.sol`: the whole supply is minted once to the treasury, and `decimals()` is fixed at 6. It has no owner, admin, pause, blacklist or upgrade path.
  - `crates/pft-token-contracts/test/PFTToken.t.sol`: 8 tests.
  - `crates/pft-token-contracts/script/DeployPFTToken.s.sol`, with `script/pft-token.env.example`.
  - The token is its own Foundry project, so `crates/ethereum-contracts` compiles as before and the pinned A666 public reader build stays reproducible.
- [x] P1.2 *(this lane)* The OpenZeppelin pin (`ad5ba32f`). The submodule `crates/pft-token-contracts/lib/openzeppelin-contracts` is at `cab19933` (`.gitmodules:7-9`), with its remapping in `crates/pft-token-contracts/foundry.toml`; the CI `evm-contracts` job fetches it and runs `forge test` there (`.github/workflows/product-security-ci.yml`).
- [x] P1.3 *(this lane)* The dry run against anvil with chain id 84532, as `crates/pft-token-contracts/README.md` describes, then a test deployment on Base Sepolia. To tick it: the dry-run output, the Base Sepolia address and its verified source. Both are recorded in [pft-token-dry-run-20261026.md](../../listings/coinbase/pft-token-dry-run-20261026.md). The Base Sepolia test token is `0x18fE4ca51A0cE042688D5A54917319d03B527D92` (2026-10-28), with its supply held by the test Safe `0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB`; record `crates/pft-token-contracts/deployments/pft-token-base-84532/deployment.json`.
- [ ] P1.4 *(other lane)* The audit, including its budget and the vendor choice. For one contract of this size it typically takes one to three weeks of calendar time. To tick it: the report, with every finding closed.
- [ ] P1.5 *(other lane)* The token document with the four decisions. To tick it: the published document.
  - [x] the total supply: 100 billion PFT, fixed, decided 2026-10-28;
  - [ ] the split at launch between the ERC-20 slice and genesis;
  - [ ] the treatment of PFT already paid on the PFT Ledger;
  - [ ] the treasury signers and threshold.
- [ ] P1.6 *(other lane)* The Safe multisig on Base, with the other lane's keys. To tick it: the Safe address, its signers and its threshold.
- [ ] P1.7 *(other lane)* The Base mainnet deployment, with the other lane's keys and money. To tick it: the token address, the deploy transaction, and a treasury balance equal to the Base ERC-20 slice.
- [ ] P1.8 *(this lane)* Source verification on the Base block explorer. To tick it: the verified-source page.
  - The test-network source is verified on [Sourcify](https://repo.sourcify.dev/84532/0x18fE4ca51A0cE042688D5A54917319d03B527D92) and [Blockscout](https://base-sepolia.blockscout.com/address/0x18fE4ca51A0cE042688D5A54917319d03B527D92?tab=contract) (Base Sepolia, 2026-10-28). The Base mainnet verification is still open.
- [x] P1.9 *(this lane)* The Coinbase one-pager, built from the paper's Appendix A: [pft-erc20-one-pager.md](../../listings/coinbase/pft-erc20-one-pager.md) (`66326c6d`). It is final for now; its figures change only when the token document is published.
- [ ] P1.10 *(other lane, then Coinbase)* The answer to the listing package's first ask, which route to plan on (`integration-plan.md:741-743`). To tick it: the message sent and Coinbase's reply.
  - The other lane sent the [exchange guidance page](https://postfiat.org/exchange-integration/) to Coinbase on 2026-10-28 (his Telegram message of 2026-10-29: "sent update to coinbase"). The item now waits for Coinbase's reply.

## Phase 2 — the genesis bridge

The order is the design note, then the code, then the qualification.

The lead developer decided two things on 2026-10-26, to be revisited only if the code shows a reason. The proof is a bridge-committee-signed BFT checkpoint of a Base block plus a receipt-log proof of the Lock event, with the SP1 path through Base's dispute games kept as a later upgrade, and the lock contract ships the dormant Phase 3 release path in Phase 2, with the PFTL finality verifier's address fixed at construction.

Decided by the product owner 2026-10-28: total supply 100 billion PFT; the Base ERC-20 is a slice of it and the rest is native PFT at genesis; the slice size fixes `reserved_atoms` and is still open.

- [x] P2.1 *(this lane)* The design note, [docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md](../../specs/pft-erc20-base-bridge-phase2-design-20261026.md) (`e48c99f4`; the two decisions recorded in `13240104`). It is not locked: the full gate scored 84.93 on 2026-10-26 (Task Node `task_f1528ebca6a6c5ae613d776328561724`) and 84.73 once the release cap and the committee lifecycle were in, then 85.67 on 2026-10-28 and 85.27 after one more pass, below 86. It covers:
  - the genesis external lane (`docs/whitepaper.md:66-70`);
  - the Base lock contract;
  - the inbound release path on PFTL and its proof;
  - the Base checkpoint source, built 2026-11-05 (`08d6e03f6`).
- [x] P2.2 *(this lane)* The genesis external lane. Genesis reserves exactly the ERC-20 slice there, and the native supply invariant counts it: merged in `bfe44841`, described in [docs/architecture/genesis-external-lanes.md](../../architecture/genesis-external-lanes.md). `reserved_atoms` is a genesis parameter until the genesis plan fixes the slice; the check that it equals the slice is P2.5's build check. No lane is configured on any network. Next: P2.4, the release operation with the Base checkpoint proof.
- [x] P2.3 *(this lane)* The Base lock contract, following `ERC20BridgeVaultL1` (`crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol:6-16`), with no holder snapshot and no closing date: `crates/pft-token-contracts/src/PFTLockV1.sol`, merged in `39bfb547`; `forge test` passes 123 tests (111 from the lock branch, 12 from main's deploy-script checks).
  - Its deployment on Base, the PFTL finality verifier first and then the lock, is a later step with its own record.
  - The lock deployment script exists: `crates/pft-token-contracts/script/DeployPFTLock.s.sol`, merged in `182b3f22`; the Base deployment waits for the PFTL finality verifier on Base.
  - The script now refuses to deploy unless the finality verifier's stored anchor and its chain, genesis, route, asset and vault bindings equal operator-pinned values; it re-reads them after deployment, and the operator checks them once more from the mined chain: merged in `1e2cc55f`, `crates/pft-token-contracts/README.md`, "Deploy the lock contract".
  - Base Sepolia rehearsal (test mode, reverting stand-in verifier): `0xE36e7Fc384564419187546c19719A5654266dE0B`, record `crates/pft-token-contracts/deployments/pft-lock-base-84532/deployment.json`.
- [x] P2.4 *(this lane)* The inbound release path on PFTL and its proof: the release record and rules and `external_lane_release_v1`, described in [the release path section](../../architecture/genesis-external-lanes.md#the-release-path); merged in `8f29aaccb` (`fb204616b`, `aab72d1eb`, `e2df7a612`, `9cce9ff59`, `a2fa13d25`, `eed475656`, `de32f05da`, `5cb1310cf`, `2eee50c28`, `7ea92375a`, `e30668a6f`, `89db575cf`, `b1be6c6ce`). Next: P2.5, the qualification before genesis.
- [ ] P2.5 *(this lane)* Its own qualification before genesis: tests, a devnet run, and an audit of the lock contract. To tick it: the qualification record.
  - [x] 2026-11-05 local EVM composition: a fresh Lock, five-of-six checkpoint, native release, duplicate refusal and watcher persistence ([record](../../listings/coinbase/pft-erc20-release-path-qualification-20261103.md#2026-11-05-local-evm-composition)).
  - [ ] Fresh Base Sepolia repetition needs a new funded deployer and token holder authority; the recorded one-time keys are retired. Lock audit and genesis allocation remain open.
  - The tests, a six-validator devnet run of the release path with a fixture Base block, and the lock contract's public review trace: [the qualification run record](../../listings/coinbase/pft-erc20-release-path-qualification-20261103.md), 2026-11-03. The items below stay open.
  - [x] No proof program of its own, corrected 2026-11-04: the lane's proof is the committee-signed Base checkpoint plus the receipt proof of the `Locked` event (design option (b), `crates/execution/src/external_lane_release.rs`, `verify_external_lane_lock`), so no SP1 guest, vkey pin or proof inventory entry is needed; an SP1 guest for Base belongs to the design's option (a) upgrade, not before.
  - [x] The Base checkpoint source and its own qualification: the four node commands that observe a finalized Base block as a lane checkpoint, sign and assemble the committee's votes and build a Lock's receipt proof ([the page](../../architecture/genesis-external-lanes.md#the-base-checkpoint-source)), merged in `08d6e03f6`; tests and a live read-only Base Sepolia run in [the qualification record](../../listings/coinbase/pft-erc20-release-path-qualification-20261103.md#base-checkpoint-source-2026-11-05). A real Lock's proof waits for the next rehearsal.
  - [x] `PFTLockV1` enforces a minimum lock amount at or above the PFTL account reserve (`ACCOUNT_RESERVE`, 10 atoms, `crates/execution/src/entrypoints.rs:139`): a smaller Lock to a new PFTL account would be final on Base but refused on PFTL with `below_account_reserve`. `minimumLockAmount`, `LockBelowMinimum`, the constructor floor `PFTL_ACCOUNT_RESERVE`, the deploy script's `PFTL_ACCOUNT_RESERVE_UNITS` check of `PFT_LOCK_MINIMUM_LOCK_AMOUNT`, `lock_minimum_binding_tests`; merged in `8351ac551` (`e5fa5dfa1`, `7ecb3e77c`, `4edd4ab15`, `7a42f5fc5`, `3f743ab6b`, `ad8f3db32`, `8cb904585`). The Base Sepolia rehearsal lock `0xE36e7Fc384564419187546c19719A5654266dE0B` predates the minimum (it accepts 1-atom Locks); the next rehearsal archives its record to free the fixed per-chain record path and deploys a new lock from a fresh deployer, since the script refuses `RecordExists` and `DeployerAlreadyUsed`.
  - [x] Prune the release records behind the finalized checkpoint, keeping the root (`ExternalLaneState::apply_release`, `crates/types/src/external_lane_types.rs`), so a lane stores at most `EXTERNAL_LANE_MAX_RELEASE_RECORDS` (1,000,000), past which a release is refused with `external_lane_release_records_full`; [pruning the release records](../../architecture/genesis-external-lanes.md#pruning-the-release-records); merged in `f53e7457c` (`f301e0225`, `3f1ecfb22`, `0abb14d62`, `db4c46708`, `6ec11d2f4`, `d07c6e2dd`, `dfc0c70b6`, `f4839b1ff`, `3875bdc92`).
  - [x] The governed lane amendment kinds are recognised (`external_lane_amendment_refusal`, `crates/types/src/external_lane_types.rs`): the cap kind is accepted and built (branch `token/lane-cap-amendment`), the committee epoch kind is accepted and built (branch `token/lane-committee-epoch`); [governed lane amendments](../../architecture/genesis-external-lanes.md#governed-lane-amendments).
  - [x] The cap amendment and its seven-window raise delay (design section 5.1), read by execution, admission and the lane equality rule: merged in `f2dedd257` (`5f2425f32`, `856540d6d`, `5e99a51da`, `1bdac4ca7`, `e2ecc90e2`, `25ca233f7`, `6915cd09c`, `d0e3f78ae`, `43b651d4f`, `35a5f3746`, `10f07ec9f`, `c856a5de7`, `c4aee0034`, `8be9117b9`, `6f7815f27`, `5ade0b897`, `260b9135d`, `23406798d`, `605ccccd2`, `af442f617`, `9906e7fec`, `485dd2e5b`, `a7225fc77`). Signed by the registry threshold of the active set (`external_lane_amendment_threshold`, 5 of 6), the full active set declared; the node's ratify command accepts five of six (`ratify_governance`, `crates/node/src/governance.rs`). A raise committed at height `h` applies at `h + 7 × release_window_blocks`, never earlier than seven windows; genesis configures at most eight lanes (`EXTERNAL_LANE_MAX_LANES`). Steps on the architecture page.
  - [x] The committee epoch move by governed amendment (design section 4.1), signed at the registry threshold by the active set and bound to its proposal slot, as the cap amendment is (`external_lane_amendment_authority_refusal`, `external_lane_amendment_slot_refusal`, `crates/types/src/external_lane_types.rs`); `ratify-external-lane-committee-epoch` accepts five of six (`ratify_governance`, `crates/node/src/governance.rs`): merged in `be09a97f1` (`48da853d3`, `c9c7d5d3c`, `47113fe82`, `8752aa100`, `a2f0ae09b`, `3563ae8c1`, `c1b282a13`, `31d51af9a`, `a593e846b`, `eeda53892`, `0f5d87991`, `dae9755ea`, `e3a2dc581`, `5d077a9c3`, `554403d5e`, `5cc806348`, `d9c5a0853`). Steps on the architecture page.
  - [x] Six-validator rehearsal of both amendment rounds with the real node CLI, five of six signing, on a local fleet started by `init --genesis-file`: a cap lowered, a halt, a raise held seven windows, a committee move after `ActivateCommittee`, the old epoch refused and the Lock re-proven by epoch 2 released; four of six and a quorum-1 batch refused ([the rehearsal](../../listings/coinbase/pft-erc20-release-path-qualification-20261103.md#six-validator-rehearsal-of-the-amendment-rounds-2026-11-04), 2026-11-04).
  - [x] The watcher of design section 5.2 ([the page section](../../architecture/genesis-external-lanes.md#the-watcher)), merged in `41bde8b8a`; tests and a live read-only check in [the qualification record](../../listings/coinbase/pft-erc20-release-path-qualification-20261103.md#the-watcher-2026-11-05).
  - Open only on the outside audit of the lock contract, which P1.4's vendor delivers.

## Phase 3 — the reverse export route (optional)

- [ ] P3.1 *(this lane)* Export native PFT as the same ERC-20, released from the Base lock contract. This needs the export route generalised beyond NAV assets (`crates/execution/src/nav_vault_asset_execution.rs:6470-6477`), and the PFTL finality verifier and relay deployed on Base.
  - [ ] Designed and refused until built (merged in `7a968007e`): the design note `docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md` and the `external_lane_export_v1` skeleton. The skeleton has its type and shape refusals, and mempool admission, batch building and execution refuse it with `external_lane_export_not_supported` (`crates/execution/src/external_lane_release.rs`, `crates/node/src/tests/external_lane_export_tests.rs`). `exported_atoms` and the Base path come with the implementation.

## Mandate deliverables (AGENTS.md)

- [x] M1 *(this lane)* Lock the source paper as the research specification at 86 or above on the full gate, requested as a Task Node task. It stood at 85.67 on 2026-10-26, then at 82.60 on 2026-10-28 and 84.00 after one more pass. Locked on 2026-11-03 at 87.27 (GPT-6 Astra Pro 85.00, Claude Fable 5.1 86.20, GLM 5.3 90.60; run group `pft-erc20-options-20261103-r11`), from 84.13 as it stood (`-r10`), after one `openai/gpt-5.6-sol-pro` pass and a fact check against main; Task Node `task_8c15b90660d7c39f5b939352a7781490`.
- [x] M2 *(this lane)* A Python CLI that reads the token's state: address, total supply, decimals and treasury balance.
  - `python/postfiat_rpc/pft_token.py` (`--record` or `--token`/`--treasury`, JSON or `--table`, exit 2 on a chain-id mismatch, a token without code or a malformed answer); `python/tests/test_pft_token.py`: 10 tests. Documented in [RPC examples](../../rpc/examples.md#erc-20-pft-on-base).
- [x] M3 *(this lane)* A user-facing interface that consumes the CLI output.
  - `python/postfiat_rpc/pft_token_ui.py` with `pft_token_ui_assets/`: a read-only page on 127.0.0.1 whose `/api/state` runs the `pft_token` reader; GET only, unknown paths 404. `python/tests/test_pft_token_ui.py`: 7 tests.
- [ ] M4 *(this lane)* Retire this document into `docs/plans/completed/` and document the token in the existing documentation; started: [the reader's page](../../architecture/pft-token-on-base.md).

## Not before

- Nothing is deployed to Base mainnet before the token document (P1.5) and the audit (P1.4).
- Nothing in Phase 2 starts before the mainnet genesis plan exists.

## Does not change

- **Consensus.** Ordering, certificates and Cobalt stay as they are. Phase 1 is a contract on Base. Phase 2's genesis lane and release path are additions that get their own qualification (P2.5).
- **The NAVCoin stack.** NAVCoins stay issued assets beside PFT (paper, section 6).
- **Native PFT's genesis-only creation.** The external lane reserves supply at genesis, and nothing creates PFT later (`docs/status/OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:12-13`).

## References

- [ERC-20 options paper](../../listings/coinbase/pft-erc20-representation-options-20261026.md)
- [Coinbase integration plan](../../listings/coinbase/integration-plan.md)
- `crates/pft-token-contracts/README.md` (the PFT token section)
- `AGENTS.md` (the research specification lock, the CLI and interface, retirement)
