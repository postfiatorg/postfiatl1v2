# pfUSDC backing headroom and Ethereum ingress

> **Legacy backing migration:** The active
> `deprecated-Arbitrum-legacy` lineage record is
> `docs/evidence/pfusdc-eth-campaign-20260725/findings/legacy-arbitrum-one-active-route.md`.
> Its campaign figures are **5,999,000 vault atoms** (`5999000`) against
> **1,000,010 circulating atoms** (`1000010`). The signed height-310 audit
> attributes the active legacy source to chain **42161**, governed vault
> `0x850e4ceea147f3551c68c2251129e5945d0afb58`, and PFTL finalized height
> **310**: `V=6000020`, `S=1000000`, `D=0`, `B=10`, `R=0`,
> `RHS=1000010`, residual **`+5000010`**. That exact surplus is recoverable
> issuer value, not new Ethereum backing and not tolerance; pre-Ethereum
> supply must eventually be reconciled to the Ethereum domain or redeemed out.
> New Ethereum claims never use a legacy-domain fallback.

## Scope and prerequisites

This runbook has two distinct paths. Select exactly one after a fail-closed
preflight:

1. **Proof-backed Ethereum ingress (normal path):** a finalized Ethereum
   deposit proof authorizes a new pfUSDC claim. The claim is self-checkpointing:
   it credits the recipient, raises circulating supply, advances the epoch, and
   commits the reserve packet hash in the same consensus transaction.
2. **Manual headroom checkpoint (exception path):** only for a non-proof-backed
   event, a historical mismatch state, or another asset whose governing
   specification authorizes a separate checkpoint. It is not part of normal
   proof-backed Ethereum ingress.

Do not use navswap, a band operation, a raw-ledger edit, a historical backing
record, a consumed authorization, or an inventory transfer as a substitute for
new proof-backed issuance. Existing issued inventory may be transferred only
when the applicable credit code is inventory-transfer based; it is not backing
for a minting ingress route.

Before any signing, require:

- a 6/6 direct-host common parent read using the Lane A method documented in
  `docs/evidence/pfusdc-eth-campaign-20260725/lane-a/live-state/collect.sh`;
- the controlling lane's green decision for the deployed binary and fleet;
- a frozen source-chain route/profile, exact source deposit receipt/evidence,
  finality reference, and proof/public-values files from the route owner;
- an authorized, unfrozen recipient trustline with enough limit;
- a current, unexpired, unconsumed record and an empty mempool;
- a source-vault conservation audit that supports both identity snapshots.

Store the live parent, proof plan, unsigned operation, signed transaction,
finality receipt, and post-state as separate files. A path failure is a stop;
never retry a consumed id, evidence root, nullifier, allocation, or signed
transaction.

## Fail-closed preflight

Set these parameters outside the runbook; paths are references, not secrets.

```bash
export NODE_BIN=/absolute/path/to/postfiat-node
export FINALITY_RUNNER=/absolute/path/to/a666-ce22-remote-finality-op.py
export REMOTE_ROUND_RUNNER=/absolute/path/to/a666-remote-sync-round.py
export PROPOSER_HOSTS_FILE=/existing/secret-safe/proposer-hosts.json
export OUT=/absolute/path/to/lane-b-run
export DATA_DIR=/absolute/path/to/validator-data
export SOURCE_CHAIN_RPC_URL=https://approved-source-rpc.example
export PFUSDC_ASSET_ID=<asset-id>
export FROZEN_POLICY_HASH=<route-policy-hash>
export PROPOSER_ADDRESS=<pftl-address>
export FINALIZER_ADDRESS=<pftl-address>
export CLAIMER_ADDRESS=<pftl-address>
export PROPOSER_KEY_FILE=/existing/secret-safe/key-file
export FINALIZER_KEY_FILE=/existing/secret-safe/key-file
export CLAIMER_KEY_FILE=/existing/secret-safe/key-file
export SOURCE_RECEIPT_FILE=/absolute/path/to/frozen-source-receipt.json
export SOURCE_PROOF_FILE=/absolute/path/to/frozen-proof.bin
export SOURCE_PUBLIC_VALUES_FILE=/absolute/path/to/frozen-public-values.bin
export EXPIRES_AT_HEIGHT=<future-finalized-height>
```

Credential values remain only in their existing key or vault locations. Pass a
key-file path directly to the signer; never print, copy, or commit its content.
For vault-managed credentials, fetch only the approved provider key at command
execution time and do not write it to disk.

Collect six direct host statuses, normalize the parent, and stop unless all
three of `block_height`, `block_tip_hash`, and `state_root` agree. The direct
host method is authoritative; do not treat five validators or a stale local
tunnel as finality.

```bash
test -r "$PROPOSER_KEY_FILE"
test -r "$FINALIZER_KEY_FILE"
test -r "$CLAIMER_KEY_FILE"
test -r "$PROPOSER_HOSTS_FILE"
test -s "$SOURCE_RECEIPT_FILE"
test -s "$SOURCE_PROOF_FILE"
test -s "$SOURCE_PUBLIC_VALUES_FILE"
```

Verify the route's proof kind is `sp1-ethereum-finality-v1`, its profile is the
frozen Ethereum/Sepolia `sp1-groth16` profile, and the deposit recipient and
amount equal the intended claim. Stop unless the current global supply is at or
below the finalized cap, the deposit is new, and the proposed cap is exactly
`previous_cap + claim_atoms`.

## Packet construction and authorization

Use the node's planner to derive the canonical evidence root and all three
operations from the frozen source artifacts. The planner computes and verifies
the proof commitments; do not hand-compose proof byte arrays.

```bash
install -d -m 700 "$OUT" "$OUT/packet"
"$NODE_BIN" vault-bridge-deposit-plan \
  --receipt-file "$SOURCE_RECEIPT_FILE" \
  --asset-id "$PFUSDC_ASSET_ID" \
  --policy-hash "$FROZEN_POLICY_HASH" \
  --proposer "$PROPOSER_ADDRESS" \
  --finalizer "$FINALIZER_ADDRESS" \
  --claimer "$CLAIMER_ADDRESS" \
  --expires-at-height "$EXPIRES_AT_HEIGHT" \
  --source-proof-kind sp1-ethereum-finality-v1 \
  --source-proof-file "$SOURCE_PROOF_FILE" \
  --source-public-values-file "$SOURCE_PUBLIC_VALUES_FILE" \
  > "$OUT/packet/plan.json"

jq -e '
  .propose_operation.operation == "vault_bridge_deposit_propose" and
  .finalize_operation.operation == "vault_bridge_deposit_finalize" and
  .claim_operation.operation == "vault_bridge_deposit_claim" and
  .claim_operation.amount_atoms > 0 and
  .claim_operation.recipient == .evidence.pftl_recipient and
  .claim_operation.amount_atoms == .evidence.amount_atoms
' "$OUT/packet/plan.json" >/dev/null
```

The packet must retain: source chain, vault/token route, source transaction and
finality reference, asset and policy, deterministic evidence root, prior and
proposed cap, prior and proposed epoch, proof/public-values hashes, and exact
conservation arithmetic. `docs/evidence/pfusdc-eth-campaign-20260725/lane-b/packet-template.json`
is the parameter schema.

Generate a one-operation request only after re-reading the 6/6 parent. This
keeps unsigned, signed, and receipt records separate.

```bash
make_ops() {
  local stage=$1 source=$2 key_file=$3 operation_key=$4
  jq -n \
    --arg label "pfusdc-${stage}" \
    --arg source "$source" \
    --arg key_file "$key_file" \
    --argfile plan "$OUT/packet/plan.json" \
    --arg operation_key "$operation_key" \
    '{schema:"postfiat-certified-asset-ops-request-v1",operations:[{
      label:$label,source:$source,key_file:$key_file,
      operation:($plan[$operation_key])
    }]}' > "$OUT/${stage}.unsigned.json"
}

submit_stage() {
  local stage=$1
  "$FINALITY_RUNNER" \
    --ops-file "$OUT/${stage}.unsigned.json" \
    --artifact-dir "$OUT/${stage}" \
    --node-bin "$NODE_BIN" \
    --remote-runner "$REMOTE_ROUND_RUNNER" \
    --proposer-hosts-file "$PROPOSER_HOSTS_FILE"
}
```

## Normal self-checkpointing Ethereum sequence

Submit and finalize in this order. Each command must return an accepted,
confirmed, 6/6 receipt before the next command. No manual cap checkpoint is
authorized between these stages.

```bash
make_ops propose "$PROPOSER_ADDRESS" "$PROPOSER_KEY_FILE" propose_operation
submit_stage propose

# Wait until the record's governed challenge/finality requirements are met,
# then refresh the 6/6 parent and confirm the evidence root is pending/current.
make_ops finalize "$FINALIZER_ADDRESS" "$FINALIZER_KEY_FILE" finalize_operation
submit_stage finalize

# Re-read current cap, supply, epoch, route backing, recipient line, and
# pending redemptions. Stop unless the record is finalized and unconsumed.
make_ops claim "$CLAIMER_ADDRESS" "$CLAIMER_KEY_FILE" claim_operation
submit_stage claim
```

The normal issuance command is the final `vault_bridge_deposit_claim` above.
It must produce one recipient credit for `claim_operation.amount_atoms`, a
global supply increase by that amount, a cap increase by that amount, and one
epoch increment. Submit the identical claim only as a negative replay check;
it must be rejected and leave state unchanged.

## Manual checkpoint exception path

This path is unavailable for normal proof-backed Ethereum ingress. Use it only
when the governing asset specification explicitly requires a separately
finalized NAV reserve packet and the source event is independently proven. A
controlled inventory balance is not proof and may not be used as the packet's
backing input.

The exception packet is machine-readable and must contain these exact fields:

```json
{
  "asset_id": "<asset-id>",
  "backing_source": "<independently-proven-source-reference>",
  "asset_route": "<governed-route-id>",
  "previous_cap_atoms": "<atoms>",
  "proposed_cap_atoms": "<atoms>",
  "previous_epoch": "<epoch>",
  "proposed_epoch": "<previous_epoch_plus_one>",
  "proof_or_authorization": "<unique-current-proof-or-authorization-id>",
  "conservation": {"V": "<atoms>", "S": "<atoms>", "D": "<atoms>", "B": "<atoms>", "R": "<atoms>"}
}
```

The independently proven packet must be created by the asset's existing
governance packet builder, then submitted as the protocol-defined sequence
`nav_reserve_submit` → required `nav_reserve_attest` operations →
`nav_epoch_finalize`. Build every stage as a separate one-operation request
with `make_ops`, submit through `submit_stage`, and preserve its unsigned,
signed, and receipt artifacts. Stop unless `proposed_epoch == previous_epoch +
1`, `proposed_cap_atoms >= current_supply_atoms`, the backing authorization is
unique/current/unconsumed, and the finalizer's packet hash matches the submitted
reserve packet. Only after finality may the governing normal issuance path run.

This exception sequence must never be inserted before or between the three
normal proof-backed Ethereum stages above. If the applicable governing packet
builder or independent backing artifact is absent, it is a blocker—not authority
to construct a packet by hand.

## Identity, postflight, and rollback

### Six-host conservation checker

For E2E start and end, use the standalone checker. It fails closed unless six
hosts agree on one finalized parent with an empty mempool, imports and verifies
one **already signed finalized-checkpoint** snapshot locally, and runs the
audit with local Foundry `cast`. It never installs a tool or copies an
executable to a validator.

```bash
PYTHONPATH=python python3 scripts/pfusdc-conservation-identity.py \
  --inventory-file "$INVENTORY_FILE" \
  --asset-id "$PFUSDC_ASSET_ID" \
  --output "$OUT/conservation/identity.json" \
  --local-node-bin "$NODE_BIN" \
  --local-cast-bin "$CAST_BIN" \
  --scratch-dir "$OUT/conservation/scratch" \
  --signed-snapshot-dir "$SIGNED_FINALIZED_CHECKPOINT_DIR" \
  --snapshot-publisher-public-key-file "$SNAPSHOT_PUBLISHER_PUBLIC_KEY_FILE" \
  --vault-interface-lineage-manifest "$VAULT_INTERFACE_LINEAGE_MANIFEST" \
  --source-rpc-url "$SOURCE_CHAIN_RPC_URL"
```

The output schema `postfiat.pfusdc.conservation-identity.v2` has status
`verified`, `violated`, or `execution_blocked`; `components`, `lhs`, `rhs`,
and `residual_atoms` are populated only for a real audit. It preserves six raw
host responses, snapshot export/import/verification records, local audit JSON,
hashes, the common height/tip/state root, and exact `V`, `S`, `D`, `B`, and `R`
atoms. `verified` requires `V == S + D + B - R`.

For an E2E closing bracket, add `--opening-identity-file` pointing to the
accepted opening verdict. The machine output records
`residual_delta_from_h310 = residual_h312 - residual_h310`; the final
criterion is exactly `residual_final - residual_opening == 0`. A nonzero delta
of either sign blocks acceptance; it is never tolerance.

When an activity freeze prohibits another fleet read or snapshot export, reuse
only an already verified signed import. Supply the retained identity that
contains its hash-checked 6/6 parent responses; this mode performs no SSH,
export, import, transaction, service, or height-producing action:

```bash
PYTHONPATH=python python3 scripts/pfusdc-conservation-identity.py \
  --inventory-file "$INVENTORY_FILE" \
  --asset-id "$PFUSDC_ASSET_ID" \
  --output "$OUT/conservation/identity.json" \
  --local-node-bin "$NODE_BIN" \
  --local-cast-bin "$CAST_BIN" \
  --scratch-dir "$OUT/conservation/recheck" \
  --signed-snapshot-dir "$SIGNED_FINALIZED_CHECKPOINT_DIR" \
  --reuse-imported-data-dir "$VERIFIED_IMPORT_DATA_DIR" \
  --prior-identity-file "$PRIOR_IDENTITY_JSON" \
  --snapshot-publisher-public-key-file "$SNAPSHOT_PUBLISHER_PUBLIC_KEY_FILE" \
  --vault-interface-lineage-manifest "$VAULT_INTERFACE_LINEAGE_MANIFEST" \
  --source-rpc-url "$SOURCE_CHAIN_RPC_URL"
```

The audit selects each vault's fixed getter ABI only from the versioned,
hash-pinned interface-lineage manifest. The selected governed runtime hash must
have a matching source-manifest digest and `live_verified` status; unknown,
pending, digest-mismatched, duplicate, or getter-reverting entries fail closed.
There is no try-another-selector fallback.

The normal checker source is Ethereum-only. A historical
`deprecated-Arbitrum-legacy` audit is a one-time explicit invocation with a
route-owner supplied legacy RPC, `--legacy-source-label
deprecated-Arbitrum-legacy`, and `--legacy-finding-file
docs/evidence/pfusdc-eth-campaign-20260725/findings/legacy-arbitrum-one-active-route.md`.
Its structured finding records the PFTL finalized height, chain ID,
vault/token addresses, exact vault atoms, route activation/expiry, canonical
finding hash, and audit evidence hash. It is not permitted as an E2E fallback
for a new Ethereum claim.

Before propose and after claim, run the source-vault conservation audit against
the same asset and record exact integer atoms in separate files:

```bash
"$NODE_BIN" vault-bridge-conservation-audit \
  --data-dir "$DATA_DIR" \
  --asset-id "$PFUSDC_ASSET_ID" \
  --source-rpc-url "$SOURCE_CHAIN_RPC_URL" \
  > "$OUT/identity-audit.json"
```

Write `identity-before.json` and `identity-after.json` with this shape:

```json
{
  "components": {"V": "<atoms>", "S": "<atoms>", "D": "<atoms>", "B": "<atoms>", "R": "<atoms>"},
  "lhs": "<V>",
  "rhs": "<S+D+B-R>",
  "verified": true
}
```

Require `lhs == rhs`, exact integer arithmetic, the previous cap/supply/epoch
transition, unchanged pending redemptions, and a 6/6 common post-state. A
failed proposed/finalized/claim transaction does not authorize reuse: preserve
its receipt, diagnose, and start from a newly proven source event only if the
governing route permits it. There is no rollback by raw ledger mutation or
cap reduction; use only the protocol's validated redemption/reversal lifecycle.

## Evidence layout

```text
$OUT/
  live-state/                 # six direct-host reads and common parent
  packet/plan.json            # proof/evidence-derived packet
  propose.unsigned.json       # unsigned request
  propose/{signed.json,summary.json,consensus/}
  finalize.unsigned.json
  finalize/{signed.json,summary.json,consensus/}
  claim.unsigned.json
  claim/{signed.json,summary.json,consensus/}
  pre-state.json
  post-state.json
  identity-before.json
  identity-after.json
  replay-rejection.json
```

Run the lane verifier only after all required records exist. It fails closed on
missing proof/finality/replay/identity evidence, non-unique authorization, or
any mismatch in cap, supply, recipient credit, epoch, or conservation.
