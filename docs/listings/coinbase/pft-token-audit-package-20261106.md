# PFT token and lock contracts: the audit package, 2026-11-06

What an auditor of the two Base contracts needs first, for items P1.4 and P2.5
of the [ERC-20 milestone](../../plans/active/pft-erc20-base-listing-milestone.md).
Every fact comes from the repository at the commit below or from a command run
on 2026-11-06 (section 7).

## 1. Scope

In scope: two Solidity files of `crates/pft-token-contracts`, at origin/main
`e5a508e7380af51a87ee132450d366d751d089b8` (2026-11-06).

| File | Lines | Git blob | SHA-256 | Last changed |
|---|---|---|---|---|
| `src/PFTToken.sol` | 41 | `d1be9291f893dbb927fd320fa113d28b70d99cf6` | `34019b60abd23c90b9e9d53b95007701a898bd041e7535b4cbfbea3f2da0f706` | `5d6e6709c`, 2026-10-27 |
| `src/PFTLockV1.sol` | 288 | `9ad9d6359f3b5e243e4a362f3e58477b23b12024` | `3a0f2fe71ea1ad17946cffe5611da51a86a7ab18a967ca27cccd7d67aa3dc856` | `7a42f5fc5`, 2026-11-04 |

- **Compiler** (`foundry.toml`): solc `0.8.24+commit.e11b9ed9`, optimizer on,
  200 runs, `evm_version = "cancun"`, `bytecode_hash = "ipfs"`,
  `cbor_metadata = true`; both files pin `pragma solidity 0.8.24`. The one
  remapping is `@openzeppelin/contracts/=lib/openzeppelin-contracts/contracts/`.
- **OpenZeppelin**: the submodule `lib/openzeppelin-contracts` at tag `v5.7.0`,
  commit `cab19933c33c2ad1d4c7a84864a3601dddfd16f3`, its nested submodules not
  initialised. `PFTToken` imports `ERC20` and `ERC20Permit`; `PFTLockV1`
  imports `IERC20`, `IERC20Permit`, `SafeERC20` and `ReentrancyGuard`.
- **Build reproduction**: today's `forge build` gives the creation code
  keccak256 `0xef4e1b79…548c6` (`PFTToken`) and `0x89de6356…c1440`
  (`PFTLockV1`), the records' `creation_code_keccak256`. `git diff` shows
  `src/`, `foundry.toml` and the submodule unchanged since the rehearsal
  records' `1c5bf4c6`, and `src/PFTToken.sol` since the test token's `8bf0fafa`.

Base Sepolia deployments (records under `crates/pft-token-contracts/deployments/`),
runtime code hashes read today with `cast keccak $(cast code …)`:

| Contract | Address | Record, mode | Runtime keccak256 (on chain today) |
|---|---|---|---|
| Test token | `0x18fE4ca51A0cE042688D5A54917319d03B527D92` | `pft-token-base-84532`, real | `0xb0939a2407cd571d158498ba0f405f4ddb3857e688d2bc5ada170dcc655903df` |
| Rehearsal token | `0x90cE303356D80eCBE359FAF061C84130F40EacF0` | `pft-token-rehearsal-base-84532`, test | `0x486d2a13ce1250e62b52acd787d2d2ff04aaf2d623f5159cb62ee55b7f1cb76b` |
| Rehearsal lock | `0xFb6CF3D7E759cB3130C094C1D7264619ecE74D73` | `pft-lock-base-84532`, test | `0x57bc7971e6878bed2cd4efc0e63020448d4cd9493f05ecde6ddc37fda90b871c` |

The test-mode values equal the records' `deployed_code_keccak256`, the test
token's its [dry-run record](pft-token-dry-run-20261026.md). The rehearsal
lock's verifier `0x531659D05d86C5C4cE6fdc3C473b246C294f2Ec9` is the tests'
`MockLockVerifier`, whose `verifyAndConsume` always reverts. No record exists
for Base mainnet (8453).

Out of scope: the PFTL node (release path, checkpoint source and watcher of
[genesis external lanes](../../architecture/genesis-external-lanes.md)); the
PFTL finality verifier on Base and the Phase 3 prover and relay, not built
([Phase 3 design](../../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md),
section 5, "What Base still needs"); `crates/ethereum-contracts`, whose
`PFTLFinalityVerifierV1` one deploy test imports. `script/` and `test/` are
supporting material.

## 2. What the contracts do

**`PFTToken`** is a fixed-supply ERC-20 with EIP-2612 permit. The constructor
`PFTToken(name, symbol, totalSupply, treasury)` mints the whole supply once to
the treasury (a Safe) and refuses a zero treasury or supply. `decimals()` is
6: one unit is one atom of native PFT. No function mints or burns later, and it
has no owner, admin, pause, blacklist, hook or upgrade path. Its ABI is a
13-function allowlist (`test/PFTTokenBoundary.t.sol`).

**`PFTLockV1`** locks ERC-20 PFT on Base for native PFT on PFTL
([Phase 2 design](../../specs/pft-erc20-base-bridge-phase2-design-20261026.md), section 3).

- `lock(amount, pftlRecipient)` and `lockWithPermit(…)` pull the tokens with
  `transferFrom`, take the next nonce (from 0, counted by the contract), write
  a permanent `lockRecord(nonce)` (holder, amount, recipient hash), add to
  `totalLocked` and emit `Locked(nonce, holder, pftlRecipientHash,
  pftlRecipient, amount)`, the recipient bytes and their keccak256.
- A Lock is refused below `minimumLockAmount`, above `maxLockAmount` (the
  genesis per-window release cap, `uint64`), with an empty or over-256-byte
  recipient, when the token's runtime code hash differs from the one fixed at
  construction, or when the balance delta differs from the amount. The
  recipient is checked for length only.
- `release(publicValues, proofBytes)`, the dormant Phase 3 path (section 6),
  pays only what the immutable `finalityVerifier`'s `verifyAndConsume` accepts,
  refuses a zero amount or recipient, a replayed withdrawal or burn commitment
  and an amount above `totalLocked - totalReleased`, checks both balance
  deltas and emits `Released`.
- The constructor fixes the token, its code hash, the verifier, the cap and
  the minimum, and refuses a zero address, an address without code, a wrong
  code hash, a zero cap, a minimum below `PFTL_ACCOUNT_RESERVE` (10) or above
  the cap. No owner, admin, pause, setter, sweep or upgrade path; the ABI is
  twelve functions (`test_contract_has_no_owner_pause_or_withdraw`).

Why the release ships dormant (section 6): the lock has no admin, so a release
path added later could never reach tokens locked before it. Deployment order
(Phase 3 design, section 5): the lock binds the verifier's address, the
verifier binds the lock's runtime code hash, the verifier needs a finalized
PFTL checkpoint, and genesis names the lock's address and code hash; this
supersedes the Phase 2 sentence that the lock is deployed before genesis.

## 3. Invariants and threat model

The lock's invariants (design section 3; `crates/pft-token-contracts/README.md`,
"Lock contract"), encoded in `test/PFTLockV1Invariant.t.sol`:

1. `totalLocked - totalReleased` is backed by the balance, which exceeds it
   only by plain transfers: `invariant_OutstandingIsBackedByBalance`.
2. Nonces 0 to `nextNonce - 1`, one record and one event each, every record at
   or above the minimum: `invariant_NextNonceEqualsLockedEvents`.
3. Only `release` lowers the balance: no function of its own; invariant 1's
   balance equality is asserted after every handler call.
4. Within the fixed supply: `invariant_TotalLockedWithinSupply` (no releases)
   and `invariant_OutstandingWithinSupply` (with releases).

The token's (`test/PFTTokenInvariant.t.sol`): `invariant_SumOfBalancesEqualsTotalSupply`,
`invariant_TotalSupplyNeverChanges`, `invariant_TreasuryBalancePlusNetOutflowEqualsSupply`.

The threat model the designs state:

| Case | Can | Cannot | Source |
|---|---|---|---|
| Colluding committee quorum (5 of 6) | sign a false Base block and release native PFT on PFTL with no Lock behind it; reach the whole lane in about 50 windows unchecked; lift the cap seven windows after an amendment commits | release more than the cap (lean 2 % of the lane) per window; raise the cap sooner than seven windows | Phase 2, sections 5.1, 5.2, 9 |
| A few stolen or lost checkpoint keys, a faulty observer, an outage | halt releases below quorum | release anything | Phase 2, section 9 |
| A lying Base RPC | have a member sign what its node reports as `finalized`; the commands do not verify Base finality against Ethereum, and five members on one shared RPC reduce the quorum to that RPC, which the model does not cover | get the vote of a member whose own RPC reports another block (`base_observation_mismatch`) | genesis external lanes, "The Base checkpoint source" |
| A false Phase 2 release | raise `released_atoms` without raising `totalLocked`; once exports exist, leave the last exporters unpaid (`ReleaseAboveLocked`) | change native supply conservation, which holds either way | Phase 2, section 5; Phase 3, sections 4, 8 |
| An unsound verifier payout | pay from the lock without an export, up to its balance; nothing can pause it | pay more than `totalLocked - totalReleased`, or a commitment twice | Phase 3, sections 4, 8; `PFTLockV1.sol:226-233` |
| The holder | lose a Lock to a mistyped recipient, or tokens sent by plain transfer | recover either: no admin | Phase 2, section 9 |

## 4. Evidence, regenerated 2026-11-06

At `e5a508e7`, with forge 1.8.1 (`982849d3`) and Slither 0.11.6.
**`forge test`**: 199 passed, 0 failed, 0 skipped, in 9 suites:
`DeployPFTLockTest` 60, `DeployPFTTokenTest` 56, `PFTLockV1Test` 34,
`PFTTokenBoundaryTest` 24, `PFTLockV1AccountingTest` 14, `PFTTokenTest` 8, and
the three invariant contracts, one entry each. `[fuzz] runs = 1000`;
`[invariant] runs = 64`, `depth = 32`, `fail_on_revert = true`; each
invariant contract reports runs 64, calls 2048, reverts 0.

**`forge coverage --report summary`** (forge turns the optimizer off; 199 passed):

| File | % Lines | % Statements | % Branches | % Funcs |
|---|---|---|---|---|
| `src/PFTLockV1.sol` | 100.00 (75/75) | 100.00 (101/101) | 100.00 (25/25) | 100.00 (8/8) |
| `src/PFTToken.sol` | 100.00 (8/8) | 100.00 (6/6) | 100.00 (2/2) | 100.00 (2/2) |
| `script/DeployPFTLock.s.sol` | 100.00 (346/346) | 99.79 (467/468) | 98.75 (79/80) | 100.00 (37/37) |
| `script/DeployPFTToken.s.sol` | 100.00 (395/395) | 99.68 (625/627) | 98.11 (104/106) | 100.00 (42/42) |
| Total, test helpers included | 78.45 (1063/1355) | 79.33 (1458/1838) | 73.16 (229/313) | 84.24 (139/165) |

**`slither .`** in `crates/pft-token-contracts`: 30 contracts (the two files
and their OpenZeppelin imports), 102 detectors, 76 results: High 1, Medium 9,
Low 4, Informational 62. Two are in `src/PFTLockV1.sol`; the rest are in
OpenZeppelin v5.7.0 or, for `pragma`, span both.

| Severity | Detector | Count | Where | Note |
|---|---|---|---|---|
| High | `incorrect-exp` | 1 | OZ `Math.mulDiv`, `Math.sol:259` | for the auditor |
| Medium | `divide-before-multiply` | 9 | OZ `Math.mulDiv`, `Math.invMod` | for the auditor |
| Low | `reentrancy-benign` | 1 | `PFTLockV1.release`, `:213-251`, state written after `verifyAndConsume` | the call goes to the verifier, "immutable, with no setter and no admin" (Phase 2, section 6); `release` is `nonReentrant` |
| Low | `reentrancy-benign` | 1 | `PFTLockV1.lockWithPermit`, `:194-209`, state written after `permit` | the call goes to the token, whose address and runtime code hash "are fixed at construction" (Phase 2, section 3); `nonReentrant` |
| Low | `shadowing-local` | 1 | OZ `ERC20Permit` constructor | for the auditor |
| Low | `timestamp` | 1 | OZ `ERC20Permit.permit` deadline | for the auditor |
| Informational | `assembly` | 46 | OZ | for the auditor |
| Informational | `too-many-digits` | 7 | OZ | for the auditor |
| Informational | `naming-convention` | 4 | OZ | for the auditor |
| Informational | `solc-version` | 4 | OZ version ranges | for the auditor |
| Informational | `pragma` | 1 | six versions across `src/` and OZ | for the auditor |

**Explorer verification**, read today from the Sourcify and Blockscout APIs:

| Contract | Sourcify | Blockscout | Basescan |
|---|---|---|---|
| Test token `0x18fE…7D92` | `exact_match` (creation and runtime), verified 2026-10-28 | verified, `PFTToken`, v0.8.24+commit.e11b9ed9, 200 runs, cancun | record: "not verified (no key)"; not read today |
| Rehearsal lock `0xFb6C…4D73` | not found (HTTP 404) | `is_verified: false` | no record field; not read today |
| Rehearsal token `0x90cE…EacF0` | not found (HTTP 404) | verified, `PFTToken`, by Blockscout's bytecode database (`is_verified_via_eth_bytecode_db`) at 10:27 UTC today, on the first read | no record field; not read today |

## 5. Deploy procedure and order

Both scripts refuse before the broadcast; the full tables are in the README's
"Deployment" (token) and "Deploy the lock contract" (lock).

- **Token** (`script/DeployPFTToken.s.sol`): the chain (8453 or 84532, equal to
  the RPC's); placeholder or over-31-byte names; the supply pair; a treasury
  that is the deployer, has no code, or is not a pinned Safe 1.4.1 or 1.3.0
  proxy and singleton with threshold at least 2, no module and no unnamed
  guard, created by the canonical factory with a plain, paymentless `setup`;
  an artifact whose creation code, IPFS metadata or runtime length differs;
  a used deployer; a real run with a test switch, no source commit or an
  existing record.
- **Lock** (`script/DeployPFTLock.s.sol`): placeholders; the chain; on 84532 a
  token other than the recorded test token; the token's code hash, decimals
  and supply; the verifier's code hash, its finalized checkpoint and its
  chain, genesis, route, asset, token and vault bindings against
  operator-pinned values (`VaultRuntimeCodeHashMismatch` for the lock's code
  hash); the cap's `uint64` fit and pair; a minimum other than
  `PFTL_ACCOUNT_RESERVE_UNITS` (10); a used deployer; the artifact binding; a
  real run with a test switch, no 40-hex source commit or an existing record.
  Test switches are refused on 8453 (`TestSwitchOnMainnet`).

The order (Phase 3 design, section 5): two fresh deployers, one for the
verifier and one for the lock; predict both CREATE addresses; compute the
lock's runtime code hash from the predicted verifier address and the other
four constructor values; write the predicted lock address and hash into
genesis; start the chain; take the first finalized PFTL checkpoint; deploy the
verifier, then the lock, each at its predicted nonce; check the bindings and
the genesis pins on chain.

The one-time deployer rule: each script refuses a deployer whose nonce is not
0 (`DeployerAlreadyUsed`); the token deployer is retired after its one
transaction; for the ordered pair, "Nothing else may be sent from either
deployer before its deployment" (Phase 3 design, section 5). A Base mainnet
deployment waits for an independent audit of the exact commit (README,
"Deployment"; Phase 2 design, section 8).

## 6. Questions for the auditor

1. **The recipient's acceptance set.** The lock accepts any 1-to-256-byte
   recipient and records its keccak256; PFTL's release takes a UTF-8
   recipient and refuses an invalid one. Which byte strings does the lock
   accept that PFTL can never credit, and is a Lock to one of them lost?
2. **The minimum against the account reserve.** The contract requires a
   `minimumLockAmount` of at least 10, the PFTL `ACCOUNT_RESERVE`, the deploy
   script exactly 10; `crates/execution/src/lock_minimum_binding_tests.rs`
   compares the contract, the script and the env example with it. Is that
   binding enough to keep every Lock at the minimum releasable to a new account?
3. **The stand-in verifier.** The rehearsal lock binds a verifier that always
   reverts, and `release` has no switch of its own: it stays dormant only
   while the bound verifier accepts no proof. What must hold of the real
   verifier, deployed after genesis, for the path to stay closed until
   Phase 3 and sound after?
4. **The pre-genesis order.** Genesis pins a predicted lock address and code
   hash before the verifier or the lock exists. Are the prediction, the
   deployer-nonce rule and the post-deployment checks sufficient, and what
   does a mismatch leave behind?

## 7. How to run everything

```bash
git fetch origin && git checkout e5a508e7380af51a87ee132450d366d751d089b8
git submodule update --init --depth 1 crates/pft-token-contracts/lib/openzeppelin-contracts
cd crates/pft-token-contracts
git -C lib/openzeppelin-contracts rev-parse HEAD   # cab19933c33c2ad1d4c7a84864a3601dddfd16f3
sha256sum src/PFTToken.sol src/PFTLockV1.sol
forge build
for c in PFTToken PFTLockV1; do cast keccak $(jq -r .bytecode.object out/$c.sol/$c.json); done
forge test
forge coverage --report summary
slither . --json "$TMPDIR/slither.json"            # Slither 0.11.6; severities in the JSON
for a in 0x18fE4ca51A0cE042688D5A54917319d03B527D92 \
         0x90cE303356D80eCBE359FAF061C84130F40EacF0 \
         0xFb6CF3D7E759cB3130C094C1D7264619ecE74D73; do
  cast keccak $(cast code "$a" --rpc-url https://sepolia.base.org)
  curl -s "https://sourcify.dev/server/v2/contract/84532/$a"
  curl -s "https://base-sepolia.blockscout.com/api/v2/smart-contracts/$a"
done
test/deploy-script-cli.sh && test/deploy-lock-cli.sh   # the scripts through forge script and anvil (CI)
```
