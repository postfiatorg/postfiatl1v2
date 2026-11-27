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
