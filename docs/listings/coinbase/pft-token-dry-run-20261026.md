# PFT token deploy-script dry run, 2026-10-26

This records the local dry runs of `DeployPFTToken` for milestone item P1.3:
the first on 2026-10-26 against commit `ad5ba32f`, and two on 2026-10-27 after
the deploy-script guards (commit `5d6e6709`). All ran on a local anvil only.
The Base Sepolia test deployment of 2026-10-28 is recorded at the end.

## Command

The current script also needs `PFT_TOKEN_CHAIN_ID` and the supply in whole
PFT, and refuses a treasury without code unless the test-only switch is set
(84532 only; see `crates/pft-token-contracts/README.md`). The dry run, with
no key and no broadcast, is:

```bash
anvil --chain-id 84532 &
PFT_TOKEN_CHAIN_ID=84532 PFT_TOKEN_NAME="Post Fiat Token" PFT_TOKEN_SYMBOL=PFT \
PFT_TOKEN_TOTAL_SUPPLY=1000000000000 PFT_TOKEN_TOTAL_SUPPLY_PFT=1000000 \
PFT_TOKEN_TREASURY=0x0000000000000000000000000000000000007EA5 \
PFT_TOKEN_ALLOW_EOA_TREASURY=1 \
  forge script script/DeployPFTToken.s.sol:DeployPFTToken --rpc-url http://127.0.0.1:8545
```

`PFT_TOKEN_ALLOW_USED_DEPLOYER=1`, the other test-only switch, is needed only
for an account that has already sent a transaction. On chain id 8453 the script
refuses both switches.

The 2026-10-26 run predates these variables. It used forge 1.8.1 in
`crates/ethereum-contracts`, where the token then lived, with only
`PFT_TOKEN_NAME`, `PFT_TOKEN_SYMBOL`, `PFT_TOKEN_TOTAL_SUPPLY` and
`PFT_TOKEN_TREASURY`, against `anvil --port 18547 --chain-id 84532` with
`--offline`.

Chain id 84532 is Base Sepolia's, so the simulation runs under the test
network's id. The supply and treasury are test values; the real supply is not
decided and the treasury Safe does not exist yet.

## Anvil runs, 2026-10-27

These runs checked the one-time-deployer guard against a real node rather than
forge's in-process EVM. Setup: `anvil --chain-id 84532` with no network. The
deployer was anvil development account 0
(`0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266`), whose public test key was given
with `--private-key`. The treasury was account 1
(`0x70997970C51812dc3A010C7d01b50e0d17dc79C8`), allowed by
`PFT_TOKEN_ALLOW_EOA_TREASURY=1`. `PFT_TOKEN_ALLOW_USED_DEPLOYER` was not set.
Both runs used `--broadcast` against the anvil.

| Run | Deployer nonce read | Outcome |
| --- | --- | --- |
| 1 | 0 | `PFTToken deployed at 0x5FbDB2315678afecb367f032d93F642f64180aa3`, `ONCHAIN EXECUTION COMPLETE & SUCCESSFUL.`, 1,232,032 gas estimated |
| 2, same key | 1 | `Error: script failed: DeployerAlreadyUsed(0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266, 1)` |

After run 1, `totalSupply()` was 1000000000000, the treasury held all of it,
the deployer held 0 and `decimals()` was 6. On a forked node the script reads
the account's real nonce, so a fresh key passes. Only forge's in-process EVM,
without `--rpc-url`, starts a sender at nonce 1.

The first attempt wrote its deployment record to
`deployments/pft-token-base-84532/`, the path reserved for the real Base
Sepolia record. The script now writes the record of a broadcast made with a
test-only switch to the ignored `cache/deploy-test/` instead. The runs above
used that version.

## Anvil runs with a code-bearing treasury, 2026-10-27

The guards above probed a Safe treasury through a self-call, which forge
1.8.1 refuses inside a script ("Usage of address(this) detected in script
contract"), so a contract treasury blocked the script; the EOA runs never
reached that branch. The script now reads the proxy's singleton from storage
slot 0 and probes with direct staticcalls. Setup: `anvil --chain-id 84532`,
no network, unlocked development accounts (no key given). `FakeSafe` (2 of 3
owners, from `test/DeployPFTToken.t.sol`) was deployed with `forge create`
and its slot 0 set to SafeL2 v1.4.1
(`0x29fcB43b46531BcA003ddC8FCB67FFE91900C762`) with `anvil_setStorageAt`. No
test switch was set; the deployer was the fresh account 2.

| Run | Treasury slot 0 | Outcome |
| --- | --- | --- |
| 3, `--broadcast` | SafeL2 v1.4.1 | `PFTToken deployed at 0x663F3ad617193148711d28f5334eE4Ed07016602`, `ONCHAIN EXECUTION COMPLETE & SUCCESSFUL.`; record under `cache/deploy-test/` with mode `test`, threshold 2, three owners, `tx_hash` `pending` |
| 4, dry run | `0x…0bad` | `Error: script failed: UnofficialSafeSingleton(0x8464135c8F25Da09e49BC8782676a84730C318bC, 0x0000000000000000000000000000000000000Bad)` |

The script has since been tightened to authenticate the proxy code,
`masterCopy()` and the singleton's code, so the `FakeSafe` of run 3 is now
refused (`ProxyCodeMismatch`). `crates/pft-token-contracts/test/deploy-script-cli.sh`
repeats the cases through `forge script` on forge's in-process chain with a
real SafeL2 v1.4.1 proxy and that lookalike.

## Results, 2026-10-26

The run printed `Script ran successfully`, then `PFTToken deployed at
0x5b73C5498c1E3b4dbA84de0F1833c4a029d90519` and the same address as the
return value. It estimated 1,231,751 gas on chain 84532 and ended with
`SIMULATION COMPLETE`. The address is a local simulation result and holds
nothing anywhere.

With the values of `script/pft-token.env.example` loaded instead, the script
stopped before deploying with `Error: script failed:
PlaceholderValue("PFT_TOKEN_NAME")`.

## Tests and versions

| Check | forge 1.8.1 (local) | forge 1.7.1 (CI's version) |
| --- | --- | --- |
| `forge test --match-contract PFTToken` | 8/8 passed | 8/8 passed |
| `forge test --no-match-path 'test/*Fork.t.sol'` | 186/186 passed, 23 suites | 186/186 passed, 23 suites |

Against an empty stub with only the constructor, the eight PFTToken tests
failed 0/8 before the contract was written. The 1.7.1 run used the official
release binary on a fresh clone, after CI's own submodule step.

OpenZeppelin is pinned as the submodule `lib/openzeppelin-contracts` at tag
`v5.7.0`, commit `cab19933c33c2ad1d4c7a84864a3601dddfd16f3`.

## Base Sepolia test deployment, 2026-10-28

This is a test deployment on the Base Sepolia test network (chain id 84532),
paid with test ETH. The token has no value.

- Token: `0x18fE4ca51A0cE042688D5A54917319d03B527D92`, name `Post Fiat Token`,
  symbol `PFT`, 6 decimals.
- Treasury: Safe 1.4.1 `0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB`
  (singleton SafeL2 v1.4.1 `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762`),
  created in transaction
  `0x41b12d27346f0ff305c1da85205d800ab8f066964d78500bbb943fdc5bdc31d2` through
  the canonical SafeProxyFactory `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67`.
  Owners `0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB` and
  `0x36b554946e43620Cc52CAe1b1A374fD808469A55`, threshold 2, no module, no guard.
- Deployer: the one-time key `0xDEe78dB2bC6f5EFCcAB7A046997E5c413C26316D`,
  nonce 0 before the deployment. Its leftover test ETH went back to
  `0xC75B…a4CB` in
  `0xb65b0194ec42242d082fc5213811b1f92288cc9183c03001b25172cd09aadc7e`; the
  key is retired.
- Transaction:
  `0xbf8e1a1900e03b1bcf06d7be93809d90a22b89adc922f2076efa2fe028336102`,
  block 51305977, status 1, 947,741 gas.
- Supply: 100,000,000,000 PFT (`100000000000000000` six-decimal units), all
  held by the Safe. This is a test figure; the real slice is fixed by the
  token document (P1.5).
- Code: runtime keccak256
  `0xb0939a2407cd571d158498ba0f405f4ddb3857e688d2bc5ada170dcc655903df`;
  creation code keccak256
  `0xef4e1b797c1dcc3a66a9d7ba5a933edc7fe38e9fcab0d294347e855df56548c6`, equal
  to the transaction input without the constructor arguments.
- Source verified:
  [Sourcify](https://repo.sourcify.dev/84532/0x18fE4ca51A0cE042688D5A54917319d03B527D92)
  (exact match) and
  [Blockscout](https://base-sepolia.blockscout.com/address/0x18fE4ca51A0cE042688D5A54917319d03B527D92?tab=contract)
  (verified). Basescan: not verified (no key).
- Record: `crates/pft-token-contracts/deployments/pft-token-base-84532/deployment.json`
  (mode `real`, source commit `8bf0fafa`, solc 0.8.24, optimizer 200 runs,
  `cancun`).

Run from `crates/pft-token-contracts` after `forge clean && forge build` and
`forge test` (71 passed), first without and then with `--broadcast`, and no
test-only switch:

```bash
PFT_TOKEN_CHAIN_ID=84532 PFT_TOKEN_NAME="Post Fiat Token" PFT_TOKEN_SYMBOL=PFT \
PFT_TOKEN_TOTAL_SUPPLY=100000000000000000 PFT_TOKEN_TOTAL_SUPPLY_PFT=100000000000 \
PFT_TOKEN_TREASURY=0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB \
PFT_TOKEN_REAL_DEPLOYMENT=1 PFT_TOKEN_SOURCE_COMMIT=$(git rev-parse HEAD) \
  forge script script/DeployPFTToken.s.sol:DeployPFTToken \
  --rpc-url https://sepolia.base.org --sender 0xDEe78dB2bC6f5EFCcAB7A046997E5c413C26316D \
  --keystore <deployer keystore> --password-file <password file> --broadcast
```

The script printed the Safe singleton, threshold 2, two owners, zero modules,
a zero guard and deployment record `real`. After the broadcast,
`totalSupply()` and the Safe's `balanceOf` were both `100000000000000000`, and
the deployer's nonce was 1.

## Base Sepolia lock rehearsal, 2026-10-28

This is a test deployment with no value and a reverting stand-in verifier: the
lock contract on the Base Sepolia test network (chain id 84532), paid with test
ETH, holding the test token above.

- Lock: `PFTLockV1` at `0xE36e7Fc384564419187546c19719A5654266dE0B`; `token()`
  is the test token, `nextNonce()` and `totalLocked()` are 0.
- Verifier stand-in: `0x531659D05d86C5C4cE6fdc3C473b246C294f2Ec9`, the tests'
  `MockLockVerifier` (`crates/pft-token-contracts/test/DeployPFTLock.t.sol`),
  deployed by the one-time key `0x2317C98D52135a43c06211b9Fa381CFe7b0eFa25` in
  `0xbd16c9022ed4fccb0d2f5d387953032d7e4e4b51eabab265bb6ec29b8cbcb33b`. Its
  `verifyAndConsume` reverts `NotDeployed()` on every input, so nothing can
  ever be released from this lock. Its source is not verified.
- Cap: `maxLockAmount()` 2,000,000,000 PFT (`2000000000000000` units), 2 % of
  the 100 billion test supply.
- Deployer: the one-time key `0x6E4E8e0ed73f807D8e8479cb8f25D87Ce8811E17`,
  nonce 0 before and 1 after the deployment.
- Transaction:
  `0xf73be7afe357a6f23dc1052d8292eef50db9635c0f3e993cfdcb8745ee934400`,
  block 51352801, status 1, 1,088,346 gas.
- Code: deployed keccak256
  `0x328a93e99cf9659bdb44807d60a9e0328e0ff867d0b584dc2eeed3b0d5442fb2` (equal to
  the record's `deployed_code_keccak256`); creation code keccak256
  `0x2af875b177e44457588eadb6660ce021474a88dd54d0e4041c0e212277a0a15a`; verifier
  stand-in keccak256
  `0xa26bc54573c6f471997352bf83746ebb0b37c702de858255ac9595e3d263fcde`.
- Source verified:
  [Sourcify](https://repo.sourcify.dev/84532/0xE36e7Fc384564419187546c19719A5654266dE0B)
  (exact match) and
  [Blockscout](https://base-sepolia.blockscout.com/address/0xE36e7Fc384564419187546c19719A5654266dE0B?tab=contract)
  (verified). Basescan: not verified (no key).
- Record: `crates/pft-token-contracts/deployments/pft-lock-base-84532/deployment.json`,
  copied by hand from the script's test-mode record (`cache/deploy-test/`) with
  the transaction and block filled in, `"mode": "test"` and
  `"stand_in_verifier": true`. Built from `b4f98c72`.

The verifier's code hash is not pinned, so this is the script's test mode
(`PFT_LOCK_ALLOW_TEST_VERIFIER=1`, which `PFT_LOCK_REAL_DEPLOYMENT=1` refuses).
Run from `crates/pft-token-contracts` after `forge clean && forge build`, first
without and then with `--broadcast`:

```bash
PFT_LOCK_CHAIN_ID=84532 PFT_LOCK_TOKEN=0x18fE4ca51A0cE042688D5A54917319d03B527D92 \
PFT_LOCK_TOKEN_CODEHASH=0xb0939a2407cd571d158498ba0f405f4ddb3857e688d2bc5ada170dcc655903df \
PFT_LOCK_VERIFIER=0x531659D05d86C5C4cE6fdc3C473b246C294f2Ec9 PFT_LOCK_ALLOW_TEST_VERIFIER=1 \
PFT_LOCK_MAX_LOCK_AMOUNT=2000000000000000 PFT_LOCK_MAX_LOCK_AMOUNT_PFT=2000000000 \
  forge script script/DeployPFTLock.s.sol:DeployPFTLock \
  --rpc-url https://sepolia.base.org --sender 0x6E4E8e0ed73f807D8e8479cb8f25D87Ce8811E17 \
  --keystore <deployer keystore> --password-file <password file> --broadcast
```

The end-to-end lock was not exercised: the only holder of the test token is the
2-of-2 treasury Safe, which needs both signatures to move any. Both one-time
keys were funded with 0.0005 test ETH from `0xC75B…a4CB` and are retired, their
leftovers returned in
`0xa41c94dc4d80895b417bf01557839c6bda739497f21bc48301d3963b4cb4e8dc` and
`0x9999c5d6f83d3f35aa5ac3f81dcd70c9b1667fe1460e543f0be8c47bd51f2987`.
