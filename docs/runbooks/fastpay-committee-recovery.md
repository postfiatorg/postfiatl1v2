# FastPay recovery after a validator key rotation

The September 2026 failure occurred because every node compared the entire live
validator registry with the pinned FastPay committee. Rotating validator-5 disabled
all six nodes, although validators 0–4 still had valid committee keys.

The repair authorizes each local signer against both its current registry entry
and its pinned committee entry. It verifies the actual signature before reserving
inputs or writing an acknowledgement. The five-signature quorum, committee epoch,
key rotation, balances and ordered-block rules are unchanged.

The active database also needs the retained FastPay journal when answering wallet
queries and preparing blocks. A signed acknowledgement alone was insufficient on
the old release: it wrote legacy JSON while reads used the finalized database.
The repaired node verifies and reconstructs pending certificates in memory, keeps
the finalized root unchanged, and anchors those effects in the next certified
block. Ordered recovery or an intervening certified omission takes precedence.

The Python wallet and Rust RPC SDK must be updated together. The SDK verifies
certificate votes and acknowledgements, then derives exact transfer output IDs
using the execution engine. Python exposes those verified outputs at the top
level for the existing wallet CLI and retains the raw response under `apply`.
From a clean `main` checkout, update and build the SDK:

```bash
git fetch https://github.com/postfiatorg/postfiatl1v2.git main
git merge --ff-only FETCH_HEAD
cargo build --release -p postfiat-rpc-sdk
```

Point `fastpay.python_root` at that checkout's `python` directory,
`fastpay.repo_root` at the checkout, and `ce22.rpc_sdk_binary` at
`target/release/postfiat-rpc-sdk`. Restart the wallet UI after updating so it
loads the matching Python code.

## Operational limit

Epoch 1 still requires five signatures, and only validators 0–4 are eligible.
Any additional signer outage stops new FastPay certificates. Validator-5 receives
accepted FastPay effects through certified ordered blocks; it does not supply an
epoch-1 acknowledgement. A governed committee replacement remains separate work.

## Client recovery

On the original wallet host, use the existing helper:

```bash
pft-fastpay start
pft-fastpay status
pft op list
pft op status EXISTING_OPERATION_ID
```

Inspect the existing operation before retrying. If its wrap succeeded and its
FastPay transfer did not, resume that same operation:

```bash
pft op resume EXISTING_OPERATION_ID
```

This retains the recorded input and reconciles an ambiguous previous application.
Do not create another wrap to hide an unresolved payment. The incident's original
one-PFT coin belongs to that original client; a payment from another test wallet
does not prove that coin was recovered.

For a new payment, amounts are integer atoms (1 PFT = 1,000,000 atoms):

```bash
pft fastpay send SENDER --to RECIPIENT --amount 1000 --operation-id UNIQUE_ID
pft op status UNIQUE_ID
pft gui
```

Completion requires verified quorum acknowledgements, a spent input and the exact
recipient output. Check Activity in the wallet UI. Verify all six nodes after a
subsequent certified ordered block; service health alone is insufficient.

## Release and backup basis

Use [the safe rollout runbook](safe-validator-rollout.md), preserving the signed
publisher, old binary and unit files. Build from the exact archived deployed
source (`03e1138e`), which captures the deployed `707e006f` working tree, plus the
bounded FastPay and checkpoint-restore repairs.

The mandatory signed backup uses finalized-checkpoint verification. The incident
height-1033 snapshot contains historical block 1011, which fails modern full-history
NAV-supply replay. The repair does not change that invariant or claim full replay
passes. Explicit checkpoint import reconstructs the transactional generation and
then verifies its certified current state, roots, tip, registry and integrity.
Ordinary import and standalone migration still require full historical replay.

The locked specifications are [FastPay authority](../specs/fastpay-committee-local-authority-repair-20260925.md),
[checkpoint restore](../specs/fastpay-checkpoint-restore-prerequisite-20260925.md),
and [transactional reads](../specs/fastpay-transactional-read-view-repair-20260925.md).
See [the deployed repair and payment evidence](../handoffs/2026-09-25_fastpay_restored.md).
