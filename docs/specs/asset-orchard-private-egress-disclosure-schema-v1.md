# Asset-Orchard private egress disclosure schema, version 1

**Schema id:** `postfiat.asset_orchard.private_egress.disclosure.v1`
**Schema hash:** `7a585bbd6b5682e0d4fe72bfafa7d7b2c0a19de73924429ef7b9e22c5e1dfa52be7c2de0804e7889900a8a32f9448831`
**Date:** 2026-10-29

This document is what a governed egress policy record
(`asset_orchard_egress_policy_v1:<asset_id>:<disclosure_schema_hash>:<policy_id>`)
commits to. Once a record for an asset is in effect, consensus refuses an
`asset_orchard_private_egress_v1` action of that asset unless:

- its `policy_id` equals the record's policy id; and
- its `disclosure_hash` equals the record's disclosure schema hash, which is the hash above.

## What an egress discloses

The action's payload is public. It is the
`AssetOrchardPrivateEgressActionPayload` in
`crates/types/src/shielded_bridge_governance.rs`. Its fields fall into four
groups:

- **Destination and value.** `to`, `asset_id` with its `asset_tag_lo` and
  `asset_tag_hi`, `amount` and `fee` (version 1 requires `fee` to be 0). These are
  what a public exit discloses.
- **Spend binding.** `anchor` is a retained tree root. `nullifier` marks the
  spent note. `pool_domain` and `randomized_verification_key` bind the spend
  to this chain and pool.
- **Governance binding.** `policy_id` and `disclosure_hash`.
- **Evidence.** `exit_binding_hash`, `proof` and
  `spend_authorization_signature`. The payload also carries `version`,
  `schema`, `pool_id`, `proof_system_id` and `circuit_id`.

The proof binds most of these fields:

- **The public instance.** The Halo2 proof's public instance carries
  `pool_domain`, `anchor`, `nullifier`, `randomized_verification_key`, the
  asset tag, `amount`, `fee` and `exit_binding_hash`.
- **The exit binding.** `exit_binding_hash` is a Poseidon hash over:
  - the proof system, circuit, action schema, pool and note version;
  - the chain id, genesis hash, protocol version and pool domain;
  - `to`, `asset_id`, `amount`, `fee`, `policy_id` and `disclosure_hash`.

  The circuit binding is `asset_orchard_private_egress_exit_binding_hash` in
  `crates/privacy_orchard/src/asset_orchard.rs`.
- **Consequence.** A proof made for one destination, asset, amount, policy or
  disclosure hash does not verify for another.
- **The evidence fields.** `proof` and `spend_authorization_signature` are the
  evidence itself; the signature is made with the randomized verification key.

Never disclosed: the note opening (diversifier, `g_d`, `pk_d`, `rho`, `psi`,
`rcm`), the note's commitment and tree position, the nullifier key, the
spend-authorizing key and its randomizer, the incoming and outgoing viewing
keys, the note seed, and any other note of the wallet. The node never
receives them, and the payload has no field for them.

## Canonical document and hash

The schema hash is computed over the bytes of
`crates/node/tests/fixtures/private-egress-disclosure-schema-v1.json`. That
file is this canonical document exactly:

- UTF-8;
- object keys sorted at every level;
- no whitespace between tokens;
- no trailing newline.

```json
{"action_kind":"asset_orchard_private_egress_v1","action_schema":"postfiat-asset-orchard-private-egress-action-v1","never_disclosed":["note_opening","note_commitment","note_position","nullifier_key","spend_authorizing_key","spend_auth_randomizer","incoming_viewing_key","outgoing_viewing_key","note_seed","other_notes"],"policy":{"disclosure_hash":"this document's schema hash","enforced_from":"the record's effective height","fee":0,"policy_id":"the governed record's policy_id for the asset","record_kind":"asset_orchard_egress_policy_v1"},"pool_id":"asset-orchard-v1","public_fields":[{"name":"to","proof_bound":true,"type":"pftl_address"},{"name":"asset_id","proof_bound":true,"type":"hex48"},{"name":"asset_tag","proof_bound":true,"type":"u128_pair"},{"name":"amount","proof_bound":true,"type":"u64_atoms"},{"name":"fee","proof_bound":true,"type":"u64_atoms_zero"},{"name":"anchor","proof_bound":true,"type":"pallas_base_hex"},{"name":"nullifier","proof_bound":true,"type":"pallas_base_hex"},{"name":"pool_domain","proof_bound":true,"type":"pallas_base_hex"},{"name":"randomized_verification_key","proof_bound":true,"type":"pallas_point_hex"},{"name":"policy_id","proof_bound":true,"type":"governed_text"},{"name":"disclosure_hash","proof_bound":true,"type":"hex48_equals_schema_hash"},{"name":"exit_binding_hash","proof_bound":true,"type":"hex64"},{"name":"proof","proof_bound":false,"type":"halo2_proof_hex"},{"name":"spend_authorization_signature","proof_bound":false,"type":"redpallas_signature_hex"}],"schema_id":"postfiat.asset_orchard.private_egress.disclosure.v1","version":1}
```

The registry checks only the hash's form: 48-byte lowercase hex, not all
zero (`validate_asset_orchard_egress_policy_kind_parts` in
`crates/types/src/shielded_bridge_governance.rs`). The hash is the
repository's domain-separated SHA3-384, `postfiat_crypto_provider::hash_hex`:

```text
SHA3-384( "postfiat.asset_orchard.private_egress.disclosure.v1" || 0x00 || canonical bytes )
= 7a585bbd6b5682e0d4fe72bfafa7d7b2c0a19de73924429ef7b9e22c5e1dfa52be7c2de0804e7889900a8a32f9448831
```

The domain is the schema id. The test
`asset_orchard_egress_disclosure_schema_hash_is_pinned` in
`crates/node/src/tests/asset_orchard_egress_policy_tests.rs` checks four things:
- the fixture is canonical;
- its hash equals the hash pinned here;
- this document carries both;
- the records proposed below validate.

To print the hash from the fixture:

```bash
python3 -c 'import hashlib; d=b"postfiat.asset_orchard.private_egress.disclosure.v1"; print(hashlib.sha3_384(d + b"\0" + open("crates/node/tests/fixtures/private-egress-disclosure-schema-v1.json","rb").read()).hexdigest())'
```

## Proposed devnet records

Both policy ids satisfy the registry's rule: 1 to 31 bytes of `[a-z0-9._-]`.
The asset ids are the `postfiat-wan-devnet-2` ids in
`crates/node/tests/fixtures/private-actions-v1/assets.json`.

| Asset | Asset id | Policy id |
| --- | --- | --- |
| A666 | `521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c` | `a666-egress-v1` |
| pfUSDC | `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b` | `pfusdc-egress-v1` |

Not run: the ratify commands for the two records follow.
`<VALIDATORS_CSV>` is the governing validator list. `<H>` is an effective
height strictly after the block that applies the amendment.

```bash
postfiat-node ratify-asset-orchard-egress-policy --data-dir <DATA_DIR> \
  --validators <VALIDATORS_CSV> \
  --asset-id 521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c \
  --policy-id a666-egress-v1 \
  --disclosure-schema-hash 7a585bbd6b5682e0d4fe72bfafa7d7b2c0a19de73924429ef7b9e22c5e1dfa52be7c2de0804e7889900a8a32f9448831 \
  --effective-height <H> --amendment-file a666-egress-policy.amendment.json

postfiat-node ratify-asset-orchard-egress-policy --data-dir <DATA_DIR> \
  --validators <VALIDATORS_CSV> \
  --asset-id 02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b \
  --policy-id pfusdc-egress-v1 \
  --disclosure-schema-hash 7a585bbd6b5682e0d4fe72bfafa7d7b2c0a19de73924429ef7b9e22c5e1dfa52be7c2de0804e7889900a8a32f9448831 \
  --effective-height <H> --amendment-file pfusdc-egress-policy.amendment.json
```

After both records are in effect, the global activation follows; it is also
not run. From that height, an asset with no record is refused with
`asset_orchard_egress_policy_not_governed`:

```bash
postfiat-node ratify-asset-orchard-egress-policy-activation-height --data-dir <DATA_DIR> \
  --validators <VALIDATORS_CSV> --height <H_ACTIVATION> \
  --amendment-file egress-policy-activation.amendment.json
```

Each amendment then goes through the ordinary signed governance path:
`governance-authorization-sign` by each validator,
`governance-amendment-assemble`, `governance-batch`, then a certified round.

## Not before

Governance writes these records on the devnet only after the wallet team
confirms one thing: the app takes the policy id and disclosure hash from
`asset_orchard_private_egress_binding` (or `private_pool_snapshot`), not from
its own configuration. A wallet that still sends another value would have its
egress refused with `asset_orchard_egress_policy_mismatch` from the record's
effective height on.

A change to this document is a new schema version with a new id and hash. It
never edits version 1.
