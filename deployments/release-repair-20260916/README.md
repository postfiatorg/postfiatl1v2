# PR41 history and withdrawal identity repairs

**Result: all three technical repair gates PASS. Deployment remains on HOLD for the separate release obligations below.** Source: `1c435f4fb482ea7830bee5c7018d370a10a34bfd`. This packet addresses the three technical blockers in the [September 15 assessment](../combined-release-20260915/README.md); the earlier evidence remains intact. Live deployment and governance activation are separate decisions.

| Repair gate | Result | Evidence |
|---|---|---|
| Full history after the V2 installation | PASS, all six copies at height 1021 | [Run report](history/post-v2-run.json), per-node reports under `history/` |
| Six-node certified V2 installation and restart | PASS | [Service receipt](v2-service-receipt-final.json) |
| Two matching node builds | PASS, clean source trees and separate existing target caches | [Build identities](node-builds.json) |
| Both historical withdrawal proof identities | PASS | [CI and independently checked artifacts](proof-identities.json), raw CI logs under `logs/` |
| Pre-activation rollback | PASS: exact old executable checkpoint/restart on six copies; final candidate independently replays restored validator-0 at height 1020 | [Rollback service receipt](old-binary-service-receipt.json), [full replay](history/final-candidate-on-rollback-validator-0.json) |
| Final workspace tests | PASS: 1,433 passed, zero failed, 39 ignored | [Gate report](software-gates.json), [raw output](logs/workspace-tests.log) |
| Isolated latency, format, proof inventory, check and Clippy | PASS | [Gate report](software-gates.json), raw output under `logs/` |
| Retained commitment size and encoding time | Measured on all 39 captured fences: V1 26,988 bytes; V2 1,973,558 bytes, 0.718 ms mean encoding time | [Measurement scope and report](retained-commitment-measurement.json) |

[Machine-readable assessment](qualification.json) · [Exact commands and limitations](verification-commands.md). Keys, node databases, signed batches and executables stay outside Git.

The original packet's seven reachable-history scan findings and unavailable pinned testnet archive state remain open release obligations. Review, fresh PR CI, deployment and the live V2 activation decision are separate from these passing local repair gates. The newer private-funding branches are not integrated or demonstrated by this packet.

## Repairs

| Blocker | Repair | Owning code |
|---|---|---|
| Historical FastPay committee fails verification | Preserve the V1 root and state bytes. A signed governance installation selects V2, binds admission heights and commits complete retained certificates. Normalize historical vote order for commitments; reject duplicate validators. | `crates/types/src/fastpay_recovery_types.rs`, `crates/execution/src/owned_transfer_recovery.rs`, `crates/node/src/state_commitment.rs` |
| Replay rejects the recorded supply at block 1011 | Pass Orchard balances to the same asset executor used by live execution, preserving the governed accounting activation. | `crates/node/src/execution_actions.rs` |
| Withdrawal rebuild differs from its destination verifier | Reproduce each historical release from its own source inputs and layout; check the selected ELF before execution and derived key before proving. | `programs/pfusdc-egress/releases.json`, `.github/workflows/arc-proof-identities.yml`, `tools/pfusdc-tier4-prover/src/egress_identity.rs` |

V2 installation changes commitments immediately. Its future admission height separately controls new orders. Recovery continues to use the committee that signed each old order. No stored historical certificate is rewritten.

## Operator checks

Use the release's recorded executable on disposable copies of the captured databases:

```sh
"$candidate" verify-state --data-dir "$clone_root/validator-0"
```

Repeat for validators 1 through 5. `verify-state` includes full block replay, governance, bridge, shielded-state and mempool verification. Original captures must verify 1,020 blocks and their original common tip/root; the six continued V2 copies must verify 1,021 blocks and the exact tip/root in the service receipt. Require `verified: true`. These are logical-history checks of downloaded state, not a claim of an atomic fleet backup.

For rollback, restore the exact deployed executable and compatible pre-upgrade data, then run:

```sh
"$rollback" verify-finalized-checkpoint --data-dir "$rollback_root/validator-0"
```

The deployed executable still contains its historical full-replay bug. Its checkpoint verification and local service restart qualify serving the restored certified tip; the repaired verifier independently checks the entire same history. A checkpoint-only snapshot import through that old executable fails because import also performs replay. Use the preserved compatible raw backup for this rollback path.

## Local continuation and restart

The Python [governance gate](run_local_governance_gate.py) reuses the existing six-node service runner. Supply disposable clones, isolated local signers, a six-peer loopback topology, and a signed FastPay V2 rotation batch produced through the node's governance CLI:

```sh
python deployments/release-repair-20260916/run_local_governance_gate.py \
  --binary "$candidate" --source-commit "$node_source_commit" \
  --clone-root "$clone_root" --key-root "$key_root" \
  --topology "$topology" --batch-file "$signed_rotation_batch" \
  --work-root "$service_work" --receipt "$service_receipt"
```

The gate exercises RPC-first and transport-first startup, a certified rotation, its accepted receipt on every node, convergence and restart. Full replay after the rotation verifies its certificate and the V2 commitment over retained V1 history. The original transfer-based exercise could not sign because the captured copies omit the faucet key; the governed rotation uses the six available isolated validator signers.

See the [withdrawal release guide](../../programs/pfusdc-egress/README.md) for the prover's `--egress-release` selection and source reproduction workflow.

No live validator, service, configuration, chain state or destination verifier was changed. Live deployment and V2 activation remain separate operator actions.
