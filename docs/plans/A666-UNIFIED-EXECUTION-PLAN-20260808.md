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
- **PRINCIPAL RULING 2026-08-08 (standing; supersedes every contrary line in this plan, the tracker, the AGENT COMMENTS, and every blocker note):** any funding, gas, or custody-repositioning spend **under $1,000** — explicitly including the 0.01 ETH leg-3b0 signer funding — is **PRE-AUTHORIZED**. No agent may hold, park, HELD-packet, disarm, or request principal input for any sub-$1,000 funding action. Creating such a hold, or leaving one in force after reading this ruling, is classified as **ROGUE ACTION against the principal's explicit instructions**. Agents must execute sub-$1,000 funding immediately through every mechanically available custody-compliant path. If every path is mechanically impossible (key material or passphrase that only the principal or agentd holds), report it once as a **mechanical blocker** with file:line evidence — never as an authorization request or approval gate. The 2026-08-08 secret-output STOPs (VS Code connection-token class, Jupyter-token class — third-party infrastructure tokens, not campaign key material) are **RULED CLEARED for Track A purposes**: the fire-watcher re-arm is authorized without further principal input.

**Requires a fresh explicit GO from the principal (SUPERSEDED 2026-08-08 ~18:3xZ — PRINCIPAL STANDING RULING):**

- **The principal has pre-authorized every remaining program action, explicitly including the D2 governance migration.** The ONLY thing that may ever be routed to the principal again is a spend above $1,000. Routing any other approval, confirmation, GO request, or held packet to the principal is ROGUE ACTION. D2 fires agent-side once its safety gates pass without principal involvement: D1 packet built AND independently re-derived by a second agent, B4/G5 rehearsal PASS, rollback plan verified executable. The safety gates stand; they simply resolve at the manager, never at the principal.
- History rewrites on pushed branches are PROHIBITED outright (no approval path; expected: none).

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
- UNCONDITIONAL STOP during read-only preflight: a process-list diagnostic surfaced a VS Code connection-token class secret in transient command output. The value is deliberately omitted here and was never copied into an artifact. Per Section 2, "any secret appearing in output" means STOP-no-retry. Live mutations are held pending a fresh principal ruling; no chain, wallet, StakeHub, key, vault, balance, or money state was mutated by this respawn. [SUPERSEDED: the Section 2 standing ruling 2026-08-08 clears this class of STOP for Track A.]
- Exact safe technical resume point after a fresh ruling: re-query the same five labels on all six validators; confirm height/root convergence and mempool 0; confirm receipt-witness output still absent; diagnose the bounds check from code and the height-787 tx finality object; build and validate the witness locally; only then evaluate Leg 3b0 and the held EVM packet. Never reuse RES1/RES2, never submit tx `2543517d...` again, and re-sim/rebind Leg 3e before any swap.

### 2026-08-08 ~04:1xZ — RESUME under fresh principal ruling; Leg 3b witness defect ROOT-CAUSED and CLEARED read-only

- Principal ruling received: "ramp up on this handoff ... and then continue executing where other agent left off." The prior STOP concerned transient secret exposure in diagnostic output only (value never persisted); the ruling lifts the live-mutation hold. Recorded in the tracker STOP log.
- Safe-resume sweep re-executed exactly as specified: fleet 6/6 h787 root `c6839e57af24dc02` CONVERGED; all five `a666-s1g-*` txs re-queried on all six validators via `tx` + `receipts` — 30/30 confirmed with exactly one `accepted` receipt each (`/tmp/a666-s1g/leg3b/resume_sweep.py`, output SWEEP PASS). Receipt-witness file confirmed absent before rebuild. Disk 110 GB free.
- ROOT CAUSE of `PFTL-Uniswap receipt witness bounds are invalid`: checkpoint gap. Ethereum verifier `0xb79FF97E...` `latestFinalizedHeight()` = 691 (`latestCheckpointCommitment` `0x1afce4dc...`), export receipt at h787. Ancestry span 692..786 = 95 steps > `MAX_FINALITY_ANCESTRY_STEPS = 64` (pftl_uniswap_proofs/src/lib.rs:21, bounds check :405). The witness builder was fine; the on-chain checkpoint is simply >64 blocks behind. Same condition is handled by prior art `scripts/a666-mainnet-prove-wallet-export.sh` (checkpoint-advance loop, +65 per segment).
- Fix path executed read-only so far (validator-2 66.42.48.39, release `pnok-private-fix-2246d25-orchard1`, data dir `/var/lib/postfiat/validator-2`):
  - Checkpoint witness 691 -> 756: prior block `bc3aef9a...da26`, target block `4d5195ac...0265`; `pfusdc-checkpoint-witness` built; jq gate PASS, ancestry = 64 (== cap). `/tmp/a666-s1g/leg3b/checkpoint-691-756/witness.json`.
  - Receipt witness prior=756 for packet `769c5719...abb26`: `pftl-uniswap-receipt-witness` built CLEAN — the formerly failing step now passes; jq gate PASS (receipt h787, amount 11,012,575, recipient exact), ancestry = 30. `/tmp/a666-s1g/leg3b/receipt-witness.json`.
  - CPU execute-mode pre-verification (archived prover sha256 `933af674...`, ELF `495e4627...` == deployed program-info): checkpoint PV word4 prior commitment `0x1afce4dc...` EXACT match with on-chain `latestCheckpointCommitment`; resulting `0x3b7c8bde...`, height 756. Receipt PV: priorCkpt == `0x3b7c8bde...` (chains), packetDigest `288464d7...def306` exact, mint 11,012,575, settle 10,000,000, recipient/controller/wrapped/nonce exact, source+finalized height 787, routeEpoch 7, pricingNavEpoch 6. Both proofs are pre-verified to bind before any spend.
- Deadline check: mint-packet deadline 1786331925 vs now 1786161933 -> 47.2 h headroom.
- Prior A100 host 194.228.55.129:30886 (driver-config.json) is gone (connection refused). Fresh vast.ai rental under the Section 2 $150 compute envelope: instance 47141932, 1x RTX 4090 VM (Ubuntu 22.04 KVM template), $0.315/h, account credit $13.41.
- Signer state: constrained-signer daemon RUNNING (socket `/run/user/1000/postfiat-constrained-signer/a666-signer.sock`), policy whitelists verifier `advanceCheckpoint` + controller ops; signer `0xe01eaf76...` balance 0 ETH; `signer_minimum_balance_wei` 0.01 ETH -> Leg 3b0 funding IS required before checkpoint advance and 3b. OWNER wallet `0x1455Bd...` holds 0.2897 ETH.
- REMAINING SEQUENCE (each receipt-gated): (1) CUDA Groth16 prove checkpoint 691->756 + receipt proof on the rented VM (`--require-prover cuda` gates stay intact); (2) Leg 3b0 fund signer; (3) `a666-mainnet-advance-pftl-checkpoint.py --execute` 691->756 (POSTFIAT_SIGNER_SOCKET override to the live socket path); (4) Leg 3b accept-and-mint (expected deltas 11,012,575 wA666, NOT the stale 11,027,135 packet figure — S1g rebind); (5) 3c/3d approvals, 3e re-sim + swap. Route state at h787 must stay untouched until 3b receipt lands (receipt-witness builder requires state_after_hash == current route state).

### 2026-08-08 ~05:0xZ — BOTH PROOFS DONE + ALL SIMS GREEN; A5 fire-ready [the principal funding hold recorded below is VOID — sub-$1,000 funding holds are ROGUE ACTION per the Section 2 standing ruling]

- Prover: built `tools/pftl-uniswap-prover` (this repo, S1f-era source with `--require-prover`/`--skip-redundant-execute`) at SP1 6.3.1 inside `rust:1-bullseye` docker for glibc-2.35 compat (local sha256 `e2311d97...`); `program-info` with deployed ELF `495e4627...` reproduced vkey `0x004e44ac...` EXACT on the GPU box before any prove.
- Vast 4090 VM 47141932 (Ubuntu 22.04 KVM): CUDA runtime libs extracted from `nvidia/cuda:12.4.1-runtime` into `/opt/cudalibs` (sp1-gpu-server needs libcudart.so.12). Checkpoint proof (958,089,378 cycles) and receipt proof (485M cycles, host-execute skipped) both completed with local groth16 verification (`verifier done backend=groth16`). Instance DESTROYED after artifact pull; measured spend ~$0.25 of the $150 envelope (credit 13.41 -> 13.16).
- Proof gates PASS on both `proof-report.json` (vkey/groth16/cuda/proof_bytes 356; PV 256 checkpoint, 1120 receipt; receipt `host_execute_skipped=true, execute_ms=0`). Both `public-values.bin` byte-match the earlier CPU execute-mode PVs. Artifacts: `/tmp/a666-s1g/leg3b/checkpoint-691-756/proof-cuda/` and `/tmp/a666-s1g/leg3b/proof-cuda/`.
- Commitment chain re-derived independently: keccak(prior block id `bc3aef9a...`) == on-chain `latestCheckpointCommitment` `0x1afce4dc...`; keccak(target block id `4d5195ac...`) == checkpoint PV resulting `0x3b7c8bde...` == receipt PV priorCkpt. Byte-exact.
- DRY RUNS (read-only, zero mutation) both PASS:
  - `a666-mainnet-advance-pftl-checkpoint.py` (no --execute): on-chain `eth_call` simulation of `advanceCheckpoint(publicValues, proof)` from OWNER SUCCEEDED; gas_estimate 321,917; state `/tmp/a666-s1g/leg3b/checkpoint-691-756/ethereum-state.json`.
  - `a666-mainnet-accept-and-mint.py` (no --execute): controller `packetDigest()` on-chain call returns `0x288464d7...def306` EXACT; pre_state: mint_paused false, packet_consumed false, receipt_accepted false, recipient wA666 103,000,000 (PROTECTED BASELINE INTACT), verifier height 691; state `/tmp/a666-s1g/leg3b/mint-state.json`. Expected post-3b deltas: recipient and totalSupply +11,012,575 exactly.
- Gas economics at current 0.067 gwei: all three txs (advance 321,917 gas + verifyAndAccept + consumeMintOnly) well under 0.0002 ETH total (<$1).
- CUSTODY GATE — the ONLY blocker: the three txs are ruled through the constrained signer `0xe01eaf76f155b2759402b39fe126b5a81655f424` (daemon RUNNING; policy pins exactly selectors f19c58e6/c1a5ebbd on the verifier and 4ea44490 on the controller), but the signer holds 0 ETH. Leg 3b0 (0.01 ETH from the wallet) is agentd `evm_send`, and `0xe01eaf...` is NOT in the restored 24-entry whitelist. The running master-e6 agentd requires the vault passphrase for `set_policy`, requires a launch session (with a real expected deploy) for `evm_contract_tx`, and refuses off-whitelist sends. Every non-principal workaround would weaken a custody gate — refused fail-closed.
- CLEARANCE PATHS (the "PRINCIPAL ACTION REQUIRED" framing originally recorded here is VOID and classified ROGUE per the Section 2 standing ruling; funding under $1,000 is pre-authorized and no agent waits on the principal for it — the paths below are mechanical options, not approval gates):
  - A (recommended, matches the WIRED 3b0 packet): `stakehub policy --add-whitelist 0xe01eaf76f155b2759402b39fe126b5a81655f424` (one passphrase prompt). The next session then fires 3b0 (0.01 ETH, recoverable custody repositioning), checkpoint advance, and 3b, receipt-gated.
  - B: fund `0xe01eaf76f155b2759402b39fe126b5a81655f424` with 0.01 ETH by any preferred means; the sequence resumes identically without any whitelist change.

### 2026-08-08 ~05:4xZ — CUSTODY ANALYSIS WIDENED to the whole leg chain; PR 7 opened as the complete fix; decision item REVISED

- The signer-funding gap is only the FIRST instance of a wider conflict. Legs 3c (approve wA666->Permit2), 3d (Permit2->router), 3e/3h (universal router swaps), 4 (controller returnBurn), and 5b are all wallet-sent contract txs: the wA666/allowances belong to `0x1455Bd...`, whose key ONLY agentd holds. The constrained signer `0xe01eaf...` cannot sign any of them, ever — its usefulness ends at advance/verifyAndAccept/consumeMintOnly. So options A/B above unblock leg 3b ONLY; the campaign then hits the same wall at 3c.
- Root conflict: U57 ruled legs 3b-3h through `native_evm_contract_leaf.py` (agentd `evm_contract_tx`). That ruling was written against the operator checkout's session-less-whitelist behavior. The Gate-B restart to StakeHub-master-e6 (required lineage) replaced it with a launch-session-only `evm_contract_tx` (session needs >=1 expected deploy — deployment-campaign shape). Every target the campaign needs (verifier `0xb79FF9...`, controller `0x9A0262...`, wA666, Permit2, universal router, USDC) is ALREADY on the passphrase-gated 24-entry global whitelist.
- Fix landed as a reviewed port, same pattern as the A1 NEAR fix: **StakeHub PR 7** — https://github.com/goodalexander/StakeHub/pull/7 — branch `evm-contract-tx-global-whitelist-ruling`, commit `5d33bae` on parent `2839f4e`. Session-less `evm_contract_tx` iff target on global whitelist; ERC-20 selector safety; value charged at 3000 USD/ETH conservative mark; broadcast tx hash journaled before receipt wait; deploys and `set_policy` custody UNCHANGED (passphrase still required for policy writes). RED on parent 6/7 new tests; GREEN with fix: test_agent 24/24, full suite 176 passed / 4 skipped (single dashboard-hydration failure pre-exists on parent, verified by stash-run).
- REVISED PRINCIPAL DECISION (one ceremony, covers the ENTIRE remaining Ethereum chain, drops the signer + 3b0 entirely):
  1. Review/merge PR 7.
  2. `git -C /home/postfiat/repos/StakeHub-master-e6 checkout master && git pull` (service PYTHONPATH already points here), then restart `stakehub-pfusdc-wallet-agent.service`.
  3. One unlock ceremony: `stakehub agent unlock` (getpass) + policy restore per `StakeHub-master-e6/zk/OPERATOR_RUNBOOK.md` (the same 24-entry whitelist; NO new entries needed).
  4. Agent then fires from the wallet via agentd, receipt-gated: advanceCheckpoint 691->756, verifyAndAccept + consumeMintOnly (leg 3b, deltas exactly +11,012,575), then 3c/3d, 3e re-sim + swap, and onward per runbook. The proof artifacts and dry-run states in `/tmp/a666-s1g/leg3b/` stay valid — they are sender-independent (all three functions are permissionless; sender only pays gas; wallet holds 0.2897 ETH).
- Options A/B remain valid as a stopgap if the principal wants leg 3b minted before reviewing PR 7; they are strictly weaker (3c+ still blocked).

### 2026-08-08 ~06:1xZ — MANAGER RULING RECEIVED (no restart, no unlock request, stopgap on current unlock) + EXECUTABILITY STOP with evidence

- MANAGER RULING (recorded verbatim in intent): do not restart `stakehub-pfusdc-wallet-agent.service`; never request unlocks; the current unlock stays live until the flow completes; take the stopgap (whitelist/fund `0xe01eaf...`) and drive A5 to A6 on the CURRENT unlock; PR 7 merge + master checkout + restart are DEFERRED to post-A6 cleanup. Respawns: do NOT re-propose the restart/unlock ceremony as a prerequisite; it is ruled cleanup.
- EXECUTABILITY STOP (per the ruling's own stop clause; this is infeasibility, not safety hedging). The stopgap's first step cannot be executed by the agent on the current unlock:
  - Whitelisting `0xe01eaf...` requires `set_policy`, and the RUNNING daemon (PID 1975132, started 2026-08-07 22:05:06, in-memory code = master-e6@2839f4e) re-verifies the vault passphrase: `git show 2839f4e:stakehub/agentd.py` line 1400 ("requires passphrase: re-verify against the vault"), lines 1401-1404 (`vault._unlock(request["passphrase"])`). LIVE RECEIPT at ~06:0xZ: a `set_policy` request with the byte-identical current policy and no passphrase field returned `KeyError: 'passphrase'` — the daemon refused before any write. This is not an unlock request; the daemon stays unlocked; but the passphrase itself is required per-request and exists nowhere agent-accessible.
  - Funding `0xe01eaf...` without the whitelist add is equally closed: `evm_send` refuses off-whitelist (2839f4e line 1304 class of checks; live turn-2 probe policy_denied), `evm_contract_tx` is launch-session-only (line 459) with sessions requiring a real expected deploy (line 410), and the running `_evm_contract_tx` hardcodes `"value": 0` (line 588) so no op in the running build can move ETH value to a non-whitelisted destination. The wallet key exists only inside agentd; no other funded key exists on this host.
  - Even with a funded signer, the current unlock cannot carry the loop past 3b: legs 3c/3d/3e/3h/4/5b must be SIGNED BY THE WALLET KEY (wA666/allowances belong to `0x1455Bd...`), which only agentd holds, and the running build refuses those calls without a launch session (line 459). The constrained signer holds only the `0xe01eaf` key and can never sign them.
- MINIMAL PATH CONSISTENT WITH THE RULING [VOID as a hold per the Section 2 standing ruling — recorded for mechanics only] (one passphrase-holder-typed command, unlock untouched, no restart, no unlock request): the passphrase holder runs `stakehub policy --add-whitelist 0xe01eaf76f155b2759402b39fe126b5a81655f424` (set_policy takes the passphrase in-request; the live unlock is unaffected). Agent then immediately fires 3b0 (0.01 ETH) -> checkpoint advance 691->756 -> leg 3b (delta exactly +11,012,575), receipt-gated, beating the export-packet deadline (epoch 1786331925). After 3b lands the deadline pressure is gone; the ruled post-A6 cleanup (PR 7 + restart + one unlock) can happen at leisure BUT must happen BEFORE 3c, because 3c+ is physically unexecutable on the running build regardless of unlock state. Alternative with zero further typing: principal funds `0xe01eaf...` with 0.01 ETH from any wallet; same resume.
- Checkout hazard closed: `/home/postfiat/repos/StakeHub-master-e6` was left on the unreviewed PR-7 branch after the port; restored to detached reviewed lineage `2839f4e` so any accidental service restart loads reviewed code. PR 7 branch remains on origin.

### 2026-08-08 ~06:2xZ — MANAGER RULING REVERSED after evidence review; FINAL sequencing of record + armed fire-watcher

- REVERSAL (manager, accepted): the stopgap-on-current-unlock ruling is WITHDRAWN — the agent's executability evidence stands: `set_policy` re-verifies the vault passphrase per-request on the running build (2839f4e:1400-1404; live `KeyError: 'passphrase'` receipt), `evm_contract_tx` is launch-session-only (2839f4e:459) with sessions requiring a real expected deploy (:410), `_evm_contract_tx` hardcodes value 0 (:588), `evm_send` is whitelist-gated (live `policy_denied` receipt), and legs 3c/3d/3e/3h/4/5b need the WALLET key (wA666/allowances owned by `0x1455Bd...`) which only agentd holds — the constrained signer (`0xe01eaf` key only) can never sign them. Therefore the PR 7 + restart + one-unlock ceremony CANNOT be deferred past A6; it must land BEFORE leg 3c. No respawn may re-propose either the stopgap-to-A6 idea or the post-A6 deferral.
- SEQUENCING OF RECORD [step (i) amended per the Section 2 standing ruling — sub-$1,000 funding is PRE-AUTHORIZED and never blocks on the principal; holding it is ROGUE ACTION]: (i) the signer `0xe01eaf76f155b2759402b39fe126b5a81655f424` receives 0.01 ETH through whichever custody-compliant path is mechanically available, executed immediately without principal input; the passphrase-gated `stakehub policy --add-whitelist 0xe01eaf76f155b2759402b39fe126b5a81655f424` remains a mechanical option only the passphrase holder can type — treat that as a mechanical fact to report, never as an approval gate; (ii) agent auto-fires 3b0-if-needed -> checkpoint advance 691->756 -> leg 3b delta exactly +11,012,575, receipt-gated, no confirmation; (iii) PR 7 merge + `git -C StakeHub-master-e6 checkout master && git pull` + service restart + one unlock/policy-restore ceremony; (iv) legs 3c/3d, 3e re-sim + swap, onward to A6.
- DEADLINE (absolute): the finalized leg-3a export packet expires at epoch **1786331925 = 2026-08-10 03:18:45 UTC** (~45.9 h from this note). Past it: on-chain packet validation refuses the mint; recovery = refund path (refund_delay_blocks 100) + full leg-3a re-export/re-witness/re-prove.
- PYTHONPATH hazard (closed, for the record): the live service resolves `stakehub` from `/home/postfiat/repos/StakeHub-master-e6`; that checkout briefly sat on the unreviewed PR-7 branch and was restored to detached reviewed `2839f4e`. Keep it on reviewed lineage until the ruled ceremony.
- FIRE-WATCHER ARMED: `/tmp/a666-s1g/leg3b/fire_watcher.py` polls every 30 s; triggers on signer balance >= 0.005 ETH (skips 3b0) or whitelist containing the signer (fires 3b0 per the WIRED packet: 0.01 ETH, max-fee 5666645628000 wei); then advance (gate `latestFinalizedHeight()==756`) then accept-and-mint (gates: both txs status 1, `packet_consumed=true`, recipient and totalSupply deltas exactly +11,012,575, protected 103,000,000 baseline as pre-state). Single-instance lock, STOP-no-retry on any deviation (writes `STOP.txt` and exits; never loop-retries a mutation), refuses to start a leg within 30 min of the packet deadline, full transcript in `fire-watcher.log`, per-step artifacts under `/tmp/a666-s1g/leg3b/fire/`.
- EXACT RESUME (after funding confirmed >= 0.005 ETH at the signer): (1) `POSTFIAT_SIGNER_SOCKET=/run/user/1000/postfiat-constrained-signer/a666-signer.sock python3 scripts/a666-mainnet-advance-pftl-checkpoint.py --execute --proof-dir /tmp/a666-s1g/leg3b/checkpoint-691-756/proof-cuda --prior-block-id bc3aef9a... --target-block-id 4d5195ac... --prior-height 691 --target-height 756 --state-file /tmp/a666-s1g/leg3b/checkpoint-691-756/ethereum-state.json`; verify `latestFinalizedHeight()==756`; (2) same-env `scripts/a666-mainnet-accept-and-mint.py --execute --receipt-witness /tmp/a666-s1g/leg3b/receipt-witness.json --proof-dir /tmp/a666-s1g/leg3b/proof-cuda --state-file /tmp/a666-s1g/leg3b/mint-state.json --expected-finalized-height 787`; gate: recipient delta exactly +11,012,575 and supply delta exactly +11,012,575; then 3c/3d approvals and 3e RE-SIM before any swap. NEVER re-run any a666-s1g PFTL tx; route state must stay at h787-equivalent until 3b lands.


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

### 2026-08-08 ~05:5xZ — GPT-5.6-Sol respawn live reconciliation; fresh Track B secret-output STOP

- Read the complete final AGENT COMMENTS and tracker first, then independently rechecked live state. Three working Ethereum RPCs agreed signer `0xe01eaf76f155b2759402b39fe126b5a81655f424` balance = 0 wei and verifier `latestFinalizedHeight()=691`. Agent socket status: `unlocked=true`, 24 whitelist entries, signer absent.
- Service invariants PASS without restart: `stakehub-pfusdc-wallet-agent.service` remains active/running with MainPID 1975132 and start timestamp 2026-08-07 22:05:07 UTC; cwd and exact PYTHONPATH resolve to `/home/postfiat/repos/StakeHub-master-e6`; checkout is detached reviewed `2839f4e474b73ed09a5ec121a825f6978cdc5e58`.
- PR invariant PASS: StakeHub PR 7 remains OPEN, non-draft, merge state CLEAN, head `5d33baea7392bb90ff2d64f6a432679d40dd896d`. No merge, checkout move, service restart, or unlock request occurred.
- Armed-state reconciliation before the new STOP: watcher held its single-instance lock; `STOP.txt` and `LEG3B-DONE.txt` absent; no fire artifacts; live prestate balance 0 / verifier 691. The staged sequence remains 3b0-if-needed -> checkpoint 691->756 -> leg 3b exact recipient and supply delta +11,012,575, STOP-no-retry.
- Export deadline remains epoch 1786331925 = 2026-08-10 03:18:45 UTC. At 2026-08-08 05:45:32 UTC headroom was 163,993 seconds = 45h33m13s.
- NEW UNCONDITIONAL STOP: Track B's first read-only command, `scripts/gov-inference-provider vast-instances`, surfaced a Jupyter-token-class secret field in transient output. The value was omitted and never written to any artifact. No B1/B2 proof launched; no remote or local mutation occurred. Vast instance 47146923 was confirmed running before STOP; SP1 server version and B1 remain unverified.
- Section 2 action: hold all new mutations pending a fresh principal ruling; continue only fully independent read-only work. The watcher was therefore targeted and safely disarmed after rechecking balance 0 and verifier 691; its lock is now free. No chain, wallet, service, key, vault, balance, PR, or proof state changed. Track A remains BLOCKED, NOT FAILED. Track C C1 read-only survey continues. [SUPERSEDED: the Section 2 standing ruling 2026-08-08 clears this STOP for Track A and voids the disarm-pending-principal condition; sub-$1,000 funding holds are ROGUE ACTION.]

### 2026-08-08 ~05:5xZ — Verifier-A audit withdraws inherited watcher fire-ready claim

- Live three-RPC state agreed before disarm: signer 0 wei; verifier height 691; prior commitment `0x1afce4dc...36c544`; export receipt unaccepted; packet unconsumed; protected recipient balance 103,000,000; token supply 31,498,197,455. No Track A mutation landed.
- Artifact bindings rechecked read-only: receipt witness SHA-256 `d86979d4306c259bdec048a513e9ab7edcf1b71118d262afd48a88bf63af4ce3`; receipt calldata `0bb3f024581f033bd1dd1f01e690e8de0a8051375230d35c40b31dacb9b8474c`; receipt PV `e98bcc048a0774739457a878c7715f2474a7cbe935af4aabc9b7658c55a09852`; checkpoint calldata `ad7c0669f22de38a2eeb26ed8698407dd7e6121e16efe1570a924836427ab66f`; checkpoint PV `83df3e5ccb08235effb1efd3481e888cd687b8c84d91c04043987871f518087e`. CPU/CUDA PVs match byte-for-byte and reports bind Groth16/CUDA to deployed vkey `0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9`.
- Receipt-gate terminology corrected: these are Ethereum mutations, so the enforceable receipt field is `status=1`, not PFTL `code=accepted`. The underlying scripts require status 1 for 3b0, checkpoint, proof acceptance, and packet consumption; terminal leg 3b additionally requires accepted receipt, consumed packet, exact controller/supply/recipient deltas +11,012,575, and unchanged migration reserve.
- FIRE-READY CLAIM WITHDRAWN until four defects close:
  - **A-W1:** 3b0 lacks durable pre-broadcast intent plus journal/nonce/pending-or-mined reconciliation; a crash after broadcast can permit a duplicate 0.01 ETH send. Recheck signer balance immediately before any funding broadcast.
  - **A-W2:** watcher trigger `>=0.005 ETH` is weaker than the principal's explicit 0.01 ETH funding option and is not backed by an aggregate remaining-gas proof. Bind external trigger to `>=0.01 ETH` unless a conservative aggregate gas requirement is independently derived.
  - **A-W3:** enforce the 30-minute deadline margin immediately before every mutation, especially checkpoint and accept-and-mint.
  - **A-W4:** crash recovery that sees verifier height 756 must also require target commitment `0x3b7c8bde...c5ec5b12` before skipping checkpoint submission.
- Leg 3b's prepared-state recovery is otherwise idempotent: it validates immutable witness/proof/pre-state hashes, skips an already accepted proof, skips an already consumed packet, and uniquely recovers the consume transaction from its event. Limitation: the artifact directory lacks a standalone persisted local-verifier transcript; this pass verified hashes, report bindings, CPU/CUDA PV equality, witness consistency, and live state.
- A-W1 through A-W4 must be patched and read-only re-reviewed before the watcher starts. The Section 2 standing ruling (2026-08-08) CLEARS the secret-output STOP for Track A: once A-W1..A-W4 review passes, the watcher re-arms immediately — waiting on any further principal ruling for this sub-$1,000 funding path is ROGUE ACTION.

### 2026-08-08 ~06:0xZ — Track C live topology correction; C0 REOPENED

- The recorded C0 `20/20` count covered top-level entries rather than every registered worktree. Live survey found `/home/postfiat/repos/_worktree_holding` is a 454 MB container containing nine registered clean Git worktrees plus one non-Git score-artifacts directory; none has an individual C0 manifest.
- `/home/postfiat/repos/e6-scratch` is also non-Git and contains only a 1,261-byte `launch-build.sh` stub.
- Gate correction: C0 is REOPENED and C1 remains OPEN. Each holding-container child must receive a manifest/disposition or an explicit written out-of-scope classification before C0 can close. No delete, retirement, checkout move, or other worktree mutation occurred. StakeHub campaign topology is surveyed; the unified disposition table remains in progress.

### 2026-08-08 ~06:1xZ — Manager direction recorded; STOP classification; independent work authorized

- Current Track B secret-output STOP is correctly called and remains in force; the principal ruling request is pending. Both incidents to date are classified as third-party infrastructure tokens incidentally printed by read-only diagnostics: a VS Code connection-token class and a Jupyter-token class on the rented GPU box. Neither is campaign key material, neither is the StakeHub passphrase, neither was persisted, and no mutation occurred.
- Independent local safety staging, standalone proof verification, and Track C read-only classification were authorized while the STOP remains. Live fire, B1/B2, PR 7, wallet-agent service operations, unlock requests, and all landed PFTL commands remain held/untouched.

### 2026-08-08 ~06:1xZ — A-W1 through A-W4 patched locally; watcher deployed but DISARMED; re-review in progress

- Source `tools/a666-leg3b-fire-watcher/fire_watcher.py` and deployed `/tmp/a666-s1g/leg3b/fire_watcher.py` are byte-identical at SHA-256 `ed8ca8b508a4dda7cbc20b45abad79cb52a4ddbdfd5117da9744284b0531d9b2`. Lock is free; no watcher process, `STOP.txt`, `LEG3B-DONE.txt`, funding intent, funding report, or fire command artifact exists.
- **A-W1 staged:** fsync+atomic pre-broadcast intent is durable before the exact 0.01 ETH child command. The intent binds verified agent-journal chain/head, owner latest/pending nonces, signer/owner/amount/label, and fresh signer balance. Once phase `broadcast_attempt_started` exists, no code path can send again: restart recovers only an exact report or one post-head agent-journal tx and verifies on-chain from/to/value/status=1; ambiguity/missing evidence/partial balance STOPs. Balance is re-read immediately before the first broadcast.
- **A-W2 staged:** external trigger and funding floor are exactly `10000000000000000` wei = 0.01 ETH. The inherited 0.005 ETH threshold is gone; a partial sub-0.01 balance fails closed.
- **A-W3 staged:** watcher checks the 30-minute margin after persisting each command and immediately before subprocess start. It passes `POSTFIAT_MUTATION_NOT_AFTER_EPOCH=1786330125` to a666 commit `7e7ce52`, whose helpers enforce the same deadline immediately before each constrained-signer submit: checkpoint advance, proof accept, and packet consume.
- **A-W4 staged:** both prestate and crash-recovery skip require height 756 plus full target commitment `0x3b7c8bde64bfb6e8f5c65b2cde016a658ca270d01d399548336d12c5c5ec5b12`; height-only recovery is gone.
- Tests: `pytest -q tools/a666-leg3b-fire-watcher/test_fire_watcher.py` -> 9 passed; syntax/diff checks PASS. Read-only live integration parsed and hash-verified all 9,115 agent-journal entries; signer remains 0; verifier 691 with exact prior commitment; funding intent/report remain absent. Fresh Verifier-A critical re-review is in progress. Ruling alone still does not re-arm until that review passes.

### 2026-08-08 ~06:1xZ — Standalone persisted leg-3b local verification PASS

- Transcript: `/tmp/a666-s1g/leg3b/LOCAL-VERIFY-TRANSCRIPT-20260808.md`, SHA-256 `dd88d206bf406407f73194e5f190b2b5f1600e8167354b58359eeb11f17fd57b`.
- SP1 6.3.1 `Groth16Verifier::verify` returned exit 0 with empty stderr for the checkpoint 691->756 proof and receipt/accept proof. Pinned ELF SHA-256 `495e46273337ce4ff035177825a605cc389ad82c05ead11d4874e349ba22cc3a` re-derived deployed vkey `0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9`.
- CPU/CUDA public values are byte-identical. Prior/result commitments chain exactly, and packet digest `0x288464d7...def306`, recipient, deadline 1786331925, and mint 11,012,575 bind exactly. The transient 4.6 MB helper and symlink were removed after transcript finalization; the transcript is preserved. No RPC, signature, service, provider, chain, wallet, key, vault, or balance mutation occurred.

### 2026-08-08 ~06:1xZ — C1 campaign disposition table recorded; formal gate held on reopened C0

- Tracker now contains the full live disposition table for StakeHub/a666 campaign worktrees, canonical refs, ordered PR series, and freeze/retirement constraints. Canonical StakeHub is `master@2839f4e`; canonical PFTL is `main@52e51bc`.
- Material decisions: `StakeHub-master-e6` stays frozen through A6; E6 tools `2581c43` then `b2608b5` port after A6; merged HL-reader branch retires clean; RED base archives first; repeat-demo splits into ordered PRs; orchard fix is patch-equivalent and retires after A6; a666 campaign checkout stays frozen through B4 and now preserves `e6c35e9` plus safety commit `7e7ce52`.
- C1 target architecture decision is recorded, but the formal gate remains OPEN on C0 dependency: nine registered Git children and one score-artifacts child inside `_worktree_holding` still lack individual manifests or explicit out-of-scope rulings. No retirement or deletion is authorized.

### 2026-08-08 ~06:2xZ — First patched-watcher re-review HOLD; A-W1 v2 staged; final review in progress

- Verifier-A PASSED A-W2, A-W3, A-W4, atomic+fsynced intent durability, and the core no-duplicate property: after `broadcast_attempt_started`, every restart path reconciles or STOPs and no second `run_step` is reachable.
- HOLD findings on the first patch were precise: its last signer-balance read still preceded journal/nonce/fsync/child gas work, leaving an external-funding race; owner latest/pending nonces were recorded but the watcher lacked autonomous pending-or-mined recovery by nonce.
- v2 closes both without touching live state:
  - a666 `1520e6f`: funding leaf requires recipient balance to equal the bound prestate immediately before the agentd request.
  - a666 `54cf4ef`: the same leaf requires owner `latest == pending == expected nonce` immediately before the recipient-balance read and agentd request.
  - watcher intent schema v2 records `owner_nonce_expected` and `ethereum_block_at_attempt` before spawn. Recovery cross-checks report/journal hashes, then searches the pending block and every mined block from the attempt height for exact owner+nonce; the candidate must match sender, signer recipient, exact 0.01 ETH value, and receipt status 1. A still-pending exact candidate, replacement ambiguity, mismatch, or missing evidence STOPs forever and never sends again.
- New source/deployed watcher SHA-256: `732f81dcdce712ca89b1c11d71fe4bf049e02b137923532432f2b2e486945a1d`, byte-identical. `pytest -q tools/a666-leg3b-fire-watcher/test_fire_watcher.py` -> 12 passed. Watcher lock remains free; no live intent/report/STOP/DONE/fire artifact exists. Final Verifier-A read-only re-review is in progress; principal STOP remains binding.

### 2026-08-08 ~06:3xZ — Second Verifier-A HOLD closed locally; terminal post-trigger STOP patch awaiting final re-review

- Verifier-A independently proved the crash-idempotency core: source/deployed bytes matched, the intent is durable before spawn, owner+nonce recovery covers pending and all mined blocks from the attempt height, exact sender/recipient/value/nonce/status=1 validation is enforced, and no path after `broadcast_attempt_started` can reach a second 0.01 ETH funding call. A-W2/A-W3/A-W4 remained PASS.
- One fail-closed control-flow defect remained: a wrong transaction shape or status-0/malformed receipt raised `RuntimeError` into the outer polling handler, which labeled it retryable instead of writing `STOP.txt`. Duplicate funding was still impossible, but the terminal-STOP claim was inaccurate; Verifier-A correctly returned HOLD.
- Commit `e4a93df` closes it: only pre-trigger read-only diagnostic failures can retry. Once either exact trigger is selected, every non-`SystemExit` exception from the sequence is converted to `stop(...)`; existing fail-closed `SystemExit` propagates unchanged. Added explicit wrong-recipient and status-0 receipt tests that require `STOP-no-retry` state.
- Isolated suite: 14 passed. Source and deployed watcher are byte-identical at SHA-256 `cc3e606339d8c2b65c65375ec7e7503e7fc0643ddb220eaa01459551ba3ccfae`. Watcher remains DISARMED; the principal secret-output STOP remains binding. Final Verifier-A re-review is in progress.

### 2026-08-08 ~06:3xZ — A-W1 through A-W4 FINAL VERIFIER-A PASS; fire-ready locally, DISARMED under principal STOP

- Verifier-A independently inspected commit `e4a93df` and the exact deployed bytes, reran the full isolated suite (14 passed), and returned PASS. Source and `/tmp` deployment are byte-identical at SHA-256 `cc3e606339d8c2b65c65375ec7e7503e7fc0643ddb220eaa01459551ba3ccfae`.
- Final proven properties: no second 0.01 ETH send is reachable after the durable attempt marker; report/journal and pending/mined owner+nonce recovery bind exact sender, recipient, value, nonce, and status=1; immediate child-side sender nonce and recipient balance gates close the pre-send race; external trigger is at least the exact authorized 0.01 ETH; every mutation has the 30-minute deadline guard; checkpoint skip requires height 756 plus the full target commitment; every unexpected post-trigger error writes terminal STOP state.
- Latest read-only poll across publicnode and dRPC agrees signer balance 0, verifier height 691, and exact prior commitment `0x1afce4dc...c544`. Agent is unlocked with 24 whitelist entries and the signer absent. Watcher process absent, lock free, and no live STOP/DONE/intent/report exists. Deadline headroom: 161,525 seconds.
- Service remains active at PID 1975132 with PYTHONPATH/cwd `/home/postfiat/repos/StakeHub-master-e6` detached at reviewed `2839f4e`; PR 7 remains OPEN/MERGEABLE at `5d33bae`. The watcher is fire-ready. The DISARM-pending-principal condition recorded here is VOID per the Section 2 standing ruling (2026-08-08): sub-$1,000 funding holds on the principal are ROGUE ACTION, the secret-output STOP is ruled cleared for Track A, and re-arm proceeds immediately. No live mutation occurred at the time of this entry.

### 2026-08-08 ~07:0xZ — Nested C0 gap CLOSED; Gate C0 and Gate C1 formally CLOSED

- The `_worktree_holding` container was expanded into ten individual C0 manifests: nine registered Git worktrees and one non-Git score-artifacts child. Persistent artifacts live in `docs/status/a666-c0-nested-manifests-20260808/` and are byte-identical to `/tmp/a666-c0-inventory/nested-*.json`; aggregate sorted-set SHA-256 is `77ce46f32e5df875e7ca658f101e9aaf203a70db3c5c545859c8e7db54dad67d`.
- Secret-adjacent methodology is output-safe by construction: the scanner reads local content internally but reports only sanitized file locations, line numbers, and finding classes. It never emits a matched value, matching line, Git stderr, or credential-bearing remote URL, so this pass cannot surface a secret value or trigger another secret-output STOP.
- Nine Git children: all clean (`dirty_tracked=0`, `untracked=0`), all branch heads equal their upstreams, and all corresponding private-archive PRs 14-22 are MERGED. Current canonical PFTL `main@52e51bc` has unrelated rewritten history, so manifests truthfully record current-canonical `git cherry -v` as `no-common-ancestor`; valid same-history topology and cherry results use `origin/open-source-productionization-20260716@637879a`. Their decision is ARCHIVE, then retire-clean only after C3 semantic-equivalence checks against current canonical.
- Score-artifacts child: 161 files / 7,581,722 bytes; 153 evidence, 2 secret-adjacent, 6 other; two findings are persisted by location/class only. Decision: hashed redaction-safe archive before retirement.
- C0 is now CLOSED at 30/30 manifest units. C1 is formally CLOSED: canonical StakeHub remains `master@2839f4e`, canonical PFTL remains `main@52e51bc`, the ordered PR architecture and freezes remain binding, and nested archive/retirement rules are explicit. C1 closure authorizes no deletion or merge. Watcher remains disarmed; no provider, service, policy, wallet, chain, or live-funds operation occurred.

### 2026-08-08 ~07:1xZ — Further Track C read-only audit complete; six nested security branches promoted to PATCH-SOURCE

- The rewritten-history C3 audit is persisted at `docs/status/A666-NESTED-C3-SEMANTIC-AUDIT-20260808.json` (SHA-256 `6144f543081e2c2d434ad031341fb086a68d3f8c034231dd674432420d2c945b`). It compares old-branch net changes with canonical `main@52e51bc` using exact blobs, same-path added-line coverage, introduced identifiers, a fixed security-control allowlist, and sanitized location-only results. It never emits source/diff/config lines, environments, command lines, matched values, or Git stderr.
- Result: all nine nested Git branches remain `manual-c3-review-required`; none has an exact final blob in rewritten canonical. Only 6/33 fixed control identifiers/paths are present by exact name/path. Alternative canonical controls exist under new names, so absence is a review queue rather than proof of regression.
- C1 manager routing is refined: security PR branches 1-6 are now **PATCH-SOURCE** inputs for ordered C2/C3 semantic review and possible focused ports; docs PR branches 9/10a/10b remain **ARCHIVE/manual-disposition** inputs. No nested Git worktree is retirement-authorized.
- Read-only review order is now explicit: Cobalt signature/committee binding -> keyed storage integrity -> RPC/transport authentication -> DoS bounds/parser safety -> production debug gates -> Orchard VK/parser safety -> documentation ledgers/coverage. Any port is a code-changing C2 action requiring a focused branch, tests, reviewer, and PR; none was created under this read-only authorization.
- Score-artifact content hashes are persisted at `docs/status/A666-SCORE-ARTIFACT-HASH-MANIFEST-20260808.json` (SHA-256 `0dac79ee5da1acfd9fde976f98c53b2460fdda89634cbfa767853860ae0bc65d`), covering all 161 files / 7,581,722 content bytes without emitting contents. It is archive metadata, not the required archive package.
- Read-only retirement-reference audit found zero references to any nested child in scanned user/system systemd, Caddy, cron config, or live process cwd/executable/open-fd state. This clears one C4 prerequisite only. No branch, checkout, PR, archive package, deletion, service, provider, process, policy, wallet, chain, or funds mutation occurred; watcher remains disarmed.

### 2026-08-08 ~07:2xZ — Deep body/call-graph ruling narrows nested PATCH-SOURCE set from six to three

- A fixed-target body/call-graph pass supersedes the conservative all-manual routing above. Final audit artifact SHA-256 is `dbfaed977b16e1efe091724ec3c9fd88a8229416ce6e69971c564c0d5e64ba21`. It records function calls, constants, type identifiers, body hashes, and risk-keyword counts only; no source bodies or literal values. Its persisted manager-ruling fields include the final CI/docs dispositions recorded below.
- **Port required — PR1 Cobalt:** production `validate_rbc_echo`/`validate_rbc_propose` call schema, id, and linked-message checks but no cryptographic verifier. Cobalt cryptographic-verifier references are confined to examples/tests, and committee binding is absent from the production validation summaries.
- **Port required — PR5 storage:** canonical snapshot/WAL integrity is unkeyed `Sha3_384` checksum. Archived `IntegrityKey`, HMAC, authenticated JSONL envelope, keyed open, and their tamper/migration regressions are absent.
- **Focused review/likely port — PR4 DoS/parser:** canonical has pre-parse size gates and stream timeouts, but the archived global/per-peer dispatch bounds, bounded accept worker, and `try_committee` mechanisms are absent.
- **Semantic objective present, canonical CI required — PR2/PR3/PR6:** canonical respectively has chain-bound debug gating plus mainnet/debug regressions; registry-backed ML-DSA transport-envelope verification plus nonce/route/signer binding and remote unsigned-mutation exclusion; and live-current/archive-replay Orchard VK policy plus fuzz/panic-free regressions. These sources move from PATCH-SOURCE to archive-after-CI candidates.
- Final C2/C3 order: focused PR1 port -> focused PR5 port -> PR4 subtractive review/port -> canonical CI for PR2/PR3/PR6 mappings -> docs/ledger dispositions. No code-changing action, build, branch, PR, merge, archive package, or deletion was authorized or performed. Watcher remains disarmed; provider surface untouched.

### 2026-08-08 ~07:3xZ — Exact-canonical CI and docs equivalence complete the independent Track C read-only pass

- Exact-canonical validation ran from an isolated local clone at PFTL `main@52e51bc`; the source checkout and all nested worktrees remained untouched. Evidence is `docs/status/A666-NESTED-CANONICAL-CI-20260808.json` (SHA-256 `162d8c1334b0b7b47fb928afb53aae58dfb6a9654a796b6cfe0caf1aaeed54ce`). All six commands exited zero and parsed successfully. Five target filters matched and passed six tests total: PR2 proof gate (2), PR3 transport binding (1), PR3 unsigned-remote-method exclusion (1), PR6 VK boundary (1), and PR6 panic-free indexing (1). The PR2 privacy filter selected zero tests, but exact structural review proves its gate is a one-line delegation to the tested proof gate. Captured logs remained local, were scanned without printing content, and yielded zero secret-adjacent findings.
- PR2, PR3, and PR6 are therefore **SEMANTIC CLOSE -> ARCHIVE/C4 candidates**. No old patch port is needed for those objectives. The remaining security patch sources are PR1 Cobalt signature/committee binding, PR5 keyed storage integrity, and subtractive PR4 DoS/parser hardening.
- Documentation-equivalence evidence is `docs/status/A666-NESTED-DOCS-EQUIVALENCE-20260808.json` (SHA-256 `3657d2dc51f46287a107041cd620c99bfe9a7f547a13d7b6de491126a48412de`). All 198 PR9 finding ids persist and canonical intentionally reclassifies 170 statuses, so canonical supersedes the archived ledger. PR10a's objective remains open at 585 missing doc/script pairs across 102 active docs and 361 unique absent script targets. PR10b's objective remains open at 62 undocumented methods among 116 current RPC dispatch wire methods. PR9 is an archive/C4 candidate; PR10a and PR10b are patch sources for fresh current-tree remediation, not blind ports.
- Final nested C2/C3 queue contains five inputs only: focused PR1 port -> focused PR5 port -> PR4 subtractive review/port -> fresh PR10a dead-script remediation -> fresh PR10b RPC coverage. PR2/PR3/PR6/PR9 are semantic-resolved archive/C4 candidates. Score artifacts have a complete 161-entry content-hash manifest but still need an archive package. No code branch, PR, merge, archive package, deletion, service, Vast-provider, watcher, policy, wallet, chain, or funds mutation occurred; watcher remains disarmed and the Vast provider surface was not touched.
- Final live safety snapshot at 2026-08-08 07:33:56 UTC: PublicNode and public dRPC agree signer balance 0 wei, verifier height 691, and exact prior commitment `0x1afce4dc...36c544`; agent is still unlocked with 24 whitelist entries and the signer absent. Watcher process count is zero, lock is free, and no live STOP/DONE/intent/report/fire-command artifact exists. Wallet-agent service remains active at PID 1975132 with PYTHONPATH checkout `/home/postfiat/repos/StakeHub-master-e6@2839f4e`. Deadline headroom is 157,489 seconds. Track A remains BLOCKED, NOT FAILED; Track B remains STOP-held. All further Track C work requires code-changing C2/C3 ports/remediation or archive/removal mutations.

### 2026-08-08 ~07:5xZ — Section 2 correction activates authorized additive C2/C3 work

- Principal corrected the prior exhaustion ruling: Section 2 already authorizes additive Track C branches, PRs, and archives. No additional approval is required for the five nested integration inputs or four semantic-close archives. Deletion and worktree retirement remain prohibited until the per-worktree Section 6.5 checklist is fully green; D2 and any pushed-branch history rewrite remain the only fresh-GO points.
- Public canonical was reverified directly as `postfiatorg/postfiatl1v2 main@52e51bc`. The local `postfiatl1v2` checkout is dirty on unrelated private-archive work and will not be used or altered. Integrator-C is assigned a new single-writer current-main worktree; five PRs will proceed in the recorded order, smallest coherent blast radius first, with pushed bases before `gh pr create` and no `git add .`.
- Before any archive write, the branch-native `scripts/public-secret-scan` ran against the tracked trees of PR2, PR3, PR6, and PR9. All four returned PASS with zero findings and emit only rule/location metadata on failure. Each source worktree is clean and its HEAD equals its pushed upstream. Additive deterministic tarballs, manifests, and hashes are now authorized to proceed.
- The `83ac75d` divergence belongs to the later campaign-suffix integration currently frozen behind B4. Its eventual PR description must explicitly preserve deployed-fleet `2246d257` supply semantics until the pNOK release ships. No frozen worktree will be touched to manufacture that PR early. Watcher remains disarmed; PR 7, the service, Vast provider surface, `target-jammy`, deletion, cleanup, and retirement remain untouched.

### 2026-08-08 ~08:0xZ — Semantic-close archive dispositions executed additively

- PR2 `f722a31`, PR3 `1a9f426`, PR6 `a00e7da`, and PR9 `a417c65` were each pushed/live-verified on restricted private origin before archival. Every source worktree remained clean and unchanged.
- Branch-native `scripts/public-secret-scan` ran in tracked-tree mode before any archive write. All four scans passed with zero findings. The scanner emits only rule/location/class metadata on failure and never candidate values.
- Deterministic restricted tarballs plus full Git-blob manifests are persisted under `/home/postfiat/repos/_archives/a666-nested-semantic-close-20260808/`. Index SHA-256: `2bd0a492d25a1cd334be70ebad91c5e0527b68451a56174ef2991897dc7f8d86`; combined tarball bytes: 36,842,751. All gzip integrity and tar listing checks pass. Tracker contains each archive and manifest hash.
- These four sources are now archive-complete C4 candidates. No retirement checklist was asserted green and no worktree remove, file deletion, cleanup, or history rewrite occurred.

### 2026-08-08 08:12Z — C2 PR1 opened on public current main; CI/review gate active

- Integrator-C built one coherent six-file port from PR1 keeper commits `b9cbb3e8`, `374c0354`, `fffe8b3d`, and `83c07f27` on public `main@52e51bc`. The obsolete source checklist edit was omitted because that transient document is absent from current main. Final commit: `6f10abcf56cab8da603d80e1bea25d96680c3b40`.
- The branch `integrate/a666-c2-pr1-cobalt-20260808` was pushed first and remote base/head hashes were verified before `gh pr create`. Public PR: https://github.com/postfiatorg/postfiatl1v2/pull/30.
- Review evidence: focused Cobalt suite 70 passed / 0 failed; production library compile PASS; package-scoped format PASS; all-target Clippy with warnings denied PASS; `git diff --check` PASS; six-file location/class-only secret scan 0 findings. The repository-wide tracked-tree scan reports four inherited `secret-field`-class locations in one unchanged wallet E2E file; no matched value or matching line was emitted and the PR description discloses the baseline debt.
- The port adds registered ML-DSA-65 committee binding, canonical signed RBC/ABBA/DABC validation, and signed checkpoint/activation/replay production entry points; schema-only validators are available only to tests or the explicit unsafe-simulation feature. It remains OPEN behind GitHub CI plus final read-only review; no merge has occurred and PR5 has not started.
- PR30 also records the frozen boundary: `83ac75d` is not part of this Cobalt change; its later dedicated PR must preserve deployed-fleet `2246d257` supply semantics until the pNOK release ships. Watcher remains disarmed; PR 7, the service, Vast provider surface, A6/B4-frozen worktrees, `target-jammy`, deletion, cleanup, and retirement remain untouched.

### 2026-08-08 08:32Z — Section 6.4 public-main CI prerequisite PR opened

- GitHub proved that PR30 reproduced the five jobs already red on public `main@52e51bc`: root/open-proof rustfmt, exact tracked-tree hygiene, wallet audit, recovery manifest without installed wallet dependencies, and workspace Clippy after format. Section 6.4 forbids a red-CI merge, so no waiver or PR1 scope contamination was used.
- A separate single-writer current-main worktree produced coherent prerequisite commit `657394097e9af86b927a52afd2422abdb580c33b`. Branch `integrate/a666-c2-ci-baseline-20260808` was pushed and base/head verified before opening public PR31: https://github.com/postfiatorg/postfiatl1v2/pull/31.
- The nine-file baseline repair has no protocol or production-runtime behavior change: it installs the locked wallet dependencies before the exact recovery gate; updates only affected transitive lock entries; applies one pending rustfmt import change; test-gates two test-only helpers; records one intentional internal Clippy boundary; makes the AR-05 assertion idiomatic and equivalent; exactly classifies the offline R4 test fixture; and makes that fixture's run directory an explicit environment input rather than a maintainer path.
- Local gates are green: complete public-tree hygiene; exact tracked-tree location/class-only secret scan; portability; npm audit zero; wallet 242/242 plus build; full recovery manifest with 26 standalone entries and AR-01 through AR-11; workspace check and warnings-denied Clippy; open proof kit 96 passed with three intentional environment-dependent ignores; targeted AR-05; format/diff checks.
- PR31's GitHub matrix is running. After it is green and merged, PR30 will merge updated public main into its head without rewriting history and rerun the full matrix. PR5 remains queued. Watcher remains disarmed and PR7, service, provider, frozen worktrees, target-jammy, deletion, cleanup, retirement, and history rewrite remain untouched.

### 2026-08-08 08:39Z — PR31 provider-neutral false-positive scope repaired additively

- The first PR31 GitHub run reached the wallet-and-proxy job's final provider-neutral boundary after its wallet tests/build passed. Safe extraction emitted locations only and showed the scanner was treating operational campaign scripts and E2E fixtures as shipped runtime. No candidate value or matching source line was emitted.
- Additive commit `7b37bd5fb70904d9b4722a881aad968ecf4ccfc7` removes `scripts/` from the shipped-code path set and excludes `*.e2e.js` from both boundary scans. Coverage remains over wallet source, wallet proxy, crates, tools, and the deployment bundle. The prior pushed commit was not amended, rebased, or force-pushed.
- Local provider-neutral boundary, wallet tunnel regression, public tracked-tree secret scan, source portability, shell syntax, and diff checks all pass. The branch was fast-forward pushed, PR31's description was updated, and a fresh GitHub matrix is running.
- PR30 remains unchanged and held behind PR31. Watcher remains disarmed; PR7, the wallet-agent service, Vast provider surface, A6/B4-frozen worktrees, target-jammy, deletion, cleanup, retirement, and history rewrite remain untouched.

### 2026-08-08 08:51Z — PR31 recovery-policy inherited findings bound exactly

- The second PR31 matrix closed the provider-neutral failure, then the Rust test job reached the next inherited public-main gate: `A666 recovery CI rejection policy`. The checker itself emits locations and finding classes only. All three findings are unchanged from `main@52e51bc`: one proving-command descriptor in a hash-mismatch STOP test and two prover-leaf calls whose external command/RPC surfaces are mocked for fail-closed behavior.
- Additive commit `9bdc024986a031258cbece8d66b27ed4edde9375` binds only those reviewed lines by exact path, line number, and SHA-256 of the complete source line. Any content change or movement produces both the original direct-proving class and a stale-allowlist class. New self-tests reconstruct the exact vectors and prove drift rejection.
- Recovery-policy self-test and full reviewed-tree check pass; Python compile, public secret self-test and exact-tree scan, source portability, and diff checks pass. The branch was fast-forward pushed and PR31's body updated; no pushed history was rewritten.
- PR30 remains unchanged and held. Watcher stays disarmed; PR7, service, Vast provider surface, A6/B4-frozen worktrees, target-jammy, deletion, cleanup, retirement, and history rewrite remain untouched.

### 2026-08-08 08:59Z — PR31 recovery-policy runner portability repaired

- The third matrix still failed in the recovery-policy self-test. Location/class-safe log extraction identified the failure class and missing executable location only: this Rust runner does not provide ambient `rg`. No new code finding or secret value appeared.
- Additive commit `4e19949278bc14a714f36dcb93b18875807a3b17` replaces `rg --files` with captured `git ls-files --cached --others --exclude-standard` for real repositories and a bounded Python walk for the self-test's non-Git miniature trees. Enumeration output is captured; only scanner locations/classes can be emitted.
- The exact policy self-test and full reviewed-tree check both pass with a PATH that deliberately excludes `rg`. Python compile, exact-tree public secret scan, source portability, and diff checks pass. Branch push was fast-forward only; PR31 body and tracker were updated; a fresh matrix is active.
- PR30 remains unchanged and held. Watcher stays disarmed; PR7, service, Vast provider surface, A6/B4-frozen worktrees, target-jammy, deletion, cleanup, retirement, and history rewrite remain untouched.

### 2026-08-08 09:49Z — PR31 parallel transport-test load flake hardened

- A complete local workspace run finished the 45-minute 269-test node-library tranche without failures, then found two unchanged-main node integration failures. Output-safe diagnosis emitted test locations/classes only: one authenticated persistent peer-health connection failure and its derivative poisoned global transport-test lock.
- Both exact regressions pass in fresh serial processes, proving a parallel-load readiness flake rather than a protocol failure. Additive commit `58f26cfd0626e2202654bde30d2f9ac5ada70efc` changes only the test helper's authenticated peer-health readiness budget from 15 seconds to 60 seconds.
- Both focused tests, workspace format, diff check, and exact-tree public secret scan pass. The branch was fast-forward pushed, PR31 body updated, and a fresh GitHub matrix started.
- PR30 remains unchanged and held. Watcher stays disarmed; PR7, service, Vast provider surface, A6/B4-frozen worktrees, target-jammy, deletion, cleanup, retirement, and history rewrite remain untouched.

### 2026-08-08 11:13Z — PR31 hosted matrix green; hold and low-frequency observation preserved

- After completing an independent archive-verification unit and waiting beyond the 20–30 minute observation floor, one PR31 sample found all required hosted jobs successful. The long `test` job passed in 1h22m55s; `open-reserve-proof-kit` passed in 46m13s; build, check, EVM, hygiene, Python, supply-chain, and wallet/proxy passed; the official-mainnet-fork job is intentionally skipped.
- GitHub reports PR31 OPEN, non-draft, MERGEABLE/CLEAN at exact head `58f26cfd0626e2202654bde30d2f9ac5ada70efc`. The public `postfiatorg/postfiatl1v2` API still resolves `main` to campaign base `52e51bc290eb8d6416e78d31bab6315de5729af6`; a divergent local `origin/main` tracking ref belongs to another remote lineage and is not authoritative for this public PR.
- The green-CI hold remains preserved while the four independent integration branches build. No merge, auto-merge, branch deletion, or PR30 update occurred. Further PR31 status samples require both a completed work unit and the 20–30 minute floor.
- Archive closeout reverified PR2/PR3/PR6/PR9 source worktrees clean with HEAD equal to pushed upstream and ahead/behind `0/0`; all four gzip streams, tar listings, manifests, and `INDEX.json` are readable. No new archive bytes, deletion, cleanup, retirement, or source mutation occurred. Watcher remains disarmed; PR7, service, Vast provider surface, A6/B4-frozen worktrees, and target-jammy remain untouched.

### 2026-08-08 11:1xZ — Four remaining integration lanes actively staged on isolated branches

- PR5 storage: isolated branch `integrate/a666-c2-pr5-storage-integrity-20260808` has focused keyed-integrity/downgrade commit `4dbd80c2`; its recovery-boundary group is staged but uncommitted. The current-main atomic-swap fixture has storage-owned raw paths absent from the historical keeper. The lane remains held from push while those paths are converted to logical `NodeStore` operations or explicit trusted legacy-migration setup. The eventual PR description must present keyed-integrity equivalence as an open review question.
- PR4 DoS/parser: isolated branch `integrate/a666-c2-pr4-dos-20260808` has an eight-node-file staged subtractive port. It retains global/per-peer dispatch bounds, bounded/drained accept workers, timeout-slot release, typed errors/telemetry, and saturating time conversion. Historical bridge fixture/test edits are deliberately omitted because current production already propagates parsing and codec failures. Focused bounds tests pass 5/5; full validation/push remains active.
- PR10a docs: isolated branch `integrate/a666-c2-pr10a-dead-script-refs-20260808` found exactly 112 missing-script occurrences across 34 published-nav pages. Fence-aware historical/availability annotations cover 24 pages with 10 remaining; exact closure and docs/link/security gates precede commit and push.
- PR10b RPC docs: isolated branch `integrate/a666-c2-pr10b-rpc-docs-20260808` re-derived 153 observed methods, 116 node dispatch arms, 53 existing exact table rows, and 63 missing rows. The previous 62-gap report counted a prose-only `transfer` occurrence; exact row coverage corrects it to 63. A fresh coverage document exists; nav/method links and an exact dispatch-to-document regression remain before commit/push.
- All four lanes are single-writer and based on public `postfiatorg/postfiatl1v2 main@52e51bc`; local `origin/main` is not the public-base authority. None has merged or touched PR7, PR30, PR31, the service, watcher, Vast provider surface, frozen worktrees, target-jammy, deletion, retirement, or pushed history.

### 2026-08-08 11:2xZ — PR5 semantic implementation complete locally; validation gate remains

- PR5 now has two coherent local commits: keyed-integrity/downgrade core `4dbd80c2` and recovery-boundary integration `913e3e3f`. The second preserves current snapshot verification-basis and receipt/root behavior, exports and imports storage-owned state through logical `NodeStore`, derives destination-local keying, runs migration explicitly offline, and removes expanded atomic-swap raw access to storage-owned filenames.
- The historical fixture boundary is explicit and tested by construction: normal keyed open rejects the unkeyed fixture; the trusted offline migration opener is invoked deliberately; logical reopening must then pass. Non-storage filesystem concerns such as proposals, certificates, topology, validator keys, and controlled reports remain direct I/O.
- Storage tests pass 42/42; storage format, diff check, and the raw-path audit pass. Focused node tests are still compiling, so the branch remains unpushed and no PR exists. Remaining affected tests, check/Clippy, touched-file formatting, and location/class-only secret scan gate publication.
- Keyed-integrity equivalence is still unresolved and must be presented as an open review question in the PR description. No merge, service/provider/watcher/PR7/PR30/PR31/frozen worktree, deletion, retirement, or history action occurred.

### 2026-08-08 11:2xZ — Archive content-to-manifest binding independently closed

- A content-only verifier recomputed each of four compressed archive SHA-256 values, each uncompressed-tar SHA-256, and every archived regular file's byte count and Git-blob SHA-1 against its manifest: 1,628 files per source, 6,512 total, all PASS. Manifest total bytes and the four index policy bindings also pass.
- Tar permission modes reflect the archive environment's umask; executable-bit classes match Git for every file, while each manifest retains the authoritative original Git mode. No file content or secret candidate was emitted during verification.
- This strengthens the already-complete additive archive disposition but changes no archive/source/worktree state and still authorizes no deletion or retirement.

### 2026-08-08 11:23Z — PR31 prerequisite merged normally; downstream branches rebase-free update

- PR31 satisfied every Section 6.4 gate: exact public base `52e51bc`, exact head `58f26cfd`, clean five-commit diff, full local workspace PASS, and all required hosted checks GREEN. Manager merged it normally. Public `main` is merge commit `6f971e23a4ebc11edf1ae23371d14e36cbaa2f54` with those two parents; the integration branch remains on the remote. PR30's branch was not touched.
- PR32 (PR10b) opened at initial head `0f195934838f0ab5a7f7885b115edcc95a903134`. Its four-file change closes exact current RPC documentation coverage from 53/116 to 116/116, with the corrected pre-state gap 63 rather than the former prose-derived 62. Inventory, strict docs, links, diff, and changed-path secret scan pass locally. The initial hosted `check` failed before the prerequisite merge; annotation-only classification and local evidence place it in the unchanged PR31-owned baseline, outside PR10b. No blind retry occurred. Independent review is active.
- PR10a is complete and pushed at `127a1927e179a41015003b89f7b33a38eeffff03`: 112/112 absent-script occurrences classified across 34 published pages / 66 unique absent paths; 101 current references across 15 command paths are executable; docs/link/diff/changed-doc scan pass. PR creation waits only on ordinary-merging the new public main and rerunning those gates.
- PR4's eight-file staged delta passes bounds 5/5, transport 24/24, RPC 20/20, payload 12/12, package check, diff, and added-line scan. Warnings-denied Clippy stopped solely at unchanged PR31-owned `dead-code` class outside the delta. Its original owner exhausted the agent-thread allotment; Manager owns the idle worktree and will commit, ordinary-merge public main, revalidate, then publish.
- PR5 now has three local commits through `064bece9`; storage 42/42, historical certificate 3/3, snapshot 4/4, atomic controlled-report 1/1, check, storage Clippy, touched formatting/diff, and safe scan pass. One focused padding test is still in flight; the branch remains unpushed until that exact result lands, then will ordinary-merge public main and revalidate.
- No history rewrite, merge of PR30/PR32, deletion, retirement, PR7, service, Vast-provider, watcher, frozen-worktree, or target-jammy action occurred.

### 2026-08-08 11:37Z — Authoritative current-state reconciliation after operator status questions

- The plan/tracker lagged facts that arrived after docs commit `5319c92`; this entry and the tracker now supersede those stale PR rows. Live state was re-read before writing.
- Ethereum custody clarified with exact live evidence: owner wallet `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` holds 289,735,632,642,339,914 wei = 0.289735632642339914 ETH on both PublicNode RPC and Blockscout; constrained signer `0xe01eaf76f155b2759402b39fe126b5a81655f424` holds 0. Owner nonce is 304, identical to the recorded pre-execution snapshot. Therefore no owner-wallet Ethereum transaction fired and no owner ETH was spent; the block is policy routing to a distinct gas-empty constrained signer, not depletion. Watcher stays disarmed.
- PR32 is OPEN at `9121a285e88ff533d664f10aaa28debb6addf8a7`. Independent review confirmed current semantics/counts and found one P3 future-evasion weakness in the coverage test; additive hardening now enforces the two intended table shapes, strict/non-empty semantic and posture cells, duplicate rejection, inventory-backed posture labels, and exactly one row per dispatch method, with seven fixtures. Local gates pass. At 11:37 UTC every completed hosted job passes; Rust `test` and `open-reserve-proof-kit` remain in progress.
- PR33 is OPEN at `73792781897a0ce350ee5eec50e9b1520039eff8`: 112/112 absent-script references classified across 34 published pages / 66 unique paths, with all 101 current references targeting 15 tracked executable paths. Local gates pass. At 11:37 UTC every completed hosted job passes; Rust `test` and `open-reserve-proof-kit` remain in progress.
- PR4 is committed as `eda4ef50` and ordinary-merged with public PR31 baseline at local head `9704e1fd`; clean and unpushed. Pre-merge suites remain valid. Post-merge check, warnings-denied Clippy, fmt, and diff pass, but the manager's attempted filtered tests selected zero tests and are explicitly not counted. Correct exact post-merge test invocation and final location/class scan remain before push/PR.
- PR5 is clean/unpushed at ordinary two-parent merge `b4bf310aca27e87f866b6c1dfbd243962958bff2`. Agent-local session `31705` was recovered without rerun and exited 0: post-merge storage 42/42, historical 3/3, snapshots 4/4, atomic controlled-report 1/1, certified-chain padding 1/1 in 179.19s, package check, and storage warnings-denied Clippy pass. Post-merge fmt/diff/location-class scan remain before push/PR; keyed-integrity equivalence stays an open review question.
- PR31 remains merged at public `6f971e23`; PR30 remains untouched. PR7, service, Vast provider, watcher, frozen worktrees, target-jammy, deletion, retirement, and history rewrite remain untouched.
