# Lane Base Sepolia dry run, local fork, full rerun 2026-11-11

**This ran on a local Anvil fork of Base Sepolia. Nothing was sent to Base Sepolia, and no keystore was opened.** The fork took its state from `https://sepolia.base.org` at block 57,336,909. Every deployment, approve and Lock went to the fork, from impersonated accounts with no keys. Against the public RPC the run made reads only (`eth_blockNumber`, `eth_gasPrice`, `eth_getCode`, `eth_call`, `eth_getTransactionCount`), plus what Anvil reads to serve its fork.

The dry-run lock on Base Sepolia is bound to a PFTL chain whose committee keys come from fixed seeds in the repository, so these two contracts are a rehearsal and must never be named as the lane's lock.

This replaces the 2026-11-10 record. It records one `--fork` run of `scripts/rehearse-lane-base-sepolia.py` through `record` (runbook `docs/runbooks/lane-base-sepolia-dry-run.md`), with Foundry 1.7.1. The command was the one at commit `0fd0e5967`. `summary.json` and `manifest.json` are what the `record` stage wrote; its `source_commit` names the checkout at record time, `d6be31932`, which changed only the relay's send-failure report and the round trip. `deploy-sheet.json` and `deploy-sheet.md` are the sheet from `preflight`.

## What changed since 2026-11-10

- **The driver no longer gives up after 20 minutes.** Its handshake timeout comes from `driver-config.json`: 111,600 s (31 hours), above the confirmation, send and finality budgets together. The command requires the driver before every transaction and inside the finality wait.
- **Checks before the first transaction:** the holder and relayer keystores, ETH and token balances at the current gas price, the node binary, the driver, chain id 84532, and deployers that differ from the holder. After the Lock, its event must name the holder.
- **On the real chain:** all 21 constructor values are read back before the Lock, the verifier's first checkpoint, height and committee root included. The gas guard is 3x, the sheet's margin, and runs again before approve and Lock.
- **Recovery:** `state.json` is written atomically. Approve and Lock record their nonce. A pending hash is never resent.
- **Off the fork:** the run pauses after `preflight` for a confirmation of the real sheet.
- **The relay:** placeholder proofs go only to a loopback Anvil with the pinned mock SP1 verifier, and the relay checks the decoded public values it sends.

## The A1 path

`--fork-finality-wait 1230` stood in for Base finality: the run waited 1,230 s before writing `base-locked.json`. The driver had written `pftl-ready.json` about 1,235 s earlier, past the old fixed 20-minute (1,200 s) limit. With the 111,600 s timeout from `driver-config.json` it took the handshake and went on. A 2026-11-11 run with `--fork-driver-timeout 75` and a 90 s wait showed the old failure: the driver gave up, and the command stopped inside the wait with "nothing more is sent".

## Stages

| Stage | Result |
|---|---|
| plan | Fresh deployers, nonce 0 each. Predicted verifier `0x305175cc…7e1a`, lock `0x4cba0581…f8f8`. The committed ELF matched its program info. |
| fleet | First finalized checkpoint at height 1. The driver reported protocol version 1, lane cap 6,000,000 (the lock's `maxLockAmount`) and its 111,600 s timeout. |
| preflight | Both contracts at the predicted addresses on a separate fork; 21 constructor values read back; one approve and Lock. Gas: verifier 2,630,563; lock 1,134,485; approve 46,331; Lock 154,401. |
| deploy | Chain id and gas price checked; intents first; both contracts landed as predicted; 21 constructor values read back on the target fork. Sourcify skipped on the fork. |
| lock | Approve at holder nonce 6 and Lock at nonce 7, each hash recorded before its receipt; 1,000,000 units locked in fork block 57,336,913, Lock nonce 0; the event names the holder. |
| release-on-pftl | 1,230 s stand-in wait with the driver alive; five checkpoint votes; release in PFTL block 2; the recipient holds 1,000,000 atoms. |
| export | 400,000 atoms in PFTL block 4, which carries the export root; lane remainder 299,400,000. Proof input sha256 `aa2c9d7e…727f`; expected public values sha256 `91b95082…88ac`. |
| prove-and-release | Stopped (exit 3), as it must without a real SP1 proof. It printed the prover-command contract, the program key `0x000cef5b…1414`, the input's sha256 and the public-values hash. |
| record | Summary (with the four intents, all sent with status `0x1`) and manifest. No process of the run was left afterwards. |

**Test ETH per key** at the gas price read (0.006 gwei), with a 3x margin: verifier deployer 0.000047350 ETH; lock deployer 0.000020421 ETH; holder 0.000004747 ETH, including its three funding transfers; relayer not estimated, because a release needs a real proof. These are estimates (a Base transaction also pays an L1 data fee, which the margin covers), and the holder figure is its own gas only. This sheet predates the funding plan: a run from the current command adds the relayer's ceiling (`--relayer-ceiling-wei`), the holder's starting balance and, in the record, each key's balance after the run (runbook, Funding).
