# NAVCoin, Cobalt, and storage: machine handoff

This branch collects the unpublished NAVCoin work and the current Cobalt/Task Node sources. It is a development handoff. **The September 7 Ethereum → pfUSDC → A666 → Uniswap → A666 → pfUSDC → Ethereum reproduction is incomplete.** The initial attempt made no new USDC deposit. The resumed run has since deployed and activated epoch 10 and deposited 10 USDC; see the [current external-route record](../status/NAVCOIN-EXTERNAL-ROUTE-20260907.md). Do not repeat that deposit.

## Check out the working set

Keep the repositories as siblings named `postfiatl1v2` and `StakeHub`; the StakeHub native transaction inspector uses a relative Cargo dependency on L1.

```bash
git clone --branch handoff/navcoin-cobalt-local-20260907 https://github.com/postfiatorg/postfiatl1v2.git postfiatl1v2
gh repo clone postfiatorg/StakeHub StakeHub -- --branch handoff/navcoin-local-20260907
cd postfiatl1v2
git submodule update --init --recursive
rustup show
cargo build --locked -p postfiat-node
# For the Solidity and pfETH package checks, install Foundry first.
forge build --root crates/ethereum-contracts
scripts/test-pfeth-eth-mainnet-package
python3 -m venv .venv
.venv/bin/pip install -r requirements-test.txt
PYTHONPATH=python .venv/bin/python -m pytest -q python/tests/test_tasknode_unl*.py python/tests/test_cobalt*.py
cd ../StakeHub
python3 -m venv .venv
.venv/bin/pip install -e . pytest mkdocs-material
PYTHONPATH="../postfiatl1v2/python:$PWD" .venv/bin/python -m stakehub.cli --help
```

Rust is pinned in `rust-toolchain.toml`. SP1 is needed to generate bridge/NAV proofs; the ordinary node build and Python checks do not establish proof artifact identity. Follow the pinned program manifests and existing proof build instructions for the specific deployed verifier, rather than replacing its ELF with a newly built guest.

The L1 PR is stacked on [Arc integration PR #37](https://github.com/postfiatorg/postfiatl1v2/pull/37), and also merges public `main` at `f2f09881`. This branch contains both prerequisites, so checking it out does not require manually applying their patches. Merge #37 before retargeting this PR to `main`. The companion StakeHub PR targets `master`.

## What is included

| Area | Source and entry point | State |
| --- | --- | --- |
| Cobalt and Consensus v2 | `crates/node/src/cobalt_handoff.rs`, `python/postfiat_rpc/cobalt.py`; [activation handoff](2026-08-25___postfiatchad__cobalt_governance_activation.md) | Already published; included through the prerequisite branches. Cobalt ratifies bounded validator-trust changes. Consensus v2 orders/finalizes blocks. |
| Task Node UNL | `python/postfiat_rpc/tasknode_unl*.py`; [MVP handoff](2026-09-04___dravlic__tasknode_unl_mvp_built_and_hardened.md) | Offline shadow derivation, not live governance authority. |
| Transactional storage | `crates/storage/src/transactional*`; [storage gate handoff](2026-08-31___postfiatchad__storage_gate_passed_rollout_pending.md) | Existing transactional storage/ordered-history work is included. Cross-process database access uses exclusive, operation-scoped leases; persistent readers are not qualified. |
| RPC status P1 repair | `crates/node/src/rpc_cli.rs`; [repair handoff](2026-09-06___codex__rpc_status_cache_fix.md) | Deployed September 6; expiration forces fresh status even when legacy JSON metadata is unchanged. |
| Historical pfETH replay | `crates/node/src/block_replay_wallet.rs`, `crates/execution/src/nav_vault_asset_execution.rs`, `crates/node/testdata/pfeth-reserve-replay/` | Preserves the narrowly pinned historical reserve-packet semantics while new packets use family supply. |
| Ethereum pfETH bridge | `programs/pfeth-eth-mainnet-ingress/`, `crates/ethereum-contracts/src/WETHBridgeVaultL1.sol`, `scripts/pfeth-eth-mainnet-*.py` | Local source, package, and deployment evidence preserved. |
| A666 source custody | `crates/execution/src/pftl_source_settlement.rs`, `crates/types/src/market_nav_asset_types.rs`, `crates/node/src/state_commitment.rs` | Deployed September 7. Signed source selection keeps primary issue/redemption in the selected pfUSDC source series. |
| StakeHub | Companion branch: `stakehub/navcoin_roundtrip.py`, `stakehub/governed_reserves.py`, CLI/dashboard, bridge/funding modules and tests | Native round-trip runner exists. It does not complete the external Ethereum/Uniswap route. |
| Paused private funding | `docs/specs/shielded-*`, `docs/status/shielded-*`, StakeHub `shielded_exit_*` | Preserved for continuity. No claim that the private Hyperliquid/Lighter objective succeeded. |

The [source inventory](../../deployments/a666-source-route-20260907/local-source-inventory.json) records copied local files and their original hashes. The three unpublished pfETH commits (`ef2dec31`, `1bcd0f0d`, `707e006f`) were cherry-picked; the unpublished source-custody delta was merged with the current Arc and main sources. Original dirty checkouts were left untouched.

Generated StakeHub `site/`, egg metadata, local `.postfiat` state, compiler output, wallet/key material, and the unrelated fill-export script are not part of this handoff. The older A666 checkout's historical deployment trees are not copied wholesale. Its exact deployed A666 proof ELF and the current attempt's witness are archived in the private StakeHub companion repository.

## Verified recovery point and deployed source

Initial handoff recovery observation: September 7, 2026, block **1005**, all six validators agreed, mempools empty. State root:

```text
6ed69ca9479b291c01d8265a914d1c005e28964496e5e63f4f93458401189f09cc64f046b4307fe9ee299419ced065f9
```

Release: `a666-source-route-20260907`; deployed binary SHA-256:

```text
57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83
```

[Deployment evidence](../../deployments/a666-source-route-20260907/) contains the source manifest, tracked source patch, separately preserved untracked source, six deployment observations, and historical test/replay reports. The original deployed base is retained as branch `handoff/navcoin-deployed-base-20260907` at `707e006f`. To reconstruct its source, use that base, apply `source.patch`, copy `untracked-source/` over the checkout, and copy the public `crates/node/testdata/pfeth-reserve-replay/` fixture from this handoff. Verify every path against `source-manifest.json`. Compiler/toolchain identity also matters to binary reproducibility.

**This consolidated branch is not the exact deployed source tree.** It merges later main/Arc content. Do not label its build with the deployed binary hash or assume a new build is authorized for deployment just because it compiles.

## Where the initial full NAVCoin attempt stopped

Read the [round-trip definition](../runbooks/A666-ROUND-TRIP-DEFINITION.md). The required route includes both Uniswap directions, export/mint, return burn/import, NAV redemption into the same pfUSDC source, external USDC withdrawal, PFTL settlement, and final reconciliation.

1. Source-custody support was implemented, tested and deployed. The original A666 primary market had used the base pfUSDC family while modern deposits issue source-series assets. New signed source selection and issuer allowlisting preserve that backing through primary custody; legacy redemption cannot drain source custody.
2. Old Ethereum pfUSDC epoch-6 verifier checkpoint 909 predates Cobalt committee rotations at 917 and 924. Its frozen proof path expects the previous transition-authorization representation. The existing attempt did not produce a compatible end-to-end proof across those changes. Do not fabricate old authorizations or bypass committee verification.
3. An empty replacement verifier/vault pair was deployed with epoch 7. Activation was rejected at block 1004: route epochs are **global per asset**, and Arc had already reached pfUSDC epoch 9. The prior Arc proof-profile binding was restored successfully at 1005. The epoch-7 contracts are unusable for this activation and must not be funded. The next epoch must be derived from authoritative governance again before any immutable deployment.
4. The A666 881→917 witness passed execution: 544,812,784 instructions, about 14.65 seconds. The CPU prover subsequently disappeared without a proof. Termination cause was not established. Execution success is not proof success. No prover remains running from that attempt.
5. No new USDC was deposited. Protected pre-existing balances at recovery were 534.079891 Ethereum USDC and 103 wA666. Epoch-7 deployment gas was approximately 0.00022545 ETH. Actual Ethereum receipts, public balances, restored fleet state, witness, proof ELF, and historical scripts are in the private StakeHub `docs/handoffs/navcoin-recovery-20260907/` archive.

The archive's Python scripts are saved as `.py.txt`: forensic source, with original absolute paths and invalid epoch-7 assumptions retained. They are not a portable execution command. First resolve proof compatibility and complete a globally valid deployment package preflight. Then wire the actual full traversal through StakeHub and retain every accepted receipt. No artificial funding minimum or mandatory GPU purchase is part of the objective.

## Cobalt, storage, and Task Node issues to carry forward

The [September 6 review](../review/storage-cobalt-tasknode-handoff-review-20260906.md) distinguishes the deployed P1 repair from open P2 findings:

- Task Node shadow admission does not require a complete, consistent wallet/account mapping; missing mappings can erase evidence of common funding.
- Candidate key identity is not joined to the authenticated binding key.
- The consolidated historical Cobalt packet verifier fails because it binds mutable publication documents. The focused Cobalt tests passing does not repair that packet. Preserve publication bytes or bind the immutable source revision; do not loosen cryptographic checks.
- Some storage plan prose overstates concurrent-reader support and retains superseded deployment status. Reconcile it against receipts before changing the process topology.

Task Node output remains `SHADOW_ONLY`. Cobalt retains its bounded role. The six-node Foundation-operated deployment is not evidence of independent operator decentralization. Follow the existing Z3 and Task Node plans; this handoff does not promote any pending admission-policy decision.

## Validation and operational setup

See [combined-branch validation](../status/NAVCOIN-HANDOFF-VALIDATION-20260907.md) for fresh combined-branch results and known failures. Historical deployment reports are explicitly separate.

The new machine can build, test and inspect the public evidence immediately. Live operation also requires its own authorized RPC/SSH access, configured endpoints, proof artifacts and unlocked StakeHub signer. None of those credentials is transferred through Git. The archived profile records the original machine's paths and loopback ports; they do not become valid on a second host automatically. Re-query all six validators, source-chain balances, governed route/profile and NAV freshness before signing a new transaction. Reconcile an existing signed request before any retry.
