# Changelog — Pillar 10 — Information Hive Mind

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/10-information-hive-mind.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/10-information-hive-mind.md).

| Reviewer item | Change |
|---|---|
| Gemini P1 | R6–R9 (forbidden questions, attestation, screening, asset-class modes). HBM-audit example replaced with public-price questions. Legal scope is open questions 2–5. AC3. |
| Gemini P2 | Deferred rewards (R18), caps and eligibility in code (R19), reviewer-agent triage (R15). Amounts and budget are open question 8. |
| Gemini P3 | Verified: the profile prompt reads Deep Memory and the context document. Contributor brief (R4), private match reasons (R5), wider AC4. |
| Gemini P4 | Interviews in Task Node chat, brief limited by access (R11), encrypted packets (R17), AC5. Access is open question 10. Correction: Finance Hub is validator-only today; top-30 is planned in Pillar 9. |
| Gemini P5 | Claim store with clusters, consensus and contested states, anonymous corroboration (R10, R14). AC6. |
| Gemini P6 | "Consistency" replaced by stance, evidence and corroboration (R13). Check events and verified or falsified status (R12, R18). |
| Gemini P7 | Verified: 158 serves an operator model-editing agent and lacks contributor fields. Schema delta (R12), coverage manifest (R1), research-host bridge (R16). |
| Gemini P8 | Areas plus information types (R2). Tags plus embeddings via recommended-connections (R3). |
| Gemini P9 | Roles and fallback queue (R15). Open questions 1 and 9. |
| Gemini P10 | Verified pseudonym and an honest anonymity statement (R21). Open question 11. |
| Gemini P11 | Eligibility (R19, open question 7), adaptive two-phase and self-started interviews with caps (R10), concurrency limits (R3). |
| Gemini P12 | Encrypted, signed Pillar 2 packet per accepted claim, referenced by hash (R17). |
| Opus P1 | As Gemini P1. Trading or NAVCoin use is open question 4. |
| Opus P2 | As Gemini P5. |
| Opus P3 | Phase 1 triangulates the person, producing a per-company relevance score (R10). Track-record weighting (R3, R14). |
| Opus P4 | As Gemini P2. Falsified claims lower weight (R18). |
| Opus P5 | Open question 8. The cap mechanism already exists in badge definitions (R19). |
| Opus P6 | Value dashboard (R20), AC8. |
| Opus P7 | As Gemini P3. |
| Opus P8 | As Gemini P10. |
| Opus P9 | Reviewer-agent triage (R15). Open question 9. |
| Opus P10 | As Gemini P7. |
| Opus P11 | Coverage manifest with a controlled vocabulary, published by the hub build (R1). |
| Opus P12 | As Gemini P12. |
| Opus P13 | Evidence-weighted matching, rotation, concurrency limits (R3). |
| Opus P14 | Lazy agents with cached briefs (R11), per-interview caps and `cost_usd` (R10). |
| Opus P15 | Partly disputed: the sentence may be complete, but it ends the deliverable without a full stop. The ellipsis is dropped, the text is quoted as written, and the additions are labelled as interpretation. Open question 1 keeps the ambiguity. |

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/10-information-hive-mind.md).

| Item | Change |
|---|---|
| Gemini P1 | Public-baseline novelty gate gathered by the reviewer agent (R15); evidence provenance with primary evidence ranked first (R14, R15); consensus-challenge multiplier as a policy parameter (R21); mosaic thesis in Economics. Non-money credit toward the top-30 score is open question 11 (designed to work under each option). |
| Gemini P2 | Main tranche only on `verified`; citation never releases it (R21). New resolution worker with deterministic rule, resolver sign-off, appeal and grace period (R17), statuses `resolving` and `appealed` (R14). Verified in code: the reward worker publishes exactly one terminal `pf.reward.v1` per task and flags submissions older than 15 minutes, so each tranche is its own task created at its trigger (R21). |
| Gemini P3 | Screening gateway holds each turn in memory and screens before any write, fails closed, stores a placeholder on refuse, confirms redactions, pauses repeat offenders (R9). Verified in code: ordinary chat loads context, memory and task state every turn and runs the memory worker after each reply, so interviews get a dedicated mode with neither, `zeroDataRetention` models only and no request-body logging (R6). AC3 now inspects tables, logs, memory and later prompts. |
| Gemini P4 | Verified 158: operator-only, agent on the model host, proposals need a thread and message. New separate `hive_*` tables; 158 stays operator-only (R11). Cluster-to-proposal hop defined: the 158 model agent on the research host drafts the proposal, the owner decides (R19). AC5, AC9. |
| Gemini P5 | Build order Step 1/2/3 with the requirements and open questions each step needs; open questions grouped by step. Disputed in part: recommended defaults for each question are not added, since they are founder decisions. |
| Gemini P6 | Corroboration counts distinct evidence, same secondary source counts once, Sybil review extended to claims with open flags withholding weight and reward, burst flagging, single well-evidenced claims advance alone (R16). Pilot subset via `interview_enabled` (R1). AC7. |
| Gemini P7 | Program envelope, per-company split by manifest priority, main-tranche reservation at acceptance, circuit breaker, compute cap (R22); value loop in Economics. Values are open question 9 (designed to work under each option, including zero). |
| Gemini P8 | Phase 3 claim cards the contributor edits, drops or approves; only approved cards are stored and signed (R12, R20). AC6. |
| Gemini P9 | Envelope readers named (contributor, hive-mind service key, compliance reviewer) via Pillar 6 `message_key` (R20). Pseudonym as separate key plus operator-attested credential (R24). Relationships never shown publicly; coarse type by opt-in (R24). |
| Gemini P10 | Link types `matched`, `following`, `steward`; rotation applies to matched only (R3). Which are on is open question 1 (designed to work under each option). |
| Opus P1 | As Gemini P3. Screening model on a zero-retention or self-hosted route; the choice is in open question 2. |
| Opus P2 | Interview service in Task Node with no model-host token or sealed-model access (R11). AC5 tests capability, not just output. |
| Opus P3 | Structured check (metric, direction or threshold, date, named source) and context claims (R14); resolution and dispute path (R17); LLM judgements stored with model id, prompt version and input hash for re-runs (R15). |
| Opus P4 | As Gemini P2: citation adds at most an acceptance-sized credit; later forecast accuracy is an option in open question 10. |
| Opus P5 | Phase 2 asks about the underlying variable and never shows another claim first; agreement after disclosure is `prompted` and weighs less; Sybil clearance before corroboration counts (R12, R16). AC2, AC7. |
| Opus P6 | Relevance score defined: versioned rubric over channel, directness and recency, self-report discounted until checked, updated from outcomes (R4). |
| Opus P7 | As Gemini P1 and P7. Pilot success measures are open question 11. |
| Opus P8 | Trade-off stated in Economics; checkability favours nearer check dates (R15); optional acceptance credit over FastPay once Pillar 2 R22 lands (R21). |
| Opus P9 | Interviews private by stated choice; each hub publish anchors one packet of claim hashes and cluster summaries a Pillar 1 steward can attach or cite in `open_questions` (R20). Publication scope is open question 12. Reproducible judgements (R15) and deterministic resolution where the source gives a number (R17). |
| Opus P10 | As Gemini P5. |
| Opus P11 | As Gemini P9; plus per-publish batch anchoring instead of one transaction per claim (R20). |
| Opus P12 | AC3 uses a versioned red-team set with size and run count published in advance, every item refused in every run; consent defined as per-field brief approval (R5); AC11 compares against founder-set measures (open question 11). Pass-rate numbers are not invented. |
| Opus P13 | Redaction limited to identifying detail; any non-public content refuses the whole answer (R9). Per-mode approval record with counsel sign-off where open question 2 requires it, and a per-mode human-review switch (R10, open question 8). Listed-equity exclusion stays open question 3. |
| Opus P14 | Versioned vocabulary file beside the manifest (R1, R2); contributor writes or edits the experience summary and approves each field (R5). |
| Opus P15 | Briefs load only on first use after a publish (R13); per-company compute cap (R22); compute spend in the dashboard (R23). |
| Cross-pillar (Pillar 9 revision) | Aligned with Pillar 9 R19: two briefs, `agent_brief_members.json` and `agent_brief_public.json` (R13); read-only service token for the bridge (R19); `tasknode-finance-hub:ask-agent` opens interview mode (R25). Manifest fields this pillar adds (mode, `interview_enabled`, priority) are stated as additions (R1). |
