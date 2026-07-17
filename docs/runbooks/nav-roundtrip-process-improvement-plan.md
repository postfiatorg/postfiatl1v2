# NAV Roundtrip Process Improvement Plan

Status: operational improvement plan
Date: 2026-06-21
Version: 2026-06-21.6
Audience: PFTL operator, StakeHub operator, protocol engineer
Related runbooks:

- `docs/runbooks/nav-roundtrip-speedup-plan.md`
- `docs/runbooks/wan-devnet-full-live-end-to-end-run.md`
- `docs/runbooks/wan-devnet-structural-fix.md`

## Purpose

Turn the live a651 <-> pfUSDC roundtrip from a fragile operator ceremony into a
repeatable command path with stable evidence, bounded runtime, and explicit
custody/NAV reconciliation.

This plan is about process, not marketing speed. A run is only faster if the
same economic proof completes with less waiting, less rediscovery, or safely
batched work. A run that skips bridge custody checks, NAV accounting,
challenge-window disclosure, validator convergence, or agentd signing is not an
improvement.

## Executive Position

The path can get materially faster, but the runtime target depends on what is
being measured:

| Run class | Current | Realistic target | Requires |
| --- | ---: | ---: | --- |
| Full Arbitrum roundtrip | ~120s | 80-95s | Process and runner cleanup only. |
| Full Arbitrum roundtrip with PFTL compression | ~120s | 55-75s | Replay-proven same-round PFTL batches. |
| Full Arbitrum roundtrip with EVM consolidation | ~120s | 35-55s | Arbitrum contract/API redeploy and fresh bridge battery. |
| Warm PFTL-only user path | not comparable | 15-30s | Funds already bridged or bridge-out deferred. |

The important distinction is that sub-minute full bridge roundtrips are
possible, but they probably require either PFTL critical-path compression,
Arbitrum withdrawal consolidation, or both. A 15-30 second result is a valid
user path only when the user is already inside PFTL; it is not the same claim as
bridging USDC in from Arbitrum and back out again.

## End-To-End Definition

For this workflow, "end to end" means all of the following complete and verify:

1. real Arbitrum USDC leaves the StakeHub wallet and enters the bridge vault;
2. the Arbitrum deposit is relayed to PFTL and mints pfUSDC;
3. pfUSDC is subscribed into real a651 through the primary NAV mint path;
4. a651 verified net assets rise by the expected money-in amount;
5. a651 exits back into pfUSDC;
6. a651 verified net assets fall by the expected money-out amount;
7. pfUSDC is burned for source-chain redemption;
8. the withdrawal proof/finalize/submit/finalize/claim path returns real
   Arbitrum USDC to the StakeHub wallet;
9. the PFTL redemption is settled;
10. final wallet/vault balances, pfUSDC bucket accounting, redemption queue,
    counted vault value, NAV state, and validator height/state-root consensus
    all match.

The run is not complete when only the PFTL leg succeeds. It is complete only
after money goes in, NAV moves up, money comes out, NAV moves back, bridge
custody reconciles, and the final PFTL settlement closes.

## Before: Two-Hour Failure Mode

The first live process took roughly two hours because the live run mixed work
that should have been separated:

- environment discovery while money was already moving;
- validator fleet troubleshooting during the demo path;
- manual reconstruction of quote/sign/submit/certify/apply stages;
- late contract ABI discovery;
- late verification of StakeHub unlock/signing behavior;
- manual calculation of expected NAV and bridge accounting deltas;
- a live reserve-accounting repair;
- repeated process launches and shell-level polling;
- missing single artifact root for evidence and timing;
- unclear distinction between full Arbitrum roundtrip and PFTL-only completion.

That failure mode is no longer acceptable for live-value demos.

## Latest Regression: Four-Hour Run

The later four-hour live attempt was a different failure mode. The runner had
already become faster, but the process still allowed infrastructure repair,
operator access confusion, funding checks, and live-value execution to collapse
into one session.

Root causes:

- the demo path depended on a mixed-provider validator fleet instead of a clean
  controlled-demo topology;
- access assumptions were debugged during the live run;
- the operator flow did not fail early enough when wallet/vault funding,
  endpoint health, or validator readiness was incomplete;
- manual intervention and recovery work were counted as if they were the same
  thing as protocol runtime;
- benchmark expectations were discussed before the run class was locked down:
  full Arbitrum roundtrip, bridge-in only, PFTL-only, or recovery path;
- speed work focused on single-stage latency while the larger process still
  permitted avoidable setup and repair inside the hot path.

Process conclusion: the speed target is realistic only when the live run starts
from a clean manifest and a known-good fleet. If infrastructure repair or
wallet funding is required, the run is not a benchmark and should be labeled
`setup/recovery`, not `full-roundtrip`.

## Current Baseline

The automated WAN benchmark is materially better than the original manual
process. The current five-run live-value benchmark averaged `120.39s`.

Current 120-second decomposition:

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

The remaining bottlenecks are:

- Arbitrum withdrawal path latency;
- sequential PFTL certification rounds;
- preflight and final validator evidence capture on the hot path;
- public RPC variance;
- repeated setup work that should be warmed or precomputed.

## Performance Hypothesis

The current `120.39s` result is no longer dominated by human typing. It is
dominated by a few serial waits:

| Bottleneck | Why it costs time | Improvement lever |
| --- | --- | --- |
| Arbitrum withdrawal path | Multiple EVM transactions plus challenge-window waits. | Contract/API consolidation or fewer required receipt waits. |
| PFTL certified rounds | Some stages still certify in separate rounds even when dependencies are deterministic. | Replay-proven same-round batching. |
| Fleet and final evidence | Public validator polling sits on the user-visible path. | Precompute fresh fleet evidence and move expanded audit after completion. |
| StakeHub/EVM setup | Launch sessions, approvals, and RPC polling repeat work. | Warm bounded session, allowance skip, receipt watcher. |

The fastest safe work is to remove repeated setup and polling. The largest
structural win is to shorten the Arbitrum withdrawal sequence, but that requires
operator approval because it touches deployed contracts.

## Measurement Contract

A live run has two clocks:

| Clock | Starts | Stops | Used for benchmark |
| --- | --- | --- | --- |
| Readiness clock | when setup, funding, fleet checks, or recovery begin | when the manifest is green and funds are ready to move | No |
| Protocol clock | immediately before the first value-moving transaction | after Arbitrum USDC is claimed, PFTL redemption is settled, and final invariants pass | Yes |

The `120.39s` baseline is a protocol-clock number for the automated full
Arbitrum roundtrip. Any future comparison to that baseline must use the same
scope. If a run includes wallet funding, validator repair, manifest repair,
contract redeploy, failed-stage recovery, or manual diagnosis, that time is
reported separately as readiness or recovery time.

Every run summary must therefore record:

- `run_class`;
- `timing_scope`;
- `protocol_clock_started_at_stage` and `protocol_clock_stopped_at_stage`;
- `readiness_preflight_ms` and `protocol_clock_ms`;
- `setup_or_recovery_work_included_in_total`;
- whether funding, fleet repair, or contract changes occurred before the
  protocol clock;
- whether the run used a prepared manifest or generated one live;
- whether the final state is full Arbitrum completion, PFTL-only completion, or
  recovery-pending.

## Process KPIs

The process is improving only if these measurements move in the right
direction. These are operational KPIs, not protocol marketing claims.

| KPI | Current problem | Target |
| --- | --- | --- |
| Unknowns after first value transaction | Environment facts were still being discovered during live-value runs. | Zero unknown chain, topology, wallet, contract, NAV, or bridge facts after deposit. |
| Hot-path owners | Multiple shells and manual interventions made causality hard to track. | One runner process mutates state during the live run. |
| Manual mutation count | Manual partial commands were used to recover or continue. | Zero manual mutations unless the runner has stopped and emitted a resume artifact. |
| Resumable failure coverage | Recovery depended on shell history and memory. | Every post-deposit failure writes custody location, failed stage, and exact resume command. |
| Benchmark cleanliness | Setup, repair, and protocol execution were conflated. | `setup_or_recovery_work_included_in_total=false` for every accepted benchmark row. |
| Evidence completeness | Some proofs were assembled after the fact. | Each stage writes its report before the next stage starts. |
| Invariant pass rate | NAV and bridge deltas had to be debugged live. | 100% wallet/vault, pfUSDC, NAV, redemption, and validator convergence checks on accepted runs. |
| Operator credential hygiene | Password auth confusion and unsafe command patterns polluted the run. | No plaintext credentials on command lines; all demo validators use approved key-based access. |

The top-level dashboard should expose at least the benchmark cleanliness,
completion class, custody location, invariant status, and slowest three stages.
If those are absent, the run may be useful evidence, but it is not a clean
operator benchmark.

## Operating Principles

1. Fail before funds move.

   Environment, topology, wallet balances, contract code, ABI class, challenge
   windows, agentd unlock, allowance, asset ids, and expected NAV/bridge deltas
   must be known before the Arbitrum deposit transaction is submitted.

2. Keep all signing in StakeHub/agentd.

   No raw EVM private-key fallback is allowed for live funds.

3. Keep validator access clean.

   No `sshpass`, no command-line plaintext passwords, and no mixed-provider
   assumptions in the default run path. The standard controlled testnet path
   should use a clean all-Vultr fleet unless the operator explicitly chooses a
   different topology. Hetzner or mixed-provider operation is a resilience test,
   not the default demo benchmark.

4. Record every run as evidence.

   The runner owns the artifact directory. Every stage writes machine-readable
   reports, timing, transaction ids, state roots, balance deltas, and stop
   reasons.

5. Separate completion from audit expansion.

   The hot path must include the checks required for safety. Deeper public RPC
   audit dumps can run in the background only after the completion checks pass.

6. Do not hide challenge windows.

   Controlled-launch challenge windows and production challenge windows must be
   reported separately. Speed claims must include the live configured windows.

7. Time only a declared run class.

   A benchmark cannot change identity mid-run. If the run starts as
   `full-arbitrum-roundtrip`, it must either complete that path or fail with a
   resumable artifact. It must not be converted into a PFTL-only or recovery
   claim after the fact.

## Control Loop

Each live-value attempt follows the same control loop.

### Pre-Run

Owner: demo lead.

Required output: green manifest plus go/no-go decision.

Steps:

1. choose run class;
2. choose topology;
3. verify StakeHub wallet funding and gas;
4. verify agentd unlock and signing route;
5. verify vault/verifier/token code and ABI class;
6. verify a651 and pfUSDC reachability;
7. compute expected USDC, pfUSDC, NAV, redemption queue, and counted-value
   deltas;
8. run fleet preflight and record height/state-root agreement;
9. create artifact root;
10. approve or reject starting the protocol clock.

No value-moving transaction is allowed before this output exists.

### Live Run

Owner: one runner process.

Required output: stage reports and either final summary or resumable failure.

Rules:

- one process owns mutation;
- observers do not submit parallel transactions;
- manual commands are allowed only after the runner has stopped and produced a
  resume artifact;
- every stage writes a report before the next stage starts;
- the run stops on any invariant mismatch instead of continuing with a manual
  interpretation.

### Post-Run

Owner: protocol engineer plus operator.

Required output: benchmark row or failure disposition.

Steps:

1. classify final state;
2. record total runtime and per-stage timings;
3. compare actual deltas to expected deltas;
4. identify the slowest three segments;
5. mark the run `accept`, `process-fix`, `code-fix`, or `infra-fix`;
6. create exactly one next-action owner for every failure or slowdown;
7. update the benchmark aggregate only if the run class and completion status
   match the benchmark definition and
   `setup_or_recovery_work_included_in_total=false`.

## Process Changes

The improvement is not "type faster." The improvement is to move every
discoverable fact out of the live-money path and make the remaining path a
bounded state machine.

### 0. Classify Before Timing

Every run must be labeled before the first transaction:

- `full-arbitrum-roundtrip`;
- `bridge-in-only`;
- `pftl-only`;
- `recovery`;
- `setup`.

Only `full-arbitrum-roundtrip` can be compared to the 120-second benchmark.
Setup, funding, fleet repair, contract redeploy, and recovery do not belong in
the benchmark timer.

### 1. Run From A Manifest

Every live-value run starts from a checked-in or archived manifest containing:

- chain id and expected genesis;
- validator endpoints and provider class;
- a651 asset id, issuer, profile id, and expected NAV profile;
- pfUSDC asset id, issuer, and bridge bucket;
- Arbitrum vault, verifier, token, wallet, and chain id;
- challenge-window policy;
- exact amount to bridge, mint, exit, burn, and claim;
- expected wallet/vault/PFTL/NAV deltas;
- artifact root;
- allowed runner modes.

If the manifest cannot compute the expected deltas before the first Arbitrum
deposit transaction, the run fails before funds move.

### 2. One Hot-Path Owner

During a live demo, one process owns the state transition sequence. Other shells
can observe, but they do not submit transactions, re-run partial commands, or
mutate validator state unless the runner stops and emits a resume command.

This prevents the original failure mode where the live run mixed demo execution,
fleet troubleshooting, accounting repair, and manual bridge recovery.

### 3. No Live Environment Discovery

The run cannot discover these during the timed path:

- where validators are;
- what chain is live;
- which asset is real a651;
- whether StakeHub is unlocked;
- whether the wallet has gas or USDC;
- whether the contract ABI is old or fixed;
- whether allowance is missing;
- whether expected NAV deltas can be computed;
- whether bridge bucket accounting is sane.

Those are preflight facts. If any are unknown, the run is not ready.

### 4. Separate Completion From Audit

The hot path ends only after bridge custody and NAV accounting reconcile, but
expanded public-RPC evidence can continue after completion. The summary must
state which checks were blocking and which were background audit.

This makes the demo faster without weakening the economic proof.

### 5. Every Failure Is Resumable

If a stage fails after funds move, the runner writes:

- the completed stage reports;
- the exact failed stage;
- the custody location of funds;
- the expected next deltas;
- the precise resume command;
- whether operator intervention is required.

No one should reconstruct the recovery path from shell history.

## Role Ownership

| Role | Owns | Must not own |
| --- | --- | --- |
| PFTL operator | validator health, chain id, asset reachability, certified rounds, state-root convergence | Arbitrum signing |
| StakeHub operator | wallet funding, agentd unlock, EVM signing, USDC allowance, Arbitrum gas | validator repair |
| Protocol engineer | runner code, replay corpus, benchmark verifier, bridge/NAV invariant tests | live manual shortcuts |
| Demo lead | manifest approval, go/no-go call, stopping on failed gates | ad hoc code or validator changes |

One person can hold multiple roles in controlled testnet, but the artifact must
make clear which responsibility failed if the run stops.

## Required Pre-Run Gate

Before another live-value run, one command must prove:

- all validators are reachable;
- all validators agree on chain id, height, tip, and state root;
- the chain is the intended WAN/testnet chain;
- real a651 is registered with the expected issuer/profile;
- pfUSDC is registered and bucket accounting is sane;
- the StakeHub wallet has enough Arbitrum USDC and gas ETH;
- StakeHub `agentd` is unlocked;
- the vault and verifier contracts have code at the configured addresses;
- the vault/verifier ABI class is detected and allowed;
- challenge windows are printed;
- USDC allowance is sufficient or an approval plan is explicit;
- expected wallet/vault USDC deltas are computed;
- expected pfUSDC mint/burn amounts are computed;
- expected a651 NAV money-in and money-out deltas are computed;
- expected redemption queue and counted-value deltas are computed;
- the run artifact directory exists before the first transaction.

If any item fails, the run does not start.

The pre-run gate must classify the run before funds move:

| Run class | Completion requirement | May be benchmarked against 120s baseline |
| --- | --- | --- |
| `full-arbitrum-roundtrip` | USDC in, pfUSDC mint, a651 mint, NAV up, a651 exit, NAV down, burn, Arbitrum claim, PFTL settle | Yes |
| `bridge-in-only` | Arbitrum deposit relayed to PFTL and pfUSDC minted | No |
| `pftl-only` | Starts from existing PFTL balances and finishes PFTL-side accounting | No |
| `recovery` | Resumes or repairs a previously incomplete run | No |
| `setup` | Funds wallet, repairs fleet, changes contracts, or prepares manifests | No |

Only `full-arbitrum-roundtrip` can be compared with the current 120-second
benchmark. Every other class must report its own scope.

## Mandatory Changes Before The Next Live Speed Run

The next live speed attempt should not start until these process changes are
in place. This is the short checklist that converts the diagnosis above into
operator behavior.

1. Use a pinned run manifest.

   The manifest must name the run class, validator topology, chain id, genesis,
   asset ids, contract addresses, challenge windows, wallet, exact amount, and
   expected deltas. The runner can verify and enrich the manifest, but it must
   not discover core identity facts during the live run.

2. Run preflight as its own stage.

   Preflight must complete before the first Arbitrum transaction and must
   produce a green artifact. If funding, unlock, allowance, contract class,
   validator convergence, or NAV delta computation fails, the run is `setup`
   or `infra-fix`, not a benchmark.

3. Use the all-Vultr controlled-demo topology by default.

   Hetzner or mixed-provider operation can be tested separately, but it must
   not be the default path for speed benchmarks. Speed runs should not debug
   provider inventory or host access after funds move.

4. Keep one orchestration command in charge.

   The live run should use the full `nav-roundtrip-live-demo` orchestration
   path with a single artifact root. Stage-only commands are for setup,
   focused testing, or runner-emitted recovery instructions.

5. Precompute what is not value-moving.

   Fleet evidence, contract code checks, allowance checks, route/amount
   planning, and expected accounting deltas should be prepared before the
   protocol clock starts. The timed run may recheck freshness, but it should
   not do discovery.

6. Make dashboard status consume normalized JSON.

   The operator UI must read the dashboard-status adapter output instead of
   inferring run state from logs. Full Arbitrum completion, PFTL-only
   completion, bridge-out deferred, and recovery-pending must be distinct.

7. Close every run with a review row.

   A passing run gets a benchmark row. A failed run gets a disposition:
   `process-fix`, `code-fix`, or `infra-fix`. No live-value run is closed
   until custody location, final invariant status, and next owner are explicit.

No-go rule: if any of the seven items above are missing, the next session may
still be useful setup work, but it should not be called a clean live speed run.

## Target Process

The standard run should be a single orchestration command over a preflighted
manifest:

1. create a run artifact root;
2. run blocking preflight;
3. optionally run fleet/public evidence capture before the timed section;
4. open one bounded StakeHub launch session;
5. deposit Arbitrum USDC into the vault;
6. relay the deposit to PFTL and mint pfUSDC;
7. primary mint a651 from pfUSDC;
8. checkpoint NAV after money-in;
9. exit a651 to pfUSDC;
10. checkpoint NAV after money-out;
11. burn pfUSDC to redeem;
12. generate and sign the withdrawal packet;
13. finalize proof, submit withdrawal, finalize withdrawal, and claim USDC;
14. settle the PFTL redemption;
15. verify final NAV, bridge custody, redemption queue, counted value, wallet
    balance, vault balance, mempool, and validator convergence;
16. close the StakeHub launch session;
17. write one final summary with pass/fail status and timing.

Every failure after funds move must produce a resumable artifact and an exact
next command.

## Demo Fleet Standard

Default live demos should run on a homogeneous all-Vultr controlled testnet.
The manifest should list exactly the validators, RPC ports, expected chain id,
and expected genesis. The run fails before funds move if any listed validator
is unreachable or disagrees on height/state root beyond the allowed freshness
window.

Mixed-provider fleets remain valuable for resilience testing, but they should
be run under a separate manifest and reported separately. They must not be used
for speed claims unless the benchmark explicitly says it used a mixed-provider
fleet.

No live demo should debug SSH access, firewall access, validator service
layout, or provider inventory after the Arbitrum deposit has been submitted.

## Runtime Targets

| Phase | Target | Scope |
| --- | ---: | --- |
| Baseline | 117-126s | Current five-run automated WAN result. |
| Phase 1 | 80-95s | No redeploy; runner/process changes only. |
| Phase 2 | 55-75s | Safe PFTL batching and critical-path compression. |
| Phase 3 | 35-55s | Arbitrum withdrawal consolidation or redeploy. |
| Phase 4 | 15-30s | Warm PFTL-only path, not full bridge roundtrip. |

Phase 4 must never be described as the full Arbitrum roundtrip. It is the fast
path for users who already hold a651 or pfUSDC on PFTL.

## Phase 1: No-Redeploy Process Speedups

Target: 80-95 seconds.

Actions:

1. Reuse one bounded StakeHub launch session.

   Deposit and withdrawal EVM transactions should share one launch session.
   The runner must close the session on success and on failure.

2. Skip approval when allowance is already sufficient.

   Query `allowance(owner, vault)` before timing. If allowance is sufficient,
   skip `approve`. If not, either perform approval before the timed section or
   include it honestly in the benchmark.

3. Split fleet preflight from timed execution.

   Public validator evidence can be precomputed if the runner rechecks
   freshness before funds move.

4. Split final completion checks from background audit.

   The hot path can use final certified-round evidence if it still verifies
   local state, mempool emptiness, and validator convergence. Public RPC audit
   can continue after completion.

5. Add provider classification and receipt timing.

   Every run should state whether it used WebSocket, local RPC, dedicated HTTP,
   gateway HTTP, or public/unknown HTTP.

6. Move setup/funding into a separate `setup` command.

   Wallet funding, vault top-up, allowance repair, manifest generation, and
   validator readiness checks should run before the benchmark command. The
   benchmark command may verify them, but it should not discover or repair
   them.

Acceptance:

- 10 clean live-value runs;
- median under `95s`;
- p90 under `105s`;
- no raw private-key signing;
- NAV and bridge invariants verified in every run;
- every report states whether preflight/audit work was on the hot path.

Immediate implementation backlog:

| Item | Owner | Expected impact | Gate |
| --- | --- | ---: | --- |
| Warm one StakeHub launch session across deposit and withdrawal. | Protocol engineer | 2-5s | Runner test plus live artifact showing one session id. |
| Skip USDC approval when allowance is sufficient. | Protocol engineer | 3-5s | Fake-agent test and live artifact with `approve_skipped=true`. |
| Precompute fleet evidence with freshness recheck. | PFTL operator | 5-10s perceived | Artifact proves fresh height/root at run start. |
| Use certified-round final evidence on hot path, public audit in background. | Protocol engineer | 3-5s | Summary labels final evidence source and writes background audit request. |
| Classify RPC provider and receipt timings. | StakeHub operator | 2-5s variance | Every EVM tx has watcher evidence. |

## Phase 2: PFTL Critical-Path Compression

Target: 55-75 seconds.

Actions:

1. Add explicit dependency declarations to every certified operation.

   Operations must declare whether dependencies are `same_round` or
   `prior_round`.

2. Batch only deterministic same-round operations.

   Candidate classes include:

   - deposit receipt submit plus receipt count;
   - subscription allocation plus `nav_mint_at_nav`;
   - deterministic bridge claim stages where ids are known before apply.

3. Keep prior-round dependencies separate.

   Reserve submit/finalize, redemption request/settle, bridge settle, and any
   state-read-dependent operation stay unbatched until replay proves otherwise.

4. Maintain a replay equivalence corpus.

   For each proposed batch class, replay old sequential blocks and the proposed
   batch from the same fixture. Live compression is allowed only when the final
   state root matches or an intended root difference is documented and gated.
   Phase 2 gates must run `nav-roundtrip-replay-corpus-verify` with
   `--require-live-compression-ready --require-candidate-classes CSV --strict`,
   where the CSV is the complete intended batch-class list from
   `roundtrip-summary.json` `pftl_candidate_batch_classes` or the underlying
   certified-ops dependency reports.
   A run summary is allowed to report replay-needed blockers; the Phase 2 gate
   closes only those blockers when the replay corpus has a live-ready case for
   every summary-derived candidate class. Missing corpus classes or any
   non-replay blocker still fail the benchmark.

5. Add adversarial dependency tests.

   Required cases: duplicate labels, wrong order, stale evidence, wrong asset,
   wrong vault, wrong recipient, double settlement, dirty mempool, and failed
   retry/resume behavior.

Acceptance:

- replay corpus green for every batch class;
- required candidate class coverage enforced by
  `--require-candidate-classes CSV`;
- adversarial dependency tests green;
- no hidden prior-round dependencies in same-round requests;
- 10 clean WAN runs;
- median under `75s`;
- no accounting regression.

Candidate batch classes currently worth pursuing:

| Candidate class | Status | Notes |
| --- | --- | --- |
| `vault_bridge_deposit_propose_attest` | Replay corpus exists | Same-round safe only if live-ready corpus gate passes. |
| `vault_bridge_receipt_submit_count` | Replay corpus exists | Starts after deposit evidence finalization. |
| `nav_subscription_allocate_mint_at_nav` | Replay corpus exists | Allocation retirement must match after height-normalization. |
| `nav_redeem_at_nav_settle` | Replay corpus exists | Redemption id derives from signed redeem sequence. |

No additional class should be enabled live until the replay corpus proves
ledger-facing equivalence and the benchmark verifier requires the exact classes
reported by the summary.

## Phase 3: Arbitrum Withdrawal Consolidation

Target: 35-55 seconds.

This phase changes or redeploys contracts and therefore requires an explicit
operator decision.

Candidate changes:

- combine verifier proof finalization with vault withdrawal submission;
- combine withdrawal finalization with claim;
- preserve challenge windows while reducing transaction count and receipt waits;
- continue binding vault address, token address, recipient, packet hash, and
  withdrawal id into replay-resistant domains.

Required before redeploy:

- Foundry bridge suite green;
- Rust packet-binding tests green if packet encoding changes;
- unauthorized challenge griefing regression green;
- cross-vault replay regression green;
- expired-withdrawal recovery regression green;
- recipient substitution and double-claim regressions green;
- fresh small-dollar bridge-in/bridge-out against the new addresses.

Acceptance:

- 10 clean WAN roundtrips;
- median under `55s`;
- summary clearly states contract class and challenge windows;
- no weaker custody or replay assumptions than the reviewed fixed contracts.

Decision required:

Phase 3 is the most likely path to a materially sub-minute full bridge
roundtrip. It also requires redeploying or extending the Arbitrum contracts, so
it must be treated as a new bridge battery, not a runner-only optimization.
Until that decision is made, speed claims should use Phase 1 or Phase 2 targets.

## Phase 4: Warm PFTL-Only Path

Target: 15-30 seconds.

This is not bridge-in plus bridge-out. It is the user path when the user already
has a651 or pfUSDC on PFTL, or when bridge-out can be resumed later.

Actions:

- use `postfiat-node nav-roundtrip-live-demo --pftl-only` to start from
  existing issued pfUSDC on PFTL;
- consume the pfUSDC into the primary NAV mint, verify NAV money-in, exit a651
  back to pfUSDC, and verify NAV money-out;
- stop with custody explicitly on PFTL and write `pftl-only-summary.json` with
  `completion_status="on_pftl_complete_bridge_out_deferred"`;
- write `bridge-out-resume.json` with the exact
  `nav-roundtrip-live-demo --burn-to-redeem-only` command for later Arbitrum
  bridge-out;
- make the UI/report consume `nav-roundtrip-dashboard-status --summary PATH`
  so `PFTL-complete` and `Arbitrum-complete` are different machine-readable
  booleans, not inferred from prose.

Acceptance:

- no custody-location ambiguity: `PFTL-only complete` cannot be displayed as
  `full Arbitrum roundtrip complete`;
- NAV money-in and money-out checkpoint deltas are verified;
- bridge-out can be resumed from `bridge-out-resume.json`;
- `postfiat-nav-roundtrip-dashboard-status-v1` reports
  `pftl_only_complete=true`, `bridge_out_deferred=true`, and
  `full_arbitrum_roundtrip_complete=false` for PFTL-only summaries;
- full-roundtrip benchmark remains separate.

## Benchmark Reporting Standard

Every benchmark report must include:

- artifact root;
- git commit and binary hash;
- run count and clean-run definition;
- validator topology;
- final height/state-root convergence;
- contract addresses and ABI class;
- challenge windows;
- Arbitrum RPC provider class;
- whether approval was warm or timed;
- whether StakeHub launch session setup was warm or timed;
- per-stage timing table;
- NAV money-in and money-out deltas;
- wallet/vault USDC deltas;
- pfUSDC bucket counted-value and redemption-queue deltas;
- final pass/fail status;
- exact resume command for any incomplete run.

Exploratory claims require at least five clean runs. Acceptance claims require
at least ten clean runs. Do not cite a single best run as the benchmark.

## Dashboard And Operator UX

The operator dashboard should show the run class explicitly:

- `preflight failed`;
- `bridge-in complete`;
- `NAV money-in verified`;
- `NAV money-out verified`;
- `PFTL redemption pending`;
- `Arbitrum claim complete`;
- `PFTL settlement complete`;
- `full Arbitrum roundtrip complete`;
- `PFTL-only complete`;
- `audit running in background`.

The dashboard should not parse these states ad hoc from shell output. It should
read `postfiat-node nav-roundtrip-dashboard-status --summary PATH --report
PATH`, then display the normalized `run_class`, `completion_status`,
`custody_location`, `full_arbitrum_roundtrip_complete`, `pftl_only_complete`,
`bridge_out_deferred`, and `bridge_out_resume_command` fields.

It should also display:

- current stage;
- elapsed time;
- stage timing table;
- validator convergence status;
- wallet/vault USDC deltas;
- NAV VNA deltas;
- bucket counted value and redemption queue;
- challenge-window timers;
- resume command on failure.

## Stop Conditions

Stop immediately if:

- validators diverge on state root;
- a validator cannot rejoin after an approved roll;
- the chain id or genesis does not match the run manifest;
- contract ABI class is unknown or disallowed;
- expected NAV deltas cannot be computed before deposit;
- StakeHub signing requires raw private-key fallback;
- operator credentials would appear on a command line;
- Arbitrum wallet/vault deltas do not match after deposit or claim;
- pfUSDC counted value or redemption queue diverges from actual custody;
- the PFTL mempool is dirty when the run expects a clean batch;
- replay-equivalence requirements are bypassed for same-round batching.

## Post-Run Review

Every live-value run gets a short review, even when it passes.

Required review fields:

- artifact root;
- git commit and binary hash;
- run class: full Arbitrum roundtrip, PFTL-only, or bridge-in-only;
- final status;
- total runtime;
- slowest three segments;
- whether allowance was warm;
- whether StakeHub session setup was hot-path or pre-warmed;
- whether final audit was blocking or background;
- any resumed stage;
- any operator manual action;
- next code/process change.

The review should produce one of four dispositions:

| Disposition | Meaning |
| --- | --- |
| `accept` | Clean evidence, no action beyond normal benchmark aggregation. |
| `process-fix` | Runner succeeded, but operator flow or documentation caused avoidable delay. |
| `code-fix` | Runner or protocol behavior needs a code change before the next run. |
| `infra-fix` | Fleet, RPC, wallet, or provider setup needs repair before the next run. |

No failed live-money run is considered closed until custody location and next
action are explicit.

## Immediate Next Actions

1. Freeze the benchmark class before the next run.

   The next speed run should be labeled either `full-arbitrum-roundtrip` or
   `pftl-only` before timing starts. Do not compare setup/recovery or PFTL-only
   numbers to the 120-second full bridge baseline.

2. Keep all-Vultr as the standard controlled-demo topology.

   Mixed-provider operation is a resilience exercise, not the default demo path.

3. Run the Phase 1 acceptance battery.

   Use `postfiat-node nav-roundtrip-benchmark-plan` to generate the ten-run
   manifest from a complete `nav-roundtrip-live-demo` base-args file, run the
   emitted warmup and timed commands, then run the emitted strict verifier.
   Report median, p90, per-stage timing, and hot-path versus background-audit
   behavior.

4. Finish and enforce the Phase 2 replay corpus.

   Do not enable more PFTL round compression until each batch class has replay
   evidence and adversarial dependency coverage. The acceptance command should
   include `--require-live-compression-ready --require-candidate-classes CSV
   --strict` so the gate fails if a planned class is missing. Use
   `pftl_candidate_batch_classes` from the latest summary as the starting CSV.
   Use `postfiat-node nav-roundtrip-benchmark-plan --phase phase2` for the
   acceptance battery; it injects `--same-round-nav-exit`, refuses to plan
   without a replay corpus path, and emits the matching strict verifier command.
   For `nav_subscription_allocate_mint_at_nav`, the replay evidence must prove
   the allocation is retired in both paths and ledger-facing accounting is
   equivalent after normalizing the expected block-height provenance difference.
   Full staged roundtrip summaries also surface
   `vault_bridge_deposit_propose_attest` from deposit relay stage 1, so a
   Phase 2 full-run claim must include that class in the required CSV and must
   provide live-ready replay evidence for it. The focused local replay fixture
   proves this class by comparing unbatched propose/attest against same-round
   propose/attest after normalizing submitted/attested height provenance.
   For `vault_bridge_receipt_submit_count`, the replay evidence must start
   after bridge-deposit evidence finalization and prove the receipt is counted
   with identical bucket counted value in both paths after normalizing the
   expected receipt/bucket block-height provenance difference.
   For `nav_redeem_at_nav_settle`, the replay evidence now proves the
   redemption id is derivable from the signed redeem sequence before apply, and
   that the same-round settle preserves owner balances, redemption state,
   settlement receipt hash, bucket accounting, and any top-up allocation after
   normalizing block-height provenance. Include `--same-round-nav-exit` in the
   benchmark base args only after the required replay corpus gate includes
   `nav_redeem_at_nav_settle`; the runner deliberately refuses this mode with a
   caller-supplied `--redemption-id` so the settlement id is derived from the
   signed redeem sequence.

5. Decide whether Phase 3 redeploy is worth it.

   The largest remaining block is the Arbitrum withdrawal path. Sub-60-second
   full bridge runs probably require contract call consolidation. The proposed
   contract/API change is specified in
   `docs/specs/nav-roundtrip-evm-withdrawal-consolidation.md` and remains
   undeployed until an operator explicitly approves a new bridge battery. Once
   approved and deployed, use `postfiat-node nav-roundtrip-benchmark-plan
   --phase phase3` to generate the ten-run battery; its verifier command fails
   unless the summaries prove the consolidated fixed-contract bridge class and
   consolidated withdrawal receipt labels.

6. Add dashboard run-class status.

   The UI must make it impossible to confuse PFTL-only completion with full
   Arbitrum roundtrip completion.

7. Keep speed claims narrow.

   State whether a number is full bridge roundtrip, PFTL-only, warm-path,
   controlled-launch challenge windows, or production-window projected runtime.

## Definition Of Done

This process improvement is complete when:

- the standard run starts from a preflighted manifest;
- no live run performs environment discovery after money moves;
- every stage is automated or emits a precise resume command;
- every run writes a complete evidence artifact;
- ten-run Phase 1 benchmark passes;
- replay corpus gates any Phase 2 batching;
- dashboard status reflects custody location and completion class;
- operators can repeat the run without reconstructing shell history.
