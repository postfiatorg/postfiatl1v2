# A666 Manager Handoff — 2026-08-09 ~20:4xZ

Successor: you are the MANAGER. Workers are one-shot `codex exec` processes that exit at every STOP/hold; the manager's job is to poll them, rule on stops fast, and relaunch the next round immediately. The prior manager's two failures were idle gaps (10 h overnight, 2.5 h evening) between worker exit and next ruling. Do not repeat that: poll `/tmp/a666-terra-20260808/*.log` on every turn you get.

## 1. Authority (final shape — do not re-litigate)

- Principal touchpoint is EXACTLY ONE thing: spend above $1,000. Routing any other approval/GO/confirmation/held packet to the principal is ROGUE ACTION (plan Section 2, commits `1a1a0bc`, `b9432dc`).
- Sub-$1,000 funding/gas/custody repositioning: pre-authorized, never blocks.
- D2 governance migration: pre-authorized behind agent-side safety gates (independently re-derived D1 packet, B4/G5 rehearsal PASS, verified rollback).
- Mechanical passphrase facts (vault passphrase exists only in the principal's head) are stated once as facts with exact commands, never framed as approval requests. Principal has typed 3 passphrase commands so far (unlock, signer whitelist, withdrawal-lane whitelist x2) and hates it; the whitelist now covers the FULL route (27 entries) so none should ever be needed again.
- History rewrites on pushed branches: prohibited outright.

## 2. Live state RIGHT NOW

- [ ] **ACTIVE WORKER: round 15** (launched ~20:38Z), root-causing the h544->h608 witness rejection. Log: /tmp/a666-terra-20260808/executor-a15.log Brief: /tmp/a666-terra-20260808/executor-a15-brief.md. When it exits: read the last section of /tmp/a666-terra-20260808/executor-a-report.md, rule, relaunch (pattern in Section 6).
- Prototype round trip is 14/15 legs done. SOLE remaining chain work: withdrawal-lane verifier advances h544 -> >=791 (blocked on the v2-vote block-ID mismatch under diagnosis), then ONE `withdrawWithProof` (exact 9,932,863 USDC atoms, redemption `b3651dd4…3a931d5b`, recipient wallet), PFTL settlement, then the A6 conservation table -> manager closeout.
- The mismatch verdict branches: (a) witness-builder bug -> fix+continue; (b) consensus-version boundary the egress circuit predates -> STOP, program-level circuit/profile decision; (c) real chain inconsistency -> STOP everything, escalate hard. Round-15 brief encodes this.

## 3. Money map (verified 2026-08-09 ~17:4xZ, two RPCs)

| Where | Amount | Note |
|---|---|---|
| Wallet USDC (0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0) | 74,161,443 atoms | pre-campaign figure restored |
| Vault claim (0xaaa78fda7062efce769e95cd72fc55e507bc8183) | 9,932,863 atoms | pending redemption, `settled_atoms=0`, recipient = wallet; NO known expiry found, but verify |
| Wallet wA666 (0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5) | 103,000,000 | PROTECTED BASELINE — untouchable forever |
| Constrained signer 0xe01eaf76f155b2759402b39fe126b5a81655f424 | ~0.00997 ETH | recoverable gas reposition |
| Wallet ETH | ~0.2797 ETH | |
| Round-trip friction when payout settles | 67,137 atoms (~6.7 cents) | deposit 10,000,000; sale 8,047,944; buy-back -8,047,944; payout 9,932,863 |
| GPU spend total | ~$3.8 of $150 | vast credit ~$10.03 remains |

## 4. Custody map

- agentd (StakeHub wallet agent): systemd user unit `stakehub-pfusdc-wallet-agent.service`, PID from 2026-08-08 12:48:39Z, checkout /home/postfiat/repos/StakeHub-master-e6 at merge `6263c89` (PR 7: session-less `evm_contract_tx` to whitelisted targets). **UNLOCKED and policy-restored; unlock is PERISHABLE — any restart locks it and only the principal can re-unlock. Do not restart this service.** Whitelist: 27 entries incl. mint verifier 0xb79FF9…, controller 0x9A0262…, wA666, Permit2, universal router, USDC, signer 0xe01eaf…, withdrawal verifier 0x9a45d6f1dc9da443a88b1c336b3188fa7924d1ae, egress vault 0xaaa78fda7062efce769e95cd72fc55e507bc8183.
- Working CLI for the live daemon (PATH `stakehub` wrapper is a WRONG-LINEAGE CLI, do not use for policy writes): `PYTHONPATH=/home/postfiat/repos/StakeHub-master-e6 /home/postfiat/repos/StakeHub/.venv/bin/python3 -m stakehub.cli …`
- Constrained signer daemon: socket /run/user/1000/postfiat-constrained-signer/a666-signer.sock; signs only its pinned selectors (mint-lane advance/accept/consume + burnForPftlReturn); DENIES withdrawal-lane calls. Never modify either custody policy.

## 5. Known environment/tooling defects (all FIXED and committed — reuse, do not rediscover)

1. Public RPC Cloudflare 403s Python-urllib default UA -> custom `user-agent` header everywhere (a666 `2faf581`, watcher lineage).
2. master-e6 agentd chain table keyed `mainnet` not `ethereum` (StakeHub-master-e6/stakehub/evm.py:19).
3. agentd returns tx hashes WITHOUT `0x` -> normalize before receipt polls (`50db9d9`).
4. `eth_estimateGas` must bind `from` or Permit2 checks the zero address -> AllowanceExpired(0) (`f583cf8`).
5. Binding renderer odd-length-hex calldata (`9d14fdc`).
6. dRPC free tier 429s under burst -> prefer https://ethereum-rpc.publicnode.com with UA header.
7. Audited leaf for contract calls: scripts/native_evm_contract_leaf.py (needs env EVM_RPC_URL).
8. vast.ai GPU: guest ELF needs glibc<=2.35 host CLI (build in ubuntu:22.04 container); SP1 groth16 circuit cache can blow the 20 GB disk floor — use 80 GiB instance disk.

## 6. Worker launch pattern (exact)

```
CODEX_HOME=/home/postfiat/.codex setsid nohup codex exec -m gpt-5.6-terra \
  -c model_reasoning_effort='"xhigh"' --dangerously-bypass-approvals-and-sandbox \
  -C /home/postfiat/repos/a666-eth-fast-lane-combined-20260724 - \
  < /tmp/a666-terra-20260808/executor-aNN-brief.md \
  > /tmp/a666-terra-20260808/executor-aNN.log 2>&1 & disown
```
- CODEX_HOME MUST be /home/postfiat/.codex (PFTerminal exports ~/.pfterminal whose config routes to a claude-plan provider that fails).
- Every brief: role + "read /tmp/a666-terra-20260808/executor-a-report.md first" + explicit ruling + unchanged hard rules (one attempt per mutation; STOP-no-retry with full reconciliation; never re-run prior txs; baseline untouchable; no `git add .`; secrets by location/class; disk floor 20 GB).
- DO NOT build pgrep-based auto-chain launchers; one self-matched its own command line and deadlocked 4.5 h. Poll and relaunch manually.

## 7. Program queue after the prototype closes (principal-ordered)

Five-demo program — /home/postfiat/repos/pftl-validation-20260807/docs/plans/A666-FIVE-DEMO-PROGRAM-20260809.md (commit `767be83`): Batch 1 single repro w/ timing baseline -> Batch 2 three CONCURRENT rounds (one writer process, three interleaved order lineages, per-demo receipt chains + verifier reconciliation) -> Batch 3 fix round (defect ledger below) -> Batch 4 verification run. Then/interleaved: B4 G5 rehearsal (a666 worktree free between batches), D1 packet, D2 fires agent-side, D3 legacy-lane retirement. Track B: B1/B2/B3 CLOSED (B3 6/6 at `a297f81`). Track C: COMPLETE (PRs 30/32/33/34/35 merged).

Fix-round defect ledger so far: (1) withdrawal-lane checkpoint cadence fell 311 blocks behind — needs automation; (2) campaigns must survey custody coverage of ALL legs' contracts on day zero; (3) h544->h608 v2-vote mismatch — pending round-15 verdict; (4) one-shot worker idle gaps — consider a persistent executor session; (5) S1f packets went stale vs S1g rebind — bindings must be receipt-chained to finalized values at fire time.

## 8. Key documents

- Plan of record: /home/postfiat/repos/pftl-validation-20260807/docs/plans/A666-UNIFIED-EXECUTION-PLAN-20260808.md
- Tracker: /home/postfiat/repos/pftl-validation-20260807/docs/status/A666-UNIFIED-EXECUTION-TRACKER-20260808.md (append dated entries; commit as `docs:`)
- Runbook: /home/postfiat/repos/pftl-validation-20260807/docs/runbooks/A666-END-TO-END-LIVE-FUNDS-RUNBOOK-20260807.md
- Full leg-by-leg evidence: /tmp/a666-terra-20260808/executor-a-report.md (worker-append; archive to the evidence tree at A6 close — it is in /tmp!)
- Worker briefs/logs: /tmp/a666-terra-20260808/
- Leg 3b evidence: /tmp/a666-s1g/leg3b/ ; r9-r12 artifacts: /tmp/a666-r9-*, /tmp/a666-r12-checkpoints/
- Principal-facing snippets land in /home/postfiat/repos/pastedocs/ (bare absolute paths; he cannot copy/paste in tmux)

## 9. Communication contract with the principal

Blunt, zero jargon, plain-English state answers ("where is the money, what is stuck, what fixes it"). Report worker idle time honestly. Correct his premises immediately when wrong. NEVER ask his approval for anything except spend >$1,000. State mechanical passphrase facts once, with an exact runnable command, only if custody makes them unavoidable (should be never now). When he asks "what is the state", lead with the money map and the single current blocker.
