# Changelog — Pillar 5 — The Task Node Local Experience

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/05-local-task-node.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/05-local-task-node.md).

| Item | Change |
|---|---|
| Gemini P1 | Checked the code: the UI is native SwiftUI with no WebView, so no remote-script risk. Parity gap: R12. Approach: open question 3. |
| Gemini P2 | Fees, assets and proof size taken from code. New *Economics* section. Fee level: open question 8. |
| Gemini P3 | Found the Mac PFSH03 port (PR 39): R1–R2. Orchard not shipped: R1, R21. Audit and capped beta: open questions 10 and 5. |
| Gemini P4 | Partly disputed: cheap checks first, proof-of-work, size limits, the switch and the block cap all exist. Added failover (R9), published limits (R10), a flood test (AC 5) and validator incentives (open question 8). |
| Gemini P5 | ARM64 kernels and an M5 measurement exist. Base-Mac measurements: R3. Budget left to the founder. |
| Gemini P6 | New *Privacy model* section. Added R15, R16 and R17. |
| Gemini P7 | R13 and R14 (PFSH03 has no memos). Open questions 6 and 7. |
| Gemini P8 | R4–R7. Found that the app and the daemon derive keys differently. PFSH03 has no outgoing viewing key. |
| Gemini P9 | Checked: the code refuses other assets. Open question 9. |
| Gemini P10 | Both clients already share the wallet crate (R2). Platforms: open questions 3 and 4. |
| Gemini P11 | Added acceptance criteria 3–11. |
| Gemini P12 | Removed the local path. Private-repo paths kept as plain text, with the key facts written into the page. |
| Opus P1 | The port exists (PR 39). R2 merges it against merged L1 code. |
| Opus P2 | Corrected: 2.89 s was on 32 threads, not 16 cores. R3 is per action and per machine. |
| Opus P3 | *Privacy model* states that one operator runs validators and Task Node. R16 sets the wording. |
| Opus P4 | Fees from code; R10 and R7. |
| Opus P5 / P6 | R14 / R13. Open questions 6 and 7. |
| Opus P7 | Pillar 1 dependency added. AC 10 lists what the activation packet contains. |
| Opus P8 | R18 and R19. Open questions 5 and 10. |
| Opus P9 | Open question 2. |
| Opus P10 | Open question 9. |
| Opus P11 | R21. Open question 11. |
| Opus P12 | R4–R5. PQ viewing key: open question 9. |
| Opus P13 | AC 1, 10 and 11. Value cap: open question 5. No soak duration, because the plan carries no timelines. |
| Opus P14 | References checked again. W-1 was wrongly described as planned; it is built on main and not deployed. |
| Opus P15 | R8 and R17. |
| Opus P16 | R12. Open question 3. |

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/05-local-task-node.md).

| Item | Change |
|---|---|
| Gemini P1 | Checked the code. The cap of four per block is a compiled-in constant (`PQ_REHEARSAL_PROOFS`, 640,000 bytes), and the fee is set by amendment. *Economics* now covers capacity and valid-action spam. AC 5 adds sustained full valid blocks; R3 measures their cost. Fee rule, cap and validator pay: open question 8 (designed to work under each option: the app takes the fee quote from the snapshot, and receipts record the endpoint for attribution). |
| Gemini P2 | Checked: FastPay carries PFT only, as prefunded objects that are unwrapped into an account (`settlement-lanes.md`). The story's payout is now 100 PFT. R13 makes *Make private* one user action with two recorded steps (unwrap, then shield), with a retry if the shield fails. The FastPay lane is in the diagram, and AC 8 covers the interrupted case. |
| Gemini P3 | Checked PR 232: two notes in and two out, both inputs under one root, cross-epoch merge by self-payment, and no exit alone for a note no larger than the fee. New R23 covers note selection, pending change, explicit consolidation and dust. New AC 7. |
| Gemini P4 | Checked the Mac app: its payment signer refuses memos (`core/src/codec.rs`). R14 now requires memo signing in Pillar 2's format. AC 9 tests today's workflows and an encrypted-memo public payment. Parity launch set and approach: open question 3 (R12 and AC 9 work under each option). |
| Gemini P5 | PFSH03 has no outgoing viewing key; this is now in the body. New R24 adds an encrypted sent-payment journal. R5 separates phrase-only restore (rescans from activation; no in-flight proofs or sent history) from vault-backup restore. R3 adds a restore-scan measurement. AC 4 extended. |
| Gemini P6 | Checked: consensus keeps 256 roots per tree. R7 retries identical bytes only while the anchor is retained and the fee is unchanged. Otherwise the intent becomes `expired` and is replaced by a fresh proof that shares a real input, so at most one settles. AC 6 adds anchor expiry. |
| Gemini P7 | Partly disputed: the state root already commits each output record with its ciphertext digest, so withholding or corrupting ciphertexts is detectable. R6 now checks output records and ciphertext digests against a pinned state root, as well as tree roots. AC 3 adds swapped ciphertexts. Per-asset floor: R16 (each asset is its own pool, confirmed in code). |
| Gemini P8 | Checked the Mac code: Keychain items are already non-synchronising and this-device-only in team-signed builds, `zeroize` is used, and the disabled Developer ID job signs with the hardened runtime. R11 now requires library validation, signed updates and a protocol-version check. R10 turns the compose legacy switch off (it is the proxy's legacy shielded certification, not `pftl_swapd`; *What exists* corrected). Backup exclusion not adopted: the vault is encrypted at rest, and its backup carries the intents and the journal. |
| Gemini P9 | Open question 5 now states that a soundness bug risks the asset's whole pool reserve, and that no pool-value cap exists in code (a beta needs a consensus-enforced ceiling). The receipt in the story and R9 name the endpoint's operator. Whether non-Foundation endpoints gate mainnet: open question 1. |
| Gemini P10 | Checked: hosted context pinning goes through the Task Node server and requires its key as a reader (`server/context-publish.js`). R14 keeps that for public context. Memos tied to private payments never pass through Task Node (also R8, R17, AC 2). Pillar 2 format named in R14. |
| Opus P1 | R12 marks Finance Hub web-only (as Pillar 9 R25 decides) and leaves the rest of the launch set to open question 3. AC 9 tests every native item. Disputed in part: no option is recommended, because plan rules leave that choice to the founder. |
| Opus P2 | *Economics* now gives capacity from the rehearsal round (four proofs, about 1.03 s) and the congestion behaviour. Fee rule options: open question 8. AC 5 measures full blocks. |
| Opus P3 | Open question 8 (fee share, Pillar 3 credit, or none). R9 records the endpoint, so either reward can be attributed. |
| Opus P4 | Open question 6 kept, now designed to work under each option: R14 puts memos behind one interface, and public memo payments meet the readiness gate under every option. Disputed: no recommendation, per plan rules. |
| Opus P5 | Open question 1 adds a relay or anonymising transport (Tor or OHTTP) run outside the Foundation. It also asks whether non-Foundation endpoints are required before "private". The privacy model now says validators see only the forwarder. |
| Opus P6 | New *Build order* (Steps 1–4, no durations), with auditor engagement in Step 1 as the critical path. |
| Opus P7 | Checked: `docs/quantum/private-payments.md` says its benchmarks were measured on PFSH02. Size and verifier figures are now labelled PFSH02, and R3 measures the PFSH03 verifier and proof size. |
| Opus P8 | R4 now picks phrase-derived keys for every client, with domain separation. The daemon gains phrase import, and existing random-seed wallets restore from their backups. |
| Opus P9 | Open question 4: Gate 2 includes "mobile qualification", and a Mac-only activation packet must scope it to the Mac. |
| Opus P10 | Checked: the chain domain, the pinned series and the cap are compiled-in constants, and the pinned pfUSDC series is the rehearsals' development series. R1 puts production values in a reviewed release and in the activation packet. Which production series: open question 9. |
| Opus P11 | R16 applies the floor per asset. Open question 5 asks what happens if a pool never clears it (label stays, Foundation cover traffic, or wait). |
| Opus P12 | *Economics*: a shield's fee is public PFT, and Task Node payouts are PFT. A same-asset shield fee: open question 8 (protocol change). |
| Opus P13 | R22 adds local agents paying through the daemon socket under a user-signed spend policy, if open question 4 asks for it. |
| Opus P14 | The "Bubbles up to" line no longer repeats the outcome title. Revision notes already live only in this changelog. |
| Opus P15 | Disputed: plan rules keep these choices with the founder, so no defaults are picked. Instead, every open question now says which mechanism works under all of its options and which requirements its answer changes. |
| Cross-pillar (Pillar 7) | Checked the pNOK fix demo: fix fills settle as private swaps through Asset-Orchard. *Economics* now says so, and that the PFSH03 pool admits no NAVCoin at launch. Pillar 7 charges fees by minting fee units, which dilutes shielded units equally, so open question 9 no longer asks for a shielded-fee rule. Open question 11 notes that retiring Orchard needs a new home for fix fills. |
| Cross-pillar | R9 now matches Pillar 3: public RPC follower hosts forward `private_action_submit` and `private_action_receipt` to a validator's admission, and no validator listens publicly. The architecture and sequence diagrams show this. R13 matches Pillar 2 R26 on balances. R12 matches Pillar 9 R25 (Finance Hub web-only). |
| Numbering | R1–R22 keep their topics, since other pages cite R13, R14 and R15. New: R23 (note handling) and R24 (sent-payment journal). New: AC 7 and AC 9. Old AC 7 is now AC 11, old AC 9–11 are now AC 10, 12 and 13, and AC 8 is unchanged. |
