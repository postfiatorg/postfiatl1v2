# NAVCoin external route — September 7, 2026

The resumed Ethereum → pfUSDC → A666 → Uniswap → A666 → pfUSDC → Ethereum run is **incomplete**. One new 10 USDC deposit is funded and finalized. Resume its existing lineage; do not create a replacement deposit.

## Current receipts

- [x] Derive the next pfUSDC route epoch from the full governance state: Arc holds global epoch 9, so the new Ethereum route uses epoch 10.
- [x] Deploy verifier `0xc398a26BD997168C3C966763B0A97829d14dB655` and vault `0xE7A432a28b20621A70E845C7F202004Cf98e7A2C`. Readback verifies the frozen `0015b046` program, checkpoint 1001, current committee root, route hash, token, owner, and zero initial obligations.
- [x] Register the proof profile at PFTL 1006, bind it at 1007, and activate the governed route at 1008. All six validators agree; mempools are empty.
- [x] Deposit 10.000000 USDC (10,000,000 atoms) to the epoch-10 vault for `pfab9b9228942e5c529633a13aa271d5297bec6353`. [Ethereum deposit receipt](https://etherscan.io/tx/0x639893f4c8d3df16baa392a1f958acd7549586f7b186068cd73ad5cd14806772). Wallet, vault, obligations, event, and deposit record reconcile exactly. The capture binds deposit block 25922792 to finalized Ethereum block 25922794.
- [x] Complete the CPU ingress Groth16 proof in 1471.57 seconds and claim exactly 10 source-series pfUSDC at PFTL 1011. The claim binds governed route epoch 10. Source asset: `2bae082a6703375b9405af44715e1e64623265392627767b040fa2c30abb100a09da403724a6f105317292d9c0073df7`.
- [x] Complete and submit A666 verifier checkpoints 881→917→924→989. Both rotation segments and the later ancestry are accepted by the deployed verifier. The 917→924 proof used CPU; 881→917 and 924→989 used an A100 with the same frozen `004e44` guest.
- [x] Authorize the source at 1012, reserve at 1013, subscribe at NAV at 1014, and export 9.611565 A666 at 1015. The exact export witness is captured; pre-existing holder inventories are unchanged.
- [x] Prove the accepted export and mint exactly 9.611565 wA666. [Ethereum mint receipt](https://etherscan.io/tx/0x08b001efba5fd3ddabc084d1080107189891654ba1cbfc52559b0c4f4ccaa9fe). The original 103 wA666 remain unchanged. The export proof took about 182 seconds on the A100 and passed an independent deployed SP1 verifier call.
- [ ] Execute both Uniswap directions, burn the actual buyback output, and import the return.
- [ ] Redeem at NAV into the same pfUSDC source, withdraw Ethereum USDC, settle on PFTL, and reconcile the whole lineage.

PFTL state root at 1008:

```text
aebb760941ff6126ad21615e09d3d35bb4ac34c3fcf991e4284d56f40117e9653041eda8a5d4c864407e61b6b1114327
```

The deployed node release and binary remain `a666-source-route-20260907` and `57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`. The merged handoff branch includes later source changes and is not that deployed binary.

## Execution and recovery

StakeHub's `python -m stakehub.navcoin_deposit` checks the deployed manifest against the active PFTL route before signing. It saves the deposit nonce and calldata before broadcast, then saves each agent response before waiting for its receipt. Existing output directories require reconciliation rather than a second deposit. `python -m stakehub.navcoin_checkpoint` verifies checkpoint public values and simulates the proof against the deployed Ethereum verifier before sending through the unlocked agent.

The issuance and redemption builders accept `--settlement-source-asset-id`. Source selection belongs to the signed reservation/redemption operation; the existing mint packet retains its canonical family asset field. The source-custody consensus implementation is unchanged by these CLI additions.

For each Uniswap direction, `a666-mainnet-uniswap-allowances.py --token … --amount-atoms …` can authorize only that trade's input amount at both the ERC-20 and Permit2 layers. The swap runner requires an evidence output path for execution. Both scripts persist intent before calling StakeHub and save any returned transaction hash before receipt processing; an existing journal requires reconciliation. Six focused tests cover exact one-atom inputs, preserving unrelated balances/allowances, expiration bounds, positive minimum output, and an interrupted signer response without a second send. Actual gas accounting uses receipt gas consumption and effective gas price, not the signer's budget metadata.

The captured ingress witness also passed all 16 native adversarial rejection cases. The completed Groth16 proof additionally passed local verification; the remaining live route legs are still pending.

The after-mint wrapper carries the issue manifest's source-series asset into NAV redemption and resolves its bucket explicitly for withdrawal. Its two swap calls use the durable output interface and exact input approvals. The CPU egress wrapper verifies the embedded program's vkey and ELF hash before a native burn, checks Docker access, and runs the supported `egress` command with bounded worker settings. These command-path repairs are not evidence that the pending trade or withdrawal has executed.

Return preflight found that the original PublicNode endpoint rejects older numbered contract-state queries without an archive token. The resumed workflow now has a separate loopback proxy at `http://127.0.0.1:28703` on all six validators, forwarding to `https://eth.drpc.org`; set `A666_VALIDATOR_ETHEREUM_RPC` to that loopback URL. The native observer verified historical block 25922792, its receipt root, and the governed contract code hashes. Five validators signed and the native node assembled a valid checkpoint certificate under the existing five-of-six policy. Validator 5's current key differs from this older Ethereum route committee and is excluded. This was a preflight checkpoint, not a return import; the actual burn receipt still needs its own certificate. The return verification class remains `BFT_CHECKPOINT`.

The original host's durable job and receipt directory is `~/.local/share/stakehub/a666-full-route-20260907/`. Its `active-resume.json` points to the current jobs and immutable transaction records. The private StakeHub companion archive contains a receipt snapshot in `docs/handoffs/navcoin-recovery-20260907/resumed-epoch10/`. Local paths and loopback endpoints require host-specific configuration.

The empty epoch-7 contracts remain invalid and must never be funded. The old epoch-6 verifier's committee-transition incompatibility is not claimed fixed: the fresh epoch-10 verifier starts after those rotations. A complete withdrawal proof is still required. The historical Cobalt publication-binding defect and the paused private-funding objective are unchanged.
