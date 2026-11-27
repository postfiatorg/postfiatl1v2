# Changelog — Pillar 1 — Hive Chat Governance Module

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/01-hive-governance.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/01-hive-governance.md).

| Item | Change |
|---|---|
| Gemini P1 | Rulings through the Verifiable Constitution lane (R27), a stage table, provenance labels and AC6. The old GPU criterion now covers the chat layer only (AC7). What the GPU migration refers to is an open question. |
| Gemini P2 | Inclusion rule (R12), pinned bundle (R13), ingest from session start (R11), AC4. Partly disputed: non-members cannot pollute Postgres, because ingest drops pubkeys outside the directory. |
| Gemini P3 | The steward's wallet signs anchors (R22), and approval is signed over the hashes (R20). |
| Gemini P4 | Versioned collector with per-field sources (R4–R5). The snapshot is split from the brief (R6). |
| Gemini P5 | One room per session, and the toggles are enums (R8–R9). The main room is never locked. |
| Gemini P6 | Event-driven facilitator (R15), board managers through the pre-read queue (R16), prompt boundary and validator (R17, R19). The latency target is an open question. |
| Gemini P7 | One kind, with the schema id in field 4 and the session id in field 6 (R23). JCS JSON is authoritative (R14). |
| Gemini P8 | Proposed Wanted items with rewards (R28), follow-up (R29), an Economics section and limits. The policy values are open questions. |
| Gemini P9 | Fixed schedule (R1–R3) and a pre-read state. Durations are open questions. |
| Gemini P10 | New states: Missed, Cancelled, Expired and AnchorFailed. Idempotent anchoring (R25). Partly disputed: Pinata is not the only store, because a queue re-pins to the first-party cluster. |
| Gemini P11 | Dependencies arranged by step, with a Step 1 MVP that needs no other pillar. Blog post added (R31, AC9). |
| Opus P1 | The fixed timeline is now a requirement (R1–R3, AC1). Only its values are open. |
| Opus P2 | As Gemini P4. Fields with no machine source are marked `steward_supplied`. |
| Opus P3 | Stage table, labels and rulings (R27). |
| Opus P4 | As Gemini P2. The signed open event defines who is eligible (R9). |
| Opus P5 | As Gemini P7. Proposed value 12. |
| Opus P6 | Checked: pointers go to the old chain, and the explorer reads only the new chain. The writer can target either chain (R24), and the choice is an open question. |
| Opus P7 | Checked: the 60-second interval is configurable, and a reply can take up to 180 seconds. R15. |
| Opus P8 | As Gemini P5. |
| Opus P9 | R17–R19, AC8. |
| Opus P10 | R21 is limited to the chat layer. R27, AC6 and the Economics section cost the rulings. |
| Opus P11 | RFC 8785, receipt order and a verifier (R12, R14, R30). Checked: `pinContextIpfsJson` uses `JSON.stringify`. |
| Opus P12 | As Gemini P10. |
| Opus P13 | Posting policy and rate limits (R9–R10). Incentives and gating are open questions. |
| Opus P14 | Public page and archive with no login (R30), blog post (R31), AC2 and AC9. |
| Opus P15 | Re-checked at `204d238`. Corrections: board managers reply asynchronously under the bot key; Wanted items have no draft state; the JSON cap is 16 MiB; pointers are on the old chain. |
| Opus P16 | Signed M-of-N approval (R20) and wallet-signed anchors (R22). The steward set is an open question. |
