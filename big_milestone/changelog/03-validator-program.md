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

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/03-validator-program.md).

| Item | Change |
|---|---|
| Gemini P1 | Verified: no stand-alone follower exists (`rpc-catch-up` is one-shot, default 64 blocks). Non-in-process reads spawn a child per request and queue on the store lock that block commits hold. Intro answer qualified; R10 builds follower mode; public profile moves to a follower host, with admission forwarded through the edge; read path follows the fast-reads note; target is open question 11 (designed to work under each option); acceptance 5. Partly disputed: admission signs only consensus contexts and already works through the signer socket, so it adds no key domain. |
| Gemini P2 | Keys table. Verified: the binding signs with the consensus key file and fails in signer mode, since the signer refuses non-consensus contexts. Rotation breaks grants (`l1_validator_key_not_current`) and blocks re-binding. R13 adds application contexts to the signer; R15 keys access by validator id and follows ratified rotations; Step 1 signer soak and acceptance 3; binding key is open question 12 (designed to work under each option). |
| Gemini P3 | R5b candidate observation through a follower supplies `uptime_window_bps` (field registry: network-observed, 7 days). R5a validator-side ratification client (commit-reveal, local Policy V1 and Cobalt recheck). Z2 explained in the gate table. |
| Gemini P4 | Open questions regrouped by what they block; participation-metrics table explains the 99.5% admission floor against the 95% reward rule; testnet reading made independent via separate switches; endpoints are open question 8. Partly disputed: no default package is picked, by founder rule; every mechanism takes the answer as a parameter. |
| Gemini P5 | R16: each observer signs its own feed, Task Node accepts k of n, one observer may fail, disagreeing days don't count; acceptance 7. |
| Gemini P6 | R3: free-disk pre-check, safe-rollout snapshot, one retained rollback pair, no rollback after H, refusal if a key file sits in the data directory. Partly wrong: the separate signer user and guard directory already exist in the signer policy; R1, R3 and R9 now require that layout. |
| Gemini P7 | R12 reads loopback status and never opens the store (verified exclusive lock); panels add membership, ratification, binding and role counts; degraded-feed mode; R12a alert delivery from the existing spool plus optional metrics endpoint; acceptance 8. |
| Gemini P8 | R13 signer mode needs neither root nor `unshare`; offline mode kept. R1 builds the offline commands for macOS on Apple silicon if they compile, else names the offline environment. |
| Gemini P9 | Acceptance 1 rewritten (no Foundation tooling or out-of-band help). Added acceptance 3 (signer soak), 5 (follower, edge admission), 6 (rotation), 7 (k-of-n), 8 (non-root, degraded), 10 (governance record). |
| Gemini P10 | Pillar 6 made a soft dependency; link emitted when Pillar 6 is live and added later by re-running. Testnet access and grants are separate switches (Economics, R15, R17). |
| Opus P1 | Step 3 "Invited operators" added. Verified Gate Zero is the operator's recorded rule and Z3's reason (no consensus churn before strangers run the binary). When Step 3 starts is open question 1 (designed to work under each option: step order fixed, only the start gate moves). |
| Opus P2 | Verified: L3 has no Foundation carve-out, and `G0` allows no inserted seats. Admission row states it; Step 4 counts `G0` groups before outreach; old-chain round sizes cited; open question 2 (designed to work under each option). |
| Opus P3 | As Gemini P1: follower/edge topology keeps validators off the public internet. Disputed: no key-domain reuse; admission signs ordinary consensus messages. |
| Opus P4 | Open question 4 adds the sunset option; Economics names the missing pull to the new chain. No default picked (founder rule). |
| Opus P5 | Open question 8; Step 3 gives Pillar 5 endpoints early; Economics gap stated. |
| Opus P6 | Participation-metrics table: one vote-based metric for members, follower-based for candidates, thresholds in one versioned policy; open question 7. |
| Opus P7 | RPC row: root cause from the fast-reads note (child process per request, lock queueing) and current figures; R10 read path; open question 11; acceptance 5. |
| Opus P8 | Economics: detection signals (placement preflight for provider, ASN, country; declarations and domain evidence for ownership; unresolved fails) and the residual risk stated. Bond in open question 3, per-operator rewards in open question 6. |
| Opus P9 | Later applicants start as followers (R5b); `top` shows the ratifier-margin alert; admissions per transition is a payload parameter. |
| Opus P10 | R3 snapshot, disk check and retention; R11 sizes snapshot disk; acceptance 4 publishes missed blocks; excusing upgrade windows is in open question 7. |
| Opus P11 | R16: at least one non-Foundation observer, any RPC operator may join, recompute script published. |
| Opus P12 | R12 panels, colour states, banner, keybindings and first-run view. |
| Opus P13 | Story step 9 and interfaces list the roles: admission, public follower, observer, optional pinning for Pillar 2. |
| Opus P14 | Intro ties admission to post-human governance; verdicts and ratifications published as records for Pillar 1 and the explorer; diagram node; acceptance 10. |
| Opus P15 | Criteria made checkable (published figures, injected failures, structural pass/fail). Partly disputed: numeric thresholds not invented; latency target is open question 11 and the soak window is set in advance. |
| Opus P16 | R11: postfiat.org page generated from the single MkDocs source, with a chain selector; old-chain page links to it. |
| Opus P17 | Open question 6 option (a); Economics "Testnet"; separate switches let either reading work. |
| Coordinator (Pillar 9) | Access rule renumbered to open question 4 to match Pillar 9's cross-reference, with a ramp-in option (d); open question 5 points to Pillar 9's question 2; *lapsed* name and meaning kept and reused for Pillar 9 grace; a stale feed never revokes access. |
| Self-check | Rewards row corrected to code: ≥ 25 observed days with mean participation ≥ 95%, not 25 days each at 95%. Binding row: the chain id is also pinned by a database constraint (R13 migrates it). |
