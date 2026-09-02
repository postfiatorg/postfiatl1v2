# pfUSDC on Arc: Tier-4 Trustless Bridge Specification

**Date:** 2026-08-28
**Status:** Proposal for engineering review. No deployment is authorized by this document.
**Supersedes nothing.** Companion to `postfiatorg.github.io/content/research/pfusdc-arc-vault-domain.md`
(2026-08-26, evidence-tier on-ramp, uncommitted) and to
`A666-MAINNET-TRUSTLESS-MINT-SPEC-20260725.md` (Arbitrum deprecation rationale).
**Tier-4 reference implementation:** the 2026-07-19 public post "pfUSDC: A Stablecoin
Bridge Secured by Proofs, Not Committees" and the code inventory in
`a666-eth-fast-lane-combined-20260724` (`ERC20BridgeVaultV2.sol`,
`PFTLFinalityVerifierV1.sol`, `PfUsdcIngressAnchorV1.sol`, `programs/pfusdc-ingress`,
`programs/pfusdc-egress`, `tools/pfusdc-tier4-prover`).

---

## 0. Premise correction, dated

The tasking premise was "Arc is on devnet, targeting testnet next week." Verified state
as of 2026-08-28:

- **Arc public testnet has been live since 2025-10-28.** Probed today:
  `https://rpc.testnet.arc.network` answers `eth_chainId = 0x4cef52` (5042002),
  block height 59,313,510, current timestamps, standard `receiptsRoot` present.
- **Arc is in private mainnet now** with 100+ institutional builders.
- **Public mainnet launches 2026-09-16** (Circle press release, 2026-08-05), chain ID
  5042, with a founding validator cohort of Circle plus BlackRock, DTCC, Galaxy,
  Global Payments, ICE, Mastercard, MoneyGram, SBI Group, Standard Chartered,
  Sumitomo Corporation, and Visa.

The deadline that matters is therefore **mainnet in ~2.5 weeks**, and the full
testnet integration surface is available today. This spec is written against that
schedule.

## 1. Objective

Port the demonstrated Tier-4 mechanism — both bridge directions authorized by
succinct finality proofs, no committee, no downgrade path — from the
Arbitrum/Ethereum prototype to Arc as the vault domain:

- **Ingress:** USDC deposited into an Arc vault mints pfUSDC on PFTL against a
  zk proof of Arc finality plus receipt inclusion.
- **Egress:** pfUSDC burned on PFTL releases vault USDC on Arc against the existing
  SP1/Groth16 proof of PFTL consensus finality (ML-DSA verified in-circuit),
  verified by a contract on Arc.

Conservation target is unchanged: `issued == Σcounted − Σredeemed`, exact to the
6-decimal atom, `VAULT_BRIDGE_UNIT = 1_000_000` matching Arc USDC's 6 decimals.

## 2. Why Arc strictly improves the Tier-4 story

The Arbitrum prototype worked, but its ingress trust chain was the expensive part:
proving a deposit settled under finalized Ethereum state either eats the ~6.4-day
Nitro assertion window (deprecated 2026-07-25 for exactly this) or falls back to
Ethereum mainnet at minutes-scale finality with a heavy light-client obligation.

Arc's consensus is Malachite BFT with **deterministic single-block finality** and a
**permissioned, identity-bearing, published validator set** signing with **Ed25519**
(`arc-malachitebft-signing-ed25519` in the arc-node dependency tree). Consequences:

1. **Ingress proof shrinks structurally.** No assertion window, no probabilistic
   ancestry segment, no beacon-chain machinery. The finality statement is "a ≥2/3
   voting-power quorum of the registered validator set signed this block." One
   header, one receipt-inclusion proof, N signature verifications with N on the
   order of a dozen.
2. **Ed25519 is cheap in SP1.** The zkVM has patched precompiles for it; this is
   far lighter than the PFTL-side ML-DSA work we already do in the egress guest.
   Ingress proving should land well under the ~3-minute GPU egress proof; the
   Stage-2 gate measures it.
3. **Confirmation depth is 1** by construction, so deposit-to-mint latency is
   bounded by proving plus PFTL admission, not by source-chain settlement.
4. **Gas is USDC.** The on-Arc Groth16 verification for egress prices in dollars,
   which makes the per-withdrawal verification cost a fixed line item instead of a
   fee-asset exposure.
5. **Issuer adjacency.** The reserve asset and the settlement chain share an
   issuer, shortening the audit chain from vault balance to canonical USDC supply.

## 3. Architecture

### 3.1 Component mapping (Arbitrum prototype → Arc)

| Component | Arbitrum prototype | Arc port | Change class |
|---|---|---|---|
| Vault | `ERC20BridgeVaultV2.sol` on Arbitrum | same contract, Arc chain 5042002/5042, `token = 0x3600…0000` (USDC system contract) | redeploy, zero code change expected |
| Ingress anchor | `PfUsdcIngressAnchorV1.sol` | same | redeploy |
| Egress verifier | `PFTLFinalityVerifierV1.sol` + SP1 Groth16 verifier on Arbitrum | same contracts on Arc | redeploy; Stage-0 precompile conformance gates it |
| Egress guest | `programs/pfusdc-egress` (PFTL finality, ML-DSA in-circuit) | **unchanged** | none — egress proves PFTL, not the vault chain |
| Ingress guest | `programs/pfusdc-eth-ingress` (Ethereum finality + Nitro assertion) | **new** `programs/pfusdc-arc-ingress` (Malachite quorum + receipt MPT) | new program, new pinned vkey |
| PFTL-side verifier | `nav_sp1_verifier.rs` bounded Groth16 admission | unchanged; new route registers the Arc ingress vkey | config + route activation |
| Prover tooling | `tools/pfusdc-tier4-prover` | extend with Arc witness builder (header, commit signatures, receipt proof) | additive |

### 3.2 Ingress proof statement (`pfusdc-arc-ingress`)

Public inputs:

```
route_id, arc_chain_id, vault_address, token_address,
deposit_id, amount_atoms, pftl_recipient_hash, deposit_nonce,
arc_block_hash, arc_block_height,
validator_set_commitment_in, validator_set_commitment_out
```

In-circuit obligations:

1. **Header finality.** Verify Ed25519 commit signatures over the Arc block
   commit for `arc_block_hash` from validators in the set committed by
   `validator_set_commitment_in`, with signed voting power ≥ 2/3 of total. The
   exact commit preimage (vote encoding, domain separation, height/round binding)
   is extracted from the pinned arc-node source; Stage 0 produces a golden-vector
   test before any circuit work.
2. **Receipt inclusion.** Keccak/RLP Merkle-Patricia walk of the deposit
   transaction receipt under the header's `receiptsRoot` (field verified present
   on testnet today). Typed-receipt (EIP-2718) envelope handling is a Stage-0
   conformance item.
3. **Log equality.** The `Deposit` log fields reproduce `deposit_id` byte-for-byte
   under the existing `vault_bridge_deposit_id()` derivation, with
   `source_domain = erc20_bridge_vault:<chain>:<vault>:<usdc>` and
   `finality_ref = evm_log:<chain>:<block_hash>:<tx_hash>:<log_index>` — the
   chain-parameterized schema admits Arc with zero on-ledger format changes.
4. **Validator-set rotation.** If the block carries a set change, the transition
   is proven and `validator_set_commitment_out` reflects it. The PFTL-side
   checkpoint for Arc advances only by proof, mirroring how the vault-side PFTL
   checkpoint already works. Rotation cadence on a permissioned set is low;
   the circuit treats it as the exceptional path.

The PFTL node admits the proof through the existing bounded Groth16 verifier and
mints exactly `amount_atoms` pfUSDC, cap-checked against Σcounted. Replay is
excluded by the existing evidence-root deduplication plus nullifier binding to
`deposit_id`.

### 3.3 Egress path on Arc

Unchanged in substance from the demonstrated prototype: burn on PFTL, prove the
burn's inclusion in finalized PFTL consensus (validator ML-DSA signatures verified
in the zkVM, checkpoint-pinned to a short ancestry segment — three blocks in the
July runs, ~3 GPU-minutes), verify the constant-size Groth16 proof in
`PFTLFinalityVerifierV1` on Arc, release exactly the burned amount from the vault,
consume the nullifier.

Arc-specific requirements, all Stage-0 gated:

- **BN254 precompiles** (`0x06`, `0x07`, `0x08`) and `sha256` (`0x02`) must exist
  with workable gas costs, since the SP1 Groth16 verifier depends on them.
- **Gas in USDC**: measure the exact USDC cost of one egress verification and one
  vault release on testnet; publish it as the standing per-withdrawal cost line.
- **Checkpoint ceremony**: at deployment the vault-side PFTL checkpoint is
  established once from a genuinely finalized PFTL block, then advances only by
  proof. Identical discipline to the prototype.

### 3.4 Trust model, stated honestly

Egress trust is unchanged: PFTL consensus soundness plus proof-system soundness.
Ingress trust changes shape: Ethereum's open validator set is replaced by Arc's
permissioned cohort of a dozen named institutions. A ≥2/3 collusion of that cohort
could fabricate an ingress fact. That is a real narrowing relative to Ethereum L1
finality and must be stated in any public claim. It is still strictly stronger
than every committee bridge this program refuses to build: the quorum being
verified is the chain's own consensus, in-circuit, with no attestation layer,
no operator checkpoint writes, and no downgrade path to a signer. The privileged
control remains pause-only.

Deposits must use Arc's transparent path; opt-in privacy transfers are out of
scope because log-based evidence must stay provable.

## 4. Staged plan with dates and gates

**Stage 0 — Conformance probes (2026-08-28 → 09-02, no new circuits).**
Against Arc testnet: BN254 + sha256 precompile behavior and gas; receipt RLP and
typed-receipt envelope conformance against the Stage-4 MPT walker; USDC system
contract `approve`/`transferFrom`/`Transfer` log conformance from the vault's
perspective; extraction of the Malachite commit/vote encoding from pinned arc-node
source with golden vectors for the Ed25519 preimage; faucet throughput for
sustained testing.
*Gate G0:* golden-vector commit verification passes outside the zkVM; a
hand-built receipt proof for a live testnet transaction verifies against
`receiptsRoot`; Groth16 verifier test contract verifies a known-good proof on
Arc testnet.

**Stage 1 — Contracts on Arc testnet (09-02 → 09-05).**
Deploy `ERC20BridgeVaultV2`, `PfUsdcIngressAnchorV1`, `PFTLFinalityVerifierV1`,
SP1 verifier gateway to chain 5042002; Blockscout source verification; checkpoint
ceremony from a finalized PFTL devnet block.
*Gate G1:* an existing PFTL egress proof (fresh ELF, vkey printed from that exact
ELF and matched to deployment — the h544 lesson) verifies on Arc testnet inside
the deployed verifier, and a deposit round-trips to a byte-identical `deposit_id`.

**Stage 2 — `pfusdc-arc-ingress` guest (09-05 → 09-12).**
Implement the Section 3.2 statement; pin the vkey; register the Arc route on PFTL
devnet; extend `pfusdc-tier4-prover` with the Arc witness builder.
*Gate G2:* measured ingress proving time published (target: under 60 GPU-seconds;
hard ceiling: under the 3-minute egress proof); deliberately corrupted witnesses
(wrong quorum, forged signature, mangled receipt, replayed deposit) all rejected
with documented errors.

**Stage 3 — Full round trip on Arc testnet (09-12 → 09-15).**
Mirror of the 2026-07-19 Arbitrum Sepolia demonstration: real testnet USDC
deposit → proof → pfUSDC mint → burn → proof → Arc-side release.
*Gate G3:* exactly 1.000000 USDC conserved to the atom; both proofs verified
on-chain; nullifiers consumed; deposit-to-mint under 120 seconds end to end;
burn-to-release under 5 minutes; artifacts published in the evidence tree.

**Stage 4 — Mainnet domain (2026-09-16 onward, explicitly gated).**
At public mainnet: deploy to chain 5042, checkpoint ceremonies from genuinely
finalized blocks on both chains, register the mainnet route behind a hard
supply cap sized for pilot funds only.
*Gate G4 (before any uncapped operation):* third-party audit of contracts and both
zkVM programs; hardened prover service; custody hardening on the exit side,
including the attestor-key off-host backup that currently blocks live activation;
measured cost/latency budget under continuous load. These are the same honest
boundaries the July post committed to publicly; Arc does not shorten them.

Parallel track: the attested-tier Stages A–C from the 2026-08-26 vault-domain spec
can run on the same deployed vault for an early ecosystem-program demo without
waiting for the ingress circuit; the Tier-4 route then supersedes the attested
route. `source_domain` namespacing keeps testnet, mainnet, attested, and proven
routes cleanly separated.

## 5. Cost and latency budget (to be measured, targets stated now)

| Item | Arbitrum prototype | Arc target |
|---|---|---|
| Deposit finality wait | minutes (Ethereum fallback) / ~6.4 d (Nitro path, deprecated) | < 1 s, depth 1 |
| Ingress witness | Ethereum finality machinery + assertion | 1 header + ~12–20 Ed25519 sigs + 1 receipt proof |
| Ingress proving | dominated by Ethereum verification | **measure**; target < 60 GPU-s |
| Egress proving | ~3 GPU-min (3-block segment, ML-DSA accel) | unchanged |
| Egress verify gas | ETH-denominated | USDC-denominated, fixed; **measure** |
| Round trip | demonstrated, conservation exact | same gate, faster ingress |

## 6. Open items

1. Malachite commit/vote preimage details and domain separation from pinned
   arc-node source (Stage 0; blocks circuit work).
2. Typed-receipt RLP conformance under `receiptsRoot` (Stage 0).
3. BN254/sha256 precompile availability and gas on Arc testnet and at mainnet
   genesis (Stage 0 / re-verify at Stage 4).
4. Whether mainnet contract deployment is permissionless at launch; if gated,
   the private-mainnet builder program is the enrollment path and Stage 1
   artifacts are the application evidence.
5. Validator-set publication and rotation mechanics at mainnet genesis
   (the founding cohort is named; the on-chain set commitment format needs
   confirmation).
6. SP1 version pin with the Ed25519 precompile patch, and vkey governance for
   the new ingress program (fresh-ELF vkey discipline per the h544 incident).
7. CCTP availability on mainnet day one, which shapes the redemption fan-out
   story but is additive to this spec.

## 7. What this spec deliberately does not claim

No deployment, timeline commitment, or program participation is authorized here.
Arc mainnet timing and mechanics are Circle's; statements about them are dated
2026-08-28 from public sources and live RPC probes. The permissioned-validator
trust narrowing in Section 3.4 must appear in any public materials derived from
this work. "Trustless round trip" remains reserved for the state where both
directions verify proofs on-chain with no committee and no downgrade path — the
standard the July demonstration set, applied unchanged to Arc.
