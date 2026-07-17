# NAV Roundtrip Performance Improvement And Test Plan

Status: Phase 1 runner/verifier implemented; fast fleet-preflight reuse enforced; Phase 2 default compression-class gate hardened; warm launch-session setup/cleanup is outside protocol clock; allowance setup and smoke commands generated; smoke fleet preflight refreshed; live approval and benchmark execution pending
Date: 2026-06-21
Owner: protocol runner / operator
Scope: live small-dollar a651 <-> pfUSDC roundtrip on the WAN devnet with Arbitrum USDC custody

Related:

- `docs/runbooks/nav-roundtrip-speedup-plan.md`
- `docs/runbooks/nav-roundtrip-process-improvement-plan.md`
- `docs/runbooks/wan-devnet-full-live-end-to-end-run.md`
- `docs/runbooks/wan-devnet-structural-fix.md`
- `docs/specs/nav-roundtrip-evm-withdrawal-consolidation.md`

## What End To End Means

For this benchmark, "end to end" means the full economic path, not a local
simulation and not a partial PFTL-only run:

1. USDC leaves the StakeHub Arbitrum wallet and enters `ERC20BridgeVault`.
2. The Arbitrum deposit is relayed to PFTL and mints pfUSDC.
3. pfUSDC is subscribed into real a651 through the primary NAV mint path.
4. a651 verified net assets increase by the expected USDC value.
5. a651 exits back into pfUSDC.
6. a651 verified net assets decrease by the expected USDC value.
7. pfUSDC is burned for redemption.
8. The Arbitrum withdrawal proof/finalize/submit/finalize/claim path returns
   USDC to the StakeHub wallet.
9. The PFTL redemption settlement closes.
10. Wallet/vault USDC balances, pfUSDC queue/counted-vault accounting, NAV
    deltas, mempool state, and validator height/state-root convergence all
    verify.

Anything less can be a useful unit test, but it is not a full roundtrip
performance result.

## Current Baseline

There are two separate measurements:

- Operator elapsed time: the recent live exercise took hours because setup,
  debugging, WAN access, retries, funding checks, manual investigation, and
  documentation were mixed into the run.
- Protocol runtime: the current automated full-roundtrip baseline is roughly
  `120s` when the runner is already configured and the validators/contracts are
  usable.

The performance goal is to reduce both. The claimable performance number must
come only from the protocol-runtime benchmark verifier.

## Target

Phase 1 target, without redeploying Arbitrum contracts:

- ten clean live full roundtrips;
- median protocol runtime under `95s`;
- p90 protocol runtime under `105s`;
- no failed NAV, bridge, custody, queue, or validator-convergence checks.

Phase 2 target, after replay-proven PFTL batching:

- ten clean live full roundtrips;
- median protocol runtime under `75s`;
- stretch target under `60s` only after the largest safe PFTL classes are live;
- proof that every batched operation is replay-equivalent to the sequential
  state transition.

Phase 3 target, only with operator approval for contract/API changes:

- ten clean live full roundtrips;
- median protocol runtime under `55s`;
- stretch target under `45s` only after Phase 2 compression and the consolidated
  bridge path are both proven live;
- fixed-contract summaries proving the new bridge path preserves vault/token,
  recipient, packet, proof, and challenge-window binding.

No phase is considered successful until a strict benchmark verifier accepts the
artifacts.

## Recovered Optimization Map

The lost optimization work is spread across the speedup plan, process plan, WAN
runbooks, and the withdrawal consolidation spec. The coherent plan is four
runtime classes:

| Class | Target | Main lever | Claim boundary |
| --- | ---: | --- | --- |
| Phase 1 full Arbitrum roundtrip | 80-95s | runner/process cleanup only | Same live bridge-in, NAV, bridge-out, and settlement path as the 120s baseline. |
| Phase 2 full Arbitrum roundtrip | 55-75s | replay-proven PFTL same-round batching | No batching without replay corpus closure for the exact candidate classes in the summary. |
| Phase 3 full Arbitrum roundtrip | 35-55s | Arbitrum withdrawal contract/API consolidation | Requires operator-approved redeploy plus fresh bridge battery. |
| Phase 4 warm PFTL-only path | 15-30s | start from existing PFTL custody | Not comparable to a full Arbitrum roundtrip; bridge-out is deferred. |

The current evidence baseline is the all-Vultr five-run live-value benchmark:

```text
$POSTFIAT_STATE/live-e2e-20260621T061254Z/roundtrip-benchmark-five-20260621T121933Z
```

That packet averaged `120.39s` across five clean full Arbitrum roundtrips. The
dominant measured segment was Arbitrum withdrawal at roughly `43.6s`; the next
meaningful blocks are fleet/preflight, EVM deposit, NAV checkpoint rounds, PFTL
relay/certification, withdrawal signature handling, and final verification.

## Recovered Implementation Status

The adjacent docs and command surface show that the first tranche of runner and
benchmark primitives already exists locally:

- `nav-roundtrip-live-demo` is the full orchestration runner and now rejects
  degraded live-value flags such as `--allow-peer-failures` and
  `--defer-certified-sends`.
- The runner emits full-run timing boundaries:
  `timing_scope`, `protocol_clock_started_at_stage`,
  `protocol_clock_stopped_at_stage`, `setup_or_recovery_work_included_in_total`,
  `readiness_preflight_ms`, `stakehub_session_ms`,
  `stakehub_session_close_ms`, and `protocol_clock_ms`.
- Phase 1 hot-path cleanup primitives exist: warm full-run StakeHub launch
  session, benchmark-enforced warm USDC allowance, allowance-skip for warm
  approvals, RPC provider classification, and EVM receipt watcher rows.
- Full-run summaries now charge warm StakeHub launch-session setup to
  readiness timing and report launch-session cleanup separately; neither is
  included in `protocol_clock_ms`, which starts at the first value-moving EVM
  deposit transaction.
- `--fleet-preflight-only`, `--fast-demo-preflight`, `--background-audit`, and
  `--reuse-final-certified-state` exist to move public-fleet polling and
  expanded audit work off the protocol clock while preserving required final
  custody, NAV, mempool, and certified-state checks. `--fast-demo-preflight`
  now requires a precomputed fresh `fleet-preflight.json` instead of falling
  back to public fleet polling inside the timed command.
- `nav-roundtrip-benchmark-base-args`, `nav-roundtrip-benchmark-plan`, and
  `nav-roundtrip-benchmark-verify` exist. The planner emits ten-run batteries
  and strict verifier commands for `phase1`, `phase2`, and `phase3`.
- The verifier records provenance: artifact roots, clean-run definition, git
  commit and dirty flag, binary path, SHA3-384 binary hash, contract addresses,
  challenge windows, provider class, approval status, StakeHub session mode,
  and per-stage timings.
- The Phase 1 verifier now rejects clean-run claims if `preflight` lacks
  USDC allowance evidence, if allowance is below the run amount, or if the
  EVM deposit included a timed USDC approval.
- Phase 2 observability exists: summaries report PFTL certified round count,
  certified operation count, candidate batch classes, replay-equivalence
  requirements, and live-compression blockers.
- Phase 2 dependency and replay primitives exist: certified operations declare
  same-round or prior-round dependencies, dependency metadata is validated
  adversarially, `nav-roundtrip-replay-corpus-verify` fails closed, and replay
  corpus cases exist for the four planned candidate classes.
- Phase 2 benchmark planning and verification now default to the four planned
  compression classes (`vault_bridge_deposit_propose_attest`,
  `vault_bridge_receipt_submit_count`, `nav_subscription_allocate_mint_at_nav`,
  and `nav_redeem_at_nav_settle`) and reject Phase 2 claims if the summaries do
  not actually report every required class.
- `--same-round-nav-exit` exists as the first live runner compression opt-in. It
  derives the redemption id from the signed redeem sequence and refuses a
  caller-supplied `--redemption-id`.
- Phase 3 has an execution gate but not a deployed implementation:
  `nav-roundtrip-benchmark-verify --phase phase3` requires the consolidated
  fixed-contract bridge class and consolidated EVM withdrawal receipt labels.
  The contract/API change remains a design proposal until operator approval.

The practical implication: the next safe step is not to re-design the whole
roundtrip. The Phase 1 manifest is generated and reviewable; the operator must
run the generated setup-only allowance command before any timed value-moving run
because the current read-only `allowance(address,address)` check on
2026-06-21 returned `0` atoms.

Current generated Phase 1 artifacts and planned output paths:

```text
$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-base-args-20260621-warm.json
$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-base-args-20260621-warm.report.json
$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-ten-run-20260621T-warm-plan/phase1-benchmark-plan.json
$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-ten-run-20260621T-warm-plan/allowance-setup/allowance-setup.json
$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-ten-run-20260621T-warm-plan-smoke/roundtrip-summary.json
```

The regenerated Phase 1 plan includes a top-level `allowance_setup_command`.
It warms a bounded USDC allowance of `55906004` atoms, covering one smoke run
plus the ten-run acceptance battery at `5082364` atoms per run. This setup
command is intentionally outside the protocol clock. It has not been run by
Codex because it submits a live ERC20 approval.

The same plan now includes a separate `smoke_run` and `smoke_verifier_command`.
The smoke artifact directory is
`$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-ten-run-20260621T-warm-plan-smoke`,
outside the acceptance `benchmark_dir`, so the strict acceptance verifier only
discovers the ten battery summaries when computing protocol-clock median and
p90.

Latest no-value fleet preflight:

```text
$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-ten-run-20260621T-warm-plan-smoke/fleet-preflight/fleet-preflight.json
```

It passed on 2026-06-21 with all six public validators and the local operator
at height `233` and state root
`edbb41082e6e1052a388cedbe29c77c204968cfb7636b8fcbdb01c7eb570a89633b1be9826708ee089b3606afa837054`.

## Execution Workstreams

### 1. Make The Benchmark Hard To Misreport

The runner and verifier must reject anything that is not a clean full Arbitrum
roundtrip.

Required summary fields:

- `run_class="full-arbitrum-roundtrip"`;
- `completion_status="full_arbitrum_roundtrip_complete"`;
- `setup_or_recovery_work_included_in_total=false`;
- binary hash and git commit;
- artifact root;
- contract addresses and bridge class;
- Arbitrum RPC provider class;
- challenge windows;
- StakeHub signing mode;
- USDC allowance evidence;
- reused fleet-preflight evidence for `--fast-demo-preflight`;
- `approve_skipped=true`;
- per-stage timings, including `stakehub_session_ms` and
  `stakehub_session_close_ms`;
- wallet/vault balance deltas;
- NAV money-in and money-out deltas;
- pfUSDC queue and counted-vault deltas;
- final mempool state;
- final validator height and state-root consensus.

Testing:

- focused runner tests reject partial summaries;
- focused runner tests reject summaries with recovery/setup inside the measured
  protocol window;
- verifier test proves aggregate protocol-clock median/p90 and slowest-stage
  reporting is derived from accepted summaries only.

Acceptance:

- old or incomplete summaries fail the new verifier;
- summaries with a timed USDC approval fail the new verifier;
- new clean summaries include enough data to reproduce the performance claim.

### 2. Remove Avoidable Work From The Hot Path

These changes improve speed without weakening the economics:

- keep one warm StakeHub signing session for deposit and withdrawal instead of
  relaunching the agent repeatedly;
- require sufficient USDC allowance before the timed protocol clock and skip
  approval in the benchmarked EVM deposit stage;
- move fleet topology, public-validator evidence, and contract-address checks
  before the protocol timer;
- require generated timed commands to reuse the precomputed fleet evidence
  rather than silently polling public validators on the hot path;
- at run start, do a freshness recheck instead of a full public audit;
- use a nearby healthy PFTL RPC endpoint for submit/poll loops;
- shorten polling intervals where finality is already observable;
- record provider class and receipt latency for every Arbitrum transaction;
- keep expensive post-run public audits outside the blocking protocol timer,
  while still requiring final custody/NAV/state checks before a run is accepted.

Testing:

- fake-agent test for warm StakeHub session reuse;
- fake-cast test for allowance skip;
- verifier test proving timed approval is rejected as non-clean benchmark
  evidence;
- runner test proving setup/preflight/session time is separate from protocol
  time;
- runner test proving summaries record skipped approval and signing mode;
- runner test proving fast fleet-preflight mode fails if cached evidence is
  missing;
- one live smoke roundtrip before the ten-run battery.

Acceptance:

- the full-run summary shows setup/preflight/session and protocol timing
  separately;
- the timed path contains value-moving protocol work plus required safety
  checks only;
- no raw EVM private keys are used.

### 3. Generate A Repeatable Benchmark Command Set

The live benchmark must be runnable from one explicit base-args file, not from
manual shell reconstruction.

Work:

- generate `BASE_ARGS.json` from a known-good prior full-roundtrip summary;
- require explicit key/signing paths;
- require explicit nonce/session base;
- generate ten deterministic run commands and one strict verifier command;
- write the plan before any value moves.

Testing:

- local test constructs base args from a synthetic full summary;
- local test feeds generated base args into the benchmark planner;
- release binary builds after the planner/verifier changes.

Acceptance:

- the generated command set is deterministic and reviewable before execution;
- if a required signer path is missing, the benchmark is not launched.

### 4. Run The Phase 1 Live Battery

The first claimable speedup attempt is a no-redeploy live battery.

Execution:

1. Run local checks:

   ```text
   cargo fmt --check
   cargo test -p postfiat-node nav_roundtrip -- --nocapture
   cargo test -p postfiat-node certified_asset_ops -- --nocapture
   cargo build -p postfiat-node --release
   ```

2. Run the generated setup-only `allowance_setup_command` outside the benchmark
   clock.
3. Run `smoke_run.fleet_preflight_command`, then `smoke_run.run_command`.
4. Verify the smoke summary with `smoke_verifier_command`.
5. If clean, run ten live full roundtrips from the generated plan.
6. Run the strict benchmark verifier over the ten summaries.
7. Publish the performance report only if the verifier passes.

Failure handling:

- if funds move and the run fails, close recovery separately;
- mark that run excluded with a reason;
- do not count recovery time as protocol runtime;
- do not cite a best run as the result.

Acceptance:

- ten clean live full-roundtrip summaries;
- median under `95s`;
- p90 under `105s`;
- every invariant listed in "What End To End Means" verified.

### 5. If Phase 1 Is Still Too Slow, Batch Only Replay-Proven PFTL Steps

The most likely PFTL bottleneck is sequential certified-operation relay work.
This can be compressed only if the batch is replay-equivalent to the existing
sequential execution.

Candidate batch classes:

- `vault_bridge_deposit_propose_attest`;
- `vault_bridge_receipt_submit_count`;
- `nav_subscription_allocate_mint_at_nav`;
- `nav_redeem_at_nav_settle`.

Rules:

- every batch class must declare dependencies explicitly;
- same-round batching is allowed only when ids, reads, and invariants are
  derivable before apply;
- prior-round dependencies stay separate unless replay proves equivalence;
- adversarial replay fixtures must cover duplicate labels, stale evidence,
  wrong asset, wrong vault, wrong recipient, double settlement, dirty mempool,
  and bad resume.

Testing:

```text
postfiat-node nav-roundtrip-replay-corpus-verify \
  --require-live-compression-ready \
  --require-candidate-classes vault_bridge_deposit_propose_attest,vault_bridge_receipt_submit_count,nav_subscription_allocate_mint_at_nav,nav_redeem_at_nav_settle \
  --strict
```

Acceptance:

- replay corpus green for every enabled batch class;
- ten clean live runs after enabling the batch classes;
- median under `75s`, with under `60s` treated as a stretch result until
  broader compression is proven;
- no NAV, bridge, redemption, or validator-convergence regression.

### 6. If EVM Withdrawal Dominates, Ask For A Contract Decision

If the slowest stage is the Arbitrum withdrawal path, runner tuning will not be
enough. The current path requires multiple EVM transactions and waits.

Work, only after operator approval:

- implement/deploy the fixed or consolidated withdrawal path in
  `docs/specs/nav-roundtrip-evm-withdrawal-consolidation.md`;
- preserve vault/token/recipient/packet binding;
- preserve challenge-window disclosure;
- rerun Foundry bridge tests;
- rerun Rust packet-binding and runner tests;
- run a fresh small-dollar bridge battery against the new addresses.

Acceptance:

- summaries show the new fixed-contract bridge class;
- ten clean live full roundtrips;
- median under `55s`, with under `45s` treated as a stretch result until Phase
  2 plus the consolidated withdrawal path are both proven;
- no weaker custody, replay, recipient-binding, or challenge-window assumption.

## Final Report Format

The final performance report must include:

- artifact root;
- git commit;
- release binary hash;
- run count and excluded-run count;
- clean-run definition;
- validator topology;
- contract addresses;
- bridge contract class;
- challenge windows;
- Arbitrum RPC provider class;
- StakeHub launch-session mode;
- USDC approval status;
- per-stage timing table;
- warm launch-session setup timing;
- protocol-clock median, p90, best, worst, and mean;
- total wall-clock values for audit context;
- slowest three stages;
- wallet USDC before/after deltas;
- vault USDC before/after deltas;
- NAV money-in expected/actual delta;
- NAV money-out expected/actual delta;
- pfUSDC redemption queue before/after;
- pfUSDC counted vault value before/after;
- final mempool state;
- final validator height/state-root convergence;
- every excluded run and why it was excluded.

## Immediate Execution Order

1. Run focused local checks and release build:

   ```text
   cargo fmt --check
   cargo test -p postfiat-node nav_roundtrip -- --nocapture
   cargo test -p postfiat-node certified_asset_ops -- --nocapture
   cargo build -p postfiat-node --release
   ```

2. Confirm the generated base-args file:
   `$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-base-args-20260621-warm.json`.
3. Confirm the generated Phase 1 ten-run manifest:
   `$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-ten-run-20260621T-warm-plan/phase1-benchmark-plan.json`.
4. Review and run the generated setup-only `allowance_setup_command` from the
   plan. The current command is:

   ```text
   $POSTFIAT_REPO/target/release/postfiat-node nav-roundtrip-live-demo --warm-usdc-allowance-only --artifact-dir $POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-ten-run-20260621T-warm-plan/allowance-setup --source-rpc-url https://arb1.arbitrum.io/rpc --source-chain-id 42161 --vault 0x1A15e6103D6Af4e88924F748e13B829D3948DEa9 --verifier 0x70C259Bf8D65AF76DfcB4991ccB26E88b2C2188E --usdc 0xaf88d065e77c8cC2239327C5EDb3A432268e5831 --stakehub-wallet 0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0 --required-allowance-atoms 55906004 --session-id nav-roundtrip-phase1-20260621-allowance-setup --agent-timeout-secs 1200 --overwrite
   ```

   This warms allowance for
   `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` -> `0x1A15e6103D6Af4e88924F748e13B829D3948DEa9`.
   The latest read-only `allowance(address,address)` check on 2026-06-21
   returned `0`, so the generated timed commands will fail closed before moving
   funds until this setup command succeeds.
5. Confirm signer paths, wallet/vault balances, all-Vultr topology, and fleet
   preflight before funds move.
6. Run `smoke_run.fleet_preflight_command`, then `smoke_run.run_command`, then
   `smoke_verifier_command` from the generated plan. The smoke uses artifact
   dir
   `$POSTFIAT_STATE/live-e2e-20260621T061254Z/phase1-ten-run-20260621T-warm-plan-smoke`.
7. If the smoke is clean, run each battery `fleet_preflight_command`, then each
   battery `run_command`, from the generated plan.
8. Verify and publish the report only if the generated strict Phase 1
   `verifier_command` accepts ten clean summaries.
9. If Phase 1 misses target, implement only the largest measured bottleneck:
   PFTL batching if certified PFTL rounds dominate, or EVM withdrawal
   consolidation if the bridge path still dominates.

## Non-Negotiables

- Do not call a local or PFTL-only run a full roundtrip.
- Do not count setup, funding, fleet repair, or recovery as protocol runtime.
- Do not use raw EVM private keys for live funds.
- Do not skip NAV money-in or money-out checks.
- Do not skip wallet/vault custody reconciliation.
- Do not hide challenge-window duration.
- Do not enable same-round PFTL batching without replay evidence.
- Do not report a speedup until the strict benchmark verifier proves it.
