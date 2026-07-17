# Overnight Launch Hardening Burndown

Status: tactical overnight execution list  
Date: 2026-05-16  
Scope: controlled-testnet hardening, RPC/tooling, validator UX, monitors

This is the narrow worklist for an overnight agent or whip run. It is not a
permission slip for broad refactors. The goal is to wake up with either fresh
launch evidence, committed operator tooling, or an explicit blocker with logs.

Canonical context remains:

- `docs/status/controlled-testnet-burndown.md`
- `docs/status/privacy-production-burndown.md`
- `docs/status/chain-state-current.md`
- `docs/ai-handoff.md`

Current whip execution plan:

- `docs/status/overnight-whip-plan-2026-05-17.md`

## Operating Rules

- Do not do unrelated cleanup.
- Do not change consensus semantics unless a failing launch gate proves the
  current behavior is broken.
- Do not expose a persistent public write edge unless an operator policy doc
  and explicit launch approval already exist.
- Commit and push coherent slices. Leave the repo clean between slices.
- If live-machine credentials or service access block a task, switch to local
  tooling/evidence work instead of waiting.
- Every completed item must leave either a committed code/doc change or a JSON
  report under `reports/`.

## Morning Success Criteria

By morning, at least one of these should be true:

1. Fresh launch/privacy/finality evidence exists and is linked from the
   appropriate status document.
2. RPC/validator monitor tooling exists, has a smoke report, and can be run
   repeatedly by an operator.
3. A Python RPC client v0 exists with account, block, receipt, transaction, and
   client-side account-history reads, plus a local/live smoke.

If none of those land, stop and record why in `docs/ai-handoff.md`.

## 2026-05-17 Progress

- Latest write-gated attempt on revision `f7920d1` ran
  `scripts/testnet-live-evidence-refresh --run-id current-write-gates-20260517T191624Z --include-write-gates`.
  The aggregate wrapper hit the outer `1500s` timeout before writing
  `testnet-live-evidence-refresh.json`; timeout details and closure reports are
  recorded at
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T191624Z/testnet-live-evidence-refresh-timeout-blocker.json`.
  No evidence child process was left running. The live write legs completed
  green: wallet finality advanced from height `134` to `136` with transaction
  `61401bdb6af79333545399ade449843d84aff624c9d58cb93772eda7afb50fc9c9d06422dfe25929441b89ff7960d2ca`,
  and Orchard direct deposit advanced from height `136` to `137` with
  transaction
  `c5c3e51c346d928345a7cbceb206c045adb736a2520bbf0d7d583a20a812eeb1d8d9e38327509608f53718b67a1bb484`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor
  snapshot, and standalone account-history pull also passed; account history
  converged across five endpoints at height `137`, returned `36` indexed rows,
  and recorded zero archive lookups and zero retained-history scans. Focused
  follow-up validator doctor passed:
  `reports/testnet-live-validator-doctor/post-timeout-validator-doctor-20260517T194210Z/testnet-live-validator-doctor.json`.
  Focused follow-up remote observability passed:
  `reports/testnet-remote-observability/testnet-remote-observability-post-timeout-remote-observability-20260517T195533Z.json`.
  The local wallet receipt fallback also passed:
  `reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-local-wallet-receipt-fallback-20260517T195608Z.json`.
  Current interpretation: the chain evidence is green through individual
  reports, but the aggregate wrapper timeout budget/reporting needs a tooling
  slice before relying on one monolithic write-gated command for unattended
  execution.
- Wrapper mitigation landed after the timeout: `scripts/testnet-live-evidence-refresh`
  now writes a partial aggregate report after each completed step and flushes
  per-step progress lines. `python3 -m py_compile scripts/testnet-live-evidence-refresh`
  passed, and no-write/no-SSH wrapper smoke passed:
  `reports/testnet-live-evidence-refresh/evidence-wrapper-partial-smoke-20260517T195956Z/testnet-live-evidence-refresh.json`.
- Read-only continuation evidence passed on revision `3b3bf11` without
  repeating live write gates, preserving the active plan's no-repeated-write
  rule:
  `reports/testnet-live-evidence-refresh/read-only-continuation-20260517T185757Z/testnet-live-evidence-refresh.json`.
  The aggregate had `git.dirty=false`, SSH checks included,
  `write_gates_included=false`, `sensitive_material_redacted=true`, and all
  steps green. Account-history index refresh, RPC doctor, Python RPC smoke,
  monitor snapshot with embedded account-history, standalone account-history
  pull, validator doctor, and remote observability all passed across five
  validators/endpoints at height `134` with zero height lag. Account history
  returned `35` indexed rows across six windows with zero archive lookups and
  zero retained-history scans. Validator doctor reconfirmed services active,
  state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, private file permissions safe, matching binary hash,
  registry root, state root, and fleet convergence at height `134`. Remote
  observability passed with five validators, services/RPC reads green, height
  lag `0`, and no reported transport rejection.
- Full current-head live write-gate evidence passed on revision `46f941a`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T180239Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `131` to `133` with transaction
  `811b0be0ebf04f629bc4fdd54b5dc29b9f7c7970f1b9f93805ba2aeae9535acb764b26ab0674cfe60a8e631d5717037e`;
  funding landed at height `132`, the spend landed at height `133`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `133` to `134` with transaction
  `7ce5c93b8aabff24e79621909d1ca6e8a69540c611ccf9a370e93833bb84dde74645f377dab2b62380a88e71cc63b92a`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in a
  94-output retained pool, local/remote private material removal checks passed,
  and public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T180239Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `134` with zero height lag. The monitor and account-history pull
  returned `35` indexed rows across six windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, registry
  root, state root, and fleet convergence at height `134`. Remote
  observability passed with height lag `0` and zero transport rejections.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `811b0be0ebf04f629bc4fdd54b5dc29b9f7c7970f1b9f93805ba2aeae9535acb764b26ab0674cfe60a8e631d5717037e`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-20260517T182947Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `133`; the
  account-history pull converged across all five endpoints at height `134`,
  returned `35` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-20260517T182947Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `68b8d49`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T182947Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T183116Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T183116Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `134` with one state root.
  Post-restart remote observability also passed:
  `reports/testnet-remote-observability/testnet-remote-observability-20260517T183759Z.json`.
  It confirmed services and RPC reads green across all five validators at
  height `134`, max height lag `0`, and zero transport rejections. Follow-on
  validator doctor passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T183828Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all commands/state checks green, local
  keys valid, private file permissions safe, account-history aggregate/disk
  indexes usable/current, matching binary hash, registry root, state root, and
  fleet convergence at height `134`.
- Full current-head live write-gate evidence passed on revision `af8655d`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T171428Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `128` to `130` with transaction
  `3e93996a7f17c3b9831a81b7d1286d4aae632f73336341e72be23e0346bb4a394f243506e8e3bc6679757b88f4eec32a`;
  funding landed at height `129`, the spend landed at height `130`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `130` to `131` with transaction
  `55bc9cf469a6ee4a07662a4e47d7759fc81d5bcec27683bb4d520ed6b52235e9112ed3fe3fa39f75b4f775e97aa62a56`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in a
  92-output retained pool, local/remote private material removal checks passed,
  and public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T171428Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `131` with zero height lag. The monitor and account-history pull
  returned `34` indexed rows across six windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, registry
  root, state root, and fleet convergence at height `131`. Remote
  observability passed with height lag `0` and zero transport rejections.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `3e93996a7f17c3b9831a81b7d1286d4aae632f73336341e72be23e0346bb4a394f243506e8e3bc6679757b88f4eec32a`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T174150Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `130`; the
  account-history pull converged across all five endpoints at height `131`,
  returned `34` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T174150Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `c2831e7`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T174150Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T174315Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T174315Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `131` with one state root.
  Post-restart remote observability also passed:
  `reports/testnet-remote-observability/testnet-remote-observability-20260517T175551Z.json`.
  It confirmed services and RPC reads green across all five validators at
  height `131`, max height lag `0`, and zero transport rejections. Follow-on
  full validator doctor was interrupted after a remote validator check
  exceeded the overnight wait budget; blocker report:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-blocked-20260517T175544Z/testnet-live-validator-doctor-blocker.json`.
- Current-head local wallet receipt packet passed:
  `reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-current-local-wallet-receipt-post-write-gates-20260517T175816Z.json`.
  `bash -n scripts/testnet-wallet-receipt-packet-smoke` passed first. The
  packet generated fresh sender/recipient wallets under `/tmp`, funded the
  sender, signed/submitted a transfer, confirmed tx
  `184491c46c1d93a6e70b1a2f923cbb0da56eaaac0d2ec5db3c7cb41baf8a7af2d71a50a626aa5c0ea2120e42995b0396`
  at local block height `2`, removed private wallet material, and proved
  indexed sender/recipient history plus CSV exports with zero archive lookups
  and zero retained-history scans.
- Full current-head live write-gate evidence passed on revision `d32691f`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T162432Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `125` to `127` with transaction
  `987596741cd53b2ab34c76c19ef3399d0b2e2b3e0c6669ba912167fed3532c057ae5397b10bdad432ee3c6bff4bfe17f`;
  funding landed at height `126`, the spend landed at height `127`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `127` to `128` with transaction
  `a0c2ffdb71f01ed23d0939be6dc30048cbf96c095159a7e00f33d8ecd213e743928a93374e7d10f71760dad77ff39801`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in a
  90-output retained pool, local/remote private material removal checks passed,
  and public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T162432Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `128` with zero height lag. The monitor and account-history pull
  returned `33` indexed rows across six windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, registry
  root, state root, and fleet convergence at height `128`. Remote
  observability passed with height lag `0` and zero transport rejections.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `987596741cd53b2ab34c76c19ef3399d0b2e2b3e0c6669ba912167fed3532c057ae5397b10bdad432ee3c6bff4bfe17f`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T165035Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `127`; the
  account-history pull converged across all five endpoints at height `128`,
  returned `33` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T165035Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `3da127b`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T165103Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T165255Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T165255Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `128`. Follow-on validator doctor
  passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T165923Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all commands/state checks green, local
  keys valid, private file permissions safe, account-history aggregate/disk
  indexes usable/current, matching binary hash, registry root, state root, and
  fleet convergence at height `128`.
- Full current-head live write-gate evidence passed on revision `806cc11`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T153630Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `122` to `124` with transaction
  `14cb1e75c1a52227314ca360687507ffc18e8f7ef8ee399d0d2cf6b037d56b3e523d7b79d7e89af259719edd49cf187a`;
  funding landed at height `123`, the spend landed at height `124`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `124` to `125` with transaction
  `22c289aabaa5ddcab6397bf130708ec168ebf42d493705c80f5bd536769aa98b1ea8302479de6e0455e02dd443fdecca`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in an
  88-output retained pool, local/remote private material removal checks passed,
  and public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T153630Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `125` with zero height lag. The monitor and account-history pull
  returned `32` indexed rows across six windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, registry
  root, state root, and fleet convergence at height `125`. Remote
  observability passed with height lag `0` and zero transport rejections.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `14cb1e75c1a52227314ca360687507ffc18e8f7ef8ee399d0d2cf6b037d56b3e523d7b79d7e89af259719edd49cf187a`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T160139Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `124`; the
  account-history pull converged across all five endpoints at height `125`,
  returned `32` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T160139Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `1b4b3fa`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T160204Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T160306Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T160306Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `125`. Follow-on validator doctor
  passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T160945Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all commands/state checks green, local
  keys valid, private file permissions safe, account-history aggregate/disk
  indexes usable/current, matching binary hash, registry root, state root, and
  fleet convergence at height `125`.
- Full current-head live write-gate evidence passed on revision `88cf26c`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T144933Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `119` to `121` with transaction
  `cade5cf3eb2de811dadb83c7799e3824d5665072c8d463cdbb0b7929a8a64ffac46b83c6e511c7f0692ebb81c45ce22a`;
  funding landed at height `120`, the spend landed at height `121`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `121` to `122` with transaction
  `68ef60297125705aea94a75591825144ce62fe0949f5b814274df26255342829e0a8507fe4d00d2d2f6cf8ba4c685cf3`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in an
  86-output retained pool, local/remote private material removal checks passed,
  and public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T144933Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `122` with zero height lag. The monitor and account-history pull
  returned `31` indexed rows across five windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, registry
  root, state root, and fleet convergence at height `122`. Remote
  observability passed with height lag `0` and zero transport rejections.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `cade5cf3eb2de811dadb83c7799e3824d5665072c8d463cdbb0b7929a8a64ffac46b83c6e511c7f0692ebb81c45ce22a`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T151429Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `121`; the
  account-history pull converged across all five endpoints at height `122`,
  returned `31` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T151429Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `187e30f`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T151451Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T151555Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T151555Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `122`. Follow-on validator doctor
  passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T152205Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all commands/state checks green, local
  keys valid, private file permissions safe, account-history aggregate/disk
  indexes usable/current, matching binary hash, registry root, state root, and
  fleet convergence at height `122`.
- Full current-head live write-gate evidence passed on revision `9c1fc84`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T140003Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `116` to `118` with transaction
  `20d31f97f585c5cb11c55fa677f421ebed9f9c43ccbf205af2bf2c7b13f0145c26f95e1e4ef8e64e53da9ada1e37599a`;
  funding landed at height `117`, the spend landed at height `118`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `118` to `119` with transaction
  `32f4eb37e1d82085df4d368ec34c54710eb2b09b86e5b2960c41e945ed765c5bfb6d5d007f09b9aa8f9026ab7986bb0f`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in an
  84-output retained pool, local/remote private material removal checks passed,
  and public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T140003Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `119` with zero height lag. The monitor and account-history pull
  returned `30` indexed rows across five windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, and
  fleet convergence at height `119`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `20d31f97f585c5cb11c55fa677f421ebed9f9c43ccbf205af2bf2c7b13f0145c26f95e1e4ef8e64e53da9ada1e37599a`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T142401Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `118`; the
  account-history pull converged across all five endpoints at height `119`,
  returned `30` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T142401Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `55952d8`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T142416Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T142543Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T142543Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `119`. Follow-on validator doctor
  passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T143154Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all commands/state checks green, local
  keys valid, private file permissions safe, account-history aggregate/disk
  indexes usable/current, matching binary hash, registry root, state root, and
  fleet convergence at height `119`.
- Current-head local wallet receipt packet passed:
  `reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-current-local-wallet-receipt-post-green-20260517T144601Z.json`.
  The packet generated fresh sender/recipient wallets under `/tmp`, funded the
  sender, signed/submitted a transfer, confirmed tx
  `128fddb1dcf6999b1cfcdbdccd8e16a681a99b7665e1e57ea134ff03bebab23bff53b927879f12d45a1a358f20138c66`
  at local block height `2`, removed private wallet material, and proved
  indexed sender/recipient history plus CSV exports with zero archive lookups
  and zero retained-history scans.
- Full current-head live write-gate evidence passed on revision `cfbc997`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T131347Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `113` to `115` with transaction
  `25bd3edc78eb29a45200986222920957e20d94ca6dd37c5d595ddf7a7dbbe60124a4ddfe9d8dc194c17d98edd0cddd9f`;
  funding landed at height `114`, the spend landed at height `115`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `115` to `116` with transaction
  `df7ddf7b231586235ef62c5cf4eddf570df34d116083747e17c8e09725b5de1be37d63688162988b2dd5235c91a0b711`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in an
  82-output retained pool, local/remote private material removal checks passed,
  and public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T131347Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `116` with zero height lag. The monitor and account-history pull
  returned `29` indexed rows across five windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, and
  fleet convergence at height `116`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `25bd3edc78eb29a45200986222920957e20d94ca6dd37c5d595ddf7a7dbbe60124a4ddfe9d8dc194c17d98edd0cddd9f`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T133725Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `115`; the
  account-history pull converged across all five endpoints at height `116`,
  returned `29` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T133725Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `3f12a9c`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T133742Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T133937Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T133937Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `116`. Follow-on validator doctor
  passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T134521Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all commands/state checks green, local
  keys valid, private file permissions safe, account-history aggregate/disk
  indexes usable/current, matching binary hash, registry root, state root, and
  fleet convergence at height `116`.
- Full current-head live write-gate evidence passed on revision `ebd33bd`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T122847Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `110` to `112` with transaction
  `95389b59a6d368e5093e795a6e9b0fbf793ad1b0f57e5d4f283e6abb18c6255612311073cf280c2226cdff0f143a97f9`;
  funding landed at height `111`, the spend landed at height `112`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `112` to `113` with transaction
  `f6b93e053dca1a854ae992fc234d9b97b96e137b2db6b5522e5b29d49166ff73e75ed420c23ec954c501d63d994be5c0`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in an
  80-output retained pool, local/remote private material removal checks passed,
  and public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T122847Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `113` with zero height lag. The monitor and account-history pull
  returned `28` indexed rows across five windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, and
  fleet convergence at height `113`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `95389b59a6d368e5093e795a6e9b0fbf793ad1b0f57e5d4f283e6abb18c6255612311073cf280c2226cdff0f143a97f9`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T125149Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `112`; the
  account-history pull converged across all five endpoints at height `113`,
  returned `28` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T125149Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `1b9feff`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T125205Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T125308Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T125308Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `113`. Follow-on validator doctor
  passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T125843Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all commands/state checks green, local
  keys valid, private file permissions safe, account-history aggregate/disk
  indexes usable/current, matching binary hash, registry root, state root, and
  fleet convergence at height `113`.
- Full current-head live write-gate evidence passed on revision `3d39dce`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T114443Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `107` to `109` with transaction
  `587054c4c84a18bf460fa0e3b4119e91223da90d68ebaba280d971ee8f7946d356c5dcf94908f9f58b79d61d277d76cb`;
  funding landed at height `108`, the spend landed at height `109`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `109` to `110` with transaction
  `4273260a01cd96fb02f07bc09fbbce747ccbe79ad552b667d4f7c902aec064329d154cc0504f8a5ca80ca77cb45af54b`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in a 78-output
  retained pool, local/remote private material removal checks passed, and
  public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T114443Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `110` with zero height lag. The monitor and account-history pull
  returned `27` indexed rows across five windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, and
  fleet convergence at height `110`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `587054c4c84a18bf460fa0e3b4119e91223da90d68ebaba280d971ee8f7946d356c5dcf94908f9f58b79d61d277d76cb`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T120827Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `109`; the
  account-history pull converged across all five endpoints at height `110`,
  returned `27` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T120827Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `76c7b4b`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T120841Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T120942Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T120942Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `110`. Follow-on validator doctor
  passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T121508Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all commands/state checks green, local
  keys valid, private file permissions safe, account-history aggregate/disk
  indexes usable/current, matching binary hash, registry root, state root, and
  fleet convergence at height `110`.
- Full current-head live write-gate evidence passed on revision `26f8b50`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T105833Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `104` to `106` with transaction
  `0ae86034eae14f0c8fabe2468817f7235cd55e33f6e4a11e2543d34b0081a13e4d8611beeff251ec954dba0ce841127b`;
  funding landed at height `105`, the spend landed at height `106`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `106` to `107` with transaction
  `a4e6fefeac6e0273d19389d70a36c3d23e4575bb5a5885c815f12c5c8ebae66bd7d837a9792d6d64e5772e2f855850c2`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed,
  the deposit amount `11` and decrypted note scan were verified in a 76-output
  retained pool, local/remote private material removal checks passed, and
  public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T105833Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `107` with zero height lag. The monitor and account-history pull
  returned `26` indexed rows across five windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, and
  fleet convergence at height `107`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `0ae86034eae14f0c8fabe2468817f7235cd55e33f6e4a11e2543d34b0081a13e4d8611beeff251ec954dba0ce841127b`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T112023Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `106`; the
  account-history pull converged across all five endpoints at height `107`,
  returned `26` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T112023Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `227ee6a`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T112038Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T112302Z.json`.
  All five validator/RPC service pairs restarted sequentially, all reported
  `restart_ok=true`, services active, and local `verify-state` passed; post
  restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T112302Z/post-restart-rpc-smoke.json`.
  The fleet remained converged at height `107`. Follow-on validator doctor
  passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T112821Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all commands/state checks green, local
  keys valid, private file permissions safe, account-history aggregate/disk
  indexes usable/current, matching binary hash, registry root, state root, and
  fleet convergence at height `107`.
- Latest local wallet receipt packet passed after the live
  write-gate/restart evidence:
  `reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-current-local-wallet-receipt-20260517T114000Z.json`.
  `bash -n scripts/testnet-wallet-receipt-packet-smoke` passed first. The
  smoke started a local four-validator harness, generated fresh
  sender/recipient wallets under `/tmp`, removed private wallet material,
  funded the sender, signed and submitted one transparent transfer, finalized
  the spend at local height `2`, confirmed `tx` finality for
  `f3b34c36977379cbb23f4a6770b6bdb9d0aca7875c63f2da3f7589501203be6e09c9567f6a7da3b6063333ae2dd31b23`,
  proved sender and recipient `account_tx_history` rows through indexed reads,
  exported matching CSVs, and recorded zero archive lookups and zero retained
  history scans.
- Full current-head live write-gate evidence passed on revision `c7806bb`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T101530Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `101` to `103` with transaction
  `2c0b64dd619385b28f10c6a4c4258175d05d8a4c46a808dc5d305d275d3eca97df4bc45458a08c75a0ac8b35dc70bdba`;
  funding landed at height `102`, the spend landed at height `103`, private
  wallet material was removed, and the temporary write edge was SSH-local,
  bounded to one signed-transfer request, and not publicly exposed. Orchard
  direct deposit advanced from height `103` to `104` with transaction
  `e2801afec76591be7800473355149b86f858f112574091cbd5f21eb00ca2b96ca8b5c5bb5915f2f88e4d620e2e5f5227`;
  the receipt was accepted, fee burned/charged `24`, `tx` finality confirmed
  through full block replay, the deposit amount `11` and decrypted note scan
  were verified in a 74-output retained pool, local/remote private material
  removal checks passed, and public shielded apply remained closed:
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T101530Z-orchard-direct-deposit/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `104` with zero height lag. The monitor and account-history pull
  returned `25` indexed rows across five windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed all services active,
  all state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, and
  fleet convergence at height `104`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `2c0b64dd619385b28f10c6a4c4258175d05d8a4c46a808dc5d305d275d3eca97df4bc45458a08c75a0ac8b35dc70bdba`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T103713Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `103`; the
  account-history pull converged across all five endpoints at height `104`,
  returned `25` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T103713Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `8609ccb`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T103740Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-green-write-gates-20260517T103922Z.json`.
  The drill restarted all five validator/RPC service pairs sequentially,
  confirmed `restart_ok=true`, services active, and local `verify-state` on
  every validator, and the fleet remained converged at height `104`.
  Post-restart RPC smoke also passed:
  `reports/testnet-remote-restart-drill/logs-post-green-write-gates-20260517T103922Z/post-restart-rpc-smoke.json`.
  Follow-on validator doctor passed:
  `reports/testnet-live-validator-doctor/post-green-write-gate-restart-validator-doctor-20260517T104434Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all state verified, local keys valid,
  account-history aggregate/disk indexes usable/current, matching binary hash,
  private file permissions safe, and fleet convergence at height `104`.
- Latest live write-gate attempt on revision `b2e68be` advanced the live chain
  but did not produce a green aggregate:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T093629Z/testnet-live-evidence-refresh.json`.
  The run used `git.dirty=false`, included live write gates and SSH checks, and
  kept `sensitive_material_redacted=true`. SDK wallet finality passed and
  advanced from height `98` to `100` with transaction
  `f1f7df88b63b58318cb4dc1f0486f635ac7a2190e57c989f217939697b812d47aff0637bb640e2585619c590e0e41f69`.
  Orchard direct deposit accepted, finalized, and scanned at height `101` with
  transaction
  `401c8bd60237ef823ce79d6beeb9de0a8bd60060630e0a39ac70ffcc0f02d1e3924bbfc41a7c7cb3122cadb0dd7ccd67`;
  the receipt accepted fee burned/charged `24`, `tx` finality confirmed
  through full block replay, and the wallet scan decrypted one output of value
  `11` in a 72-output retained pool. Orchard's smoke report failed before
  writing its final JSON because post-write `verify-state` collection hit an
  SSH timeout:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T093629Z/logs/orchard_direct_deposit.stderr.log`.
  Supporting finalized Orchard evidence is in
  `reports/testnet-live-orchard-direct-deposit/current-write-gates-20260517T093629Z-orchard-direct-deposit/logs/direct-deposit-h101.summary.json`.
  This is recorded as a machine-access/post-write evidence blocker, not
  validator divergence or private-key exposure. No live write-gate rerun was
  attempted to overwrite it.
- All downstream read-only/fleet checks in the same attempt passed after the
  height `101` write: account-history index refresh, RPC doctor, Python RPC
  client smoke, monitor snapshot with embedded account-history, standalone
  account-history pull, validator doctor, and remote observability. Monitor and
  account-history pull converged across five validators/endpoints at height
  `101`, returned `24` indexed rows across five windows, and recorded zero
  archive lookups and zero retained-history scans. Validator doctor confirmed
  all services active, all state verified, local keys valid, account-history
  aggregate/disk indexes usable/current, matching binary hash, private file
  permissions safe, and fleet convergence at height `101`. Because the P0
  aggregate was not green, no P2 restart drill was run after this attempt.
- Local wallet receipt packet fallback passed after the write-gate blocker:
  `reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-20260517T095434Z.json`.
  It generated sender `pf4cdce79323ca50bfe926ef75ee96f5ef0f197ea2` and
  recipient `pfb41c35f21af05547e1bb60d8088dbae259957eb0`, funded the sender,
  spent `250` to the recipient, confirmed transaction
  `048e825f895158e6b7c15fc4820a0291407d85ba1f18cc40e656ba1b3c249df33c8f7e240b30db062778fad7fa29b698`
  at local height `2`, proved sender and recipient indexed account-history
  rows, wrote CSV exports, and recorded zero archive lookups/scans.
- Follow-on live read-only receipt pull passed for the latest live wallet
  transaction:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T095456Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `100`; the
  account-history pull converged across all five endpoints at height `101`,
  returned `24` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T095456Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `b2e68be`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T095511Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Follow-on current-head read-only live evidence passed on revision `73789b7`
  with `git.dirty=false`, SSH checks included, write gates excluded, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/post-blocker-readonly-20260517T100005Z/testnet-live-evidence-refresh.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `101` with zero height lag. Monitor/account-history returned `24`
  indexed rows across five windows with zero archive lookups and zero retained
  history scans. Validator doctor again confirmed services active, state
  verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, private file permissions safe, and
  fleet convergence at height `101`.
- Full current-head live write-gate evidence passed on revision `6156f32`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T085724Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `95` to `97` with transaction
  `37265af0c7cac584695a43d406319b24c98e4bc8a97d03b8766f1c9cb06caeed9ea0c864c89272d1e7c25a89cd83f808`.
  Orchard direct deposit advanced from height `97` to `98` with transaction
  `b2088ed329447f61f4d5f4fc180411b31e9b03cb7f3f99c081e287efca1687cfffb287c6dfc9cbc40791e40cd9166f79`;
  the receipt was accepted, `tx` finality confirmed through full block replay,
  the deposit amount and decrypted note scan were verified, and local/remote
  private material removal checks passed. Account-history index refresh, RPC
  doctor, Python RPC smoke, monitor snapshot with embedded account-history,
  standalone account-history pull, validator doctor, and remote observability
  all passed across five validators/endpoints at height `98` with zero height
  lag. The monitor and account-history pull returned `23` indexed rows across
  four windows with zero archive lookups and zero retained-history scans.
  Validator doctor confirmed services active, state verified, local keys
  valid, account-history aggregate/disk indexes usable/current, matching
  binary hash, private file permissions safe, and fleet convergence at height
  `98`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `37265af0c7cac584695a43d406319b24c98e4bc8a97d03b8766f1c9cb06caeed9ea0c864c89272d1e7c25a89cd83f808`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T091722Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `97`; the
  account-history pull converged across all five endpoints at height `98`,
  returned `23` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T091722Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `a328265` after the latest
  write-gate and receipt evidence:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T091833Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-latest-write-gates-20260517T091916Z.json`.
  The drill restarted all five validator/RPC service pairs sequentially,
  confirmed `restart_ok=true` and local `verify-state` on every validator, and
  post-restart RPC reads converged at height `98`:
  `reports/testnet-remote-restart-drill/logs-post-latest-write-gates-20260517T091916Z/post-restart-rpc-smoke.json`.
  Follow-on validator doctor also passed:
  `reports/testnet-live-validator-doctor/post-latest-write-gate-restart-validator-doctor-20260517T092409Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all state verified, local keys valid,
  account-history aggregate/disk indexes usable/current, matching binary hash,
  private file permissions safe, and fleet convergence at height `98`.
- Full current-head live write-gate evidence passed on revision `da565ce`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T081924Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `92` to `94` with transaction
  `aabc5eb6ea68acf3af3df3b4cf3408c1c1679f624bcf657a48ac74f14ddc197eea9388b18d21050c21d198535b8db627`.
  Orchard direct deposit advanced from height `94` to `95` with transaction
  `5e0a5cd5b9a5b153ae91f0b32b82b56c196875cce3ea283066d38f719423f81e17d02dcc465a50b1976d67ff376d9900`;
  the receipt was accepted, `tx` finality confirmed through full block replay,
  the deposit amount and decrypted note scan were verified, and local/remote
  private material removal checks passed. Account-history index refresh, RPC
  doctor, Python RPC smoke, monitor snapshot with embedded account-history,
  standalone account-history pull, validator doctor, and remote observability
  all passed across five validators/endpoints at height `95` with zero height
  lag. The monitor and account-history pull returned `22` indexed rows across
  four windows with zero archive lookups and zero retained-history scans.
  Validator doctor confirmed services active, state verified, local keys
  valid, account-history aggregate/disk indexes usable/current, matching
  binary hash, private file permissions safe, and fleet convergence at height
  `95`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `aabc5eb6ea68acf3af3df3b4cf3408c1c1679f624bcf657a48ac74f14ddc197eea9388b18d21050c21d198535b8db627`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T083905Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `94`; the
  account-history pull converged across all five endpoints at height `95`,
  returned `22` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T083905Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `bd7cbed` after the latest
  write-gate and receipt evidence:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T084010Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-latest-write-gates-20260517T084100Z.json`.
  The drill restarted all five validator/RPC service pairs sequentially,
  confirmed `restart_ok=true` and local `verify-state` on every validator, and
  post-restart RPC reads converged at height `95`:
  `reports/testnet-remote-restart-drill/logs-post-latest-write-gates-20260517T084100Z/post-restart-rpc-smoke.json`.
  Follow-on validator doctor also passed:
  `reports/testnet-live-validator-doctor/post-latest-write-gate-restart-validator-doctor-20260517T084553Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all state verified, local keys valid,
  account-history aggregate/disk indexes usable/current, matching binary hash,
  private file permissions safe, and fleet convergence at height `95`.
- Full current-head live write-gate evidence passed on revision `e36301d`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T074027Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `89` to `91` with transaction
  `969ae097cee8725cee22478356fd08e5fb12641dfe68b98fa4aa01df8a4017910e93f3f696b018eb037172a53d786113`.
  Orchard direct deposit advanced from height `91` to `92` with transaction
  `f947aa9a5fd4eff0cd86c62e8cb5669ce311c54c15f884b1549ed91269822d8bfcee6f7b4da041b46f4da6cbed5a2f44`;
  the receipt was accepted, `tx` finality confirmed through full block replay,
  the deposit amount and decrypted note scan were verified, and local/remote
  private material removal checks passed. Account-history index refresh, RPC
  doctor, Python RPC smoke, monitor snapshot with embedded account-history,
  standalone account-history pull, validator doctor, and remote observability
  all passed across five validators/endpoints at height `92` with zero height
  lag. The monitor and account-history pull returned `21` indexed rows across
  four windows with zero archive lookups and zero retained-history scans.
  Validator doctor confirmed services active, state verified, local keys
  valid, account-history aggregate/disk indexes usable/current, matching
  binary hash, and fleet convergence at height `92`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `969ae097cee8725cee22478356fd08e5fb12641dfe68b98fa4aa01df8a4017910e93f3f696b018eb037172a53d786113`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T080007Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `91`; the
  account-history pull converged across all five endpoints at height `92`,
  returned `21` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T080007Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `4fcb088` after the latest
  write-gate and receipt evidence:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T080110Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-latest-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-latest-write-gates-20260517T080248Z.json`.
  The drill restarted all five validator/RPC service pairs sequentially,
  confirmed `restart_ok=true` and local `verify-state` on every validator, and
  post-restart RPC reads converged at height `92`:
  `reports/testnet-remote-restart-drill/logs-post-latest-write-gates-20260517T080248Z/post-restart-rpc-smoke.json`.
  Follow-on validator doctor also passed:
  `reports/testnet-live-validator-doctor/post-latest-write-gate-restart-validator-doctor-20260517T080724Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all state verified, local keys valid,
  account-history aggregate/disk indexes usable/current, matching binary hash,
  private file permissions safe, and fleet convergence at height `92`.
- Live write-gate attempt on revision `5769ce4` advanced the chain but the
  aggregate report failed on the old validator-doctor timeout:
  `reports/testnet-live-evidence-refresh/testnet-live-evidence-refresh.json`.
  The run used an empty `--run-id` by operator shell mistake, so reports
  landed under empty-run paths. Wallet finality passed and advanced from
  height `86` to `88` with transaction
  `c2fe29509c98920fc093d2e6471e923da195f26030d87c9bb4387c639f2030eadd9214a35320505d4cc92be95c896f01`.
  Orchard direct deposit passed and advanced from height `88` to `89` with
  transaction
  `d1f8b522a4e1f0622fd79abe9fe996c3519e5a679b03d3487a4010a5a9296342c1f9c9ac3eb9bb7fb54323789116fce4`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor
  snapshot, standalone account-history pull, and remote observability all
  passed across five endpoints at height `89` with zero height lag;
  account-history returned `20` indexed rows with zero archive lookups and
  zero retained-history scans. The original aggregate failed because
  `scripts/testnet-live-validator-doctor` timed out `verify-state` and
  `history-status` at the old per-command limit of `45` seconds. Follow-on
  validator doctor with `COMMAND_TIMEOUT_SECONDS=180` passed:
  `reports/testnet-live-validator-doctor/post-write-gate-timeout-diagnosis-20260517T070039Z/testnet-live-validator-doctor.json`.
  It confirmed services active, state verified, local keys valid,
  account-history indexes usable/current, matching binary hash, and fleet
  convergence at height `89`. Tooling fix in this slice rejects empty run IDs
  and defaults/passes the validator-doctor per-command timeout to `180`
  seconds.
- Current-head read-only live evidence passed after the timeout/tooling fix on
  revision `b2ee9b1` with `git.dirty=false`, SSH checks included, write gates
  excluded, and `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/post-timeout-fix-readonly-20260517T071115Z/testnet-live-evidence-refresh.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor snapshot
  with embedded account-history, standalone account-history pull, validator
  doctor, and remote observability all passed across five validators/endpoints
  at height `89` with zero height lag. The monitor and account-history pull
  returned `20` indexed rows across four windows with zero archive lookups and
  zero retained-history scans. Validator doctor confirmed services active,
  state verified, local keys valid, account-history aggregate/disk indexes
  usable/current, matching binary hash, and fleet convergence at height `89`.
- Follow-on live read-only receipt pull passed for the latest wallet-finality
  transaction
  `c2fe29509c98920fc093d2e6471e923da195f26030d87c9bb4387c639f2030eadd9214a35320505d4cc92be95c896f01`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-post-timeout-fix-20260517T072306Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `88`; the
  account-history pull converged across all five endpoints at height `89`,
  returned `20` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-post-timeout-fix-20260517T072306Z/account-history.csv`,
  proved the transaction appears in history, and recorded zero archive lookups
  and zero retained-history scans.
- RPC method inventory was refreshed on clean `7b6c544` after the timeout fix
  and latest receipt pull:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T072422Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-timeout-fix restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-timeout-fix-20260517T072511Z.json`.
  The drill restarted all five validator/RPC service pairs sequentially,
  confirmed `restart_ok=true` and local `verify-state` on every validator, and
  post-restart RPC reads converged at height `89`:
  `reports/testnet-remote-restart-drill/logs-post-timeout-fix-20260517T072511Z/post-restart-rpc-smoke.json`.
  Follow-on validator doctor also passed:
  `reports/testnet-live-validator-doctor/post-timeout-fix-restart-validator-doctor-20260517T072943Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all state verified, local keys valid,
  account-history aggregate/disk indexes usable/current, matching binary hash,
  private file permissions safe, and fleet convergence at height `89`.
- Full current-head live write-gate evidence passed on revision `0f7deb4`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T060121Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `83` to `85` with transaction
  `98bccf352db9ae8a2a62938477b14c5ccd675c2423ee931ac19cd110e7dece7c05c5796b036790815041653440b3de5e`.
  Orchard direct deposit advanced from height `85` to `86` with transaction
  `5a670ebf7545aa97d91ac77c964937f6b2dbccb9e57128cbbad72f6079580f08b4641c8db76e9cd5bba336a83d5a937e`;
  the receipt was accepted, `tx` finality confirmed through full block replay,
  the deposit amount and decrypted note scan were verified, and local/remote
  private material removal checks passed. Account-history index refresh, RPC
  doctor, Python RPC smoke, monitor snapshot with embedded account-history,
  standalone account-history pull, validator doctor, and remote observability
  all passed across five validators/endpoints at height `86` with zero height
  lag. The monitor and account-history pull returned `19` indexed rows across
  four complete windows with zero archive lookups and zero retained-history
  scans. Validator doctor confirmed services active, state verified, local
  keys valid, account-history aggregate/disk indexes usable/current, matching
  binary hash, and private key file permissions safe.
- Follow-on live read-only receipt pull passed for the new write-gate
  wallet-finality transaction
  `98bccf352db9ae8a2a62938477b14c5ccd675c2423ee931ac19cd110e7dece7c05c5796b036790815041653440b3de5e`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T060121Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `85`; the
  account-history pull converged across all five endpoints at height `86`,
  returned `19` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T060121Z/account-history.csv`,
  proved the transaction appears in every endpoint history, and recorded zero
  archive lookups and zero retained-history scans.
- Post-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-fresh-write-gates-20260517T062232Z.json`.
  The drill restarted all five validator/RPC service pairs sequentially,
  confirmed `restart_ok=true` and local `verify-state` on every validator, and
  post-restart RPC reads converged at height `86`:
  `reports/testnet-remote-restart-drill/logs-post-fresh-write-gates-20260517T062232Z/post-restart-rpc-smoke.json`.
  Follow-on validator doctor also passed:
  `reports/testnet-live-validator-doctor/post-fresh-write-gate-restart-validator-doctor-20260517T062655Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all state verified, local keys valid,
  account-history aggregate/disk indexes usable and current, matching binary
  hash, private file permissions safe, and fleet convergence at height `86`.
- RPC method inventory was refreshed on clean `7cb9c77` after the latest
  restart evidence:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T063639Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Local wallet receipt packet passed on latest head:
  `reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-wallet-receipt-latest-20260517T063800Z.json`.
  It started a local four-validator harness, generated fresh sender/recipient
  wallets, funded the sender, signed transparent spend
  `4d6040819629f75539b93fe88419a4b0c7e10873eb3f40a3eae780b3507c93642fa8595e606a4e55f5c70d97f40fb2ef`,
  confirmed read-only `tx` finality at height `2`, proved sender and
  recipient `account_tx_history` rows, exported matching CSVs, used indexed
  reads only, recorded zero archive lookups and zero retained-history scans,
  and removed private wallet material.
- Full current-head live write-gate evidence passed on revision `c5876e6`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T052626Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `80` to `82` with transaction
  `9a9c990747c7ad7cd9b99c1f662a93d2243b3e7424121376dd51f02245727fef94d7b7ddf7aa1a91d8ef4cff5e812327`.
  Orchard direct deposit advanced from height `82` to `83` with transaction
  `c1838ed0e973ad6bee2999d797a112ce29acdaf1e1401918572c757752042f7feee620151b771e1fa75837e5161461f4`;
  the receipt was accepted, `tx` finality confirmed through full block replay,
  the deposit amount and decrypted note scan were verified, and local/remote
  private material removal checks passed. Account-history index refresh, RPC
  doctor, Python RPC smoke, monitor snapshot with embedded account-history,
  standalone account-history pull, validator doctor, and remote observability
  all passed across five validators/endpoints at height `83` with zero height
  lag. The monitor and account-history pull returned `18` indexed rows across
  four complete windows with zero archive lookups and zero retained-history
  scans. Validator doctor confirmed services active, state verified, local
  keys valid, account-history aggregate/disk indexes usable/current, matching
  binary hash, and private key file permissions safe.
- Follow-on live read-only receipt pull passed for the new write-gate
  wallet-finality transaction
  `9a9c990747c7ad7cd9b99c1f662a93d2243b3e7424121376dd51f02245727fef94d7b7ddf7aa1a91d8ef4cff5e812327`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T052626Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `82`; the
  account-history pull converged across all five endpoints at height `83`,
  returned `18` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T052626Z/account-history.csv`,
  proved the transaction appears in every endpoint history, and recorded zero
  archive lookups and zero retained-history scans.
- RPC method inventory was refreshed on clean `3deab35`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T054545Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-post-latest-write-gates-20260517T054635Z.json`.
  The drill restarted all five validator/RPC service pairs sequentially,
  confirmed `restart_ok=true` and local `verify-state` on every validator, and
  post-restart RPC reads converged at height `83`:
  `reports/testnet-remote-restart-drill/logs-post-latest-write-gates-20260517T054635Z/post-restart-rpc-smoke.json`.
  Follow-on validator doctor also passed:
  `reports/testnet-live-validator-doctor/post-latest-write-gate-restart-validator-doctor-20260517T055051Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all state verified, local keys valid,
  account-history aggregate/disk indexes usable and current, matching binary
  hash, and fleet convergence at height `83`.
- Full current-head live write-gate evidence passed on revision `6a5df4f`
  with `git.dirty=false`, live write gates included, SSH checks included, and
  `sensitive_material_redacted=true`:
  `reports/testnet-live-evidence-refresh/current-write-gates-20260517T044721Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `77` to `79` with transaction
  `df81ae8cb0c423d518c3ddbec84dc7e35e37b8aa8aad7bcf3f34eeda60e27d983a9282866c0b3d65bf70c84001b7fc4d`.
  Orchard direct deposit advanced from height `79` to `80` with transaction
  `f61951930b9baaa72a509a8e915e1e5f095e9b9239e1c7c1869563d6bb98c022694628c5c46be37c6194b1ab5617c341`;
  the receipt was accepted, `tx` finality confirmed through full block replay,
  the deposit amount and decrypted note scan were verified, and local/remote
  private material removal checks passed. Account-history index refresh, RPC
  doctor, Python RPC smoke, monitor snapshot with embedded account-history,
  standalone account-history pull, validator doctor, and remote observability
  all passed across five validators/endpoints at height `80` with zero height
  lag. The monitor and account-history pull returned `17` indexed rows across
  four complete windows with zero archive lookups and zero retained-history
  scans. Validator doctor confirmed services active, state verified, local
  keys valid, account-history aggregate/disk indexes usable/current, matching
  binary hash, and private key file permissions safe.
- Follow-on live read-only receipt pull passed for the new write-gate
  wallet-finality transaction
  `df81ae8cb0c423d518c3ddbec84dc7e35e37b8aa8aad7bcf3f34eeda60e27d983a9282866c0b3d65bf70c84001b7fc4d`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T044721Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `79`; the
  account-history pull converged across all five endpoints at height `80`,
  returned `17` indexed rows for
  `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T044721Z/account-history.csv`,
  proved the transaction appears in every endpoint history, and recorded zero
  archive lookups and zero retained-history scans. New helper:
  `scripts/testnet-live-readonly-receipt-pull`.
- RPC method inventory was refreshed on clean `b76d69f`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T051106Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Post-write-gate restart evidence passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-20260517T051200Z.json`.
  The drill restarted all five validator/RPC service pairs sequentially,
  confirmed `restart_ok=true` and local `verify-state` on every validator, and
  post-restart RPC reads converged at height `80`:
  `reports/testnet-remote-restart-drill/logs-20260517T051200Z/post-restart-rpc-smoke.json`.
  Follow-on validator doctor also passed:
  `reports/testnet-live-validator-doctor/post-write-gate-restart-validator-doctor-20260517T051606Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all state verified, local keys valid,
  account-history aggregate/disk indexes usable and current, matching binary
  hash, and fleet convergence at height `80`.
- Current-head read-only live evidence passed on pushed `ff08ed2` with
  `git.dirty=false`, SSH checks included, and write gates intentionally
  excluded:
  `reports/testnet-live-evidence-refresh/current-readonly-20260517T043417Z/testnet-live-evidence-refresh.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor
  snapshot with embedded account-history, standalone account-history pull,
  validator doctor, and remote observability all passed across five
  validators/endpoints at height `77` with zero height lag. The monitor and
  account-history pull both returned `16` indexed rows for
  `pflivewalletrecipient0000000000000001` across four complete windows with
  zero archive lookups and zero retained-history scans. Validator doctor
  confirmed services active, state verified, local keys valid, aggregate and
  disk account-history indexes usable/current, matching binary hash, and
  private key file permissions safe.
- Current-head write-gate attempt on clean `8af4c0b` wrote
  `reports/testnet-live-evidence-refresh/live-evidence-refresh-20260517T040929Z/testnet-live-evidence-refresh.json`
  and is recorded as failed because the `orchard_direct_deposit` wrapper step
  timed out at `300s` before writing its final report. Diagnosis:
  `reports/testnet-live-evidence-refresh/live-evidence-refresh-20260517T040929Z/write-gate-timeout-diagnosis.json`.
  This was a wrapper timeout, not validator divergence. SDK wallet finality
  advanced from height `74` to `76`, and the Orchard direct-deposit summary
  proved a certified accepted deposit at height `77` with transaction
  `40a5ae89258647e5f06176f7def4816efa1243ba52f551bcf7c6abb639f19960c9dbc5fd3603ef1b24174bf049185bf6`;
  `tx` finality confirmed through full block replay, the scan decrypted the
  new unspent note, and remote private material was removed. All downstream
  live checks passed at height `77`: account-history index refresh, RPC
  doctor, Python RPC smoke, monitor snapshot with embedded account-history,
  standalone account-history pull, validator doctor, and remote observability,
  with zero height lag. Code fix: `scripts/testnet-live-evidence-refresh`
  now gives live write-gate steps the SSH/write timeout instead of the short
  generic timeout. No live retry was performed to avoid repeated live write
  spam. Local wallet receipt fallback passed:
  `reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-wallet-receipt-fallback-20260517T042803Z.json`.
- Follow-on live read-only receipt pull passed for the current-head
  wallet-finality transaction
  `3ba11a04ed92358fad1deb9080900616055a94b09b5cafade6e165b029c7dac80b02703bf6e83c53effa6e8542b50a7b`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-current-20260517T043055Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `76` through the
  selected-block hot path on `validator-0`; the account-history pull
  converged across all five endpoints at height `77`, returned `16` indexed
  rows for `pflivewalletrecipient0000000000000001`, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-current-20260517T043055Z/account-history.csv`,
  proved the transaction appears in every endpoint history, and recorded zero
  archive lookups and zero retained-history scans.
- Full live write-gate evidence passed on pushed `1046a90` with
  `git.dirty=false`, live write gates included, and SSH checks included:
  `reports/testnet-live-evidence-refresh/overnight-write-gates-20260517T031925Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `71` to `73`, Orchard direct
  deposit advanced from height `73` to `74`, account-history index refresh
  passed, and RPC doctor, Python RPC smoke, monitor snapshot with embedded
  account-history, standalone account-history pull, validator doctor, and
  remote observability all passed across five validators/endpoints at height
  `74` with zero height lag. The first endpoint account-history canary
  returned `15` indexed rows across three complete windows with zero archive
  lookups and zero retained-history scans. Validator doctor confirmed all
  services active, state verified, local keys valid, aggregate and disk
  account-history indexes usable, binary hashes matching, and private key file
  permissions safe.
- Live read-only receipt pull passed for transaction
  `ce39f51a1dfde59380f5cfdb6d96d566f28a70877108c4eb6e780fab73659874ec6a053367e5b807d31a008bee488ca8`
  after fixing RPC JSON boolean handling for `audit_block_log: false`.
  Evidence:
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-20260517T035053Z/live-readonly-receipt-pull.json`.
  Public read-only `tx` confirmed the spend at block height `73`; the
  account-history pull converged across all five endpoints at height `74`,
  returned `15` indexed rows, wrote
  `reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-20260517T035053Z/account-history.csv`,
  proved the tx appears in every endpoint history, and recorded zero archive
  lookups and zero retained-history scans.
- RPC method inventory was refreshed on clean revision `d352f04`:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T035236Z.json`.
  The public surface remained unchanged at `53` observed methods: `27`
  read-only public, `1` controlled write-gated, `3` privacy-alpha gated, and
  `22` operator/local-only.
- Live restart drill passed:
  `reports/testnet-remote-restart-drill/testnet-remote-restart-drill-overnight-restart-20260517T035428Z.json`.
  The drill restarted all five validator/RPC service pairs sequentially,
  confirmed each validator reported `restart_ok=true`, services active, and
  local `verify-state` green, then ran a post-restart RPC smoke that converged
  at height `74`:
  `reports/testnet-remote-restart-drill/logs-overnight-restart-20260517T035428Z/post-restart-rpc-smoke.json`.
  Follow-on validator doctor also passed:
  `reports/testnet-live-validator-doctor/post-restart-validator-doctor-20260517T035903Z/testnet-live-validator-doctor.json`.
  It confirmed all services active, all state verified, local keys valid,
  account-history aggregate/disk indexes usable and current, binary hashes
  matching, and fleet convergence at height `74`. The restart and doctor
  reports were scanned for private-key-shaped fields, credentials, and raw IP
  literals with no matches.
- `scripts/testnet-wallet-receipt-packet-smoke` landed as the local operator
  receipt packet. It starts a local four-validator harness, generates
  sender/recipient wallets, funds the sender, signs one transparent transfer,
  seals it, verifies `tx` finality over read-only RPC, verifies sender and
  recipient `account_tx_history`, and writes sender/recipient CSV exports.
  Latest evidence passed at height `2`:
  `reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-wallet-receipt-packet-20260517T034103Z.json`.
  The packet proved the sender history contains both funding and spend rows,
  the recipient history contains the inbound spend row, both CSV row counts
  match the JSON histories, all reads were indexed, archive lookup count was
  zero, retained-history scan count was zero, and private wallet material was
  removed before final reporting.
- Fresh read-only live evidence passed on pushed `d4391db` with
  `git.dirty=false`, SSH checks included, and write gates intentionally
  excluded:
  `reports/testnet-live-evidence-refresh/bandwidth-readonly-20260517T025250Z/testnet-live-evidence-refresh.json`.
  Account-history index refresh, RPC doctor, Python RPC smoke, monitor
  snapshot with embedded account-history, standalone account-history pull,
  validator doctor, and remote observability all passed across five
  validators/endpoints at height `71` with zero height lag. The first endpoint
  account-history canary returned `14` indexed rows across three complete
  windows with zero archive lookups and zero retained-history scans. Validator
  doctor confirmed all services active, state verified, local keys valid,
  aggregate and disk account-history indexes usable, binary hashes matching,
  and private key file permissions safe.
- `scripts/postfiat-rpc-account-tx` now supports `--csv-output` for a flat
  operator/integration export of transparent account history. The JSON report
  remains canonical and records the CSV path, source endpoint label, and row
  count. Local six-wallet fan-in smoke now exercises the CSV path and passed
  with six sink rows plus header:
  `reports/testnet-six-wallet-account-tx-smoke/testnet-six-wallet-account-tx-smoke-csv-account-tx-20260517T030434Z.json`.
  Live read-only CSV evidence also passed across all five endpoints at height
  `71`, with row fingerprints converged, `14` indexed rows exported from the
  first endpoint, zero archive lookups, and zero retained-history scans:
  `reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-live-account-tx-csv-20260517T030514Z.json`.

## 2026-05-16 Progress

- Six-wallet account-history fan-in smoke landed as
  `scripts/testnet-six-wallet-account-tx-smoke`. It starts a local
  four-validator harness, generates six sender wallets plus one sink wallet,
  funds the six senders, signs six wallet transfers into the sink, seals them
  in one transparent batch, rebuilds `account_tx`, and proves read-only RPC
  `account_tx_history` registers the sink fan-in plus each sender's funding
  and outbound transaction. Latest local evidence passed at height `7` with
  `12` indexed rows across `8` accounts, all history reads indexed, zero
  archive lookups, zero retained-history scans, and no key-shaped fields in
  the RPC evidence:
  `reports/testnet-six-wallet-account-tx-smoke/testnet-six-wallet-account-tx-smoke-six-wallet-account-tx-20260516T130635Z.json`.
- Full current-head launch/privacy/finality evidence on pushed `cff47ba`
  passed with `git.dirty=false`, live write gates included, and SSH checks
  included:
  `reports/testnet-live-evidence-refresh/full-launch-current-cff47ba-20260516T121723Z/testnet-live-evidence-refresh.json`.
  SDK wallet finality advanced from height `66` to `68`, Orchard direct
  deposit advanced from height `68` to `69`, account-history index refresh
  passed, and RPC doctor, Python RPC smoke, monitor snapshot with embedded
  account-history, standalone account-history pull, validator doctor, and
  remote observability all passed at height `69` with zero height lag. The
  embedded monitor history canary returned 13 rows on the first endpoint
  across three complete indexed windows, with zero archive lookups and zero
  retained-history scans:
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-full-launch-current-cff47ba-20260516T121723Z.json`.
- Python RPC query CLI landed in `14ee4ec` as `scripts/postfiat-rpc-query` and
  `python -m postfiat_rpc`. It exposes the stdlib Python client as a
  read-only operator CLI for one-off queries, including bounded
  `account_tx_history`, and writes redaction-safe `postfiat-rpc-query-v1`
  reports under `reports/postfiat-rpc-query/`. Post-push local smoke passed
  with the CLI proving one indexed account-history row and zero
  archive/scanned-history fallback under an exact 17-request read-only RPC
  budget:
  `reports/testnet-python-rpc-client-smoke/testnet-python-rpc-client-smoke-python-rpc-query-cli-clean-20260516T124021Z.json`.
  Post-push live single-endpoint CLI evidence also passed at height `69`,
  returning 13 rows across three complete indexed windows with the endpoint
  host redacted:
  `reports/postfiat-rpc-query/postfiat-rpc-query-live-python-rpc-query-clean-20260516T124035Z.json`.
- Monitor snapshot account-history canary landed in `3bd5e6a`
  (`Embed account history in monitor snapshots`). Operators can now run
  `scripts/testnet-monitor-snapshot --include-account-tx-history` to embed the
  same bounded `PostFiatRpcClient.account_tx_history()` health check directly
  in the cron-friendly monitor JSON. The canary records per-endpoint row
  counts, window counts, indexed-read status, archive lookup counts, and
  retained-history scan counts, and warns when history fails, is not indexed,
  or falls back to archive/scan paths. Local smoke passed with an exact
  read-only RPC budget of 19 requests:
  `reports/testnet-monitor-snapshot-smoke/testnet-monitor-snapshot-smoke-monitor-history-canary-20260516T115133Z.json`.
  Clean-tree read-only live aggregate evidence on `3bd5e6a` passed with
  `git.dirty=false` at height `66`; RPC doctor, Python RPC smoke, monitor
  snapshot with embedded account-history, and standalone account-history pull
  all passed across five endpoints with zero height lag:
  `reports/testnet-live-evidence-refresh/monitor-history-clean-20260516T115737Z/testnet-live-evidence-refresh.json`.
  The embedded monitor history canary returned 12 rows on the first endpoint
  across three complete indexed windows, with zero archive lookups and zero
  retained-history scans:
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-monitor-history-clean-20260516T115737Z.json`.
  Follow-on SSH-inclusive read-only aggregate evidence on pushed `cb06510`
  also passed with `git.dirty=false`; account-history index refresh, RPC
  doctor, Python RPC smoke, monitor snapshot with embedded account-history,
  standalone account-history pull, validator doctor, and remote observability
  all passed at height `66` with zero height lag:
  `reports/testnet-live-evidence-refresh/monitor-history-ssh-clean-20260516T120349Z/testnet-live-evidence-refresh.json`.
- Fresh follow-on live evidence passed on the controlled network at height
  `54`. SDK wallet finality advanced from height `51` to `53`, Orchard direct
  deposit advanced from height `53` to `54`, account-history index refresh
  passed on all five validators, and RPC doctor, Python RPC smoke, monitor
  snapshot, Python account-history pull, validator doctor, and remote
  observability all passed with zero height lag:
  `reports/testnet-live-wallet-finality/overnight-finality-refresh-20260516T094649Z/testnet-live-wallet-finality.json`,
  `reports/testnet-live-orchard-direct-deposit/overnight-orchard-refresh-20260516T094813Z/testnet-live-orchard-direct-deposit.json`,
  `reports/testnet-live-account-tx-index-refresh/overnight-account-tx-index-20260516T095215Z/testnet-live-account-tx-index-refresh.json`,
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-overnight-refresh-20260516T095305Z.json`,
  `reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-overnight-refresh-20260516T095306Z.json`,
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-overnight-refresh-20260516T095306Z.json`,
  `reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-overnight-refresh-20260516T095306Z.json`,
  `reports/testnet-live-validator-doctor/overnight-validator-doctor-20260516T095306Z/testnet-live-validator-doctor.json`,
  and
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T095306Z.json`.
  The account-history pull returned eight converged rows per endpoint, all
  indexed, with zero archive lookups and zero retained-history scans. The
  fresh report set was scanned against the live credential file and
  private-key-shaped fields with no matches.
- Live evidence refresh wrapper landed as `scripts/testnet-live-evidence-refresh`.
  It orchestrates the read-only live RPC doctor, Python RPC smoke, monitor
  snapshot, and Python account-history pull into one aggregate report, includes
  SSH checks by default, and can explicitly include live write gates with a
  flag. Read-only
  wrapper evidence passed at height `54`:
  `reports/testnet-live-evidence-refresh/live-evidence-readonly-20260516T100102Z/testnet-live-evidence-refresh.json`.
  SSH-inclusive read-only wrapper evidence also passed at height `54`:
  `reports/testnet-live-evidence-refresh/live-evidence-ssh-readonly-20260516T101651Z/testnet-live-evidence-refresh.json`.
  Full clean-tree wrapper evidence on pushed `7e7c910` passed with
  `git.dirty=false`, live write gates included, and SSH checks included:
  wallet finality advanced from height `57` to `59`, Orchard direct deposit
  advanced from height `59` to `60`, account-history refresh passed, and RPC
  doctor, Python RPC smoke, monitor snapshot, Python account-history pull,
  validator doctor, and remote observability all passed at height `60` with
  zero lag:
  `reports/testnet-live-evidence-refresh/live-evidence-full-clean-20260516T104752Z/testnet-live-evidence-refresh.json`.
  The remote observability path now treats expensive `verify_state` RPC reads
  as opt-in, matching RPC doctor; validator doctor remains the
  state-verification gate.
- Fresh current-head wrapper evidence on pushed `d0a9683` passed with
  `git.dirty=false`, live write gates included, and SSH checks included:
  wallet finality advanced from height `60` to `62`, Orchard direct deposit
  advanced from height `62` to `63`, account-history refresh passed, and RPC
  doctor, Python RPC smoke, monitor snapshot, Python account-history pull,
  validator doctor, and remote observability all passed at height `63` with
  zero lag:
  `reports/testnet-live-evidence-refresh/live-evidence-full-current-20260516T110640Z/testnet-live-evidence-refresh.json`.
- Python RPC account-history helper landed as
  `PostFiatRpcClient.account_tx_history()`, and `scripts/postfiat-rpc-account-tx`
  now uses the same client helper instead of carrying separate window-walk
  logic. The helper resolves current height when needed, walks deterministic
  bounded windows, deduplicates rows, fails closed on truncation unless
  explicitly allowed, and records indexed/scanned/archive counters. Local
  smoke passed:
  `reports/testnet-python-rpc-client-smoke/testnet-python-rpc-client-smoke-20260516T112406Z.json`.
  Live Python client smoke passed across all five read-only endpoints at
  height `63`, returning 11 rows per endpoint across three complete indexed
  windows:
  `reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-python-history-helper-live-20260516T112425Z.json`.
  The standalone account-history pull also passed with row fingerprints
  converged and zero archive lookups/scans:
  `reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-python-history-helper-account-tx-20260516T112425Z.json`.
  Post-commit read-only aggregate evidence on pushed `fbb7a0a` passed with
  `git.dirty=false`, proving RPC doctor, Python RPC smoke with
  `account_tx_history()`, monitor snapshot, and standalone account-history
  pull at height `63`:
  `reports/testnet-live-evidence-refresh/python-history-helper-readonly-20260516T112740Z/testnet-live-evidence-refresh.json`.
- Fresh overnight launch/privacy/finality evidence passed on the live
  controlled network at height `51`. SDK wallet finality advanced from height
  `48` to `50`, Orchard direct deposit advanced from height `50` to `51`,
  account-history index refresh passed on all five validators, and RPC doctor,
  Python RPC smoke, monitor snapshot, validator doctor, and remote
  observability all passed at height `51` with zero height lag:
  `reports/testnet-live-wallet-finality/overnight-finality-refresh-20260516T090806Z/testnet-live-wallet-finality.json`,
  `reports/testnet-live-orchard-direct-deposit/overnight-orchard-refresh-20260516T090925Z/testnet-live-orchard-direct-deposit.json`,
  `reports/testnet-live-account-tx-index-refresh/overnight-account-tx-index-20260516T091319Z/testnet-live-account-tx-index-refresh.json`,
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-overnight-refresh-20260516T091420Z.json`,
  `reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-overnight-refresh-20260516T091420Z.json`,
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-overnight-refresh-20260516T091420Z.json`,
  `reports/testnet-live-validator-doctor/overnight-validator-doctor-20260516T091420Z/testnet-live-validator-doctor.json`,
  and
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T091420Z.json`.
  The live validator doctor confirms all five services active, state verified,
  binary hashes matching, local keys valid, account-history aggregate and disk
  indexes usable, and 31 indexed rows across 23 disk account shards on every
  validator. The fresh report set was scanned against the live credential file
  and private-key-shaped fields with no matches.
- Disk-only account-history operator smoke landed as
  `scripts/testnet-account-tx-disk-index-smoke`. It starts a local
  four-validator harness, finalizes a transparent canary transfer, removes the
  aggregate `account_tx_index.json` from the serving validator, confirms
  `account_tx_index_status` reports aggregate absent but disk index usable, and
  proves both CLI and read-only RPC `account_tx` return the canary row with
  `index_used=true`, `scanned_block_count=0`, and `archive_lookup_count=0`.
  The paired monitor snapshot now reports `status=ok` using effective
  aggregate-or-disk index readiness when the aggregate index is absent and the
  disk shards are usable. Validator doctor uses the same effective readiness
  rule, so a validator with aggregate absent but disk shards usable remains
  `account_tx_index_all_ready=true`.
  Evidence:
  `reports/testnet-account-tx-disk-index-smoke/testnet-account-tx-disk-index-smoke-20260516T093544Z.json`,
  `reports/testnet-monitor-snapshot/testnet-account-tx-disk-index-smoke-20260516T093544Z.json`,
  and
  `reports/testnet-validator-doctor/testnet-account-tx-disk-index-smoke-20260516T093544Z.json`.
  The updated live validator doctor also passed at height `51`:
  `reports/testnet-live-validator-doctor/effective-index-validator-doctor-20260516T093606Z/testnet-live-validator-doctor.json`.
- Python account-history pull tooling landed as
  `scripts/postfiat-rpc-account-tx`. It reads one or more read-only endpoints,
  walks a requested height range in bounded windows, deduplicates transparent
  account-history rows, redacts endpoint hosts, and fails closed on truncated
  windows unless `--allow-truncated` is explicit. Live evidence across all five
  endpoints for the public wallet canary passed at height `51` with row
  fingerprints converged, seven rows per endpoint, all windows indexed, zero
  archive lookups, and zero retained-history scans:
  `reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-live-canary-20260516T093108Z.json`.
- Disk-backed account-history read index v0 was pushed and deployed as
  `8e43645`. The live release binary hash is
  `1888dcc877cd4e9cbbdd6c88979cce99ad05ef601d73cf6991d28c4573119964`.
  Upgrade evidence:
  `reports/testnet-live-orchard-binary-upgrade/live-account-tx-disk-index-upgrade-20260516T084228Z/testnet-live-orchard-binary-upgrade.json`.
  Manual live index refresh wrote `account_tx_index_meta.json` and per-account
  shards on all five validators at height `45`:
  `reports/testnet-live-account-tx-index-refresh/live-account-tx-disk-index-20260516T084521Z/testnet-live-account-tx-index-refresh.json`.
  Fresh wallet finality then advanced the network from height `45` to `47`,
  and Orchard direct deposit advanced it to `48`:
  `reports/testnet-live-wallet-finality/post-disk-index-wallet-finality-20260516T084601Z/testnet-live-wallet-finality.json`
  and
  `reports/testnet-live-orchard-direct-deposit/post-disk-index-orchard-direct-deposit-20260516T084737Z/testnet-live-orchard-direct-deposit.json`.
  RPC doctor, Python client, monitor snapshot, validator doctor, and remote
  observability all passed at height `48`; validator doctor reports 29 indexed
  rows and 22 disk account shards on every validator:
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-20260516T085123Z.json`,
  `reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-live-disk-index-python-rpc-20260516T085206Z.json`,
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-live-disk-index-monitor-20260516T085247Z.json`,
  `reports/testnet-live-validator-doctor/live-disk-index-validator-doctor-20260516T085327Z/testnet-live-validator-doctor.json`,
  and
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T085805Z.json`.
- Incremental retained account-history index refresh landed and was deployed.
  `55b3fdb` adds a cache append path; `0221d62` extends it to catch up from a
  known ancestor cache tip. The live release binary hash is
  `c2764ef60f1cdf66a1b3bc0bc916010869c37c5be1f89a29f6481e168846f37b`.
  Upgrade evidence:
  `reports/testnet-live-orchard-binary-upgrade/live-account-tx-index-catchup-upgrade-20260516T080506Z/testnet-live-orchard-binary-upgrade.json`.
  Local code evidence:
  `reports/testnet-account-tx-index-incremental/account-tx-index-incremental-20260516T075611Z/testnet-account-tx-index-incremental.json`.
  Focused checks passed: `cargo fmt --check`, `cargo check -p
  postfiat-node`, `cargo test -p postfiat-node
  account_tx_index_auto_refresh_catches_up_after_archive_prune -- --nocapture`,
  and `cargo test -p postfiat-node init_then_run_once -- --nocapture`.
- Post-upgrade live evidence is green at height `45`. Wallet finality passed
  from height `42` to `44`:
  `reports/testnet-live-wallet-finality/post-upgrade-wallet-finality-20260516T080751Z/testnet-live-wallet-finality.json`.
  Orchard direct deposit passed from height `44` to `45`:
  `reports/testnet-live-orchard-direct-deposit/post-upgrade-orchard-direct-deposit-20260516T080908Z/testnet-live-orchard-direct-deposit.json`.
  Account-history index refresh passed with 27 indexed rows across 21 accounts:
  `reports/testnet-live-account-tx-index-refresh/post-upgrade-account-tx-index-20260516T081246Z/testnet-live-account-tx-index-refresh.json`.
  RPC doctor, Python client, monitor, validator doctor, and remote
  observability all passed with five endpoints/validators, zero height lag,
  usable indexed `account_tx`, read-only RPC posture, active services, and
  matching deployed binary hashes:
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-20260516T081331Z.json`,
  `reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-post-upgrade-python-rpc-20260516T081412Z.json`,
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-post-upgrade-monitor-20260516T081503Z.json`,
  `reports/testnet-live-validator-doctor/post-upgrade-validator-doctor-20260516T081546Z/testnet-live-validator-doctor.json`,
  and
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T082041Z.json`.
- Local validation refresh passed before the first tooling slice:
  `cargo check --workspace` and `cargo test -p postfiat-rpc-sdk`.
- P0 RPC doctor tooling landed as `scripts/testnet-rpc-doctor`, with local
  smoke wrapper `scripts/testnet-rpc-doctor-smoke`.
- Latest smoke evidence:
  `reports/testnet-rpc-doctor-smoke/testnet-rpc-doctor-smoke-20260516T054735Z.json`.
- Latest doctor report:
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-smoke-20260516T054735Z.json`.
- The latest smoke exercised one local read-only validator RPC endpoint, 16
  read methods including `account`, server-side indexed `account_tx`, and
  `account_tx_index_status`, an Orchard pool report, and a harmless
  write-posture probe. It confirmed
  `doctor_ok=true`, zero height lag, registry-root consistency, read-only
  posture, and a non-empty indexed `account_tx` canary row matching a
  finalized local funding transfer. The smoke keeps default node key material
  in `/tmp` and removes it on exit.
- Python RPC client v0 landed under `python/postfiat_rpc/`, with runbook
  `docs/runbooks/python-rpc-client.md` and smoke wrapper
  `scripts/testnet-python-rpc-client-smoke`.
- Latest Python client smoke:
  `reports/testnet-python-rpc-client-smoke/testnet-python-rpc-client-smoke-20260516T054717Z.json`.
  It read 14 public methods including `account_tx_index_status` plus
  server-side indexed `account_tx` through a local read-only RPC endpoint,
  created and applied one transparent funding transfer, and proved
  `account_tx` returned the finalized faucet history row with
  `index_used=true`. It keeps default smoke node key material in `/tmp` with
  cleanup on exit.
- Live Python client smoke now passes:
  `reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-live-python-rpc-20260516T073702Z.json`.
  It used `scripts/testnet-live-python-rpc-client-smoke` against the five
  deployed read-only endpoints, checked 16 methods per endpoint, verified
  `account_tx_index_status` is usable on every validator, and proved indexed
  `account_tx` returns account-history rows for the public wallet canary with
  zero height lag at height `42`.
- Monitor snapshot tooling landed as `scripts/testnet-monitor-snapshot`, with
  smoke wrapper `scripts/testnet-monitor-snapshot-smoke`.
- Latest monitor snapshot smoke:
  `reports/testnet-monitor-snapshot-smoke/testnet-monitor-snapshot-smoke-20260516T054750Z.json`.
  It wraps RPC doctor output into cron-friendly health JSON with endpoint
  status, height lag, read-only posture, RPC latency thresholding, mempool
  counters, Orchard public pool counters, and optional transparent account /
  `account_tx` canary status, and top-level `account_tx_index` freshness. The
  latest smoke proves the monitor canary sees a non-empty finalized
  account-history row with `account_tx_index_used=true` and
  `account_tx_index.usable=true`. The smoke keeps default node key material in
  `/tmp` and removes it on exit.
- Live monitor snapshot now passes:
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-live-monitor-20260516T073741Z.json`.
  It records five online/read-only endpoints, BFT quorum observable, zero
  height lag at height `42`, no warnings/criticals, indexed `account_tx`
  canary rows, usable account-history indexes, and Orchard public pool
  counters.
- Validator doctor tooling landed as `scripts/testnet-validator-doctor`, with
  runbook `docs/runbooks/validator-doctor.md` and smoke wrapper
  `scripts/testnet-validator-doctor-smoke`.
- Latest validator doctor smoke:
  `reports/testnet-validator-doctor-smoke/testnet-validator-doctor-smoke-20260516T055726Z.json`.
  Latest doctor report:
  `reports/testnet-validator-doctor/testnet-validator-doctor-smoke-20260516T055726Z.json`.
  It exercised four local validator data dirs after one finalized transparent
  transfer, confirmed BFT quorum health `3/4`, zero height lag, consistent
  chain/state/registry roots, partial-history readiness, `4/4` usable
  auto-refreshed account-history indexes, data-dir and private-key-file
  permissions, and redaction-safe public-key fingerprints.
- Live validator doctor now passes:
  `reports/testnet-live-validator-doctor/live-validator-doctor-20260516T073826Z/testnet-live-validator-doctor.json`.
  It checked all five controlled validators over SSH, with validator/RPC
  services active, local state verified, partial-history retention ready,
  usable/current account-history indexes, local split validator keys valid with
  safe permissions, required public state files present, matching deployed
  binary hash, and full height/tip/state/registry convergence at height `42`.
- RPC method inventory landed as `scripts/testnet-rpc-method-inventory` and
  `docs/runbooks/rpc-method-inventory.md`.
- Latest RPC inventory report:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260516T054841Z.json`.
  It derives 53 methods from Rust SDK constants, node RPC dispatch, remote
  `rpc-serve` allowlists, and the Python client. Current posture: 27
  read-only public methods, 1 controlled-write gated method, 3 privacy-alpha
  gated methods, and 22 operator/local-only methods.
- The inventory found and this slice fixed the Rust SDK block-range mismatch:
  `postfiat-rpc-sdk request --method blocks` now supports `--from-height`
  alongside `--limit`, matching node RPC and the Python client.
- Operator day-two runbook landed as `docs/runbooks/operator-day-two.md`,
  with copy-paste paths for validator doctor, RPC doctor, monitor snapshot,
  Python RPC client checks, RPC method inventory, controlled-write posture,
  history status, restart/log handling, and key rotation.
- Local overnight evidence refresh wrapper landed as
  `scripts/testnet-overnight-evidence-refresh`. It is the one-command local
  refresh for workspace checks, RPC/validator/monitor/Python smoke evidence,
  RPC method inventory, Orchard deterministic vectors, and a short transparent
  tx-finality latency benchmark.
- Canonical clean overnight refresh evidence on the current pushed head:
  `reports/testnet-overnight-evidence-refresh/overnight-evidence-refresh-20260516T050131Z/testnet-overnight-evidence-refresh.json`.
  It passed all 9 wrapper steps with `git.dirty=false`, `cargo check
  --workspace`, `cargo test -p postfiat-rpc-sdk`, RPC/validator/monitor/Python
  smokes, RPC method inventory, Orchard deterministic vectors, and a 4-validator
  3-round tx-finality benchmark. The benchmark recorded submit-to-finality
  p50 `1472.666216ms`, p95 `1508.592396ms`, and p99 `1508.592396ms`. The
  RPC doctor, monitor, and Python smoke summaries include non-empty
  `account_tx` rows matching local finalized funding transfers. The report
  confirms sensitive tmp cleanup and no key-bearing files under the final
  evidence tree.
- Latest clean current-head tooling refresh on revision
  `1473789a67ba530aff5ad4ad9c0a4d3cab9fde73`:
  `reports/testnet-overnight-evidence-refresh/current-head-tooling-20260516T072431Z/testnet-overnight-evidence-refresh.json`.
  It passed with `git.dirty=false` before/after, `cargo check --workspace`,
  `cargo test -p postfiat-rpc-sdk`, RPC doctor smoke, validator doctor smoke,
  monitor snapshot smoke, Python RPC client smoke, and RPC method inventory.
  Privacy/finality were intentionally skipped because the current-head delta
  was operator tooling and evidence docs; the earlier full refresh remains the
  full local privacy/finality packet.
- Server-side bounded `account_tx` is included in the current canonical refresh. The
  node exposes it as a read-only public `rpc-serve` method, the Rust SDK can
  build and validate `account_tx` requests/responses, the Python client uses it
  with client-side fallback for old endpoints, and the latest smoke/inventory
  evidence above and the current canonical refresh record it. Remaining
  inventory gaps are intentional/no-blocker follow-ups: aggregate index
  compaction/metadata-only mode and public write edge gating.
- Rebuildable retained-history `account_tx` index v0 landed after the
  canonical full refresh. `postfiat-node account-tx-index-build` writes
  `account_tx_index.json` from local block/archive/receipt data,
  `account-tx-index-status` and public read RPC `account_tx_index_status`
  report freshness without leaking an operator filesystem path, ordered block
  commit refreshes the cache automatically on a best-effort basis, commit
  refresh now appends instead of rebuilding when the previous cache tip is a
  known ancestor of the current local tip, and `account_tx` prefers
  disk-backed per-account shards when `account_tx_index_meta.json` matches the
  local chain/genesis/protocol/tip. It falls back to the aggregate index and
  then bounded retained-history scan when the disk index is absent or stale.
  Operator doc: `docs/runbooks/account-tx-index.md`.
- Disk-backed account-history read index v0 landed after the ancestor-catchup
  slice. Local evidence:
  `reports/testnet-account-tx-disk-index/account-tx-disk-index-20260516T083817Z/testnet-account-tx-disk-index.json`.
  The focused Rust regression deletes the aggregate `account_tx_index.json`
  and proves `account_tx` still returns indexed rows from per-account shards
  with zero archive scans. The RPC doctor, monitor snapshot, validator doctor,
  and Python client smokes all pass and report the disk index as usable:
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-smoke-20260516T083613Z.json`,
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-smoke-20260516T083629Z.json`,
  `reports/testnet-validator-doctor/testnet-validator-doctor-smoke-20260516T083641Z.json`,
  and
  `reports/testnet-python-rpc-client-smoke/logs-20260516T083525Z/python-client-report.json`.
- Latest indexed account-history smoke evidence:
  `reports/testnet-python-rpc-client-smoke/testnet-python-rpc-client-smoke-20260516T054717Z.json`,
  `reports/testnet-rpc-doctor-smoke/testnet-rpc-doctor-smoke-20260516T054735Z.json`,
  and
  `reports/testnet-monitor-snapshot-smoke/testnet-monitor-snapshot-smoke-20260516T054750Z.json`.
  All three omit the manual index-build step and prove a finalized local
  funding transfer is returned with `index_used=true` after commit-time cache
  refresh. Latest generated RPC inventory:
  `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260516T054841Z.json`
  with 53 methods, 27 read-only public methods, and the remaining aggregate
  index compaction scale follow-up.
- Clean post-index tooling refresh evidence on revision
  `76d5614f2dc5188b1f078bd51ae72ca34d99db00`:
  `reports/testnet-overnight-evidence-refresh/overnight-evidence-refresh-20260516T055152Z/testnet-overnight-evidence-refresh.json`.
  It passed the local wrapper with `git.dirty=false`, skipping only privacy and
  finality because the previous canonical full refresh already covered them.
  The refresh passed `cargo check --workspace`, `cargo test -p
  postfiat-rpc-sdk`, RPC/validator/monitor/Python smokes, and RPC method
  inventory. RPC doctor, monitor, and Python summaries all record finalized
  `account_tx` canaries with `index_used=true` after commit-time cache refresh;
  the monitor summary also records `account_tx_index.usable=true`. The final
  evidence tree was checked for key-bearing files.
- Clean validator-doctor index-readiness refresh evidence on revision
  `7118dbaefa18e9366a4ebcf30acf56aacbab2789`:
  `reports/testnet-overnight-evidence-refresh/overnight-evidence-refresh-20260516T055907Z/testnet-overnight-evidence-refresh.json`.
  It passed the local wrapper with `git.dirty=false`, skipping only privacy and
  finality because the previous canonical full refresh already covered them.
  The refresh passed `cargo check --workspace`, `cargo test -p
  postfiat-rpc-sdk`, RPC/validator/monitor/Python smokes, and RPC method
  inventory. The validator doctor smoke now commits one local transparent
  transfer across the four-validator harness and records
  `account_tx_index_all_ready=true` with `4/4` present and usable
  auto-refreshed account-history indexes. The final evidence tree was checked
  for private-key-shaped fields and key files.
- Live observability retry passed with available credentials after the local
  tooling refresh:
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T060955Z.json`.
  It records five active validator/RPC service pairs, live read RPC success
  across six methods per validator, zero height lag at height `32`, non-empty
  data/log/event counters, wildcard RPC listeners, and successful loopback TCP
  connects on every validator. The script now fails soft on RPC smoke failure
  and still collects SSH/systemd service posture; the live RPC smoke timeout is
  raised to 30s by default from observability to avoid false failures on
  expensive `verify_state` reads.
- Live RPC/index upgrade closed the current-head doctor gap. The pushed source
  now includes `account-tx-index-status` in the live binary-upgrade command
  surface check, the live binary upgrade installed hash
  `7a31dd010021bb589ab19cba6f7bb3ece277e6a25ae5625a8dc22d404772b35d` across
  three machines/five validators with services active and state verified:
  `reports/testnet-live-orchard-binary-upgrade/live-rpc-index-20260516T065327Z/testnet-live-orchard-binary-upgrade.json`.
- Latest live account-history index refresh passed:
  `reports/testnet-live-account-tx-index-refresh/live-account-tx-index-20260516T073517Z/testnet-live-account-tx-index-refresh.json`.
  It built `account_tx_index.json` on all five validators at height `42`
  without changing chain state; each validator reports a present/usable index,
  matching tip hash, 25 indexed rows, and 20 accounts.
- Full current-head live RPC doctor now passes:
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-20260516T073552Z.json`.
  It checked five live read-only endpoints against the full 16-method surface
  including `account`, indexed `account_tx`, and `account_tx_index_status`;
  all endpoints passed at height `42`, height lag was zero, chain/genesis /
  protocol / registry roots were consistent, all endpoints remained read-only,
  and account-history index status was usable/path-redacted on every validator.
- Post-upgrade live observability passed:
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T074314Z.json`.
  It records five active validator/RPC service pairs, live read RPC success,
  zero height lag at height `42`, and zero transport rejections after the
  wallet/finality, privacy, index, RPC/Python/monitor, and validator-doctor
  refreshes.
- Live continuity refresh passed with available credentials:
  `reports/testnet-remote-continuity/overnight-20260516T0612/testnet-remote-continuity.json`.
  It ran pre- and post-continuity read RPC smokes against five validators,
  executed two certified transparent rounds without redeploying, advanced the
  network from height `32` to expected height `34`, and ended with all five
  validators converged on the same tip/state root. The continuity wrapper now
  also defaults its live RPC smoke timeout to 30s, matching observability and
  avoiding false failures on expensive `verify_state` reads.
- Live SDK wallet/finality refresh passed with available credentials:
  `reports/testnet-live-wallet-finality/overnight-20260516T073001Z/testnet-live-wallet-finality.json`.
  It funded a fresh SDK wallet, quoted and signed a transfer locally, submitted
  through a one-request SSH-local write edge, certified the wallet spend, and
  verified `tx` finality through read-only RPC at height `41`. The refreshed
  script now removes generated private wallet material after signing and the
  report records `wallet_private_material_removed=true`.
- Live Orchard direct-deposit privacy-alpha refresh passed with available
  credentials:
  `reports/testnet-live-orchard-direct-deposit/overnight-20260516T073133Z/testnet-live-orchard-direct-deposit.json`.
  It certified a transparent-to-Orchard direct deposit at height `42`, verified
  `tx` finality, scanned one decrypted unspent Orchard output, kept the public
  write edge closed, and records both remote and local private material cleanup.

## P0 - Evidence Refresh And Soak

Goal: make sure the currently deployed network and current repo head still
behave like the docs say.

Tasks:

- [x] Run the highest-signal local validation that is reasonable for the
  machine: `cargo check --workspace`, focused node/RPC SDK tests, and any
  existing evidence-pack scripts that do not require live credentials.
- [x] Capture a live network status snapshot if credentials are available:
  validator service status, RPC status, heights, registry root, block tip,
  read-only/write-edge posture, disk usage, and recent logs.
- [x] Run a bounded continuity soak if live services are reachable: enough
  certified rounds to prove height advances and all validators reconverge.
- [x] Run the live wallet/finality path if the controlled write edge is
  available; otherwise record that write access is blocked and continue with
  read-only RPC/monitoring work.
- [x] Run one privacy-alpha smoke only if it is already scripted and does not
  block the whole night on proving cost.

Acceptance:

- A report exists under `reports/` with `git.dirty=false` where applicable.
- Any failure includes the exact command, service, host, and log excerpt needed
  for triage.
- No claim docs are upgraded unless the evidence supports the claim.

## P0 - RPC Doctor

Status: done for local smoke and full current-head live surface.

Goal: operators need one command that tells them whether public/read RPC is
usable.

Deliverable:

- Add or extend a script such as `scripts/testnet-rpc-doctor`.

Required checks:

- Connect to each configured RPC endpoint.
- Validate `status`, `server_info`, `metrics`, `ledger`, `fee`, `validators`,
  `manifests`, `blocks`, `receipts`, `mempool_status`, `bridge_status`,
  `shield_turnstile`, and `orchard_pool_report` when available.
- Report per-method latency, endpoint height, height lag, chain id, protocol
  version, validator count, registry root, and read-only/write-capable posture.
- Fail closed on schema mismatch, oversized response, endpoint timeout, height
  divergence, or key-material leakage.

Acceptance:

- Script has `--endpoint` and `--endpoint-file` modes.
- Script writes a redaction-safe JSON report under
  `reports/testnet-rpc-doctor/`.
- Local or live smoke passes and is linked from the status doc.

## P0 - Validator Doctor

Status: v0 done for local one/all-validator data-dir checks and live
five-validator SSH fleet checks.

Goal: validators should be operable without reading source.

Deliverable:

- Add or extend a script such as `scripts/testnet-validator-doctor`.

Required checks:

- Binary version/checksum.
- Systemd service active state for validator and RPC units.
- Config/data-dir existence and permissions.
- Validator id, public key, active registry membership, registry root, and
  expected quorum.
- Current height, last committed block id, latest certificate id, and lag
  against peers.
- History retention mode, disk usage, archive role, and prune-journal state.
- Recent service logs with private material redacted.

Acceptance:

- One-validator and all-validator modes exist.
- Report is redaction-safe and machine-readable.
- A failing validator produces a short actionable reason, not just raw logs.

## P1 - Monitor Snapshot

Status: v0 done for RPC/height/read-only/mempool/Orchard public counters,
with local smoke and live five-endpoint monitor snapshot passing.

Goal: get useful monitoring before building a full dashboard.

Deliverable:

- Add or extend a one-shot monitor script such as
  `scripts/testnet-monitor-snapshot`.

Metrics to record:

- Per-endpoint RPC health and latency.
- Height lag and convergence.
- Quorum availability and stopped/unreachable validator count.
- Disk usage, history retention posture, and archive-window availability.
- Mempool pending counts.
- Block production/finality timing if recent blocks are available.
- Orchard public counters: output count, nullifier count, root count,
  turnstile deposit total, withdraw total, fee burn total.

Acceptance:

- Emits JSON suitable for cron collection.
- Defines warn/critical thresholds in the report.
- Does not require private keys.

## P1 - Python RPC Client V0

Status: v0 done for read methods and indexed transparent `account_tx`, with
local and live endpoint smokes passing. Incremental ancestor-tip catch-up and
disk-backed per-account read shards have landed for the retained-history
index; aggregate JSON compaction remains the scale follow-up.

Goal: build the start of a Python equivalent to the Rust RPC SDK, useful for
buy-side/integration users who expect Python tooling.

Deliverable:

- Create a minimal Python package or module under a stable path such as
  `python/postfiat_rpc/`.
- Add a smoke script under `scripts/`, for example
  `scripts/testnet-python-rpc-client-smoke`.

Required client methods:

- `status()`
- `server_info()`
- `ledger(limit=None)`
- `fee()`
- `validators()`
- `manifests()`
- `metrics()`
- `blocks(limit=None)`
- `receipts(tx_id=None, limit=None)`
- `tx(tx_id, audit_block_log=False)`
- `account(address)`
- `mempool_status()`
- `bridge_status()`
- `orchard_pool_report()`

Historical transaction support:

- Implement `account_tx(address, from_height=None, to_height=None, limit=None)`.
  Current state uses the server-side bounded `account_tx` read when available
  and falls back to a bounded client-side scan for pre-upgrade endpoints.
- The current server method uses an auto-refreshed retained-history index when
  current, appends missing blocks when the cache tip is a known ancestor of the
  local tip, prefers disk-backed per-account shards for reads, and falls back
  to aggregate index / bounded retained-history scan when the disk index is
  absent or stale.

Safety requirements:

- Timeout on every request.
- Request/response byte caps.
- Deterministic JSON request ids or caller-supplied ids.
- No private key handling in v0.
- Redact key-like fields in logs and reports.

Acceptance:

- `python3 -m py_compile` passes for all new Python files.
- Smoke can hit a local or live read RPC endpoint and produce a JSON report.
- Smoke demonstrates `account_tx` over at least one account with a finalized
  transparent funding row returned through the read-only RPC edge.
- Docs include a short usage example.

## P1 - RPC Method Inventory And Gaps

Status: v0 done; one SDK gap found and fixed. Auto-refreshed indexed
`account_tx` exists now, including incremental ancestor-tip catch-up and
disk-backed per-account read shards. Aggregate JSON compaction remains the
scale follow-up.

Goal: make the public RPC surface legible.

Tasks:

- [x] Generate a method inventory from the Rust SDK constants and node RPC
  dispatch.
- [x] Mark each method as read-only, controlled-write, privacy-alpha gated, or
  operator-only.
- [x] Identify missing XRP-like methods needed for users: account history,
  transaction lookup, ledger range, fee, server info, validators, manifests.
- [x] Decide account-history posture: auto-refreshed retained-history index
  exists now; incremental ancestor-tip catch-up exists for normal commit-time
  refresh; disk-backed read shards exist; aggregate JSON compaction remains the
  scale follow-up.

Acceptance:

- Add/update a doc under `docs/runbooks/` or `docs/specs/`.
- The inventory matches code, not aspiration.

## P2 - Operator UX Polish

Status: v0 done as `docs/runbooks/operator-day-two.md`.

Goal: reduce human error for validator operators.

Tasks:

- [x] Add a one-page "operator day two" runbook covering start, stop, restart,
  status, logs, RPC health, disk pressure, key rotation, and what not to post
  publicly.
- [x] Add copy-paste commands for validator doctor, RPC doctor, monitor
  snapshot, and evidence-pack generation.
- [x] Add a checklist for onboarding an independent operator without exposing
  private material.

Acceptance:

- Runbook references real scripts and real report paths.
- It distinguishes validator RPC, read-only public RPC, and controlled write
  edge.

## Stop Conditions

Stop implementation and write a handoff note if:

- Live access is unavailable and local tooling cannot proceed.
- A validator state divergence appears.
- A script would require private key exposure in a report.
- A test failure indicates consensus/state behavior changed.
- The repo becomes dirty with unrelated changes.

## Recommended Overnight Order

1. Run/check existing evidence scripts and capture a fresh status snapshot.
2. Implement RPC doctor if it does not already exist.
3. Implement validator doctor or monitor snapshot.
4. Start Python RPC client v0 only after P0 evidence/doctor work is green.
5. Update `docs/ai-handoff.md` with what landed, what failed, and exact report
   paths.
