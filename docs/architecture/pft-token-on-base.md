# The PFT Token On Base

PFT is to list first as an ERC-20 on Base, with a one-for-one path into native PFT at PFTL's mainnet genesis ([milestone](../plans/active/pft-erc20-base-listing-milestone.md)). This page says what the token is, what exists today and how a Lock on Base becomes native PFT. Each fact links its source; the source is authoritative.

## The token

`PFTToken` is a fixed-supply ERC-20 for Base ([contracts README, "PFT token (Base)"](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/README.md#pft-token-base)):

- The constructor mints the whole supply once to the treasury. The intended treasury is a Safe multisig; a test-only switch (`PFT_TOKEN_ALLOW_EOA_TREASURY`) admits an EOA treasury, used only for the disposable rehearsal token. No function mints, burns or changes the supply later.
- `decimals()` is fixed at 6: one token unit is one atom of native PFT, so 1 PFT is 1,000,000 units.
- It has no owner, admin, pause, blacklist, hook or upgrade path. It is built only from OpenZeppelin `ERC20` and `ERC20Permit`, so approvals by signed message (EIP-2612) work.
- OpenZeppelin is the submodule `crates/pft-token-contracts/lib/openzeppelin-contracts` at tag v5.7.0, commit `cab19933c33c2ad1d4c7a84864a3601dddfd16f3`; `foundry.toml` pins solc 0.8.24, 200 optimizer runs, `cancun` and the IPFS metadata hash ([README, "Deployment"](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/README.md#deployment)).

The total supply is 100 billion PFT, fixed, decided 2026-10-28. The Base ERC-20 is a slice of it, not additional supply; the rest is native PFT at genesis, and the slice size is open ([milestone, Phase 2](../plans/active/pft-erc20-base-listing-milestone.md#phase-2-the-genesis-bridge); [options paper, "Decided"](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-representation-options-20261026.md#decided)).

The token is its own Foundry project, `crates/pft-token-contracts`, and the CI `evm-contracts` job runs `forge test` there ([README](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/README.md); [milestone P1.2](../plans/active/pft-erc20-base-listing-milestone.md#phase-1-the-token-on-base)):

| What | Where |
|---|---|
| Contract | [`src/PFTToken.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/src/PFTToken.sol) |
| Tests | [`test/PFTToken.t.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/test/PFTToken.t.sol), [`test/PFTTokenBoundary.t.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/test/PFTTokenBoundary.t.sol), [`test/PFTTokenInvariant.t.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/test/PFTTokenInvariant.t.sol) |
| Deploy script | [`script/DeployPFTToken.s.sol`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/script/DeployPFTToken.s.sol) with placeholders in `script/pft-token.env.example`; its tests are `test/DeployPFTToken.t.sol` and `test/deploy-script-cli.sh`, and its refusals (chain, supply, Safe treasury, artifact, one-time deployer) are listed in [README, "Deployment"](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/README.md#deployment) |
| Reader CLI and page | `python/postfiat_rpc/pft_token.py` and `python/postfiat_rpc/pft_token_ui.py` ([RPC examples](../rpc/examples.md#erc-20-pft-on-base); milestone M2, M3) |

## What exists on Base today

Base Sepolia (chain id 84532) holds test deployments with no value ([dry-run record](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-token-dry-run-20261026.md#base-sepolia-test-deployment-2026-10-28)):

| Contract | Address | Record | Source verified |
|---|---|---|---|
| Test token `PFTToken` (`Post Fiat Token`, `PFT`, 6 decimals) | `0x18fE4ca51A0cE042688D5A54917319d03B527D92` | [`deployments/pft-token-base-84532/deployment.json`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/deployments/pft-token-base-84532/deployment.json): mode `real`, block 51305977 | [Sourcify](https://repo.sourcify.dev/84532/0x18fE4ca51A0cE042688D5A54917319d03B527D92) (exact match), [Blockscout](https://base-sepolia.blockscout.com/address/0x18fE4ca51A0cE042688D5A54917319d03B527D92?tab=contract) |
| Test treasury, Safe 1.4.1 | `0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB` | two owners, threshold 2, no module, no guard (same record) | — |
| Rehearsal token `PFTToken` (`PFT Rehearsal 20261106`, `PFTR`, 6 decimals), disposable | `0x90cE303356D80eCBE359FAF061C84130F40EacF0` | [`deployments/pft-token-rehearsal-base-84532/deployment.json`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/deployments/pft-token-rehearsal-base-84532/deployment.json): mode `test`, block 55132270, treasury the EOA `0x6E4E8e0ed73f807D8e8479cb8f25D87Ce8811E17` | [Sourcify](https://repo.sourcify.dev/84532/0x90cE303356D80eCBE359FAF061C84130F40EacF0) (exact match), [Blockscout](https://base-sepolia.blockscout.com/address/0x90cE303356D80eCBE359FAF061C84130F40EacF0?tab=contract) |
| Rehearsal lock `PFTLockV1` | `0xFb6CF3D7E759cB3130C094C1D7264619ecE74D73` | [`deployments/pft-lock-base-84532/deployment.json`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/deployments/pft-lock-base-84532/deployment.json): mode `test`, block 55132483, on the rehearsal token | [Sourcify](https://repo.sourcify.dev/84532/0xFb6CF3D7E759cB3130C094C1D7264619ecE74D73) (exact match), [Blockscout](https://base-sepolia.blockscout.com/address/0xFb6CF3D7E759cB3130C094C1D7264619ecE74D73?tab=contract) |
| Archived lock `PFTLockV1` | `0xE36e7Fc384564419187546c19719A5654266dE0B` | [`deployments/pft-lock-base-84532/archived-0xE36e7Fc384564419187546c19719A5654266dE0B.json`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/deployments/pft-lock-base-84532/archived-0xE36e7Fc384564419187546c19719A5654266dE0B.json): mode `test`, block 51352801, never locked | [Sourcify](https://repo.sourcify.dev/84532/0xE36e7Fc384564419187546c19719A5654266dE0B) (exact match), [Blockscout](https://base-sepolia.blockscout.com/address/0xE36e7Fc384564419187546c19719A5654266dE0B?tab=contract) |

- The Safe holds the whole test supply, 100,000,000,000 PFT; that figure is a test value, not the slice ([dry-run record](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-token-dry-run-20261026.md#base-sepolia-test-deployment-2026-10-28)).
- The rehearsal lock's finality verifier is the same stand-in, whose `verifyAndConsume` always reverts, so nothing can be released from it; its `maxLockAmount` is 1,000,000 PFT and its `minimumLockAmount` 10 atoms. It holds one Lock: nonce 0, 1 PFT (1,000,000 units) to the PFTL account `pfb2f06f1346000c42db0246a0fcf6ce0356662eb6`, transaction `0x599e15fb…5c859` in block 55132949 ([qualification record, "Base Sepolia rehearsal (2026-11-06), part 1"](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-release-path-qualification-20261103.md#base-sepolia-rehearsal-2026-11-06-part-1-the-deployment-and-the-lock)).
- The archived lock never saw a Lock; it predates the minimum lock amount ([lock rehearsal](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-token-dry-run-20261026.md#base-sepolia-lock-rehearsal-2026-10-28)).
- Basescan has verified none of the sources; the 2026-11-06 rehearsal token and lock were verified from their source commit `1c5bf4c6` on 2026-11-06, an exact match of creation and runtime code on Sourcify and fully verified on Blockscout (each record's `verification_*` fields).
- Nothing is deployed on Base mainnet ([milestone P1.7 and "Not before"](../plans/active/pft-erc20-base-listing-milestone.md#not-before)).

## The genesis external lane

A genesis external lane reserves native PFT for the ERC-20 slice. Genesis gives
the faucet the native supply minus the lanes' `reserved_atoms`; a proven release
moves atoms from the reserve into the recipient's balance. With export accounting,
the lane remainder is `reserved_atoms - (released_atoms - exported_atoms)`;
`exported_atoms` is zero while exports are disabled. Native conservation counts
that remainder alongside circulating balances ([lane accounting](genesis-external-lanes.md#what-the-conservation-check-counts)).

The Base slice belongs to the decided 100-billion-PFT total. `reserved_atoms`
must equal the slice fixed by the token document; the production genesis build
must check that binding ([genesis fields](genesis-external-lanes.md#genesis-fields)).
The cross-chain backing claim also depends on genuine Locks behind releases and,
for the reverse route, settlement on Base ([export design](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md#4-accounting-on-pftl)).

The November 6 local six-validator rehearsal configured a Base Sepolia lane.
The recorded `postfiat-wan-devnet-2` genesis has no such lane, and no mainnet
lane is deployed. Rehearsal supply and reserve amounts are test fixtures, not the
approved production allocation ([milestone P2.2](../plans/active/pft-erc20-base-listing-milestone.md#phase-2-the-genesis-bridge)).

## From a Lock on Base to native PFT

**The lock contract.** A holder calls `PFTLockV1.lock(amount, pftlRecipient)`. The lock pulls the tokens with `transferFrom`, takes the next nonce from 0, writes a permanent record, emits one `Locked` event and keeps the tokens. It refuses an amount below `minimumLockAmount` (at least the PFTL account reserve, 10 atoms) or above `maxLockAmount`, and a recipient that is empty or over 256 bytes; it checks the recipient only for length. It has no owner, admin, pause, setter, sweep or upgrade path, and its `release` path stays dormant until Phase 3, paying only what its immutable finality verifier accepts ([README, "Lock contract (Phase 2, Base)"](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/README.md#lock-contract-phase-2-base); [design note, section 3](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md#3-the-lock-contract-on-base)).

**The committee checkpoint.** The bridge committee the lane's genesis policy names, a `FastSwapCommitteeV1` registered on PFTL, signs a checkpoint of one Base block: its number, hash and receipts root, the observed head, and the lane, chain, lock contract, token, code hashes and confirmation depth. A certificate needs the committee quorum, ⌊2n/3⌋ + 1, which is 5 of 6, and the observed head must be at least `confirmation_depth` blocks past the Lock's block ([lanes page, "The release path"](genesis-external-lanes.md#the-release-path); [design note, section 4.1](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md#41-the-bridge-committee-and-its-lifecycle)). The design signs only Base blocks derived from finalized Ethereum data, and the bridge is committee-trusted: a colluding quorum could release native PFT with no Lock behind it ([design note, section 4](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md#4-proving-a-base-lock-on-pftl) and [section 9](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md#9-risks)). Members observe Base, sign and assemble checkpoints with the node commands of the section "The Base checkpoint source" of the [lanes page](genesis-external-lanes.md).

**The receipt proof.** The proof carries, beside the certificate, the receipt-inclusion proof of the lock contract's `Locked` log under the checkpoint's receipts root. Execution decodes the log's nonce, amount and recipient and requires them to equal the claim; there is no SP1 program for Base ([lanes page, "The release path"](genesis-external-lanes.md#the-release-path); `verify_external_lane_lock` in [`crates/execution/src/external_lane_release.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/execution/src/external_lane_release.rs)).

**The release operation.** `external_lane_release_v1` names the lane, the Lock (chain, lock contract, nonce), the amount, the PFTL recipient and the proof. Any account may submit it and pay the fee. Execution checks it in a fixed order and refuses with no state change; an accepted release credits the recipient, marks the Lock nonce released so it is never paid twice, and appends a release record. A release to a new account below the account reserve is refused and can be retried later ([lanes page, "The release path"](genesis-external-lanes.md#the-release-path)).

**The window cap.** Each lane has `release_window_blocks` and `release_cap_bps`; one window may release `reserved_atoms × release_cap_bps / 10,000` atoms, rounded down, and that cap must be positive at genesis. The design's lean is 200 bps, 2 % of the lane, per window of one day's blocks, so a quorum that nobody stops needs about 50 windows to empty the lane ([lanes page, "Genesis fields"](genesis-external-lanes.md#genesis-fields); [design note, section 5.1](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md#51-the-per-window-release-cap)).

**The governed amendments.** Two amendment kinds change a configured lane, each signed by the registry threshold of the active validator set, 5 of 6. A cap amendment that lowers the cap applies at once, and a cap of zero halts releases and moves no PFT; a raise applies seven windows after it commits. A committee epoch move holds from the block after it commits; from then on checkpoints of the old epoch are refused, and a Lock is re-proven by the new committee from the same Base block ([lanes page, "Governed lane amendments"](genesis-external-lanes.md#governed-lane-amendments)). Both rounds were rehearsed on a six-validator fleet with the node CLI on 2026-11-04 ([qualification record](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-release-path-qualification-20261103.md#six-validator-rehearsal-of-the-amendment-rounds-2026-11-04)).

**The first real one.** On 2026-11-06 the Lock of 1 PFT on the Base Sepolia rehearsal lock (nonce 0, block 55132949) went this whole way. Five of six members of a local six-validator fleet certified its block, `external-lane-lock-proof` built the operation from the real receipts, and the fleet credited 1,000,000 atoms to `pfb2f06f1346000c42db0246a0fcf6ce0356662eb6` once and refused the same Lock again. The watcher found the Lock behind the release. The fleet was local and the lock's verifier a stand-in, with no mainnet involved ([qualification record, part 2](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-release-path-qualification-20261103.md#base-sepolia-rehearsal-2026-11-06-part-2-the-release-on-a-local-fleet)).

## Not built or not decided

P2.5 remains open on the lock audit after its engineering rehearsals. This does
not close the other launch gates listed here, or qualify an undeployed release.

- **The audit** of the token and lock: the [package for quotes](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-token-audit-package-20261106.md) is ready. Vendor, budget, the report and closure of findings remain open ([milestone P1.4 and P2.5](../plans/active/pft-erc20-base-listing-milestone.md#phase-1-the-token-on-base)).
- **The token document's open decisions:** the split at launch between the ERC-20 slice and genesis, which also sets `reserved_atoms`; the treatment of PFT already paid on the PFT Ledger; and the treasury signers and threshold ([milestone P1.5](../plans/active/pft-erc20-base-listing-milestone.md#phase-1-the-token-on-base); [options paper, "Open items"](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-representation-options-20261026.md#open-items)).
- **The Base mainnet deployment:** the production Safe, the token deployment and its source verification ([milestone P1.6–P1.8](../plans/active/pft-erc20-base-listing-milestone.md#phase-1-the-token-on-base)), and for the lock, a PFTL finality verifier on Base first ([README, "Deploy the lock contract"](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/pft-token-contracts/README.md#deploy-the-lock-contract)).
- **The reverse route, Phase 3:** the native-chain accounting, export packets and paged reads are built behind `exports_enabled`, default off. The Base verifier deployment, export prover and relay remain open, along with the decision to ship this optional route ([exports](genesis-external-lanes.md#exports); [milestone P3.1](../plans/active/pft-erc20-base-listing-milestone.md#phase-3-the-reverse-export-route-optional)).
- **The user-facing lane interface:** the Python CLI reads and watches a lane but does not build or submit a release; the node commands do ([lanes page, "Not built yet"](genesis-external-lanes.md#not-built-yet)).

## Read more

- [The ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md): every step, its owner and its state.
- [The options paper](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-representation-options-20261026.md): why an ERC-20 on Base first, the decisions and the open items.
- [The Phase 2 design note](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md): the lock contract, the proof, the lane, the cap and the risks.
- [The Coinbase one-pager](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/listings/coinbase/pft-erc20-one-pager.md): the token in a page for Coinbase.
- [Genesis External Lanes](genesis-external-lanes.md): what the lane and release code do, in detail.
