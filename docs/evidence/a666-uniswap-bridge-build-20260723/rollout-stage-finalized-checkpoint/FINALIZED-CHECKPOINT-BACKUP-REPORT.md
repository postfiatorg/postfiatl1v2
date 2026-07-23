# Finalized-checkpoint backup report — 2026-07-23

Status: **GREEN / signed backup verified / rollout held**.

The legacy governance history is not treated as a new consensus input. The
new, explicit finalized-checkpoint path is activation gated and accepts a
backup only when all of the following bind to the same retained chain tip:

- committed `consensus_v2_activation_height` exists and the tip is at or
  above it;
- durable height, block hash, state root, ordered batch, archived payload,
  and recomputed replicated state agree;
- the current validator registry and exact consensus-v2 commit/precommit QC
  validate at quorum;
- QC height, parent, payload hash, state root, bridge-exit root, block ID,
  proposer, view, committee epoch, and domain all match the checkpoint;
- every checkpoint-referenced receipt exists exactly once.

Full-history snapshot verification remains the default for the existing
commands. The new commands are explicitly named
`*-finalized-checkpoint`; missing activation, missing QC, root/certificate
tampering, or a receipt journal smaller than the canonical set fails closed.
The only legacy metadata accommodation is that the durable raw receipt journal
may be larger than the canonical unique receipt set because this fleet retains
a rejected receipt before a later accepted retry of the same transaction ID.

The safe-rollout backup copied the exact staged candidate into the isolated
snapshot directory, checked its SHA-256, exported the validator-1 checkpoint,
created an ML-DSA-65 signed snapshot, imported it into a fresh directory, and
ran the finalized-checkpoint verifier again.

## Live evidence

- candidate commit: `8949759410e5167b3d3853d9d614418354e1c2c6`
- candidate binary SHA-256:
  `dc448115eb2b65f699fcfbf809bb7fbaec71984f8ad9b448f98fdbc470e65d65`
- verification basis: `consensus-v2-finalized-checkpoint`
- activation height: `1`
- checkpoint height: `296`
- checkpoint block:
  `7bea52c025b519ed3c1f60cf9c3afd1fa11416b063b1dd40998ec9ef655da514c084790a2d45a1a9a656e4eca500bf22`
- checkpoint root:
  `ae09bfefa1b870c3aacda61913c850d836395b3e6bb74c00bf62e5c28445634a8b267cd74b896f228af625ecbf418296`
- certificate:
  `2059c875f4ba55364e5a40fa4545547110298eda8571a561490b72fc2b88228ed71e407f1480e602efbf97021fbff72e`
- committee: epoch `1`, validators `6`, quorum `5`
- signed manifest SHA-256:
  `23260ee390b187fc841f344aea24529c9bb59941dad1eaa88770f55590d153da`
- checkpoint report SHA-256:
  `182775a6123944f65e974c5a80ee417019766b72d8ecf858505228b840db03c2`
- rollout state: `preflight.verified=true`,
  `backup.verified=true`, `applied=[]`

Post-backup RPC checks remained converged at the same height, tip, and root,
with empty mempools on all six validators. All six active services still run
SHA-256 `0c27df02d0e59f89deafb7f2d9d7fed96bcc714f04a1949c48d9b7a7dfc12a2c`.
No validator was restarted.

## Tests

- `cargo test -p postfiat-node snapshot_deployment`: 11 passed, 0 failed.
- `PYTHONPATH=python python3 -m unittest python.tests.test_safe_rollout -v`:
  14 passed, 0 failed.
- `cargo test -p postfiat-node vault_bridge`: 24 passed, 0 failed,
  2 real-Anvil tests intentionally ignored.

The staged source retains proof-bounded cap growth: commit `8f94cf5` is an
ancestor of the candidate. The release binary embeds revision `89497594`.

The next command is mechanically available only after explicit rollout GO.
It was not run:

```text
scripts/postfiat-safe-rollout apply-next --state-file docs/evidence/a666-uniswap-bridge-build-20260723/rollout-stage-finalized-checkpoint/rollout-state.json
```
