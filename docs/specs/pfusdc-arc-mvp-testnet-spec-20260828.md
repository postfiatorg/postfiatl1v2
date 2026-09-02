# pfUSDC on Arc Testnet: MVP Build Specification

**Date:** 2026-08-28
**Target:** one complete proof-verified USDC round trip on **Arc Testnet (chain 5042002, live today)** against PFTL devnet. Deposit mints against a proof of Arc finality; burn redeems against the existing proof of PFTL finality. Conservation exact to the atom.
**Explicitly out of scope:** mainnet (5042), audits, agent-settlement demo, grant mechanics, CCTP/Paymaster/EURC, uncapped funds. This MVP spends only faucet USDC.
**Code base:** `postfiatl1v2`, new worktree/branch `arc-tier4-mvp-20260828` cut from the Tier-4 state in `a666-eth-fast-lane-combined-20260724` (contains `ERC20BridgeVaultV2.sol`, `PFTLFinalityVerifierV1.sol`, `PfUsdcIngressAnchorV1.sol`, `programs/pfusdc-egress`, `programs/pfusdc-ingress`, `tools/pfusdc-tier4-prover`). That worktree is evidence-bearing; work happens on the new branch, never by mutating it.
**Evidence root:** `docs/evidence/arc-mvp-20260828/` in the working branch. Every gate below names its artifact.

**Fixed facts (verified 2026-08-28):** RPC `https://rpc.testnet.arc.network`, `eth_chainId = 0x4cef52`; USDC system contract `0x3600000000000000000000000000000000000000`, 6 decimals, gas in USDC; faucet `faucet.circle.com`; explorer `testnet.arcscan.app`; consensus signing Ed25519 (`arc-malachitebft-signing-ed25519`); headers expose `receiptsRoot`.

---

## [ ] Workstream 1 — Arc conformance probes (no circuit work until this closes)

### [ ] To-do 1.1 — Pin arc-node and extract the commit structure
Clone/pin `external/arc-node` at one commit hash recorded in the evidence root. From source, document: the vote/commit preimage (encoding, domain separation, height/round binding), the validator-set representation, and the **commit-to-execution binding** (the exact chain from the value validators sign to the EVM header carrying `receiptsRoot`). Output: `docs/evidence/arc-mvp-20260828/arc-commit-structure.md` with struct layouts and the pinned hash.
**Accept when:** a reviewer can reconstruct the signed preimage for an arbitrary block from the doc alone.

### [ ] To-do 1.2 — Golden-vector commit verification (offline, Rust)
New test crate `crates/arc-conformance`: fetch one recent Arc testnet block plus its commit via RPC, rebuild the preimage per 1.1, verify every Ed25519 signature and the 2/3 voting-power quorum, and verify the binding from the signed value to the header's `receiptsRoot`. No zkVM involvement.
**Accept when:** `cargo test -p arc-conformance golden_commit` passes against two different live blocks and fails when any byte of the preimage is perturbed. Artifact: test log + the two block fixtures committed as JSON.

### [ ] To-do 1.3 — Receipt inclusion conformance
Using the existing Stage-4 MPT walker: fetch a live testnet transaction receipt, build the Merkle-Patricia inclusion proof, verify under the header `receiptsRoot`. Must cover a typed (EIP-2718) receipt and a legacy receipt if both exist on Arc.
**Accept when:** verification passes for two live transactions and fails on a mutated receipt. Artifact: fixtures + test log.

### [ ] To-do 1.4 — Precompile and gas probe
Deploy a probe contract exercising `sha256` (0x02) and BN254 add/mul/pairing (0x06/0x07/0x08) on Arc testnet; record existence, correctness against known vectors, and gas.
**Accept when:** all four behave per EVM spec; measured gas table lands in `precompile-gas.md`. If any is absent, **stop: MVP is blocked and the finding escalates immediately.**

### [ ] To-do 1.5 — USDC system-contract conformance
From a faucet-funded EOA: `approve`, `transferFrom`, `Transfer` log emission, decimals, and behavior under a vault-style pull. Note faucet rate limits for sustained testing.
**Accept when:** the vault's expectations (standard ERC-20 pull semantics, 6-decimal amounts, standard log topics) are confirmed in `usdc-conformance.md`.

**GATE G0:** all five to-dos checked, artifacts in the evidence root. Circuit work may begin.

---

## [ ] Workstream 2 — Contracts live on Arc testnet

### [ ] To-do 2.1 — Deploy vault and ingress anchor
`forge create` `ERC20BridgeVaultV2` (`token = 0x3600…0000`) and `PfUsdcIngressAnchorV1` to chain 5042002; verify source on Arcscan. Record addresses and deploy txs in `deployments.md`.
**Accept when:** contracts verified on explorer; a test deposit emits the expected `Deposit` log.

### [ ] To-do 2.2 — Byte-identical evidence derivation
Run one real deposit; assemble `VaultBridgeDepositEvidence` with `source_domain = erc20_bridge_vault:5042002:<vault>:<usdc>` and `finality_ref = evm_log:5042002:<block_hash>:<tx_hash>:<log_index>`; derive `deposit_id` through `vault_bridge_deposit_id()`.
**Accept when:** the PFTL-side derivation reproduces the deposit id byte-for-byte from raw Arc log data. Artifact: the evidence row JSON.

### [ ] To-do 2.3 — Deploy the egress verification stack
Deploy the SP1 Groth16 verifier gateway and `PFTLFinalityVerifierV1` to Arc testnet. Checkpoint ceremony: initialize from a genuinely finalized PFTL devnet block; record the ceremony transcript.
**Accept when:** contracts verified; checkpoint state readable and matching the PFTL devnet block. Artifact: ceremony transcript.

### [ ] To-do 2.4 — Existing egress proof verifies on Arc
Produce a fresh egress proof with `programs/pfusdc-egress`. **vkey discipline (h544 lesson): print the vkey from the exact fresh ELF, compare its SHA-256 against the deployed verifier's pinned key, and record both.** Submit the proof to the Arc-deployed verifier.
**Accept when:** on-chain verification succeeds on Arc testnet; measured USDC gas cost recorded in `egress-verify-cost.md`.

**GATE G1:** vault accepting deposits, evidence derivation exact, PFTL finality proof verified on Arc. Artifact set complete.

---

## [ ] Workstream 3 — `pfusdc-arc-ingress` circuit

### [ ] To-do 3.1 — Guest program
New `programs/pfusdc-arc-ingress` implementing, in-circuit: (1) Ed25519 quorum verification over the commit per the 1.1 structure (SP1 Ed25519 precompile, version pinned); (2) commit-to-execution binding, every intermediate commitment opened; (3) receipt MPT walk under `receiptsRoot` (reuse existing walker code); (4) `Deposit` log equality reproducing `deposit_id`; (5) validator-set rotation as an explicit transition committing `validator_set_commitment_out`. Public inputs exactly as the Tier-4 spec §A4.
**Accept when:** guest proves a real testnet deposit fixture in the SP1 executor; unit tests cover each obligation independently.

### [ ] To-do 3.2 — Witness builder
Extend `tools/pfusdc-tier4-prover` with an Arc witness path: fetch block, commit, validator set, receipt proof from RPC; emit the guest input.
**Accept when:** one command produces a proving-ready witness from a deposit tx hash.

### [ ] To-do 3.3 — vkey pin and PFTL route registration
Generate and pin the ingress vkey (same fresh-ELF discipline as 2.4). Register the Arc route/proof profile on PFTL devnet for the 5042002 source domain.
**Accept when:** PFTL devnet admits a valid proof and mints; the pinned vkey and registration tx are recorded.

### [ ] To-do 3.4 — Negative suite
Corrupted witnesses must all reject with documented error codes: forged signature, sub-quorum signing power, mutated receipt, wrong log fields, replayed `deposit_id`, stale validator-set commitment.
**Accept when:** all six rejections reproduce in CI with logged error codes.

### [ ] To-do 3.5 — Proving benchmark
Measure end-to-end ingress proving on the GPU prover. Target under 60 GPU-seconds; hard ceiling 3 minutes (parity with egress).
**Accept when:** measured number published in `ingress-benchmark.md`; ceiling breach is a stop-and-review, not a waiver.

**GATE G2:** valid deposits prove and mint on devnet; invalid ones cannot; cost is measured.

---

## [ ] Workstream 4 — The round trip (MVP exit)

### [ ] To-do 4.1 — Ingress end to end
Faucet USDC → vault deposit on Arc → witness → proof → PFTL devnet mint. Record deposit-to-mint wall time (target ≤ 120 s).

### [ ] To-do 4.2 — Egress end to end
Burn the minted pfUSDC → egress proof → Arc verifier → vault release to the withdrawal address. Record burn-to-release wall time (target ≤ 5 min) and confirm the nullifier is consumed (a resubmission must fail).

### [ ] To-do 4.3 — Conservation gate
The round trip conserves **exactly 1.000000 USDC**; PFTL invariant `issued == Σcounted − Σredeemed` holds before, during, and after; vault balance equals counted outstanding capacity at every step.

### [ ] To-do 4.4 — Evidence pack and runbook
`docs/evidence/arc-mvp-20260828/round-trip/`: all tx hashes (Arc + PFTL), proofs, vkeys, timings, gas costs in USDC, and a runbook that lets a second operator repeat the trip unassisted.

**GATE G3 — MVP complete:** 4.1–4.4 checked. The claim earned, and the only claim earned: *both directions of a pfUSDC round trip verified by proofs on Arc testnet, no committee, no downgrade path.* Mainnet, audits, and everything else remain gated outside this document.

---

## Dependencies and stop conditions

- G0 blocks G2 (commit structure before circuits). G1 and G2 can proceed in parallel after G0. G3 requires both.
- **Stop conditions:** missing BN254 precompiles (1.4); commit binding unresolvable from arc-node source (1.1); SP1 Ed25519 precompile unavailable in the pinned SP1 version (3.1). Each halts the affected workstream and escalates the finding same-day rather than working around it.
- Faucet limits (1.5) size how many round trips CI can run; the MVP requires only single-digit trips.
