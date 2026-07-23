# a666 blocker-clearance continuation — 2026-07-23

Status: **A GREEN / B requires backed bridge-in / rollout held**.

## A — signed backup

The activation-gated finalized-checkpoint verifier is implemented and tested
without weakening the existing full-history verifier. It validates the exact
retained consensus-v2 QC, committee/domain, certified block and state root,
archived payload, ordered batch, and checkpoint receipt bindings. Tampered
state, certificate, and receipt accounting fail closed.

The fresh safe-rollout preflight passed at 6/6 and the ML-DSA-65 backup now
has `backup.verified=true`. The rollout state has `applied=[]`. All six active
validators remain on the prior binary, height 296, identical tip/root, and
empty mempools. No service was restarted.

Evidence:
`rollout-stage-finalized-checkpoint/FINALIZED-CHECKPOINT-BACKUP-REPORT.md`.

## B — pfUSDC funding

The height-296 signed checkpoint has zero transparent pfUSDC across all seven
trustlines and zero live Orchard pfUSDC. The full finalized 10-atom supply is
already held in a pending vault-bridge redemption queue owned by the target
holder; it is not transferable inventory.

No mint and no transfer were attempted. Funding requires a new proof-backed
dust bridge-in or the separately authorized founder finalizing five-dollar
bridge-in.

Evidence: `live-ce22/08-pfusdc-inventory/`.

## Holds preserved

- ce22 rollout remains held for explicit founder/nazgul GO.
- no mainnet spend, pool, or liquidity action occurred.
- the founder five-dollar job was untouched.
- the route remains labeled `CONTROLLED`; no trustless claim is made.
