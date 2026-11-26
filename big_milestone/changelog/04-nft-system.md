# Changelog — Pillar 4 — The NFT System

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/04-nft-system.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/04-nft-system.md).

| Item | Change |
|---|---|
| Gemini P1 | Verified that the user is the old issuer and that only the owner (or a burnable issuer) can burn. Retirement now comes before re-issue (R7), and duplicates are blocked on chain (R8). The mode is an open question. |
| Gemini P2 | R2: one signed envelope, with SHA3-384 over those exact bytes. JSON is display only. Test vectors added. |
| Gemini P3 | Semantics documented from code. Partly disputed: mint goes straight to `owner`, and ids are issuer-scoped. The name-spoof risk is handled in R15. Flags are an open question. |
| Gemini P4 | Threat-model section (R3) and claim-window close (R9, open question). Partly disputed: prompts are encrypted to Task Node keys, so a transfer needs no re-encryption. |
| Gemini P5 | Fee and reserve facts taken from code. Economics section added. Payer, pinning, fees and spam are open questions. |
| Gemini P6 | Inventory first (R5), a generic `origin.chain`, extensible media (R1), a "smart" section and a scope question. |
| Gemini P7 | Dual-read inventory (R11). The wallet signing gap was verified (R13). |
| Gemini P8 | `mint_tx` is optional (R5), and origin is checked in the new-chain explorer (R15). Partly disputed: Task Node already stores mint hashes. |
| Opus P1, P2, P3 | Burn-first or snapshot before mint (R7). The memo overclaim is removed. |
| Opus P4 | R6: proof through the retirement memo or `signWalletChallenge`, plus an ML-DSA claim. |
| Opus P5 | Corrected: 79 is the number of CIDs, not of NFTs. Inventory step and scope question added. |
| Opus P6 | Issuer powers stated. The issuer model is an open question. |
| Opus P7, P8 | Same changes as Gemini P4 and P2. The CID is a locator, and SHA3-384 is the commitment. |
| Opus P9 | "Smart" section, plus an open question. |
| Opus P10 | Per-user switch (R12). |
| Opus P11 | Economics section, plus open questions. |
| Opus P12 | R18 tests and multi-model review, acceptance criterion 7. |
| Opus P13 | Dependency states added. Disputed: the new-chain explorer exists. R14 is now conditional. |
| Opus P14 | Spot-checked. Collection flags separated out, the field is named `metadata_uri`, and the inert `ISSUER_BURNABLE` is noted. |
| Opus P15 | All five decisions added to Open questions. |

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/04-nft-system.md).

| Item | Change |
|---|---|
| Gemini P1 | `origin` now carries `claim_cid` and `evidence_cid`; the claim names the exact target `nft_id`. R17 marks *checked* only if the claim hash, ML-DSA signature, NFTokenID, target id, first owner (from the `account_tx` mint row), evidence and image CID all agree, and no earlier mint holds the same NFTokenID. |
| Gemini P2 | R7: a retirement counts only with `tesSUCCESS` in a validated ledger and signed by the prior holder; issuer burns with an `Owner` field never count (old NFTs are `Flags: 9`). R8: burn-first state machine with preflight, claim pinned and stored before the burn, and a recovery path. Verified `try_sign` is randomised. Verified `signWalletChallenge` signs arbitrary bytes, so R6 (ii) uses Pillar 6 R14's domain-separated digest; R6 (iii) adds a memo-carrying no-op transaction for external wallets. |
| Gemini P3 | New "Issuer settings": `migration_issuer` and `profile_issuer`, each Task Node or user; R9, R14, R15 and AC5/AC7 state what each needs. Distinct collection ids for migrated, new and record items (R4). Open question 3 (designed to work under each option). |
| Gemini P4 | Verified: lookups are linear, and `execute_nft_transaction` copies the ledger and runs `validate_nft_state` over every NFT on every NFT transaction; `account_nfts`/`issuer_nfts` have no cursor. R18 makes keyed storage, incremental checks and cursors unconditional before Stage 2. R19 adds default followed-issuer display; the chain rule is open question 12 (designed to work under each option). |
| Gemini P5 | R10 evidence bundle pinned at retirement and committed at mint, so origin verifies without the old chain (AC9). Partly disputed: `account_tx` already returns NFT rows with `nft_id` (NFT-005) and the explorer has a block indexer; R17 builds history from those. |
| Gemini P6 | R12 picks one design: a non-transferable, holder-burnable record NFT with `pf.nft.provenance.v1`, "as of ledger" owner and an *Owner changed since record* label. |
| Gemini P7 | R9: on `duplicate_nft`, read `nft_info` and compare origin to tell skip from collision. R2 now follows Pillar 2 R5's deterministic encoding, bans `map`/float fields, and limits unknown fields to display. |
| Gemini P8 | Fees now stated in atoms (1 PFT = 1,000,000 atoms; a mint is a few dozen atoms) from code and the faucet runbook. R2 applies Pillar 2's 1 MiB envelope cap to fetches; R20 tests over-cap envelopes. Spam price and pinning/media limits are open questions 13 and 7 (designed to work under each option). |
| Gemini P9 | XRPL parity line corrected (no offers, brokered sales or percentage royalties). Recommended defaults not added: those are founder decisions; instead each question now states trade-offs, and the mechanism works under every option. Snapshot excluded for XRPL mainnet (R7, open question 1). `ISSUER_BURNABLE` is never set and shows *no effect* until decided. |
| Gemini P10 | R5 flags bad or non-IPFS URIs as *Needs review*, keeping the raw URI. `source_chain` fixed as `xrpl:<NetworkID>`; verified the old chain's default NetworkID 2025 in `pftl-submit.js`. Build order added as Steps 1–6 (no durations). |
| Opus P1 | Same as Gemini P3, plus the claim binding to one `nft_id` and earliest-wins rule, so uniqueness holds across issuers. AC5 now holds under both settings. |
| Opus P2 | Same as Gemini P5 (R10, AC9). |
| Opus P3 | Same as Gemini P2 (R8 states and recovery; AC6 forces failures after a burn). |
| Opus P4 | Same as Gemini P4 and P8. Owner reserve or higher fee: open question 13 (designed to work under each option). |
| Opus P5 | Dependencies name the Pillar 2 public-envelope subset NFTs need first; R15 lets the founder's batch run from the Python CLI; Build order Step 1 needs no Pillar 2. |
| Opus P6 | R13 self-serve claims and cohort report; AC8 covers a non-founder claim and the disposition of unclaimed items; disposition options in open question 8. |
| Opus P7 | R4 and AC3: full devnet dry run and founder sign-off of flags before the first new-chain mint. |
| Opus P8 | Separate collection ids for migrated and new mints (R4); new-mint serials derive from the generation run id (R14). |
| Opus P9 | Same as Gemini P6. |
| Opus P10 | Post-quantum boundary table added to the Security model and required in the spec (R3). |
| Opus P11 | Open question 10 now cross-links Pillar 8's native credentials and Pillar 9; issuer burn stays open question 11. No default chosen: founder decision. |
| Opus P12 | Build order Steps 1–6 with dependencies. Disputed in part: effort estimates and timelines are excluded by the planning rules. |
| Opus P13 | Already done in revision 1: revision notes live in this changelog. |
| Opus (gap, no number): Lean model | R20 model-checks the R8 state machine for "at most one live, checked copy". |
| Cross-pillar | R2 no longer refers to Pillar 2's `envelope_hash`, which Pillar 2 dropped (its R3); the commitment is SHA3-384 per Pillar 2 R5. |
