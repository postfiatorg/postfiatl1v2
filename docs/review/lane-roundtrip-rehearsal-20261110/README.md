# Lane round-trip rehearsal, local chains, 2026-11-10

**What was real:** the contracts `PFTLockV1` and `PFTLFinalityVerifierV1`, deployed from source on chain. The PFTL chain was generated with six validators, and all six signed the consensus v2 finality of every block. The Lock receipt path ran as observe, five checkpoint signatures, assemble, prove and release. The exports, the lane export root, the witness check `verify_external_lane_export_witness_v1` and the guest `programs/pft-lane-export` ran in SP1 execute mode.

**What was NOT real:** the SP1 proof. The verifier was built with the Foundry tests' mock SP1 verifier (`Tier4MockSP1Verifier`, `crates/ethereum-contracts/test/PFUSDCTier4.t.sol`) and a placeholder program key. It accepted the relay's placeholder bytes, and no proof was generated. A local Anvil with chain id 84532 stood in for Base. The guest ELF was a local SP1 6.3.1 build without a committed key. The "six-validator fleet" was one data directory driven by an ignored node test: every block carries consensus v2 commits from all six validators, but no separate node processes or network ran. The exports were applied by that driver, not submitted through a running node's RPC.

This records one run of `scripts/rehearse-lane-roundtrip.py`, which uses the relay `scripts/pft-lane-export-relay.py`. No public network was touched, and no request went to a prover network. `summary.json` holds every value below. `manifest.json` holds the source and tool hashes, the ELF's sha256 and the hashes of the unpublished run files. No key material is recorded.

- **Source:** commit `cf06ff863dcabebcb6393e426328f26493a6254a`, tree `84809aa257f941aac679599d5d45317744b641a2`.
- **Tools:** Foundry 1.8.1 (anvil, forge, cast), Rust 1.95.0.
- **ELF:** sha256 `13dd27f5b521259268fce63cf93df6d0b3d5fbd92da51e8f29818475b4e30b7e`, a local test build.

## The run

| Step | PFTL | Base (Anvil) |
|---|---|---|
| Setup | Lane genesis pins the predicted lock. Chain `postfiat-lane-lane-roundtrip`, six validators, consensus v2 and the export root from block 2. First checkpoint: block 1. | Token, mock SP1 verifier, then the verifier `0x01a63a3b…6430` and the lock `0x165d84d9…2001` at their predicted addresses. All 21 constructor values read back. |
| (a) Lock | | 1,000,000 locked to the PFTL recipient (block 6). `totalLocked` 1,000,000; lock balance 1,000,000. |
| (b) Release | Block 2. The recipient holds 1,000,000 atoms. Lane released 1,000,000, remainder 299,000,000. | unchanged |
| (c)–(d) Export 400,000 | Block 4 carries the lane export root. Lane exported 400,000, remainder 299,400,000. | The relay paid 400,000 to `0xfb012590…f93b` (block 135). `totalReleased` 400,000, lock balance 600,000. `Released` names the packet digest. Guest: 76,727,816 instructions. |
| (e) Same packet again | | Refused before sending (`already_consumed`). Forced, it reverted on chain. State unchanged. |
| (f) Export 600,000 | Block 5. Lane released 1,000,000, exported 1,000,000, remainder 300,000,000. | Paid 600,000 to `0x74cd441b…49aa` (block 136). `totalLocked - totalReleased` 0, lock balance 0. Guest: 48,863,644 instructions. |
| (g) Export 1 atom | Refused at admission: `external_lane_export_above_released`. | |
| (i) Watcher | One release, checked, `backed`, no alarm. | Its Lock found on Base. |

**Conservation.** After every step on PFTL, the blocks and state replayed, and the lane remainder equalled `reserved - (released - exported)`. The native total of accounts plus lane remainder moved only by burned fees: 1,000 for the release, 22 for the fee transfer, and 1,000 for each export. On Base, `token.balanceOf(lock) == totalLocked - totalReleased` held at every step, and so did `totalLocked - totalReleased == released - exported` on the lane.

**Not shown here.** The watcher has no export view and reports nothing about exports. The recipient got 10,000 native atoms from the faucet to pay the two export fees. The PFTL side ran inside the ignored node test `local_evm_lane_roundtrip`, which the script drives step by step through files. The exports were signed with `postfiat-node wallet-sign-asset-transaction --operation-json`, and the relay's `input` ran `postfiat-node external-lane-export-proof-input`. `advance-checkpoint` and `prove --mode command` were not run.

## Reproduce

```sh
cargo build -p postfiat-node --bin postfiat-node
cargo build --release --manifest-path tools/pft-lane-export-prover/Cargo.toml
# a local guest build: cargo prove build in programs/pft-lane-export (SP1 6.3.1)
python3 scripts/rehearse-lane-roundtrip.py --output OUT --elf ELF \
  --prover-tool target/release/pft-lane-export-prover --node-bin target/debug/postfiat-node
```
