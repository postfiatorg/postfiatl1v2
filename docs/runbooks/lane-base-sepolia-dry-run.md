# Lane Base Sepolia dry run

This runbook runs the Phase 3 deployment order (export design note, section 5) on Base Sepolia as one resumable command, `scripts/rehearse-lane-base-sepolia.py`. Without a real SP1 proof, the run stops before any payout on Base. The same command with `--fork` runs every stage on a local Anvil fork and sends nothing to Base Sepolia; its record is `docs/review/lane-base-sepolia-fork-rehearsal-20261110/`.

The dry-run lock on Base Sepolia is bound to a PFTL chain whose committee keys come from fixed seeds in the repository, so these two contracts are a rehearsal and must never be named as the lane's lock.

## Preconditions

- **Program key:** the lane export program's ELF and `program-info.json` are committed under `programs/pft-lane-export/` from two matching Docker builds (workflow `pft-lane-export-identity`). The identity check must be green on the commit you run. By default the command reads the committed `program-info.json` (`--program-info` overrides it) and never a constant. `plan` refuses when the committed ELF's sha256 differs from the program info.
- **Review:** this script and the deploy sheet of a `--fork` run have been reviewed.
- **Holder:** the rehearsal-token holder `0x6E4E8e0ed73f807D8e8479cb8f25D87Ce8811E17` (keystore `pft-sepolia-lock-deployer`) has enough test ETH to fund the two deployers and pay for its own approve and Lock. The deploy sheet gives the amounts.
- **Keystores:** three one-time Foundry keystores exist and are funded from the holder: the verifier deployer, the lock deployer and a separate relayer. The two deployers have never been used. The operator who holds them is named before the run. The command takes `--verifier-account`, `--lock-account`, `--holder-account` and `--relayer-account`, each with its `--*-password-file`. It has no raw-key option, and it redacts account names and password-file paths from every error.
- **Checks before the first transaction:** `plan` refuses deployers equal to the holder or to each other. Before the first intent, `deploy` requires: each deployer keystore's address equals the planned deployer; the holder keystore's address equals the holder; the relayer keystore opens; the two deployers and the holder hold the ETH their steps need at the current gas price; the holder holds the Lock amount; the node binary exists; the PFTL driver is alive. It re-reads both deployer nonces, latest and pending, and asserts chain id 84532. The gas price may be at most 3x the plan's, the margin the sheet funds; the guard runs again before approve and Lock. After the Lock, the `Locked` event must name the holder.
- **Committed key only:** off the fork, the program key comes only from the committed `programs/pft-lane-export/program-info.json`, in a clean worktree at a commit. The committed ELF must hash to its `elf_sha256`, and `releases.json` must name the same identity and its two runs. `--program-info` is accepted only with `--fork`. The ELF hash, key and source commit go into the deploy sheet and the constructor record.
- **Not the mock:** the SP1 verifier is Succinct's versioned `V6_1_0_SP1_VERIFIER_GROTH16` at `0xb69f2584CBcFf99a58C4e7002E8b89Af54a6f4e2` (sp1-contracts `v6.1.0`, `contracts/deployments/84532.json`). This command never deploys a mock SP1 verifier.
- **Untested pairing:** the lane export program is built with SP1 6.3.1. The repository's evidence that an SP1 6.3.1 Groth16 proof verifies against the v6.1.0 verifier (`docs/evidence/arc-mvp-20260828/deployments.md:107` and `conformance-test.log`) is for the Arc egress program only, not for the lane export program. One real proof of the lane export program verified against `V6_1_0_SP1_VERIFIER_GROTH16` is the acceptance gate of `prove-and-release`. Until that proof exists, no verifier meant to last (anything beyond a rehearsal pair) is constructed with this pairing.
- **Source verification:** Sourcify needs no API key. `crates/pft-token-contracts/foundry.toml` names `BASESCAN_API_KEY` for Basescan on chain 84532 only; whether forge 1.7.1 asks for it during a Sourcify verification was not tested. A failed verification is recorded, the run goes on, and `record` retries it.

## The one order that works

1. `--through plan`, then review `lane-genesis-pins.json`.
2. One invocation through `record` with every keystore flag and `--i-accept-no-resume-between-deploy-and-export`. After `preflight` it prints the real sheet (the real deployers, the constructor values, the funding amounts) and waits up to 24 hours. Fund the keys from that sheet, then write the sheet's sha256 (printed with it) to `confirm-deploy` in the output directory. The run then deploys, locks, waits for Base finality (up to 4 hours) and exports. Without `--proof` it stops at `prove-and-release` (exit 3) after writing nothing more.
3. Later, `--proof PROOF` on the same output directory, from the same commit: the release, then the record again.

Off the fork, `--through deploy`, `lock` and `release-on-pftl` are refused: a stop there ends the PFTL chain the contracts are bound to. The driver waits 31 hours for `base-locked.json`, longer than the confirmation, send and finality budgets together, and the command requires the driver before every transaction and in the finality wait.

## Stages

A stage runs once, and only after the stages before it (`state.json`, written atomically). Every later start requires the commit recorded at `deploy`.

1. **`plan`.** Reads the head, the gas price, the token's code hash and the SP1 verifier's code hash and `VERIFIER_HASH`. Predicts both CREATE addresses and the lock's code hash on a throwaway fork. Writes the lane pins.
   - Check: `lane-genesis-pins.json` names the predicted lock.
2. **`fleet`.** Starts the PFTL chain from those pins and takes its first finalized checkpoint. The fleet is the round-trip driver: one data directory driven by an ignored node test, with all six validators signing every block's consensus v2 commit, but no separate node processes or network.
   - Check: the verifier constructor values in `state.json`. The driver's protocol version (1) and lane cap must equal the verifier's and the lock's.
3. **`preflight`.** On a fork at the planned block, deploys both contracts at the planned nonces with the real constructor values, reads every value back, and runs one approve and Lock. Refuses unless the fork matches the predictions. Writes `deploy-sheet.json` and `deploy-sheet.md` with gas and test ETH per key.
   - Off the fork: the pause for the confirmation (see above).
4. **`deploy`.** The verifier, then the lock, at the pinned nonces. After the verifier, it requires the predicted address and the preflight's code hash before the lock is sent; if they differ, the plan is dead and the lock is not deployed. The same check runs for the lock before the stage is marked complete. Then it reads back every constructor value on the real chain, the verifier's first checkpoint, height and committee root included, and verifies both on Sourcify. `deploy` starts only with `--i-accept-no-resume-between-deploy-and-export` (see Recovery).
   - Check: the addresses equal the sheet's.
5. **`lock`.** The holder approves and Locks 1,000,000 units to the PFTL recipient, each at a recorded nonce.
6. **`release-on-pftl`.** The first wait: Base finality of the Lock's block, about an hour on 2026-11-06, at most 4 hours. Then the four checkpoint commands read Base Sepolia, and the release lands on the PFTL chain.
   - Check: the recipient holds 1,000,000 atoms.
7. **`export`.** Exports 400,000 atoms to a fresh Base address, then builds the proof input with the relay.
   - Check: the export block carries the lane export root.
8. **`prove-and-release`.** Without `--proof` it stops. It prints the prover-command contract, the program key, the input's sha256 and the expected public-values hash. With a proof it records an intent and runs the relay's `release` with every pre-send check.
9. **`record`.** Writes the summary (with the intents) and the manifest; rewritten after a release.

Stages 2 to 7 share the fleet driver, so they run in one invocation. A crash between them restarts from a fresh output directory, but already deployed contracts cannot be redeployed at the same nonces.

## Recovery

Before every public transaction, the state file records the stage, the role, the sender, the nonce and the purpose. For approve and Lock it then records the hash as soon as cast prints it, and then the receipt; for a deployment, `forge create` returns the hash only once the transaction is mined.

On a rerun, recorded intents are reconciled before anything else:

- **By hash:** the receipt and its status. A recorded hash without a receipt may be pending: the command waits for you, it never resends.
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
