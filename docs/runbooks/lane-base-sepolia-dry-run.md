# Lane Base Sepolia dry run

This runbook runs the Phase 3 deployment order (export design note, section 5) on Base Sepolia as one resumable command, `scripts/rehearse-lane-base-sepolia.py`. Without a real SP1 proof, the run stops before any payout on Base. The same command with `--fork` runs every stage on a local Anvil fork and sends nothing to Base Sepolia; its record is `docs/review/lane-base-sepolia-fork-rehearsal-20261110/`.

## Preconditions

- **Program key:** the lane export program's ELF and `program-info.json` are committed under `programs/pft-lane-export/` from two matching Docker builds (workflow `pft-lane-export-identity`). The identity check must be green on the commit you run. By default the command reads the committed `program-info.json` (`--program-info` overrides it) and never a constant. `plan` refuses when the committed ELF's sha256 differs from the program info.
- **Review:** this script and the deploy sheet of a `--fork` run have been reviewed.
- **Holder:** the rehearsal-token holder `0x6E4E8e0ed73f807D8e8479cb8f25D87Ce8811E17` (keystore `pft-sepolia-lock-deployer`) has enough test ETH to fund the two deployers and pay for its own approve and Lock. The deploy sheet gives the amounts.
- **Keystores:** three one-time Foundry keystores exist and are funded from the holder: the verifier deployer, the lock deployer and a separate relayer. The two deployers have never been used. The operator who holds them is named before the run. The command takes `--verifier-account`, `--lock-account`, `--holder-account` and `--relayer-account`, each with its `--*-password-file`. It has no raw-key option, and it redacts account names and password-file paths from every error.
- **Address checks:** before any broadcast, `deploy` derives each deployer keystore's address and requires it to equal the deployer `plan` predicted from (`--verifier-deployer`, `--lock-deployer`). It re-reads both nonces, latest and pending, against the plan. It refuses when Base Sepolia's gas price is more than 5x the plan's.
- **Committed key only:** off the fork, the program key comes only from the committed `programs/pft-lane-export/program-info.json`, in a clean worktree at a commit. The committed ELF must hash to its `elf_sha256`, and `releases.json` must name the same identity and its two runs. `--program-info` is accepted only with `--fork`. The ELF hash, key and source commit go into the deploy sheet and the constructor record.
- **Not the mock:** the SP1 verifier is Succinct's versioned `V6_1_0_SP1_VERIFIER_GROTH16` at `0xb69f2584CBcFf99a58C4e7002E8b89Af54a6f4e2` (sp1-contracts `v6.1.0`, `contracts/deployments/84532.json`). A mock SP1 verifier is refused off a loopback Anvil.

## Stages

Run each stage with `--through STAGE`. A stage runs once, and only after the stages before it (`state.json`).

1. **`plan`.** Reads the head, the gas price, the token's code hash and the SP1 verifier's code hash and `VERIFIER_HASH`. Predicts both CREATE addresses and the lock's code hash on a throwaway fork. Writes the lane pins.
   - Check: `lane-genesis-pins.json` names the predicted lock.
2. **`fleet`.** Starts the PFTL chain from those pins and takes its first finalized checkpoint. The fleet is the round-trip driver: one data directory driven by an ignored node test, with all six validators signing every block's consensus v2 commit, but no separate node processes or network.
   - Check: the verifier constructor values in `state.json`.
3. **`preflight`.** On a fork at the planned block, deploys both contracts at the planned nonces with the real constructor values, reads every value back, and runs one approve and Lock. Refuses unless the fork matches the predictions. Writes `deploy-sheet.json` and `deploy-sheet.md` with gas and test ETH per key.
   - Check: fund the keys from the sheet.
4. **`deploy`.** The verifier, then the lock, at the pinned nonces. After the verifier, it requires the predicted address and the preflight's code hash before the lock is sent; if they differ, the plan is dead and the lock is not deployed. The same check runs for the lock before the stage is marked complete. Then it verifies both on Sourcify. `deploy` starts only with `--i-accept-no-resume-between-deploy-and-export` (see Recovery).
   - Check: the addresses equal the sheet's.
5. **`lock`.** The holder approves and Locks 1,000,000 units to the PFTL recipient.
6. **`release-on-pftl`.** The first wait: Base finality of the Lock's block, about an hour on 2026-11-06. Then the four checkpoint commands read Base Sepolia, and the release lands on the PFTL chain.
   - Check: the recipient holds 1,000,000 atoms.
7. **`export`.** Exports 400,000 atoms to a fresh Base address, then builds the proof input with the relay.
   - Check: the export block carries the lane export root.
8. **`prove-and-release`.** Without `--proof` it stops. It prints the prover-command contract, the program key, the input's sha256 and the expected public-values hash. With a proof it runs the relay's `release` with every pre-send check.
9. **`record`.** Writes the summary and the manifest.

Stages 2 to 7 share the fleet driver, so they run in one invocation. A crash between them restarts from a fresh output directory, but already deployed contracts cannot be redeployed at the same nonces.

## Recovery

Before every public transaction, the state file records the stage, the role, the sender, the nonce and the purpose. For approve and Lock it then records the hash as soon as cast prints it, and then the receipt; for a deployment, `forge create` returns the hash only once the transaction is mined.

On a rerun, recorded intents are reconciled before anything else:

- **By hash:** the receipt and its status.
- **Deployments:** otherwise by the sender's nonce and the code at the predicted address.
- **Approve and Lock:** by the allowance, and by the lock's `nextNonce` and `lockRecord`.

A mined deployment or Lock that matches the plan is adopted. One that does not is refused loudly. A second Lock is never sent for one plan.

**What cannot be recovered:** the PFTL chain between `fleet` and `export`. It lives in the command's process. Its data directory stays on disk under the output directory, but the driver cannot reopen it.

A crash after `deploy` therefore leaves the verifier and the lock deployed and bound to a chain that is gone. The verifier pins that chain's genesis hash, chain id and first checkpoint. A rerun reconciles and records what was mined, then stops. The contracts are usable for a new run only if the same genesis and first checkpoint can be rebuilt; that has not been shown.

`deploy` refuses to start unless the operator passes `--i-accept-no-resume-between-deploy-and-export`.

## What `prove-and-release` needs

A prover for SP1 v6.3.1 Groth16: either a GPU host or a prover-network account. Who provides it is still open. That is also the second wait.

## What the dry run proves

Without a proof, it shows:

- the deployment order on a public chain;
- the lock bound to the real verifier and the real SP1 verifier;
- a real Lock released on PFTL;
- a real export and its proof input.

It does not show a payout on Base. That needs the proof.
