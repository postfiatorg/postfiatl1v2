# A666 Unified Execution Tracker

- **Plan:** ../plans/A666-UNIFIED-EXECUTION-PLAN-20260808.md
- **Started:** 2026-08-08 UTC
- **Principal GO:** received 2026-08-08 ("get this done") — Section 2 envelope active
- **Manager:** Codex session, postfiatfoundationv2

## Assignment table

| Agent | Role | Worktree(s) | Status |
|---|---|---|---|
| Manager | orchestration, docs, evidence commits | pftl-validation-20260807 (docs only) | ACTIVE |
| Executor-A | live loop | per runbook | UNASSIGNED |
| Verifier-A | read-only verification | n/a | UNASSIGNED |
| Prover-B | Groth16 env + proves | a666-eth-fast-lane-combined-20260724 | UNASSIGNED |
| Qualifier-B pool | per-source qualification | a666-eth-fast-lane-combined-20260724 | UNASSIGNED |
| Surveyor-C pool | read-only inventory | in-scope worktrees | UNASSIGNED |
| Integrator-C | PR series | canonical checkouts | UNASSIGNED |

## Gate states

| Gate | State | Evidence |
|---|---|---|
| Docs publication (Step 0) | CLOSED | postfiatl1v2 main 65c0e71..fe72c3a; StakeHub master 6382478..e3fed38; strict builds + redaction PASS |
| A0 preflight | CLOSED | /tmp/a666-unified-a0/A0-PREFLIGHT-REPORT.md — zero drift vs handoff snapshot; agent unlocked, policy restored (12k/50k/24 whitelist); fleet 6/6 h779 converged; wA666 baseline intact; nonce 304 |
| A1 NEAR fix | CLOSED | StakeHub master 2839f4e; threshold 150->85; real mainnet v6 fixture h210383329 lpv86; RED on parent, 63/63 GREEN with fix |
| A2 reader session | CLOSED | session hl-existing-reader-20260808T005232Z open->snapshot->close; tx 03f13d56... status 1 TO pinned reader 0xd5c4200b (no deploy), HL block 42592633, cost 0.000022654 HYPE (cap 0.02); reader code verified live 9006B/2e49ae2b; evidence StakeHub-master-e6/zk/target/operator-real-20260808/ |
| A3 E6 proof | CLOSED | pinned governed vkey `0x00580ee8...`; PV policy exact; Groth16 proof locally verified; plan AGENT COMMENTS |
| A4 E6 finalize | CLOSED | reserve submit c71c0222 h780; epoch finalize 8389696a h781; E6 nav 90,353,505; plan AGENT COMMENTS |
| A5 legs 2a-5b | FIRE-READY — HELD on one principal ceremony (PR 7 + agent restart/unlock) | Checkpoint gap root-caused (691 vs 787 > 64); both CUDA groth16 proofs done + locally verified + PV byte-matched; advanceCheckpoint eth_call sim PASS (gas 321,917); accept-and-mint dry run PASS (packetDigest 0x288464d7 exact, baseline 103,000,000 intact); GPU spend ~$0.25, VM destroyed. Custody analysis widened: constrained signer can never sign 3c+ (wallet-owned assets); master-e6 agentd needs the session-less whitelist ruling -> StakeHub PR 7 (5d33bae, RED-parent/GREEN-fix, 176 passed) covers the WHOLE remaining chain in one merge+restart+unlock ceremony. Stopgap: whitelist/fund signer unblocks 3b only. Plan final AGENT COMMENTS |
| A6 closeout | OPEN | |
| B1 Groth16 env | OPEN | |
| B2 epoch 7/8 proofs | OPEN | |
| B3 qualification 6/6 | OPEN | |
| B4 G5 rehearsal | OPEN | |
| C0 inventory | CLOSED | 20/20 manifests at /tmp/a666-c0-inventory/*.json; notable: postfiatl1v2 on open-source-productionization branch 266 ahead of origin/main; nav-proof-external-verifier 265 ahead; postfiatl1v2-fastswap 210 ahead; StakeHub-repeat-demo 154 ahead + 34 dirty; a666 worktree 345 untracked/64G (protected evidence) |
| C1 target architecture | OPEN | |
| C3 integration complete | OPEN | |
| C4 retirement complete | OPEN | |
| D1 migration packet | OPEN | |
| D2 live migration (PRINCIPAL GO) | OPEN | |
| D3 legacy lane retired | OPEN | |

## STOP log

(append-only)

- 2026-08-08 GPT-5.6-Sol respawn: unconditional secret-output STOP during read-only preflight. A process-list diagnostic surfaced a VS Code connection-token class secret in transient command output. Value omitted and never persisted. No live mutation occurred. A5 held at the failed Leg 3b witness build pending fresh principal ruling.
- 2026-08-08 ~04:1xZ RESUME RULING: principal instructed "ramp up on this handoff ... and then continue executing where other agent left off." Hold lifted. Safe-resume sweep re-run first: 6/6 h787 root c6839e57 converged, 30/30 tx+receipt checks accepted, witness absent, disk 110 GB. Live mutations re-authorized inside the Section 2 envelope; fail-closed rules unchanged.

## Stage state journal

(append-only; every agent writes before/after each gate)

- 2026-08-08 ~03:40 UTC: SESSION DIED — Anthropic API credit exhaustion (`invalid_request_error: Your credit balance is too low`), goal stalled mid leg-3b. RULING RECORDED: we run the Claude Code subscription **PLAN** lane (`claude-fable-5-plan`, `/model` -> `Claude Plan` tab), NEVER the `Anthropic` API-key tab (`claude-fable-5`) which bills metered API credit. Recovery procedure written to runbook section 0.0 and plan AGENT COMMENTS. Restarted fresh on `pfterminal --yolo` (never `pfterminal resume` — stale model pin); interim lane GPT-5.6-Sol xhigh pending switch to Fable 5 Plan. No funds at risk: RES2 ed3c0b77... live and unspent, failed leg-3b witness step wrote nothing.

- 2026-08-08 ~01:30 UTC: A3 in progress. E6 rebuild run dir: nav-e6-fresh/20260808T005948Z-e5compat. XMR sidecar rebuilt (coingecko:XMR, price 37259000000 e8). HL receipt witness built from fresh legacy-reader snapshot (block 42592633). First aggregate-witness attempt FAILED CLOSED on NEAR head hash: 213618e host verification has the pre-fix V6 threshold. Fix committed on branch e6-e5compat-near-v6-fix (8512776, guest contains no NEAR code); script rebuild running. vkey gate: rebuilt guest ELF must byte-match archived governed-aggregate-program-00580ee8.elf (sha256 dd743c38...) before any prove.

- 2026-08-08 ~01:15 UTC: Manager: Step 0 + A0 + A1 closed in one session. Executor-A/Verifier-A roles currently performed by Manager (no live mutation yet; A2 is the first custody op).
- 2026-08-08: Manager: plan committed to docs/plans/, tracker created, fire-discipline skill read and bound to Track A. Envelope active per principal GO.

## Journal

- 2026-08-08 ~06:0xZ HOLD CONFIRMED THIRD CHECK; DEADLINE CLOCK NOTED: PR 7 still OPEN, signer still 0 ETH, whitelist still 24, verifier still 691 — no principal action yet, no further principal-independent work remains on A5. TIME PRESSURE: the finalized leg-3a export packet deadline is epoch 1786331925 (~46 h out at this check). If the ceremony slips past it, the on-chain packet validation refuses the mint; recovery is the refund path (refund_delay_blocks 100) plus a full leg-3a re-export, re-witness, and re-prove. The one-ceremony resume (merge PR 7 -> master checkout+pull -> restart stakehub-pfusdc-wallet-agent.service -> unlock + policy restore) takes minutes; everything downstream is staged and verified in /tmp/a666-s1g/leg3b/.

- 2026-08-08 ~05:4xZ CUSTODY FIX PR OPENED: the signer-funding gap generalizes — legs 3c/3d/3e/3h/4/5b are wallet-key contract txs the constrained signer can never sign; master-e6 agentd's launch-session-only evm_contract_tx blocks them all despite every target being on the passphrase-gated whitelist. Ported the operator checkout's session-less-whitelist ruling onto master as StakeHub PR 7 (branch evm-contract-tx-global-whitelist-ruling, 5d33bae on 2839f4e): whitelist-bound session-less calls, ERC-20 selector safety, value charged at 3000 USD/ETH mark, broadcast journaled pre-receipt-wait; set_policy passphrase custody UNCHANGED. RED on parent (6/7 fail policy_denied), GREEN with fix (test_agent 24/24; suite 176 passed/4 skipped; dashboard-hydration failure pre-exists on parent, stash-verified). ONE ceremony (merge PR 7 -> checkout master + pull -> restart stakehub-pfusdc-wallet-agent.service -> unlock + policy restore) unblocks the entire remaining Ethereum chain from the wallet; signer + 3b0 drop out. Proof artifacts remain valid (permissionless functions, sender-independent). Signer still 0 ETH, whitelist still 24, verifier still 691 — no live mutation this session.

- 2026-08-08 ~05:0xZ A5 FIRE-READY: checkpoint proof 691->756 and receipt proof (h787, mint 11,012,575) both CUDA groth16 proved on vast 4090 VM 47141932 (destroyed after pull; ~$0.25 spent), locally verified, PV byte-matched vs CPU execute. advanceCheckpoint on-chain simulation PASS from OWNER, gas 321,917. accept-and-mint dry run PASS: controller packetDigest 0x288464d7...def306 exact, mint unpaused, packet unconsumed, recipient wA666 103,000,000 baseline intact. Artifacts /tmp/a666-s1g/leg3b/. SOLE BLOCKER: constrained signer 0xe01eaf76...f424 (the ruled custody leaf for advance/accept/consume selectors) holds 0 ETH; 3b0 funding via agentd evm_send is policy_denied (signer absent from 24-entry whitelist; set_policy passphrase-gated; launch session requires a real deploy). Fail-closed refusal to bypass custody. ONE principal action resumes: `stakehub policy --add-whitelist 0xe01eaf76f155b2759402b39fe126b5a81655f424` (then agent fires 3b0/advance/3b receipt-gated) OR direct 0.01 ETH send to that address. Exact resume commands in plan final AGENT COMMENTS. No chain/wallet/key/vault mutation this session beyond GPU rental spend.

- 2026-08-08 ~04:1xZ RESUME: Leg 3b witness defect root-caused as checkpoint gap (verifier latestFinalizedHeight 691, receipt h787, 95 > 64 ancestry cap) — witness builder itself is sound. Checkpoint witness 691->756 and receipt witness (prior 756, packet 769c5719...) built read-only on validator-2 (release pnok-private-fix-2246d25-orchard1) and pre-verified via CPU execute mode: checkpoint prior commitment EXACT match with on-chain latestCheckpointCommitment 0x1afce4dc, receipt PV chains through resulting commitment 0x3b7c8bde with packetDigest 288464d7...def306 and mint 11,012,575 exact. Signer 0xe01eaf balance 0 -> 3b0 funding required (min 0.01 ETH). Prior A100 gone; vast 4090 VM instance 47141932 rented (~$0.32/h, $150 envelope). Remaining: CUDA proves, 3b0, checkpoint advance, 3b mint, 3c-3e. Evidence: /tmp/a666-s1g/leg3b/.

- 2026-08-08 GPT-5.6-Sol respawn HANDOFF: live read-only reconciliation found fleet 6/6 h787 / root c6839e57 / mempool 0. All five a666-s1g labels return confirmed tx plus exactly one receipt on every validator, accepted=true/code=accepted (release fee 22; advance/reserve/subscribe/export fee 23). Never re-run. RES2 terminal. Leg 3b witness file remains absent after prior bounds error; no EVM send fired. Live mutations HELD by STOP-log entry; no money/keys/vault/balances touched. Full handoff in plan final AGENT COMMENTS.

- 2026-08-08T06:2xZ GATE E CLOSED (S1f binding + linter PASS, a666 e6c35e9) and A5 legs 2a/2b/3a FINALIZED via S1g corrective sequence (release 34f281f5 h783, epoch-advance 2d42f270 h784 -> route epoch 7 policy 50af7455 pricing E6, reserve 2610adb9 h785, subscribe b7716ed8 h786 conservation exact -10,000,000 pfUSDC/+11,012,575 A666, export 2543517d h787). Fleet h787 mempool 0. Next: Ethereum legs 3b0-3e (agent custody), re-sim 3e before fire.

- 2026-08-08T04:0xZ GATE A4 CLOSED: E6 finalized on-chain. Proof phase: pinned vkey 0x00580ee8 proof verified locally; policy 0x076c07 exact. Ops: nav_reserve_submit tx c71c0222... h780, nav_epoch_finalize tx 8389696a... h781, both 6/6 converged. Live A666 profile now epoch 6 / nav 90,353,505 / packet b06262a1.... Max compliant mint recomputed: 11,012,575 atoms. Evidence: nav-e6-fresh/20260808T005948Z-e5compat/e6-ops/.

- 2026-08-08T02:3xZ A3: vkey tripwire FIRED (rebuilt ELF -> 0x00fa3bef...) — first prove killed early. Resolved by injecting archived governed ELF dd743c38... with SP1_SKIP_PROGRAM_BUILD=true host rebuild. vkey-print (new tool, StakeHub-e6-213618e 2581c43) confirms 0x00580ee8... EXACT. Pinned-guest --execute PASS (22.6M cycles), PV policy 0x076c071e44... EXACT. Groth16 prove relaunched pinned: PID /tmp/a666-unified-a0/agg-prove2.pid, log /tmp/a666-unified-a0/agg-prove2.log. Gates remaining for A3 close: proof completes + vkey line + PV policy recheck + local verify.
