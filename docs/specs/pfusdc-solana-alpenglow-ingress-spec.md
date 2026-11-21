# PFUSDC from Solana: an Alpenglow-finality deposit route

- **Status:** proposal, nothing built. Written 2026-11-17 for review by Domagoj Ravlić.
- **Goal:** credit a USDC deposit as PFUSDC in about 5 minutes with one wallet confirmation,
  against about 26 minutes and two confirmations on the Ethereum route today, with no new
  trusted signer.
- **Reference code:** `/home/postfiat/repos/ref-solana/` on the main machine (see section 10).
  Agave paths below are relative to `ref-solana/agave` at commit `141da0c3` (master,
  `5.0.0-alpha.0`, 2026-11-16). Mainnet runs the 4.3 line; re-check every format against the
  mainnet release tag before writing code.

## 1. Why this is worth doing

Measured on the Ethereum route on 2026-11-16 (5,025 USDC, deposit
`0x009c8320…2f85`, record in `b23-coord/evidence/launch/bridge-job/job-latest.json`):

| Stage | Seconds | Share |
| --- | ---: | ---: |
| Waiting for Ethereum finality | 1,049 | 67% |
| Building the proof (GPU) | 186 | 12% |
| Verifying the proof locally | 255 | 16% |
| Raising the backed cap and claiming on the network | 62 | 4% |
| **Total, deposit sent to PFUSDC credited** | **1,554 (25.9 min)** | |

Two thirds of the wait is Ethereum finality. It cannot be engineered away on a trustless
Ethereum route. The user also signs twice (approve, then deposit) and pays Ethereum gas.

Solana activated Alpenglow on mainnet on 2026-10-21 (epoch 1064, per solana.com; not
independently verified here). It changes three things that matter to a bridge:

1. **Finality in well under a second**, with an explicit certificate per finalized block.
2. **The certificate is one aggregate BLS signature plus a bitmap** of which validators
   signed. Before Alpenglow a light client had to check hundreds of kilobytes of individual
   votes per slot.
3. **The certificate names a block ID that is a Merkle root over the block's data**, so
   the block's contents can be tied to the certificate with hashes alone.

Expected result on a Solana route, using our own measured numbers for the stages that stay:

| Stage | Seconds (estimate) | Basis |
| --- | ---: | --- |
| Solana finality | under 2 | Alpenglow design target is about 150 ms |
| Fetching block data and changed accounts | 5 to 20 | depends on the data feed, section 7 |
| Building the proof (GPU) | 190 to 350 | ours is 186 s today; the Janus reference reports 5.8 min for a mainnet-size block on one RTX 4090 |
| Verifying locally, cap, claim | 60 to 320 | 317 s today; the 255 s local verify step should be cut regardless of route |
| **Total** | **about 4.5 to 11.5 min** | |

The honest range is wide because block-proving time on real mainnet blocks is the unknown.
Phase 0 below exists to measure it before anything else is built.

## 2. What Alpenglow provides, from the code

**Votes and certificates.** `votor-messages/src/certificate.rs`:

- `CertificateType::FinalizeFast(Block)` needs 80% of stake and names `(slot, block_id)`.
- `CertificateType::Finalize(Slot)` needs 60% of stake and names only the slot. It is only
  meaningful together with a `Notarize(Block)` certificate (60%) for the same slot.
- A `Certificate` is `{cert_type, signature: BLSSignature, bitmap: Vec<u8>}`. The bitmap
  lists signers by rank; the encoding is the `solana-signer-store` format (base 2 for a
  single payload, base 3 when primary and fallback votes are mixed).

**What is signed.** `votor-messages/src/wire.rs`, `VotePayloadToSign`: a tagged struct of
the vote type, the `Block {slot, block_id}` or the slot, and the cluster's `shred_version`,
serialised with wincode. `get_vote_payload_to_sign` returns the exact bytes.

**How a certificate is checked.** `bls-cert-verify/src/cert_verify.rs`,
`verify_certificate(cert, max_validators, total_stake, rank_map)`: decode the bitmap,
aggregate the BLS public keys at those ranks, verify the aggregate signature over the
payload, and require the signers' stake to meet the threshold.

**The validator set.** `runtime/src/epoch_stakes.rs`, `BLSPubkeyToRankMap::new`: built once
per epoch from the epoch's vote accounts. A validator is included if it has stake, has a BLS
key, and neither its BLS key nor its node key is duplicated. Rank is by stake, highest first.
SIMD-0326 caps the set at the 2,000 highest-staked validators.

**The finalization certificate travels in the chain.** `entry/src/block_component.rs`:
`BlockFooterV1 {bank_hash, block_producer_time_nanos, block_user_agent, block_final_cert,
skip_reward_cert, notar_reward_cert}`. `BlockFinalizationCert {slot, block_id,
final_aggregate, notar_aggregate}` is the leader's latest finalization certificate, so a
certificate for block N is normally found in the footer of a block shortly after N.

**Block ID.** `ledger/src/blockstore_meta.rs`, `DoubleMerkleMeta`: the block ID is the root
of a Merkle tree whose leaves are the Merkle roots of each FEC set of shreds, plus one leaf
for the parent (`parent_slot`, parent block ID). So the block ID commits to every data shred
in the block and to the parent chain.

**Bank hash.** `runtime/src/bank.rs`, `hash_internal_state`:
`bank_hash = sha256(sha256(parent_bank_hash, signature_count, last_blockhash), accounts_lt_hash)`,
with an extra hash step at a hard-fork slot. `accounts_lt_hash` is the lattice hash of every
account in the bank (1,024 16-bit lanes; code in `lattice-hash/`). The footer of a block
carries that block's own bank hash.

**Consequence.** There is no per-account Merkle proof on Solana. The lattice hash is a
running sum, so the only way to show an account's new state is committed is to show that
`lt_hash(parent) - sum(old states) + sum(new states) = lt_hash(this block)` over **every**
account the block changed. That is a whole-block computation. It is too large for a
transaction but is routine inside a zero-knowledge proof.

## 3. Design

The route mirrors the Ethereum one (`programs/pfusdc-eth-mainnet-ingress`, proof kind
`sp1-ethereum-finality-v1`): one SP1 program with a pinned verification key, a governed
route profile, and an on-ledger anchor that pins who the source chain's signers are.

### 3.1 Components

| Component | What it does | New or reused |
| --- | --- | --- |
| Solana vault program | Takes USDC into a vault token account and writes one **receipt account** per deposit | New. Must have its upgrade authority removed before real money |
| SP1 program `pfusdc-sol-mainnet-ingress` | Proves "this receipt was created in a finalized Solana block" | New. Structure follows the Ethereum ingress program |
| On-ledger Alpenglow anchor | Stores, per Solana epoch, the hash of the validator set and total stake the proof must use | New ledger state, the analogue of `PfUsdcBeaconFinalityStateV1` |
| Route profile `solana-mainnet-usdc-v1` | Pins the vault program ID, USDC mint, program verification key, cluster `shred_version` and genesis hash | New profile; existing governance path |
| Proof kind `sp1-solana-alpenglow-finality-v1` | Admission rule in execution next to `sp1-ethereum-finality-v1` | New. **Consensus change** |
| Bridge job service | Same stages as today without the finality wait | Reused, new source adapter |
| GPU prover | Same rented machine and locking | Reused |
| Task Node deposit screen | Phantom, one confirmation | New screen; relay and job tracking reused |

Deposits credit a **new PFUSDC series** backed by the Solana vault, the same way the Ethereum
vault backs series S today. Whether Solana-backed and Ethereum-backed PFUSDC are one fungible
family for creating NAVCoins is a product decision; the ledger already tracks series
separately.

### 3.2 Solana vault program

One instruction, `deposit(amount, pftl_recipient, nonce)`:

1. Transfer `amount` USDC from the user's token account to the vault token account by CPI
   to the SPL Token program. If the transfer fails the whole transaction fails.
2. Create a receipt account at a program-derived address seeded by the nonce, owned by the
   vault program, with fixed-layout data:
   `magic | route_binding | pftl_recipient (20 bytes) | amount (u64) | nonce | depositor`.

The receipt is the only thing the proof reads. Because the program creates it only after a
successful transfer, "a receipt owned by the vault program was created in this block" means
the USDC arrived. Failed transactions are included in Solana blocks, so proving that a
transaction is *in* a block is not enough; proving a new account owned by the program is.

Receipts are never closed by anyone but the program, and the proof only counts accounts that
did not exist before the block (old lamports zero, new lamports non-zero), so a receipt
cannot be replayed from a later block. The ledger also rejects a repeated deposit ID, as it
does for Ethereum.

### 3.3 The proof

Private inputs: the block's data shreds, the bank-hash preimages for the block and its
parent, the old and new state of every account the block changed, the finalization
certificate(s), and the validator set for the epoch.

The program checks, in order:

1. **Shreds to block ID.** Hash each data shred into its FEC-set Merkle tree, combine the
   FEC roots and the parent leaf into the double Merkle root. Output: `block_id`.
2. **Block contents to bank hash.** Parse the reassembled block stream, require exactly one
   footer, read `bank_hash` from it.
3. **Bank hash to lattice hash.** Check the supplied preimage hashes to `bank_hash` and
   exposes `lt_hash_after`; check the parent's preimage hashes to the `parent_bank_hash`
   inside it and exposes `lt_hash_before`.
4. **Block delta.** For every changed account subtract the old lattice value and add the new
   one, reject duplicate keys, and require the result to equal `lt_hash_after`.
5. **Receipts.** Among the changed accounts, select those newly created, owned by the pinned
   vault program, with the receipt layout and the pinned route binding.
6. **Finality.** Verify a `FinalizeFast` certificate for `(slot, block_id)`, or a `Finalize`
   certificate for the slot together with a `Notarize` certificate for `(slot, block_id)`:
   decode the bitmap, aggregate the signers' keys from the supplied validator set, check the
   BLS signature over the exact payload including `shred_version`, and check the stake
   threshold. Hash the supplied validator set and expose it.

Public values: route ID, cluster genesis hash and `shred_version`, vault program ID, slot,
`block_id`, Solana epoch, validator-set hash, total stake, and the deposit evidence
(recipient, amount, nonce, deposit ID).

The ledger accepts the proof if the program verification key matches the route profile, the
validator-set hash and total stake match the on-ledger anchor for that epoch, and the deposit
ID is new. That is the same shape as today's Ethereum admission, where the proof's sync
committee root must match the on-ledger beacon anchor.

Steps 1 to 5 are implemented in the Janus reference (`ref-solana/janus/zk/lib/src/lib.rs`,
278 lines, `verify_block` and `verify`). Janus does step 6 in a Solidity contract instead; we
do it inside the proof because our verifier is our own ledger and the existing admission path
already takes "one SP1 proof plus an anchor". SP1 has BLS12-381 acceleration, and the
Ethereum route already verifies a 512-key aggregate inside SP1 through Helios.

An alternative is to verify the certificate natively in ledger execution (two pairings and up
to 2,000 point additions) and keep the proof to steps 1 to 5. That is cheaper to prove but a
larger consensus change. Decide after Phase 0 measures the cost of step 6 in the proof.

### 3.4 Tracking the validator set (the hard part)

The proof is only as good as the validator set it checks against. The set changes every
Solana epoch, roughly every two days. Nothing in Alpenglow signs "the next epoch's set", so a
light client has to derive it. Options, weakest to strongest:

| Option | How the next set is established | Trust added | Effort |
| --- | --- | --- | --- |
| A. Governed anchor | Post Fiat validators ratify each epoch's set hash through the existing governance path, from their own reading of Solana | The Post Fiat validator quorum, which already controls every route profile | Small |
| B. Recorder program | A Solana program reads the runtime's epoch stakes into an account; a proof in "set mode" shows that account's contents in a block finalized by the **current** set, which establishes the next set | None beyond Solana itself, if the recorded stakes equal the stakes Alpenglow actually uses | Medium. Janus does this and reports a mismatch on testnet (404 of 466 validators), so it is not yet sound |
| C. Derive from vote accounts | The proof reads the vote accounts and stakes directly from finalized state and applies `BLSPubkeyToRankMap::new` | None beyond Solana itself | Large. Thousands of accounts, and stake totals come from stake accounts |

Recommendation: ship with **A** and say so plainly, then replace it with B or C. Under A a
deposit still cannot be forged by the bridge operator, the relayer or the prover; forging one
needs a Post Fiat validator quorum to ratify a false Solana validator set, and that same
quorum could already rewrite the route profile. It is the same trust the Ethereum route has
for its initial beacon anchor, applied every epoch instead of once.

The cost of A is operational: someone, or a job, must post each epoch's set before deposits
in that epoch can be credited. If the anchor is late, deposits wait; they are not lost.

### 3.5 Withdrawals (PFUSDC back to USDC on Solana)

Out of scope for the first phase, and not where the time goes: withdrawals already take about
7 minutes (measured 2026-11-16), almost all of it our own proof. The Solana version needs the
vault program to verify an SP1 Groth16 proof of Post Fiat finality using the `alt_bn128`
syscalls. Succinct publishes a Solana verifier and Janus carries a port of the v6.1.0
verifier (`ref-solana/janus/solana/`); confirm which matches our SP1 version. The existing
egress program (`programs/pfusdc-egress`) and checkpoint logic carry over; the checkpoint
freshness rule that the Ethereum verifier enforces must be re-implemented in the program.

Until withdrawals exist on Solana, a Solana route is deposit-only. It must not be opened to
users in that state unless the exit through the Ethereum vault is documented and funded,
because the two vaults hold separate collateral.

## 4. What the user trusts, against the Ethereum route

| | Ethereum route today | Solana route, option A | Solana route, option B or C |
| --- | --- | --- | --- |
| Source chain finality | Ethereum sync committee (512 validators sampled) | Alpenglow certificate, 60% or 80% of all stake | same |
| Who the signers are | Beacon anchor set once, then self-updating by proof | Ratified each epoch by Post Fiat validators | Self-updating by proof |
| Deposit happened | Storage proof against the state root | Lattice-hash block delta | same |
| Hash assumptions | Keccak, SHA-256 | SHA-256, BLAKE3, lattice hash (a newer assumption), 20-byte Merkle nodes in the shred tree (Solana's own choice) | same |
| Bridge operator, relayer, prover | Cannot forge | Cannot forge | Cannot forge |
| Token issuer | Circle can freeze the vault | Circle can freeze the vault token account | same |

## 5. Risks and open questions

1. **Proving time on real blocks is unmeasured by us.** The whole block and every changed
   account go through the proof. Janus reports 36 MB and 3,013 accounts in 5.8 minutes on one
   RTX 4090 for a synthetic mainnet-size block. Busy mainnet blocks may be larger. Measure
   first.
2. **Getting the data.** Standard Solana RPC does not return raw shreds with Merkle proofs,
   bank-hash preimages or the old state of changed accounts. This needs our own non-voting
   node with a Geyser plugin, or a commercial feed that exposes shreds, footers and account
   updates. This is a new always-on dependency and the largest operating cost of the route.
3. **Format drift.** Shred layout, footer version, vote payload and bitmap encoding are
   pinned inside the proof program. Any Solana upgrade that changes them stops deposits until
   a new program key is governed in. Deposits wait; nothing is lost. Budget for this
   recurring work.
4. **Cluster restarts and hard forks.** A restart changes `shred_version`, which is in every
   signed payload, and a hard fork adds a step to the bank hash. Both must be handled by a
   governed profile update.
5. **Slow-path finality.** When fewer than 80% sign quickly, finality is the two-certificate
   form. The proof must accept both forms and must never accept a `Finalize(slot)`
   certificate without the matching notarization for the same block ID.
6. **Fallback votes.** Notarize-fallback certificates use the base-3 bitmap and two payloads.
   Decide whether the proof needs them at all; the simplest sound rule is to accept only
   `FinalizeFast`, or `Finalize` plus `Notarize`.
7. **Validator-set soundness** under options B and C, as in section 3.4.
8. **Vault program immutability.** An upgradable vault program is a backdoor around every
   proof. Remove the upgrade authority before real funds and verify it on chain.
9. **The lattice hash is a younger assumption** than Merkle trees. It is what Solana itself
   relies on for state integrity, so the route is no weaker than Solana, but it is a
   different assumption from the Ethereum route's and should be stated in user-facing text.
10. **Reference code provenance.** Janus is a small, unlicensed repository by an unknown
    author with a testnet round trip. Use it to learn the formats and to cross-check our
    implementation. Do not copy it into the tree. A second project named in search results
    (SODA) could not be found at the cited address.

## 6. Plan

Each phase ends in something that can be checked. No phase touches the live network until
Phase 3.

**Phase 0. Measure (about one week).** No Post Fiat code.
- Stand up a non-voting Solana mainnet node or a paid feed; capture shreds, footers and
  account changes for a sample of real blocks.
- Run the Janus prover on real mainnet blocks on our rented GPU. Record block size, changed
  accounts and proving time for 50 blocks, including busy ones.
- Verify one real mainnet finalization certificate natively with `bls-cert-verify` against a
  rank map built from an RPC snapshot of the epoch's vote accounts.
- **Gate:** median and worst-case proving time, and the monthly cost of the data feed. If the
  worst case is above about 10 minutes, stop and reconsider.

**Phase 1. Proof program (two to three weeks).**
- Write `pfusdc-sol-mainnet-ingress` in our tree with our witness, policy and public-value
  schemas, following the Ethereum ingress program's structure and tests.
- Negative tests: wrong block ID, altered shred, missing changed account, duplicated account,
  receipt from another program, wrong route binding, certificate below threshold, wrong
  `shred_version`, `Finalize` without `Notarize`, validator set not matching the anchor.
- **Gate:** real mainnet blocks prove and every negative test is rejected.

**Phase 2. Solana vault program and ledger support (two to three weeks, parallel with 1).**
- Vault program with the receipt layout; deploy to devnet, then mainnet with a small cap.
- Ledger: route profile, anchor state, proof kind and admission rule, behind an activation
  height. Full replay and rejection tests as for the Ethereum route.
- **Gate:** a local six-validator network credits a devnet deposit from a real proof and
  rejects each forged variant.

**Phase 3. Live, small (one week).** Needs approval for the consensus change.
- Activate on the live network. Deposit 1 USDC, then 100, from a wallet we control. Time
  every stage.
- **Gate:** deposits credited, timing table published, a late or missing anchor shown to
  delay and not lose a deposit.

**Phase 4. Withdrawals and Task Node (two to three weeks).**
- Solana-side verifier and payout in the vault program; withdrawal service adapter.
- Phantom deposit and withdraw screens in the Task Node wallet.
- **Gate:** full round trip in a browser with Phantom, with real funds, by us before anyone
  else is asked to try it.

**Phase 5. Remove the governed anchor.** Replace option A with B or C.

Durations are estimates for one engineer who knows the Ethereum route.

## 7. Operating cost and dependencies

- A Solana data source that exposes shreds, footers and account changes (item 2 above).
- The same rented GPU, used for a few minutes per deposit block. Several deposits in one
  block share one proof.
- A job that posts each epoch's validator-set anchor (option A).
- Solana transaction fees are negligible; the user pays them.

## 8. What can be done now regardless of this route

- The 255-second "verifying" stage on the Ethereum route is our own local check. It is 16% of
  the wait and is worth cutting on its own.
- Keeping the proving machine warm and starting capture at the moment of finality removes
  idle gaps between stages.

Neither removes the finality wait.

## 9. Decisions needed

1. Go or no-go on Phase 0 (a week of measurement and a data-feed trial).
2. Whether shipping with the governed anchor (option A) is acceptable as a first version.
3. Whether Solana-backed PFUSDC is fungible with Ethereum-backed PFUSDC for creating
   NAVCoins, or a separate series end to end.
4. Whether a deposit-only period is acceptable, and what the documented exit is during it.

## 10. Code reference index

On the main machine under `/home/postfiat/repos/ref-solana/` (shallow clones, read-only
reference, not part of any build):

| Path | Commit | What to read |
| --- | --- | --- |
| `agave/votor-messages/src/certificate.rs` | `141da0c3` | Certificate types and thresholds |
| `agave/votor-messages/src/wire.rs` | | `VotePayloadToSign`, the exact signed bytes |
| `agave/votor-messages/src/vote.rs` | | Vote types |
| `agave/bls-cert-verify/src/cert_verify.rs` | | `verify_certificate`, base-2 and base-3 bitmaps, stake check |
| `agave/runtime/src/epoch_stakes.rs` | | `BLSPubkeyToRankMap`, how the set and ranks are derived |
| `agave/entry/src/block_component.rs` | | `BlockFooterV1`, `BlockFinalizationCert`, block markers |
| `agave/ledger/src/blockstore_meta.rs` | | `DoubleMerkleMeta`, block ID construction |
| `agave/runtime/src/bank.rs` | | `hash_internal_state`, bank hash composition |
| `agave/lattice-hash/` | | Lattice hash |
| `agave/votor/` | | Consensus state machine |
| `simd/proposals/0326-alpenglow.md` | `ae7b00f` | The protocol proposal |
| `alpenglow/` | `65be16b` | Anza's Alpenglow repository |
| `janus/zk/lib/src/lib.rs` | `68f25fb` | Block proof: shreds, block ID, footer, bank hash, block delta, receipts, validator-set mode |
| `janus/ethereum/src/AlpenglowBLS.sol` | | Certificate check written out step by step |
| `janus/solana/` | | Vault program, recorder program, SP1 Groth16 verifier on Solana |

Our side, in this repository:

| Path | What it is |
| --- | --- |
| `programs/pfusdc-eth-mainnet-ingress/src/lib.rs` | The Ethereum deposit proof this route copies in structure |
| `programs/pfusdc-egress/` | Withdrawal proof |
| `docs/vault-bridge-deposit-evidence-spec.md` | Deposit evidence format and admission |
| `crates/execution/src/nav_vault_asset_execution.rs` | Deposit admission and the `sp1-ethereum-finality-v1` rule |
| `crates/types/src/pfusdc_tier4_types.rs` | Public values and the beacon anchor type |
