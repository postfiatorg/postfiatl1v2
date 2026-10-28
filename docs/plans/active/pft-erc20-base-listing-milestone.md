# PFT as an ERC-20 on Base: the Coinbase listing path

**Status:** Active. Priority one for this lane from 2026-10-26. Authorizes no Base mainnet deployment.
**Date:** 2026-10-26
**Source paper:** [ERC-20 options paper](../../listings/coinbase/pft-erc20-representation-options-20261026.md), Text Improvement Harness full gate 85.67 on 2026-10-26 after four passes, 82.60 on 2026-10-28 as it stood and 84.00 after one more pass. It is not yet locked as the research specification (M1).
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

## Phase 2 — the genesis bridge

The order is the design note, then the code, then the qualification.

The lead developer decided two things on 2026-10-26, to be revisited only if the code shows a reason. The proof is a bridge-committee-signed BFT checkpoint of a Base block plus a receipt-log proof of the Lock event, with the SP1 path through Base's dispute games kept as a later upgrade, and the lock contract ships the dormant Phase 3 release path in Phase 2, with the PFTL finality verifier's address fixed at construction.

Decided by the product owner 2026-10-28: total supply 100 billion PFT; the Base ERC-20 is a slice of it and the rest is native PFT at genesis; the slice size fixes `reserved_atoms` and is still open.

- [x] P2.1 *(this lane)* The design note, [docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md](../../specs/pft-erc20-base-bridge-phase2-design-20261026.md) (`e48c99f4`; the two decisions recorded in `13240104`). It is not locked: the full gate scored 84.93 on 2026-10-26 (Task Node `task_f1528ebca6a6c5ae613d776328561724`) and 84.73 once the release cap and the committee lifecycle were in, then 85.67 on 2026-10-28 and 85.27 after one more pass, below 86. It covers:
  - the genesis external lane (`docs/whitepaper.md:66-70`);
  - the Base lock contract;
  - the inbound release path on PFTL and its proof;
  - the Base checkpoint source, which nothing in the repository provides today (paper, section 7).
- [ ] P2.2 *(this lane)* The genesis external lane. Genesis reserves exactly the ERC-20 slice there, and the native supply invariant counts it (today it counts only live custody and fee burns, `docs/status/OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:23`).
- [x] P2.3 *(this lane)* The Base lock contract, following `ERC20BridgeVaultL1` (`crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol:6-16`), with no holder snapshot and no closing date: `crates/pft-token-contracts/src/PFTLockV1.sol`, merged in `39bfb547`; `forge test` passes 123 tests (111 from the lock branch, 12 from main's deploy-script checks).
  - Its deployment on Base, the PFTL finality verifier first and then the lock, is a later step with its own record.
  - The lock deployment script exists: `crates/pft-token-contracts/script/DeployPFTLock.s.sol`, merged in `182b3f22`; the Base deployment waits for the PFTL finality verifier on Base.
- [ ] P2.4 *(this lane)* The inbound release path on PFTL and its proof. It extends the BFT-checkpoint ingress (`docs/status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md:391`) with a Base checkpoint source. The SP1 ingress program is pinned to Sepolia (`programs/pfusdc-eth-ingress/src/lib.rs:19`, `:91`).
- [ ] P2.5 *(this lane)* Its own qualification before genesis: tests, a devnet run, and an audit of the lock contract. To tick it: the qualification record.

## Phase 3 — the reverse export route (optional)

- [ ] P3.1 *(this lane)* Export native PFT as the same ERC-20, released from the Base lock contract. This needs the export route generalised beyond NAV assets (`crates/execution/src/nav_vault_asset_execution.rs:6470-6477`), and the PFTL finality verifier and relay deployed on Base.

## Mandate deliverables (AGENTS.md)

- [ ] M1 *(this lane)* Lock the source paper as the research specification at 86 or above on the full gate, requested as a Task Node task. It stood at 85.67 on 2026-10-26, then at 82.60 on 2026-10-28 and 84.00 after one more pass.
- [x] M2 *(this lane)* A Python CLI that reads the token's state: address, total supply, decimals and treasury balance.
  - `python/postfiat_rpc/pft_token.py` (`--record` or `--token`/`--treasury`, JSON or `--table`, exit 2 on a chain-id mismatch, a token without code or a malformed answer); `python/tests/test_pft_token.py`: 10 tests. Documented in [RPC examples](../../rpc/examples.md#erc-20-pft-on-base).
- [x] M3 *(this lane)* A user-facing interface that consumes the CLI output.
  - `python/postfiat_rpc/pft_token_ui.py` with `pft_token_ui_assets/`: a read-only page on 127.0.0.1 whose `/api/state` runs the `pft_token` reader; GET only, unknown paths 404. `python/tests/test_pft_token_ui.py`: 7 tests.
- [ ] M4 *(this lane)* Retire this document into `docs/plans/completed/` and document the token in the existing documentation.

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
