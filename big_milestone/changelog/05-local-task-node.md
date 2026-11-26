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
