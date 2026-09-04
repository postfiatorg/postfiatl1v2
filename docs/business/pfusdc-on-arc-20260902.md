# pfUSDC on Arc: a proof-verified USDC bridge, running on Arc testnet

**Date:** 2026-09-02
**Status:** Draft for founder review. Not yet published. Nothing in this
document is reviewed, audited, endorsed, or co-sponsored by the auditor or Circle.

---

## What this is

pfUSDC is a USDC-backed settlement asset on the Post Fiat L1 (PFTL). USDC sits
in an on-chain vault; pfUSDC is minted and redeemed against it. What makes it
different from a bridge is how the vault is controlled: there is no
multisig, no attestation committee, and no signer fallback. Both directions
are authorized only by succinct zero-knowledge proofs of the other chain's
finality, and the single privileged control is pause.

We are porting that system to Arc so that Arc is the USDC rail underneath
PFTL's settlement flow. This post says exactly what already runs on Arc
testnet, what we have shipped elsewhere, and what remains.

## What already runs on Arc testnet

Everything below has an accepted (status `1`) receipt on Arc testnet
(chain ID 5042002) and is recorded in the
[Arc evidence ledger](../evidence/arc-mvp-20260828/deployments.md).

| What | Evidence |
| --- | --- |
| SP1 v6.1.0 Groth16 verifier and a gateway with the proof route registered | Deployed at block 59,332,379; runtime code hashes read back and match the pinned SP1 contracts source |
| One-shot pfUSDC deployment factory | Deployed at block 59,332,489; predicts the immutable anchor and vault addresses |
| Precompile conformance (SHA-256, BN254 add/mul/pairing) | Four live probe transactions with measured gas; the gates a Groth16 verifier needs are present and correct on Arc |
| USDC conformance | Approve and an exact 1,000,000-atom pull, balances before and after, gas paid in USDC |
| Arc consensus certificate conformance | Two live commit certificates verified against pinned `arc-node` source, forged and sub-quorum negatives rejected |
| Receipt inclusion conformance | Typed and legacy receipts opened under a live header's `receiptsRoot`; mutations rejected |
| A real 1.000000 USDC deposit | Vault deposit at Arc block 59,335,780 producing a canonical deposit ID and evidence root |
| A zero-knowledge proof of that deposit | Groth16 proof that the deposit's receipt is included in a block finalized by Arc's validator quorum, generated in 54.42 GPU-seconds on one A100 and locally verified; six corrupted-witness cases rejected |

In plain terms: we deposited one USDC on Arc, and we produced a constant-size
proof that Arc's own validators finalized that deposit. That proof, not a
committee, is what mints pfUSDC.

## What we shipped before Arc

On 2026-07-19 the same design completed a full public round trip on a
controlled testnet against Arbitrum Sepolia: a genuine USDC deposit minted
pfUSDC against a proof of finalized source-chain state, then burned and
withdrew against a proof of finalized PFTL consensus, releasing exactly
1.000000 USDC. The withdrawal settled in
[Arbitrum Sepolia transaction `0x664b2897…e702c1f9`](https://sepolia.arbiscan.io/tx/0x664b2897f9f569bebeb5ef50968fde89c162c56c96cfa676740c4e42e702c1f9).
The acceptance record is immutable at commit
[`2d39bbbf`](https://github.com/postfiatorg/postfiatl1v2/blob/2d39bbbfa370fcf3d0252b6594f578fd9914d763/docs/evidence/pfusdc-tier4-v3-epoch6-core-acceptance-20260719/ACCEPTANCE.json).

The egress side is the part that does not change for Arc. PFTL validators sign
with ML-DSA (FIPS-204). The withdrawal proof verifies those post-quantum
signatures inside an SP1 zkVM and wraps the result in a Groth16 proof that the
Arc verifier contract checks.

## Why Arc

- Deterministic sub-second finality removes the multi-day ingress wait we had
  on the previous domain.
- Arc consensus signs with Ed25519, which is cheap to verify in-circuit; the
  measured ingress proof is under one GPU-minute.
- Gas is USDC, so on-chain proof verification has a USDC-denominated cost
  that we can publish as a standing line item.
- Arc's validator set is a named institutional cohort. Our ingress proof
  verifies that quorum's signatures directly, so the trust assumption is
  Arc's own consensus, disclosed, rather than an added committee.

## Where the port stands

- **Arc contracts.** An immutable finality-verifier/anchor/vault set is
  deployed on Arc testnet (anchor `0x661D558a…`, vault `0xe88FB9ab…`,
  verifier `0xC59EBED2…`) and holds the 1.000000 USDC deposit above. It is
  pinned to the August egress program. The current release re-pins the egress
  proof and the PFTL checkpoint, so a fresh pair is deployed from the same
  one-shot factory pattern before the round trip below.
- **PFTL side.** The Arc-capable node release runs on all six validators of
  the current PFTL devnet. It passed a deployment-exact gate first: six exact
  clones of the live chain, identical state root under the new binary, two
  finality rounds, restart, and rollback — then a canary-first rolling deploy
  and a live finality round.
  [Receipt](../evidence/arc-mvp-20260828/devnet-20260902/gate-931-and-rollout-receipt.json).
- **Validator-set proofs.** The ingress circuit requires an exact-block
  EIP-1186 proof of Arc's validator registry so set changes are proven, not
  assumed. Public Arc RPCs do not serve historical proofs, so we run our own
  Arc archive node (Arc v0.8.0 execution + consensus follower, full history,
  proof window at the Reth maximum). The circuit fails closed without it; we
  did not weaken it.
- **Full Arc round trip.** Ingress has been proven on a real deposit. The
  fresh deposit → current-v2 proof → PFTL mint → burn → egress proof → Arc
  release sequence runs end to end against the deployed pair and devnet, with
  every receipt published in the evidence ledger as each step lands.
- **Audit.** No third party has reviewed the contracts or zkVM programs yet.
  The [audit scope](arc-audit-scope-20260902.md) is ready for the auditor.

## Trust model

Egress trust: PFTL consensus soundness plus proof-system soundness.
Ingress trust: Arc's permissioned validator quorum, verified in-circuit. A
two-thirds collusion of that cohort could fabricate an ingress fact. That is
narrower than an open validator set and we say so in every public
document. It is still categorically different from a committee bridge: the
quorum being verified is the chain's own consensus, there is no operator
checkpoint write, and there is no signer fallback.

Vault deposits use Arc's transparent path only, because the proof opens the
deposit log.

## What we are asking for

We are building the Arc port regardless. We are applying to the Circle
Developer Grants program for the two things a self-funded schedule would
defer: independent audits of the contracts and both zkVM programs, and a
hardened prover service with a published cost and latency budget.

Milestones pay on delivered gates, not dates. Proposed:

| Milestone | Gate | Target |
| --- | --- | --- |
| M1 | Immutable finality verifier, anchor, and vault deployed and source-verified on Arc testnet; an existing PFTL egress proof verifies in the deployed verifier | to be confirmed |
| M2 | Validator-registry proof source qualified; full Arc testnet round trip with exact conservation, both proofs on-chain, corrupted-witness suite rejected | to be confirmed |
| M3 | Third-party contract audit delivered and remediated | to be confirmed |
| M4 | Capped mainnet pilot activated only after M3; zkVM program audits before any cap increase; prover service in continuous operation | to be confirmed |

Reserve size, pilot cap, and budget are founder commitments and will be stated
in the application, not here.

## Independent review

We are asking the auditor to review the technical claims in this post and the
underlying source, and we have prepared a
[claim-by-claim packet](arc-grant-claim-classification-20260902.md) that classifies
every statement in our proposal as verified in the repository, demonstrated
on a controlled devnet, implemented but not independently verified, planned,
or aspirational. The auditor has not yet reviewed, audited, endorsed, or
co-sponsored anything; if that changes, it will be stated by them.

## Reproduce it

Everything is open source at
[github.com/postfiatorg/postfiatl1v2](https://github.com/postfiatorg/postfiatl1v2).

```bash
cargo test -p arc-conformance --locked
forge test --root crates/ethereum-contracts --match-contract PFUSDCTier4Test -vv
```

The Arc ingress program's ELF and verifying key are rebuilt independently in
CI and must be byte-identical to the checked-in artifacts; see the
[reproduction manifest](../evidence/arc-mvp-20260828/program-reproduction.current-v2-docker-20260902.json).

---

*Arc timing, chain configuration, and validator composition are Circle's.
Statements about them are dated from public sources and live RPC probes.*
