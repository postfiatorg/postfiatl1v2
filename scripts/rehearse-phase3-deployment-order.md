# Phase 3 deployment-order rehearsal (local Anvil)

`scripts/rehearse-phase3-deployment-order.py` rehearses the deployment order of
the export design note, section 5 "Proof on Base"
(`docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md`). It runs
on a loopback Anvil with chain id 84532 only. It never takes a remote URL or a
key: the deployer keys are generated at run time, kept in memory and never
written.

```
scripts/rehearse-phase3-deployment-order.py --output <fresh-directory> [--node-bin target/debug/postfiat-node]
```

The worktree must be clean, so the manifest names the commit and tree that ran.
Foundry writes its artifacts, cache and broadcasts under `<fresh-directory>`.
Without `--node-bin` the script builds `postfiat-node` into this checkout's
`./target`. Exit 0 means every check held. Exit 1 means a step failed, and
`summary.json` holds the error. The `deployment` block of `summary.json` and
`manifest.json` is filled as each step succeeds, so after a failure it still
says what landed (`steps_completed`, `complete: false`). Exit 2 means a precondition was refused.

## What it does

1. Starts Anvil, generates four disposable keys and funds them with
   `anvil_setBalance`: the token's deployer, the verifier's deployer and the
   lock's deployer (the two dedicated deployers of section 5), and a token
   holder. The token mints its whole supply, the lane's `reserved_atoms`, to
   the holder.
2. Predicts the verifier's CREATE address from the verifier deployer's next
   nonce and the lock's from the lock deployer's next nonce (RLP
   `[deployer, nonce]`, Keccak-256). Each prediction is cross-checked with
   `cast compute-address`.
3. Predicts both runtime code hashes on an `evm_snapshot` that is then reverted.
   - The lock holds the verifier's address as an immutable. So a probe verifier
     with a placeholder vault hash and the lock are deployed from their
     deployers at the planned nonces, and the lock's code is hashed.
   - On a second snapshot, the verifier is deployed with that lock code hash,
     and its code is hashed.
   - After each revert the script checks that both deployers' nonces and both
     addresses are back as they were.
   - The lock's constructor values come from the genesis it is pinned in.
     `maxLockAmount` is the `base-erc20` lane's per-window release cap,
     `reserved_atoms * release_cap_bps // 10_000` (Phase 2 design note,
     section 5.1): 6,000,000 atoms for the fixture's 300,000,000 atoms at 200
     basis points. After the node reads the genesis back, the script refuses if
     the two differ. `minimumLockAmount` is 10 atoms, the lock's floor
     `PFTL_ACCOUNT_RESERVE` (`PFTLockV1.sol:128`), which is the PFTL account
     reserve `ACCOUNT_RESERVE` of `crates/execution/src/entrypoints.rs:139`.
4. Writes a lane genesis that pins the predicted lock address and code hash, plus
   the token. The base is the fixture
   `crates/node/testdata/external-lane-genesis/genesis.json`, with its fixture
   committee. The node reads it with `postfiat-node init --genesis-file`, which
   validates the genesis. The script compares the lane pins in the data
   directory's `genesis.json` with the written ones.
5. Refuses a deployer nonce off by one, once per deployer. On a snapshot, a
   stray zero-value transaction moves that deployer's nonce. The other
   contract's predicted address stays intact, but the guard refuses the pins as
   a whole: genesis pins the lock and the lock pins the verifier, so one lost
   address loses both. No code appears at either predicted address.
6. Deploys the verifier, then the lock, each from its deployer at its planned
   nonce. The verifier is seeded with the fixture checkpoint of
   `crates/ethereum-contracts/test/PFUSDCTier4.t.sol`.
   - Immediately before each deployment the guard reads the `latest` and
     `pending` transaction counts: of both deployers before the verifier, of
     the lock deployer before the lock. Both counts must equal the planned
     nonce, or the script refuses.
   - Each `forge create` pins its nonce with `--nonce`. A transaction that takes
     the nonce first makes the deployment fail; it cannot shift an address.
7. Checks on chain:
   - both addresses and both runtime code hashes equal the predictions;
   - every constructor value is read back through its getter and compared, by
     name, with the value the script passed: all 17 fields of the verifier's
     `Config` (the three `initial...` values through `latestCheckpointCommitment`,
     `latestFinalizedHeight` and `latestCommitteeRootCommitment`) and the lock's
     `token`, `finalityVerifier`, `maxLockAmount` and `minimumLockAmount`. The
     predicted code hash comes from the same arguments, so only this read-back
     catches two same-typed values swapped in them;
   - the genesis pins equal the deployed lock and token.
8. Runs the ERC-20 path once. The holder calls `approve` and one `lock` of the
   minimum, 10 atoms, to a canonical PFTL address (`pf` + 40 lower-case hex,
   derived from a label, so no key holds it). Then `nextNonce()` is 1,
   `totalLocked()` and the lock's token balance are 10, `lockRecord(0)` names
   the holder, the amount and the recipient's hash, and `token.decimals()` is 6.

`summary.json` and `manifest.json` are written to the output directory, and
Anvil is stopped.
- The manifest holds the addresses, code hashes, nonces, tool versions, the
  source commit and tree, the sha256 of each source (the script, its test file,
  the contracts, the lane types and the genesis fixture) and Foundry artifact,
  the OpenZeppelin submodule revision the token crate built against, and the
  node binary's sha256.
- `genesis-lane-pins.json` is published with the summary and manifest: the
  generated genesis's `base-erc20` lane (its `reserved_atoms`, window and cap
  parameters, and its policy with the four pins) and the sha256 of the whole
  generated `genesis.json`, which is not published.
- It also records the cargo, rustc and Python versions, the node's build
  profile and the host OS.
- `postfiat_node_sha256` identifies the exact binary used. It is not expected to
  reproduce across builds; the source, configuration and artifact hashes are.
- `not_published` lists every other file under the output directory,
  recursively, by its path relative to that directory, with sha256 and size.
- The generated node directory holds dev validator keys. It is removed after a
  passing run.

## Limits

- **Fixture checkpoint.** The verifier's checkpoint, genesis-hash commitment,
  route profile, epoch and asset id are fixture values. In production the first
  finalized PFTL checkpoint of the pinned genesis and the Base route profile
  provide them. The genesis is only validated by `init`; no chain is run.
- **No proof and no release.** The SP1 verifier is a fixture address, which
  the constructor only requires to be non-zero, so no proof is verified and no
  `release` runs. The one Lock stays locked.
- **The guard is not race-free.** A transaction sent between the check and the
  deployment can still take the planned nonce. The pinned nonce then makes the
  deployment fail, but the predicted address is lost and the genesis pins can
  never be met. Locally only the script holds the keys and Anvil mines at once.
  In production nothing else may hold or use the two dedicated deployer keys,
  and nothing else may be sent from them.
- **A verifier without its lock is unusable.** If the verifier is deployed and
  the lock then cannot be deployed at its predicted address, the verifier is
  unusable: it pins the predicted lock's code hash, which holds the verifier's
  address. Redeploying the verifier changes its address, so the lock's code
  hash and the genesis pins change with it.
- **Base Sepolia waits.** The Base Sepolia dry run waits for the Base route
  profile and asset id.

The tests in `python/tests/test_rehearse_phase3_deployment_order.py` run
without Anvil. They cover the Keccak and CREATE vectors, the nonce guard and
the two-deployer plan, the constructor read-back, the lock cap, the Lock path
checks, the genesis pinning, the partial record and the manifest shape.
