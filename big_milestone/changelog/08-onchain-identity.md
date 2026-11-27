# Changelog — Pillar 8 — On-chain Identity (DID, KYC, Optional Trust Lines)

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/08-onchain-identity.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/08-onchain-identity.md).

| Reviewer item | What changed |
|---|---|
| Gemini P1 | R2 evaluates the gate live, with a precedence formula. R3 sets direction rules. The exit rule is Q1. `requires_authorization` was verified immutable (R4). Policy changes are Q3. |
| Gemini P2 | Verified: implicit lines pay no fee on `issued_payment`, set `limit = amount`, are never deleted, and limits auto-raise. Added the XRPL comparison table, R13 and R14. Fee amounts and deletion are Q8. |
| Gemini P3 | Added an enforcement table built from code, covering 16 paths: shielded pool, the fix, FastPay, fast lane, offers and fee mints. R12 handles shielded pools and lanes. |
| Gemini P4 | R11: the wrapper option is recorded on chain, return imports must stay recoverable, and later Solana and Hyperliquid venues follow the same choice. The restricted wrapper is Q7, which removes the old R6 contradiction. |
| Gemini P5 | R10: (issuer, type) pairs in bounded AND/OR rules, with an indexed lookup. |
| Gemini P6 | R7 adds the `did:pftl` method. R9 adds a passport-proof pathway. Q11 covers on-chain proofs. |
| Gemini P7 | Credentials attach to a PFTL account. Pillar 6 is an optional link, and R15 allows fresh accounts. |
| Gemini P8 | R6: accept and delete, and expiry by height. R5: deregistration handles key compromise. Q13 covers type names. |
| Gemini P9 | R15: salted commitment, coarse types, explorer listing opt-in. Pillar 2 no longer carries evidence (R9). |
| Gemini P10 | Verified: no slashing code exists for `NavAttestor` bonds. Bonds are Q5, approval and liability Q4, fees and KYC cost Q8, liquidity Q7. Credential reuse is left to B7.2. |
| Gemini P11 | R8 adds the field mapping. Acceptance 8 requires a real sandbox integration. |
| Opus P1 | One gate (R1), the 16-row table, and per-path tests (acceptance 1 and 3). |
| Opus P2 | R15 and Q6. Credentials no longer bind to the Pillar 6 identity. |
| Opus P3 | Verified the test and its contradiction with `TrustSet` and the Access panel. R0 now defines issuer-sourced credits, the holder-line flow, activation and an audit of existing lines. |
| Opus P4 | R7, R8 |
| Opus P5 | R6 adds `credential_accept`. Fees are Q8. |
| Opus P6 | R2 (expiry by height), R3, Q1. Verified that redeem, burn and escrow refunds are blocked today for de-authorized lines. |
| Opus P7 | Partly disputed: in code, Ethereum is the only NAVCoin wrapper, and the Solana bridge carries USDC series. R11 covers later venues. Q7 covers the credential bridge. |
| Opus P8 | R13, R14 |
| Opus P9 | Q4, Q5, Q8 |
| Opus P10 | Q12 |
| Opus P11 | R15 (salt), R16 (ML-DSA-65), R5 (deregistration cascade) |
| Opus P12 | Acceptance expanded: per-path matrix, expiry, override, exit rule, DID, sandbox and benchmark. |
| Opus P13 | Disputed: every reference exists on main. Links now point to exact files. |

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/08-onchain-identity.md).

| Item | Change |
|---|---|
| Gemini P1 | R0 now creates gated implicit lines with `authorized = false` and drops the issuer-source exception. Only an issuer-signed `trust_set` sets `authorized`, so revocation and expiry stay live. Acceptance 1 adds receive-by-credential, revoke, then send fails. |
| Gemini P2 | Added "What a gated NAVCoin can do" under the summary, mapping the founder's five core elements to open questions 6 and 7. Verified that the fix fills only inside a shielded swap (`consume_fx_fix_swap_plan`). Q6 gains option (d), a transparent fix path. Open questions 6 and 7 (designed to work under each option: only the shielded-entry bar waits on Q6). |
| Gemini P3 | R5: stable `issuer_id` with an admin account and rotating signing accounts, a compromise cut-off height and a deregistration grace height. Policies name issuer ids, so rotation never changes a policy. Verified that PFTL accounts and `NavAttestor` have no key rotation. Deregistration authority is open question 4. Q3 notes that a fixed policy is closed to new holders if every named issuer goes. |
| Gemini P4 | Verified: offers and escrow need existing lines even on default assets, and `offer-setup.js` sends `trust_set`. R14 creates the line implicitly on `offer_create` and escrow creation, and the wallet drops its `trust_set` step. R14 adds `holder_limit: Option<u64>`. Summary and acceptance 6 updated. |
| Gemini P5 | Verified in code: a resting maker failing the line check fails the taker's whole transaction. `offer_cancel` refunds unchecked. Clawback reaches free balance only. Issuer `trust_set` can clear `authorized`. R3 adds unlock-is-not-move, maker skip-and-cancel and clawback reach into offers and escrows. R6 defines renewal, which keeps acceptance. Acceptance 2 and 4. |
| Gemini P6 | R11 adds holds for rows 7, 9 and 10. Verified that one failing receipt rejects a whole Solana proof. R12 adds `RevokeHolderPermit` and admission checks of L1 freeze and the gate. Verified that no revoke action exists and admission ignores L1 freeze. Acceptance 7 and 8. |
| Gemini P7 | R9 adds proof of control (signed nonce, key checked against `address_from_public_key`), the account as a public input of passport proofs, one credential per nullifier and scope, and an open-source verifier as a registered issuer. Who may run it is folded into open question 4. Q10 asks whether vendors can bind a session to a nonce. |
| Gemini P8 | Verified: the fee is 10 atoms (1 PFT = 1,000,000 atoms), burned, against about 22 atoms per transfer. Solana proofs and vault claims already pay. R13 states the size, the payer per path and the distribution cost. R6 adds a refundable credential deposit, modelled on the offer reserve. Amounts, reserve vs fee, and a cleanup reward are open question 8 (designed to work under each option). |
| Gemini P9 | R10 names three bounds, starting at XRPL's 10 until the benchmark sets them, with the 64-cross taker (`MAX_DEX_CROSSES_PER_TRANSACTION`) as the worst case. R6 fixes inclusive expiry, matching `FastHolderPermitV1` and `FxFixPacketV1` (verified), with empty meaning no expiry. |
| Gemini P10 | R16 states the classical upstream boundary. Questions kept as founder decisions, without recommended answers. Each now says what it changes, and the preamble says none blocks build step 1. |
| Opus P1 | Same fix as Gemini P1. |
| Opus P2 | Holds for rows 7, 9 and 10 (R11, acceptance 7). Disputed for row 13: a shielded exit runs on a trial ledger, so a rejected exit leaves the note unspent (`orchard_state_application.rs`). Rejection stays. |
| Opus P3 | R6: revocation sets `status = revoked` and keeps the record until expiry or issuer deletion. Codes listed, with a deterministic choice when no rule passes. |
| Opus P4 | R8: the on-chain record is authoritative. The off-chain VC keeps the vendor's own DID and signature, linked to `issuer_id` in the registry. `commitment` is described as having `URI` as its nearest field. R7 notes no ML-DSA-65 cryptosuite exists yet. |
| Opus P5 | Partly disputed: `NavMintAtNav` is issuer-signed (`transactions_mempool_receipts.rs`), not subscriber-signed. Made moot because R0 drops the issuer-source exception. |
| Opus P6 | Same as Gemini P2. |
| Opus P7 | Added a three-step build order matched to the README's suggested order. R0 notes that default assets are untouched, so near-term NAVCoin swaps are not slowed. Holds ship with R0 because A666 is gated and uses the route. No sizes or durations given. |
| Opus P8 | Same as Gemini P8. Partly disputed: bridge credits do have a payer today (Solana proof submitter, vault claimer). |
| Opus P9 | R12 adds revocation and live freeze and gate checks at admission. The delay is the lane's finality lag, stated in the docs. Acceptance 8. |
| Opus P10 | R15: wallets default to a separate account for gated assets and warn before credentialing an account with a Pillar 6 identity. |
| Opus P11 | R5: deregistration grace height, holders fall under the exit rule, docs advise multi-issuer policies. Authority is open question 4. |
| Opus P12 | R4 states the consequence. One-way opening added to open question 3. R4 adds `asset_access_set` with a change rule set at creation, so any Q3 answer is a parameter. |
| Opus P13 | Acceptance 12 adds an end-to-end SDK, mint, DEX, Uniswap route and redeem scenario. |
| Opus P14 | Row 6 says "by design". R10 has a provisional bound. Live evaluation and fast-lane permits are reconciled in R12. Disputed for the filename: `identity-flag-pipeline-spec-20261023.md` exists on main and is dated before this review. |
| Opus P15 | Disputed: revision notes were already moved to this changelog. The spec has none. |
| Other | Code references now include `core_chain.rs`, `execution_actions.rs`, `crypto_provider/src/lib.rs` and `exchange-guide.md`. Fee constants are cited in `entrypoints.rs`, where they live. Row 16 now follows Pillar 7 R9 (`nav_fee_claim`) instead of fee mints. |
