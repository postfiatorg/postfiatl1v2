# Overnight Whip Plan

Status: active overnight execution plan  
Date: 2026-05-17  
Scope: controlled-testnet launch hardening, live evidence, wallet/RPC operator tooling

This is the whip reference for the next overnight run. Treat this file as the
current priority order. Do not infer priorities from older historical evidence
sections except when checking context or report paths.

Latest execution status on 2026-05-17:

- A later write-gated attempt on revision `f7920d1` advanced live wallet
  finality to height `136` and Orchard direct deposit to height `137`, but the
  aggregate wrapper hit its outer `1500s` timeout before writing the aggregate
  report. The timeout/blocker record is
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T191624Z/testnet-live-evidence-refresh-timeout-blocker.json`.
  Focused follow-up validator doctor and remote observability both passed at
  height `137`, and the local wallet receipt fallback also passed. Do not rerun
  write gates merely to replace the missing aggregate; fix or tune the wrapper
  as a tooling slice.
- Wrapper mitigation was implemented: `scripts/testnet-live-evidence-refresh`
  now writes a partial aggregate after each completed step and flushes per-step
  progress. No-write/no-SSH wrapper smoke passed at
  `reports/testnet-live-evidence-refresh/evidence-wrapper-partial-smoke-20260517T195956Z/testnet-live-evidence-refresh.json`.
- P0 full live write-gated refresh passed on current head and is linked from
  `docs/ai-handoff.md` and
  `docs/status/overnight-launch-hardening-burndown.md`.
- P1 live read-only receipt pull and RPC method inventory passed and are
  linked from the same docs.
- P2 restart drill passed, post-restart remote observability passed, and the
  follow-on full validator doctor passed after the restart.
- The current-head local wallet receipt packet passed after the live/restart
  evidence.
- A follow-on read-only continuation refresh passed on revision `3b3bf11`
  without repeating write gates:
  `reports/testnet-live-evidence-refresh/read-only-continuation-20260517T185757Z/testnet-live-evidence-refresh.json`.
- Check `origin/main` for the latest pushed head; this status block may be
  updated by doc-only follow-up commits.

Canonical context:

- `docs/ai-handoff.md`
- `docs/status/overnight-launch-hardening-burndown.md`
- `docs/status/controlled-testnet-burndown.md`
- `docs/status/privacy-production-burndown.md`

Current known good head before this plan:

- `81ced81 Add account history CSV export`
- Fresh read-only live evidence:
  `reports/testnet-live-evidence-refresh/bandwidth-readonly-20260517T025250Z/testnet-live-evidence-refresh.json`
- Latest live account-history CSV evidence:
  `reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-live-account-tx-csv-20260517T030514Z.json`

## Operating Rules

- Make technical calls without asking preference questions.
- Keep the repo clean between coherent slices.
- Commit and push every coherent code/docs/evidence slice.
- Do not do broad cleanup.
- Do not redesign consensus overnight.
- Do not expose a persistent public write edge.
- Do not repeatedly spam live writes. One full write-gate refresh is useful;
  repeated live write loops are not.
- If live-machine credentials, SSH, or service access blocks a task, record the
  blocker with logs and switch to local tooling/evidence work.
- Every completed item must leave either a JSON report under `reports/` or a
  committed code/doc change.

## P0: Full Live Evidence With Write Gates

Run a full current-head live evidence refresh with write gates included:

```bash
scripts/testnet-live-evidence-refresh --include-write-gates
```

Expected coverage:

- live wallet finality advances the chain;
- live Orchard direct deposit advances the chain;
- account-history index refresh passes;
- RPC doctor passes across five endpoints;
- Python RPC smoke passes;
- monitor snapshot with embedded account-history passes;
- standalone account-history pull passes;
- validator doctor passes over SSH;
- remote observability passes;
- `git.dirty=false` in the aggregate report.

On success:

- Update `docs/ai-handoff.md`.
- Update `docs/status/overnight-launch-hardening-burndown.md`.
- Commit and push.

On failure:

- Do not retry blindly.
- Inspect the failed step logs under the run directory.
- If the failure is credentials, SSH, or machine access, record the blocker in
  `docs/ai-handoff.md` and move to P0 fallback.
- If the failure is validator divergence, stuck finality, bad index state, or
  write-gate failure, stop broad work and diagnose that as the active P0.

## P0 Fallback: Local Wallet Receipt Packet

If live write-gate evidence is blocked by machine access or credentials, build
local operator tooling instead.

Create a wallet receipt packet script that proves the end-to-end user story:

- generate a fresh wallet;
- fund it from the local faucet;
- sign and submit a transparent transfer;
- seal/apply/finalize the transfer on a local validator harness;
- query `tx` finality;
- query sender and recipient `account_tx_history`;
- export account history CSV;
- write a single redaction-safe JSON packet under `reports/`.

Suggested script name:

```text
scripts/testnet-wallet-receipt-packet-smoke
```

Required report properties:

- no private key material in the final report;
- generated wallet key/backup material kept under `/tmp` and removed;
- `tx.confirmed=true`;
- sender and recipient history include the expected transaction;
- account-history reads are indexed;
- `total_archive_lookup_count=0`;
- `total_scanned_block_count=0`;
- CSV export exists and row count matches the JSON history rows.

Checks to run:

```bash
bash -n scripts/testnet-wallet-receipt-packet-smoke
scripts/testnet-wallet-receipt-packet-smoke
```

If the script changes shared Python tooling, also run:

```bash
python3 -m py_compile scripts/postfiat-rpc-account-tx
```

On success:

- Update `docs/runbooks/python-rpc-client.md` if the operator command changes.
- Update `docs/ai-handoff.md`.
- Update `docs/status/overnight-launch-hardening-burndown.md`.
- Commit and push.

## P1: Live Read-Only Receipt Pull

If P0 evidence and the local receipt packet are green, add or run a live
read-only receipt pull for an existing public canary address / transaction.

Goal:

- Given an address and optional tx id, pull `tx`, `account_tx_history`, and CSV
  from live read-only RPC endpoints.
- Require endpoint convergence for multi-endpoint pulls.
- Do not submit a live write in this step.

Acceptable artifact:

- a JSON report under `reports/`;
- a CSV export under `reports/postfiat-rpc-account-tx/`;
- docs updated with exact commands.

## P1: RPC Method Inventory Refresh

After any RPC/operator-tooling change, refresh inventory if the public method
surface could have changed:

```bash
scripts/testnet-rpc-method-inventory
```

Goal:

- prove no accidental public write exposure;
- keep read-only/write-gated/operator-local method classifications current.

Commit and push if reports/docs change.

## P2: Controlled Restart/Outage Evidence

Only run this if P0 and P1 are green and there is enough time.

Goal:

- restart one validator/RPC service or run the existing bounded restart/outage
  drill;
- prove convergence returns;
- prove height lag is zero;
- prove validator doctor remains green.

Do not perform destructive machine work. If service access is uncertain, skip
this and record the reason.

## Morning Success Criteria

The overnight run is successful if at least one of these is true:

- full live write-gate evidence on current head passed and is linked from
  `docs/ai-handoff.md`;
- wallet receipt packet tooling exists, passed locally, and is committed;
- a live read-only receipt/account-history export passed and is linked;
- a real P0 blocker is recorded with concrete logs and no silent waiting.

Ideal morning state:

- current `main` has fresh full live write-gate evidence;
- wallet receipt packet tool exists;
- local smoke passed;
- docs point to the exact evidence;
- every coherent slice is committed and pushed.

## Do Not Do Overnight

- No broad refactors.
- No large privacy/proof rewrites.
- No Cobalt/governance redesign.
- No storage architecture rewrite.
- No persistent public write-edge exposure.
- No repeated live write spam.
- No historical markdown cleanup unless it directly prevents the whip from
  using this plan.
