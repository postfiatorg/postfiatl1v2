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
`summary.json` holds the error. Exit 2 means a precondition was refused.

## What it does

1. Starts Anvil, generates two disposable keys and funds them with
   `anvil_setBalance`. One key deploys the PFT token; the other deploys the
   verifier and the lock.
2. Predicts the verifier's CREATE address at the deployer's next nonce `n` and
   the lock's at `n + 1` (RLP `[deployer, nonce]`, Keccak-256). The prediction
   is cross-checked with `cast compute-address`.
3. Predicts both runtime code hashes on an `evm_snapshot` that is then reverted.
   - The lock holds the verifier's address as an immutable. So a probe verifier
     with a placeholder vault hash and the lock are deployed at the predicted
     nonces, and the lock's code is hashed.
   - On a second snapshot, the verifier is deployed with that lock code hash,
     and its code is hashed.
   - After each revert the script checks that the deployer's nonce and both
     addresses are back as they were.
4. Writes a lane genesis that pins the predicted lock address and code hash, plus
   the token. The base is the fixture
   `crates/node/testdata/external-lane-genesis/genesis.json`, with its fixture
   committee. The node reads it with `postfiat-node init --genesis-file`, which
   validates the genesis. The script compares the lane pins in the data
   directory's `genesis.json` with the written ones.
5. Refuses a deployer nonce off by one. On a snapshot, a stray zero-value
   transaction moves the nonce. The guard computes where both contracts would
   land, refuses, and no code appears at either predicted address.
6. Deploys the verifier, then the lock, at the predicted nonces. The verifier is
   seeded with the fixture checkpoint of
   `crates/ethereum-contracts/test/PFUSDCTier4.t.sol`.
   - Immediately before each deployment the guard reads the deployer's `latest`
     and `pending` transaction counts. Both must equal the planned nonce (`n`
     for the verifier, `n + 1` for the lock), or the script refuses.
   - Each `forge create` pins its nonce with `--nonce`. A transaction that takes
     the nonce first makes the deployment fail; it cannot shift an address.
7. Checks on chain:
   - both addresses and both runtime code hashes equal the predictions;
   - `lock.finalityVerifier()` is the verifier;
   - the verifier's `vaultRuntimeCodeHash`, `token` and `tokenRuntimeCodeHash`
     are the expected values;
   - the verifier's seeded height and checkpoint are as given, and its chain id
     is 84532;
   - the genesis pins equal the deployed lock and token.

`summary.json` and `manifest.json` are written to the output directory, and
Anvil is stopped.
- The manifest holds the addresses, code hashes, nonces, tool versions, the
  source commit and tree, the sha256 of each source and Foundry artifact, and
  the node binary's sha256.
- The generated node directory holds dev validator keys. It is removed after a
  passing run.

## Limits

- **Fixture checkpoint.** The verifier's checkpoint, genesis-hash commitment,
  route profile, epoch and asset id are fixture values. In production the first
  finalized PFTL checkpoint of the pinned genesis and the Base route profile
  provide them. The genesis is only validated by `init`; no chain is run.
- **No proofs.** The SP1 verifier is a fixture address, which the constructor
  only requires to be non-zero, so no proof is verified and no `release` runs.
- **One deployer.** Section 5 recommends one deployer for the verifier and one
  for the lock. This rehearsal uses a single deployer and its next two nonces.
  The ordering argument is the same: a stray transaction moves either
  prediction.
- **The guard is not race-free.** A transaction sent between the check and the
  deployment can still take the planned nonce. The pinned nonce then makes the
  deployment fail, but the predicted address is lost and the genesis pins can
  never be met. Locally only the script holds the key and Anvil mines at once. A
  production run must also use the two dedicated deployer keys of section 5.
  Nothing else may hold or use them, and nothing else may be sent from them.
- **Base Sepolia waits.** The Base Sepolia dry run waits for the Base route
  profile and asset id.

The tests in `python/tests/test_rehearse_phase3_deployment_order.py` run
without Anvil. They cover the Keccak and CREATE vectors, the nonce guard, the
genesis pinning and the manifest shape.
