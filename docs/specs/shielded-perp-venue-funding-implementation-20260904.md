# Shielded Perp-Venue Funding: Implementation Milestone

Date: 2026-09-04  
Status: **local MVP complete; deployment and live-funds gates open**  
Design companion: `docs/specs/shielded-perp-venue-funding-mvp-spec.md`  
Requirement ledger:
`docs/status/SHIELDED-PERP-VENUE-FUNDING-COMPLETION-MANIFEST-20260904.md`

Legend:

- `[x]` implemented and verified locally, or backed by the cited live probe.
- `[~]` packaged and fail-closed, but awaiting an external action.
- `[ ]` optional phase 2.

## 0. Claim boundary

The MVP breaks a deterministic public treasury-to-burner funding edge by using a
shared WETH vault, standard 1/2/5/10 ETH bands, delayed Asset-Orchard notes,
fresh PFTL and EVM accounts, and relayer-paid EIP-7702 batches.

It does **not** provide private trading. Lighter uses zero-knowledge proofs for
rollup validity; account ownership, deposits, balances and positions remain
public. Hyperliquid venue activity is also public. Amount, timing, a thin vault,
the shared PFTL fee sponsor, RPC operators, relayers and the local planner can
all reduce the practical anonymity set. The outbound builder therefore releases
only after a historical same-band candidate gate passes, and the offline
observer reports the residual public linkability. This gate is evidence about
the observed cohort, not a posterior anonymity guarantee for one user.

## 1. End-to-end milestone

| ID | Flow | Status | Evidence or gate |
| --- | --- | --- | --- |
| A | WETH deposits into the Ethereum vault; vault accounts in 9-decimal pfETH atoms | [x] local / [~] live | `WETHBridgeVaultL1.sol`; 6 Foundry tests; external review and deployment pending |
| B | Ethereum-finality ingress proof mints pfETH through the WETH route | [x] local / [~] live | pfETH guest, route constants, execution selection and distinct vkey complete; route activation pending |
| C | Public pfETH becomes fixed-band Asset-Orchard notes | [x] generic path / [~] pfETH live | existing Asset-Orchard path; needs active pfETH asset and route |
| D | Planner schedules delayed fresh-account exits and enforces the candidate gate | [x] | StakeHub planner, observer and CLI |
| E | Fresh PFTL account `P_i` is materialized and activated from the shared fee sponsor | [x] local / [~] live | canonical wallet keygen and accepted-receipt activation gate; live sponsor transfer pending |
| F | Private egress reaches `P_i`; `bridge_out` burns pfETH to EVM burner `E_i` | [x] generic path / [~] pfETH live | existing private-egress/bridge path; pfETH live round trip pending |
| G | Permissionless relayer verifies the withdrawal and releases WETH to `E_i` | [x] local / [~] live | immutable signed job, proof-artifact hashes, durable restart state; vault deployment and proof pending |
| H1 | Lighter tail unwraps WETH and deposits native ETH from `E_i` | [x] | live type-4 probe credited account 743108; official SDK margin action implemented |
| H2 | Hyperliquid tail bridges ETH, swaps exact-output USDC and transfers to Bridge2 | [x] | live Arbitrum type-4 probe credited 5 USDC; exact-output builder and official SDK withdrawal implemented |
| J | Venue return reaches the WETH vault in a standard band | [x] local / [~] live | Lighter owner-`E_i` return and Hyperliquid fresh-`C_i` return jobs; live withdrawals pending |
| K | Public-data observer measures candidate sets and enumerates possible ingress/release matchings | [x] local / [~] live cohort | incremental observer and offline linkability test; deployed-vault cohort pending |

## 2. Implemented surfaces

### 2.1 Ethereum contracts

- [x] `ExitExecutorV1.sol`: EIP-712 batch authorization, EIP-7702 sponsored
  execution, ERC-7201 nonce, expiry, atomic calls, low-s enforcement and no
  admin custody.
- [x] 13 `ExitExecutorV1` Foundry tests.
- [x] Existing live probes use:
  - Ethereum: `0x0c51DB40B16691319E027Ae7E1fb71A8D4F2b8bA`
  - Arbitrum: `0xf56aB47F7D720c96145E1404E2630954F6b2F16A`
- [x] `WETHBridgeVaultL1.sol`: immutable WETH/verifier bindings, 9-decimal
  pfETH accounting, exact `1 atom = 10^9 WETH wei`, replay protection,
  proof-gated withdrawals and aggregate u64 supply enforcement.
- [x] 6 WETH-vault Foundry tests, including deposits above the obsolete
  raw-wei u64 ceiling.
- [~] All three contract subjects (`ExitExecutorV1`, the WETH vault and the
  finality verifier) plus both SP1 guests require external review. The current
  `ExitExecutorV1` deployments are unreviewed v1; a reviewed release must be
  redeployed on Ethereum and Arbitrum and its addresses substituted in jobs.

### 2.2 pfETH route in postfiatl1v2

- [x] PFETH asset definition uses precision 9 and deterministic asset id
  `629f1ba8f36963662d6e36243aea85627787cd72d41a9e92c5f01b60e3e68c1b951af771d16780bf13c4036b8d2a4996`.
- [x] Route id `ethereum-mainnet-weth-v1`, chain id 1, WETH9, slot-3
  `balanceOf(vault)` storage binding and WETH runtime code hash are pinned.
- [x] The pfETH guest calls the route-specific WETH verifier and has its own
  reproducible ELF and vkey:
  - ELF SHA-256:
    `d49f826e9d8850e3d4762e475a2c7841b82b30dd79a78bde2d7cd449ea9b12aa`
  - vkey:
    `0x000dd724902a8a24abde9f3fda01477e0d454ac506fbb6a0613fd4486ad4953c`
- [x] Egress ELF/vkey are pinned:
  - ELF SHA-256:
    `0ea4dbabc8a36b44824c861cf2e1ae90454df2a6e4bb8a357f9d3a8672696d83`
  - vkey:
    `0x00140f09ca6f1b917e3999a806df5ef4ac4468bd65b85e8580cbc16f832dfe47`
- [x] Execution selects the WETH route only for PFETH and rejects substitution.
- [x] The proof-public-input inventory now covers seven proof systems, including
  pfETH ingress and egress.

### 2.3 Deployment and governance package

Directory: `deployments/pfeth-eth-mainnet-20260904/`

- [x] Deterministic init code, constructor payloads, predicted addresses,
  runtime hashes, ingress policy, route profile, PFTL bootstrap operations and
  validator-governance instructions are generated together.
- [x] Predicted verifier:
  `0xBCF1Aab56BA21edbc5b77124a7cE8b4C68B535ca`.
- [x] Predicted vault:
  `0x40187922Ab0761C11E50Bb971cc5a8c77164BEc4`.
- [x] Route-profile hash:
  `c0e981d6b0b6fe6731068476e63271604c39938c20989d8c8aaed871b4abca638728fa81fca7eca06e05a00aa553e40c`.
- [x] Route binding:
  `fa7628c20373728be6fd15605cbc436133a4e58b015334527f4080ee7576f9b3`.
- [x] Package manifest SHA-256:
  `75129786b578fa4f6990834376d2675bd33616825cf1e4be59e06ec23bd66dac`.
- [x] Preflight recomputes init-code hashes and deployment addresses.
- [x] Live mode also requires exact nonce, balance, chain, code-hash, review
  and PFTL-checkpoint gates; it validates vault runtime code after deployment.
- [x] Package regeneration preserves matching PASS gates and resets them on
  manifest drift.
- [~] The review gate is `PENDING`.
- [~] The checkpoint gate is `PENDING`. Its height-924 input is historical and
  must be rebased to the active PFTL finalized tip and committee immediately
  before deployment.
- [~] Route activation must use
  `vault-bridge-route-profile-governance`; `bridge_batch_domain` does not
  activate this proof route.

### 2.4 StakeHub operator path

- [x] Planner: denomination decomposition, delay/jitter, sibling-bucket rule,
  HKDF-derived PFTL/EVM keys and encrypted 0600 mapping store.
- [x] Account materializer: canonical PFTL wallet keygen, private account/key
  files and mapping-store update.
- [x] PFTL activation: shared fee sponsor only; preview by default; typed live
  confirmation; accepts success only with a matching finalized accepted
  receipt. This creates a public sponsor-to-`P_i` edge and never uses the
  treasury.
- [x] Incremental Ethereum vault observer: confirmed, reorg-aware withdrawal
  capture and same-band unrelated counts.
- [x] Job builder: reviewed template to immutable 0600 job; copies and hashes
  proof/observer artifacts; rejects sub-band, over-precision and under-`k`
  jobs.
- [x] Permissionless relayer: dry-run default, typed live confirmation,
  durable stages, job-digest pinning, on-chain nonce/replay reconciliation,
  preconditions and postconditions.
- [x] Lighter official-SDK actions: enable unified trading and ETH margin;
  withdraw ETH only to the owning `E_i`.
- [x] Hyperliquid official-SDK action: withdraw USDC to an arbitrary fresh
  destination `C_i`.
- [x] Return jobs:
  - Lighter uses the already-delegated owner `E_i`, batch nonce 1.
  - Hyperliquid uses fresh `C_i` on Arbitrum and Ethereum, batch nonce 0.
    The job explicitly records the Nitro L2-to-L1 challenge/finalization gate.
- [x] Dashboard route `/shielded-exit` shows public plan, relayer and observer
  state without serving the private mapping or keys.
- [x] CLI root: `stakehub shielded-exit`.

## 3. External gates

These items require authority, money, time or independent review. No local code
change can satisfy them honestly.

1. **External security review.** Review all three contracts, ingress/egress guests,
   public-value bindings, 9-decimal scaling, constructor bytes and predicted
   addresses; sign `external-review.json`.
2. **Fresh PFTL state.** Query the active validator fleet, rebase the package to
   its finalized tip/committee, attach an authenticated redaction-safe receipt
   and set `pftl-checkpoint-gate.json` to PASS within the freshness window.
3. **Reviewed deployments.** Manager authorizes live ETH spend after gates 1
   and 2: redeploy the reviewed ExitExecutor on Ethereum and Arbitrum, then
   deploy the packaged verifier and WETH vault. Substitute the reviewed
   executor addresses in every job template. The deploy tooling refuses live
   submission without its exact approval gates.
4. **PFTL bootstrap and route activation.** Authorized issuer/validator keys
   submit the generated PFETH profile/asset operations and governed route
   profile. Success requires finalized accepted receipts plus active-route
   readback.
5. **Paid proof generation.** A Vast GPU rental incurs external spend. Surface
   the current price and ETA and obtain an explicit manager GO before renting.
6. **Live 0.01 ETH round trips.** Manager supplies funds and authorizes the
   Lighter and Hyperliquid round trips after deployment and activation.
7. **Live venue return tests.** A funded Lighter account and a funded
   Hyperliquid account must sign actual withdrawals.
8. **Arbitrum return finalization.** Hyperliquid return waits through the Nitro
   challenge period and needs a permissionless L2-to-L1 outbox finalization
   before the Ethereum ingress tail can execute.
9. **Probe-relayer ETH disposition.** Returning or retaining the small live
   probe balances remains a manager funds decision.

## 4. Optional phase 2

- [ ] Private split/merge for odd amounts.
- [ ] Batched burn/finality proofs.
- [ ] Protocol-enforced `k_min`.
- [ ] Selective-disclosure export.
- [ ] Additional denominations or assets.
- [ ] Delete the Hyperliquid USDC swap if the venue accepts ETH collateral.

## 5. Verification snapshot

Verified on 2026-09-04:

- `forge test --match-contract 'WETHBridgeVaultL1Test|ExitExecutorV1Test' -vv`
  — 19 passed.
- `cargo test --manifest-path programs/pfusdc-eth-mainnet-ingress/Cargo.toml --locked`
  — 8 passed.
- `cargo test --manifest-path tools/eth-l1-mainnet-fast-lane-p0/Cargo.toml --locked`
  — 8 passed.
- `cargo test -p postfiat-execution --locked` — 190 passed.
- `scripts/test-proof-public-input-inventory` — 7 systems, 150 public fields,
  93 source hashes.
- `scripts/test-pfeth-eth-mainnet-package` — deterministic package and
  fail-closed deployment gates passed.
- StakeHub shielded modules plus dashboard:
  `.venv/bin/pytest -q tests/test_shielded_exit.py tests/test_shielded_exit_cli.py tests/test_shielded_exit_jobs.py tests/test_shielded_exit_observer.py tests/test_shielded_exit_pftl.py tests/test_shielded_exit_relayer.py tests/test_shielded_exit_venues.py tests/test_dashboard_server.py`
  — 173 passed.

The full final workspace and Orchard/Halo2 boundary gates are recorded in the
completion manifest after they run.

## 6. Operator entry points

```bash
stakehub shielded-exit observe ...
stakehub shielded-exit plan ...
stakehub shielded-exit materialize ...
stakehub shielded-exit pftl-activate ...
stakehub shielded-exit observer-test ...
stakehub shielded-exit job-template ...
stakehub shielded-exit job-prepare ...
stakehub shielded-exit lighter-enable ...
stakehub shielded-exit lighter-withdraw ...
stakehub shielded-exit hyperliquid-withdraw ...
stakehub shielded-exit relayer once ...
stakehub shielded-exit relayer run --live ...
stakehub dashboard
```

No pfETH contracts, asset, route or 0.01 ETH round trip were deployed or
executed by this milestone.
