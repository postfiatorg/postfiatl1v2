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

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/01-hive-governance.md).

| Item | Change |
|---|---|
| Gemini P1 | Ruling request schema with answer options and `hold-no-op`, evidence limited to snapshot, brief, steward events and approved minutes (R27). Ruling states and cost cap (R27a), `Acting`/`Closed` states. Signer contradiction resolved: a ruling signer (steward wallet or registered runner key) anchors rulings and validity rests on replay hashes; which signer is open question 6 (designed to work under each option). AC6 is Step 1 with one capped rental; Step 3 is only the cheap recurring home. |
| Gemini P2 | Steward set binds wallet and Nostr keys by signature (R20a); eligibility list pinned and hashed in the open event (R9); verifier split into deterministic and best-effort `--check-relays` modes (R30, AC3); security table states that receipt order is a server claim the steward approves. |
| Gemini P3 | Two isolated drafting passes (R17), verbatim `quote` checked against the cited steward text (R18, R19), decisions may cite only steward events or brief items, review screen shows item beside quote; AC8 covers spoofed citations. |
| Gemini P4 | Verified `pftl-pointer.js`: schema is an integer varint, CID is text, kinds 1–11 and 99 only (12 must be added). Verified new-chain limits: 256 data bytes per memo and **512 per payment in total**, lower-case hex. Byte budget computed (about 102 bytes per memo, three per payment) (R23). Pillar 2 sanctions `CANONICAL_JSON` bodies, so JSON stays the one canonical form (R14). Archive record for old-chain history (R24). |
| Gemini P5 | Pre-read questions are signed events tagged `pre_read`, with per-pubkey caps, accepted into the agenda by the open event (R8a, R12). Freeze sweep until EOSE with `recovered` events (R11). Checked: relay queries cap at 200 events, so ingest must page. |
| Gemini P6 | Facilitator context contract with field-path citations (R15a); triggers only on steward turns, single job queue, starts at local receipt, UI status (R15). |
| Gemini P7 | One close-out payment carries approval, transcript and minutes (R22, AC2). Retry signs a new transaction with the same memo bytes (R25, state diagram). Gateway check replaced by Pinata and cluster pin confirmation (R25). Partly disputed: CID parameters are already fixed in code (CIDv0, dag-pb, raw leaves off) and the first-party pin rejects a CID mismatch. |
| Gemini P8 | Bracketed `status` reads with state-root check, multi-endpoint `chain_read` trust level, raw responses pinned separately and failing loudly over the cap (R4). Checked: the documented RPC reads take no height parameter, hence the bracket. Cobalt now read from the machine-readable `component-observations.json`, with a `stale` flag (R5). |
| Gemini P9 | Board budgets and governance totals in the snapshot (R5); `proposed` in follow-up states; expiry option (R1, R28); per-task cap and session ceiling enforced (R19); attribution recorded (R18); cost formula and cap (Economics, R27a). Incentives and board ownership are open questions 10 and 12. Partly disputed: board budgets already exist (`board_reward_budgets`, `board_reward_spend`). |
| Gemini P10 | Steward set artifact with versions and roles (R20a), `CollectingApprovals` state, edits void signatures (R20); security table states the single-key risk honestly. |
| Gemini P11 | Moderation keeps event id and reason, drops content (R13a). |
| Opus P1 | Machine brief (R6a), rulings on flagged decisions in Stage 0, public page leads with machine artifacts (R30), stage-criteria options in open question 7. Whether every session must produce a ruling is open question 8. |
| Opus P2 | Ruling request schema, evidence binding and two-host replay rule specified (R27); cost-reduction options added to open question 8. The framework's wider formalisation stays cross-pillar (README). |
| Opus P3 | As Gemini P1; ruling signer participant added to the flow diagram. |
| Opus P4 | `trust_level` per field (R4); Cobalt field sourced from operator records and labelled `operator_attested`; AC3 reworded. |
| Opus P5 | As Gemini P2. |
| Opus P6 | Checked `hive-group-worker.js`: ingest drops non-directory pubkeys. Session ingest now accepts the eligibility list (R11); AC4 tests an allowlisted key outside the directory. |
| Opus P7 | New-chain path specified: self-payment of the smallest non-zero amount, lower-case hex memos, `accepted: true` receipt, via `wallet_sign_payment_v2` (verified present, no caller) (R24). Chain choice stays open question 5 (designed to work under each option). |
| Opus P8 | Two wallet signatures per session plus rulings (R22, AC2). |
| Opus P9 | As Gemini P4: one canonical form via Pillar 2 `CANONICAL_JSON` envelopes (R14). |
| Opus P10 | Acceptance criteria tagged by step; AC6 is a capped one-off rental. |
| Opus P11 | All open questions (designed to work under each option): each now sets a value or picks a built option, and none blocks Step 1. Recommended defaults not given, since these are founder decisions. Step 1 scope listed by R-number in Dependencies. |
| Opus P12 | Embargo with hash commitment and signed reveal, plus a disclosure check (R7). |
| Opus P13 | Cost formula, per-session publication of anchor and inference cost, reward ceiling and per-task cap (Economics, R19). Disputed: board budgets exist in code. |
| Opus P14 | `Acting`, `Closed`, `CollectingApprovals` and `Missed → Scheduled` added; ruling states in R27a. |
| Opus P15 | Checked: API posts get a `sequence` only on relay acknowledgement. Session channels stamp receipt at first sight and trigger the facilitator then (R10, R15); agenda wording drafted in pre-read. |
| Opus P16 | Dedicated facilitator key, named in the open event, always eligible (R15). |
| Opus P17 | Spot-checked paths, `HIVE_GROUP_ID`, the 16 MiB cap and the status enum; all hold. The `20260901` results file exists. L1 commit `5936d4563` pinned. |
| Opus P18 | Disputed: the page already had no revision-notes section; notes live in this changelog. |
| Coordinator (cross-pillar) | Hashes inside governance records are SHA3-384 as in Pillar 2 R5; only CIDs use SHA-256 (R14). Pillar 3 admission verdicts and ratifications added to `chain_state` (R5). |
