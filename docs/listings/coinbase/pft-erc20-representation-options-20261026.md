# An ERC-20 form of PFT before mainnet: options and a recommended path

**Status:** Draft, 2026-10-26. Operator dravlic, for the product owner's Coinbase document. Text Improvement Harness full gate on 2026-10-26, after four passes: 85.67 (GPT-6 Astra Pro 83.80, Claude Fable 5.1 83.00, GLM 5.3 90.20; 5 runs per judge, scored without the score line). On 2026-10-28 the text as it stood scored 82.60 (GPT-6 Astra Pro 79.20, Claude Fable 5.1 80.80, GLM 5.3 87.80; run group `pft-erc20-options-20261028-r6`, `c9bb449d808a26b4…`), and after one wording and structure pass from the judges' points 84.00 (80.20, 82.60, 89.20; `pft-erc20-options-20261028-r7`, `4639ecb08d221763…`). With the launch state of section 11 added on 2026-10-28, it scored 79.60 (78.00, 74.80, 86.00; `pft-erc20-options-20261028-r8`, `1dccd5bb4b9ef29d…`). It is not locked, being below 86.

## 1. BLUF

1. I recommend "ERC-20 first, bridge at mainnet": a plain fixed-supply ERC-20 PFT on Base now, and a 1:1 bridge into native PFT that opens at this L1's mainnet genesis and never closes.
2. Coinbase integrates ERC-20 tokens with little engineering work. A native chain without mainnet or post-quantum custody cannot be integrated soon. Injective took this route onto Coinbase. Coinbase custody cannot sign ML-DSA-65 before about mid-2027, so even after mainnet the ERC-20 is most likely Coinbase's listed form for a long time, not a stopgap.
3. The token costs about two days of engineering plus the audit, the genesis bridge about one to two weeks, and an optional reverse bridge about two weeks; all are my estimates, without audit time.
4. The four decisions in section 9 are: the supply number, the treasury multisig's signers and threshold, the allocation with the treatment of PFT already paid on the PFT Ledger, and when to tell Coinbase. The supply number is decided (100 billion PFT, fixed, 2026-10-28); the open items are the ERC-20 slice size, the launch allocation, the issuing entity, the treasury signers and threshold, the treatment of PFT already paid on the PFT Ledger, and when to tell Coinbase.
5. The contract, its tests and the Base Sepolia deploy script are on main (`crates/pft-token-contracts/src/PFTToken.sol`, `test/PFTToken.t.sol`, `script/DeployPFTToken.s.sol`); the deployment waits for the open items among the four decisions in section 9, and the one-page note for Coinbase and the Phase 2 design note follow.
6. The [milestone](../../plans/active/pft-erc20-base-listing-milestone.md) tracks the work.

This paper extends the "Interim path: PFT as an ERC-20" section of the [integration plan](integration-plan.md) (`integration-plan.md:71-74`, `:719-737`). That section tied the scoping to Coinbase's interest (`:736-737`), which came with the product owner's message of 2026-10-23. Coinbase wants to start getting ramped up, he is assembling a document for them, and in his view Coinbase cannot get this on mainnet imminently. He also asked whether PFT itself could be the first NAVCoin with an ERC-20 form. Section 6 answers that.

## 2. Terms

- **PFTL** is this L1, Post Fiat L1, the chain in this repository (`docs/start/glossary.md:12-14`).
- **PFT Ledger** is the separate, older rippled-fork network run by `postfiatd`, where Task Node pays today (`docs/start/glossary.md:15-22`). This paper keeps the two apart.
- **Native PFT** is the base asset of PFTL, created only at genesis (`docs/status/OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:12-13`).
- **Issued asset** is an asset an issuer creates on PFTL, such as pfUSDC; it is not native PFT (`OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:48-49`).
- **NAVCoin** is an issued asset whose supply, mint and redeem are tied to proven reserve packets and a floating net asset value (`docs/navcoins/index.md:9-12`).
- **wA666** is the ERC-20 on Ethereum mainnet that represents the A666 NAVCoin (`docs/navcoins/assets-and-venues.md:29-33`).
- **ERC-20** is the standard fungible-token interface on Ethereum and other EVM chains.
- **Base** is Coinbase's own Ethereum layer-2 network, built on the OP Stack and named on its listings page (coinbase.com/listings, 2026-10-26). The ERC-20 PFT is to be deployed there.

## 3. Where PFT exists today

**On the PFT Ledger.** Task Node rewards are payments from a "Task Node distribution wallet", most with encrypted `pf.ptr/v4` reward pointers (`docs/governance/tasknode-unl-shadow-run-20260904.md:36-43`). Our completed plans record those rewards in PFT, for example 4.5 PFT for one task (`docs/plans/completed/cobalt-activate-or-retire-milestone.md:100`). The PFT Ledger's PFT supply and issuer are not recorded in this repository. postfiat.org says Task Node pays in PFT, PFT has a fixed supply and every fee is burned (postfiat.org, 2026-10-26).

**On PFTL's devnet.** The genesis constant is `GENESIS_NATIVE_SUPPLY_ATOMS = 1_000_000_000` (`crates/types/src/core_chain.rs:10`). Wallets show 1 PFT as 1,000,000 atoms, so the devnet holds 1,000 PFT; that decimal convention is not a Rust constant (`docs/listings/coinbase/comparisons.md:96-101`). Genesis is the only creation event, and NAV and bridge records cannot create native PFT. Native PFT has no asset id. PFTL is a controlled devnet, not a production mainnet (see also `OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:12-13`, `:69`, `docs/rpc/account-history.md:73`, `integration-plan.md:22-29`).

**On Base or any other EVM chain.** Nowhere on a mainnet yet. The token contract is on main and deployed only on the Base Sepolia testnet (`crates/pft-token-contracts/src/PFTToken.sol`, section 11); no bridge, proof profile or threat model exists (`integration-plan.md:729`). postfiat.org mentions no ERC-20, bridge or migration for PFT (postfiat.org, 2026-10-26).

**The supply number.** The total supply is 100 billion PFT, fixed, decided by the product owner on 2026-10-28; it is not yet a protocol artifact, and no production genesis, allocation table or vesting escrow exists (`integration-plan.md:99-105`). The devnet still holds 1,000 PFT, a gap the 2026-10-23 handoff left open (`docs/handoffs/2026-10-23___codex__chain_pr_merge_handoff.md:198-200`). Token migration from the PFT Ledger is out of scope of the launch spec and left to "a separate document", which does not exist (see also `docs/architecture/l1v2-testnet-genesis-and-launch-spec.md:27`, `:402`).

## 4. What Coinbase can take

Coinbase's listings page says it can integrate ERC-20 tokens on Base, Ethereum, Optimism, Arbitrum and Polygon, and Solana SPL and ARC-20 tokens, with relatively little engineering work. Native blockchains and other asset types need extra resources and are prioritised lower. Its 2020 technical criteria for ERC-20s are verified source code, industry-standard libraries, restrictions on privileged participants, and a simple modular design. There is a separate track for tokens that have not launched yet (all coinbase.com/listings, 2026-10-26; the page text is about two years old).

Our listing package assumes the native chain. Coinbase would run its own nodes, treat one commit certificate as final, and sign with ML-DSA-65 (`integration-plan.md:42-47`). Today there is no mainnet, no external audit and a Foundation-run validator set (`:52`, see also `:58`).

Coinbase custody cannot sign ML-DSA-65, and its post-quantum signing pipeline is about mid-2027 (`integration-plan.md:59`, `:343`). So even after PFTL's mainnet, Coinbase would most likely keep the ERC-20 as its listed form for a long time. The ERC-20 is therefore not a stopgap but the listing form for the foreseeable future, and the native bridge serves holders who want to move. The package calls the ERC-20 the interim path, and its first ask to Coinbase is which route to plan on (see also `:71-74`, `:741-743`).

Three precedents bracket the choice. INJ ran as an ERC-20 on Ethereum for years and was listed on Coinbase in that form. Coinbase supported the migration to native INJ on the Injective EVM on 2026-07-20..22. It converted customer balances 1:1 at no cost, then ended ERC-20 INJ deposits and withdrawals; self-custodied holders used the Injective Bridge (Coinbase Markets on X and Coinbase help centre "Token delistings and migrations", June–July 2026; cryptobriefing.com, 2026-06-18). dYdX moved its ERC-20 to the native chain token through a one-way bridge that closed on 2025-06-13. More than 41.6 million ethDYDX were left behind, unmigratable and unlisted, and holders now ask governance to reopen it (forklog.com, 2025). Fetch.ai keeps a two-way bridge between ERC-20 FET and native FET (fetch.ai docs, 2026-10-26).

## 5. The five technically possible forms

Sizes are my estimates in working days, without audit time.

| Form | What it is | Exists / missing | Main risk | Days |
| --- | --- | --- | --- | ---: |
| A. Treasury ERC-20, one-way swap at mainnet | Fixed-supply ERC-20 held by a treasury; holders later swap into native PFT reserved at genesis | Exists: migration precedent, the token (contract on main, Base Sepolia test deployment, section 11), Ethereum-to-PFTL ingress. Missing: the token's Base mainnet deployment and the open decisions of the token document, the genesis reserve, the release path | Backed only by the genesis promise until mainnet | 2 + 5–10 |
| B. Bridged wPFT after mainnet | Native PFT locked on PFTL; an equal ERC-20 minted on Base against a finality proof | Exists: A666 export route, verifier, relay. Missing: a native-PFT export lane, a custody lane, route generalisation, threat model | Only as strong as the PFTL validator set | 10–15 |
| C. PFT-backed NAVCoin with an ERC-20 | An issued NAVCoin holding native PFT in reserve, exported like wA666 | Exists: the A666 stack. Missing: a PFT reserve adapter, a PFT valuation source, route generalisation | Coinbase reviews an issued product, not PFT | 8–12 |
| D. ERC-20 now, "external" lane at genesis | Fixed ERC-20 now; genesis places the same native amount in the external supply lane, released on bridge-in | Exists: the four-way supply partition, the A666 opening precedent. Missing: genesis support for the lane, the inbound release path | Supply number chosen now binds genesis | 8–12 |
| E. ERC-20 backed by PFT on the PFT Ledger | Lock PFT Ledger PFT and mint an ERC-20 against it | Exists: only read-only PFT Ledger readers for Task Node scoring. Missing: everything, outside this repository | A custodial bridge on a testnet, then a second migration | 10–15 |

**A. Treasury ERC-20, one-way swap.** The precedent is `A651ToA666MigrationV1`, an ownerless, fixed-ratio retirement path funded from a frozen holder snapshot (`crates/ethereum-contracts/src/A651ToA666MigrationV1.sol:14-18`). A token template with a lockable controller exists (`crates/ethereum-contracts/src/PFTLUniswapHandoffController.sol:903`). Ethereum-to-PFTL ingress runs on a disclosed BFT checkpoint plus receipt inclusion. Missing are the token, a genesis reserve and the token document. A's 5–10 days are the one-way swap alone, without the genesis external lane that form D adds (see also `docs/status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md:391`, `l1v2-testnet-genesis-and-launch-spec.md:402`).

**B. Bridged wPFT after mainnet.** This is the route the integration plan sketched (`integration-plan.md:729-731`). The A666 export route and relay run today (`deployments/a666-export-relay-mainnet-20260731/README.md:3`, `:7-12`). The route requires a registered NAV asset, so native PFT cannot use it yet. The primary-market contract hard-codes A666 constants. The token is only as strong as the validator committee (see also `crates/execution/src/nav_vault_asset_execution.rs:6470-6477`, `docs/navcoins/navcoin-create-and-swap-spec.md:778`, `integration-plan.md:733-734`).

**C. PFT-backed NAVCoin.** Section 6 covers it.

**D. External lane at genesis.** The whitepaper already partitions supply into native, shielded, external and in-flight claims (`docs/whitepaper.md:66-70`). A666 opened with its whole supply locked in a migration contract (`docs/whitepaper.md:42`). Today's native invariant counts only live custody and fee burns (`OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:23`), so a genesis external lane is new consensus work.

**E. PFT Ledger-backed ERC-20.** This repository only reads frozen PFT Ledger views for Task Node validator scoring (`docs/plans/completed/tasknode-unl-mvp-plan.md:196`, `:216`). The PFT Ledger is a testnet (`l1v2-testnet-genesis-and-launch-spec.md:33`). Holders would need a second migration to PFTL later.

## 6. The product owner's idea: PFT as the first NAVCoin

It is a good instinct. The NAVCoin stack is our most finished cross-chain product, and wA666 already lives on Ethereum mainnet (`docs/navcoins/assets-and-venues.md:39`). But native PFT cannot itself be a NAVCoin. NAVCoins are issued by issuers under registered profiles (`docs/whitepaper.md:34`), and a NAV mint names its issuer, while native PFT comes only from genesis (section 3; see also `crates/types/src/transactions_mempool_receipts.rs:1039-1068`).

What could be built is a NAVCoin that holds native PFT in reserve, on the A666 stack. Coinbase would then review an issued, reserve-backed product with issuer and reserve questions. Its value would float with its reserve (`docs/navcoins/index.md:9-12`). The listing would not be PFT itself; our package keeps NAVCoins outside the native PFT integration (`integration-plan.md:66-68`). So the NAVCoin stack stays the product beside PFT, and PFT's own ERC-20 is a plain token.

## 7. Recommendation: ERC-20 first, bridge at mainnet

This is the Injective pattern (Coinbase help centre "Token delistings and migrations", June–July 2026). It combines forms A and D now and leaves B for later.

**Phase 1, now (about two days of engineering plus the audit).**

- One fixed-supply ERC-20 PFT contract on Base, because it is Coinbase's own network and the simplest listing path, its fees are low, and a plain token needs none of our Ethereum-mainnet contracts (coinbase.com/listings, 2026-10-26).
- Built from the OpenZeppelin `ERC20` and `ERC20Permit` libraries, pinned as a submodule (`.gitmodules:7-9`) with its remapping in `crates/pft-token-contracts/foundry.toml`.
- No mint after construction, no pause, no blacklist, no upgrade path and no admin role. These properties are meant to address Coinbase's four published criteria (verified source, standard libraries, no privileged participants, one simple contract; coinbase.com/listings, 2026-10-26); the source verification and Coinbase's own assessment are still to come.
- Six decimals, the same unit as native PFT on PFTL (1 PFT = 1,000,000 atoms, `docs/listings/coinbase/comparisons.md:96-101`). One ERC-20 unit equals one atom, so the 1:1 bridge needs no conversion. Coinbase and wallets handle 6-decimal tokens routinely: USDC uses 6 decimals, and pfUSDC's bridge unit matches it (`docs/specs/pfusdc-arc-tier4-spec-20260828.md:47`).
- The whole supply minted at construction to a multi-signature treasury, a Safe multisig on Base. It holds the product owner's keys with a threshold he sets, and is never a single key. The deployer key receives no tokens and has no privilege; the deployment procedure requires a fresh key and retires it afterwards.
- Source to be verified on the block explorer at deployment.
- Deployed first on Base Sepolia with a deploy script and tests in their own Foundry project, `crates/pft-token-contracts`, built like the existing scripts and tests (`crates/ethereum-contracts/script/DeployERC20Bridge.s.sol:38-67`, `crates/ethereum-contracts/test/ERC20BridgeVaultL1.t.sol`).
- An audit before mainnet deployment. An audit of one ~100-line fixed-supply ERC-20 by an established firm typically takes one to three weeks of calendar time and is the product owner's budget item; the two days are our engineering time only.

**Phase 2, at PFTL's mainnet genesis (about one to two weeks).** This is form D's missing work: the genesis external lane and the inbound release path.

- Genesis reserves the same amount of native PFT in the "external" lane of the four-way partition (`docs/whitepaper.md:66-70`).
- A holder locks ERC-20 PFT in a contract on Base and receives native PFT on PFTL. The proof follows the BFT-checkpoint ingress used for pfUSDC deposits from Ethereum (`A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md:391`).
- Base is an EVM chain built on the OP Stack, so two parts apply as they are. The lock contract can follow `ERC20BridgeVaultL1`, whose deposit event is the whole ingress evidence (`crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol:6-16`), and the `evm-erc20-bft-checkpoint-mpt-v1` adapter proves an ERC-20 balance against a quorum-signed state root of whichever EVM chain the checkpoint names (`tools/nav-reserve-proof/README.md:31-35`). The Arbitrum variant `ERC20BridgeVaultV2` does not carry over, because it sends each deposit through ArbSys (see also `ERC20BridgeVaultV2.sol:22`, `ERC20BridgeVaultL1.sol:8-9`).
- The new work for Base is a checkpoint source for the ingress proof. The SP1 ingress program verifies Ethereum beacon-chain finality and is pinned to Sepolia's chain id (`programs/pfusdc-eth-ingress/src/lib.rs:19`, `:91`), and nothing in the repository reads Base today: the Base entries in the NAVCoin docs are Ethereum registry records, not Base deployments (see also `docs/navcoins/assets-and-venues.md:117-119`, `docs/navcoins/uniswap-pool.md:113-114`).
- The other new part is releasing native PFT from the reserved lane. Today the vault bridge credits issued assets, not native PFT (`docs/pfusdc-issuance-paths-inventory.md:55`).
- The bridge has no holder snapshot and stays open with no closing date. That is the dYdX lesson (forklog.com, 2025).

**Phase 3, later and optional (about two weeks).**

- The reverse direction exports native PFT as the same ERC-20 through a generalised export route. Today the route requires a NAV asset (`nav_vault_asset_execution.rs:6470-6477`).
- ERC-20 PFT bridged in during Phase 2 stays locked in the Base lock contract. A Phase 3 export of native PFT releases ERC-20 PFT from that same contract, so the ERC-20 supply never changes. The supply invariant on PFTL counts the exported amount in the external lane (`docs/whitepaper.md:66-70`).
- That makes the token two-way, as Fetch.ai's is (fetch.ai docs, 2026-10-26).
- Release on Base needs a PFTL finality check there. The vault already releases against `PFTLFinalityVerifierV1`, an EVM contract (`ERC20BridgeVaultL1.sol:13-16`), but today's verifier and relay run on Ethereum mainnet (`deployments/a666-export-relay-mainnet-20260731/README.md:7-12`), so Base needs its own deployment of them.
- **Fixed-supply reverse-bridge limit.** The reverse bridge can only release ERC-20 PFT that was bridged in, so a holder who only ever held native PFT can export only while such a balance exists.
- **Phase 1 contract choice.** The alternative, a token that mints on export and burns on import, is a different contract that must be chosen in Phase 1; my lean stays the fixed-supply token, because it meets Coinbase's "no privileged participants" criterion and is simpler to audit.

The day counts assume one engineer who knows the repository, the existing BFT-checkpoint ingress reused with a Base checkpoint source added, and no audit time; consensus work in Phase 2 may run longer.

**Decided.**

- The total supply number (`integration-plan.md:99-105`): decided 2026-10-28, 100 billion PFT, fixed.

**What this path does not solve.** These questions belong in the token document. They are yours to decide, with counsel on the legal ones, including the securities framing; I do not decide them.

- The allocation, that is, how much supply is in the ERC-20 at launch and how much stays in genesis for the Foundation, validators and the Task Node pool (`integration-plan.md:710-711`). The ERC-20 supply at launch is one slice of the decided 100 billion PFT total supply, and the genesis lane reserves exactly that slice, so the ERC-20 never exceeds its share of genesis.
- Whether PFT already paid on the PFT Ledger is swapped, after a snapshot, into the ERC-20 or into native PFT at genesis. If you choose that one-time swap, it follows the frozen-snapshot pattern of `A651ToA666MigrationV1` (`A651ToA666MigrationV1.sol:14-18`), which the ERC-20 bridge does not use.
- PFT's legal classification, which the package already lists as a question for counsel (`integration-plan.md:749-751`).

## 8. Risks

- **Single-key custody would be the largest operational risk.** The treasury holds the whole supply, so Phase 1 puts it in a multisig, never under one key.
- **The audit, not the two days, sets Phase 1's calendar.**
- **The six-decimal unit is a wallet convention, not a code constant on PFTL** (`comparisons.md:96-101`). The token's `decimals()` must be fixed at 6 in the contract and the genesis lane must use the same unit, or the 1:1 claim breaks.
- **Before mainnet, the ERC-20 is backed by the genesis promise only.** No chain holds native PFT behind it until Phase 2.
- **The listing still depends on Coinbase's review of the project, not only the contract.** Coinbase's process includes legal, compliance and technical-security review (`integration-plan.md:142`).
- **A one-way bridge that closes strands holders.** dYdX left more than 41.6 million tokens behind (forklog.com, 2025).
- **The supply number binds genesis later.** It is now decided, 100 billion PFT on 2026-10-28, and the devnet still differs at 1,000 PFT (`2026-10-23___codex__chain_pr_merge_handoff.md:198-200`).
- **The Foundation-run validator set is the trust base of any later bridge** (`integration-plan.md:58`, `:733-734`).

## 9. Decisions for the product owner

1. **Decided.** The total supply number: decided 2026-10-28, 100 billion PFT, fixed.
2. **Open.** The treasury multisig's signers and threshold; they are your keys.
3. **Open.** The allocation and the treatment of PFT Ledger holders.
4. **Open.** Whether to tell Coinbase now that the ERC-20 is the path, answering the package's first ask (`integration-plan.md:741-743`).

On 2026-10-28 the product owner also decided that PFT gets an SPL version on Solana alongside the Base ERC-20; it is not this lane's work, and no bridge route for it is designed.

## 10. What is being built

1. The ERC-20 contract, its tests and the Base Sepolia deploy script exist (`crates/pft-token-contracts/src/PFTToken.sol`, `crates/pft-token-contracts/test/PFTToken.t.sol`, `crates/pft-token-contracts/script/DeployPFTToken.s.sol`). The deployment waits for the open items among the four decisions in section 9.
2. Next, a one-page note for Coinbase built from Appendix A.
3. Then the Phase 2 design note for the genesis external lane and the lock-and-release path.

Also decided by the product owner on 2026-10-28: the product owner's target for token generation is late Q1 2027, as the ERC-20 on Base; the development organization's allocation unlocks monthly over five years from an on-chain escrow (planned); and the Base ERC-20 is a slice of the 100 billion, the rest being native PFT at mainnet genesis, with the slice size still open.

## 11. Launch state

As of 2026-10-28.

| Exists today | Open |
| --- | --- |
| The token contract and deploy script on main (`crates/pft-token-contracts`), reviewed, 127 tests | The Base slice size |
| The Base Sepolia test deployment, 2026-10-28, source verified: token `0x18fE4ca51A0cE042688D5A54917319d03B527D92`, treasury Safe `0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB` | The launch allocation |
| The Phase 2 lock contract on main (`PFTLockV1`, `39bfb547`) | The issuing entity |
| The Python state reader and read-only page (`python/postfiat_rpc/pft_token.py`, `python/postfiat_rpc/pft_token_ui.py`) | The treasury signers and threshold |
| The total supply, decided 2026-10-28: 100 billion PFT | The PFT Ledger treatment |
| The Solana SPL version, a separate product decision; its own supply accounting is still to be defined by the product owner | The audit |
| | The mainnet deployment |

## Appendix A: text for the Coinbase document

Paste-ready once the open decisions in section 9 are made.

On Base, PFT is to exist as a standard ERC-20 token built only from OpenZeppelin ERC20 and ERC20Permit, release v5.7.0. It has no owner or admin role and no further mint, pause, blacklist or upgrade path. Wallets show native PFT with six decimals (1,000,000 base units per whole PFT); one whole ERC-20 PFT is likewise 1,000,000 base units. The whole ERC-20 supply, the slice, is minted once at deployment to a treasury planned to be a Safe multisig, which the deployment procedure checks; the contract gives it no privilege beyond holding the supply. An independent audit is planned before the Base mainnet deployment, and its report is planned to be shared with Coinbase; the source is to be verified on the Base block explorer at deployment. Post Fiat L1 is not yet on mainnet; its genesis is designed to reserve, in the supply partition's external category, one native base unit per ERC-20 base unit. A bridge is designed to exchange ERC-20 PFT for native PFT one for one in quantity, relying on signed validator-committee checkpoints and governed release limits, with no planned closing date; the reverse direction is optional. The total supply of PFT is 100 billion, fixed, decided on 2026-10-28. Launch quantity and allocation are undecided; the ERC-20 is designed as a slice of native genesis supply, not additional supply; the token document will fix both before any Base mainnet deployment.

## Appendix B: sources

The paper cites these repository files.

- `docs/listings/coinbase/integration-plan.md`, `docs/listings/coinbase/comparisons.md`
- `docs/start/glossary.md`, `docs/whitepaper.md`, `docs/rpc/account-history.md`
- `docs/status/OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md`, `docs/status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md`
- `docs/governance/tasknode-unl-shadow-run-20260904.md`, `docs/plans/completed/cobalt-activate-or-retire-milestone.md`
- `docs/handoffs/2026-10-23___codex__chain_pr_merge_handoff.md`, `docs/architecture/l1v2-testnet-genesis-and-launch-spec.md`
- `docs/navcoins/index.md`, `docs/navcoins/assets-and-venues.md`, `docs/navcoins/navcoin-create-and-swap-spec.md`, `docs/pfusdc-issuance-paths-inventory.md`, `docs/specs/pfusdc-arc-tier4-spec-20260828.md`
- `deployments/a666-export-relay-mainnet-20260731/README.md`
- `crates/types/src/core_chain.rs`, `crates/types/src/transactions_mempool_receipts.rs`, `crates/execution/src/nav_vault_asset_execution.rs`
- `crates/ethereum-contracts/src/A651ToA666MigrationV1.sol`, `crates/ethereum-contracts/src/PFTLUniswapHandoffController.sol`, `crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol`, `crates/ethereum-contracts/src/ERC20BridgeVaultV2.sol`
- `programs/pfusdc-eth-ingress/src/lib.rs`, `tools/nav-reserve-proof/README.md`, `docs/navcoins/uniswap-pool.md`
- `crates/ethereum-contracts/script/DeployERC20Bridge.s.sol`, `crates/pft-token-contracts/foundry.toml`, `.gitmodules`
- `docs/plans/completed/tasknode-unl-mvp-plan.md`

I verified these outside sources on 2026-10-26.

- Coinbase listings page, <https://www.coinbase.com/listings> (text about two years old).
- Coinbase Markets on X, on the Injective migration, <https://x.com/CoinbaseMarkets/status/2067313744741355774>; Coinbase help centre, "Token delistings and migrations", <https://help.coinbase.com/en/coinbase/trading-and-funding/sending-or-receiving-cryptocurrency/token-migrations> (June–July 2026); cryptobriefing.com, 2026-06-18, <https://cryptobriefing.com/injective-migration-native-inj/>.
- forklog.com, 2025, on the dYdX bridge closure, <https://forklog.com/en/dydx-tokens-worth-26m-stranded-on-ethereum/>.
- fetch.ai documentation on the FET token bridge, <https://fetch.ai/docs/guides/fetch-network/how-to-convert-fet-to-and-from-erc20>.
- postfiat.org, <https://postfiat.org>.
