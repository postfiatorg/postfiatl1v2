# Lane Base Sepolia dry run, local fork, rerun 2026-11-11

**This ran on a local Anvil fork of Base Sepolia. Nothing was sent to Base Sepolia, and no keystore was opened.** The fork took its state from `https://sepolia.base.org` at block 57,332,555. Every deployment, approve and Lock went to the fork, from impersonated accounts with no keys. Against the public RPC the run made reads only (`eth_blockNumber`, `eth_gasPrice`, `eth_getCode`, `eth_call`, `eth_getTransactionCount`), plus what Anvil reads to serve its fork.

The dry-run lock on Base Sepolia is bound to a PFTL chain whose committee keys come from fixed seeds in the repository, so these two contracts are a rehearsal and must never be named as the lane's lock.

This replaces the 2026-11-10 record. It records `--fork` runs of `scripts/rehearse-lane-base-sepolia.py` (runbook `docs/runbooks/lane-base-sepolia-dry-run.md`) as of commit `5d3128cc6` (unchanged by `7ee267440`), with Foundry 1.7.1. `summary.json` holds the values, `deploy-sheet.json` and `deploy-sheet.md` are the sheet, and `manifest.json` holds the hashes.

## What changed since 2026-11-10

- **The driver no longer gives up after 20 minutes.** Its handshake timeout comes from `driver-config.json`: 111,600 s (31 hours), above the confirmation, send and finality budgets together. The command requires the driver before every transaction and inside the finality wait.
- **Checks before the first transaction:** the holder and relayer keystores, ETH and token balances at the current gas price, the node binary, the driver, chain id 84532, and deployers that differ from the holder. After the Lock, its event must name the holder.
- **On the real chain:** all 21 constructor values are read back before the Lock, the verifier's first checkpoint, height and committee root included. The gas guard is 3x, the sheet's margin, and runs again before approve and Lock.
- **Recovery:** `state.json` is written atomically. Approve and Lock record their nonce. A pending hash is never resent.
- **Off the fork:** the run pauses after `preflight` for a confirmation of the real sheet.
- **The relay:** placeholder proofs go only to a loopback Anvil with the pinned mock SP1 verifier, and the relay checks the decoded public values it sends.

## The A1 path, exercised on the fork

`--fork-finality-wait 90` stands in for Base finality: the run waits 90 s before writing `base-locked.json`, as a real run waits for finality.

| Run | Driver timeout | Result |
|---|---|---|
| Old failure | `--fork-driver-timeout 75` | The driver panicked: "the rehearsal did not write base-locked.json within 75 s". The command stopped inside the wait: "the PFTL fleet driver exited before Base finality of the Lock block (the fork's stand-in wait); nothing more is sent". The stages `plan` through `lock` had completed. |
| With the fix | 111,600 s (default) | The 90 s wait passed, and the driver took `base-locked.json`. Five checkpoint votes, release in PFTL block 2; the recipient holds 1,000,000 atoms. |

## Stages of the run with the fix

| Stage | Result |
|---|---|
| plan | Fresh deployers, nonce 0 each. Predicted verifier `0x500a…7cf9`, lock `0xb902…4cc0`. The committed ELF matched its program info. |
| fleet | First finalized checkpoint at height 1. The driver reported protocol version 1, lane cap 6,000,000 (the lock's `maxLockAmount`) and its 111,600 s timeout. |
| preflight | Both contracts at the predicted addresses on a separate fork; 21 constructor values read back; one approve and Lock. Gas: verifier 2,630,563; lock 1,134,485; approve 46,331; Lock 154,401. |
| deploy | Chain id and gas price checked; intents first; both contracts landed as predicted; 21 constructor values read back on the target fork. Sourcify skipped on the fork. |
| lock | Approve at holder nonce 6 and Lock at nonce 7, each hash recorded before its receipt; 1,000,000 units locked in fork block 57,332,559, Lock nonce 0; the event names the holder. |
| release-on-pftl | 90 s stand-in wait with the driver alive, then the release on PFTL. |

**Not run:** `export`, `prove-and-release` and `record`. They need `target/debug/postfiat-node`, which this step did not build. `summary.json` was written from the run's `state.json` with the command's own redaction. It names the stages that ran.

**Test ETH per key** at the gas price read (0.006 gwei), with a 3x margin: verifier deployer 0.000047350 ETH; lock deployer 0.000020421 ETH; holder 0.000004747 ETH, including its three funding transfers; relayer not estimated, because a release needs a real proof.
