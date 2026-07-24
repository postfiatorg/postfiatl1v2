# Synthetic PFTL Lightning-demo harness

This directory is the PFTL adapter for the zero-value Lightning/NAVcoin demo.
It consumes, but does not build or modify, the consensus binary from the
parallel escrow hardening track.

Safety properties:

- requires exactly six declared validators;
- requires a `local-`, `devnet-`, or `regtest-` chain id;
- pins `POSTFIAT_NODE_BIN` by SHA-256 and reported git revision;
- runs a fresh semantic probe and refuses binaries that accept a
  non-canonical SHA-256 hashlock profile, a wrong hashlock witness, an early
  cancel, or a finish at `cancel_after`;
- creates only `LNNAVTEST`, a synthetic issued asset with authorization,
  freeze, and clawback all disabled;
- routes each accepted operation to the deterministic proposer and finalizes
  it through the existing peer-certified transport;
- reads receipts, balances, escrow state, supply, height, tip, and state root
  independently from all six RPCs;
- publishes a secret-free proof for every effect containing the signed block
  proposal, full vote certificate, validator public-key registry, certified
  receipt aggregate, batch identity/hash, and six post-state statuses;
- accepts caller-supplied `effect_key` values and returns secret-free,
  idempotent effect mappings for the coordinator journal;
- stores full signed/finality material below `private/` (mode `0700`) because
  a transparent escrow finish necessarily discloses its preimage.

The one-validator-down case omits one consensus voter, requires a five-vote
certificate, proves the five online replicas converge, applies that exact
certified batch to the lagging replica, and then requires 6/6 convergence.
That is a controlled local outage/catch-up test, not a public decentralization
claim.

Typical control sequence:

```bash
export POSTFIAT_NODE_BIN=/absolute/path/from/orc2/postfiat-node
export POSTFIAT_NODE_GIT_REV=<orc2-commit>

scripts/lightning-navcoin-pftl-devnet probe
scripts/lightning-navcoin-pftl-devnet init --root /tmp/pftl-ln-demo
scripts/lightning-navcoin-pftl-devnet up --root /tmp/pftl-ln-demo
scripts/lightning-navcoin-pftl-devnet status --root /tmp/pftl-ln-demo
scripts/lightning-navcoin-pftl-devnet one-validator-down \
  --root /tmp/pftl-ln-demo --effect-key chaos-one-down-1
scripts/lightning-navcoin-pftl-devnet down --root /tmp/pftl-ln-demo
```

Coordinator integration imports `PftlDevnet` and calls `submit_create`,
`submit_finish`, or `submit_cancel`. Each returns:

```json
{
  "accepted": true,
  "reason": "accepted",
  "tx_id": "...",
  "finalized_height": 12,
  "state_root": "...",
  "block_tip_hash": "...",
  "agreeing_validator_count": 6,
  "validator_count": 6,
  "receipt_count": 6,
  "certificate_id": "...",
  "finality_proof_path": "/tmp/.../evidence/finality/12-swap-id:pftl-finish.json",
  "finality_proof_sha256": "...",
  "effect_key": "swap-id:pftl-finish",
  "escrow_id": "..."
}
```

The returned mapping intentionally contains no preimage or fulfillment.
`public_finality_proof(effect_key)` (or the `finality-proof` CLI command)
hash-checks and returns the corresponding public certificate bundle, so an
integrated runner can copy it into its immutable evidence package without
touching the signed escrow payload stored below `private/`.

Before locking, `plan_create` reads finalized state 6/6 and returns the
owner's next sequence and deterministic `expected_escrow_id`; `submit_create`
requires and verifies that id. Adversarial tests use
`submit_expected_rejection` and `submit_duplicate`. Those methods put the
signed invalid/replayed transaction into a peer-certified block and require
its literal rejected receipt on all six replicas. A receipt-bearing rejected
block legitimately advances height, block tip, and aggregate state
commitment, so mutation-freedom is instead enforced over the full relevant
application projection: both principals' account records (including native
balance and sequence), every test-asset trustline, owner and recipient escrow
indexes, the target escrow, and asset supply/control state.

An exact transaction replay reconstructs an already-applied batch id and is
rejected before proposal, which cannot produce the consensus receipt required
by the acceptance suite. `submit_duplicate` therefore creates a fresh signed
envelope carrying the same finalized operation and stale sequence; it changes
only the fee by one atom to obtain a distinct batch. All six validators then
execute it and reject it with `bad_sequence`, while the full application
projection remains byte-for-byte equal. `restart-proof` hard-kills all six RPC
readers, restarts them, and verifies their durable finalized state is
unchanged.
