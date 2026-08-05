# A666 Recovery and Live-Demo Handoff

- **Prepared:** 2026-08-05 UTC
- **Worktree:** `/home/postfiat/repos/a666-eth-fast-lane-combined-20260724`
- **Branch:** `feature/pnok-private-fix`
- **HEAD at handoff:** `02c53c38e705424ec823f2243f190fbd48f0d050`
- **Reason:** the supervising Nazgûl pane failed; the principal ordered all Orc work stopped and requested one comprehensive continuation document.
- **Status:** evidence-backed handoff only. It authorizes no service change, credential action, transaction, live-chain mutation, or fund movement.
- **Controlling tracker:** `docs/plans/A666-RECOVERY-EXECUTION-TRACKER-20260804.md`
- **Prior summary:** `docs/plans/A666-REMAINING-WORK-SUMMARY-20260805.md`
- **Evidence root:** `docs/evidence/a666-public-reserve-product-20260803/`

## 1. Read this first

The project has proven considerably more than the tracker’s current R4 score suggests, but the highest fully closed recovery gate remains **R3**. R2 is fully closed and CI-backed. R4 construction reached an 18/18 fire-control state, the browser officially completed display steps 1 through 4, and the bridge proof/return machinery was exercised in rehearsal. R4 still has no complete official journey pass.

A separate principal-ordered live-money attempt was started after the rehearsal work. It performed read-only balance, accounting, fleet, profile, and code-path work. **It moved zero live funds and submitted zero live transactions.** The guards stopped the fire because the current live runner was retired, profile identity handling is inconsistent, the active services carry a stale profile, the configured route is a651 rather than the deployed literal A666 mainnet route, and the requested full out-and-back loop is absent from the current StakeHub CLI.

Do not infer progress from untracked runtime files or old process state. The source of truth is committed evidence. A checked tracker box without an evidence path in the same commit is invalid.

## 2. Immediate safety state

### 2.1 StakeHub is a permanent product

The principal’s standing words are recorded in the tracker:

> “Stakehub is a product i dont want it deleted or destroyed -- i just want it decoupled from NAVCoin calculation.”

Binding meaning:

- Never delete, destroy, uninstall, decommission, defund, empty, or strip StakeHub.
- Never alter its funds, balances, keys, software, services, configuration, or data except under an explicit scoped authorization.
- R10 means removing StakeHub as NAVCoin/NAV calculation, reserve-proof, or public-runtime authority. It does not mean removing StakeHub itself.
- StakeHub must remain available for live risk management.
- Rehearsal-only unit stops require a preserved inventory, minimum downtime, and an inventory-matched restart proof.
- Live operations keep all five StakeHub services running unless the principal explicitly orders otherwise.

### 2.2 “Live funds” definition

For this campaign, **LIVE FUNDS** means real funds in the StakeHub wallet on Ethereum mainnet and their route-bound Arbitrum/PFTL representations. It never means Sepolia ETH, faucet assets, testnet tokens, rehearsal fixtures, Anvil balances, fork balances, or any test asset.

Never use Sepolia, Anvil, a local fork, `local_l4`, or a constructed-proof shortcut as evidence of a live loop.

### 2.3 Money-path mechanics

Every irreversible step requires:

1. a separate HELD packet;
2. exact before-state arithmetic;
3. a current spend-ledger and cap assertion;
4. one invocation;
5. exact receipts and after-state arithmetic;
6. STOP-no-retry on any deviation;
7. a committed evidence increment before the next leg.

Credential values never appear in commands, chat, logs, diffs, or evidence. Refer to a credential only by file path or PFTerminal vault label, and fetch it at use time.

Use module invocation, never `python -c`:

```bash
/usr/bin/python3 -m stakehub.cli ...
```

Before using the interpreter, verify the recorded interpreter identity. The 2026-08-05 read-only snapshot used:

- path: `/usr/bin/python3`
- SHA-256: `1643dacd9feaedc58f3cc581e4d22577dfe25c09b10282936186ccf0f2e61118`

### 2.4 Repository safety

This is a protected, heavily dirty worktree.

- Current exact count at handoff: **2 tracked modified files and 9,078 individual untracked entries**.
- Tracked modifications:
  - `docs/evidence/a666-public-reserve-product-20260803/browser/r4-construction/private-swap-dependencies.json`
  - `docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/environment-manifest.json`
- Never run `git add .`.
- Never run `git clean`, `git reset --hard`, or `git checkout --`.
- Never revert a file merely to make the worktree clean.
- Add only exact owned paths.
- Treat all existing untracked deployment, evidence, profile, and runtime paths as protected until individually classified.

## 3. Current runtime snapshot

The following is a read-only host snapshot taken while preparing this handoff. PIDs are volatile and must be refreshed before use.

| Unit | State | Main PID |
|---|---|---:|
| `stakehub-private-swap-dashboard.service` | active | 3002164 |
| `stakehub-private-swap-asset-orchard.service` | active | 1895104 |
| `stakehub-private-swap-wallet-agent.service` | active | 1895107 |
| `stakehub-pfusdc-wallet-agent.service` | active | 1895112 |
| `stakehub-private-swap-dashboard-bfinal.service` | active | 2193828 |

Known listeners at handoff:

| Loopback port | Observed owner | Meaning |
|---:|---|---|
| 8787 | Python PID 3002164 | canonical dashboard |
| 8788 | Python PID 2193828 | bfinal dashboard |
| 18792 | Asset-Orchard PID 1895104 | restored stale-profile orchard |
| 18793 | no listener | current-profile warm prover is absent |
| 8080 | Node PID 2185052 | wallet proxy owner differs from the earlier staged PID |

The five services are active, but active does not mean live-demo-ready:

- bfinal carries stale profile identity `91aa6c9d…`;
- current canonical profile identity is `bfb3d043…`;
- current raw profile-file SHA-256 is `8c7a2d6b…`;
- the warm prover required at port 18793 is absent;
- the restored orchard mirror was last evidenced at height 598;
- a later uncommitted read-only probe saw the six-validator fleet at height 836;
- port 8080 is owned by a different proxy process than the accepted staging evidence.

Do not restart, rebind, or terminate any of these processes from this handoff alone.

### 3.1 Agent state

The principal ordered all Orc work stopped before this handoff was written.

- Snaga: completed/interrupted; no handoff-turn mutation.
- Ghash: completed/interrupted; no handoff-turn mutation.
- Krimp: completed/interrupted; no handoff-turn mutation.
- No Orc tool process remains assigned.
- The supervising Nazgûl pane errored and is not a valid continuation authority.

## 4. Gate scoreboard

The tracker contains 40 unchecked boxes, eight actual principal decisions, three decided decisions, and five still-open principal decisions. The remaining-work coverage table has 45 source rows and no uncovered tracker source line.

| Gate | State | Honest status |
|---|---|---|
| R0 | PASSED | Truth freeze complete |
| R1 | PASSED | Fast loop, controlled reports, signed checkpoint/import complete |
| R2 | PASSED, 12/12 | AR-01 through AR-11, manifest, and CI closed |
| R3 | PASSED | Four lifecycle passes, including three consecutive repeatable passes |
| R4 | OPEN | Tracker shows 2/5; DEFECT-13 and full journey evidence remain open |
| R5 | OPEN, 0/2 | Cold qualification not started |
| R6 | OPEN, 0/3 | Reproducible signed release not produced |
| R7 | OPEN, 0/6 | Clean public reproduction remains blocked |
| R8 | FROZEN | 6 open items; key-rotation decision is recorded |
| R9 | FROZEN | 7 open items and three canary decisions |
| R10 | FROZEN | 4 open items; restoration GO executed |
| Phase 3 | OPEN, 0/8 | Stays-good operations remain open |

Open counts by section:

- R4: 4
- R5: 2
- R6: 3
- R7: 6
- R8: 6
- R9: 7
- R10: 4
- Phase 3: 8

The tracker footer is stale. It says the next action is a first R4 journey pass. Reality has progressed through multiple official attempts, product fixes, trustline work, and a stopped live-money discovery campaign. Update the tracker only when the successor decides the actual route and sequence, and preserve the same-commit evidence rule.

## 5. R0 through R3: verified closed

### 5.1 R0 and R1

Closed capabilities include:

- six-validator baseline inventory;
- repository/runtime/private-swap/snapshot state capture;
- computed controlled reports on success and failure;
- mandatory controlled-report path;
- disabled proving-chained qualification runner;
- signed lifecycle checkpoint and fail-closed importer;
- seven positive/adversarial import vectors;
- cold test split and checkpoint repack;
- real pre-migration checkpoint extraction at height 792.

### 5.2 R2 regression closure

R2 closed at accepted campaign commit `a3c3de8`. The tracker records 12/12 boxes.

Important artifacts:

- `scripts/check-a666-recovery-regression-manifest`
- `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest.json`
- `docs/evidence/a666-public-reserve-product-20260803/regressions/regression-manifest-ci.txt`
- `.github/workflows/rust-ci.yml`

Important CI/manifest commits:

- `6884268` - initial validator and CI integration
- `d677e14` - complete-manifest enforcement
- `13ae124` - final R2 CI normalization/closure lineage
- `34cfa1a` - route-live wallet regression collection

The manifest maps all observed defect classes and AR-01 through AR-11 to exact tests, runtimes, first-passing commits, and evidence. The false-green class is explicitly recorded: an invocation that matches zero tests is a defect even when it exits zero.

Working execution-test filter:

```bash
cargo test -p postfiat-execution unit_tests
```

Any exact invocation must prove at least one matched test.

R2 validator:

```bash
scripts/check-a666-recovery-regression-manifest --run
```

Known proving-budget debt:

- `wan_devnet_invalid_asset_orchard_swap_proof_is_rejected_and_valid_swap_still_applies`
- observed at more than 31 minutes inside the node-lib suite;
- candidate for a proving feature gate;
- out of the campaign’s fast qualification loop.

### 5.3 R3 repeatability

- Run 4: first full lifecycle pass, about 16.7 minutes.
- Runs 5, 6, and 7: three consecutive `ok:true` runs on the same commit and binary.
- Evidence: `r3-repeatability-gate.json`, commit `5bfe466`.

## 6. R4 browser recovery: what is built

### 6.1 Wallet and browser capabilities

Implemented and requalified:

- step-9 reload/reconnect/recovery through the production wallet recovery module;
- durable pending-state survival across proxy restart and Chromium reload;
- public receipt download;
- redacted recovery serialization preserving receipt identity and final balance tuple;
- custody scans for seed, mnemonic, private/owner key, and spend authorization material;
- route-live filtering;
- provider-neutral reserve-proof display;
- request-ID-bound proof frames;
- same-origin proxy and wallet serving;
- build-identity validation from the pinned Git object rather than mutable checkout bytes;
- official-input contract extraction with strict 1:1 runner accessor coverage;
- mode-symmetric official/construction validation;
- DOM selectors that expose exactly one market container;
- fresh-wallet Add-asset trustline UI.

Key wallet/candidate lineage:

- `39f7fae` - accepted successor candidate for step-9/receipt work
- `5849cda` - successor requalification evidence
- `e1b71a1`, `23b9bff` - market DOM identity fixes
- `0a9e552` - filter non-live route rows
- `293abbc` - request-ID-bound reserve-proof choreography
- `e73690c` - public reserve-proof-status proxy allowlist
- `2d14aa4` - step-9 route fixture
- `1bc25ac` - holder-side Add-asset product surface

The current repository HEAD is later than every qualified wallet candidate above. Do not call HEAD a qualified release without rerunning all required suites and served-artifact verification.

### 6.2 R4 construction/proof work

Construction reached:

- checkpoint-restored six-validator fleet;
- governed route activation;
- candidate proxy and private dependencies;
- real SP1 proof generation;
- real verifier deployment and acceptance;
- certified destination consume;
- `ReturnBurned` receipt proof;
- Ethereum checkpoint observation;
- six-vote certificate;
- one certified return import;
- six-way equality and conservation in rehearsal;
- 18/18 fire-control readiness.

Important evidence/commits:

- `ee23d58` - fresh checkpoint fleet/lifecycle
- `254042f` - governed rehearsal route activation
- `10acdb4` - private dependency staging lineage
- `f54645e` - certified return terminal evidence
- `af466bc` - v6 fire-control 18/18 ready

R4 used loopback Anvil for qualification. Separate mainnet scripts contain chain-ID-1 execution paths. Never report the Anvil evidence as a live mainnet transaction.

### 6.3 Official journey history

| Attempt | Deepest outcome |
|---|---|
| early refusal windows | invocation count 0; preconditions or launch contract refused before an official run |
| v2 | failed step 1 |
| v3 | failed step 1 |
| v4 | failed step 1 |
| v5 | steps 1-2 passed; failed step 3 |
| v6 | steps 1-4 passed; failed step 5; zero receipts |

The display plane is proven through step 4. No official journey reached a successful mutation step.

v6 failed because the fresh generated self-custody wallet lacked the asset/trustline/funding state needed to enable `Mint 1 A666`. That became tracker DEFECT-13.

### 6.4 DEFECT-13 state

The product now has a holder-side Add-asset flow that reuses the existing quote/sign/submit custody machinery. Issuer authority was deliberately excluded from the browser.

Approved separation:

- wallet user creates holder-signed pfUSDC/A666-class trustlines;
- pfUSDC auto-authorization is product behavior;
- issuer authorization, if required, is a direct certified operator action;
- no issuer key or operator surface enters the self-custody wallet.

Rehearsal work reached finalized trust-set receipts on the loopback fleet. The full fresh-wallet funding arithmetic and step-5 readiness were not closed before the principal redirected the campaign to live money.

Tracker wording still says pfUSDC/A666. Later live-demo evidence identifies the current StakeHub profile route as literal `a651`. Preserve this distinction rather than silently renaming either side.

## 7. Principal decisions and constraints

### 7.1 Decided

1. **Key rotation:** no rotation.
   - Evidence: `docs/plans/A666-DECISION-KEY-ROTATION-20260805.md`
   - Principal words: “no rotation. i am the only employee now.”
2. **StakeHub restoration:** GO executed.
   - Commit: `688b67a`
   - Evidence: `docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/stakehub-restoration-20260805.json`
   - Result: five of five services restored.
3. **Staffing:** single operator plus automation accepted risk.
   - Evidence: `docs/plans/A666-DECISION-STAFFING-20260805.md`
   - Principal confirms each live step as the compensating control.

### 7.2 Tracker decisions still open

1. demo/investor date after R7;
2. R8 preflight report-hash confirmation;
3. transparent canary approval;
4. private canary approval;
5. Ethereum export/return canary approval.

### 7.3 New decisions exposed after the tracker summary

These are not yet represented as tracker checkboxes and must be recorded before resuming execution:

1. **Route choice**
   - Use the current StakeHub devnet-2 `a651` route and build/deploy a matching mainnet bridge binding; or
   - use the already deployed literal A666 route `pftl-a666-ethereum-wA666-usdc-v1`.
   - Recommendation: use the deployed literal A666 route because the principal’s requested asset is A666 and the production export/return machinery already binds it.
2. **Runner choice**
   - rebuild the full live runner;
   - run a narrower loop;
   - stop with evidence.
   - The Nazgûl presented these as options A/B/C. No principal answer was recorded before the Nazgûl pane failed.
3. **Spend-cap choice**
   - the code-derived full-loop floor is 10.000000 USDC and fits the current cap;
   - the legacy profile amount is 30 USDC and exceeds the cap by 1.024845 USDC;
   - never raise or bypass the cap without the principal’s explicit recorded decision.
4. **Sequencing override**
   - the principal temporarily ordered an immediate live loop before R4-R7 closure;
   - no live money moved;
   - a successor must confirm whether that override remains in force or whether execution returns to the tracker’s R4-R9 order.
5. **Per-step live packet hashes**
   - even with a broad GO, each irreversible leg needs a current preflight report and HELD packet.

## 8. Live-money attempt: exact state

### 8.1 Accounting snapshot

Evidence:

- `docs/reports/A666-STAKEHUB-PNL-REPORT-20260805.md`
- `docs/evidence/a666-public-reserve-product-20260803/live-demo/stakehub-pnl-snapshot-v1.json`
- commit `5ddd418`

Captured at 2026-08-05T14:45:37Z:

| Metric | USD |
|---|---:|
| Gross assets | 43,338.56 |
| Aave liabilities | 201.01 |
| Net equity | 43,137.55 |
| Hyperliquid unrealized PnL | +2,877.40 |
| Journaled realized funding | +485.78 |

Included components:

- Hyperliquid perpetuals;
- Hyperliquid spot;
- SOL liquid/staked/deactivating stake;
- NEAR liquid and staked balance;
- Aave collateral;
- Aave debt;
- mainnet/Arbitrum/Base cash;
- native XMR reference;
- pfUSDC/A666-class bridge inventory class;
- funding journal.

**Total PnL is not computed.** Current net equity is a marked valuation, not since-inception PnL. Missing sources are historical cost basis, complete trading-realized PnL, a complete fee ledger, and the Aave principal/accrued-interest split.

The pfUSDC/A666-class bridge inventory was not read because the profile guard refused. Do not assume it is zero.

### 8.2 Read-only balances and cap

Evidence:

- `docs/evidence/a666-public-reserve-product-20260803/live-demo/live-balances-sizing-nav-red.json`
- commit `f146cb8`

| Chain | Asset | Balance |
|---|---|---:|
| Ethereum mainnet | USDC | 84.161443 |
| Ethereum mainnet | ETH | 0.289781906358769172 |
| Arbitrum | USDC | 0.000000 |
| Arbitrum | ETH | 0.005556809103608 |

Spend ledger:

- spent: 501.024845 USDC
- cap: 530 USDC
- headroom: 28.975155 USDC
- ceremonies: 70

The code-derived smallest full-loop size is **10,000,000 atoms = 10.000000 USDC**.

Arithmetic:

```text
501.024845 + 10.000000 = 511.024845
530.000000 - 511.024845 = 18.975155
```

The legacy runner reads a 30-USDC profile amount:

```text
501.024845 + 30.000000 = 531.024845
```

That exceeds the cap by 1.024845 USDC. A legacy 30-USDC fire is forbidden without an explicit cap decision.

### 8.3 NAV status

The StakeHub `nav` read returned 10,341.69 USD. That is aggregate portfolio NAV, not route NAV.

A later uncommitted read-only probe reportedly found:

- fleet 6/6 converged at height 836;
- proof epoch 8 finalized;
- proof submitted at height 813;
- observation heights 776-784;
- `nav_per_unit = 89871706`;
- proof freshness 23/900 blocks.

Those data live only in scratch paths and were never reviewed or committed. They are not a final NAV deliverable. Refresh and commit a new route-bound NAV report before any issue/redeem leg.

### 8.4 Money-movement verdict

- transactions: 0
- signing actions: 0
- ledger mutations: 0
- balance mutations: 0
- live-chain mutations: 0
- StakeHub fund mutations: 0
- live receipts: 0

The Leg-1 packet is HELD, never fired:

- commit: `4210e1c`
- path: `docs/evidence/a666-public-reserve-product-20260803/live-demo/leg1-mainnet-cctp-pftl-held.json`
- proposed substep A: Ethereum-mainnet USDC to Arbitrum through CCTP;
- proposed substep B: Arbitrum USDC to 10,000,000 pfUSDC atoms;
- do not execute both a restored canonical runner and this packet without proving a funded/recovered skip, or the money could be bridged twice.

## 9. Profile/service identity blocker

Evidence:

- `02c53c3`
- `docs/evidence/a666-public-reserve-product-20260803/live-demo/profile-identity-hash-contract-contradiction.json`
- `d786cba`
- `docs/evidence/a666-public-reserve-product-20260803/live-demo/bfinal-current-profile-corrective-after.json`

Three hashes exist:

| Meaning | Hash prefix |
|---|---|
| current raw profile-file SHA-256 | `8c7a2d6b…` |
| current canonical parsed-JSON product identity | `bfb3d043…` |
| stale active service identity from an older profile | `91aa6c9d…` |

Code semantics:

- `wallet_demo.py:733-740` and `wallet_demo_state.py:36-40` compute canonical JSON identity.
- `wallet demo up` injects the canonical identity.
- `wallet_demo.py:343,1759-1768` and `live_swap_preflight.py:242` record raw file hashes.
- `service_identity.py` performs an algorithm-agnostic equality comparison.

The bug is both semantic and naming-related: raw artifact hash and canonical product identity are different values but are both surfaced as “profile SHA-256” in different paths.

Required fix shape:

1. add failing tests that prove whitespace/key-order changes do not change product identity;
2. make service identity comparisons use canonical JSON identity;
3. name raw bytes separately, for example `profile_file_sha256`;
4. make evidence schemas carry both named values when both are required;
5. requalify all preflight/up/units/workflow/verify code paths.

Do not solve this by changing an expected literal until tests define the hash contract.

### 9.1 Failed corrective choreography

A bfinal-only attempt changed `91aa…` to `bfb3…` successfully. Full preflight then failed because the required warm prover at `127.0.0.1:18793` was absent. The packet rolled bfinal back to `91aa…`.

A later canonical `wallet demo up` attempt warmed the prover successfully after about 324 seconds, but the launcher then rejected the still-stale bfinal dashboard and cleaned its controlled process group.

The required choreography, after the hash contract is fixed and tested, is atomic:

1. identify the exact profile-carrying set from code;
2. move that controlled set to the current canonical identity without running the rollback-triggering full preflight;
3. start the canonical warm service with its owned mode-700 directory and built-in `start_new_session`;
4. require port 18793 readiness, binary hash, warm state, and identity;
5. run the full read-only preflight exactly once;
6. roll back the entire controlled set on any deviation.

Wallet-agent units are outside the service-identity carrying set and remain untouched.

## 10. Live runner and route blockers

### 10.1 Retired runner

Evidence:

- commit `8935f31`
- `docs/evidence/a666-public-reserve-product-20260803/live-demo/canonical-live-runner-mapping-fire-plan-red.json`

Commit `6c41a4a` retired the CLI registration and dispatch for:

```text
wallet demo run --live
```

The underlying orchestration functions remain in `stakehub/wallet_demo.py`, but the current CLI deliberately rejects the legacy command. The current:

```text
wallet demo workflow --action execute
```

is not equivalent and declares `live_authority=false`.

Restoring parser/dispatch alone is unsafe. The old transparent leg uses the legacy dashboard roundtrip, not the governed receipt-count -> subscription-allocation -> `nav_mint_at_nav` sequence now required.

### 10.2 Governed issuance gap

Evidence:

- commit `7bb8478`
- `docs/evidence/a666-public-reserve-product-20260803/live-demo/leg2-pfusdc-to-a651-held.json`

Required governed sequence:

1. reserve receipt submit;
2. count;
3. subscription allocation;
4. `nav_mint_at_nav`;
5. reserve submit;
6. epoch finalize.

The current default profile has no matching atomic-swap authority object and no current StakeHub CLI/builder/path-only signer surface for this exact sequence.

Rejected substitutes:

- the dual-wallet transparent swap;
- issuer-only `get-navcoin`;
- any flow that mints a NAV asset without consuming the corresponding pfUSDC;
- any flow that bypasses certified reserve and epoch transitions.

### 10.3 a651 versus literal A666

Evidence:

- commit `8a84ada`
- `docs/evidence/a666-public-reserve-product-20260803/live-demo/route-identity-a651-not-a666.json`

The current StakeHub devnet-2 profile uses literal NAV asset `a651`. Receipts must record that literal identity and never relabel it A666.

Separate production machinery exists for the deployed literal A666 route:

- route: `pftl-a666-ethereum-wA666-usdc-v1`
- native asset: a different literal asset beginning `521c…`
- its own controller, wrapped token, verifier, remote layout, and proving infrastructure.

The two routes are not interchangeable.

### 10.4 Production export/return reuse

Production-capable pieces already exist:

| Capability | Commit / implementation |
|---|---|
| wallet-signed export debit | `wallet-proxy/navswap-transparent.js` |
| durable export/return workers | `41b1ce4` |
| mainnet export + Groth16 + accept/mint | `016c735` |
| certified destination consume | export pipeline |
| constrained mainnet return burn | `b19ce4c` |
| receipt proof + checkpoint + six-vote certificate | return pipeline |
| certified return import | `pftl_uniswap_return_import` |
| rehearsal certification evidence | `f54645e` |

Evidence for the current a651 gap:

- commit `2b2f0b1`
- `docs/evidence/a666-public-reserve-product-20260803/live-demo/leg3-a651-mainnet-export-red.json`

The reusable mainnet scripts bind the literal A666 route, not a651. Integration estimates from read-only review:

- deployed literal A666 route: about 80-120 lines of StakeHub profile/config/service wiring;
- a651 route: about 200-300 lines plus mainnet deployment/binding and tests.

Recommendation: choose the deployed literal A666 route unless the principal explicitly wants a new a651 mainnet deployment.

## 11. R5 through R10 and Phase 3

### R5

Open:

- unchanged-commit prerequisites;
- one genesis-to-tip six-validator cold qualification with outage, catch-up, restart, replay, and rollback.

Any R5 failure returns to the fast loop and requires a new AR regression before retry.

### R6

Open:

- reproducible release build;
- signed binary hashes;
- strict CI on the exact revision;
- signed and archived release manifest.

### R7

Open blockers:

- repository-relative or archived guest ELF;
- source-commit versus candidate-revision reconciliation;
- circulating versus outstanding supply reconciliation;
- stranger-runnable fresh-clone reproduction script;
- archived clean-clone transcript/report;
- principal demo-date decision.

Verifier identity debt:

- stale historical `0x004e44…`;
- candidate/proof `0x003af4…`;
- proof acceptance in rehearsal does not close public reproduction.

### R8

Frozen, with no-rotation decision recorded. Still needs:

- terminated-staff credential inventory;
- signed live recovery snapshot;
- independent verification;
- all-six live convergence;
- route pause and signer separation;
- rollback rehearsal;
- principal confirmation of the final preflight report hash.

### R9

Frozen. Each live step needs prior-step evidence and principal confirmation:

- signed release deployment;
- successor/profile binding;
- bounded admission;
- transparent canary;
- private canary;
- Ethereum canary;
- full lifecycle and conservation report.

The emergency live-money order departed from this sequence, but no transaction fired. Confirm whether that override survives before resuming.

### R10

Frozen. Deliverable is StakeHub decoupling from NAV/public proof authority while StakeHub remains fully operational as a product.

### Phase 3

All eight stays-good items remain open, including nightlies, two-week green streak, monthly recovery drill, operator runbooks, and an external stranger transcript.

## 12. Evidence ledger

| Commit | Artifact or fact | Verdict |
|---|---|---|
| `a3c3de8` | R2 closed 12/12 | GREEN |
| `6884268`, `d677e14`, `13ae124` | regression validator/CI | GREEN |
| `5849cda` | successor requalification | GREEN at that candidate |
| `ee23d58` | checkpoint-restored fleet | GREEN rehearsal |
| `254042f` | governed route activation | GREEN rehearsal |
| `f54645e` | certified return terminal | GREEN rehearsal |
| `af466bc` | fire control 18/18 | GREEN for v6 construction |
| `767b71a` | v6 official failure | RED at step 5 |
| `b92ca97` | mandatory StakeHub restart after v6 | GREEN teardown |
| `688b67a` | StakeHub restoration | GREEN 5/5 |
| `49410b0` | no-key-rotation accepted risk | DECIDED |
| `398af78` | staffing accepted risk | DECIDED |
| `5ddd418` | StakeHub PnL report v1 | GREEN read-only, bridge inventory unavailable |
| `f146cb8` | live balance/sizing preflight | RED profile mismatch |
| `d786cba` | bfinal correction | RED, rolled back |
| `02c53c3` | profile hash contract split | RED defect record |
| `4210e1c` | 10-USDC Leg-1 packet | HELD, never fired |
| `7bb8478` | governed a651 issuance packet | HELD/RED missing surface |
| `2b2f0b1` | a651 mainnet export | RED missing binding |
| `8935f31` | canonical live-runner mapping | RED retired/incomplete runner |
| `8a84ada` | route identity | DECISION INPUT |

## 13. Uncommitted and scratch state

Do not treat these as source-of-truth evidence.

Reported scratch from the interrupted read-only Leg0 probe:

- `/tmp/leg0c-fleet-probe.json`
- `/tmp/leg0c-env-manifest.json`
- `/home/postfiat/repos/.ghash-scratch/leg0c_reprobe.py`
- `/home/postfiat/repos/.ghash-scratch/leg0c-fleet-fresh.json`
- `/home/postfiat/repos/.ghash-scratch/leg0c-proof-fresh.json`

Before reading or committing any scratch artifact:

1. verify it belongs to this campaign;
2. scan it without displaying credential-class values;
3. confirm its command source and time;
4. compare it with current machine truth;
5. write a new reviewed evidence artifact rather than promoting raw scratch blindly.

Two tracked evidence files are modified. Preserve them until their owner and provenance are classified.

## 14. Credential and incident discipline

The campaign recorded an overbroad-filesystem-search defect class after tool output exposed credential-class values. Binding correction:

- never search bare `/tmp` or `$HOME`;
- search only named subdirectories;
- exclude key/PEM/credential-class paths outside a repository;
- any tool output containing a credential value invalidates the increment;
- report locations and classes, never values;
- signature material can be public, but still avoid dumping transcripts;
- a missing fact in committed evidence is an evidence defect, not authority to sweep the filesystem.

No rotation was required for the recorded synthetic fixture because hash comparison found no match to persistent keys. That conclusion does not authorize future exposures.

## 15. Recommended continuation plan

### Milestone A: freeze and reconcile control truth

1. Commit or discard no existing dirty file without owner review.
2. Add the new route/runner/cap/sequencing decisions to the tracker.
3. Refresh read-only five-service status and fleet convergence.
4. Refresh the spend ledger and mainnet/Arbitrum balances.
5. Produce a new route-bound NAV report with committed command evidence.
6. Fix the profile hash contract with RED-first tests.
7. Requalify the profile/preflight test suite.
8. Atomically stage the canonical bfinal identity and warm prover.
9. Run one full read-only preflight.

Acceptance:

- canonical and raw profile hashes have distinct schema fields;
- service identity is canonical everywhere;
- warm prover 18793 is ready;
- five StakeHub services remain active;
- fleet is 6/6 converged;
- no live transaction or signing action occurred.

### Milestone B: choose and build the route

Recommended path:

1. principal selects the deployed literal A666 route;
2. wire its existing controller/wrapped/verifier/export/return configuration into StakeHub;
3. restore a supported live runner or explicitly compose the audited steps;
4. add `--amount-atoms 10000000` with exact cap enforcement;
5. implement the governed pfUSDC subscription/allocation/`nav_mint_at_nav` sequence;
6. integrate the existing export and return workers;
7. test every guard and STOP path.

Alternative:

- deploy and bind a new mainnet a651 route;
- keep every receipt literal;
- perform the larger 200-300-line integration plus deployment verification.

Never relabel a651 as A666.

### Milestone C: requalify

At minimum:

```bash
cd /home/postfiat/repos/StakeHub-repeat-demo
/usr/bin/python3 -m pytest tests/test_wallet_demo.py tests/test_wallet_demo_verify.py
/usr/bin/python3 -m stakehub.cli wallet demo run --help
git diff --check
```

Then in the A666 recovery worktree:

```bash
scripts/check-a666-recovery-regression-manifest --run
```

Wallet qualification:

```bash
cd wallet-web
npm test
npm run test:custody-browser
npm run test:public-browser
npm run build
```

Also run the construction browser preflight with the exact committed environment contract. Do not synthesize endpoint/path bindings from memory.

Verify the served artifact, not merely source files.

### Milestone D: close R4 rehearsal

1. finish DEFECT-13 operator-side authorization and fresh-wallet funding;
2. prove step-5 readiness in construction;
3. requalify the exact candidate;
4. official pass #1, one shot;
5. immediate StakeHub restart if a rehearsal window stopped units;
6. review receipts/custody evidence;
7. official pass #2, fresh run.

Only then mark R4 closed.

### Milestone E: live loop

Before any live leg:

1. principal confirms route, cap, runner, sequencing override, and packet hash;
2. current NAV/proof/fleet/profile/service/cap checks are green;
3. all irreversible legs have separate HELD packets;
4. no prior receipt makes the next operation a replay or double bridge.

Per-leg order:

1. mainnet USDC -> Arbitrum -> pfUSDC;
2. governed pfUSDC -> literal selected NAV asset at certified NAV;
3. PFTL export -> Ethereum wrapped/Uniswap asset;
4. Ethereum return burn -> certified PFTL import;
5. NAV redeem -> pfUSDC -> external USDC;
6. final atom-exact conservation and loop-PnL report;
7. PnL v2 with the now-readable bridge inventory.

Commit each leg before starting the next.

## 16. First 30 minutes for the successor

Read-only only:

```bash
cd /home/postfiat/repos/a666-eth-fast-lane-combined-20260724
git rev-parse HEAD
git branch --show-current
git status --short --untracked-files=no
```

Read these documents in order:

1. this handoff;
2. `docs/plans/A666-RECOVERY-EXECUTION-TRACKER-20260804.md`;
3. `docs/plans/A666-REMAINING-WORK-SUMMARY-20260805.md`;
4. `docs/reports/A666-STAKEHUB-PNL-REPORT-20260805.md`;
5. `docs/evidence/a666-public-reserve-product-20260803/live-demo/profile-identity-hash-contract-contradiction.json`;
6. `docs/evidence/a666-public-reserve-product-20260803/live-demo/canonical-live-runner-mapping-fire-plan-red.json`;
7. `docs/evidence/a666-public-reserve-product-20260803/live-demo/leg1-mainnet-cctp-pftl-held.json`;
8. `docs/evidence/a666-public-reserve-product-20260803/live-demo/leg2-pfusdc-to-a651-held.json`;
9. `docs/evidence/a666-public-reserve-product-20260803/live-demo/leg3-a651-mainnet-export-red.json`.

Read-only service check:

```bash
for unit in \
  stakehub-private-swap-dashboard.service \
  stakehub-private-swap-asset-orchard.service \
  stakehub-private-swap-wallet-agent.service \
  stakehub-pfusdc-wallet-agent.service \
  stakehub-private-swap-dashboard-bfinal.service
do
  systemctl --user is-active "$unit"
  systemctl --user show "$unit" -p MainPID --value
done
```

Then obtain the principal’s unresolved route/runner/cap/sequencing decisions. Do not fire Leg 1 merely because its packet exists.

## 17. Stop conditions

Stop immediately on any of the following:

- active service/profile identity differs from the reviewed packet;
- warm service or fleet is unavailable;
- any validator differs in height/tip/root;
- route identity is ambiguous;
- literal asset/route/contract differs from the packet;
- cap projection exceeds the configured cap;
- existing receipt indicates a replay or already-completed substep;
- endpoint is Sepolia, testnet, Anvil, fork, or `local_l4`;
- proof/VK/program identity differs;
- NAV, supply, packet, overlay, or profile is stale or mismatched;
- credential value appears in output;
- exact before/after arithmetic fails;
- a live command has no committed RED-first regression and reviewed HELD packet;
- the principal decision or packet-hash confirmation is missing.

## 18. Bottom line

- R0-R3 are closed.
- R4 remains open after reaching official step 4.
- R5-R7 remain open.
- R8-R10 remain frozen.
- StakeHub is restored and active 5/5.
- The current service profile is stale for the new demo profile.
- The canonical live runner is retired.
- The current a651 route lacks the requested production mainnet binding.
- The deployed literal A666 route has reusable production export/return machinery.
- The 10-USDC code floor fits the cap; the legacy 30-USDC profile amount does not.
- The PnL v1 report is committed, but total PnL and bridge inventory remain incomplete.
- **No live funds moved.**
- **No live transaction was submitted.**
- **All Orc work was stopped before this handoff was finalized.**
