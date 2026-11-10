# Lane Base Sepolia dry run, local fork, 2026-11-10

**This ran on a local Anvil fork of Base Sepolia. Nothing was sent to Base Sepolia, and no funded key was opened.** The fork took its state from `https://sepolia.base.org` at block 56,972,342. Every deployment, approve and Lock went to the fork, from impersonated accounts with no keys. Against the public RPC, the run made reads only: `eth_blockNumber`, `eth_gasPrice`, `eth_getCode`, `eth_call` and `eth_getTransactionCount`, plus what Anvil reads to serve its fork.

This records one `--fork` run of `scripts/rehearse-lane-base-sepolia.py` (runbook `docs/runbooks/lane-base-sepolia-dry-run.md`). The script came from commit `d5365e285d`. The `release-on-pftl` finality wait was added afterwards; it applies only off the fork. `summary.json` holds every value, `deploy-sheet.json` and `deploy-sheet.md` are the sheet, and `manifest.json` holds the hashes.

**Real:**

- `PFTLFinalityVerifierV1` and `PFTLockV1` deployed from source on the fork.
- Succinct's SP1 verifier `V6_1_0_SP1_VERIFIER_GROTH16` at `0xb69f2584CBcFf99a58C4e7002E8b89Af54a6f4e2`, as it exists on Base Sepolia:
  - source: sp1-contracts `v6.1.0`, commit `2ac5ecbb`, `contracts/deployments/84532.json`;
  - runtime code hash `0xcceb864c…04dd`;
  - `VERIFIER_HASH` `0x4388a21c…6696`, the v6.1.0 circuit SP1 6.3.1 proves for;
  - `owner()` reverts, and storage slot 0 and the EIP-1967 implementation slot are zero.
- The rehearsal token `0x90cE…acF0` with its live code, and its real holder impersonated on the fork.
- The PFTL chain with consensus v2 finality from six validators.
- The four Base checkpoint commands reading the fork, the release, the export and its proof input.

**Not real:**

- **Base Sepolia:** a fork stood in for it.
- **The PFTL "fleet":** one data directory driven by an ignored node test. Every block carries consensus v2 commits from all six validators, but no separate node processes or network ran.
- **The program key:** `0x000cef5b…1414`, taken from a `program-info.json` that holds the values reported from workflow run 42818337185. It is not committed yet.
- **The proof:** none. `prove-and-release` stopped, as it must without a real SP1 proof.

## Stages

| Stage | Result |
|---|---|
| plan | Fresh deployers `0x1310…8291` (verifier) and `0x0e2a…f4c4` (lock), nonce 0 each. Predicted verifier `0x0d17…663a`, lock `0x9cf7…878c`, lock code hash `0xa1c7…8eac`. |
| fleet | First finalized checkpoint at height 1. Verifier constructor from the genesis-owned pins, the real SP1 verifier and the program key. |
| preflight | Both contracts landed at the predicted addresses on a separate fork; all 21 constructor values read back; one approve and Lock. Gas: verifier 2,630,563; lock 1,134,485; approve 46,331; Lock 154,401. |
| deploy | Verifier, then lock, at the pinned nonces on the target fork. Code hashes equal the preflight's. Sourcify skipped on the fork. |
| lock | 1,000,000 units locked from the holder (fork block 56,972,346). |
| release-on-pftl | Five checkpoint votes; release in PFTL block 2; the recipient holds 1,000,000 atoms. |
| export | 400,000 atoms to `0x19c9…6bcb` in PFTL block 4, which carries the export root. Proof input sha256 `15728d04…809d`; expected public values sha256 `4eafc5fb…022d`. |
| prove-and-release | Stopped (exit 3). It printed the prover-command contract, the program key, the input's sha256 and the public-values hash. |
| record | Summary and manifest. |

**Test ETH per key** at the gas price read (0.006 gwei), with a 3x margin: verifier deployer 0.000047350 ETH; lock deployer 0.000020421 ETH; holder 0.000003613 ETH. A release's gas is not estimated, because it needs a real proof.
