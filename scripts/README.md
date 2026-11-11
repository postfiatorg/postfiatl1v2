# Scripts Directory

This directory keeps only core operational scripts. One-off smoke tests,
generated verification harnesses, and historical release probes were archived
outside the repository under `~/repos/postfiat-archive/postfiatl1v2/`.

## Retained Scripts

| Script | Description |
| --- | --- |
| `check` | Runs Rust formatting, workspace compilation, and shell syntax checks for retained scripts. |
| `node-init` | Initializes a single local `postfiat-node` data directory. |
| `node-run` | Runs a single local node from `DATA_DIR`. |
| `node-status` | Prints status for a single local node. |
| `node-faucet` | Runs the node faucet command for the selected local node. See `docs/runbooks/faucet-testnet-transactions.md` for the full faucet send flow. |
| `node-transfer` | Creates a simple transfer from the selected local node. |
| `node-account` | Reads account state for an address from the selected local node. |
| `node-helper-smoke` | Runs the single-node helper path end-to-end and verifies the devnet storage acknowledgement. |
| `pftl-transfer.py` | One-command Python wrapper for `request_faucet_pft` and `send_pft` with env/flag-driven defaults. |
| `devnet-up` | Builds and initializes a local multi-validator devnet. |
| `devnet-submit-transfer` | Submits and applies a transfer batch across the local devnet. |
| `devnet-cobalt-transition` | Exercises a local Cobalt validator-set transition batch. |
| `devnet-restart` | Restarts local devnet validators from existing data directories. |
| `devnet-status` | Checks local devnet validator height, root, and status convergence. |
| `devnet-down` | Removes local devnet data. |
| `devnet-helper-smoke` | Runs the local multi-validator devnet path end-to-end (`devnet-up`, `devnet-submit-transfer`, `devnet-restart`) on two validators and checks convergence. |
| `wan-devnet-transaction-preflight` | Read-only WAN transaction gate: checks validator RPC reachability, height/root convergence, mempool emptiness, finality capability, and optional SSH process/binary inventory before wallet or Python transaction tests run. |
| `wan-devnet-transaction-matrix` | Read-only Stage 7 matrix command: records current accepted evidence, wallet-facing read probes, disabled write surfaces, and open transaction categories without guessing schemas or mutating state. |
| `wan-devnet-latency-run` | Stage 8 latency evidence runner: gates on fleet health, then measures wallet-facing native PFT, memo `payment_v2`, and FastPay proxy-cycle latency into raw JSONL plus summary reports. |
| `wan-devnet-state-sync` | Fast controlled-WAN repair for a lagging validator: majority-checks the fleet, backs up the target, copies replicated state from a healthy validator, preserves local keys/topology, and restarts services. Use this instead of `rpc-catch-up` when historical proof-heavy blocks would replay slowly. |
| `testnet-readiness-gate` | Runs the local controlled-testnet readiness gate. |
| `testnet-p0-network-gate` | Runs the P0 network readiness gate for testnet infrastructure. |
| `testnet-node-run-peer-certified-smoke` | Runs the peer-certified batch smoke test. |
| `testnet-wallet-minimum-smoke` | Runs the minimum wallet flow smoke test. |
| `repro-transfer-fee-quote-pending-recipient` | Regression check for issue #193: quotes and submits a transfer to a recipient a pending transfer creates, then checks the quote, the declared fee and the receipt agree. Exits 0 on main, 1 on a regression; `--control` runs the empty-mempool case. See `repro-transfer-fee-quote-pending-recipient.md`. |
| `rehearse-phase3-deployment-order.py` | Rehearses the Phase 3 deployment order on a local Anvil (chain id 84532, disposable keys): predicts the verifier and lock addresses from two dedicated deployers and their code hashes, pins the lock in a lane genesis read by `postfiat-node init`, deploys at the planned nonces, reads back every constructor value, runs one Lock and refuses a deployer nonce off by one. See `rehearse-phase3-deployment-order.md`. |
| `pft-lane-export-relay.py` | Takes a lane export from PFTL to the Base lock: `input` (the proof input once the block is final), `prove` (execute-only placeholder or an external prover), `release` (checks every readable revert, then sends; off a loopback Anvil it needs `--gas-limit` and `--max-fee-per-gas`), `advance-checkpoint` and `status`. |
| `rehearse-lane-roundtrip.py` | One local round trip of PFT: a Lock on Anvil released on a local PFTL chain, exported back and paid out of the same lock through the real verifier with a mock SP1 verifier. Record `docs/review/lane-roundtrip-rehearsal-20261110/`. |
| `rehearse-lane-base-sepolia.py` | The Phase 3 deployment-order dry run on Base Sepolia as one resumable command; `--fork` runs it on a local Anvil fork and sends nothing. Runbook `docs/runbooks/lane-base-sepolia-dry-run.md`. |
