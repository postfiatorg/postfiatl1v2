# NAVCoin reserve-proof mechanics — correctness review, 2026-09-24

Task Node: `task_0d9ebb1c222dbb7f15d68d3b3b815dc2`. Reviewed checkout:
`ec583795`. No chain, host, wallet or key was queried or used. The guest
sources reviewed (`near_receipt.rs`, `lib.rs`, `bft_checkpoint.rs`) are
unchanged, apart from test-fixture paths, since the registered successor
identity's source commit `5b8f0317`. The findings therefore apply to the
program behind vkey `0x00f3857f…41bf`.

## 1. Plain-language assessment

**Verdict.** The open kit checks a lot with real mathematics. However, every
source ultimately rests on a **six-member checkpoint committee** (the six keys
in `tools/nav-reserve-proof/manifests/a666/checkpoint-committee.json`; five
signatures are enough). The committee signs which block or chain state is the
real, final one. The kit then proves balances *beneath* that signed point.
It never checks NEAR's, Ethereum's, Solana's or Monero's own validator
signatures. "Cryptographic" in the kit's trust classes means "mathematically
checked below a point the committee vouched for".

**NEAR has a specific gap:** the kit accepts a balance snapshot of **any age**.
Old NEAR stake can therefore be counted after it has been withdrawn
(finding N1, reproduced).

**Aave has a similar gap:** only debts named in the policy are subtracted. A
debt in any other asset is not seen (A1).

| Source | Proven by mathematics in the guest | Vouched for only by a signature | Taken from the manifest/policy without verification |
| --- | --- | --- | --- |
| NEAR stake | The reader contract's receipt reporting staked + unstaked yoctoNEAR is in a NEAR block that is an ancestor of the committee-signed head (Merkle paths). The event, payload hash, account, pool and salt agree. | Committee: the head is real and final, and the reader and pool code hashes match at that head. Reserve owner's Ed25519 key, which is the NEAR account's own key and so the issuer side, authorises the use. | Account, pool (`astro-stakers.poolv1.near`), reader account and code hashes, committee, and the NEAR/USD Chainlink feed |
| Solana stake | The owner's withdraw-authority signature is valid, and the reader output is recomputed from the supplied stake-account bytes. | Committee: signs a commitment to the exact stake-account state at the slot. Solana has no state root to prove against, so the committee attests the quantity. | Stake-account set, wallet, committee |
| Hyperliquid | Reader-contract receipt in the **exact** committee-signed HyperEVM block (receipt trie) | Committee: HyperEVM block. Hyperliquid's own validators: the HyperCore balances and prices exposed to HyperEVM. | Reader contract and code, venue positions |
| Monero | Outputs belong to the reserve: key-image signatures, RingCT amounts, and transactions linked to the certified head | Committee: the Monero head and "each key image is unspent" | Address and keys in the policy |
| Aave v3 (Arbitrum) | Collateral, debt, indices and oracle storage via Merkle-Patricia proofs against the committee-signed state root | Committee: Arbitrum state root | Contract addresses and code hashes. The assumption that the listed debt (USDC) is the owner's only debt is not verified (A1). |
| EVM spot | Native and ERC-20 balances via Merkle-Patricia proofs under committee-signed roots | Committee: each chain's state root | Token set and decimals |

**Trust class in the A666 manifests.**
`tools/nav-reserve-proof/manifests/a666/source-manifest-public-successor.json`
labels all six sources `cryptographic` for both quantity and valuation. It
sets `max_age_blocks=20` and `max_observation_span_blocks=8`, and
`profile-registration-public-successor.json` sets
`allow_controlled_sources=false`.

**What backs the live NAV today.** It is an **older StakeHub-produced SP1
proof**. It is neither the open kit's proof nor an attestation-only proof:

- `docs/status/z3-cycle1-inputs-20260922.md` line 78 reports the live profile
  as `verifier_kind=sp1-groth16`, `source_class=stakehub-six-leg-reserves-v3`.
  `docs/review/z3-g2-route-compatibility-20260917.md` line 63 gives valuation
  policy `389a52d3…`.
- For `sp1-groth16`, L1 runs `verify_sp1_groth16_with_options`
  (`crates/execution/src/nav_sp1_verifier.rs:344`). That function decodes
  StakeHub's `AggregatePublicValuesV2` totals (`decode_aggregate_v2_totals`,
  line 395), not the 584-byte `postfiat.nav_reserve_public_values.v1` decoded
  by `crates/types/src/nav_reserve_public_values.rs`.
- `qualifications/a666-public-successor-20260802/README.md` covers the open
  kit's successor. It is a proposed migration that "does not by itself
  authorize live activation". The attestation-only
  `qualifications/a666-shadow-20260730` fixture was "never registered or
  activated".
- On-chain, the live proof carries **no trust classes**; L1 reads only the
  policy hash and totals. The shadow README says the old program labelled
  NEAR quantity cryptographic. That program's NEAR logic is not in the open
  guest, so this review cannot confirm it. Its public values also carry no
  epoch or freshness binding (finding G1).

**Turning "the NEAR is there" into a cryptographic statement:**

1. Bound the receipt to the certified head so that only a fresh snapshot
   counts (N1).
2. Verify NEAR's own block-producer approvals from a governed epoch anchor
   instead of trusting the committee for the head (N3). The code has
   `verify_approvals_v1_fixture_bps` but uses it only in fixtures.
3. Prove the reader's and pool's code, and the reader's lack of access keys,
   with NEAR state proofs at the receipt block (N2).

After those changes, the remaining trust is NEAR's validator supermajority,
the pool contract's accounting and the Chainlink price.

## 2. Scope and method

This is a correctness review, done by reading code, with one scratch
reproduction. In order, it covered:

- the NEAR path, `near_receipt.rs`, the NEAR CLI adapter, the
  `near-stake-reader` contract and the guest dispatch;
- the shared arithmetic and bindings in `reserve-proof-types/src/lib.rs` and
  `bft_checkpoint.rs`;
- the Chainlink valuation arithmetic;
- the L1 decoder and its consumer, `nav_sp1_verifier.rs` and
  `nft_escrow_asset_execution.rs`.

Other adapters were read only far enough to classify their trust, freshness
and position completeness. Paths under `tools/nav-reserve-proof/crates/reserve-proof-types/src/`
are shortened to the file name.

## 3. Findings

| ID | Sev | Location | Summary |
| --- | --- | --- | --- |
| N1 | P1 | `near_receipt.rs:492–649` (ancestor check at 544) | A NEAR snapshot receipt of any age is accepted under a fresh certified head. Reproduced. |
| A1 | P1 | `aave_v3.rs:364–392` | Aave debt completeness is not proven. Borrowing in an unlisted reserve is invisible. |
| G1 | P1 | `crates/execution/src/nav_sp1_verifier.rs:344–432`; `nft_escrow_asset_execution.rs:1297–1342` | The live `sp1-groth16` reserve path binds no genesis, asset, epoch or observation time. |
| N2 | P2 | `near_receipt.rs:476–490, 559–563`; CLI `near_adapter.rs:849–865` | Reader and pool code hashes are committee-checked at the head only. The reader is a redeployable implicit account. |
| N3 | P2 | `near_receipt.rs:500–525, 548–558, 1077` | The NEAR head is trusted on committee signatures only but is counted as cryptographic. |
| X1 | P2 | `solana_stake.rs:473`; `monero_reserve.rs:376`; `lib.rs:269–285` | "Cryptographic" includes committee-attested facts and single-signature protocol receipts. |
| B1 | P2 | `bft_checkpoint.rs:67–100` | Committee validation does not reject one public key registered under two validator IDs. |
| M1 | P2 | `lib.rs:386–447, 519–600` | Nothing prevents one position from being listed under two source IDs and counted twice. |
| N4 | P3 | `near_receipt.rs:604–607` | `unstaked_yocto` is counted at full value, although it can leave the pool shortly after the snapshot. |
| L1 | P3 | `nav_sp1_verifier.rs:255` | `max_snapshot_age_blocks=0` silently disables the staleness check. |
| L2 | P3 | `nav_sp1_verifier.rs:216–295` | Public-values `valuation_scale` is not compared with the profile. |

Counts: **P1 3, P2 5, P3 3.**

## 4. Finding detail

**N1 — stale NEAR receipt (P1).**

- *Condition:* the guest checks that the proven block folds into the certified
  head's `block_merkle_root`, which commits to every ancestor block. Nothing
  compares the proven block's height or timestamp, or the payload's
  `block_timestamp`, with the head. Line 592 compares the payload only with
  its own event.
- *Observed:* a scratch test was added and then reverted. It moved the
  snapshot 90 days and 7,776,000 blocks before the historical head, re-signed
  the checkpoint and owner statement, and
  `verify_near_receipt_quantity_proof_v1` returned `Ok`.
- *Attack:* the issuer snapshots, withdraws the stake, then proves the old
  receipt under a new head. The owner signature does not stop this, because
  the owner is the issuer. Every other adapter proves state at the certified
  block itself.
- *Expected:* the guest rejects a receipt older than a governed bound.
- *Change:* in `verify_near_receipt_quantity_proof_v1`, require all of the
  following:
    - `payload.block_timestamp <= head.header.timestamp`;
    - `head.header.timestamp - payload.block_timestamp <= policy.max_receipt_age_ns`;
    - `head.header.height - proof.block_header_lite.inner_lite.height <= policy.max_receipt_age_blocks`.
- *Commitment impact:* add both new fields to `NearReceiptPolicyV1::commitment`,
  and add a negative test.
- *Governance:* this changes the guest ELF, the policy commitment and the
  source-manifest hash. It **requires a new program identity and governance
  registration**, so it was not made.

**A1 — Aave debt completeness (P1).**

- *Condition:* the verifier sums only the positions listed in the policy. The
  A666 policy lists WETH collateral and USDC variable debt. Nothing proves
  the owner's Aave `UserConfigurationMap` (the per-reserve "borrowing" bits in
  Pool storage).
- *Observed (code reading):* after registration, the owner can borrow another
  asset against the listed collateral and move it away. The proof still
  reports full collateral minus USDC debt only, so net assets are overstated
  by the unlisted debt.
- *Expected:* the proof shows that the owner has no borrowing bit set for any
  reserve outside the policy.
- *Change:* add a storage proof of `_usersConfig[owner]`, and require its
  borrowing bits to match the policy's debt reserves. Reserve IDs come from
  the already-proven `ReserveData`. This is a guest change: it **requires a
  new program identity and governance registration**. Hyperliquid already
  has an equivalent completeness check (account `ntl_pos` must equal the sum
  of the governed perps).

**G1 — live legacy path has no freshness binding (P1).**

- *Condition:* for `sp1-groth16` profiles, L1 verifies the Groth16 proof and
  then reads only the policy hash and the spot, cash and liability totals.
  `max_snapshot_age_blocks` is not applied to this proof. The only epoch rule
  is `epoch > finalized_epoch` on the packet.
- *Observed (code reading, not executed):* a previously valid aggregate
  proof with the same net assets satisfies every proof check for a later
  epoch. Only an off-chain attestation or challenge process could catch it.
- *Expected:* the proof binds the asset, epoch and observation time.
- *Change:* move A666 to the registered `sp1-nav-reserve-v1` successor. Its
  context check (lines 216–295) binds genesis, asset, profile, manifest,
  epoch, span and age. Otherwise make a consensus change. This review made no
  repair.

**N2 — code hashes at the wrong block (P2).**

- *Condition:* `source_state_commitment` commits the reader and pool code
  hashes. The committee is asked to check them at the **head**. The receipt
  can execute earlier (N1), and the pinned reader (`eed15bed…`) is an
  implicit account, whose key holder can redeploy it.
- *Expected:* the proof shows the reader's code at the receipt's block, and
  that the reader has no access keys.
- *Change:* add NEAR state proofs for the code and access keys against
  `prev_state_root`, or restrict receipts to the head's final block. This is
  a guest change and requires a new identity.

**N3 — NEAR head trust (P2).**

- *Condition:* the head is accepted when the committee certificate is valid.
  NEAR block-producer approvals are never verified outside fixtures.
- *Observed:* the public values report 100% cryptographic value.
- *Expected:* either verify approvals from an epoch anchor, or disclose that
  the head is committee-anchored.
- *Change:* adopt the light-client verification described in section 1 in a
  new identity, and document the trust in the profile until then.

**X1 — trust-class labelling (P2).**

- *Condition:* the following are all classified `Cryptographic`:
    - Solana reader quantity, whose state the committee signs (`solana_stake.rs:473`);
    - Monero unspent status, which the committee signs (`monero_reserve.rs:376`);
    - `ProtocolReceiptEd25519`, a single manifest-pinned signature (`lib.rs:273`).
- *Expected:* a signature-only fact is not reported alongside mathematical
  proofs.
- *Change:* add a committee-anchored class, or publish the per-source
  anchoring in the profile documentation. A new class needs a new ABI and
  identity.

**B1 — duplicate committee keys (P2).**

- *Condition:* `BftCheckpointCommitteeV1::validate` checks that validator IDs
  are unique, but not that public keys are.
- *Observed:* one key under two IDs casts two votes.
- *Exposure:* the A666 committee has six distinct keys (checked), so there is
  no current exposure.
- *Change:* reject duplicate `public_key` values. This is a guest change and
  requires a new identity.

**M1 — cross-source double count (P2).**

- *Condition:* the manifest checks only `source_id` order. Two entries, for
  example two NEAR policies with different `position_id` values over the same
  account and pool, both verify and are both counted.
- *Why the manifest is the only guard:* profile registration is
  permissionless (`nft_escrow_asset_execution.rs:2372`), so manifest review is
  the only control.
- *Exposure:* the A666 owner commitments are distinct (checked).
- *Change:* require unique `(source_domain, reserve_owner_commitment)` pairs
  in `SourceManifestV1::validate`. This requires a new identity; until then,
  add the rule to the manifest review checklist.

**N4 (P3).** Unstaked balance is still in the pool at the snapshot, but its
owner can withdraw it within about four epochs. Counting it is defensible
only with a fresh snapshot (N1). A haircut policy or a staked-only mode would
be more conservative.

**L1, L2 (P3).** These are governance-input hazards.

- *L1:* the successor profile sets 900 blocks, so the staleness check is
  active. Reject `max_snapshot_age_blocks=0` for `sp1-nav-reserve-v1` at
  profile registration.
- *L2:* the scale is bound indirectly, through the Chainlink policy
  commitment for A666 or through the attestor signature. A future profile
  that uses controlled valuation would leave it unbound.

**Checked and correct.**

- The snapshot payload's little-endian widths match the contract. The
  contract's callback is `#[private]`.
- `SuccessValue` is required, and it must equal the payload.
- Malformed `EVENT_JSON` is fatal, and duplicate matching events are
  rejected.
- The account must be the implicit account of the owner key.
- The policy commitment binds the committee root.
- The sum `staked + unstaked` uses checked arithmetic, and the decimal places
  are correct: NEAR 24, SOL 9, XMR 12.
- Chainlink valuation rounds down per row, keeps the haircut at or below
  10,000 bps and checks oracle age.
- The weakest class chooses the trust bucket, and liabilities may not exceed
  assets for each source.
- Monero status-set checks imply unique key images.
- The Hyperliquid receipt must be in the certified block.
- Aave rounds collateral down and debt up.
- Hyperliquid spot rows must exactly equal the governed token list, and perp
  notional must equal the account total.
- EVM spot rejects duplicate chains, tokens and positions.
- The A666 Aave and spot token sets do not overlap (checked).
- The 584-byte decoder enforces the exact length, magic, version, no trailing
  bytes and the value identities.

## 5. Repairs

None. Every P1 and P2 finding is in guest code (`reserve-proof-types`, which
is compiled into the ELF) or in consensus execution. These are not small,
local, identity-preserving changes. A CLI-only receipt-age preflight was
considered and rejected: it would not stop a dishonest prover, and it would
need an operational bound that is not yet known.

## 6. Verification

- Ran `RUST_TEST_THREADS=2 cargo test -j 2 -p reserve-proof-types` in
  `tools/nav-reserve-proof`. Result: 82 + 3 + 4 passed, 0 failed.
- Ran the N1 scratch reproduction with the same crate and filter
  `scratch_stale`. Result: `age_days=90 result_ok=true`. The test was
  reverted and is not committed.
- The committee key uniqueness and A666 owner-commitment uniqueness were
  checked with a local script over the committed manifests.

## 7. What to do next, by risk

The N1, A1, B1 and M1 repairs are prepared, unmerged, in the
[successor proposal](nav-reserve-proof-successor-proposal-20260924.md).

1. **Before A666 relies on the open-kit NEAR leg:** build a successor identity
   with the N1 receipt-age bound, the A1 debt-completeness proof and the B1
   and M1 validation rules. Register it through governance, and re-qualify
   the successor epochs. Until then, check off-chain that the reserve account
   has no Aave debt outside USDC.
2. **Retire the legacy `sp1-groth16` reserve path for A666** (G1) by
   migrating to the `sp1-nav-reserve-v1` successor. Until then, check each
   live packet's proof against its epoch off-chain.
3. **Label committee-anchored facts honestly** in the profile documentation
   (N3, X1), and plan a NEAR light-client identity (N2, N3).
4. **Add manifest-review checks** now (M1, B1, L1): unique owners per domain,
   unique committee keys and a nonzero snapshot age.
5. **Coverage:** add negative tests for receipt age and ancestor distance with
   the N1 fix.

Not reached, beyond the trust, freshness and completeness checks above:

- line-by-line review of the Solana reader parser and
  `verify_reader_transaction`;
- Monero RingCT and transaction-tree code;
- Chainlink feed-proof internals;
- the remaining CLI adapters.
