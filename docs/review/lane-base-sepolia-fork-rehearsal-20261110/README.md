# Lane Base Sepolia dry run, local fork, 2026-11-10

**This ran on a local Anvil fork of Base Sepolia. Nothing was sent to Base Sepolia, and no funded key was opened.** The fork took its state from `https://sepolia.base.org` at block 56,983,631. Every deployment, approve and Lock went to the fork, from impersonated accounts with no keys. Against the public RPC, the run made reads only: `eth_blockNumber`, `eth_gasPrice`, `eth_getCode`, `eth_call` and `eth_getTransactionCount`, plus what Anvil reads to serve its fork.

This records one `--fork` run of `scripts/rehearse-lane-base-sepolia.py` (runbook `docs/runbooks/lane-base-sepolia-dry-run.md`). The script came from commit `c8e672672`, after the review fixes: the address and keystore checks, the committed-key rule, the intent journal and the relayer account. The `release-on-pftl` finality wait applies only off the fork. `summary.json` holds every value, `deploy-sheet.json` and `deploy-sheet.md` are the sheet, and `manifest.json` holds the hashes.

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
- **The program key:** `0x000cef5b…1414` from the committed `programs/pft-lane-export/program-info.json` (ELF `e92f2004…48ab`, source commit `439803aaa`, two reproducible runs), recorded in the deploy sheet and the constructor record.
- **The proof:** none. `prove-and-release` stopped, as it must without a real SP1 proof.

## Stages

| Stage | Result |
|---|---|
| plan | Fresh deployers `0xf1df…62d2` (verifier) and `0x5d1c…b5be` (lock), nonce 0 each. Predicted verifier `0x3ef8…300d`, lock `0x0c38…13cc`. The committed ELF matched its program info. |
| fleet | First finalized checkpoint at height 1. Verifier constructor from the genesis-owned pins, the real SP1 verifier and the committed key. |
| preflight | Both contracts landed at the predicted addresses on a separate fork; all 21 constructor values read back; one approve and Lock. Gas: verifier 2,630,551; lock 1,134,485; approve 46,331; Lock 154,401. |
| deploy | Gas price checked against Base Sepolia's own. Intents recorded first; the verifier landed at its predicted address with the preflight's code hash before the lock was sent; then the lock, checked the same way. Sourcify skipped on the fork. |
| lock | Approve and Lock sent asynchronously, each hash recorded before its receipt; 1,000,000 units locked (fork block 56,983,635, Lock nonce 0). |
| release-on-pftl | Five checkpoint votes; release in PFTL block 2; the recipient holds 1,000,000 atoms. |
| export | 400,000 atoms in PFTL block 4, which carries the export root. Proof input sha256 `e02b9808…aace`; expected public values sha256 `6f10001e…9c9f`. |
| prove-and-release | Stopped (exit 3). It printed the prover-command contract, the program key, the input's sha256 and the public-values hash. |
| record | Summary and manifest. |

**Test ETH per key** at the gas price read (0.006 gwei), with a 3x margin:

- verifier deployer: 0.000047350 ETH;
- lock deployer: 0.000020421 ETH;
- holder: 0.000004747 ETH, including its three funding transfers of 21,000 gas each, to the two deployers and the relayer;
- relayer: not estimated, because a release needs a real proof.

**Intent journal** (`state.json`, not published, since it names the local data directory): four intents were recorded before sending — deploy/verifier, deploy/lock, lock/approve and lock/lock. All four were sent with status `0x1`.
