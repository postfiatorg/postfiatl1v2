# NAVCoin external route — September 7, 2026

The resumed Ethereum → pfUSDC → A666 → Uniswap → A666 → pfUSDC → Ethereum run is **incomplete**. One new 10 USDC deposit is funded and finalized. Resume its existing lineage; do not create a replacement deposit.

## Current receipts

- [x] Derive the next pfUSDC route epoch from the full governance state: Arc holds global epoch 9, so the new Ethereum route uses epoch 10.
- [x] Deploy verifier `0xc398a26BD997168C3C966763B0A97829d14dB655` and vault `0xE7A432a28b20621A70E845C7F202004Cf98e7A2C`. Readback verifies the frozen `0015b046` program, checkpoint 1001, current committee root, route hash, token, owner, and zero initial obligations.
- [x] Register the proof profile at PFTL 1006, bind it at 1007, and activate the governed route at 1008. All six validators agree; mempools are empty.
- [x] Deposit 10.000000 USDC (10,000,000 atoms) to the epoch-10 vault for `pfab9b9228942e5c529633a13aa271d5297bec6353`. [Ethereum deposit receipt](https://etherscan.io/tx/0x639893f4c8d3df16baa392a1f958acd7549586f7b186068cd73ad5cd14806772). Wallet, vault, obligations, event, and deposit record reconcile exactly. The capture binds deposit block 25922792 to finalized Ethereum block 25922794.
- [ ] Finish the ingress proof and claim the source-series pfUSDC.
- [ ] Complete and submit the existing A666 verifier checkpoints 881→917→924→989. All three canonical witnesses execute with the frozen `004e44` guest; no completed proof is claimed yet.
- [ ] Authorize the new settlement source, subscribe at NAV, export, prove, and mint wA666.
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

The original host's durable job and receipt directory is `~/.local/share/stakehub/a666-full-route-20260907/`. Its `active-resume.json` points to the current jobs and immutable transaction records. The private StakeHub companion archive contains a receipt snapshot in `docs/handoffs/navcoin-recovery-20260907/resumed-epoch10/`. Local paths and loopback endpoints require host-specific configuration.

The empty epoch-7 contracts remain invalid and must never be funded. The old epoch-6 verifier's committee-transition incompatibility is not claimed fixed: the fresh epoch-10 verifier starts after those rotations. A complete withdrawal proof is still required. The historical Cobalt publication-binding defect and the paused private-funding objective are unchanged.
