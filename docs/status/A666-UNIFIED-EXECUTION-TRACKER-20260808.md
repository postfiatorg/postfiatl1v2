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
| A3 E6 proof | OPEN | |
| A4 E6 finalize | OPEN | |
| A5 legs 2a-5b | OPEN | |
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

## Stage state journal

(append-only; every agent writes before/after each gate)

- 2026-08-08 ~01:30 UTC: A3 in progress. E6 rebuild run dir: nav-e6-fresh/20260808T005948Z-e5compat. XMR sidecar rebuilt (coingecko:XMR, price 37259000000 e8). HL receipt witness built from fresh legacy-reader snapshot (block 42592633). First aggregate-witness attempt FAILED CLOSED on NEAR head hash: 213618e host verification has the pre-fix V6 threshold. Fix committed on branch e6-e5compat-near-v6-fix (8512776, guest contains no NEAR code); script rebuild running. vkey gate: rebuilt guest ELF must byte-match archived governed-aggregate-program-00580ee8.elf (sha256 dd743c38...) before any prove.

- 2026-08-08 ~01:15 UTC: Manager: Step 0 + A0 + A1 closed in one session. Executor-A/Verifier-A roles currently performed by Manager (no live mutation yet; A2 is the first custody op).
- 2026-08-08: Manager: plan committed to docs/plans/, tracker created, fire-discipline skill read and bound to Track A. Envelope active per principal GO.

## Journal

- 2026-08-08T02:3xZ A3: vkey tripwire FIRED (rebuilt ELF -> 0x00fa3bef...) — first prove killed early. Resolved by injecting archived governed ELF dd743c38... with SP1_SKIP_PROGRAM_BUILD=true host rebuild. vkey-print (new tool, StakeHub-e6-213618e 2581c43) confirms 0x00580ee8... EXACT. Pinned-guest --execute PASS (22.6M cycles), PV policy 0x076c071e44... EXACT. Groth16 prove relaunched pinned: PID /tmp/a666-unified-a0/agg-prove2.pid, log /tmp/a666-unified-a0/agg-prove2.log. Gates remaining for A3 close: proof completes + vkey line + PV policy recheck + local verify.
