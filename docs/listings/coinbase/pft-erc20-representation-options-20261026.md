# An ERC-20 form of PFT before mainnet: options and a recommended path

**Status:** Draft, 2026-10-26, revised 2026-11-03. Operator dravlic, for the product owner's Coinbase document. Decisions and recommendation unchanged; the state is `main` on 2026-11-03. **Locked** on 2026-11-03 as the research specification (milestone M1). Text Improvement Harness full gate, five runs per judge, scored without this line. Earlier: 85.67 on 2026-10-26; 82.60, 84.00, 79.60 and 84.00 on 2026-10-28 (run groups `pft-erc20-options-20261028-r6` to `-r9`). 2026-11-03, as it stood: 84.13 (GPT-6 Astra Pro 82.60, Claude Fable 5.1 81.20, GLM 5.3 88.60; `pft-erc20-options-20261103-r10`, `80ae6529c6546048…`). After one improvement pass on those critiques through `openai/gpt-5.6-sol-pro` (AGENTS.md step 4) and a fact check against main: 87.27 (85.00, 86.20, 90.60; `pft-erc20-options-20261103-r11`, `c3afe5bc9dee1cb6…`), at or above 86, so it is locked. Task Node task `task_8c15b90660d7c39f5b939352a7781490`.

## 1. BLUF

1. **Recommendation.** “ERC-20 first, bridge at mainnet”: launch PFT as a plain, fixed-supply ERC-20—a standard fungible token for Ethereum-compatible networks—on Base, Coinbase’s Ethereum layer-2 network. At Post Fiat L1 (PFTL) mainnet genesis, open a bridge: a 1:1 lock-and-release path from Base PFT into native PFT. Do not close it. The active [milestone](../../plans/active/pft-erc20-base-listing-milestone.md) tracks this work. [R1]
2. **Why.** Coinbase can integrate ERC-20 tokens with less engineering than a new native blockchain. PFTL has no production mainnet, external audit or Coinbase-compatible custody path today. Coinbase custody cannot yet sign ML-DSA-65, the post-quantum signature scheme used by PFTL, and its planned support is around mid-2027. The Base token is therefore the practical Coinbase form for the foreseeable future, not a temporary wrapper. [R2] Injective followed this ERC-20-first path. The dYdX migration shows why the bridge must never close. Fetch.ai shows that a later two-way path is possible. [R25] [R26] [R27]
3. **Decisions.** The network, the token design, the fixed 100-billion-PFT total, the never-closing bridge, the late-Q1-2027 target, the five-year development-organization escrow and the Solana SPL form are settled. Section 9 records them once.
4. **Open items.** Section 9 holds the single list: the Base slice, the launch allocation, the Solana supply accounting, the issuing entity, the production treasury signers, the PFT Ledger treatment, the audit and the timing of the Coinbase notice.
5. **Current state.** The Base token, Base Sepolia test deployment, genesis external-lane support, Phase 2 lock contract, guarded lock deployment process, Python command-line interface and read-only page exist. No production bridge operates. No external lane is configured on any network. Native release, a Base checkpoint source, production qualification and the lock-contract audit remain open. [R1] [R16] [R17] [R18] [R19] [R20] [R21] [R22]
6. **Cost and calendar.** Earlier engineering estimates remain useful only for comparing the five forms in section 5; they are not current completion forecasts. The independent token audit was estimated at one to three calendar weeks. Audit and qualification, not contract coding, now set the immediate calendar.

This document extends the [integration plan](integration-plan.md)’s interim ERC-20 path. That plan tied the work to Coinbase’s interest and asked which integration route Coinbase should plan around. [R2] The product owner’s 2026-10-23 message said Coinbase wanted to begin ramping up, that he was preparing a document for Coinbase, and that native-mainnet integration was not imminent. He also asked whether PFT could be the first NAVCoin with an ERC-20 form. Section 6 answers that question. [R28]

## 2. Terms

- **PFT** is the asset discussed in this document.
- **Post Fiat L1 (PFTL)** is the chain in this repository. [R3]
- **PFT Ledger** is the separate, older rippled-fork network operated by `postfiatd`. It is where Task Node rewards are paid today. It is not PFTL. [R3]
- **Task Node** is the current system that assigns work and pays rewards on the PFT Ledger. [R6]
- **Native PFT** is PFTL’s base asset. It is created only at genesis. It has no asset identifier. [R4]
- **Atom** is the base-unit convention used by PFTL wallets: 1 PFT is displayed as 1,000,000 atoms. [R7]
- **Issued asset** is an asset created by an issuer on PFTL, such as pfUSDC. It is not native PFT. [R4]
- **NAVCoin** is an issued asset whose minting, redemption and supply are tied to proven reserve packets and a floating net asset value. [R5]
- **A666** is an existing NAVCoin. **wA666** is its ERC-20 representation on Ethereum mainnet. [R5]
- **ERC-20** is the standard fungible-token interface used on Ethereum-compatible networks.
- **Ethereum Virtual Machine (EVM)** is the execution environment used by Ethereum-compatible chains.
- **Base** is Coinbase’s EVM layer-2 network, built on the OP Stack. **Base Sepolia** is its test network and carries no production value. [R24]
- **Solana SPL form** means the separately approved token form on Solana.
- **Safe** is the multisignature wallet software used for the treasury. A multisignature wallet requires a configured threshold of its owners to approve an action.
- **Bridge** means the 1:1 path between Base PFT and native PFT. Inbound bridging locks ERC-20 PFT on Base and releases the corresponding native amount on PFTL.
- **External lane** is a genesis supply record reserved for externally represented PFT. It tracks the amount reserved, the amount released, paid Lock nonces and release-window fields. A **Lock nonce** identifies a Base Lock event already paid on PFTL. [R18]
- **BFT checkpoint** is a Byzantine-fault-tolerant checkpoint signed by a quorum. In Phase 2, it attests to Base state.
- **Bridge committee** is the six-member signing committee used by the Phase 2 design. A 5-of-6 quorum is required. Below quorum, the bridge halts and releases nothing. [R19]
- **Receipt proof** proves that a particular Lock event appears in a transaction receipt covered by the checkpoint.
- **Release window** is the block interval over which the genesis-set release cap applies.
- **PFTL finality verifier** is an EVM contract that checks finalized PFTL checkpoints before permitting a reverse release on Base.
- **Anchor** is the verifier’s stored latest finalized checkpoint, height and committee root.
- **SP1** is a zero-knowledge virtual machine. The **SP1 ingress program** is the existing proof program of the Ethereum ingress route. It verifies Ethereum beacon-chain finality, not Base finality. [R15]
- **Command-line interface (CLI)** is the Python tool that reads the token's state: address, total supply, decimals and treasury balance. [R22]

## 3. Where PFT exists today

**On the PFT Ledger.** Task Node rewards are paid from a Task Node distribution wallet. Most payments carry encrypted reward pointers. Completed plans record rewards denominated in PFT, including a 4.5-PFT task payment. [R6] This repository does not record the PFT Ledger’s PFT supply or issuer. Postfiat.org states that Task Node pays in PFT, PFT has a fixed supply and every fee is burned. [R27]

**On PFTL’s devnet.** The genesis constant remains 1,000,000,000 atoms. Under the wallet convention of 1,000,000 atoms per PFT, that is 1,000 PFT. Genesis is the only creation event. NAV and bridge records cannot create native PFT. PFTL remains a controlled devnet rather than a production mainnet. [R4] [R7]

Genesis external-lane support is now merged. Genesis may accept an optional list of external lanes. The faucet receives the native supply minus the amounts reserved to those lanes. Each lane records its reserved amount, released amount, paid Lock nonces and release-window state. Every native-supply check counts the lane remainder—reserved minus released—as native supply that is not in circulation. [R18]

Nothing can yet release from a lane. Until the `external_lane_release_v1` release operation is implemented, every history checkpoint and imported snapshot must carry exactly the genesis lane records with nothing released. No external lane is configured on any network. [R1] [R18]

**On Base.** No PFT token exists on Base mainnet. A test token exists on Base Sepolia with a test supply of 100 billion PFT. Its treasury is a two-owner Safe with a threshold of two. The source is verified on Sourcify and Blockscout. This deployment has no production value. [R17]

The Phase 2 lock contract, PFTLockV1, is also deployed on Base Sepolia. It was rehearsed in test mode against a stand-in verifier that always reverts, so nothing can ever be released from that deployment. Its per-lock cap was set to 2 billion PFT, 2% of the test supply. [R20]

**The bridge.** Components now exist, but the bridge does not. The design note, genesis lane, lock contract and read-only tools are present. The inbound native-release operation and Base checkpoint source are not. No PFTL finality verifier exists on Base, so the reverse path cannot operate there. [R1] [R19] [R21] [R22]

**The supply number.** The product owner fixed total PFT supply at 100 billion on 2026-10-28. The Base ERC-20 is one slice of that total, not additional supply. The amount of that slice remains open. Production genesis, the final allocation table and the planned vesting escrow do not yet exist. The devnet still contains 1,000 PFT. [R2] [R9] [R23]

Treatment of PFT already paid on the PFT Ledger remains outside the existing launch specification, which leaves token migration to a separate document that does not exist yet. The Solana SPL form is decided, but its supply accounting is still to be defined by the product owner; under the fixed total it cannot add units beyond 100 billion. [R9] [R23]

## 4. What Coinbase can take

Coinbase states that it can integrate ERC-20 tokens on Base, Ethereum, Optimism, Arbitrum and Polygon, as well as Solana SPL and ARC-20 tokens, with relatively little engineering. Native blockchains and other asset types require more resources and receive lower priority. Its published ERC-20 criteria emphasize verified source, standard libraries, limited privileged control and simple modular design. Coinbase also has a separate path for assets that have not launched. [R24]

The current native-chain package assumes Coinbase would run PFTL nodes, treat one commit certificate as final and sign with ML-DSA-65. PFTL has no production mainnet or external audit, and its current validator set is Foundation-run. [R2]

Coinbase custody cannot yet sign ML-DSA-65. Its planned post-quantum signing pipeline is around mid-2027. The Base ERC-20 is therefore the practical listed form even after PFTL mainnet. The native bridge serves holders who want native PFT. [R2]

Three precedents shape the recommendation:

- **Injective.** INJ operated as an Ethereum ERC-20 and was listed by Coinbase in that form. Coinbase later supported a 1:1 migration to native INJ, ended ERC-20 deposits and withdrawals, and directed self-custodied holders to the Injective Bridge. This supports the ERC-20-first path. [R25]
- **dYdX.** Its one-way bridge closed on 2025-06-13. More than 41.6 million ethDYDX remained behind, unmigratable and unlisted. This supports a bridge with no closing date. [R26]
- **Fetch.ai.** It maintains a two-way path between ERC-20 FET and native FET. This supports an optional reverse direction after the inbound bridge is proven. [R27]

## 5. The five technically possible forms

The day figures below are the original estimates for comparing the forms. They assume one engineer familiar with the repository and exclude audit time. Completed work is not subtracted, so the figures are not remaining-work forecasts.

| Form | What it is | Exists / missing as of 2026-11-03 | Main risk | Earlier estimate |
| --- | --- | --- | --- | ---: |
| A. Treasury ERC-20, one-way swap | Fixed-supply ERC-20 held by a treasury; holders later exchange it for native PFT | Exists: token, test deployment, migration precedent, genesis-lane capability and lock contract. Missing: production deployment, configured lane, native release and Base checkpoint source | Before mainnet, backing is only the genesis commitment | 2 + 5–10 days |
| B. Bridged wPFT after mainnet | Native PFT is locked on PFTL and an equal ERC-20 is minted on Base | Exists: A666 export route, verifier and relay patterns. Missing: native-PFT export support, Base verifier deployment, route generalization and production qualification | Security depends on PFTL finality and the operating validator set | 10–15 days |
| C. PFT-backed NAVCoin with an ERC-20 | An issued NAVCoin holds native PFT in reserve and exports an ERC-20 representation | Exists: A666 stack. Missing: PFT reserve adapter, PFT valuation source and route generalization | Coinbase would review an issued reserve product, not native PFT | 8–12 days |
| D. ERC-20 now, external lane at genesis | Fixed ERC-20 now; genesis reserves the same amount in an external lane and releases native PFT after Base locks | Exists: token, design note, genesis lane and lock contract. Missing: configured lane, release operation, Base checkpoint source, qualification and audit | The chosen Base slice binds genesis; the Phase 2 route trusts a committee | 8–12 days |
| E. ERC-20 backed by PFT on the PFT Ledger | PFT Ledger PFT is locked and an ERC-20 is minted against it | Exists: read-only PFT Ledger data used for Task Node scoring. Missing: the custody and bridge system | It creates a custodial bridge from a testnet and still requires a later PFTL migration | 10–15 days |

**A. Treasury ERC-20, one-way swap.** The existing A651-to-A666 migration is an ownerless, fixed-ratio retirement path funded from a frozen holder snapshot. A lockable controller template also exists. [R11] The recommended PFT bridge does not use a holder snapshot. It uses live Base locks and a genesis external lane. Form A therefore supplies the treasury-token pattern, while form D supplies the permanent conversion path.

**B. Bridged wPFT after mainnet.** The A666 export route and relay already operate on Ethereum. That route requires a registered NAV asset, and the primary-market code contains A666-specific assumptions. Native PFT cannot use it without generalization. [R5] [R12]

**C. PFT-backed NAVCoin.** Section 6 covers this form.

**D. External lane at genesis.** PFTL’s supply model already distinguishes native, shielded, external and in-flight claims. [R8] The optional genesis lane now implements the external record and includes its remainder in every native-supply check. The missing consensus operation is release from that record after a valid Base Lock. [R18]

**E. PFT Ledger-backed ERC-20.** The repository only reads frozen PFT Ledger views for Task Node validator scoring. The PFT Ledger is a testnet. Holders would still require a later migration to PFTL. [R6] [R9]

## 6. The product owner’s idea: PFT as the first NAVCoin

The idea uses the most complete cross-chain product in the repository. wA666 already exists on Ethereum mainnet. [R5]

Native PFT cannot itself be a NAVCoin. NAVCoins are issued under registered issuer profiles, and a NAV mint names its issuer. Native PFT is created only at genesis and has no issuer or asset identifier. [R4] [R8] [R13]

A separate NAVCoin could hold native PFT in reserve and use the A666 stack. Coinbase would then be reviewing an issued, reserve-backed product with an issuer, reserve proofs and a floating reserve value. It would not be listing PFT itself. The native integration package already keeps NAVCoins outside the PFT listing path. [R2] [R5]

PFT’s own Base form should therefore remain a plain fixed-supply ERC-20. NAVCoins remain a separate product beside PFT.

## 7. Recommendation: ERC-20 first, bridge at mainnet

The recommendation combines form A’s treasury issuance with form D’s genesis lane. Form B remains an optional later direction.

### Phase 1: Base ERC-20

**Done.**

- The token is a fixed-supply ERC-20 built from OpenZeppelin’s ERC20 and ERC20Permit libraries. OpenZeppelin is the standard contract library used by the project. The dependency and build mapping are pinned in the token project. [R16]
- The contract has six decimals. One ERC-20 base unit therefore corresponds to one PFTL atom under the current wallet convention. USDC and pfUSDC also use six-base-unit accounting. [R7] [R10]
- The complete supply is minted once at construction.
- The contract has no later mint, pause, blacklist, upgrade path, owner or administrator role.
- The deployer receives no contract privilege.
- The deployment tooling checks that the initial holder is a Safe.
- Tests and the Base deployment script exist.
- A Base Sepolia test token was deployed on 2026-10-28 with a test supply of 100 billion PFT. The Safe has two owners and a threshold of two. The source is verified on Sourcify and Blockscout. [R17]

The fixed-supply design is settled (section 9). The token does not mint on export or burn on import. This meets Coinbase's "no privileged participants" criterion, keeps the audit simple, and brings the reverse-bridge limit described under Phase 3.

**Remaining before Base mainnet.**

- Close the section 9 decisions, including the supply accounting.
- Complete the independent token audit.
- Verify production source at deployment.
- Deploy to Base mainnet.

The original estimate of about two engineering days applied to implementing the simple token. That work is complete. The audit estimate remains one to three calendar weeks and excludes vendor procurement and remediation.

### Phase 2: inbound bridge at PFTL mainnet genesis

The Phase 2 milestone has five items. P2.1 through P2.3 are done. P2.4 and P2.5 remain open. [R1]

**P2.1 — design note: done, not locked.**

The design uses a bridge-committee-signed BFT checkpoint plus a receipt proof for the Base Lock event. It does not use Base dispute games for the initial route; that trustless route remains a later upgrade. The bridge is therefore committee-trusted. [R19]

A 5-of-6 committee quorum is required. Below quorum, the bridge halts and releases nothing. A colluding quorum could release native PFT with no Lock behind it, so genesis sets a release cap that limits releases to a fixed share of the lane per window. The design lean is 2% per window, with one window equal to one day’s blocks. The committee changes only by a governed epoch activation. [R19]

**P2.2 — genesis external lane: done, not configured.**

Genesis can accept optional external lanes. For each lane, the faucet receives the native supply minus its reservation. The lane stores reserved and released amounts, paid Lock nonces and window state. The `reserved_atoms` genesis parameter must equal the final Base ERC-20 slice. [R18]

The release operation does not exist. Until it does, checkpoints and imported snapshots must preserve the exact genesis lane records with zero released. No network currently configures a lane. [R1] [R18]

**P2.3 — Base lock contract: done, test-only.**

PFTLockV1 follows `ERC20BridgeVaultL1`, the existing Ethereum vault whose deposit event is the whole ingress evidence. [R14] It accepts Base PFT locks with no holder snapshot and no closing date. Each lock is capped by `maxLockAmount`, fixed at construction to the genesis per-window release cap. The Base Sepolia rehearsal set it to 2 billion PFT and used a reverting stand-in verifier, so that deployment can never release tokens. [R20]

The deployment script refuses to deploy unless the proposed PFTL finality verifier’s stored anchor and all chain, genesis, route, asset and vault identifiers match operator-pinned values. It re-reads those values after deployment. No PFTL finality verifier exists on Base, so only test runs are possible. [R21]

The lock contract already contains the dormant Phase 3 release path. Construction binds that path to an immutable PFTL finality verifier, as the lead developer decided on 2026-10-26. [R19]

**P2.4 — inbound release and Base checkpoint source: open.**

The new release operation must validate the committee checkpoint and Base receipt proof, reject already-paid Lock nonces, enforce the genesis cap and window, reduce the lane remainder and release native PFT.

The current SP1 ingress program verifies Ethereum beacon-chain finality and is pinned to Sepolia. The repository does not contain a Base checkpoint source. Existing Base references in the NAVCoin documentation are Ethereum registry records, not Base deployments. [R5] [R15]

The existing vault path credits issued assets, not native PFT. Native release from the genesis lane is therefore new work. [R10]

**P2.5 — qualification and lock-contract audit: open.**

Qualification before genesis (tests, a devnet run) and the PFTLockV1 audit remain milestone items. The lock contract is not deployed on Base mainnet before its audit is complete. [R1] [R18] The prior one-to-two-week Phase 2 estimate excluded audit time. Consensus work may exceed it.

**Operating rule.**

The inbound bridge uses no holder snapshot and has no closing date. This is the direct lesson from the dYdX bridge closure. [R26]

### Phase 3: optional reverse bridge

The reverse direction would export native PFT by releasing ERC-20 PFT already held in PFTLockV1.

- ERC-20 PFT locked during Phase 2 remains in the Base contract.
- A valid PFTL finality proof can authorize release from that locked balance.
- The ERC-20 total supply never changes.
- Base requires a deployed PFTL finality verifier and relay. The existing vault already releases against `PFTLFinalityVerifierV1`, an EVM contract, but today's verifier and relay run on Ethereum mainnet, not Base. [R12] [R14]
- The export route must be generalized because the current route requires a NAV asset. [R12]
- **Fixed-supply limit:** a holder who has only native PFT can export only while the Base lock contract holds enough previously imported ERC-20 PFT.

This limit is the accepted consequence of the settled no-mint design. A mint-on-export and burn-on-import token would be a different contract and is not the chosen path.

## 8. Status and limitations

As of 2026-11-03:

| Area | Current status | Production limitation |
| --- | --- | --- |
| Base token | Contract, tests and deployment script exist; Base Sepolia deployment is source-verified | No Base mainnet deployment or completed independent audit |
| Genesis lane | Merged on `main`; supply checks include the unreleased remainder | No lane configured; no release operation |
| Phase 2 design | Checkpoint-plus-receipt route and committee policy documented | Note is not locked; route is committee-trusted |
| Lock contract | Merged and rehearsed on Base Sepolia | Test verifier always reverts; no Base PFTL verifier |
| Operator tooling | Python CLI and read-only page exist | Read-only; they do not make the bridge operational |
| Milestone | P2.1, P2.2 and P2.3 done | P2.4 and P2.5 open |

The material limitations are:

- **Before PFTL mainnet, Base PFT is backed by the genesis commitment only.** No production chain holds native PFT behind it.
- **No production bridge exists.** The lock contract and lane are components, not an operating release path.
- **The inbound bridge trusts the bridge committee.** A 5-of-6 quorum can authorize state. The genesis release cap limits the amount exposed per window but does not make the route trustless.
- **Below quorum, the bridge stops.** It never releases with fewer than five committee signatures.
- **Production treasury control remains open.** The test Safe is 2-of-2, but production signers and threshold are not decided. A single-key treasury is not acceptable.
- **The audit sets the Base deployment calendar.**
- **Six decimals are a wallet convention on PFTL, not a Rust protocol constant.** The token fixes six decimals in its contract, and genesis must use the same atom convention for the 1:1 claim to remain correct. [R7]
- **The reverse bridge has an inventory limit.** It can release only ERC-20 PFT already locked on Base.
- **The global supply model is incomplete until Solana and PFT Ledger treatment are recorded.** Neither may create units beyond the fixed 100 billion.
- **The 100-billion decision binds production genesis.** The controlled devnet still uses 1,000 PFT. [R9]
- **Coinbase reviews the project, not just the contract.** Its process includes legal, compliance and technical-security review. [R2]
- **PFT’s legal classification belongs to counsel.** The integration package already identifies it as a legal-review question. [R2]
- **The current validator environment remains a trust dependency.** The native integration package identifies a Foundation-run validator set, while the Phase 2 bridge separately trusts its bridge committee. [R2]

## 9. Decisions for the product owner

### Decided

By the product owner, 2026-10-28: [R23]

- Base is the network for the ERC-20 launch.
- The Base token is a fixed-supply ERC-20 with six decimals, its whole Base slice minted once to a Safe treasury.
- A 1:1 bridge into native PFT opens at PFTL mainnet genesis and has no closing date.
- Total PFT supply is 100 billion, fixed.
- The Base ERC-20 is a slice of that total, not additional supply; the rest is native PFT at mainnet genesis.
- Token generation is targeted for late Q1 2027 on Base.
- The development organization’s share unlocks monthly over five years from an on-chain escrow (planned).
- PFT will also have a Solana SPL form. It is a separate product decision and not this lane's work; no bridge route for it is designed.

By the lead developer, 2026-10-26: [R19]

- The Phase 2 proof is a bridge-committee-signed BFT checkpoint of a Base block plus a receipt proof of the Lock event; the route through Base's dispute games stays a later upgrade.
- The lock contract ships the dormant Phase 3 release path, bound at construction to an immutable PFTL finality verifier.

Because the token never mints after construction, reverse releases can use only ERC-20 PFT previously locked on Base (Phase 3).

### Open items

The Base mainnet deployment waits for these decisions.

| # | Item | Owner |
| ---: | --- | --- |
| 1 | Set the Base slice. This also sets the genesis lane’s `reserved_atoms`. | Product owner |
| 2 | Approve the launch allocation: how much stays in genesis for the Foundation, validators and the Task Node pool, and the development-organization escrow. [R2] | Product owner |
| 3 | Define the Solana SPL form's supply accounting inside the fixed 100 billion. | Product owner |
| 4 | Choose the issuing entity. | Product owner with counsel |
| 5 | Choose the production treasury Safe’s signers and threshold. | Product owner |
| 6 | Decide whether PFT already paid on the PFT Ledger is swapped, after a snapshot, into the ERC-20 or into native PFT at genesis. A one-time swap would follow the frozen-snapshot pattern of `A651ToA666MigrationV1`, which the bridge does not use. [R11] | Product owner |
| 7 | PFT's legal classification, which the integration package lists as a question for counsel. [R2] | Product owner with counsel |
| 8 | Set the audit budget and select the audit vendor; the audit sets the Base calendar. | Product owner |
| 9 | Decide when to tell Coinbase that the Base ERC-20 is the path, answering the package's first ask. [R2] | Product owner |

### Supply accounting

The fixed total is 100 billion PFT, that is 10^17 atoms. Let `B` be the Base slice and `x` the amount locked on Base and released natively.

| Balance | At genesis | After `x` is bridged in |
| --- | --- | --- |
| Base PFT held by users and the treasury | `B` | `B − x` |
| Base PFT held by PFTLockV1 | 0 | `x` |
| Native PFT in circulation | `100 billion − B` | `100 billion − B + x` |
| Native PFT in the external lane, not in circulation | `B` | `B − x` |

Circulating Base PFT plus circulating native PFT stays 100 billion. The ERC-20 total supply stays `B`; the locked `x` and the lane remainder are backing, never a second allocation. PFTL's supply checks already count the lane remainder. [R18] Before any lane-bearing genesis is produced, the genesis plan's build check (P2.5) compares the lane with the token document and the deployed token's total supply. [R18] The Solana form and any PFT Ledger swap must fit inside the same 100 billion (items 3 and 6).

## 10. Delivery order

| Order | Work | Precedes |
| ---: | --- | --- |
| 1 | The section 9 decisions, with the audit vendor chosen first | The audit and the Base deployment |
| 2 | The token audit, one to three calendar weeks, and its findings addressed | The Base mainnet deployment |
| 3 | The Base mainnet deployment, source verified | Token generation, targeted for late Q1 2027 |
| 4 | P2.4: the Base checkpoint source and the native lane release | The bridge |
| 5 | P2.5: qualification and the PFTLockV1 audit | The lock's Base mainnet deployment and the bridge |
| 6 | The Base lane configured in the mainnet genesis; the inbound bridge opens | PFTL mainnet genesis |
| 7 | The reverse direction, if approved | Later, optional (Phase 3) |

## Appendix A: text for the Coinbase document

Paste-ready after the section 9 decisions, supply accounting, legal review and audit are complete.

PFT is planned for Base mainnet as a standard fixed-supply ERC-20 built from OpenZeppelin ERC20 and ERC20Permit release v5.7.0. The contract has six decimals and has no owner, administrator, later mint, pause, blacklist or upgrade path. The entire Base allocation is minted once at deployment to a Safe multisignature treasury. The deployer receives no tokens or contract privilege.

Total PFT supply is fixed at 100 billion across native and external representations. The Base ERC-20 is one slice of that total, not additional supply. Its final quantity, launch allocation, treasury signers and threshold will be fixed before production deployment. The development organization’s allocation is planned to unlock monthly over five years from an on-chain escrow.

The Base token contract and deployment tooling exist and have been exercised on Base Sepolia, a test network with no production value. The test token’s source is verified on Sourcify and Blockscout. An independent audit is required before Base mainnet deployment, and the production source will be verified at deployment.

Post Fiat L1 is not yet on mainnet. Its genesis implementation can reserve one native atom in an external lane for each atom in the Base slice. A planned inbound bridge locks ERC-20 PFT on Base and releases native PFT one for one. It uses a receipt proof under a BFT checkpoint signed by a 5-of-6 bridge committee. Below quorum, it halts and releases nothing. A genesis-set release cap limits the share of the lane that can be released in each window. The bridge has no planned closing date.

The bridge is not operational today. The genesis lane and Base lock contract exist, but the native release operation, Base checkpoint source, production qualification and lock-contract audit remain open. The optional reverse direction can release only Base PFT previously locked in the contract; the ERC-20 supply never changes.

## Appendix B: sources

### Repository and decision references

| Label | Source | Claims supported |
| --- | --- | --- |
| [R1] | [`docs/plans/active/pft-erc20-base-listing-milestone.md`](../../plans/active/pft-erc20-base-listing-milestone.md) | Active milestone. P2.1 design, P2.2 genesis lane and P2.3 lock contract are done. P2.4 inbound release and Base checkpoint source, and P2.5 qualification and lock-contract audit, are open. |
| [R2] | `docs/listings/coinbase/integration-plan.md:22-29,42-47,52,58-59,66-68,71-74,99-105,142,343,710-711,719-739,729-734,741-743,749-751` | Native integration assumptions; mainnet and validator status; ML-DSA-65 custody gap; interim ERC-20 path; 100-billion decision record; Coinbase review; allocation questions; bridge sketch; Coinbase ask; legal review. |
| [R3] | `docs/start/glossary.md:12-22` | Distinction between PFTL and the PFT Ledger. |
| [R4] | `docs/status/OPEN-SOURCE-NATIVE-SUPPLY-INVENTORY-20260716.md:12-13,23,48-49,69` | Native PFT is created at genesis; native-supply invariant; issued assets differ from native PFT; controlled-network status. |
| [R5] | `docs/navcoins/index.md:9-12`; `docs/navcoins/assets-and-venues.md:29-33,39,117-119`; `docs/navcoins/navcoin-create-and-swap-spec.md:778`; `docs/navcoins/uniswap-pool.md:113-114` | NAVCoin definition; A666 and wA666; Base references are registry records; NAV-route requirements. |
| [R6] | `docs/governance/tasknode-unl-shadow-run-20260904.md:36-43`; `docs/plans/completed/cobalt-activate-or-retire-milestone.md:100`; `docs/plans/completed/tasknode-unl-mvp-plan.md:196,216` | Task Node distribution-wallet payments, encrypted reward pointers, example payment and read-only PFT Ledger use. |
| [R7] | `crates/types/src/core_chain.rs:10`; `docs/listings/coinbase/comparisons.md:96-101`; `docs/rpc/account-history.md:73` | Devnet genesis constant; 1 PFT to 1,000,000 atoms wallet convention; native-PFT handling. |
| [R8] | `docs/whitepaper.md:34,42,66-70` | NAVCoin issuers; A666 opening state; native, shielded, external and in-flight supply partition. |
| [R9] | `docs/handoffs/2026-10-23___codex__chain_pr_merge_handoff.md:198-200`; `docs/architecture/l1v2-testnet-genesis-and-launch-spec.md:27,33,402` | Devnet supply gap; PFT Ledger testnet status; migration treatment deferred from the launch specification. |
| [R10] | `docs/status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md:391`; `docs/pfusdc-issuance-paths-inventory.md:55`; `docs/specs/pfusdc-arc-tier4-spec-20260828.md:47` | Existing checkpoint ingress precedent; current bridge credits issued assets; six-decimal pfUSDC unit. |
| [R11] | `crates/ethereum-contracts/src/A651ToA666MigrationV1.sol:14-18`; `crates/ethereum-contracts/src/PFTLUniswapHandoffController.sol:903` | Frozen-snapshot migration precedent and lockable controller template. |
| [R12] | `deployments/a666-export-relay-mainnet-20260731/README.md:3,7-12`; `crates/execution/src/nav_vault_asset_execution.rs:6470-6477` | Existing A666 export relay and verifier deployment; export route requires a NAV asset. |
| [R13] | `crates/types/src/transactions_mempool_receipts.rs:1039-1068` | NAV mint identifies an issuer. |
| [R14] | `crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol:6-16`; `crates/ethereum-contracts/src/ERC20BridgeVaultV2.sol:22`; `crates/ethereum-contracts/script/DeployERC20Bridge.s.sol:38-67`; `crates/ethereum-contracts/test/ERC20BridgeVaultL1.t.sol` | EVM lock-event and finality-verifier patterns; Arbitrum-specific path; deployment and test precedent. |
| [R15] | `programs/pfusdc-eth-ingress/src/lib.rs:19,91`; `tools/nav-reserve-proof/README.md:31-35` | SP1 ingress is Ethereum-finality-specific and Sepolia-pinned; EVM checkpoint proof adapter. |
| [R16] | `crates/pft-token-contracts/src/PFTToken.sol`; `crates/pft-token-contracts/test/PFTToken.t.sol`; `crates/pft-token-contracts/script/DeployPFTToken.s.sol`; `crates/pft-token-contracts/foundry.toml`; `.gitmodules:7-9` | Token implementation, tests, deployment script, OpenZeppelin mapping and pinned submodule. |
| [R17] | `docs/listings/coinbase/pft-token-dry-run-20261026.md`; Base Sepolia deployment on 2026-10-28, chain ID `84532`: token `0x18fE4ca51A0cE042688D5A54917319d03B527D92`; treasury Safe `0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB`; two owners, threshold two; test supply 100 billion PFT; source verified on Sourcify and Blockscout. | Base Sepolia token status and addresses. |
| [R18] | [`docs/architecture/genesis-external-lanes.md`](../../architecture/genesis-external-lanes.md); commit `bfe44841` dated 2026-10-30 | Optional genesis lanes; faucet deduction; lane records; supply checks; exact-snapshot rule before release; no configured lane; `reserved_atoms` requirement. |
| [R19] | [`docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md`](../../specs/pft-erc20-base-bridge-phase2-design-20261026.md), sections 4.1, 5.1 and 10 | Phase 2 design; committee-trusted checkpoint and receipt proof; 5-of-6 halt rule; genesis release cap; 2%-per-day-window lean; dormant Phase 3 path. The note is not locked. |
| [R20] | `crates/pft-token-contracts/src/PFTLockV1.sol`; commit `39bfb547` dated 2026-10-28; `crates/pft-token-contracts/deployments/pft-lock-base-84532/deployment.json`; Base Sepolia address `0xE36e7Fc384564419187546c19719A5654266dE0B` | PFTLockV1 implementation, 2-billion-PFT per-lock cap and reverting-verifier test deployment. |
| [R21] | `crates/pft-token-contracts/script/DeployPFTLock.s.sol`; `crates/pft-token-contracts/README.md`, section “Deploy the lock contract”; commit `1e2cc55f` dated 2026-11-03 | Deployment-time anchor and binding checks; post-deployment re-read; no PFTL finality verifier on Base. |
| [R22] | `python/postfiat_rpc/pft_token.py`; `python/postfiat_rpc/pft_token_ui.py` | Python CLI and read-only token page. |
| [R23] | Product-owner decisions of 2026-10-28, also given in the exchange guidance: `docs/handoffs/2026-10-28___dravlic__base_sepolia_token_live_lock_contract_merged_and_exchange_page_corrected.md`; `docs/listings/coinbase/pft-erc20-one-pager.md` | Fixed 100-billion supply; Base slice; late-Q1-2027 Base target; five-year monthly development-organization escrow; Solana SPL form. |
| [R28] | The product owner's message of 2026-10-23 to this lane (not in the repository); the scoping it led to is `docs/listings/coinbase/integration-plan.md:736-737` | Coinbase ramp-up context and the NAVCoin question. |

### Outside references

These sources were verified on 2026-10-26.

| Label | Source | Claims supported |
| --- | --- | --- |
| [R24] | Coinbase listings page, <https://www.coinbase.com/listings> | Supported token networks and formats; relative integration effort; published token-review criteria; Base description. Page text was about two years old when checked. |
| [R25] | Coinbase Markets on X, <https://x.com/CoinbaseMarkets/status/2067313744741355774>; Coinbase Help Centre, “Token delistings and migrations,” <https://help.coinbase.com/en/coinbase/trading-and-funding/sending-or-receiving-cryptocurrency/token-migrations>; Crypto Briefing, 2026-06-18, <https://cryptobriefing.com/injective-migration-native-inj/> | Injective ERC-20 listing and later 1:1 native migration, June–July 2026. |
| [R26] | Forklog, 2025, <https://forklog.com/en/dydx-tokens-worth-26m-stranded-on-ethereum/> | dYdX bridge closure and more than 41.6 million stranded ethDYDX. |
| [R27] | Fetch.ai bridge documentation, <https://fetch.ai/docs/guides/fetch-network/how-to-convert-fet-to-and-from-erc20>; Post Fiat, <https://postfiat.org> | Fetch.ai two-way bridge; Post Fiat statements about Task Node payments, fixed supply and fee burning. |
