# NAV Round Trip Speedup Plan

Status: implementation plan
Audience: PFTL operator, StakeHub operator, protocol engineer
Date: 2026-06-20
Updated: 2026-06-21

Related operational runbook:

- `docs/runbooks/nav-roundtrip-process-improvement-plan.md` defines the
  manifest discipline, role ownership, stop conditions, post-run review, and
  benchmark reporting standard for live-value roundtrip runs.
- `docs/specs/nav-roundtrip-evm-withdrawal-consolidation.md` defines the Phase
  3 Arbitrum withdrawal consolidation proposal. It is a design artifact only;
  it is not deployed and does not authorize a live contract migration.

## Process Improvement Plan

Objective: turn the live a651 <-> pfUSDC round trip from an operator-driven
ceremony into a repeatable one-command process with stable evidence, explicit
timing attribution, and no hidden trust or custody assumptions.

Current measured baseline:

- Original manual process: roughly two hours, mostly operator sequencing,
  rediscovery, late compatibility checks, and one live accounting fix.
- Current automated WAN benchmark: five clean live-value runs averaging
  `120.39s`.
- Current bottleneck: not human process drag anymore, but sequential PFTL
  certification rounds, Arbitrum receipt/challenge waits, and blocking
  evidence capture.

Target state:

- Phase 1: `80-95s` without redeploying Arbitrum contracts.
- Phase 2: `55-75s` by reducing safe PFTL critical-path rounds.
- Phase 3: `35-55s` if the operator approves fixed-contract redeploys and EVM
  withdrawal consolidation.
- Phase 4: `15-30s` for the warm PFTL-only path where users already hold a651
  or pfUSDC on PFTL.

Workstreams:

1. Instrument the runner.

   Every full run must emit stable per-stage timing fields in
   `roundtrip-summary.json`, including preflight, deposit, relay, primary mint,
   NAV money-in, exit, NAV money-out, burn-to-redeem, withdrawal signature,
   EVM withdrawal, PFTL settle, and final verification.

2. Remove avoidable EVM overhead.

   The runner should reuse a bounded StakeHub launch session, skip USDC approval
   when allowance is already sufficient, and record whether approval was warm or
   included in the timed run.

3. Split safety-critical checks from background evidence capture.

   Blocking preflight should only include checks that must pass before funds
   move. Full audit artifacts should still be produced, but they should not
   extend the user's critical path unless they reveal a state-safety failure.

4. Compress PFTL rounds only where deterministic dependencies permit it.

   Add explicit certified-op dependency declarations. Batch operations only when
   their ids, reads, and invariants are derivable before apply. Prove each batch
   against an unbatched replay fixture before using it live.

5. Keep bridge timing honest.

   Challenge windows stay inside the timer. If controlled-launch windows differ
   from production windows, the runner must print both and the report must not
   imply a public trustless bridge runtime.

6. Preserve evidence quality.

   Each run must still prove wallet/vault USDC deltas, pfUSDC bridge accounting,
   a651 verified-net-assets money-in and money-out deltas, redemption settlement,
   and six-validator height/root convergence.

Acceptance gates:

- Phase 1 passes with ten clean WAN runs, median under `95s`, p90 under `105s`,
  no private-key bypass, and all NAV/bridge invariants verified.
- Phase 2 passes with replay tests for every new batch class, adversarial tests
  for stale/wrong/duplicate/dependency failures, and ten clean WAN runs with
  median under `75s`.
- Phase 3 passes only after Foundry and Rust bridge-binding suites are green,
  fixed Arbitrum contracts are redeployed, and a fresh small-value bridge
  battery passes against the new addresses.
- Phase 4 passes when the PFTL-only user path has separate status reporting and
  cannot be confused with "bridged back to Arbitrum complete."

Operating rule: a speedup is valid only if it reduces waiting, repeated work, or
safe batching overhead. It is not valid if it skips NAV accounting, hides
challenge windows, changes custody assumptions, uses raw EVM private keys, or
weakens final bridge reconciliation.

Implementation status:

- `pftl-submit-certified-asset-ops` implemented in `postfiat-node` as the first
  performance primitive.
- It accepts a JSON list of asset operations, validates labels/key files, writes
  per-operation artifacts, quotes/signs/submits sequentially, rejects dirty
  mempools by default, drains the submitted operations into one mempool batch,
  and can hand that batch to the existing peer-certified finality path.
- Focused tests: `cargo test -p postfiat-node certified_asset_ops -- --nocapture`
  covers prepare/resume, duplicate-label rejection, dependency validation,
  dependency report normalization, prior-round dependency rejection, and
  batch-only quote/sign/submit/batch behavior.
- `pftl-certified-asset-ops-from-bundle` implemented as the adapter between
  existing vault-bridge bundle directories and the certified-ops request format.
  It consumes the generated `*.operation.json` files and writes one request that
  can be submitted through the batch helper.
- `nav-roundtrip-live-demo --preflight-only` implemented as the first runner
  stage. It writes `preflight.json`, checks local PFTL state, Arbitrum USDC/gas
  balances, vault/verifier code presence, challenge-window values, and
  classifies the withdrawal ABI as either controlled-launch old tuple or F-03
  fixed tuple.
- `nav-roundtrip-live-demo --evm-deposit-only` implemented for the real
  Arbitrum approve/deposit step through StakeHub `agentd`. It opens a bounded
  launch session, sends exact-amount USDC approval and vault deposit via
  `evm_contract_tx`, writes calldata and agent response artifacts, closes the
  session, and verifies wallet/vault USDC deltas.
- `nav-roundtrip-live-demo --deposit-relay-only` implemented for the next
  PFTL stage. It consumes the EVM deposit report, builds the existing
  `vault-bridge-deposit-relay-rpc-bundle`, converts it to certified asset ops,
  and submits it through the shared batch helper. Unit coverage uses
  `--prepare-only`; live WAN runs should omit that flag to sign, submit, and
  certify the relay operations.
- `nav-roundtrip-live-demo --primary-mint-only` implemented for pfUSDC-to-a651
  primary issuance. It discovers the counted active pfUSDC receipt, computes the
  required settlement atoms from the live a651 NAV formula, creates the
  deterministic subscription allocation, writes the allocation and
  `nav_mint_at_nav` operations, submits them through certified ops, and records
  before/after vault-bridge status.
- `nav-roundtrip-live-demo --nav-checkpoint-only` implemented for the two NAV
  checkpoint gates. It builds a deterministic `nav_reserve_submit` plus
  `nav_epoch_finalize` from the current ledger. For a651 SP1 profiles it reuses
  the finalized SP1 proof/public-values packet and recomputes the private
  pfUSDC subscription overlay source root; for vault-bridge profiles it uses
  deterministic bucket counted value and source root. It writes submit/finalize
  operations, runs two certified-ops rounds so profile challenge-window blocks
  are not hidden, and in live mode verifies the VNA delta.
- `nav-roundtrip-live-demo --nav-exit-only` implemented for a651-to-pfUSDC
  exit. It consumes the primary-mint report, writes `nav_redeem_at_nav`, derives
  or accepts the redemption id, writes `nav_redeem_settle`, submits both through
  certified ops, and records NAV/pfUSDC balances and settlement status.
- `nav-roundtrip-live-demo --burn-to-redeem-only` implemented for PFTL
  pfUSDC burn-to-source-chain redemption. It consumes the NAV-exit report,
  builds the existing `vault_bridge_burn_to_redeem` bundle, adapts it to
  certified ops, submits it, and records the PFTL redemption id.
- `nav-roundtrip-live-demo --evm-withdrawal-only` implemented for the
  source-chain proof/finalize/withdraw/finalize/claim path through StakeHub
  `agentd`. It detects whether the live vault is the controlled-launch old tuple
  or the fixed F-03 tuple, derives calldata from the deployed contract helper
  methods, requires an explicit signatures file, waits the configured challenge
  windows, sends all EVM transactions through agentd, and verifies wallet/vault
  USDC deltas.
- `nav-roundtrip-live-demo --pftl-settle-only` implemented for closing the PFTL
  redemption after source-chain claim. It derives a deterministic settlement
  receipt hash from the EVM withdrawal report, writes
  `vault_bridge_redeem_settle`, submits through certified ops, and in live mode
  verifies redemption state plus bucket redemption-queue/counted-value deltas.
- `nav-roundtrip-live-demo` without a `--*-only` stage flag is now implemented
  as the end-to-end orchestration runner. It executes the stage sequence,
  derives the expected after-money-in and after-money-out NAV VNA deltas from
  the actual primary mint settlement atoms, validates every stage's own
  `*_ok`/certified-round checks, validates final local status against final
  certified validator state evidence, and writes `roundtrip-summary.json`.
  The summary now explicitly marks
  `run_class="full-arbitrum-roundtrip"`,
  `completion_status="full_arbitrum_roundtrip_complete"`, and
  `custody_location="arbitrum_wallet_usdc"` so dashboards do not infer full
  bridge completion from schema alone.
  If `--withdrawal-signer-key-file` is supplied, it writes the withdrawal
  signature request bundle, signs the verifier proof digest in-process, checks
  the signer is approved by the deployed verifier, and continues without a
  manual pause. If neither verifier signatures nor a signer key are supplied,
  it stops after burn-to-redeem, writes a withdrawal signature request bundle,
  and leaves `roundtrip-failure.json` with the exact resume instruction.
- `nav-roundtrip-live-demo --pftl-only` implemented for the warm in-PFTL user
  path. It starts from existing issued pfUSDC on PFTL, consumes that issued
  settlement asset into the primary a651 mint, verifies the NAV money-in
  checkpoint, exits a651 back to pfUSDC, verifies the NAV money-out checkpoint,
  and writes `pftl-only-summary.json` with
  `run_class="pftl-only"` and
  `completion_status="on_pftl_complete_bridge_out_deferred"`. It deliberately
  does not burn pfUSDC or claim Arbitrum USDC. Instead it writes
  `bridge-out-resume.json` containing the exact next
  `nav-roundtrip-live-demo --burn-to-redeem-only` command. The initial safe
  implementation requires `--subscriber` and `--owner` to be the same account
  so the consumed issued pfUSDC and received NAV remain under one signed owner
  path.
- `nav-roundtrip-dashboard-status --summary PATH [--report PATH]` implemented
  as the dashboard/status adapter. It reads either `roundtrip-summary.json` or
  `pftl-only-summary.json` and emits
  `postfiat-nav-roundtrip-dashboard-status-v1` with explicit booleans for
  `full_arbitrum_roundtrip_complete`, `pftl_only_complete`, and
  `bridge_out_deferred`, plus custody location and bridge-out resume command
  when applicable.
- Focused tests: `cargo test -p postfiat-node nav_roundtrip_preflight --
  --nocapture` covers the preflight stage and old-tuple ABI classification with
  a fake `cast` binary.
- Focused tests: `cargo test -p postfiat-node nav_roundtrip_evm_deposit --
  --nocapture` covers the agentd-backed EVM deposit path with fake agent/fake
  `cast`; `cargo test -p postfiat-node nav_roundtrip_deposit_relay --
  --nocapture` covers relay bundle generation and certified-op normalization.
- Current focused suite: `cargo test -p postfiat-node nav_roundtrip --
  --nocapture` covers 32 runner tests, including the resumed full-run
  summary path, primary mint, withdrawal auto-signing,
  SP1-overlay NAV checkpoint generation, NAV exit, burn-to-redeem, old-ABI EVM
  withdrawal through fake agent/fake `cast`, and PFTL settlement operation
  generation. It also covers the dashboard status distinction, PFTL-only
  run-class guards, PFTL-only benchmark rejection, and the live-ready replay
  corpus cases for primary mint, bridge deposit propose/attest, bridge receipt
  submit/count, and NAV redeem/settle.
- Phase 1 timing instrumentation implemented: full-run
  `roundtrip-summary.json` now emits first-class segment timings for
  fleet-preflight, preflight, EVM deposit, PFTL relay, primary mint, both NAV
  checkpoints, NAV exit, burn-to-redeem, withdrawal signature handling, EVM
  withdrawal, PFTL settle, and final verification.
- Phase 1 timing boundary reporting implemented: full-run and PFTL-only
  summaries now include `timing_scope`,
  `protocol_clock_started_at_stage`, `protocol_clock_stopped_at_stage`,
  `setup_or_recovery_work_included_in_total`, plus
  `readiness_preflight_ms` and `protocol_clock_ms` inside `timings_ms`. The
  benchmark verifier surfaces these fields and rejects summaries missing the
  timing split, so benchmark reports can distinguish blocking readiness checks
  from the value-moving protocol clock.
- Phase 1 clean-run timing gate implemented: `nav-roundtrip-benchmark-verify`
  now requires the full-roundtrip timing boundary fields to be present and to
  match the declared full Arbitrum protocol clock. It rejects summaries with
  `setup_or_recovery_work_included_in_total=true`, so recovery/resume evidence
  cannot be counted as a clean benchmark run.
- Dashboard timing contract implemented: `nav-roundtrip-dashboard-status` now
  surfaces the timing scope, protocol-clock start/stop stages,
  `setup_or_recovery_work_included_in_total`, `benchmark_clean_timing`,
  `readiness_preflight_ms`, `protocol_clock_ms`, the full stage timing table,
  RPC provider class, bridge class, and final audit source. This keeps the UI
  from treating a custody-complete run, a PFTL-only run, and a benchmark-clean
  full roundtrip as the same status.
- Phase 1 allowance-skip implemented for EVM deposit: the runner queries
  USDC `allowance(owner, vault)`, records `allowance_before_atoms` and
  `approve_skipped`, and skips the StakeHub `approve_pfusdc_vault` transaction
  when allowance already covers the run amount while still writing an
  `agent-approve.json` evidence artifact.
- Phase 1 warm StakeHub session mode implemented for full end-to-end runs: the
  full runner opens one bounded launch session with the union wallet, USDC,
  vault, and verifier allowlist, reuses it for EVM deposit and EVM withdrawal,
  writes central open/close artifacts, closes on normal completion or early
  error, and leaves stage-only commands on their existing self-managed session
  behavior.
- Phase 1 preflight critical-path split started: `nav-roundtrip-live-demo
  --fleet-preflight-only` can precompute the expensive public-validator fleet
  evidence into `RUN_DIR/fleet-preflight/fleet-preflight.json`. A full run with
  `--resume` reuses that artifact only if it is green, contains public
  validator evidence, proves operator/public endpoint agreement, and its saved
  local height/state root still match current local status. This lets operators
  move fleet RPC polling before the timed run without accepting stale validator
  evidence.
- Phase 1 final-verification split started: full runs now support
  `--reuse-final-certified-state`, which skips the duplicate final public-RPC
  polling pass and uses the final PFTL certified-round validator-state evidence
  as `final_validator_states`. The summary marks
  `final_validator_state_source="certified_round"`, keeps public RPC as the
  default source, and still requires nonempty final validator evidence,
  height/root consensus, exact match to local final status, empty final mempool,
  and matching certified-round evidence. This moves final audit polling off the
  hot path without weakening the mandatory local/finality checks.
- Phase 1 fast demo/background-audit mode implemented: full runs now expose
  `--fast-demo-preflight` and `--background-audit`. Fast demo preflight allows a
  previously generated `fleet-preflight.json` to be reused after the same
  freshness checks as `--resume`, so public-validator polling can happen before
  the timed run. Background audit uses the final certified-round validator
  evidence on the hot path, records
  `final_audit_profile="background_audit_certified_round_hot_path"`, and writes
  `background-audit/background-audit-request.json` with the post-run public
  validator audit command. The default remains blocking public-RPC final
  verification.
- Phase 1 RPC variance metadata implemented: preflight, EVM deposit, EVM
  withdrawal, and full-run summaries now include `source_rpc_provider_class`
  (`websocket`, `local`, `dedicated_or_gateway_http`,
  `public_or_unknown_http`, or `unknown`). Every benchmark claim records whether
  it used public RPC fallback or a lower-variance provider class.
- Phase 1 EVM receipt watcher evidence implemented: EVM deposit and EVM
  withdrawal reports now include `receipt_watches` rows for each agent-confirmed
  source-chain transaction. Each row records label, tx hash, provider class,
  confirmation source, status, gas used, and elapsed confirmation time. The
  benchmark verifier surfaces deposit/withdrawal watcher counts so run reports
  can distinguish confirmed EVM transaction evidence from bare balance-delta
  checks.
- Phase 2 observability primitive implemented: full-run
  `roundtrip-summary.json` now includes `pftl_certified_round_count`,
  `pftl_certified_operation_count`, and a per-round critical-path table with
  stage, round label, operation count, start/end height, end state root,
  `round_ok`, total time, and certify time.
- Phase 2 dependency declaration primitive implemented: certified-ops request
  entries now support explicit `dependencies` with `same_round` or
  `prior_round` mode. Validation rejects hidden prior-round dependencies inside
  a same request, normalized summaries include a dependency report, and generated
  NAV round-trip requests now mark the deterministic primary-mint and
  receipt-count dependencies plus the prior-round checkpoint, redemption-settle,
  bridge-settle, and deposit-relay stage dependencies. The dependency report now
  separates dependency-only same-round candidacy from live round-compression
  readiness: same-round candidates set `replay_equivalence_required=true` and
  `live_round_compression_ready=false` until replay corpus evidence or an
  explicit operator-approved root-difference gate exists. Full
  `roundtrip-summary.json` reports aggregate `pftl_replay_equivalence_required_count`,
  `pftl_live_round_compression_ready`, and `pftl_live_round_compression_blockers`
  so Phase 2 benchmark scripts can reject unsafe speed claims without traversing
  every nested certified-ops artifact.
- Phase 2 replay-corpus starter implemented for same-round certified asset ops:
  a focused fixture signs the same two asset operations from one source, applies
  them once as two one-operation blocks and once as a single same-round batch,
  verifies both replays, compares ledger-facing asset definitions, and writes a
  machine-readable corpus report. The first result is deliberately conservative:
  the ledger-facing assets match, but the final state roots differ because the
  old path commits two ordered blocks while the same-round path commits one.
  Therefore this class remains `safe_for_live_round_compression=false` until a
  protocol-level proof or explicit root-difference gate is approved.
- Phase 2 replay-corpus verifier implemented:
  `postfiat-node nav-roundtrip-replay-corpus-verify` reads one corpus JSON or
  discovers corpus cases under a directory, validates the schema, checks that
  `state_root_match` agrees with the supplied roots, requires documented
  intended root differences, rejects contradictory "safe" claims, and can run
  with `--require-live-compression-ready --strict` so Phase 2 automation fails
  closed until every corpus case is actually safe for live round compression.
- Phase 2 primary-mint replay corpus implemented for
  `nav_subscription_allocate_mint_at_nav`: the fixture signs the real
  allocation and mint operations, applies them once as two ordered blocks and
  once as a single same-round batch, requires both paths to retire the
  allocation, and verifies ledger-facing accounting equivalence after
  normalizing block-height provenance. The corpus marks
  `ledger_facing_state_match=true` only for that accounting-equivalent
  projection and documents the expected state-root difference caused by
  two-block versus one-block history.
- Phase 2 deposit receipt replay corpus implemented for
  `vault_bridge_receipt_submit_count`: the fixture starts from a ledger where
  the bridge-deposit evidence record is already finalized, signs the real
  receipt submit/count operations, applies them as two ordered blocks and as a
  single same-round batch, and verifies that the receipt is counted and bucket
  counted value is identical after normalizing receipt/bucket block-height
  provenance. This does not collapse proposal/attestation/finalization; it only
  closes the deterministic receipt-submit plus receipt-count class.
  It also accepts `--require-candidate-classes CSV`; use this in Phase 2 gates
  so a corpus cannot pass while silently omitting one of the intended batch
  classes.
- Phase 2 deposit propose/attest replay corpus implemented for
  `vault_bridge_deposit_propose_attest`: the fixture starts from a clean
  bridge-deposit ledger with the pfUSDC asset/profile/NAV asset and registered
  attestor, signs the real propose and attest operations, applies them as two
  ordered blocks and as one same-round batch, and verifies the deposit is
  pending and attested in both paths after normalizing submitted/attested
  block-height provenance.
- Phase 2 NAV redeem/settle replay corpus implemented for
  `nav_redeem_at_nav_settle`: the fixture starts from a post-primary-mint NAV
  exit ledger with a retired pfUSDC subscription allocation, derives the
  redemption id from the signed `nav_redeem_at_nav` sequence before apply, signs
  the real `nav_redeem_settle` against that id, applies redeem/settle as two
  ordered blocks and as one same-round batch, and verifies the owner balances,
  redemption state, settlement receipt hash, bucket accounting, and top-up
  allocation are ledger-facing equivalent after normalizing block-height
  provenance.
- Phase 2 same-round NAV exit runner path implemented behind the explicit
  `nav-roundtrip-live-demo --same-round-nav-exit` opt-in. In this mode the
  runner refuses a caller-supplied `--redemption-id`, quotes the
  `nav_redeem_at_nav` operation to derive the redemption id from the exact
  signed sequence, builds the paired `nav_redeem_settle` operation with a
  `same_round` dependency, and verifies the signed redeem sequence did not
  drift before allowing the combined certified-ops request to proceed. Full-run
  reports collapse the exit stage into one `nav_exit/redeem_settle` PFTL round
  only when this flag is enabled.
- Phase 2 adversarial dependency coverage expanded: certified-ops validation now
  has focused regressions for same-round dependencies declared out of order,
  missing same-round labels, self-dependencies, duplicate dependency labels, and
  unsupported dependency modes. This keeps malformed dependency metadata from
  becoming an accidental live batching path.
- Phase 2 candidate-class reporting implemented: certified-ops dependency
  reports now emit deterministic `candidate_batch_classes` for same-round
  dependency pairs, and full `roundtrip-summary.json` aggregates them as
  `pftl_candidate_batch_classes`. Operators should use that field as the source
  for `--require-candidate-classes CSV` when closing replay evidence.
- Phase 2 staged full-run candidate coverage tightened: full-run summaries now
  exercise staged deposit relay candidates as well as primary mint in the
  regression fixture. A full staged path emits
  `vault_bridge_deposit_propose_attest`,
  `vault_bridge_receipt_submit_count`, and
  `nav_subscription_allocate_mint_at_nav`; the Phase 2 benchmark gate fails if
  the replay corpus covers only the primary mint class.
- Phase 2 benchmark verifier now derives required replay-corpus classes from
  `pftl_candidate_batch_classes` and unions them with any explicit
  `--require-candidate-classes CSV`. This makes the gate fail closed if a
  corpus is live-ready for some other class but omits the same-round candidate
  actually present in the benchmark summary.
- Phase 2 verifier-side replay closure implemented: a raw run summary may still
  report `pftl_replay_equivalence_required_count > 0` and
  `pftl_live_round_compression_ready=false` because the run itself does not
  embed corpus evidence. `nav-roundtrip-benchmark-verify --phase phase2` now
  closes only the exact replay-evidence blockers when the supplied corpus has a
  live-ready case for every summary-derived candidate class. Non-replay
  blockers, missing candidate classes, or a failing corpus still fail the gate.
- Phase 2 benchmark planner implemented:
  `postfiat-node nav-roundtrip-benchmark-plan --phase phase2` now generates the
  same ten-run manifest shape as Phase 1, injects `--same-round-nav-exit` into
  every timed run, uses the Phase 2 `75_000ms` median target by default, and
  refuses to produce a Phase 2 plan unless `--replay-corpus-file` or
  `--replay-corpus-dir` is supplied. The generated verifier command carries the
  replay corpus path plus any explicit `--require-candidate-classes CSV` list
  into `nav-roundtrip-benchmark-verify --phase phase2 --strict`.
- Phase 1/2 benchmark verifier implemented:
  `postfiat-node nav-roundtrip-benchmark-verify` reads one
  `roundtrip-summary.json` or recursively discovers summaries under a benchmark
  artifact directory. It verifies the economic proof fields before counting a
  run as clean: final summary, validator convergence, empty final mempool, EVM
  wallet/vault deltas, NAV money-in and money-out VNA deltas, PFTL redemption
  settlement accounting, certified-round success, complete timing fields, and
  the configured runtime gate. Phase 2 additionally rejects summaries with
  remaining replay-equivalence requirements or live round-compression blockers,
  and now requires `--replay-corpus-file` or `--replay-corpus-dir` evidence that
  passes the live-ready replay corpus gate. Add `--require-candidate-classes
  CSV` with the complete operator-approved batch-class list before treating a
  Phase 2 benchmark as accepted. Use `--strict` for CI/nonzero exit and
  `--report PATH` to save the machine gate result.
- Benchmark standard fields implemented: each verified summary row now carries
  contract addresses, source-chain id, bridge class, Arbitrum RPC provider
  classes, configured preflight challenge windows, actual withdrawal challenge
  waits, approval warm/timed status, StakeHub session mode, background-audit
  mode, final validator evidence source, wallet/vault USDC before/after
  balances, NAV expected/actual VNA deltas, and PFTL redemption queue/counting
  deltas. The verifier fails if core benchmark evidence such as challenge
  windows, provider class, contract addresses, approval status, or session mode
  is absent from a full-roundtrip summary.
- Benchmark provenance implemented: `nav-roundtrip-benchmark-verify` now emits
  `artifact_roots`, a machine-readable clean-run definition, package version,
  current binary path, SHA3-384 binary hash, git commit, git dirty flag, and
  tracked dirty-line count. The binary hash path uses OpenSSL SHA3-384 when
  available and falls back to a streaming Rust SHA3-384 implementation.
- Phase 1 benchmark planner implemented:
  `postfiat-node nav-roundtrip-benchmark-plan` reads a JSON array of
  `nav-roundtrip-live-demo` base arguments and writes an execution manifest for
  the acceptance battery. It emits one fleet-preflight warmup command per run,
  one timed full-run command per run with `--fast-demo-preflight
  --background-audit --reuse-final-certified-state`, unique per-run artifact
  dirs, incremented bytes32 nonces, per-run StakeHub session ids, and the final
  `nav-roundtrip-benchmark-verify --phase phase1 --strict` command. The planner
  rejects static `--signatures-file`, stage-only flags, the `--pftl-only`
  warm-path run class, degraded finality flags, and `--batch-only` so the
  manifest cannot accidentally describe an incomplete, non-comparable, or
  weakened live-value benchmark.
- Phase 3 EVM consolidation design drafted:
  `docs/specs/nav-roundtrip-evm-withdrawal-consolidation.md` specifies the
  additive fixed-contract methods `finalizeProofAndSubmitWithdrawal` and
  `finalizeWithdrawalAndClaim`, the required challenge-window semantics,
  Foundry/Rust regression tests, runner bridge-class detection, and deployment
  gates. This does not modify the current live contract path.
- Phase 3 benchmark verifier gate implemented:
  `postfiat-node nav-roundtrip-benchmark-verify --phase phase3` now uses the
  `55_000ms` median target and rejects summaries unless they report
  `bridge_class="fixed_contracts_redeployed_consolidated"` at the full-run and
  EVM-withdrawal levels, include confirmed withdrawal receipt watcher rows, and
  prove the consolidated receipt labels `submit-proof`,
  `finalize-proof-and-submit-withdrawal`, and
  `finalize-withdrawal-and-claim` instead of the old separate
  `finalize-proof`/`submit-withdrawal`/`finalize-withdrawal`/`claim-withdrawal`
  path.
- Phase 3 benchmark planner implemented:
  `postfiat-node nav-roundtrip-benchmark-plan --phase phase3` now emits the
  same controlled ten-run manifest shape, uses the Phase 3 `55_000ms` median
  target by default, and points the verifier command at
  `nav-roundtrip-benchmark-verify --phase phase3 --strict`. This is an
  acceptance-planning primitive only; it does not approve or perform the
  Arbitrum contract redeploy required for a real Phase 3 claim.
- Focused tests: `cargo test -p postfiat-node nav_roundtrip -- --nocapture`
  covers the warm-session guard, externally managed EVM deposit mode,
  self-managed stage behavior, resume-with-existing-EVM-artifacts behavior,
  allowance skip, reusable fleet-preflight artifacts, strict summary timing
  fields, certified-round final-state reuse, PFTL certified-round critical path
  summary fields, generated dependency reports, benchmark planner/verifier
  gates, and the existing NAV/bridge runner stages.
- Focused tests: `cargo test -p postfiat-node certified_asset_ops -- --nocapture`
  covers certified-op preparation/resume, bundle adaptation, direct signing,
  same-round dependency reporting, prior-round rejection, adversarial dependency
  declaration rejection, batch-only submission, and the conservative replay
  corpus fixture.

## 2026-06-21 Process Improvement Update

The original process goal was to move from a manual two-hour operator sequence
to a sub-10-minute live controlled-launch round trip. That goal has been
surpassed. The latest all-Vultr WAN devnet benchmark ran five consecutive live
a651 <-> pfUSDC round trips with real Arbitrum USDC and all five completed
cleanly.

Evidence packet:

```text
$POSTFIAT_STATE/live-e2e-20260621T061254Z/roundtrip-benchmark-five-20260621T121933Z
```

Benchmark result:

| Run | Wall time | Final height | Result |
| --- | ---: | ---: | --- |
| 1 | 122.16s | 185 | pass |
| 2 | 126.00s | 197 | pass |
| 3 | 117.02s | 209 | pass |
| 4 | 119.03s | 221 | pass |
| 5 | 117.74s | 233 | pass |

Average clean runtime: 120.39 seconds.

All five runs verified:

- six-validator WAN height/root convergence;
- Arbitrum wallet/vault USDC deltas;
- PFTL deposit relay and pfUSDC mint;
- a651 primary mint from pfUSDC;
- NAV after-money-in verified net assets delta of `+508236400`;
- a651 exit back to pfUSDC;
- NAV after-money-out verified net assets delta of `-508236400`;
- pfUSDC burn-to-redeem;
- Arbitrum proof/finalize/withdraw/finalize/claim;
- PFTL redemption settlement;
- final bridge accounting.

The current bottleneck is therefore no longer manual process drag. It is
sequential orchestration latency across PFTL certified rounds, Arbitrum receipt
waits, short challenge windows, and blocking audit checks.

### Current 120-Second Decomposition

Average timing across the five clean runs:

| Segment | Average |
| --- | ---: |
| Fleet/preflight checks | 13.4s |
| Arbitrum USDC deposit | 12.1s |
| PFTL deposit relay | 9.1s |
| Primary a651 mint from pfUSDC | 3.0s |
| NAV money-in checkpoint | 8.5s |
| a651 exit to pfUSDC | 4.7s |
| NAV money-out checkpoint | 9.1s |
| pfUSDC burn-to-redeem | 2.4s |
| Withdrawal signature packet | 7.6s |
| Arbitrum withdrawal proof/finalize/claim | 43.6s |
| PFTL redemption settle | 2.8s |
| Final summary and verification | 4.1s |
| Total | 120.4s |

The largest block is the Arbitrum withdrawal path. It currently performs:

```text
submitProof
wait verifier challenge window
finalizeProof
submitWithdrawal
wait vault challenge window
finalizeWithdrawal
claimWithdrawal
```

The runner is also doing conservative blocking work that should remain
available for evidence capture but does not all need to sit on the user's
critical path.

## Process Improvement Plan: 120 Seconds to Sub-Minute

The next target is not just "make it faster." The target is to keep the same
economic proof while moving work out of the critical path, reducing redundant
round trips, and keeping security-sensitive waits explicit.

### Target Runtime Classes

| Class | Runtime target | Scope |
| --- | ---: | --- |
| Current clean baseline | 117-126s | Proven five-run WAN benchmark. |
| Phase 1: no redeploy | 80-95s | Runner/process changes only. |
| Phase 2: PFTL batching and orchestration | 55-75s | No EVM contract changes, but better PFTL critical path. |
| Phase 3: EVM withdrawal consolidation | 35-55s | Requires Arbitrum contract changes or redeploy. |
| Phase 4: warm in-PFTL path | 15-30s | Funds already bridged; no Arbitrum bridge-in/out in the user path. |

These are controlled-launch engineering targets. Public production challenge
windows must be reported separately and added to wall-clock runtime.

### Phase 1: No-Redeploy Runner Improvements

Target: 80-95 seconds.

This phase does not change consensus rules or deployed Arbitrum contracts.

Work items:

1. Keep the StakeHub EVM launch session warm across deposit and withdrawal.

   Current process opens and closes sessions around EVM stages. The runner
   should open one bounded session at the start of the timed run, reuse it for
   all EVM calls, and close it in a `finally` path.

   Expected savings: 2-5 seconds.

2. Avoid repeated USDC approval where allowance is already sufficient.

   Preflight should query allowance and skip `approve` if the vault already has
   enough allowance for the exact run amount. If allowance is insufficient, the
   runner should either approve exact amount or fail before timing starts,
   depending on operator mode.

   Expected savings: 3-5 seconds when allowance is warm.

3. Split preflight into blocking and asynchronous tiers.

   Blocking preflight should include only checks needed before funds move:
   validator consensus, wallet balance, gas balance, contract code, vault ABI,
   asset ids, and expected NAV/bridge deltas. Non-critical evidence capture
   should continue in the background and attach to the final artifact.

   Expected perceived savings: 5-10 seconds.

4. Split final verification into user-facing completion and background audit.

   The user-visible run is complete after Arbitrum claim, PFTL settle, local
   delta checks, and one six-validator convergence check. Full deep replay,
   large status dumps, and expanded evidence scans should be written as
   follow-up artifacts without extending the hot timer.

   Expected perceived savings: 3-5 seconds.

5. Replace public-RPC polling loops with a receipt watcher abstraction.

   Use a single receipt watcher for Arbitrum transactions with bounded polling,
   backoff, and per-tx timing. Prefer a dedicated RPC or WebSocket provider when
   available. Public RPC remains acceptable fallback, but the artifact must
   record which provider class was used.

   Expected savings: 2-5 seconds and lower variance.

Phase 1 acceptance:

- 10 clean WAN runs;
- median runtime under 95 seconds;
- p90 runtime under 105 seconds;
- every run still includes configured challenge waits;
- every run still verifies NAV in/out deltas and final bridge accounting;
- no raw EVM private key usage.

### Phase 2: PFTL Batching and Critical-Path Compression

Target: 55-75 seconds.

This phase reduces the number of sequential certified PFTL rounds without
changing the economic proof.

Work items:

1. Batch same-block operations where deterministic dependencies permit it.

   Candidate batches:

   - deposit receipt submit + receipt count;
   - NAV subscription allocation + `nav_mint_at_nav`;
   - NAV redeem request + NAV redeem settle, if the redemption id is
     deterministic from the signed payload;
   - reserve submit + finalize only when the profile's freshness and challenge
     rules permit same-block finalization.

2. Add explicit dependency declarations to the certified-ops helper.

   The helper should understand when operation B reads an id generated by
   operation A. If the id is derivable before apply, batch. If not, keep
   separate rounds. No implicit best-effort batching.

3. Produce a per-round critical path artifact.

   Each run should report:

   - number of PFTL certified rounds;
   - operation count per round;
   - certification time per round;
   - height delta per stage;
   - state root after each round.

4. Add a regression corpus comparing batched and unbatched state roots.

   For each candidate batch, run the old sequence and the new batch against a
   controlled fixture and assert equivalent final state or explicitly document
   the intended state-root difference.

Phase 2 acceptance:

- no consensus shortcuts;
- local replay tests for each batched operation class;
- `nav-roundtrip-replay-corpus-verify --require-live-compression-ready
  --require-candidate-classes CSV --strict` passes for the full intended
  batch-class list;
- adversarial tests for duplicate, stale, wrong asset, wrong vault, wrong
  recipient, and dependency-order failures;
- 10 clean WAN runs with median under 75 seconds;
- no regression in accounting invariants.

### Phase 3: EVM Withdrawal Consolidation

Target: 35-55 seconds.

This phase requires contract changes or redeployment. It should not be mixed
into the current controlled-launch contract path without an explicit operator
decision.

Work items:

1. Add combined verifier/vault methods.

   Candidate methods:

   ```solidity
   finalizeProofAndSubmitWithdrawal(...)
   finalizeWithdrawalAndClaim(...)
   ```

   These preserve the same challenge windows but reduce transaction count and
   receipt waits.

2. Parameterize challenge windows by deployment class.

   The runner must print:

   ```text
   verifier_challenge_wait_secs
   vault_challenge_wait_secs
   bridge_class
   contract_addresses
   ```

   Devnet/demo can use short windows. Production cannot inherit short windows
   by accident.

3. Re-run the Arbitrum contract security battery after redeploy.

   Required regression coverage:

   - unauthorized challenge cannot freeze a valid withdrawal;
   - proof replay across vaults fails;
   - withdrawal replay across token/vault domains fails;
   - expired accepted withdrawal recovery works;
   - recipient substitution fails;
   - double claim fails.

4. Re-run the full bridge battery with small real value.

   The current live proof is valid for the controlled-launch contracts. A new
   EVM deployment needs a fresh small-dollar proof before it is used in demos.

Phase 3 acceptance:

- Foundry suite green;
- Rust bridge packet binding tests green if packet encoding changes;
- one small-dollar Arbitrum bridge-in/bridge-out battery green;
- 10 clean WAN round trips with median under 55 seconds;
- artifact clearly distinguishes fixed redeployed contracts from old
  controlled-launch contracts.

### Phase 4: Warm In-PFTL User Flow

Target: 15-30 seconds for users already holding pfUSDC or a651 on PFTL.

The full 120-second benchmark includes bridge-in and bridge-out. Many real user
actions will not need both sides. Once liquidity is already on PFTL, the hot
path is:

```text
pfUSDC -> a651 primary mint
NAV money-in checkpoint
a651 -> pfUSDC exit
NAV money-out checkpoint
```

For trading or OTC settlement, the product should expose this shorter path
separately from the full bridge round trip.

Work items:

- `nav-roundtrip-live-demo --pftl-only` starts from existing PFTL pfUSDC
  balances, exits back to pfUSDC, and writes separate PFTL-only status;
- report NAV deltas and PFTL custody without Arbitrum receipt waits;
- use the generated `bridge-out-resume.json` when the operator later wants to
  burn pfUSDC and resume the full source-chain withdrawal path;
- feed dashboards with `nav-roundtrip-dashboard-status --summary PATH`, which
  distinguishes "on-PFTL complete" from "bridged back to Arbitrum complete" in
  one normalized report.

Phase 4 acceptance:

- no user-visible ambiguity about custody location: summaries must show
  `completion_status="on_pftl_complete_bridge_out_deferred"` until the Arbitrum
  bridge-out runs;
- PFTL-only runs prove the same NAV accounting invariants through the
  money-in and money-out NAV checkpoints;
- bridge-out can be resumed later from `bridge-out-resume.json`;
- warm-path timing must never be reported as a full Arbitrum roundtrip timing.

## Operational Rules for Future Benchmarks

Every performance claim must include:

- artifact root;
- number of runs;
- clean-run definition;
- validator topology and final consensus check;
- bridge class and contract addresses;
- configured challenge windows;
- Arbitrum RPC provider class;
- whether USDC approval was warm or included;
- whether StakeHub session setup was included;
- per-segment timing table;
- NAV money-in and money-out deltas;
- final wallet/vault and PFTL bucket accounting deltas.

Never report only a single best run. Use at least five runs for exploratory
claims and ten runs for acceptance gates.

## Next Engineering Actions

1. Run the Phase 1 benchmark profile.

   First write a base-args file containing the `nav-roundtrip-live-demo`
   arguments for one complete unattended run. The file must be a JSON array, or
   an object with an `args` array, and must use `--withdrawal-signer-key-file`
   rather than `--signatures-file`.

   Generate the battery plan:

   ```text
   postfiat-node nav-roundtrip-benchmark-plan \
     --base-args-file BASE_ARGS.json \
     --benchmark-dir ARTIFACT_ROOT/phase1-ten-run \
     --run-count 10 \
     --report ARTIFACT_ROOT/phase1-ten-run/phase1-benchmark-plan.json
   ```

   Then run every `fleet_preflight_command` before the timed section, run every
   `run_command`, and finish with the emitted `verifier_command`. The generated
   timed commands include `--fast-demo-preflight --background-audit
   --reuse-final-certified-state`; the default manual live command remains
   conservative. The fast profile still fails before money moves if any
   safety-critical check fails, and it leaves a background audit request next to
   each summary.

2. Continue Phase 2 replay closure for certified-ops dependency declarations.

   Use `postfiat-node nav-roundtrip-replay-corpus-verify
   --require-live-compression-ready --require-candidate-classes CSV --strict`
   for corpus-only checks, or include `--replay-corpus-dir`/
   `--replay-corpus-file` plus `--require-candidate-classes CSV` in
   `nav-roundtrip-benchmark-verify --phase phase2`. The CSV must be the full
   intended batch-class list from `roundtrip-summary.json`
   `pftl_candidate_batch_classes` or the underlying certified-ops dependency
   reports. The current same-round asset-op fixture is valid evidence but
   intentionally not live compression ready because its state root differs from
   the two-block replay.

3. Implement the EVM consolidation contract change only after operator approval.

   The separate design is now in
   `docs/specs/nav-roundtrip-evm-withdrawal-consolidation.md`. Keep this out of
   the current live contract path until the operator approves redeploy and fresh
   bridge testing. After implementation, use
   `postfiat-node nav-roundtrip-benchmark-verify --phase phase3 --strict` so an
   old or partially consolidated bridge path cannot count as a Phase 3
   benchmark.

This runbook plans the work needed to turn the live a651 <-> pfUSDC round trip
from a manual two-hour operator sequence into a sub-10-minute, resumable
one-command controlled-launch demo with bounded, auditable state transitions.

The target flow is live-value only. It does not use a local no-value chain.
The end-state target is under 10 minutes. Longer runtimes below are interim
milestones only, not acceptable final targets.

The claim boundary is strict: sub-10 minutes is a controlled-launch demo metric,
not a production trustless bridge metric. A production bridge run requires the
fixed Arbitrum contracts, a production challenge-window policy, and a fresh
bridge battery against those addresses. If the production challenge windows are
longer than the controlled-launch windows, the extra wait time must be added to
the wall-clock target.

## Executive Summary

The plan has three moves:

1. automate the current sequence without weakening safety, so the operator is
   not hand-driving 15 ceremonies;
2. precompute every NAV and custody delta before funds move, so failures happen
   before the Arbitrum deposit;
3. collapse PFTL-only transitions into deterministic bundles, because the
   current number of certified PFTL rounds cannot reliably fit under 10 minutes.

The current Arbitrum contracts proved the MVP path, but the code-review fixes
for challenge scoping, packet domain binding, and expired-withdrawal recovery
are only in source as of commit `59af43d9`. A public trustless claim requires
redeploying those fixed contracts and re-running the bridge battery against the
new addresses. Until then, the speed target is valid only as a controlled-launch
operator demo.

The 10-minute target matters because the round trip is an operator proof of
economic completeness. A two-hour run leaves too much time for stale NAV inputs,
manual state rediscovery, unlock/session drift, and human error. A sub-10-minute
run is short enough to be repeated as a demo, regression check, and preflight
for larger-value bridge work.

## Contract-Fix Identifiers

The Arbitrum review findings referenced by this plan are:

| Finding | Fix status | Meaning |
| --- | --- | --- |
| F-01/F-02 | Fixed in source commit `59af43d9` | Challenge scoping prevents zero-cost permanent withdrawal freeze. |
| F-03 | Fixed in source commit `59af43d9` | Withdrawal packets bind vault address and token, preventing cross-vault replay. |
| F-04 | Fixed in source commit `59af43d9` | Accepted expired withdrawals have a recovery/claim path. |

The current live Arbitrum demo contracts are not assumed to include those fixes.
The runner therefore reports one of two bridge classes:

```text
bridge_class = controlled_launch_existing_contracts
bridge_class = fixed_contracts_redeployed
```

Only `fixed_contracts_redeployed` supports a public trustless bridge claim.

## Timing Classes

The document uses three different timing classes. They must not be blurred:

- Manual baseline: the observed two-hour June 20 operator run.
- Controlled-launch target: live Arbitrum USDC and live WAN PFTL, with the
  currently configured short challenge windows counted in wall-clock runtime.
  This is the sub-10-minute target.
- Public trustless bridge target: fixed Arbitrum contracts redeployed, proof
  and challenge path re-verified, and production challenge-window policy
  chosen. If the production challenge windows are materially longer than the
  controlled-launch windows, this class will not be sub-10 without a different
  finality design.

The phrase "do not hide challenge windows" means the runner starts its timer
before the proof/withdrawal challenge waits and stops only after claim,
settlement, and final verification. It does not mean every security policy must
use a long public-mainnet challenge duration during the controlled-launch demo.

## Goal

The fast path must prove the full economic round trip:

1. send real Arbitrum USDC into `ERC20BridgeVault`;
2. relay the deposit to PFTL and mint pfUSDC;
3. subscribe pfUSDC into real a651 as primary NAV issuance;
4. finalize NAV after money-in and prove verified net assets rose;
5. redeem a651 back into pfUSDC;
6. finalize NAV after money-out and prove verified net assets returned;
7. burn pfUSDC to redeem;
8. relay and claim USDC back on Arbitrum;
9. settle the PFTL redemption and prove bridge accounting matches custody.

The demo is not complete until both NAV checkpoints and the final Arbitrum
wallet/vault deltas are verified.

## Before: 2 Hours

The June 20 live run took roughly two hours because each state transition was
hand-driven, several compatibility checks were discovered late, and one real
accounting bug required a code fix plus a WAN validator roll.

### Existing Manual Steps

1. Preflight live WAN state.

   - Query all six WAN validators.
   - Confirm height/root consensus.
   - Confirm a651 asset id, pfUSDC asset id, issuer, buyer account, and vault
     bucket.
   - Confirm StakeHub agentd is unlocked.

   Observed cost: 5-10 minutes.

2. Check Arbitrum balances.

   - Query StakeHub wallet USDC balance.
   - Query vault USDC balance.
   - Confirm gas ETH is present.

   Observed cost: 2-5 minutes.

3. Deposit real USDC into the Arbitrum vault.

   - Open a bounded StakeHub launch/signing session.
   - Approve USDC to the vault.
   - Call `ERC20BridgeVault.deposit`.
   - Read the deposit event and record deposit id, nonce, block, log index,
     wallet delta, and vault delta.

   Observed cost: 5-10 minutes.

4. Build and submit the PFTL deposit relay.

   Existing relay stages:

   - `vault_bridge_deposit_propose`;
   - `vault_bridge_deposit_attest`;
   - `vault_bridge_deposit_finalize`;
   - `vault_bridge_receipt_submit`;
   - `vault_bridge_receipt_count`.

   Each stage was quoted, signed, submitted, certified, applied, and checked.

   Observed cost: 15-25 minutes.

5. Allocate pfUSDC into a651 and mint.

   Existing stages:

   - `nav_subscription_allocate`;
   - `nav_mint_at_nav`.

   Observed cost: 5-10 minutes.

6. Verify NAV after money-in.

   Existing stages:

   - construct reserve packet with the pfUSDC cash leg;
   - `nav_reserve_submit`;
   - `nav_epoch_finalize`;
   - verify a651 verified net assets and NAV/unit.

   Observed cost: 10-15 minutes.

7. Redeem a651 back to pfUSDC.

   Existing stages:

   - `nav_redeem_at_nav`;
   - inspect redemption claim and reserve packet hash;
   - `nav_redeem_settle`.

   Observed cost before bug: 5-10 minutes.

8. Fix live redemption-settle capacity bug.

   The live run exposed that `nav_redeem_settle` could require a small top-up
   from another counted receipt in the same bucket. The code only looked at the
   original subscription receipt. The fix allowed deterministic top-ups from
   counted bucket receipts, with regression coverage.

   Extra cost in the live run: 30-45 minutes.

9. Build, test, and roll the consensus fix.

   Existing checks:

   - focused execution test;
   - `cargo fmt --check`;
   - full `postfiat-execution` tests;
   - focused node vault-bridge tests;
   - release build;
   - one-at-a-time six-validator WAN roll with height/root checks.

   Extra cost in the live run: 20-30 minutes.

10. Finalize NAV after money-out.

    Existing stages:

    - `nav_reserve_submit` for the post-exit reserve baseline;
    - `nav_epoch_finalize`;
    - verify verified net assets returned to the pre-money-in baseline.

    Observed cost: 5-10 minutes.

11. Burn returned pfUSDC for Arbitrum withdrawal.

    Existing stages:

    - build `vault_bridge_burn_to_redeem` bundle;
    - quote, sign, submit, certify, apply;
    - verify buyer pfUSDC balance is zero;
    - extract pending redemption id and withdrawal packet.

    Observed cost: 5-10 minutes.

12. Resolve live Arbitrum ABI compatibility.

    The source tree had the F-03 fixed v2 vault packet binding, but the live
    Arbitrum vault was intentionally not redeployed. The live contract accepted
    the controlled-launch old tuple:

    ```text
    submitWithdrawal((uint64,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes),bytes)
    ```

    The runner had to derive the live ABI digest rather than blindly using the
    current source ABI.

    Observed cost: 10-20 minutes.

13. Relay and claim withdrawal on Arbitrum.

    Existing stages:

    - sign verifier proof digest with the configured verifier signer;
    - submit proof to `PFTLWithdrawalVerifier`;
    - wait challenge delay;
    - finalize proof;
    - submit withdrawal to `ERC20BridgeVault`;
    - wait challenge delay;
    - finalize withdrawal;
    - claim withdrawal;
    - verify wallet/vault USDC deltas.

    Observed cost: 10-15 minutes.

14. Settle the PFTL redemption.

    Existing stages:

    - derive settlement receipt hash from the Arbitrum claim;
    - build `vault_bridge_redeem_settle`;
    - quote, sign, submit, certify, apply;
    - verify redemption state is `settled`;
    - verify counted value and redemption queue both decreased by the claim
      amount.

    Observed cost: 5-10 minutes.

15. Final evidence sweep.

    Existing checks:

    - all six validators same height/root;
    - `verify-bridge`;
    - Arbitrum balance deltas;
    - final a651 NAV state;
    - final pfUSDC bucket state;
    - write summary artifact.

    Observed cost: 5-10 minutes.

### Main Bottlenecks

- Manual quote/sign/mempool/certify/apply/check repeated for every PFTL
  operation.
- No resumable state machine; every interruption required rediscovery.
- No single source of truth for live asset ids, accounts, vault addresses,
  verifier addresses, and expected balances.
- No preflight simulation of NAV accounting invariants before moving funds.
- Live Arbitrum ABI compatibility was discovered during the run instead of at
  preflight.
- Bridge relay stages were serialized as separate operator ceremonies even
  where controlled-launch dependencies were already deterministic.

## After: Sub-10-Minute Controlled-Launch Target

The end-state target is a complete live-value round trip in less than 10
minutes, including:

- Arbitrum USDC deposit;
- PFTL deposit relay and a651 primary mint;
- NAV after-money-in verification;
- a651 redemption back to pfUSDC;
- NAV after-money-out verification;
- pfUSDC burn, Arbitrum withdrawal claim, and PFTL redemption settlement;
- final summary artifact and six-validator height/root check.

There are three implementation milestones. Milestone A and B are useful because
they remove operational drag, but they are not the goal. Milestone C is the
first architecture expected to hit the target consistently.

### Sub-10 Timing Budget

This budget is the acceptance model for Milestone C. It assumes Arbitrum One RPC
responsiveness is normal, WAN validators are already healthy, the StakeHub agent
is unlocked before the command starts, and both Arbitrum challenge windows are
configured to 5 seconds for controlled launch.

The goal is not to scrape under 600 seconds once. The nominal engineering target
is 540 seconds, with a hard pass ceiling of 600 seconds. A Milestone C build
passes only if at least 8 of 10 clean runs finish under 600 seconds and the
median clean run is under 540 seconds. Any miss must be classified by segment.

A clean run means:

- all six WAN validators are healthy and in consensus at preflight;
- Arbitrum RPC is reachable before funds move;
- StakeHub agentd is unlocked before the timed command starts;
- no operator pause, manual fix, code deploy, validator roll, or external chain
  outage occurs during the run;
- every state transition still executes against live Arbitrum and live WAN
  PFTL, and all configured challenge waits are included in elapsed time.

Clean does not mean successful. A run with an invariant failure, wrong balance
delta, wrong NAV delta, stale packet, or mismatched bridge accounting is a clean
failed run and counts against the acceptance rule.

| Segment | Target |
| --- | ---: |
| Preflight consensus, config, balances, and ABI detection | 20s |
| Arbitrum approve/deposit and deposit-event capture | 50s |
| PFTL composite deposit relay to counted pfUSDC | 50s |
| Primary a651 mint from counted pfUSDC | 35s |
| NAV after-money-in reserve submit/finalize/check | 35s |
| a651 redeem back to pfUSDC | 35s |
| NAV after-money-out reserve submit/finalize/check | 35s |
| pfUSDC burn-to-redeem on PFTL | 35s |
| Arbitrum proof submit, 5s challenge wait, finalize proof | 30s |
| Arbitrum withdrawal submit, 5s challenge wait, finalize and claim | 40s |
| PFTL redeem settle, bridge accounting check, final summary | 35s |
| Work subtotal | 400s |
| Required slack for RPC jitter and validator convergence | 140s |
| Nominal target | 540s |
| Hard pass ceiling | 600s |

If production challenge windows differ from the 5-second controlled-launch
setting, the runner must print both values and add the difference directly to
the expected wall-clock time. For example, changing each of the two challenge
windows from 5 seconds to 60 seconds adds 110 seconds before any other
production overhead.

This budget is intentionally tight. Milestone A and B cannot satisfy it because
they preserve too many separate PFTL ceremonies. Milestone C is required because
the PFTL side must collapse to a small number of certified state transitions.

### Milestone A: One-Command Runner, Same Protocol

Interim runtime: 25-35 minutes.

This keeps the same protocol and the same number of PFTL committed steps, but
removes manual shell work.

Build a command:

```bash
postfiat-node nav-roundtrip-live-demo \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --source-rpc-url "$ARBITRUM_RPC_URL" \
  --cast-bin "$CAST" \
  --stakehub-home "$STAKEHUB_HOME" \
  --vault "$ERC20_BRIDGE_VAULT" \
  --verifier "$PFTL_WITHDRAWAL_VERIFIER" \
  --usdc "$USDC" \
  --stakehub-wallet "$STAKEHUB_WALLET" \
  --nav-asset "$A651_ASSET_ID" \
  --pfusdc "$PFUSDC_ASSET_ID" \
  --policy-hash "$POLICY_HASH" \
  --pftl-recipient "$BUYER_ACCOUNT" \
  --proposer "$HOLDER_ACCOUNT" \
  --attestor "$HOLDER_ACCOUNT" \
  --finalizer "$HOLDER_ACCOUNT" \
  --claimer "$BUYER_ACCOUNT" \
  --proposer-key-file "$HOLDER_KEY_FILE" \
  --attestor-key-file "$HOLDER_KEY_FILE" \
  --finalizer-key-file "$HOLDER_KEY_FILE" \
  --claimer-key-file "$BUYER_KEY_FILE" \
  --issuer-key-file "$ISSUER_KEY_FILE" \
  --owner-key-file "$BUYER_KEY_FILE" \
  --amount-atoms 5082364 \
  --mint-amount 1 \
  --nonce "$NONCE" \
  --session-id "$SESSION_ID" \
  --withdrawal-signer-key-file "$WITHDRAWAL_SIGNER_KEY_FILE" \
  --expires-at-height "$EXPIRES_AT_HEIGHT" \
  --artifact-dir "$RUN_DIR" \
  --local-apply-before-certified-send \
  --quorum-early-full-propagation \
  --resume
```

The runner accepts no stage flag for full execution. It still accepts exactly
one completed stage flag at a time for manual recovery or debugging:
`--preflight-only`, `--evm-deposit-only`, `--deposit-relay-only`,
`--primary-mint-only`, `--nav-checkpoint-only`, `--nav-exit-only`,
`--burn-to-redeem-only`, `--evm-withdrawal-only`, or `--pftl-settle-only`.
If `--withdrawal-signer-key-file` is supplied in full-run mode, the runner
signs the verifier proof digest after the PFTL burn-to-redeem step and proceeds
into the source-chain withdrawal. If both `--withdrawal-signer-key-file` and
`--signatures-file` are omitted, the runner intentionally stops after the PFTL
burn-to-redeem step and writes
`flow8-withdrawal-signature-request/signature-request.json` plus an empty
`signatures.json`; fill that file and resume the same command with
`--signatures-file`.

Implemented EVM deposit stage:

```bash
postfiat-node nav-roundtrip-live-demo \
  --evm-deposit-only \
  --artifact-dir "$RUN_DIR/flow1-evm-deposit" \
  --source-rpc-url "$ARBITRUM_RPC_URL" \
  --cast-bin "$CAST" \
  --stakehub-home "$STAKEHUB_HOME" \
  --source-chain-id 42161 \
  --vault "$ERC20_BRIDGE_VAULT" \
  --usdc "$USDC" \
  --stakehub-wallet "$STAKEHUB_WALLET" \
  --pftl-recipient "$BUYER_ACCOUNT" \
  --amount-atoms 1000000 \
  --nonce "$NONCE" \
  --session-id "$SESSION_ID" \
  --resume
```

Implemented deposit relay stage:

```bash
postfiat-node nav-roundtrip-live-demo \
  --deposit-relay-only \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --artifact-dir "$RUN_DIR/flow2-deposit-relay" \
  --evm-deposit-report "$RUN_DIR/flow1-evm-deposit/evm-deposit.json" \
  --source-rpc-url "$ARBITRUM_RPC_URL" \
  --cast-bin "$CAST" \
  --vault "$ERC20_BRIDGE_VAULT" \
  --usdc "$USDC" \
  --pfusdc "$PFUSDC_ASSET_ID" \
  --policy-hash "$POLICY_HASH" \
  --proposer "$HOLDER_ACCOUNT" \
  --attestor "$HOLDER_ACCOUNT" \
  --finalizer "$HOLDER_ACCOUNT" \
  --claimer "$BUYER_ACCOUNT" \
  --proposer-key-file "$HOLDER_KEY_FILE" \
  --attestor-key-file "$HOLDER_KEY_FILE" \
  --finalizer-key-file "$HOLDER_KEY_FILE" \
  --claimer-key-file "$BUYER_KEY_FILE" \
  --expires-at-height "$EXPIRES_AT_HEIGHT" \
  --local-apply-before-certified-send \
  --quorum-early-full-propagation \
  --resume
```

The deposit relay stage supports `--prepare-only` for local fixture checks where
the vault bridge asset is not bootstrapped. Do not use `--prepare-only` in the
live run; the live WAN state already has pfUSDC registered.

Implemented primary mint stage:

```bash
postfiat-node nav-roundtrip-live-demo \
  --primary-mint-only \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --artifact-dir "$RUN_DIR/flow3-primary-mint" \
  --deposit-relay-report "$RUN_DIR/flow2-deposit-relay/deposit-relay.json" \
  --nav-asset "$A651_ASSET_ID" \
  --pfusdc "$PFUSDC_ASSET_ID" \
  --subscriber "$BUYER_ACCOUNT" \
  --issuer-key-file "$ISSUER_KEY_FILE" \
  --mint-amount 1 \
  --local-apply-before-certified-send \
  --quorum-early-full-propagation \
  --resume
```

Implemented NAV after-money-in checkpoint stage:

```bash
postfiat-node nav-roundtrip-live-demo \
  --nav-checkpoint-only \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --artifact-dir "$RUN_DIR/flow4-nav-money-in" \
  --nav-asset "$A651_ASSET_ID" \
  --issuer-key-file "$ISSUER_KEY_FILE" \
  --expected-vna-delta 508236400 \
  --local-apply-before-certified-send \
  --quorum-early-full-propagation \
  --resume
```

The stage signs and certifies `nav_reserve_submit` first, then signs and
certifies `nav_epoch_finalize` in a second round. That is deliberate: the live
a651 SP1 profile has a one-block challenge-window setting, even though SP1
packets are consensus-verified and not challengeable.

Implemented NAV exit stage:

```bash
postfiat-node nav-roundtrip-live-demo \
  --nav-exit-only \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --artifact-dir "$RUN_DIR/flow5-nav-exit" \
  --primary-mint-report "$RUN_DIR/flow3-primary-mint/primary-mint.json" \
  --nav-asset "$A651_ASSET_ID" \
  --pfusdc "$PFUSDC_ASSET_ID" \
  --owner-key-file "$BUYER_KEY_FILE" \
  --issuer-key-file "$ISSUER_KEY_FILE" \
  --local-apply-before-certified-send \
  --quorum-early-full-propagation \
  --resume
```

Implemented NAV after-money-out checkpoint stage:

```bash
postfiat-node nav-roundtrip-live-demo \
  --nav-checkpoint-only \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --artifact-dir "$RUN_DIR/flow6-nav-money-out" \
  --nav-asset "$A651_ASSET_ID" \
  --issuer-key-file "$ISSUER_KEY_FILE" \
  --expected-vna-delta -508236400 \
  --local-apply-before-certified-send \
  --quorum-early-full-propagation \
  --resume
```

Implemented burn-to-redeem stage:

```bash
postfiat-node nav-roundtrip-live-demo \
  --burn-to-redeem-only \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --artifact-dir "$RUN_DIR/flow7-burn-to-redeem" \
  --nav-exit-report "$RUN_DIR/flow5-nav-exit/nav-exit.json" \
  --pfusdc "$PFUSDC_ASSET_ID" \
  --owner-key-file "$BUYER_KEY_FILE" \
  --destination-ref "evm-erc20:42161:$STAKEHUB_WALLET" \
  --local-apply-before-certified-send \
  --quorum-early-full-propagation \
  --resume
```

Implemented EVM withdrawal stage:

```bash
postfiat-node nav-roundtrip-live-demo \
  --evm-withdrawal-only \
  --data-dir "$PFTL_DATA_DIR" \
  --artifact-dir "$RUN_DIR/flow8-evm-withdrawal" \
  --burn-to-redeem-report "$RUN_DIR/flow7-burn-to-redeem/burn-to-redeem.json" \
  --source-rpc-url "$ARBITRUM_RPC_URL" \
  --cast-bin "$CAST" \
  --stakehub-home "$STAKEHUB_HOME" \
  --source-chain-id 42161 \
  --vault "$ERC20_BRIDGE_VAULT" \
  --verifier "$PFTL_WITHDRAWAL_VERIFIER" \
  --usdc "$USDC" \
  --stakehub-wallet "$STAKEHUB_WALLET" \
  --pfusdc "$PFUSDC_ASSET_ID" \
  --signatures-file "$WITHDRAWAL_SIGNATURES_FILE" \
  --session-id "$SESSION_ID" \
  --resume
```

This stage intentionally requires an existing signatures file. The runner
builds calldata and submits via StakeHub `agentd`; it does not place verifier
signer private keys in the command line.

Implemented PFTL settle stage:

```bash
postfiat-node nav-roundtrip-live-demo \
  --pftl-settle-only \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --artifact-dir "$RUN_DIR/flow9-pftl-settle" \
  --evm-withdrawal-report "$RUN_DIR/flow8-evm-withdrawal/evm-withdrawal.json" \
  --pfusdc "$PFUSDC_ASSET_ID" \
  --settlement-key-file "$ISSUER_KEY_FILE" \
  --local-apply-before-certified-send \
  --quorum-early-full-propagation \
  --resume
```

The settlement stage fails if the EVM withdrawal report did not verify
wallet/vault deltas. In live mode it also verifies that the PFTL redemption is
settled and the source bucket's redemption queue and counted value both fall by
the claimed amount.

Required behavior:

- fail before moving money unless all six validators agree on height/root;
- fail before moving money unless the StakeHub wallet has enough USDC and gas;
- detect the live vault ABI before bridge-out;
- detect already-completed stages from artifacts and chain state;
- never use a raw StakeHub EVM private key;
- keep verifier signer material inside the process;
- write one final summary JSON.

Engineering estimate: 1-2 engineering days.

### Milestone B: PFTL Operation Helper and Safe Local Batching

Interim runtime: 15-25 minutes.

This adds reusable PFTL orchestration and combines operations where the current
state machine already allows multiple transactions in one certified block.

Implemented first helper:

```bash
postfiat-node pftl-submit-certified-asset-ops \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --ops-file "$RUN_DIR/asset-ops.json" \
  --artifact-dir "$RUN_DIR/pftl-batch" \
  --local-apply-before-certified-send \
  --resume
```

Implemented bundle adapter:

```bash
postfiat-node pftl-certified-asset-ops-from-bundle \
  --bundle "$RUN_DIR/bundles/flow2-deposit-relay" \
  --output "$RUN_DIR/flow2-certified-ops.json" \
  --proposer-key-file "$HOLDER_KEY" \
  --attestor-key-file "$HOLDER_KEY" \
  --finalizer-key-file "$HOLDER_KEY" \
  --claimer-key-file "$BUYER_KEY" \
  --overwrite
```

The output can be submitted through `pftl-submit-certified-asset-ops`, turning
the legacy bundle script's repeated quote/sign/submit ceremony into one
certified mempool round where dependencies allow a same-block batch.

The submit helper can also consume the bundle directly:

```bash
postfiat-node pftl-submit-certified-asset-ops \
  --data-dir "$PFTL_DATA_DIR" \
  --topology "$PFTL_TOPOLOGY" \
  --key-file "$VALIDATOR_KEY_FILE" \
  --bundle "$RUN_DIR/bundles/flow2-deposit-relay" \
  --proposer-key-file "$HOLDER_KEY" \
  --attestor-key-file "$HOLDER_KEY" \
  --finalizer-key-file "$HOLDER_KEY" \
  --claimer-key-file "$BUYER_KEY" \
  --artifact-dir "$RUN_DIR/flow2-certified" \
  --local-apply-before-certified-send \
  --overwrite
```

That direct path writes the generated certified-ops request as a sibling file
to the artifact directory, then signs, submits, and drains the mempool through
the existing certified batch path.

Input shape:

```json
{
  "schema": "postfiat-certified-asset-ops-request-v1",
  "operations": [
    {
      "label": "receipt-submit",
      "source": "<PFTL account>",
      "key_file": "<owner-only key file>",
      "operation": { "operation": "<asset operation kind>" }
    }
  ]
}
```

The command has three modes:

- `--prepare-only`: validate and write normalized operation artifacts, but do
  not quote/sign/submit;
- `--batch-only`: quote/sign/submit and create one mempool batch, but do not
  run peer certification;
- default: quote/sign/submit, create one mempool batch, and run the existing
  peer-certified mempool finality round.

Target helper contract:

```text
submit_certified_asset_ops(ops[]) -> {
  height,
  batch_id,
  certificate_id,
  tx_ids[],
  state_root_after
}
```

It must:

- quote/sign each operation with the correct account/key;
- submit all signed operations to the mempool;
- certify a single mempool batch with `--max-transactions N`;
- apply to the live validator;
- wait until all six validators converge;
- persist artifacts for every transaction.

Candidate batches:

- receipt submit + receipt count;
- NAV subscription allocate + NAV mint, if the allocation id can be derived or
  exposed deterministically before applying the batch;
- reserve submit + epoch finalize, if finalization can read the reserve packet
  submitted earlier in the same batch;
- burn-to-redeem + non-dependent bookkeeping assertions are not a batch, but
  the burn certification can use the same helper.

Do not batch across:

- Arbitrum transactions and PFTL transactions;
- real challenge windows;
- steps that need an externally observed tx receipt from a prior on-chain tx;
- steps where the second operation requires a generated id that is not
  deterministic from the first operation's signed payload.

Engineering estimate: 2-4 engineering days, depending on how many operation ids
are made precomputable.

### Milestone C: Protocol-Level Relay Bundles

Target runtime: median clean run under 540 seconds and at least 8 of 10 clean
runs under 600 seconds with 5-second controlled-launch challenge windows. The
budget includes the two challenge waits, final PFTL settlement, and the
six-validator height/root check. A `7-12 minute` result is evidence that the
architecture is close, but it is not a pass unless the acceptance rule above is
met.

This reduces the number of PFTL certified rounds rather than only automating
them.

Add purpose-built composite operations or relay bundle validation:

1. `vault_bridge_deposit_relay_finalize`

   Input: EVM deposit event, evidence root, policy hash, attestation fields.

   Effect:

   - verifies the event and policy binding;
   - records the deposit proof;
   - finalizes the deposit;
   - creates the counted receipt in one state transition.

2. `nav_primary_mint_from_counted_receipt`

   Input: counted receipt id, NAV asset id, amount, reserve packet binding.

   Effect:

   - allocates counted pfUSDC;
   - mints a651 at NAV;
   - records the allocation/retirement relationship.

3. `nav_reserve_submit_and_finalize`

   Input: reserve packet and attestation bundle.

   Effect:

   - validates the packet;
   - finalizes the epoch if freshness and profile gates are satisfied.

4. `vault_bridge_withdrawal_settle_from_claim`

   Input: PFTL redemption id, Arbitrum claim receipt hash, claimed amount.

   Effect:

   - settles the PFTL redemption;
   - reduces counted value and redemption queue together.

These are consensus changes. They require:

- deterministic serialization;
- replay tests for each composite operation;
- adversarial tests for bad packet order, stale evidence, wrong asset, wrong
  vault, wrong recipient, partial settlement, and double settlement;
- WAN rollout only after local and controlled devnet green.

Before implementation, each composite operation needs a proof obligation:

| Bundle | Must prove before WAN |
| --- | --- |
| `vault_bridge_deposit_relay_finalize` | Equivalent to the existing propose, attest, finalize, receipt-submit, and receipt-count sequence for all accepted inputs; rejects stale, wrong-vault, wrong-token, wrong-recipient, duplicate, and malformed evidence. |
| `nav_primary_mint_from_counted_receipt` | Allocation id, retirement id, mint amount, and reserve binding are deterministic from signed inputs and current state; no receipt can be double-counted or partially retired inconsistently. |
| `nav_reserve_submit_and_finalize` | Same-transition reserve reads are deterministic and produce the same finalized epoch as submit-then-finalize; stale or mismatched packets fail closed. |
| `vault_bridge_withdrawal_settle_from_claim` | Settlement receipt hash, claim amount, counted value reduction, and redemption queue reduction are atomic and idempotent. |

Safety gates:

- composite operation replay must match the old multi-step sequence state root
  for a corpus of known good runs;
- adversarial tests must cover wrong order, duplicate evidence, duplicate
  settlement, stale evidence, wrong vault, wrong recipient, wrong asset,
  partial settlement, and post-failure retry;
- consensus changes must be protocol-version gated;
- rollout requires local green, controlled devnet green, then one-at-a-time WAN
  validator rollout with height/root checks;
- if a validator diverges or cannot rejoin, stop immediately. Do not continue
  the roll.

Engineering estimate: 1-2 weeks for a prototype, 4-6 weeks for a
security-gated WAN candidate if the proof obligations require new deterministic
ids or broader replay harness work.

Milestone C is also the point where the plan needs a go/no-go decision on
Arbitrum redeployment. The speed runner can support the currently deployed
controlled-launch ABI for measurement, but a security-correct public bridge run
must use redeployed F-01 through F-04 fixed contracts and fresh addresses. The
runner must print which class it executed.

If the class is `controlled_launch_existing_contracts`, the output may prove
live value and round-trip speed, but it must not claim the current Arbitrum
contracts include the `59af43d9` fixes.

If the proof obligations cannot be satisfied safely, do not ship Milestone C.
The fallback is to stop at Milestone B, report the reliable runtime, keep the
runner and invariant checks, and revisit the finality design rather than adding
unsafe consensus shortcuts.

## Risks and Gates

| Risk | Why it matters | Gate |
| --- | --- | --- |
| Challenge-window ambiguity | The same flow can be fast with 5-second demo windows and slow with public windows. | Summary artifact records configured delays and includes them in elapsed time. |
| Current Arbitrum contracts are not the fixed review version | Speed work could look like a public bridge claim. | Runner emits bridge class and contract addresses; docs forbid public-trustless claim until redeploy. |
| PFTL composite ops introduce consensus risk | Bundles change state-transition behavior. | Local replay tests, adversarial tests, and controlled devnet rollout before WAN. |
| NAV accounting can fail after funds move | A fast bad run is worse than a slow correct run. | Preflight sim computes every expected VNA and bridge-capacity delta before deposit. |
| RPC or validator jitter dominates runtime | Optimization must identify the real slow segment. | Runner emits per-segment timings and convergence samples. |
| Resume can double-apply a stage | Retrying a live money flow must be idempotent. | Every stage keys off chain/PFTL facts and rejects duplicate or mismatched artifacts. |

## Delivery Timeline

This timeline is for engineering execution, not runtime:

1. Milestone A runner: 1-2 engineering days.
2. Shared PFTL operation helper and safe local batching: 2-4 engineering days.
3. Invariant simulation and ABI/deployment classifier: 1-2 engineering days.
4. Composite relay operations, tests, and controlled devnet validation: 1-2
   weeks.
5. Optional fixed Arbitrum redeploy and re-verification: separate operator
   approval, then one small-dollar bridge battery against the new addresses.

## Recommended Implementation Order

1. Implement `nav-roundtrip-live-demo` as an orchestration runner.

   Expected runtime improvement: 2 hours -> 25-35 minutes.
   This is only the first checkpoint, not the target.

2. Add a reusable certified PFTL operation helper.

   Expected runtime improvement: 25-35 minutes -> 15-25 minutes.
   This is still an interim state.

3. Add preflight invariant simulation.

   Preflight must compute expected:

   - USDC wallet/vault deltas;
   - pfUSDC minted atoms;
   - a651 minted units;
   - after-money-in verified net assets delta;
   - redemption claim amount;
   - after-money-out verified net assets delta;
   - final bucket counted value and redemption queue deltas.

   Expected benefit: avoids live fund movement when NAV accounting will fail.

4. Add ABI and deployment compatibility detection.

   Preflight must classify the live vault as:

   - controlled-launch old withdrawal tuple;
   - F-03 fixed v2 withdrawal tuple;
   - unknown, fail closed.

   Expected benefit: avoids late Arbitrum relay debugging.

5. Implement protocol-level relay bundles only after the runner has produced
   several clean live runs.

   Expected runtime improvement: 15-25 minutes -> under 10 minutes.
   This is the required path to the sub-10-minute goal.

## Success Criteria

The speedup work is complete only when one command passes the Milestone C
acceptance rule and emits a summary artifact proving:

- preflight height/root consensus;
- bridge class, contract addresses, and configured challenge-window durations;
- Arbitrum USDC deposit tx and balance deltas;
- PFTL deposit relay tx ids and final receipt id;
- a651 primary mint tx id;
- NAV after money-in VNA and NAV/unit;
- a651 redeem tx id;
- NAV after money-out VNA and NAV/unit;
- pfUSDC burn tx id;
- Arbitrum proof/finalize/withdraw/finalize/claim tx ids;
- final PFTL redeem settle tx id;
- final bridge bucket counted value, outstanding amount, redemption queue, and
  unallocated capacity;
- all six validators same final height/root.

The runner should stop and leave an actionable failure artifact if any expected
delta fails. A run that uses `controlled_launch_existing_contracts` may satisfy
the speedup demo, but only `fixed_contracts_redeployed` can satisfy a public
trustless bridge claim.

## Non-Goals

- Do not hide challenge windows.
- Do not bypass StakeHub agentd for EVM wallet signing.
- Do not use raw private keys for the StakeHub wallet.
- Do not batch across external-chain observation boundaries.
- Do not claim a public trustless bridge until the fixed Arbitrum contracts are
  redeployed and the proof/challenge path is live against those contracts.
