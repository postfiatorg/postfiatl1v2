# pfUSDC on Arc: Launch-Partner Proposal for the Arc Ecosystem Program

**Date:** 2026-08-28
**Applicant:** Post Fiat (postfiat.org). Open source at github.com/postfiatorg/postfiatl1v2.
**Ask:** $250,000 across four milestone-locked tranches. Tranches pay on delivered gates; dates are targets and gates govern.
**One line:** A shipped, proof-verified stablecoin bridge brings audited USDC reserves, recurring settlement flow, and a live AI-agent economy to Arc at public mainnet, with every claim anchored to running code rather than promises.

---

## Part I. Commercial proposal

### 1. What Arc gets

**Locked USDC from month one.** Post Fiat operates pfUSDC, a USDC-backed settlement asset on the Post Fiat L1 (PFTL) whose reserves sit in an on-chain vault that releases funds only against cryptographic proofs. We will deploy that vault to Arc and hold an initial reserve pilot of **$500,000 USDC on Arc**, capped by protocol governance and activated only after the contract audit in M3 clears. The target window is within 30 days of the September 16 public mainnet launch, and the audit gate governs if the two conflict. Contingent on audit completion and chain stability, our stated intent is to migrate the majority reserve share from Ethereum to Arc, a path to **$5M+ of durable, verifiable TVL by Q1 2027**. Reserve migration is a governance decision Post Fiat controls, so this number depends on our own gates rather than third-party adoption.

**Recurring settlement flow, priced in USDC.** pfUSDC is the settlement leg for Post Fiat NAVCoin subscription and redemption cycles, so vault traffic recurs by construction. The mainnet pilot targets a standing cadence of **10+ deposit/redemption round trips per week** from live NAV operations, each a real USDC transaction on Arc with gas paid in USDC. Arc receives transaction flow tied to asset-management activity rather than incentive farming.

**A live agent economy that settles to Arc.** Circle positions Arc for agentic commerce. Post Fiat runs one today: autonomous agents earn task rewards, operate research and trading pipelines, and settle value on PFTL. This proposal makes Arc the dollar rail underneath that economy. Milestone M3 includes a public demonstration in which an AI agent completes paid work and settles its earnings to USDC on Arc through pfUSDC, end to end, with sub-second Arc finality. To our knowledge this would be the first live agent economy settling to Arc at launch.

**Two launch-narrative firsts, offered for co-marketing.**
1. **The first bridge on Arc secured by proofs rather than a committee.** Both directions verify succinct zero-knowledge finality proofs on-chain. There is no multisig, no attestation layer, and no downgrade path to a signer. Bridge committees are the layer behind most of the roughly $2.8B of bridge losses since 2022; Arc's flagship early integrations should have none.
2. **The first post-quantum consensus verified in zero knowledge settling to Arc.** PFTL validators sign with ML-DSA (FIPS-204), and our withdrawal proof verifies those lattice signatures inside an SP1 zkVM, wrapped to a constant-size Groth16 proof an Arc contract checks. Arc's founding validators include institutions with active quantum-risk programs; a post-quantum settlement corridor is a differentiated story for exactly that audience. We commit to a joint technical post, reproducible verification artifacts, and full open source.

**Standards alignment that preserves Circle's optionality.**
- **Bridged USDC Standard:** pfUSDC conforms, preserving the path for the bridged representation to be superseded by native USDC issuance on PFTL. We are building a conforming on-ramp for USDC distribution, not a competing wrapper.
- **CCTP:** redemption fan-out routes exits through CCTP where available, so USDC leaving our vault moves natively to any supported domain.
- **Arc-native operations:** Paymaster for gas UX, Arcscan source verification, transparent-path transfers for all vault activity. A EURC extension is a natural sequel given our shipped private-FX work, opening a tokenized FX corridor on Arc.

**Institutional posture.** Vault deposits use the transparent path only. The single privileged control is pause; no party can move, mint, or redirect locked funds. Third-party audits of the contracts and both zkVM programs are scheduled before any uncapped operation. Arc's permissioned, identity-bearing validator cohort is treated in our design as the finality authority it is, verified in-circuit rather than assumed.

### 2. Why this team clears the shipped-code bar

This proposal ports a system that already ran, publicly, end to end. On 2026-07-19 we published a complete round trip on a controlled testnet against Arbitrum Sepolia: a genuine USDC deposit minted as pfUSDC against a zero-knowledge proof of finalized source-chain state, then burned and withdrawn against a proof of finalized PFTL consensus, releasing **exactly 1.000000 USDC, conservation to the atom**. The withdrawal settled in transaction `0x664b2897…e702c1f9` with an accepted receipt; its proof covered a three-block segment produced in roughly three minutes of GPU time, with the proof nullifier consumed against replay. The vault, verifier contracts, zkVM programs, and the acceptance gate that validated that exact run are open source at github.com/postfiatorg/postfiatl1v2.

Arc improves this system in every dimension that hurt on the prior domain: deterministic depth-1 finality removes a multi-day ingress objection, Ed25519 commit signatures make ingress proving far lighter than the post-quantum work we already do, and USDC gas turns verification into a fixed dollar cost. The full technical treatment, including the honest trust-model deltas, is Part II.

### 3. Funded milestones, budget, and what the grant uniquely enables

We are building the Arc port regardless; the grant buys acceleration and independent assurance. Funding is requested specifically for the items a self-funded startup schedule would otherwise defer: third-party audits and a hardened, user-facing prover service.

| Milestone | Deliverable and acceptance gate | Date | Tranche |
| --- | --- | --- | --- |
| **M1: Testnet contracts live** | Vault, ingress anchor, and PFTL finality verifier deployed and source-verified on Arc testnet; an existing PFTL egress proof verifies on Arc inside the deployed verifier; deposit round-trips to a byte-identical evidence id | 2026-09-05 | $50,000 |
| **M2: Arc ingress circuit** | New Arc ingress zkVM program with pinned verifying key; measured proving time published (target under 60 GPU-seconds); corrupted-witness suite rejected with documented errors | 2026-09-12 | $50,000 |
| **M3: Testnet round trip + contract audit** | Full round trip on Arc testnet mirroring the July demonstration (conservation exact, both proofs on-chain, corrupted-witness suite rejected); third-party audit of the Solidity contracts delivered and remediated | 2026-09-15 through 10-10 | $75,000 |
| **M4: Audited mainnet pilot + agent demo** | Mainnet vault live behind the $500K pilot cap, activated only after M3's contract audit; public agent-settlement demonstration; joint co-marketing artifact; zkVM program audits delivered and remediated before any cap increase; hardened prover service in continuous operation with a published cost/latency budget | 2026-10-15 through Q4 2026 | $75,000 |

Budget allocation across tranches: $150K third-party audits (contracts plus two zkVM programs), $60K prover-service hardening and GPU infrastructure, $40K mainnet pilot operations and monitoring. Post Fiat funds all core protocol engineering itself.

**Team.** Post Fiat's core engineering team, with shipped public systems including the PFTL L1 (post-quantum consensus, ML-DSA), the Tier-4 bridge prototype above, NAVCoin settlement infrastructure, and a production research/agent stack. Working systems and their acceptance gates are public in the repository; we are legible by our commits.

### 4. Risks, stated plainly

Arc launches in under three weeks, and we treat that compression honestly rather than heroically. The schedule works because the new surface is small: the ingress circuit reuses our audited-in-practice receipt/MPT code and SP1's Ed25519 precompile, and the genuinely unknown items (commit-to-execution binding, validator-set commitment format, precompile behavior, receipt encodings) are all front-loaded into week-one conformance gates that block circuit work rather than being discovered late. If any week-one gate slips, every downstream date slips with it and tranches simply pay later; no tranche pays for a missed gate, and mainnet exposure waits for the contract audit regardless of calendar. Our ingress trust anchor becomes Arc's permissioned validator quorum, which we verify cryptographically in-circuit and disclose in all public materials. Uncapped reserves wait for audits. If any gate fails, the pilot cap holds and tranches stop; the committee's downside is bounded by construction.

---

## Part II. Technical annex: Tier-4 bridge specification for Arc

**Status:** engineering specification, review copy. Deployment authorization follows the milestone gates above.

### A1. Objective

Port the demonstrated Tier-4 mechanism, both bridge directions authorized by succinct finality proofs with no committee and no downgrade path, from the Arbitrum/Ethereum prototype to Arc:

- **Ingress:** USDC deposited into an Arc vault mints pfUSDC on PFTL against a zk proof of Arc finality plus receipt inclusion.
- **Egress:** pfUSDC burned on PFTL releases vault USDC on Arc against the existing SP1/Groth16 proof of PFTL consensus finality, ML-DSA verified in-circuit.

Conservation target: `issued == Σcounted − Σredeemed`, exact to the 6-decimal atom, `VAULT_BRIDGE_UNIT = 1_000_000` matching Arc USDC's 6 decimals.

### A2. Verified Arc facts (2026-08-28)

- `https://rpc.testnet.arc.network` answers `eth_chainId = 0x4cef52` (5042002); block height 59,313,510 with current timestamps; standard `receiptsRoot` present in headers (probed live).
- Public testnet live since 2025-10-28; Arc in private mainnet with 100+ builders; public mainnet 2026-09-16, chain ID 5042 (Circle, 2026-08-05).
- Founding validator cohort: Circle plus BlackRock, DTCC, Galaxy, Global Payments, ICE, Mastercard, MoneyGram, SBI Group, Standard Chartered, Sumitomo Corporation, Visa.
- Consensus signing is Ed25519 (`arc-malachitebft-signing-ed25519` in the arc-node dependency tree).
- USDC is the 6-decimal system contract at `0x3600000000000000000000000000000000000000`; gas is USDC.

### A3. Component mapping (Arbitrum prototype to Arc)

| Component | Prototype | Arc port | Change class |
|---|---|---|---|
| Vault | `ERC20BridgeVaultV2.sol` on Arbitrum | same contract, `token = 0x3600…0000` | redeploy, zero expected code change |
| Ingress anchor | `PfUsdcIngressAnchorV1.sol` | same | redeploy |
| Egress verifier | `PFTLFinalityVerifierV1.sol` + SP1 Groth16 verifier | same contracts on Arc | redeploy; precompile conformance gated |
| Egress guest | `programs/pfusdc-egress` (PFTL finality, ML-DSA in-circuit) | unchanged | none; egress proves PFTL, not the vault chain |
| Ingress guest | `programs/pfusdc-eth-ingress` | new `programs/pfusdc-arc-ingress` | new program, new pinned vkey |
| PFTL-side verifier | bounded Groth16 admission (`nav_sp1_verifier.rs`) | unchanged; new route registers the Arc vkey | config + route activation |
| Prover tooling | `tools/pfusdc-tier4-prover` | extended with Arc witness builder | additive |

### A4. Ingress proof statement (`pfusdc-arc-ingress`)

Public inputs:

```
route_id, arc_chain_id, vault_address, token_address,
deposit_id, amount_atoms, pftl_recipient_hash, deposit_nonce,
arc_block_hash, arc_block_height,
validator_set_commitment_in, validator_set_commitment_out
```

In-circuit obligations:

1. **Header finality.** Verify Ed25519 commit signatures over the Arc block commit from validators in the set committed by `validator_set_commitment_in`, with signed voting power at or above 2/3 of total. The exact commit preimage (vote encoding, domain separation, height and round binding) is pinned from arc-node source with golden vectors produced before circuit work begins.
2. **Commit-to-execution binding.** The circuit proves the complete linkage from the value the validators actually sign to the EVM execution header that carries `receiptsRoot`. In a Malachite/CometBFT-style stack the commit signs a consensus block identifier; the chain from that identifier through the consensus header's application-state commitment to the EVM header is a pinned, versioned structure extracted from arc-node source and verified in-circuit. If Arc's commit signs the execution header hash directly, this obligation collapses to a field equality; if the linkage is indirect, every intermediate commitment is opened inside the proof. No step of this binding is assumed. Golden vectors for the full path are a week-one gate.
3. **Receipt inclusion.** Keccak/RLP Merkle-Patricia walk of the deposit receipt under `receiptsRoot`, including typed-receipt (EIP-2718) envelope handling, conformance-tested against live testnet transactions.
4. **Log equality.** The `Deposit` log fields reproduce `deposit_id` byte-for-byte under the existing `vault_bridge_deposit_id()` derivation, with `source_domain = erc20_bridge_vault:<chain>:<vault>:<usdc>` and `finality_ref = evm_log:<chain>:<block_hash>:<tx_hash>:<log_index>`. The chain-parameterized evidence schema admits Arc with zero on-ledger format changes.
5. **Validator-set rotation.** A set change is proven as a transition and `validator_set_commitment_out` reflects it. The PFTL-side Arc checkpoint advances only by proof, mirroring the vault-side PFTL checkpoint.

The PFTL node admits the proof through the existing bounded Groth16 verifier and mints exactly `amount_atoms`, cap-checked against Σcounted. Replay is excluded by evidence-root deduplication plus nullifier binding to `deposit_id`.

### A5. Egress path on Arc

Unchanged in substance from the demonstrated prototype: burn on PFTL; prove the burn's inclusion in finalized PFTL consensus with validator ML-DSA signatures verified in the zkVM, checkpoint-pinned to a short ancestry segment (three blocks, roughly three GPU-minutes in the July runs); verify the constant-size Groth16 proof in `PFTLFinalityVerifierV1` on Arc; release exactly the burned amount; consume the nullifier.

Arc-specific gates: BN254 pairing precompiles (`0x06`, `0x07`, `0x08`) and `sha256` (`0x02`) present with workable gas; measured USDC cost per verification published as a standing cost line; checkpoint ceremony established once from a genuinely finalized PFTL block, advancing only by proof thereafter.

### A6. Trust model, stated honestly

Egress trust is unchanged: PFTL consensus soundness plus proof-system soundness. Ingress trust changes shape: Ethereum's open validator set is replaced by Arc's permissioned cohort of named institutions, and a 2/3 collusion of that cohort could fabricate an ingress fact. That narrowing appears in all public materials. It remains categorically stronger than committee bridges: the quorum verified is the chain's own consensus, in-circuit, with no attestation layer, no operator checkpoint writes, and no signer fallback. The privileged control is pause only. Deposits use Arc's transparent path; opt-in privacy transfers are out of scope because log-based evidence must stay provable.

### A7. Cost and latency budget

| Item | Prototype | Arc target |
|---|---|---|
| Deposit finality wait | minutes (Ethereum) / ~6.4 d (deprecated Nitro path) | under 1 s, depth 1 |
| Ingress witness | Ethereum finality machinery | 1 header + ~12–20 Ed25519 sigs + 1 receipt proof + commit binding |
| Ingress proving | dominated by Ethereum verification | measured at M2; target under 60 GPU-s |
| Egress proving | ~3 GPU-min | unchanged |
| Egress verify gas | ETH-denominated | USDC-denominated, fixed; measured at M1 |
| Round trip | demonstrated, conservation exact | same gate, faster ingress |

### A8. Open items

1. Malachite commit preimage and commit-to-execution binding structure from pinned arc-node source (week one; blocks circuit work).
2. Typed-receipt RLP conformance under `receiptsRoot` (week one).
3. BN254/sha256 precompile availability and gas at testnet and mainnet genesis.
4. Whether mainnet deployment is permissionless at launch; if gated, the builder program is the enrollment path and M1 artifacts are the application evidence.
5. Validator-set publication and on-chain commitment format at mainnet genesis.
6. SP1 version pin with the Ed25519 precompile patch; verifying-key governance under fresh-ELF discipline.
7. CCTP availability on mainnet day one (shapes redemption fan-out; additive).

### A9. Scope boundaries

This document commits Circle to nothing and Post Fiat to the milestone gates only. Arc timing and mechanics are Circle's; statements about them are dated 2026-08-28 from public sources and live RPC probes. "Trustless round trip" is reserved for the state where both directions verify proofs on-chain with no committee and no downgrade path, the standard the July demonstration set, applied unchanged to Arc.
