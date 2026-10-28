# PFT token contracts

Stand-alone Foundry project for the PFT token. It is kept apart from
`crates/ethereum-contracts` so that project's library directory and compiler
remappings, and with them the pinned A666 public reader build
(`scripts/check-a666-public-reader-candidates`), stay unchanged. Same solc
0.8.24 and optimizer settings (`foundry.toml`); CI's `evm-contracts` job runs
`forge test` here after fetching the OpenZeppelin submodule.

## PFT token (Base)

`src/PFTToken.sol` is the fixed-supply ERC-20 representation of PFT, Phase 1 of
the ERC-20 representation options paper. It is meant for Base, with test
deployments on Base Sepolia.

- Fixed supply: the whole supply is minted once, in the constructor, to the
  treasury. No function mints, burns or changes it later.
- Six decimals: one token unit is one atom of native PFT (1 PFT = 1,000,000 units).
- No privileged role: no owner, admin, pause, blacklist, hook or upgrade path.
- Permit: EIP-2612 approvals by signed message.
- Built only from OpenZeppelin `ERC20` and `ERC20Permit`.

Constructor: `PFTToken(string name, string symbol, uint256 totalSupply, address treasury)`.
`totalSupply` is in six-decimal units; `treasury` is the Safe multisig that
receives the whole supply. A zero treasury (`ZeroTreasury()`) or a zero supply
(`ZeroSupply()`) reverts. The contract fixes no supply number. `name` is at
most 31 bytes: OpenZeppelin v5.7.0's EIP712 stores it as a `ShortString`
(`EIP712.sol:79`) and a longer name reverts with `StringTooLong`
(`ShortStrings.sol:54-56`); the deploy script refuses it first with
`NameTooLong(length)`.

Interface notes:

- `approve` or `permit` to the zero address reverts with `ERC20InvalidSpender`
  (OpenZeppelin v5).
- Besides ERC-20 and EIP-2612, the ABI exposes `eip712Domain()` (ERC-5267).
- The approve race is the standard ERC-20 one, and `permit` has it too (a
  permit also replaces the allowance). Set the allowance to zero, confirm it,
  then set the new one, accounting for anything spent in between.

Tests:

```bash
git submodule update --init --depth 1 crates/pft-token-contracts/lib/openzeppelin-contracts
cd crates/pft-token-contracts
forge test
```

`test/PFTTokenBoundary.t.sol` holds the ERC-20 and permit boundary cases, the
exact 13-function allowlist (selectors pushed with PUSH1..PUSH4 and compared
in the deployed dispatcher, cross-checked against the artifact's
`methodIdentifiers` except under `forge coverage`) and the fuzz tests (1000
runs); `test/PFTTokenInvariant.t.sol` checks that the balances sum to the fixed
supply over random transfers, approvals and permits (64 runs, depth 32,
`fail_on_revert`). `test/DeployPFTToken.t.sol` covers every refusal of the
deploy script and its accepted path; `test/deploy-script-cli.sh` runs the
script through `forge script` with a real Safe treasury, a wrong singleton, a
lookalike, an enabled module and the artifact of a `--optimizer-runs 201` build,
and against a local anvil with Safes created by the factory (an 84532 creation
transaction refused once the anvil is 8453), offline and isolated (`env -i`, `FOUNDRY_OFFLINE=true`, no RPC variable,
`FOUNDRY_CONFIG` unset, a temporary `HOME`, URLs redacted from the output; the
project's `foundry.toml` is used on purpose).

## Deployment

`script/DeployPFTToken.s.sol` reads its values from the environment
(`script/pft-token.env.example` holds placeholders only; keep the filled copy
outside the repository, `*.env` is ignored) and refuses, before the broadcast:

| Check | Refusal |
|---|---|
| `PFT_TOKEN_CHAIN_ID` is 8453 or 84532 and equals the RPC's chain id | `UnsupportedChain`, `WrongChain(expected, actual)` |
| name and symbol are not empty, blank or `replace...` (any casing); name at most 31 bytes | `PlaceholderValue(name)`, `NameTooLong(length)` |
| `PFT_TOKEN_TOTAL_SUPPLY` (units) equals `PFT_TOKEN_TOTAL_SUPPLY_PFT` (whole PFT) × 1,000,000 | `SupplyMismatch(units, pft)` |
| the treasury is not the deployer and has code on the target chain | `TreasuryIsDeployer`, `TreasuryNotAContract` |
| the treasury's code hash is the pinned SafeProxy 1.4.1 or GnosisSafeProxy 1.3.0 runtime code | `ProxyCodeMismatch(treasury, codehash)` |
| its storage slot 0 (the proxy's singleton) is a pinned official Safe singleton | `UnofficialSafeSingleton(treasury, singleton)` |
| its `masterCopy()` (staticcall) returns the slot 0 address | `MasterCopyMismatch(treasury, singleton)` |
| the singleton's code hash on the chain is its pinned runtime code hash | `SingletonCodeMismatch(singleton, codehash)` |
| `getThreshold()` and `getOwners()` answer (checked staticcalls) | `TreasuryIsNotASafe` |
| threshold >= 2; owners distinct, non-zero, at least the threshold | `SafeThresholdTooLow`, `SafeOwnersInvalid` |
| no module: `getModulesPaginated(0x1, 10)` returns an empty list and next `0x1` (a module spends without owner signatures) | `ModulesEnabled(count)` |
| the guard slot (`keccak256("guard_manager.guard.address")`) is zero, or exactly the address in `PFT_TOKEN_ALLOW_GUARD` (a reverting guard blocks every owner transaction, its own removal included); recorded either way | `GuardEnabled(guard)` |
| the signed creation transaction (`PFT_TOKEN_TREASURY_CREATION_RAW_TX`, else `eth_getRawTransactionByHash`) hashes to `PFT_TOKEN_TREASURY_CREATION_TX`; its whole envelope reads: type 2 with exactly 12 items or legacy with exactly 9, integers of at most 32 bytes, an access-list list, yParity 0 or 1, r and s non-zero | `TreasuryCreationUnknown(hash)`, `TreasuryCreationUnreadable` |
| it was signed for this chain: type-2 `chainId`, or legacy `v` = 2 × chain id + 35 or + 36 (v 27/28 refused); the Safe's CREATE2 address and the factories are the same on 8453 and 84532 | `TreasuryCreationWrongChain(chain id)` |
| its `to` is the canonical SafeProxyFactory 1.4.1 `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` or 1.3.0 `0xa6B71E26C5e0845f74c812102Ca7114b6a896AB2` with its pinned code | `TreasuryCreationNotFromFactory(to)` |
| it calls `createProxyWithNonce` for the slot 0 singleton, and the factory's CREATE2 address for that call (from its on-chain `proxyCreationCode()`) is the treasury | `TreasuryCreationMismatch(field)` |
| its initializer is a plain `setup`: `to` zero and `data` empty | `TreasuryInitializerDelegates(to, data length)` |
| the initializer's owners and threshold are the live ones; its fallback handler is zero or the canonical CompatibilityFallbackHandler of that version with its pinned code | `TreasuryCreationMismatch(field)` |
| the initializer pays nothing: `paymentToken` zero and `payment` 0 | `TreasurySetupHasPayment(token, payment)` |
| the artifact under `$FOUNDRY_OUT` (else `out`) `/PFTToken.sol/PFTToken.json` has `bytecode.object` equal to the deployed creation code | `ArtifactMismatch(path, artifact hash, compiled hash)` |
| its `rawMetadata` has `settings.metadata.bytecodeHash` `ipfs` (the editable `metadata` object is not read); the creation code ends with its `deployedBytecode.object`, which has the runtime length the creation code's constructor copies (the size of its `CODECOPY`), exceeds its CBOR trailer by at least 512 bytes and whose trailer holds the IPFS digest of that `rawMetadata` | `MetadataHashDisabled`, `ArtifactMetadataMismatch(path)` |
| the deployer's nonce on the target chain is 0 (a one-time key) | `DeployerAlreadyUsed(deployer, nonce)` |
| a real deployment sets no test switch, sets `PFT_TOKEN_SOURCE_COMMIT` and has no record yet | `RealDeploymentWithTestSwitch`, `SourceCommitMissing`, `RecordExists(path)` |
| right after the broadcast in forge's simulation, before the transaction is submitted, the token's code has the length and the CBOR trailer of `deployedBytecode.object` (the immutables make the rest differ) | `DeployedCodeMismatch(token, code length, artifact length)` |

The Safe checks fail closed on both chains. Pinned singletons (the canonical
deployments in safe-global/safe-deployments, commit `b45417dd`, listed there
for 8453 and 84532): Safe v1.4.1 `0x41675C099F32341bf84BFc5382aF534df5C7461a`,
SafeL2 v1.4.1 `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762`, GnosisSafe v1.3.0
`0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552`, GnosisSafeL2 v1.3.0
`0x3E5c63644E683549055b9Be8653de26E0B4CD36E`. A Safe on the v1.3.0 `eip155`
deployment is refused; create the treasury Safe as v1.4.1.

It prints the chain id, deployer, treasury, Safe singleton, threshold and owner
count, and both supply values. `PFT_TOKEN_ALLOW_EOA_TREASURY=1` (admits a
treasury without code; a contract must still pass the Safe checks) and
`PFT_TOKEN_ALLOW_USED_DEPLOYER=1` are test-only switches for a local run on
84532; on 8453 the script refuses them (`TestSwitchOnMainnet`).

Dry run against a local anvil, with no key and no broadcast:

```bash
anvil --chain-id 84532 &
PFT_TOKEN_CHAIN_ID=84532 PFT_TOKEN_NAME="Post Fiat Token" PFT_TOKEN_SYMBOL=PFT \
PFT_TOKEN_TOTAL_SUPPLY=1000000000000 PFT_TOKEN_TOTAL_SUPPLY_PFT=1000000 \
PFT_TOKEN_TREASURY=0x0000000000000000000000000000000000007EA5 \
PFT_TOKEN_ALLOW_EOA_TREASURY=1 \
  forge script script/DeployPFTToken.s.sol:DeployPFTToken --rpc-url http://127.0.0.1:8545
```

Without `--rpc-url`, forge's in-process EVM starts the sender at nonce 1, so an
in-process run also needs `PFT_TOKEN_ALLOW_USED_DEPLOYER=1`. The same holds for
a read-only re-check of the deployed Base Sepolia token's inputs: its retired
deployer has nonce 2, so a dry run with `PFT_TOKEN_REAL_DEPLOYMENT=1` stops at
`DeployerAlreadyUsed` before `RecordExists`; run it without
`PFT_TOKEN_REAL_DEPLOYMENT` and with `PFT_TOKEN_ALLOW_USED_DEPLOYER=1` (84532
only), and without `--broadcast`.

Pre-deployment checklist:

- [ ] The treasury Safe is deployed on the target chain and its address,
      owners and threshold are checked on the explorer.
- [ ] The treasury Safe has no module (the script refuses one). A guard is
      refused by default; it may be allowed only by naming it in
      `PFT_TOKEN_ALLOW_GUARD` after an independent review.
- [ ] The treasury Safe was created through the canonical SafeProxyFactory
      with a plain `setup` (no `to`, no `data`), and
      `PFT_TOKEN_TREASURY_CREATION_TX` is that transaction's hash. A setup
      delegatecall can write a module the module list never shows, so a
      delegating initializer cannot be told from a hidden module; the
      Safe{Wallet} app's L2 setup contract delegates and is not accepted.
- [ ] `PFT_TOKEN_TREASURY_CREATION_RAW_TX` holds the signed creation
      transaction (`cast tx <hash> --raw --rpc-url <base-rpc-url>`) when the
      RPC does not serve `eth_getRawTransactionByHash`;
      https://sepolia.base.org does not, so it is required there.
- [ ] The build keeps the IPFS metadata hash: no `--no-metadata`, no
      `bytecode_hash = "none"`, and forge's default `out/` (or the same
      `FOUNDRY_OUT` for the build and the script).
- [ ] `PFT_TOKEN_CHAIN_ID` is the target chain (84532 first; 8453 only after
      the audit below).
- [ ] The supply pair `PFT_TOKEN_TOTAL_SUPPLY` / `PFT_TOKEN_TOTAL_SUPPLY_PFT`
      is the decided supply.
- [ ] The one-time deployer key is fresh (nonce 0) and on a hardware wallet,
      used with `--ledger` (or typed once with `--interactives 1`).
- [ ] Neither test-only switch is set; `PFT_TOKEN_REAL_DEPLOYMENT=1` and
      `PFT_TOKEN_SOURCE_COMMIT=$(git rev-parse HEAD)` are.

A Base mainnet (8453) deployment happens only after an independent audit of
the exact commit being deployed.

Deployment (the RPC URL and the key are given on the command line only):

```bash
PFT_TOKEN_REAL_DEPLOYMENT=1 forge script script/DeployPFTToken.s.sol:DeployPFTToken \
  --rpc-url <base-rpc-url> --ledger --sender <deployer-address> --broadcast
```

Only a broadcast with `PFT_TOKEN_REAL_DEPLOYMENT=1` writes the durable
`deployments/pft-token-base-<chain id>/deployment.json`, and never over an
existing one; every other broadcast (a local anvil) writes under the ignored
`cache/deploy-test/`, and a dry run writes none. A real deployment is refused
without `PFT_TOKEN_SOURCE_COMMIT`. The record holds the mode (`real`/`test`),
token, chain id, constructor arguments, deployer, Safe singleton, threshold,
owners, modules (empty), guard, creation transaction, factory and fallback handler, `PFT_TOKEN_SOURCE_COMMIT`, the OpenZeppelin
commit, the artifact path, the keccak256 of the deployed creation code
(`creation_code_keccak256`), of the artifact's runtime code
(`runtime_code_keccak256`) and of the token's code on the chain
(`deployed_code_keccak256`), and the effective compiler settings of the build
(solc version, optimizer and runs, `evm_version`, `bytecode_hash`), read from
the `rawMetadata` of that artifact (`$FOUNDRY_OUT/PFTToken.sol/PFTToken.json`,
else `out/...`), the string whose IPFS digest the code's trailer carries, never
from its unchecked `metadata` object. The artifact must carry the deployed
creation code, so a command-line override such as `--optimizer-runs` is what the
record shows and an artifact of another build is refused. The script runs before the
transaction is sent, so `tx_hash` and `block_number` are `pending`.
`broadcast/` stays ignored.

The treasury check fails closed unless the treasury's code hash is the pinned
SafeProxy 1.4.1 or GnosisSafeProxy 1.3.0 runtime code, its slot 0 and its
`masterCopy()` name the same pinned singleton, and that singleton carries its
pinned runtime code on the chain (hashes and their source in
`script/DeployPFTToken.s.sol`; the real bytecodes the tests run are in
`test/fixtures/safe/`).

After the broadcast, in this order:

1. Confirm the receipt: the transaction in
   `broadcast/DeployPFTToken.s.sol/<chain id>/run-latest.json` is mined with
   status 1 on the explorer, and the token address it created has the code.
   Every script check, `checkDeployedCode` included, ran in forge's simulation
   before the transaction was submitted; none of them read the mined chain.
2. Verify the source on Basescan (`BASESCAN_API_KEY` exported in the shell,
   never in a file; `foundry.toml` reads it for chain ids 8453 and 84532):

   ```bash
   forge verify-contract <address> src/PFTToken.sol:PFTToken --chain <8453|84532> \
     --constructor-args $(cast abi-encode "constructor(string,string,uint256,address)" \
       "$PFT_TOKEN_NAME" "$PFT_TOKEN_SYMBOL" "$PFT_TOKEN_TOTAL_SUPPLY" "$PFT_TOKEN_TREASURY") \
     --compiler-version v0.8.24+commit.e11b9ed9 --num-of-optimizations 200 --evm-version cancun
   ```

   The encoded arguments equal `constructor_args` in the deployment record,
   and the compiler settings equal the record's.
3. On the explorer, `totalSupply()` equals `PFT_TOKEN_TOTAL_SUPPLY` and the
   treasury Safe holds all of it, and the record's `treasury_safe_creation_tx`
   is a mined transaction of the target chain that created the treasury. The
   script binds the raw bytes to that hash, not the hash to the chain; this
   step does.
4. Finalise the record (outcome 3 below): check that `deployed_code_keccak256` equals
   `cast keccak $(cast code <token> --rpc-url <base-rpc-url>)`, copy `tx_hash`
   and `block_number` in from `run-latest.json`, fill
   `verification_sourcify`, `verification_sourcify_url`,
   `verification_blockscout`, `verification_blockscout_url` and
   `verification_basescan` with each explorer's verification result, then
   commit it.
5. Retire the deployer key; it holds nothing and is never used again.

If the broadcast fails or its outcome is unclear (a timeout, a dropped
connection, an unknown receipt), remember that the record and the script's checks are
results of forge's simulation before submission, not of the chain. Read the
deployer's nonces and receipt, then act on the one outcome that holds; the
explorer showing no transaction is not, alone, proof that none was submitted.

1. **Not submitted.** The deployer's latest and pending nonces
   (`cast nonce <deployer> --block latest`, `--block pending`) are both
   unchanged and no transaction exists: delete the uncommitted record; the run
   may be repeated.
2. **Pending.** The pending nonce is ahead of the latest: wait and re-read.
   Never send a second transaction.
3. **Mined and successful.** Receipt status 1: copy `tx_hash` and
   `block_number` into the record, compare the token's code hash on the chain
   (`cast keccak $(cast code <token> --rpc-url <base-rpc-url>)`) with
   `deployed_code_keccak256` and the explorer's; only then is the record final.
4. **Mined and reverted.** Receipt status 0: the nonce is consumed and no
   token exists. Archive the record under a `failed-<tx hash>` name with the
   receipt (`cast receipt <tx hash> --json`) beside it; any retry uses a fresh
   deployer at nonce 0.

If any check fails after a broadcast was mined (an ambiguous or mismatching
outcome: a step above fails, or forge reports an error once the transaction
is on chain), stop. Keep `broadcast/DeployPFTToken.s.sol/<chain id>/run-latest.json`
and the explorer evidence, reconcile the record by hand as the outcomes above
say, and never retry with the same deployer.

OpenZeppelin pin: `lib/openzeppelin-contracts` is a git submodule at tag
`v5.7.0`, commit `cab19933c33c2ad1d4c7a84864a3601dddfd16f3`. `foundry.toml`
pins solc 0.8.24, optimizer 200 runs, `evm_version = "cancun"`,
`bytecode_hash = "ipfs"` and `cbor_metadata = true` (the verification inputs);
`src/PFTToken.sol` pins `pragma solidity 0.8.24`.

## Lock contract (Phase 2, Base)

`src/PFTLockV1.sol` is the Phase 2 lock of the
[design note](../../docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md)
(sections 3, 6 and 8): it turns ERC-20 PFT on Base into a claim on native PFT
at PFTL's genesis lane. `lock(amount, pftlRecipient)` pulls the tokens with
`transferFrom`, takes the next nonce (from 0), writes a permanent record
(`lockRecord(nonce)`: holder, amount, keccak256 of the recipient), emits one
`Locked` event and keeps the tokens. `lockWithPermit` does the same after an
EIP-2612 permit. A failed permit (expired, wrongly signed, or submitted first
by someone else) is tolerated when the existing allowance covers the amount
and otherwise refused (`PermitFailed`); the deadline bounds the permit, not
the lock.

A Lock is refused for a zero amount, an amount above `maxLockAmount` (the
genesis per-window release cap, `uint64`), an empty or over-256-byte
recipient, a token whose runtime code hash differs from the one fixed at
construction, or a balance delta different from the amount. The recipient is
checked only for length: check the PFTL address before locking; a wrong one
cannot be undone. Tokens sent by plain transfer are not a Lock and are lost.

No privileged role: no owner, admin, pause, setter, sweep or upgrade path.
The constructor `PFTLockV1(token, finalityVerifier, tokenRuntimeCodeHash,
maxLockAmount)` refuses a zero address, an account without code, a wrong code
hash and a zero cap; everything it takes is immutable. The ABI is exactly
`IPFTLockV1` (ten functions, checked by
`test_contract_has_no_owner_pause_or_withdraw`).

Invariants (design note, section 3; `test/PFTLockV1Invariant.t.sol`, 64 runs,
depth 32, `fail_on_revert`):

1. `totalLocked - totalReleased` is the sum of `Locked` amounts minus
   releases, and the lock's balance is at least that; the difference is
   plain transfers only.
2. Nonces run from 0 to `nextNonce - 1`, each with one record and one event.
3. Only `release` lowers the lock's balance.
4. `totalLocked` stays within the token's fixed supply while nothing is
   released; once releases exist, `totalLocked - totalReleased` does (a
   released token can be locked again, so the cumulative total can pass the
   supply).

The release path, `release(publicValues, proofBytes)`, is dormant until
Phase 3. It ships now because the contract has no admin: a release function
added later could never reach tokens locked before it. It pays out only what
the immutable `finalityVerifier` (`IPFTLFinalityVerifierV1.verifyAndConsume`,
as in `ERC20BridgeVaultL1`) accepts, refuses a zero amount or recipient, a
replayed withdrawal or burn commitment and an amount above
`totalLocked - totalReleased`, checks both balance deltas, and emits
`Released`. The release path has no phase switch of its own: it is dormant
only as long as the finality verifier accepts no export proof, so the
verifier's deployment and qualification on Base decide when Phase 3 opens.

Not deployed: no lock contract exists on Base Sepolia or Base mainnet, and no
PFTL finality verifier is deployed on Base, which must come first. The
genesis lane and the PFTL release operation (P2.2, P2.4) are not built, and
mainnet deployment waits for the audit (design note, section 8). The tests
use a mock verifier.

## Deploy the lock contract

Order (design note, sections 5 and 6): the PFTL finality verifier is deployed
on Base first, then the lock contract, which fixes the verifier at
construction; genesis later names the lock's address and runtime code hash.
No PFTL finality verifier exists on Base yet, so only test runs are possible.

`script/DeployPFTLock.s.sol` follows `script/DeployPFTToken.s.sol` (its own
cheatcode interface, named errors, placeholder refusal, test-only switches on
84532 only, the artifact binding). Its values come from the environment
(`script/pft-lock.env.example`, placeholders only). Before the broadcast it
refuses:

| Check | Refusal |
|---|---|
| any input empty, blank, `replace...` or zero | `PlaceholderValue(name)` |
| `PFT_LOCK_CHAIN_ID` is 8453 or 84532 and equals the RPC's chain id | `UnsupportedChain`, `WrongChain(expected, actual)` |
| on 84532, `PFT_LOCK_TOKEN` and `PFT_LOCK_TOKEN_CODEHASH` are the recorded test token and its code hash (`BASE_SEPOLIA_TEST_TOKEN`, `BASE_SEPOLIA_TEST_TOKEN_CODEHASH`) unless `PFT_LOCK_ALLOW_TEST_TOKEN=1` | `TokenNotTheTestToken(token)` |
| `PFT_LOCK_TOKEN` has code whose keccak256 is `PFT_LOCK_TOKEN_CODEHASH`; `decimals()` is 6 and `totalSupply()` > 0 (checked staticcalls) | `TokenNotAContract`, `TokenCodeHashMismatch(expected, actual)`, `TokenUnreadable(field)`, `TokenDecimalsMismatch`, `TokenSupplyZero` |
| `PFT_LOCK_VERIFIER` has code whose keccak256 is `PFT_LOCK_VERIFIER_CODEHASH` | `VerifierNotAContract`, `VerifierCodeHashMismatch(expected, actual)` |
| `PFT_LOCK_MAX_LOCK_AMOUNT` (units) fits `uint64` and equals `PFT_LOCK_MAX_LOCK_AMOUNT_PFT` × 1,000,000 | `MaxLockAmountTooLarge`, `MaxLockAmountMismatch(units, pft)` |
| the deployer's nonce is 0 | `DeployerAlreadyUsed(deployer, nonce)` |
| `out/PFTLockV1.sol/PFTLockV1.json` (or under `$FOUNDRY_OUT`) is the deployed creation code with its IPFS metadata; its `deployedBytecode.object` exceeds its CBOR trailer by at least 512 bytes | `ArtifactMismatch`, `MetadataHashDisabled`, `ArtifactMetadataMismatch` |
| a real deployment sets no test switch, sets `PFT_LOCK_SOURCE_COMMIT` to exactly 40 hexadecimal characters (either casing; the shape only, the value is the operator's statement) and has no record yet | `RealDeploymentWithTestSwitch`, `SourceCommitMissing`, `SourceCommitInvalid(value)`, `RecordExists(path)` |
| right after the broadcast, the lock's code has the length and the CBOR trailer of `deployedBytecode.object` (the immutables make the rest differ) | `DeployedCodeMismatch(lock, code length, artifact length)` |

After construction it re-reads `token()`, `finalityVerifier()`,
`maxLockAmount()`, `nextNonce() == 0` and `totalLocked() == 0`
(`LockStateMismatch(field)`), and logs every checked value and the lock's
runtime code hash. `PFT_LOCK_ALLOW_TEST_VERIFIER=1` admits a verifier whose
code hash is not pinned (`PFT_LOCK_VERIFIER_CODEHASH` may stay unset) and
`PFT_LOCK_ALLOW_USED_DEPLOYER=1` a used deployer and `PFT_LOCK_ALLOW_TEST_TOKEN=1`
a token other than the recorded test token; all three are refused on 8453
(`TestSwitchOnMainnet`) and with `PFT_LOCK_REAL_DEPLOYMENT=1`.
`test/DeployPFTLock.t.sol` covers every refusal and the accepted paths (100 %
line coverage of the script).

On 84532 the token must be the test token `0x18fE4ca51A0cE042688D5A54917319d03B527D92`,
runtime keccak256 `0xb0939a2407cd571d158498ba0f405f4ddb3857e688d2bc5ada170dcc655903df`
(its [dry-run record](../../docs/listings/coinbase/pft-token-dry-run-20261026.md));
any other token needs the test-only `PFT_LOCK_ALLOW_TEST_TOKEN=1`.
`testDryRunAgainstTheRecordedBaseSepoliaTestToken` rebuilds it at that address
with that hash. Initialise the OpenZeppelin submodule without `--recursive`: its
nested submodules add remappings to the metadata and change the bytecode.

Preflight: `test/deploy-lock-cli.sh` (run in CI with `test/deploy-script-cli.sh`)
first compares `git -C lib/openzeppelin-contracts rev-parse HEAD` with
`OPENZEPPELIN_COMMIT` and fails on a mismatch or an initialised nested
submodule; run that comparison before any deployment. Its anvil cases, isolated
as the token script's: the recorded test token rebuilt at its address, a dry run
refusing an unpinned verifier without the switch, the switch with
`PFT_LOCK_REAL_DEPLOYMENT=1` and another token without
`PFT_LOCK_ALLOW_TEST_TOKEN`, then a broadcast with the switch and a stand-in
verifier whose test record holds the five compiler settings and both code hashes.

Dry run against a local anvil (no key, no broadcast; the token and a stand-in
verifier deployed there first, e.g. `forge create` with `--unlocked`):

```bash
anvil --chain-id 84532 &
PFT_LOCK_CHAIN_ID=84532 PFT_LOCK_TOKEN=<token> \
PFT_LOCK_TOKEN_CODEHASH=$(cast keccak $(cast code <token> --rpc-url http://127.0.0.1:8545)) \
PFT_LOCK_VERIFIER=<verifier> PFT_LOCK_ALLOW_TEST_VERIFIER=1 PFT_LOCK_ALLOW_TEST_TOKEN=1 \
PFT_LOCK_MAX_LOCK_AMOUNT=2000000000000000 PFT_LOCK_MAX_LOCK_AMOUNT_PFT=2000000000 \
  forge script script/DeployPFTLock.s.sol:DeployPFTLock --rpc-url http://127.0.0.1:8545 \
  --unlocked --sender <fresh-anvil-account>
```

A locally deployed token is not the recorded Base Sepolia test token, so the
example sets `PFT_LOCK_ALLOW_TEST_TOKEN=1`; the switch is accepted on 84532 only
and is never used for a real deployment. Adding `--broadcast` deploys on the
anvil and writes the test record. The
test-network run is the same command against Base Sepolia with the recorded
test token, a stand-in verifier deployed there, `--ledger --sender
<deployer-address> --broadcast` and the switch; it writes its record under the
ignored `cache/deploy-test/`, because a real deployment refuses the switch.

The record: only a broadcast with `PFT_LOCK_REAL_DEPLOYMENT=1` writes the
durable `deployments/pft-lock-base-<chain id>/deployment.json`, never over an
existing one; other broadcasts write under `cache/deploy-test/`, a dry run
none. It holds the mode, the lock and its runtime code hash, the chain id, the
token and its code hash, the verifier, its on-chain code hash and whether it
was pinned, `maxLockAmount` in units and PFT, the constructor arguments, the
deployer, `PFT_LOCK_SOURCE_COMMIT`, the OpenZeppelin commit, the artifact, the
keccak256 of the creation code (`creation_code_keccak256`), of the artifact's
runtime code (`runtime_code_keccak256`) and of the lock's code on the chain
(`deployed_code_keccak256`), and the effective compiler settings (solc,
optimizer, runs, `evm_version`, `bytecode_hash`), read from the artifact's
`rawMetadata`, whose IPFS digest the code carries, never from its unchecked
`metadata` object. The record is written during the run, before the
transaction is mined, so `tx_hash` and `block_number` are `pending` until copied
from `broadcast/DeployPFTLock.s.sol/<chain id>/run-latest.json`.

If the broadcast fails or its outcome is unclear (a timeout, a dropped
connection, an unknown receipt), remember that the record and its code checks (`DeployedCodeMismatch`, `LockStateMismatch`) are
results of forge's simulation before submission, not of the chain. Read the
deployer's nonces and receipt, then act on the one outcome that holds; the
explorer showing no transaction is not, alone, proof that none was submitted.

1. **Not submitted.** The deployer's latest and pending nonces
   (`cast nonce <deployer> --block latest`, `--block pending`) are both
   unchanged and no transaction exists: delete the uncommitted record; the run
   may be repeated.
2. **Pending.** The pending nonce is ahead of the latest: wait and re-read.
   Never send a second transaction.
3. **Mined and successful.** Receipt status 1: copy `tx_hash` and
   `block_number` into the record, compare the lock's code hash on the chain
   (`cast keccak $(cast code <lock> --rpc-url <base-rpc-url>)`) with
   `deployed_code_keccak256` and the explorer's; only then is the record final.
4. **Mined and reverted.** Receipt status 0: the nonce is consumed and no
   lock exists. Archive the record under a `failed-<tx hash>` name with the
   receipt (`cast receipt <tx hash> --json`) beside it; any retry uses a fresh
   deployer at nonce 0.

The Base mainnet run additionally needs:

- [ ] the real PFTL finality verifier deployed and qualified on Base, its
      address and runtime code hash pinned in `PFT_LOCK_VERIFIER` and
      `PFT_LOCK_VERIFIER_CODEHASH` (no test switch);
- [ ] the independent audit of the exact commit (design note, section 8);
- [ ] `maxLockAmount` as the genesis plan fixes it (section 5.1);
- [ ] the mainnet PFTToken and its code hash, a fresh deployer on a hardware
      wallet, `PFT_LOCK_REAL_DEPLOYMENT=1` and
      `PFT_LOCK_SOURCE_COMMIT=$(git rev-parse HEAD)`.
