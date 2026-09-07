# Shielded Perp-Venue Funding MVP Completion Manifest

Date: 2026-09-04  
Task Node: `task_7e5652b32cc6bd868570491003c5fb60`  
Source milestone:
`docs/specs/shielded-perp-venue-funding-implementation-20260904.md`

Worktree bases:

- postfiatl1v2 `3d0e5c012950` on `main`
- StakeHub `dbaa54182012` on `master`

All paths below are uncommitted worktree artifacts. No commit, push, live transfer,
contract deployment, PFTL mutation, venue withdrawal or paid GPU rental was
performed for this completion pass.

## Status vocabulary

| Status | Meaning |
| --- | --- |
| **LOCAL COMPLETE** | Code and focused tests exist; no external state is implied |
| **LIVE EVIDENCE COMPLETE** | A prior mainnet probe named below verified this narrow behavior |
| **EXTERNAL GATE** | Requires independent review, authorization, keys, funds, time or live infrastructure |
| **OPTIONAL PHASE 2** | Excluded by the MVP spec |

## Requirement ledger

### Privacy and product constraints

| ID | Requirement | Status | Concrete evidence |
| --- | --- | --- | --- |
| R0.1 | Remove the direct public treasury-to-venue-account funding edge | **LOCAL COMPLETE, CONDITIONAL** | `StakeHub/stakehub/shielded_exit_planner.py`, `shielded_exit_pftl.py`, `shielded_exit_jobs.py`; standard bands, shared PFTL fee sponsor and proof-vault release |
| R0.2 | Refuse release below the configured candidate threshold | **LOCAL COMPLETE** | `shielded_exit_jobs._prepare_anonymity_gate`; requires a fresh observer state, at most a 30-day window, same band and `candidates >= k_min` |
| R0.3 | State the privacy limit accurately | **LOCAL COMPLETE** | `StakeHub/dashboard/shielded-perp-funding.html` and both specs say venue positions, amount, timing and sponsor activity remain public |
| R0.4 | Keep stablecoins out of shared addresses | **LOCAL COMPLETE** | Outbound HL batch uses exact-output swap then direct `USDC.transfer(Bridge2)`; return uses fresh `C_i`; no shared stablecoin account |
| R0.5 | Flat per-exit protocol path | **LIVE EVIDENCE COMPLETE for tails** | live Lighter and Hyperliquid probe txs in §Live evidence; full pfETH proof cost remains gate A6/A7 |
| R0.6 | ETH-only MVP with 1/2/5/10 ETH bands | **LOCAL COMPLETE** | `shielded_exit_planner.Policy`; job builder rejects values outside the four bands |
| R0.7 | No claim of private trading on Lighter or Hyperliquid | **LOCAL COMPLETE** | milestone §0 and dashboard boundary copy |

### ExitExecutor and Ethereum vault

| ID | Requirement | Status | Concrete evidence |
| --- | --- | --- | --- |
| E1 | EIP-712 authorized atomic call batch | **LOCAL COMPLETE** | `crates/ethereum-contracts/src/ExitExecutorV1.sol` |
| E2 | EIP-7702 sponsored execution from zero-gas burner | **LOCAL COMPLETE** | contract plus `StakeHub/stakehub/shielded_exit_executor.py` |
| E3 | Replay, expiry, wrong signer, tampering, high-s and inner-revert protection | **LOCAL COMPLETE** | 13 tests in `ExitExecutorV1.t.sol` |
| E4 | Namespaced nonce, no admin custody | **LOCAL COMPLETE** | contract storage and Foundry nonce-slot test |
| E5 | Mainnet and Arbitrum implementation deployment | **LIVE EVIDENCE COMPLETE** | Ethereum `0x0c51DB40B16691319E027Ae7E1fb71A8D4F2b8bA`; Arbitrum `0xf56aB47F7D720c96145E1404E2630954F6b2F16A` |
| E6 | External review before relying on a reviewed release | **EXTERNAL GATE** | Exact manifest binds `ExitExecutorV1`, WETH vault, finality verifier and both guests; `external-review.json` is `PENDING`; existing ExitExecutor is explicitly unreviewed v1 and must be redeployed after review |
| E7 | WETH vault with pfETH scaling and route-bound proof release | **LOCAL COMPLETE** | `WETHBridgeVaultL1.sol`; 6 tests in `WETHBridgeVaultL1.t.sol` |
| E8 | Exact `1 pfETH atom = 10^9 WETH wei` and u64 aggregate conservation | **LOCAL COMPLETE** | contract constants/tests and ingress guest |
| E9 | CREATE2 same-address convenience | **OPTIONAL PHASE 2** | explicitly optional; deterministic nonce-based predicted addresses are packaged |

### Planner, keys and PFTL burners

| ID | Requirement | Status | Concrete evidence |
| --- | --- | --- | --- |
| P1 | Greedy fixed-band decomposition | **LOCAL COMPLETE** | `shielded_exit_planner.decompose` |
| P2 | 6-hour minimum plus up to 18-hour jitter and one-hour buckets | **LOCAL COMPLETE** | `shielded_exit_planner.Policy` and planner tests |
| P3 | Sibling notes for different accounts avoid the same band/bucket | **LOCAL COMPLETE** | `shielded_exit_planner.plan` and tests |
| P4 | Observed unrelated-exit gate with downgrade/refusal | **LOCAL COMPLETE** | planner gate, job-preparation gate and CLI nonzero return on refusal |
| P5 | Deterministic distinct PFTL and secp256k1 keys from a protected root | **LOCAL COMPLETE** | HKDF derivation, u32 index bounds and scalar validation |
| P6 | Encrypted local `note -> P_i -> E_i` map | **LOCAL COMPLETE** | AES-256-GCM `MappingStore`, separate 0600 key and ciphertext tests |
| P7 | Canonical PFTL wallet materialization from derived seed | **LOCAL COMPLETE** | `materialize_pftl_account`; seed-file deletion on success/failure |
| P8 | Fresh `P_i` activation and fee reserve | **LOCAL COMPLETE / EXTERNAL GATE live** | `shielded_exit_pftl.py`; shared sponsor only, preview, typed live confirmation, matching finalized accepted receipt |
| P9 | Exclude all owned burners from unrelated counts | **LOCAL COMPLETE** | `MappingStore.owned_evm_addresses`, observer and job gate |
| P10 | Prevent materialization of a refused note | **LOCAL COMPLETE** | `shielded_exit_cli.py`; focused CLI test |

### Observer, signed jobs and relayer

| ID | Requirement | Status | Concrete evidence |
| --- | --- | --- | --- |
| O1 | Incremental confirmed vault-withdrawal observer | **LOCAL COMPLETE** | `shielded_exit_observer.sync_observer` |
| O2 | Reorg rewind and duplicate-log suppression | **LOCAL COMPLETE** | observer state machine and tests |
| O3 | Counts for all 1/2/5/10 ETH bands | **LOCAL COMPLETE** | observer counter and dashboard |
| O4 | Offline public-linkability analysis | **LOCAL COMPLETE** | exact feasible-bijection enumerator; truncated enumeration is inconclusive |
| O5 | Immutable private job preparation | **LOCAL COMPLETE** | 0600 job; no overwrite; no private key in output |
| O6 | Copy and hash proof and observer artifacts beside job | **LOCAL COMPLETE** | basename-only artifact policy and digest validation |
| O7 | Reject under-k, stale observer, over-30-day window, bad precision and non-band amounts | **LOCAL COMPLETE** | `shielded_exit_jobs.py` and focused tests |
| O8 | Durable permissionless relayer with dry-run default | **LOCAL COMPLETE** | `shielded_exit_relayer.process_job/run_loop` |
| O9 | Restart safety and no blind resubmission | **LOCAL COMPLETE** | job digest, persisted broadcast state, vault commitment and executor nonce reconciliation |
| O10 | Exact pre/postconditions for each tail | **LOCAL COMPLETE** | native/ERC20 balances and executor nonce checks |
| O11 | Disclose Hyperliquid Nitro finalization between return tails | **LOCAL COMPLETE** | required `external_gates` entry in immutable HL return jobs |
| O12 | Foundation-hosted relayer | **OPTIONAL PHASE 2** | self-run permissionless CLI is the MVP |

### Venue actions

| ID | Requirement | Status | Concrete evidence |
| --- | --- | --- | --- |
| V1 | Lighter unwrap plus native ETH deposit | **LOCAL COMPLETE + LIVE EVIDENCE COMPLETE** | `lighter_batch`; live probe credited 0.002 ETH |
| V2 | Lighter official-SDK API-key registration, unified mode and ETH margin | **LOCAL COMPLETE / EXTERNAL GATE live** | `shielded_exit_venues.lighter_enable_eth_margin`; failed registration cannot persist an unusable key |
| V3 | Lighter return withdrawal | **LOCAL COMPLETE / EXTERNAL GATE live** | official SDK withdraw, registered key index and fixed owning `E_i` destination |
| V4 | Hyperliquid Ethereum native bridge and Arbitrum exact-output deposit | **LOCAL COMPLETE + LIVE EVIDENCE COMPLETE** | executor batches; live probe credited 5 USDC and left burner USDC at zero |
| V5 | Hyperliquid withdrawal to a distinct fresh destination | **LOCAL COMPLETE / EXTERNAL GATE live** | official SDK `withdraw_from_bridge`; rejects owner as return destination |
| V6 | Hyperliquid return exact-output ETH and ArbSys exit | **LOCAL COMPLETE / EXTERNAL GATE live** | `hyperliquid_return_arbitrum_batch` plus immutable return job |
| V7 | Nitro challenge and L2-to-L1 outbox finalization | **EXTERNAL GATE** | required job metadata and mainnet native-balance precondition |
| V8 | Return leftover relayer probe ETH | **EXTERNAL GATE** | manager decision because it moves existing live funds |

### pfETH route and deployment sequence A1-A7

| ID | Requirement | Status | Concrete evidence |
| --- | --- | --- | --- |
| A1 | WETH-specific Ethereum ingress guest, slot-3 policy, tests and vkey | **LOCAL COMPLETE** | `programs/pfeth-eth-mainnet-ingress`; shared verifier in `programs/pfusdc-eth-mainnet-ingress/src/lib.rs`; vkey in package |
| A2 | PFETH asset id, precision, route and execution selection | **LOCAL COMPLETE** | `crates/types` and `crates/execution` changes plus focused tests |
| A3 | Deploy WETH-bound `PFTLFinalityVerifierV1` | **LOCAL COMPLETE package / EXTERNAL GATE live** | deterministic init code and predicted address in deployment package; review/fresh checkpoint/funds required |
| A4 | Deploy `WETHBridgeVaultL1` | **LOCAL COMPLETE package / EXTERNAL GATE live** | deterministic init code, predicted address and runtime-hash readback check |
| A5 | Bootstrap PFETH and activate governed route profile | **LOCAL COMPLETE package / EXTERNAL GATE live** | PFTL operation bundle and `route-activation-instructions.json`; authorized issuer/validator actions required |
| A6 | Full 0.01 ETH Lighter round trip | **EXTERNAL GATE** | requires A3-A5, live funds and a priced paid GPU proof; no live claim |
| A7 | Full 0.01 ETH Hyperliquid round trip | **EXTERNAL GATE** | requires A3-A6, live funds, cross-chain time and a priced paid GPU proof; no live claim |

### Return flow, interface and phase 2

| ID | Requirement | Status | Concrete evidence |
| --- | --- | --- | --- |
| J1 | Lighter owner-`E_i` wrap and WETH-vault return | **LOCAL COMPLETE / EXTERNAL GATE live** | return job uses existing delegation with batch nonce 1 |
| J2 | Hyperliquid fresh-`C_i` conversion, Nitro exit and vault return | **LOCAL COMPLETE / EXTERNAL GATE live** | two-chain return job uses fresh authorizations with batch nonce 0 |
| K1 | Observer cannot report the bound as passing when timing narrows it | **LOCAL COMPLETE** | enumerator reports `public_timing_narrows_set`; truncation reports `inconclusive` |
| U1 | Human-operable Python CLI | **LOCAL COMPLETE** | `stakehub shielded-exit`: plan, materialize, activate, observe, analyze, prepare, relay and venue actions |
| U2 | User-facing interface | **LOCAL COMPLETE** | `StakeHub/dashboard/shielded-perp-funding.html`; served at `/shielded-exit`; public-only status endpoint |
| Q1 | Private split/merge | **OPTIONAL PHASE 2** | excluded |
| Q2 | Batched burn proof | **OPTIONAL PHASE 2** | excluded |
| Q3 | Protocol-level `k_min` | **OPTIONAL PHASE 2** | excluded |
| Q4 | Selective-disclosure export | **OPTIONAL PHASE 2** | excluded |

## Deployment package identity

| Field | Value |
| --- | --- |
| Manifest SHA-256 | `75129786b578fa4f6990834376d2675bd33616825cf1e4be59e06ec23bd66dac` |
| Ingress ELF SHA-256 | `d49f826e9d8850e3d4762e475a2c7841b82b30dd79a78bde2d7cd449ea9b12aa` |
| Ingress vkey | `0x000dd724902a8a24abde9f3fda01477e0d454ac506fbb6a0613fd4486ad4953c` |
| Egress ELF SHA-256 | `0ea4dbabc8a36b44824c861cf2e1ae90454df2a6e4bb8a357f9d3a8672696d83` |
| Egress vkey | `0x00140f09ca6f1b917e3999a806df5ef4ac4468bd65b85e8580cbc16f832dfe47` |
| Predicted verifier | `0xBCF1Aab56BA21edbc5b77124a7cE8b4C68B535ca` |
| Predicted vault | `0x40187922Ab0761C11E50Bb971cc5a8c77164BEc4` |
| Vault runtime hash | `0x2453f23d0ca464f4599f0c2526a7f82867a1058fec3f62751bdae2a0b8a9cd81` |
| Route-profile hash | `c0e981d6b0b6fe6731068476e63271604c39938c20989d8c8aaed871b4abca638728fa81fca7eca06e05a00aa553e40c` |
| Route binding | `fa7628c20373728be6fd15605cbc436133a4e58b015334527f4080ee7576f9b3` |
| Review gate | `PENDING` |
| PFTL checkpoint gate | `PENDING`; height 924 package input is historical |

## Live evidence retained

| Narrow claim | Evidence |
| --- | --- |
| Ethereum EIP-7702 ExitExecutor deployment | tx `0x370165be7596b126c2914d1dd107cb7a51936b05398cfcb12c91e4306fb50237` |
| Arbitrum EIP-7702 ExitExecutor deployment | tx `0x6ffd63b380fa29c998872718c5132fe874bda332773bfad528ff74fe1c66dbc2` |
| Sponsored Lighter deposit from zero-ETH burner | tx `0xdfe51e718fde962cb2430088e7218f167d08d2eac9c867cf8fe5a5868c447bd1`; account 743108 credited 0.002 ETH |
| Sponsored Hyperliquid Arbitrum tail | tx `0xe56eb0955c4b255214cd74e83fffe86f5ebfa3e7d4f007a3828315c097bde666`; account value 5 USDC; burner USDC zero |

These probes verify only the EIP-7702 venue tails. They do not prove a deployed
pfETH route, a complete private round trip, live Lighter margin enablement or a
live return flow.

## Verification record

### Passing focused gates

- `forge test --match-contract 'WETHBridgeVaultL1Test|ExitExecutorV1Test' -vv`
  — **19 passed**.
- `cargo test --manifest-path programs/pfusdc-eth-mainnet-ingress/Cargo.toml --locked`
  — **8 passed**.
- `cargo test --manifest-path tools/eth-l1-mainnet-fast-lane-p0/Cargo.toml --locked`
  — **8 passed**.
- `cargo test -p postfiat-types pfeth_asset_id_is_deterministic_and_uses_the_nine_decimal_definition --locked`
  — **1 passed**.
- `cargo test -p postfiat-execution pfeth_ethereum_ingress_selects_only_the_weth_route --locked`
  — **1 passed**.
- `cargo test -p postfiat-execution --locked` — **190 passed**.
- `scripts/test-proof-public-input-inventory` — **PASS**, 7 systems,
  150 public fields, 93 source hashes.
- `scripts/test-pfeth-eth-mainnet-package` — **PASS**, deterministic package,
  review/checkpoint gates enforced, zero live transactions.
- StakeHub focused shielded-exit and dashboard suite — **173 passed**.
- StakeHub Python compileall — **PASS**.
- Both repositories `git diff --check` — **PASS**.

### Known repository-baseline failures

- Full `postfiat-types` currently has one unrelated missing golden fixture:
  `crates/types/testdata/genesis_registry_v1.json`.
- Broad `cargo fmt --all -- --check` currently reports pre-existing unrelated
  formatting in:
  `crates/consensus_cobalt/tests/genesis_registry_checker.rs` and
  `crates/storage/src/transactional.rs`.

### Final release gates

Broad Orchard/workspace testing was run once because this milestone crosses
Asset-Orchard ingress/private-egress and proof-verification boundaries.

- Changed Rust packages/standalone manifests and the four touched Solidity
  files pass their format checks.
- `scripts/verify-vendored-halo2` — **PASS** at upstream
  `f6200adaa6ca064d8d2eaa6fcc5e2671232d7249`.
- `cargo test -p postfiat-privacy-orchard --locked` — **88 passed, 19 ignored**;
  its compile-fail doc test also passed.
- `mkdocs build --strict` in StakeHub — **PASS**; generated site output was
  restored so it does not pollute the worktree.
- A real StakeHub dashboard process served `/shielded-exit` and its JSON status;
  Playwright rendered the full page with no page failure — **PASS**.
- `cargo check --workspace --all-targets --locked` — **PASS**.
- `cargo test --workspace --locked` reached `postfiat-node`: **335 passed,
  2 failed, 3 ignored**. Both failures reproduce individually in the unchanged
  node surfaces `tests::consensus_history::cross_view_vote_and_legacy_lock_migration_fail_closed`
  and `tests::replicated_state_activation::ordered_history_v2_active_commit_uses_one_database_transaction_without_jsonl`.
  The relevant long-running
  `wan_devnet_invalid_asset_orchard_swap_proof_is_rejected_and_valid_swap_still_applies`
  test passed. Because Cargo stopped at `postfiat-node`, this is recorded as a
  repository-baseline failure rather than a workspace pass.

## External authorization sequence

1. Independent reviewer changes the exact-manifest review gate to PASS.
2. Operator queries current PFTL finality/committee, rebases if necessary and
   satisfies the short-lived checkpoint gate.
3. Manager authorizes reviewed ExitExecutor redeployments on Ethereum and
   Arbitrum plus the packaged verifier/vault deployment, then updates job
   templates to the reviewed executor addresses.
4. Authorized issuer and validators submit PFETH bootstrap and governed route
   activation; operator verifies finalized accepted receipts and active route.
5. Operator prices the Vast rental and asks the manager for explicit GO.
6. Manager authorizes each 0.01 ETH live round trip and venue withdrawal.
7. Evidence is updated with real receipts; until then A3-A7/J1-J2 remain
   external gates.
