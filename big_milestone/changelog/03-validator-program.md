# Changelog — Pillar 3 — Validator Rewards Program

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/03-validator-program.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/03-validator-program.md).

| Item | Change |
|---|---|
| Gemini P1 | Admission path corrected (G0, Policy V1, registry-quorum ratification); R4–R7. The premise was partly wrong: self-service join was never the design, but the page had implied it. |
| Gemini P2 | Economics section; R15 access states. Verified that access depends only on binding. Open questions 1, 2 and 4. |
| Gemini P3 | R3 rewritten (classify, root, stop then back up, paired restore, guard untouched); R3a. Signing uses one ML-DSA-65 key; m-of-n is open question 9. |
| Gemini P4 | R0 steps; default flip in R14 and open question 11; old-chain access kept throughout. |
| Gemini P5 | Feed verified live but unsigned; R16. Overstated: a stale feed blocks binds and grants, not existing access. `top` reads the same feed (R12). |
| Gemini P6 | RPC rows corrected; R10 adds profiles, checkpoint sync, follower mode and latency. |
| Gemini P7 | Step 6 corrected: the script uses the network but the signer doesn't. Security table; R13. Single-use 30-minute token and Unbind confirmed in code. |
| Gemini P8 | Gate table; interfaces to Pillars 1, 2, 5, 6 and 9. |
| Gemini P9 | R11: tabs need `pymdownx.tabbed`, otherwise sections; postfiat.org face; hardware and RPC usage. |
| Opus P1 | R0 steps; open question 11. |
| Opus P2 | R3 classification; R3a activation heights. |
| Opus P3 | Verified the signer guard exists. R3 and R9 exclude it from backup; acceptance 3. |
| Opus P4 | Admission floors, removal and caps stated; R6 and R7; open questions 6 and 7. |
| Opus P5 | R8; acceptance 8. |
| Opus P6 | Security table; open question 9. |
| Opus P7 | R16; failure list. |
| Opus P8 | Verified $20 Task Node credit per validator id; open questions 1, 2 and 12. |
| Opus P9 | R15; Pillar 9 interface; open questions 4 and 5. |
| Opus P10 | R9 Keys section. |
| Opus P11 | Verified: `account_info` doesn't exist and `tx` does. Acceptance 4 uses `account` and `tx`. Checkpoint anchor in R2 and R10. |
| Opus P12 | Verified: only key rotations were ratified live. Stated; R7 rehearsal. |
| Opus P13 | Story step 9; R18. |
| Opus P14 | R11 postfiat.org page. |
| Opus P15 | Disputed: founder rule, steps not dates. The R0 steps and the gate table give the order. |
| Coordinator (Pillar 9) | Research-access row corrected. Finance Hub bundles and validator research API keys need a binding on either chain, and chat Deep Research is open to all. The registry is checked only at bind time, and participation affects credit only. |
| Coordinator (Pillar 5) | Private-action admission added to the RPC row, R10 and the Pillar 5 interface; reward treatment is open question 10. |
| Opus P16 | R11: measured hardware and cost; none committed today. |
