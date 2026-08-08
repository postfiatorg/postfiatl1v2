# A666 Unified Execution Plan: Live-Loop Close, StakeHub Proof Exit, Worktree Consolidation

- **Created:** 2026-08-08 UTC
- **Principal:** goodalexander
- **Manager of record:** Codex session on postfiatfoundationv2 (this plan's author)
- **Worker model:** `gpt-5.6-terra`, reasoning effort `xhigh`, for every spawned agent
- **Operational context:** StakeHub agent is UNLOCKED as of plan creation. Unlock is perishable (any restart clears it). Principal opportunity cost exceeds $1,000/day; this plan is designed to never block on the principal after the single authorization in Section 2.
- **Supersedes nothing.** This plan composes and sequences three existing authorities:
  - Runbook: `docs/runbooks/A666-END-TO-END-LIVE-FUNDS-RUNBOOK-20260807.md` (operational truth for Track A)
  - Handoff: `docs/handoffs/A666-LIVE-FUNDS-EXECUTION-HANDOFF-20260807.md` (state truth at audit)
  - Decoupling plan: `/home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md` (gate definitions G0-G8 for Track B/D)

Where this plan and a source authority conflict on a safety rule, the stricter rule wins. Where they conflict on sequencing, this plan wins.

## 1. Objective

Four outcomes, in dependency order:

1. **Track A — Close the open live round trip** using the unlocked StakeHub infrastructure: E6 NAV refresh, then legs 2a through 5b, ending with external Ethereum USDC credit and conservation evidence. The 10.000000 USDC deposit and the PFTL claim at height 779 are final; the loop resumes at E6 refresh, never earlier.
2. **Track B — Productionize the chain-native successor proof lane**: Groth16 proofs of qualification epochs 7 and 8, per-source qualification decisions (G3 to 6/6), G4 closed.
3. **Track C — Convert the worktree sprawl into one clean unified codebase**: every unique commit either merged to a canonical branch, archived with evidence, or explicitly discarded with a written reason; worktrees retired one at a time behind a checklist.
4. **Track D — Governance migration of live A666 to the successor profile** (G5, G6, G7), after which StakeHub is custody-only and the legacy proof lane is retired.

Definition of done for the whole plan: a fresh operator on a clean checkout can run the complete canonical route (USDC -> pfUSDC -> A666 -> wA666 -> Uniswap -> back) with chain-native NAV verification, and the repository tree contains one canonical PFTL branch, one canonical StakeHub branch, and zero load-bearing dirty worktrees.

## 2. Standing authorization (the no-block envelope)

The principal approves this plan once. That approval constitutes GO for everything inside this envelope, with no further per-step principal input:

**Authorized without further approval:**

- Track A legs 2a, 3a-3h, 4, 5a, 5b exactly as specified in the HELD packet set (packet lineage `de93c56` -> `5bd637b` v3, all five legs WIRED) and the 20260807 runbook, with fresh S1f binding values, at amounts bounded by the packet arithmetic already recorded (mint ceiling 11,011,167 atoms old-projection; final number recomputed from the policy-compatible E6 proof; one-atom breach rule applies).
- One governed existing-reader Hyperliquid session against reader `0xd5c4200b74929952dca4db70fdc65317c2705207` with runtime code SHA-256 `2e49ae2b32f2598c8a77a3b234180101191396c77046e50498dda1df68bbe713`, snapshot cost capped at 0.02 HYPE, closed and reconciled in the same session.
- E6 aggregation, prove, verify, batch-only build, and single finalize under required policy `076c071e44127158ef82350e7feeb64e0be0a06bf8ba4be5f0374ac36b992ac7` and vkey `0x00580ee8c389192568a29dc23d54c22e73a3a45203b22e3d5a934801871e11a7`.
- All Track B compute and rehearsal-fleet work (no live funds, no live validators).
- All Track C read-only inventory, plus mutations that are additive (branches, PRs, archives). Deletions and worktree retirement follow the checklist in Section 6.5 and are authorized once the checklist for that worktree is green.
- Spend for compute rental (vast.ai GPU for SP1 proving) up to $150 total, treated as a runaway guard.

**Requires a fresh explicit GO from the principal (the only two blocking points in this plan):**

- Track D live governance migration of A666 to the successor profile (D2). HELD packet delivered; principal fires it.
- Any history rewrite on a pushed branch (expected: none).

**STOP-no-retry, always, regardless of envelope:** any admission failure with an unexpected code, any arithmetic that deviates from the packet by one atom or more, any policy/vkey/reader/code-hash mismatch, any evidence of double-spend or duplicate submission, any touch of the protected 103.000000 wA666 baseline, disk under 20 GB free, or any secret appearing in output. On STOP: hold everything, reconcile mutations, write the STOP report, continue other tracks only if fully independent.

The NEVER DO list from the 20260807 handoff is incorporated by reference and binds every agent in every track.

## 3. Agent orchestration model

### 3.1 Roles

| Role | Count | Model config | Mutation rights |
|---|---|---|---|
| Manager (this session) | 1 | n/a | Orchestration, docs, commits of evidence/docs |
| Executor-A (live loop) | 1 | gpt-5.6-terra xhigh | Live mutations inside Track A envelope only |
| Verifier-A | 1 | gpt-5.6-terra xhigh | Read-only; independent re-derivation of every Executor-A receipt |
| Prover-B | 1 | gpt-5.6-terra xhigh | Compute jobs, env fixes; no live systems |
| Qualifier-B pool | up to 6 | gpt-5.6-terra xhigh | One per source family; writes qualification evidence only |
| Surveyor-C pool | up to 6 | gpt-5.6-terra xhigh | Read-only worktree inventory manifests |
| Integrator-C | 1 | gpt-5.6-terra xhigh | Branches, cherry-picks, PRs; never `git add .`, never clean/reset |
| Reviewer-C | 1 | gpt-5.6-terra xhigh | Read-only PR review with file:line findings |

### 3.2 Rules of engagement

1. **No fan-out where money moves.** Track A is one Executor and one read-only Verifier. Fan-out applies only to Track B qualification, Track C inventory, and PR review.
2. **Single-writer-per-worktree.** At most one agent with mutation rights inside any given worktree at any time. The Manager owns the assignment table and updates it in `docs/status/A666-UNIFIED-EXECUTION-TRACKER-20260808.md`.
3. **Evidence-first reporting.** Every agent reports with exact commands, file:line, hashes, and receipt values. The three-way communication rule from the handoff binds all agents: finalized receipt with before/after values, or long-running job with PID/start/progress, or fail-closed STOP with reconciliation. Nothing else counts as a status.
4. **Secrets discipline.** No agent prints, copies, or moves key material. The StakeHub passphrase exists nowhere in this system. Reports name secret locations and classes, never values.
5. **Compaction resilience.** Every agent writes its stage state to the tracker doc before and after each gate so any agent (or the Manager) can be replaced mid-stage without archaeology.
6. **Concurrency cap:** at most 8 concurrent agents; at most 2 concurrent heavy compute jobs (SP1 prove, full test suite). Disk guard: every agent checks free space before large writes; global floor 20 GB.

## 4. Track A — close the live loop (starts immediately; unlock is perishable)

Source of operational truth: the 20260807 runbook, steps 1-15. This section only assigns stages, gates, and agents.

### A0. Preflight (Executor-A + Verifier-A, ~30 min)
- Verify agent singleton, unlock state, and policy. Post-restart state is expected to show default caps and empty whitelists; restore the exact required policy per `StakeHub-master-e6/zk/OPERATOR_RUNBOOK.md` ceremony and verify it reads back byte-exact.
- Verify fleet 6/6, height advancing, mempool empty, holder balances match the handoff snapshot (11.358493 pfUSDC; 99.000000 A666; 74.161443 USDC; 103.000000 wA666 untouched; nonce 304; zero reservations/entitlements/claims).
- **Gate A0:** every value matches or the delta is explained by chain progress alone. Mismatch on any balance = STOP.

### A1. NEAR BlockHeaderV6 threshold fix (Prover-B assists; patch lands before any aggregation)
- Extract the fix from the dirty `/home/postfiat/repos/StakeHub` checkout as a patch, land it on `StakeHub-master-e6` via PR with a failing-then-passing current-mainnet regression.
- **Gate A1:** regression red on parent commit, green on fix commit. Dirty checkout itself remains untouched.

### A2. Existing-reader session, snapshot, close (Executor-A)
- One governed session against the pinned legacy reader; require exact runtime code hash; one snapshot at <= 0.02 HYPE; close and reconcile.
- **Gate A2:** session closed, cost within cap, snapshot bound to reader identity. Any reader deployment event = STOP (that is the exact defect that burned Aug 7).

### A3. E6 aggregation and proof (Executor-A drives; StakeHub-e6-213618e for E5-compatible topology)
- Aggregate with XMR sidecar topology and the explicitly attested Solana witness. Require policy hash `076c07...` byte-exact before proving. Prove, verify against pinned vkey, zero reconciliation residual.
- **Gate A3:** policy hash match AND vkey match AND residual zero. The 20260807 policy-mismatched proof is quarantined and never submitted.

### A4. E6 operations, batch-only, single finalize (Executor-A; Verifier-A re-derives arithmetic)
- Build E6 operations, batch-only validation, then finalize exactly once. Recompute mint arithmetic from the final proof; the old 11,027,135 figure is dead.
- **Gate A4:** finalized E6 on-chain, NAV fresh within the route's 100-block gate, mint number recomputed and within bound.

### A5. S1f binding and legs 2a -> 5b (Executor-A, receipt-by-receipt; Verifier-A confirms each)
- Build S1f from final E6 values with a fresh four-hour swap deadline. Execute leg 2a, then each subsequent leg per its HELD packet: 3a-3h (bridge + Uniswap), 4 (return burn/import), 5a (primary redeem), 5b (bridge-out).
- **Gate A5 (per leg):** finalized receipt with exact before/after values, verified independently, before the next leg fires. Deadline expiry = rebind fresh, never reuse.

### A6. Closeout (Verifier-A writes; Executor-A stands down)
- External Ethereum USDC credit confirmed, conservation identity across PFTL + Ethereum + bridge claims verified, before/after table published, temporary `/tmp` evidence archived redaction-safe into the campaign evidence tree.
- **Gate A6 = Track A done:** conservation holds exactly; evidence archived; tracker updated. This is the first moment the plan reports "live loop closed."

## 5. Track B — successor proof productionization (starts in parallel with A)

Worktree: `/home/postfiat/repos/a666-eth-fast-lane-combined-20260724` (branch `feature/pnok-private-fix`). Safety: never `git add .`; the ~344 untracked evidence paths are protected; the frozen demo checkout is untouchable.

### B1. Groth16 environment repair (Prover-B)
- Reproduce the Docker-context failure that stopped work on Aug 3 (`b4ed59c` added the fail-early guard). Fix the environment. Respect the memory bound work in `e8ab60c`; the Aug 4 CPU OOM wall on the receipt prover is a known hazard — pin memory limits before launching.
- Decision point (Prover-B decides, no principal input): CPU prove locally vs GPU rental within the $150 cap. Prior campaign data: SP1 Groth16 CUDA wrap ~190-210s on rented GPU vs hours on CPU.
- **Gate B1:** a trivial proof completes end-to-end in the fixed environment.

### B2. Prove and verify epochs 7 and 8 (Prover-B; long-running jobs with PID/progress reporting)
- Prove both qualification epochs under the immutable successor profile. Verify against pinned vkey `0x00f3857f96ef97e00bd15b4030acd8d6b0a72740b28c6160d154bc2c9bb141bf`, ELF `2b41e4e8...`, profile `f8784629...`.
- **Gate B2:** both proofs verify; artifacts committed to the qualification bundle with hashes.

### B3. Per-source qualification decisions (Qualifier-B pool, 6 parallel agents)
- One agent per source family (Aave, EVM spot, Hyperliquid, NEAR, Solana, XMR). Each writes the final qualification decision against the decoupling plan's own criteria, citing the epoch 7/8 evidence, fuzz results, and the now-verified proofs.
- **Gate B3 = G3 6/6 and G4 core closed.** Recorded in the decoupling plan's gate table with evidence paths.

### B4. G5 rehearsal completion (single rehearsal Executor; defect fixes fanned to a 2-agent pool)
- Resume the R4+ rehearsal lineage (R2/R3 already passed Aug 3-4). Run the complete controlled lifecycle on the six-validator rehearsal fleet under the proven successor profile: activation, transparent/private issue/redeem, export/return, restart, replay, conservation, pause, rollback.
- Fix in rehearsal the known open defects: DEFECT-13 (add-asset/trustline finality) and the v6 StakeHub-absence failure root cause.
- **Gate B4 = G5 PASS** with a signed lifecycle checkpoint, same format as the Aug 3 runs.

## 6. Track C — worktree consolidation into one clean codebase

### 6.1 Doctrine for the dirty mess

1. **Nothing is deleted before it is archived or merged.** Every retirement produces either merged commits on a canonical branch or a tagged archive (branch pushed to origin + evidence tarball with manifest and hashes).
2. **Untracked files are cargo, never trash.** Classify every untracked path as evidence (archive into the committed evidence tree or an archive tarball), scratch (listed, then removed only at retirement sign-off), or secret-adjacent (reported by location and class, moved only by documented ceremony).
3. **The dirty `/home/postfiat/repos/StakeHub` checkout is a patch source, never an execution environment and never a bulk-add target.**
4. **Live-load-bearing worktrees are frozen until their consumer track completes.** `pftl-validation-20260807`, `StakeHub-master-e6`, `StakeHub-e6-213618e`, and `a666-orchard-fix-2246d25` are frozen until Gate A6. `a666-eth-fast-lane-combined-20260724` is frozen until Gate B4.
5. Redaction scan (`redaction-scan` per campaign convention) runs on every archive before it is written anywhere shareable.

### 6.2 C0: Inventory (Surveyor-C pool, read-only, starts immediately)

Scope: the PFTL family and StakeHub family only. Out of scope for this plan: everything else under `/home/postfiat/repos`.

| Family | Worktrees |
|---|---|
| PFTL | postfiatl1v2 (canonical), pftl-validation-20260807, pftl-main-integration-20260807, pftl-port-depositv2-20260807, a666-eth-fast-lane-combined-20260724, a666-orchard-fix-2246d25, nav-proof-external-verifier-20260725, nav-proof-explorer-20260725, pftl-escrow-htlc-spike-20260724, pftl-lightning-navcoin-demo-20260724, postfiatl1v2-fastswap, e6-scratch, _worktree_holding |
| StakeHub | StakeHub (dirty original), StakeHub-master-e6, StakeHub-e6-213618e, StakeHub-hl-existing-reader, StakeHub-red-base, StakeHub-repeat-demo, StakeHub-vkey-repro-20260730 |

Each Surveyor produces one manifest JSON per worktree: branch, upstream, ahead/behind counts, unique commits vs canonical (`git cherry -v`), dirty tracked files, untracked classification counts, secret-adjacent findings (locations only), disk footprint, and a proposed disposition (merge / archive / patch-source / retire-clean).

**Gate C0:** every in-scope worktree has a manifest; Manager compiles the disposition table into the tracker doc.

### 6.3 C1: Target architecture (Manager decides, records in tracker)

- Canonical PFTL: `postfiatorg/postfiatl1v2` `main`. Canonical StakeHub: `master`.
- Campaign lineages (fire-20260806, validation-20260807, orchard fix, depositV2 port) merge as ordered PR series with evidence preserved in-tree.
- The `83ac75d` divergence warning (non-NAV-spread custody accounting vs deployed 2246d257 supply semantics) is resolved explicitly in its PR description before merge — the deployed-fleet semantics win until the pNOK release ships.

### 6.4 C2-C3: Integration PRs (Integrator-C writes, Reviewer-C reviews, Manager merges)

- One PR per coherent commit group, smallest blast radius first, base branch pushed before `gh pr create`.
- CI green (including the recovery regression manifest gates from Aug 4) is a merge requirement.
- Evidence directories move by `git mv`/addition into canonical evidence paths, never by copy-detach.

**Gate C3:** canonical branches contain every keep-commit; `git cherry` against every source worktree shows zero unmerged keepers.

### 6.5 C4: Worktree retirement (Integrator-C, one worktree at a time)

Retirement checklist, all boxes required, recorded per worktree in the tracker:

- [ ] Manifest disposition = merge/archive complete, verified by `git cherry` + archive hash
- [ ] Untracked evidence archived (redaction-scanned) or explicitly listed as discarded scratch
- [ ] No secret-adjacent files remaining
- [ ] Not referenced by any systemd unit, Caddyfile, cron, tmux session, or running process (`lsof +D` clean)
- [ ] Not frozen by Section 6.1 rule 4
- [ ] Removal executed and logged (`git worktree remove` / `rm` of the directory, recorded with what was removed and how to recover it)

**Gate C4 = Track C done:** in-scope tree reduced to canonical checkouts plus explicitly retained archives; disk usage reported before/after (host is at 80% today; target meaningful reclaim).

## 7. Track D — migrate live A666 to the successor, retire the legacy proof lane

Prerequisites: Gates A6 (loop closed, no open campaign state) and B4 (G5 PASS).

- **D1:** Build the governance migration HELD packet: profile rotation to `f8784629...` per the rotation manifest in the qualification bundle, exact before/after supply and NAV bindings, rollback plan. Verifier-A-style independent re-derivation.
- **D2:** **Principal GO required.** Fire the migration; verify all six validators converge; run one fresh chain-native NAV epoch end to end; confirm mint/redeem admission works against the successor profile.
- **D3:** Retire the legacy lane: mark the E5/E6 StakeHub aggregation path historical in docs, close the decoupling plan gates G6/G7 with evidence, record StakeHub's remaining role as custody-only. G7 requires one clean-checkout reproduction run by an agent with no access to internal paths.

**Gate D3 = program done:** `stakehub_deprecated=true` becomes writable in the decoupling plan, truthfully.

## 8. Dependency graph and schedule

```text
A0 -> A1 -> A2 -> A3 -> A4 -> A5 -> A6 ----\
                                            +--> D1 -> D2 -> D3
B1 -> B2 -> B3 -> B4 -----------------------/
C0 (read-only, immediate) -> C1 -> C2/C3 (unfrozen worktrees first) -> C4
                                   C4 final retirements wait on A6/B4 freezes
```

Expected wall-clock, honest ranges: Track A is hours-not-days if A2/A3 hold (the topology is pinned and the reader fix is merged); the long pole is proving. Track B1/B2 is 1-2 days dominated by prove time (less with GPU rental). B4 is 1-2 days of rehearsal iterations. Track C inventory lands day one; integration is 2-4 days of PR grind. D fits in one day once prerequisites hold. Every number here is an estimate, and gates, never the schedule, decide when a stage is done.

## 9. STOP conditions and risk register

| Risk | Mitigation |
|---|---|
| StakeHub restart clears unlock mid-Track-A | A0 runs first; Executor-A checks unlock before every mutating step; on loss, Track A pauses, principal notified once, all other tracks continue |
| Reader/policy topology drift (the Aug 7 killer) | Gate A2/A3 byte-exact pins; any deployment or preimage change = STOP |
| Prove OOM / disk exhaustion | Memory bounds pinned before launch; 20 GB disk floor; cleanup sweep before heavy jobs |
| Cross-track worktree collision | Single-writer rule + Section 6.1 freezes |
| Duplicate live submission after agent replacement | Tracker stage-state written before/after every gate; resume rules from FIRE-20G precedent (never re-run landed commands) |
| Evidence loss at reboot | `/tmp` archival is part of Gate A6, not optional cleanup |

## 10. Reporting

- The Manager maintains `docs/status/A666-UNIFIED-EXECUTION-TRACKER-20260808.md`: assignment table, gate states, per-worktree retirement checklists, STOP log.
- Principal-facing reports on gate transitions only, in the three-way format (receipt / running-with-PID / STOP-with-reconciliation), with exact values and bare absolute paths.
- Readable copies of principal-facing reports land in `/home/postfiat/repos/pastedocs/`.

## AGENT COMMENTS

(append-only; every agent records material findings, deviations, and gate evidence here or in the tracker with a pointer here)

### 2026-08-08 ~01:5xZ — resumed session (post-crash respawn) — A3 in flight

State for any respawn (verify live before trusting):

- Step 0, A0, A1, A2: CLOSED (see tracker). A2 spent 0.0000227 HYPE, tx 03f13d56..., evidence /home/postfiat/repos/StakeHub-master-e6/zk/target/operator-real-20260808/
- A3 run dir: /home/postfiat/repos/a666-eth-fast-lane-combined-20260724/docs/evidence/a666-public-reserve-product-20260803/nav-e6-fresh/20260808T005948Z-e5compat (also in /tmp/a666-unified-a0/e6-run-dir.txt)
- NEAR v6 fix (150->85) applied host-side in StakeHub-e6-213618e branch e6-e5compat-near-v6-fix commit 8512776; script bins rebuilt OK.
- aggregate-witness: PASS (six legs). reconcile: PASS. aggregate-prove --execute: PASS, PV 2720 bytes.
- POLICY GATE CLOSED: execute PV bytes[96:128] = 0x076c071e44127158ef82350e7feeb64e0be0a06bf8ba4be5f0374ac36b992ac7 (exact pin). Cross-check: witness policy preimage byte-identical to known-good fresh-old-xmr-and-hl.json (/tmp/ghash-e6-policy/policy-isolator-recompute.txt).
- Groth16 prove RUNNING detached: PID in /tmp/a666-unified-a0/agg-prove.pid, log /tmp/a666-unified-a0/agg-prove.log. Outputs -> run dir aggregate-proof.bin / -calldata.bin / aggregate-public-values.bin.
- VKEY TRIPWIRE OPEN: rebuilt guest ELF sha256 bbd5aa35... differs from archived governed ELF dd743c38... (may be path-embed nondeterminism). vkey only prints at END of --prove, so a vkey-print bin was added (script/src/bin/vkey_print.rs, branch commit pending) and is computing the rebuilt ELF vkey: log /tmp/a666-unified-a0/vkey-print.log, PID /tmp/a666-unified-a0/vkey-print.pid. GATE: if vkey != 0x00580ee8c389192568a29dc23d54c22e73a3a45203b22e3d5a934801871e11a7 -> KILL the prove, fallback = cfg-gate the NEAR fix out of the zkvm target so guest bytes revert, rebuild, re-verify ELF hash, relaunch. STOP-no-retry on anything weirder.
- expected-signer env vars are NOT needed: option_env fallback is DECLARED_OWNER = 0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0 which IS the required signer.
- Money since resume: zero. Only local builds, reads, witness assembly.


### 2026-08-08 ~02:2xZ — vkey tripwire FIRED and RESOLVED; pinned-guest Groth16 prove relaunched

- Tripwire: rebuilt-ELF vkey came back 0x00fa3bef... != pinned 0x00580ee8... First prove KILLED at ~5.5 min (no output written).
- Root cause: rebuilding the 213618e script package regenerates the guest ELF and drifts the vkey (NEAR patch and/or path nondeterminism — not isolated further, did not need to be).
- Resolution (no gate weakened): copied archived governed ELF (sha256 dd743c38..., the exact E5 artifact) over target/elf-compilation/.../stakehub-aggregate-program, rebuilt host bins with SP1_SKIP_PROGRAM_BUILD=true so sp1-build leaves the guest untouched but still wires include_elf paths. Injected ELF hash verified before AND after rebuild.
- New tool: vkey-print bin (StakeHub-e6-213618e commit 2581c43) prints the embedded guest vkey without proving. Against the injected ELF: 0x00580ee8c389192568a29dc23d54c22e73a3a45203b22e3d5a934801871e11a7 — EXACT PIN. VKEY GATE CLOSED.
- Pinned-guest --execute on the fresh witness: PASS, 22,631,608 cycles, PV 2720 bytes, policy 0x076c071e44... exact. The pinned guest does NOT re-verify NEAR head hashing in-circuit, so the fresh lpv-86 NEAR leg is fine under the pinned vkey; the corrected NEAR hashing lives host-side only (branch commit 8512776).
- Groth16 prove RELAUNCHED with the pinned-guest binary (aggregate-prove sha256 fd931709...): PID file /tmp/a666-unified-a0/agg-prove2.pid, log /tmp/a666-unified-a0/agg-prove2.log, outputs -> run dir. Expected: prints "aggregate program vkey: 0x00580ee8..." at completion; verify PV policy bytes[96:128] again and vkey line before calling A3 closed.
- If a respawn finds the prove dead with no proof-out: safe to relaunch the same command from run-dir env; nothing on-chain moved. Money since resume: still zero.


### 2026-08-08 ~02:5xZ — Gate D step 4 closed; A4 HELD packet written; B1 diagnosis done

- Gate D step 3: reconcile residual 0 (report in run dir). Step 4: preview PV == pinned-guest execute PV byte-for-byte, sha256 40074b8e... CLOSED.
- A4 packet (HELD, fires only after proof + local verify): <run dir>/PACKET-A4-e6-finalize.HELD.md sha256 ddb819b0... Uses authoritative 57ec4168 builder (byte-for-byte copy /tmp/ghash-e6-recon/builder-57ec4168.py sha 1cc5a13b...), fresh status captures over postfiat-local-rpc-v1, batch-only validate, finalize once, mint recomputed from final NAV.
- B1 diagnosis: docker works from this launch context (group 988(docker) active, server 29.1.3, /var/run/docker.sock reachable) — the Aug 3 failure was stale supplementary groups in user-systemd managers (guard b4ed59c). Gate B1 trivial end-to-end proof DEFERRED until the A3 Groth16 prove frees RAM (would contend, OOM risk = STOP hazard).
- Groth16 prove in flight: PID file /tmp/a666-unified-a0/agg-prove2.pid, log /tmp/a666-unified-a0/agg-prove2.log. Started ~02:37Z.


### 2026-08-08 ~03:1xZ — GATE D PROOF PHASE CLOSED (steps 1-6)

- Groth16 prove COMPLETED (~10 min wall on CPU, 24 cores): proof.bin 4412 B, calldata 356 B, PV 2720 B.
- Prove log final line: aggregate program vkey: 0x00580ee8c389192568a29dc23d54c22e73a3a45203b22e3d5a934801871e11a7 — EXACT PIN.
- final PV sha256 40074b8e... == execute PV == preview PV (byte-identical all three).
- Step 6 independent local verify: new verify-aggregate bin (StakeHub-e6-213618e, embeds injected governed ELF dd743c38...) -> "verified OK" + pinned vkey. CLOSED.
- In-process gates also passed during prove: client.verify, PV==native-verifier bytes, reconcile gate (0 mismatch lines in log).
- NEXT: A4 per PACKET-A4-e6-finalize.HELD.md in run dir — fresh status captures (read-only), 57ec4168 builder (unsigned templates), batch-only validate on fresh clone, then submit/finalize EXACTLY ONCE.


### 2026-08-08 ~04:0xZ — GATE A4 / GATE D FULLY CLOSED: E6 FINALIZED ON-CHAIN

- Builder inputs captured fresh from a byte-clone of live validator-1 (rsync, exclude validator_keys.json; clone height 779 root 2a2a9bf6... == live 6/6). Clone: see /tmp/a666-unified-a0/a4-clone-dir.txt. Ledger-row inputs shape-matched the recon dry-run exactly (alloc 62 / buckets 3 / receipts 59 after settlement-asset filter).
- 57ec4168 builder output (unsigned): epoch 6, proof net assets 2,833,885,006,774 + overlay 21,032,560,900 (root e5201317...b82ddc, unchanged) = 2,854,917,567,674 usd_1e8; supply 31,597,197,455 atoms -> nav_per_unit 90,353,505; reserve_packet_hash b06262a1e6e4eba7851b7326638cba350f8deffbc0bdd9069595ae9e0f56475e05fd1b4804a1e40a8948ac344c49951a.
- Batch-only round 01 validated on clone; live nav_reserve_submit tx c71c0222... FINALIZED at height 780 (6/6 converged root b8ee4042...).
- Clone refreshed, height gate 780==780 PASS; batch-only round 02 validated; live nav_epoch_finalize tx 8389696a... FINALIZED at height 781.
- POST-STATE (all six validators): height 781 root bf8a7010..., A666 profile finalized_epoch=6, nav_per_unit=90,353,505, finalized_reserve_packet_hash=b06262a1... EXACT, halted=false. Pricing freshness trivially inside the 100-block gate.
- Mint recompute (Gate E prep): at NAV 90,353,505 the max compliant mint is 11,012,575 A666 atoms (base 9,950,248 -> principal exactly 10,000,000 pfUSDC atoms; 11,012,576 breaches). The old 11,027,135 and the 11,011,167 hypothetical are both dead.
- Evidence: run-dir e6-ops/ (ops jsons, manifest, both finality receipts); clone round artifacts under the a4-clone dir.
- Money: PFTL chain-state ops only (reserve submit + finalize, authorized by plan A4); no Ethereum/StakeHub/keys/vault balances touched. NEXT: A5 — S1f binding from these values, then legs 2a->5b receipt-by-receipt.


### 2026-08-08 ~05:0xZ — GATE E CLOSED: S1f binding rendered, linted, committed

- values-S1f.json + binding-S1f.json (sha256 d6a165b72b8a7fdeeb9f0a12d10bb335e17ca42cd1480d9afaa0364d8b32c35b) + packets-S1f/ committed as a666-eth-fast-lane e6c35e9. Active legs 2a,2b,3a,3b0,3b,3c,3d,3e; 3f-3h/4/5a/5b stay receipt-chained (S2f/S3f/S4f pattern).
- Every Gate E requirement bound: mint 11,012,575 (boundary-verified, principal exactly 10,000,000), reservation sha384(native-v1|route|leg2a-reserve|781|20260808) 96-hex, nonces sha256 same rule (subscribe/export/redeem), all recipients lowercase, pricing epoch 6 + packet b06262a1..., gas ceilings from live base fee 36.8Mwei ETH/USD 1913.24 (total 0.253739, projection 511.278584 <= 530), swap deadline bind-time+4h in calldata, two-fork delta-zero sim at 2-RPC-agreed block 25707323 (fill 8,047,252; min-out 7,805,834; leg3h sequential reference 10,998,833), leg3a digest fields OMITTED per FIRE-10 ruling, linter validate-executable --through 3e PASS with proper staged exemptions.
- Sim + derived-value evidence: /tmp/a666-s1f/ (agreement, calldata jsons, derived-values.json). Freshness: leg3e must fire by Ethereum block 25707451 (sim+128) else re-sim + rebind (S1b precedent).
- Executor surface: scripts/native_campaign_driver.py run-leg (fail-closed, binary pins verified: client a982f8d2... present at /tmp/fire-20260806-bin/). Journal artifact dir to be created at fire.
- NEXT: fire leg 2a (order reserve) via driver, then 2b, 3a, then EVM legs 3b0-3e; each receipt-gated.


### 2026-08-08 ~05:4xZ — leg 2a fired+finalized; SEQUENCING DEFECT found fail-closed at 2b; corrective S1g sequence in flight

- LEG 2A: batch-only PASS (the S1c stale-pricing gate is cleared by E6), live submit ok=true, tx 363b00d8... h782, receipt accepted=true fee 23. Reservation 1c78d7b2... (96-hex) live, expires h1781.
- LEG 2B REJECTED fail-closed at clone batch-only: pftl_uniswap_pricing_binding_mismatch — the route v2 primary_market_policy (policy_epoch 6) still pins pricing E5. Subscribing at E6 requires pftl_uniswap_route_epoch_advance (6->7) whose admission REQUIRES zero active reservations (nav_vault_asset_execution.rs:5147-5158). Correct order is: release -> epoch advance -> fresh reserve -> subscribe.
- Corrective S1g sequence (ops in /tmp/a666-s1g/, each batch-only then live): 01 order_release (holder releases 1c78d7b2..., escrow refund, no fund loss), 02 route_epoch_advance issuer-signed 6->7 policy_epoch 7 policy_hash 50af7455e7ed12b3... (derivation verified byte-exact vs epoch-6 db6be8d0... via sha3-384 postfiat.pftl_uniswap.primary_market_policy.v2 preimage; pricing pinned to E6 b06262a1..., valid_from 782, expires 10000, max_nav_age 1000, governed ratios), 03 order_reserve fresh RES2 ed3c0b77...fae46 (sha384 native-v1|route|leg2a-reserve-r2|782|20260808) route/policy epoch 7 mint 11012575 max_settle 10000000 expires 1782, 04 subscribe_v2 RES2 pricing E6.
- Note for A5 later stages: leg 5a rebind (S4f) must use policy_epoch 7 + policy_hash 50af7455... + pricing E6.
- If respawning mid-sequence: check receipts for labels a666-s1g-* via tx/receipts RPC before re-firing anything; never reuse RES1 or RES2 after a terminal state.


### 2026-08-08 ~06:2xZ — S1g corrective sequence + legs 2a/2b/3a ALL FINALIZED (PFTL side of loop complete through 3a)

Receipts (all accepted=true, fee 23 each, ok=true fail-closed submitter gates):
- order_release (RES1): tx 34f281f5... h783
- route_epoch_advance 6->7 (issuer-signed): tx 2d42f270... h784. Route now epoch 7, policy_epoch 7 policy_hash 50af7455..., pricing pinned E6 b06262a1..., outbound TRUSTLESS_FINALITY.
- order_reserve (RES2 ed3c0b77...fae46, mint 11,012,575, max_settle 10,000,000, expires 1782): tx 2610adb9... h785
- primary_subscribe_v2 (LEG 2B): tx b7716ed8... h786. CONSERVATION EXACT: holder pfUSDC 11,358,493 -> 1,358,493 (-10,000,000); A666 99,000,000 -> 110,012,575 (+11,012,575).
- export_debit (LEG 3A): tx 2543517d... h787. Holder A666 back to 99,000,000 (11,012,575 exported to entitlement). Fleet at h787 root c6839e57..., mempool 0.
- CODE-TRUTH CORRECTION found live: FIRE-10 digest-omission ruling is STALE — deployed orchardfix REQUIRES ethereum_packet_digest + schema_version=2 on live export (pftl_uniswap_ethereum_verification.rs verify_live_export). Digest computed per PftlUniswapMintPacketV2.evm_digest (keccak packed; policy commitment keccak(policy_hash bytes)); value 288464d7f9d92bd3abf61b523e2d7336b21a3bdf5de6c074f948849300def306. packet_hash must be 96-hex (sha384) not 64.
- Leg 3a op values for the Ethereum side: packet_hash sha384(native-v1|route|leg3a-export-packet-r2|786|20260808), export_nonce sha256(...|leg3a-export-r2|786|20260808), deadline now+48h, recipient 0x1455bd...c0 lowercase, controller 0x9a0262c0572fb4db08765408eb225e207f40c3d9, wrapped 0xee4c92edb03efdd9b519339edc19ad70c69a9be5.
- Evidence: /tmp/a666-s1g/ (ops, signed txs, finality jsons, batch-only rounds).

NEXT (Ethereum side): leg 3b0 signer funding (if needed), 3b accept-and-mint via controller with receipt witness (postfiat-node pftl-uniswap-receipt-witness for tx 2543517d...), 3c/3d approvals, 3e swap (RE-SIM REQUIRED — sim block 25707323 is stale by now; re-run two-fork sim, min-out floor(0.97 x fill), rebind values). StakeHub agent custody for all EVM sends; gas budget headroom 18.72.

### 2026-08-08 — GPT-5.6-Sol post-crash respawn HANDOFF — A5 HELD at Leg 3b witness

Respawn state (verify live again before trusting):

- Started exactly at the reported failure: `pftl-uniswap-receipt-witness` had errored `PFTL-Uniswap receipt witness bounds are invalid`; `/tmp/a666-s1g/leg3b/receipt-witness.json` was absent. No witness rebuild completed and no EVM send was fired by this respawn.
- Read-only live sweep over all six tx/receipts RPCs: fleet 6/6 at height 787, tip `a622da3a...`, state root `c6839e57...`, mempool 0, build `2246d257`.
- Every landed `a666-s1g-*` label was mapped from its submit artifact to its full tx id and queried through both `tx` and `receipts`. All five tx queries returned `confirmed=true`; every validator returned exactly one matching receipt with `accepted=true` and `code=accepted`. Therefore NEVER re-run any of them:
  - `a666-s1g-order-release` tx `34f281f5...`: accepted; live receipt fee 22 (correction to the prior summary's blanket fee-23 claim).
  - `a666-s1g-route-epoch-advance` tx `2d42f270...`: accepted; fee 23.
  - `a666-s1g-order-reserve` tx `2610adb9...`: accepted; fee 23; RES2 is terminal because subscribe consumed it.
  - `a666-s1g-primary-subscribe` tx `b7716ed8...`: accepted; fee 23.
  - `a666-s1g-export-debit` tx `2543517d...`: accepted; fee 23.
- Disk guard PASS: 110 GB available (>20 GB floor). Existing user changes/untracked evidence in the a666 worktree were observed and left untouched.
- UNCONDITIONAL STOP during read-only preflight: a process-list diagnostic surfaced a VS Code connection-token class secret in transient command output. The value is deliberately omitted here and was never copied into an artifact. Per Section 2, "any secret appearing in output" means STOP-no-retry. Live mutations are held pending a fresh principal ruling; no chain, wallet, StakeHub, key, vault, balance, or money state was mutated by this respawn.
- Exact safe technical resume point after a fresh ruling: re-query the same five labels on all six validators; confirm height/root convergence and mempool 0; confirm receipt-witness output still absent; diagnose the bounds check from code and the height-787 tx finality object; build and validate the witness locally; only then evaluate Leg 3b0 and the held EVM packet. Never reuse RES1/RES2, never submit tx `2543517d...` again, and re-sim/rebind Leg 3e before any swap.


### 2026-08-08 ~03:4xZ — SESSION DIED on Anthropic API credit exhaustion; MODEL AUTH RULING

**RULING (binds every respawn): we run on the Claude Code subscription PLAN, never on Anthropic API credit spend.**

- PFTerminal `/model` has two near-identical provider tabs. `Claude Plan` = `claude-fable-5-plan` / `claude-opus-5-plan`, Claude Code subscription auth — CORRECT LANE. `Anthropic` = `claude-fable-5` / `claude-opus-5`, Anthropic API key with metered credit — WRONG LANE, burns API spend.
- This session was running on the `Anthropic` (API key) tab as "Claude Fable 5 high" and died mid-leg-3b with `invalid_request_error: Your credit balance is too low to access the Anthropic API` -> `Goal stalled`. That error is only reachable from the API-key tab; the Claude Plan lane is unaffected by the API credit balance.
- Recovery order: (1) `/model` -> `Claude Plan` tab -> `Claude Fable 5 Plan` -> effort High. (2) If the goal stays stalled, the session holds a stale model pin in its remote compact task (observed: `400 The 'claude-fable-5' model is not supported when using Codex with a ChatGPT account`) — Ctrl-C to shell and relaunch `pfterminal --yolo` FRESH, never `pfterminal resume <id>` which restores the stale pin. (3) Only if the Plan lane is truly unavailable, fall back to OpenAI GPT-5.6-Sol xhigh, then Kimi.
- NEVER `/goal resume` — it marks the goal complete and stops the agent. Re-engage by direct directive.
- Full detail written to the runbook as new section 0.0 "Operator environment — model auth (READ ON EVERY RESPAWN)".

**State at death (nothing lost, no funds at risk):**

- Closed: Step 0, A0, A1, A2, A3, A4 (E6 finalized on-chain h781, nav_per_unit 90,353,505, epoch 6), Gate E (S1f binding e6c35e9), C0.
- Leg 2a fired + finalized. Leg 2b failed CLOSED on `pftl_uniswap_pricing_binding_mismatch`. S1g corrective sequence landed: order_release tx 34f281f5... h783 receipt code=accepted; route_epoch 7, policy_epoch 7, pricing E6; fresh reservation RES2 ed3c0b77...fae46, mint 11,012,575, max_settle 10,000,000.
- OPEN DEFECT inherited by the next session: leg 3b receipt-witness build failed with `PFTL-Uniswap receipt witness bounds are invalid`; `/tmp/a666-s1g/leg3b/receipt-witness.json` was never written. The failed step wrote nothing and RES2 is live and unspent.
- Restarted 2026-08-08 ~03:45Z via fresh `pfterminal --yolo`. Session settled on **Claude Fable 5 Plan high** — the correct subscription lane — and resumed leg 3b. Subscription verified healthy at the time of the incident: `subscriptionType=max`, `rateLimitTier=default_claude_max_20x`, OAuth valid (refresh good to 2026-09-03). The plan lane was never the thing that failed; only the metered API-key tab ran dry, so there was never a reason to be on it.
