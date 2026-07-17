---
title: AssetOrchardSwapCircuit Design
date: 2026-06-19
status: implemented v1 production-candidate; public-network/privacy claims still gated on external cryptographic review and broader wallet/service hardening
related:
  - docs/specs/private-otc-shielded-scope.md
  - docs/status/shielded-layer-map.md
---

# AssetOrchardSwapCircuit Design

## 1. Purpose

`AssetOrchardSwapCircuit` is the implemented v1 circuit for private NAV OTC swaps inside one asset-typed shielded pool. The consensus-facing `ShieldedSwapV1` action can carry an `AssetOrchardSwapAction`; the old transcript-scaffold `ShieldedSwapAction` remains legacy compatibility/test scaffolding and is not the NAV OTC production-candidate path.

The v1 circuit is an Orchard/Halo2-native, fixed-shape, two-input/two-output shielded swap. Its consensus soundness boundary for hidden asset/value conservation is the consensus-verified Halo2 proof over the hidden witness. Spend authorization additionally requires consensus-verified RedPallas spend authorization signatures.

The proof statement implies:

1. Both input notes exist under the retained anchor.
2. Both input notes are asset-typed notes whose commitments bind:
   - pool domain,
   - recipient address material,
   - private `AssetTag`,
   - private nonzero value,
   - note rho,
   - note randomness.
3. Both input nullifiers are correctly derived from those exact asset-typed note commitments and the corresponding nullifier deriving keys.
4. The prover is authorized to spend both input notes through the Orchard full-viewing-key relation and randomized spend authorization keys.
5. Both output note commitments bind private asset identities and private nonzero values.
6. The two output `(asset_tag, value)` pairs are exactly a permutation of the two input `(asset_tag, value)` pairs.
7. The proof is bound to the chain, genesis, protocol version, single shielded pool, circuit id, public action fields, encrypted output bytes through collision-resistant hashes, and spend authorization transcript.

For internal swaps, raw asset ids, asset tags, values, owners, recipients, and price are never consensus-visible.

## 2. Status, Normative Language, and Non-Goals

This document describes the implemented v1 production-candidate circuit. Public-network activation or production privacy claims MUST NOT occur until:

- the implementation continues to match this spec or the spec is updated,
- independent cryptographic review accepts the construction,
- the breaking-test suite in Section 18 passes,
- the verifying key and all parameter/layout hashes are pinned as in Sections 11 and 12.

The terms MUST, MUST NOT, SHOULD, and MAY are used in their RFC 2119 sense.

### 2.1 Fixed v1 decisions

The following are fixed for `AssetOrchardSwapV1` and are not open design placeholders:

- `swap_binding_hash` is a field-native Poseidon action binding recomputed inside the circuit from the circuit public inputs.
- Variable-length encrypted output bytes are bound through consensus-computed SHA3-384 encrypted-output hashes; those hash limbs are circuit public inputs and are included in the in-circuit Poseidon action binding.
- `AssetTag` is two 128-bit limbs derived from the first 256 bits of a domain-separated SHA3-384 digest of the canonical transparent asset id.
- The all-zero asset tag is reserved and invalid.
- There is no `asset_tag_0 != asset_tag_1` constraint. Same-asset swaps/settlements are consensus-valid if the two output `(tag, value)` pairs are exactly a permutation of the two input pairs.
- Input and output values MUST be nonzero `u64` values.
- The nullifier directly includes `pool_domain` and the asset-typed note commitment leaf `cmx`.
- Output `rho` is deterministic and circuit-constrained from the public spend context; there is no private output-rho nonce in v1.
- Fee support inside the shielded swap circuit is disabled: `fee == 0`.

### 2.2 Current state

The existing Orchard path in this repository uses real Halo2/Orchard components for value-only notes: note commitments, nullifiers, retained anchors, spend authorization signatures, encrypted outputs, and disclosure plumbing.

The Asset-Orchard path adds asset-typed note commitments and the fixed-shape two-input/two-output swap circuit described here. Consensus verifies the serialized `AssetOrchardSwapAction` proof and RedPallas spend authorization signatures before updating public pool state.

The older `ShieldedSwapAction` transcript scaffold is still not the production-candidate proof path. It is useful for serialization and legacy adversarial tests only; NAV OTC should use the `AssetOrchardSwapAction` carried by `ShieldedSwapV1`.

### 2.3 Non-goals

This document does not implement:

- a public AMM,
- separate shielded pools per asset,
- RingCT-style algebraic-only conservation,
- hidden backing or shielded reserves,
- NAV-band policy enforcement inside the swap circuit,
- shielded fees, change, splits, or merge/split accounting,
- encryption correctness proofs for note ciphertexts.

Those features require later circuit ids or separate reviewed actions.

## 3. Curves, Fields, Encodings, and Parameter Set

### 3.1 Curves and proof backend

Use the same curve cycle and Halo2 backend as the Orchard 0.14 path in this repository:

- circuit field: Pallas base field `Fp`,
- note commitment group: Pallas,
- proof curve: Vesta affine,
- polynomial commitment backend: Halo/IPA as exposed by `halo2_proofs::poly::commitment::Params::new(K)`.

Switching to another proof backend, including any KZG backend, is a consensus cryptography change and requires a new review and new circuit id.

### 3.2 Canonical byte encodings

Consensus MUST use the following canonical encodings.

- `u8`, `u16`, `u32`, `u64`, `u128`: unsigned little-endian fixed-width encodings.
- `len_bytes(x)`: `u32le(len(x)) || x`.
- `FieldEnc(x)`: 32-byte little-endian canonical encoding of a Pallas base field element, interpreted as an integer `< p`. Non-canonical encodings MUST be rejected before proof verification.
- `ScalarEnc(s)`: 32-byte little-endian canonical encoding of a Pallas scalar, interpreted as an integer `< q`. Non-canonical encodings MUST be rejected where scalar bytes are parsed.
- `PointEnc(P)`: canonical compressed Pallas point encoding used by Orchard/RedPallas: canonical x-coordinate plus sign bit. Identity, non-canonical, and non-subgroup encodings MUST be rejected.
- `PointFields(P)`: affine `(x, y)` Pallas base field elements after canonical decompression and subgroup validation.

The Halo2 public instance uses field elements, not raw bytes. Nodes MUST construct the public instance from parsed canonical action fields. A serialized action MUST NOT supply its own arbitrary public-instance vector.

### 3.3 Poseidon parameterization

All in-circuit field hashes in this design use the Pallas-base Poseidon parameter set:

```text
PoseidonPallasV1 = halo2_gadgets P128Pow5T3 over Pallas base field
width t = 3
rate = 2
S-box = x^5
full/partial rounds and constants = the exact P128Pow5T3 constants pinned for the release
```

The release MUST pin:

```text
poseidon_parameter_set_id
poseidon_constants_hash
poseidon_mds_hash
poseidon_round_schedule_hash
```

No implementation may substitute a different Poseidon parameterization under the same circuit id.

Define:

```text
ConstField(name) =
    HashToPallasBase(
        "postfiat.asset_orchard.const.v1",
        ascii(name)
    )
```

Define `PoseidonHash2(name, fields)` as:

```text
input_fields = [
    ConstField(name),
    field(len(fields))
] || fields

(ctx0, ctx1) = PoseidonPallasV1_sponge_squeeze_2(input_fields)
```

The sponge absorption, padding, and squeezing MUST be identical in the host implementation and the circuit gadget. The activation release MUST include test vectors for every named `PoseidonHash2` invocation in this document.

When a single field output is required, use the first output:

```text
PoseidonHash1(name, fields) = PoseidonHash2(name, fields)[0]
```

## 4. Domain-Separated Primitives

### 4.1 `HashToPallasBase`

`HashToPallasBase(dst, msg)` maps bytes to a Pallas base field element by rejection sampling:

```text
for counter = 0, 1, 2, ...:
    digest = SHA3-512(
        "postfiat.hash_to_pallas_base.v1" ||
        len_bytes(dst) ||
        len_bytes(msg) ||
        u32le(counter)
    )

    x = little_endian_uint256(digest[0..32])

    if x < p:
        return Fp(x)
```

This function is used only by host/consensus code to derive public constants or public domain fields. It is not simulated with unconstrained SHA3 inside the circuit.

### 4.2 `HashToPallasScalarNonzero`

`HashToPallasScalarNonzero(dst, msg)` maps bytes to a nonzero Pallas scalar:

```text
for counter = 0, 1, 2, ...:
    digest = SHA3-512(
        "postfiat.hash_to_pallas_scalar.v1" ||
        len_bytes(dst) ||
        len_bytes(msg) ||
        u32le(counter)
    )

    x = little_endian_uint256(digest[0..32])

    if 0 < x < q:
        return Scalar(x)
```

This function is wallet/host-side. It is not used to prove a hidden SHA3 relation in the swap circuit.

### 4.3 `AssetTag`

`asset_id` is the canonical transparent ledger asset identifier. Implementations MUST reject non-canonical asset ids before deriving tags.

For a canonical asset id:

```text
digest = SHA3-384(
    "postfiat.asset_orchard.asset_tag.v1" ||
    len_bytes(canonical_bytes(asset_id))
)

asset_tag_lo = little_endian_u128(digest[0..16])
asset_tag_hi = little_endian_u128(digest[16..32])
asset_tag    = (asset_tag_lo, asset_tag_hi)
```

The remaining 16 digest bytes are unused in v1.

Both limbs are embedded as field elements and are range-constrained to 128 bits in every circuit that handles private asset tags.

The all-zero tag is reserved:

```text
(asset_tag_lo, asset_tag_hi) != (0, 0)
```

Public edge actions that introduce an asset into the shielded pool MUST reject an asset id whose derived tag is all-zero.

The transparent asset registry SHOULD store `AssetTag -> canonical_asset_id` for all registered public assets and MUST reject registration or public ingress of a different canonical asset id with an already-registered tag.

#### Collision and second-preimage target

The split into two 128-bit limbs is injective because each limb is an integer `< 2^128` and Pallas base field elements can represent all 128-bit integers exactly. Therefore two asset ids have the same `AssetTag` if and only if the first 32 bytes of their domain-separated SHA3-384 digests are equal.

Assuming SHA3-384 truncated to 256 bits behaves as a 256-bit collision-resistant digest for this domain:

- targeted confusion of an existing asset tag requires a second preimage for a 256-bit truncated digest, with generic cost about `2^256`;
- finding any pair of colliding asset ids has birthday cost about `2^128`;
- for an asset universe of size `N`, accidental collision probability is approximately `N(N-1)/2^257`.

The circuit cannot distinguish two asset ids with the same tag. The asset-tag collision assumption is therefore part of the v1 soundness assumptions.

### 4.4 `PoolDomain`

`pool_domain` is a public Pallas base field element binding notes and proofs to one chain, genesis, protocol version, pool, and note version.

```text
PoolDomainPreimage =
    len_bytes(chain_id) ||
    genesis_hash[32] ||
    u32le(protocol_version) ||
    len_bytes(pool_id) ||
    u16le(note_version)

pool_domain =
    HashToPallasBase(
        "postfiat.asset_orchard.pool_domain.v1",
        PoolDomainPreimage
    )
```

For v1:

```text
pool_id      = "asset-orchard-v1"
note_version = 1
```

Consensus MUST recompute `pool_domain` from local chain parameters and the parsed `pool_id`, and MUST reject any action whose serialized `pool_domain` does not match.

### 4.5 `OrchardPsi` and `OrchardRcm`

For AssetOrchard v1, note plaintexts carry a 32-byte `rseed`. Conforming wallets derive note randomness as:

```text
OrchardPsi(rseed, rho) =
    HashToPallasBase(
        "postfiat.asset_orchard.rseed.psi.v1",
        rseed || FieldEnc(rho)
    )

OrchardRcm(rseed, rho) =
    HashToPallasScalarNonzero(
        "postfiat.asset_orchard.rseed.rcm.v1",
        rseed || FieldEnc(rho)
    )
```

Consensus circuits use `psi` and `rcm` as private witnesses when recomputing note commitments. The swap circuit does not constrain SHA3-based derivation from `rseed`; `rseed` is encrypted plaintext/wallet material, not a consensus public input.

This is not a soundness gap for conservation: consensus soundness depends on the committed `(psi, rcm)`, not on ciphertext correctness. A malformed ciphertext can make an output unspendable by its intended recipient, but it cannot create value or relabel an asset. Spend authorizations sign the exact encrypted output bytes as specified in Section 10.

Wallets and disclosure verifiers MUST check that decrypted or disclosed `rseed` derives the `psi` and `rcm` used to recompute the public output commitment.

### 4.6 `AssetDeriveNullifier`

`AssetDeriveNullifier` is a new AssetOrchard v1 nullifier. It is not the legacy value-only Orchard nullifier.

For an asset-typed note with nullifier deriving key `nk`, rho `rho`, note randomness field `psi`, and note commitment leaf `cmx`:

```text
nf =
    PoseidonHash1(
        "postfiat.asset_orchard.nullifier.v1",
        [
            ConstField("asset_orchard_note_version_1"),
            pool_domain,
            nk,
            rho,
            psi,
            cmx
        ]
    )
```

The circuit MUST use the same private `nk` in:

1. the Orchard full-viewing-key/address authority relation, and
2. `AssetDeriveNullifier`.

Including both `pool_domain` and `cmx` is mandatory. `pool_domain` gives direct cross-domain separation; `cmx` binds the nullifier to the asset-typed commitment that contains the private asset tag and value.

### 4.7 `AssetOutputRho`

Output rho is deterministic and circuit-constrained. It is derived from public spend context fields and the output index, but not from output commitments, avoiding circularity.

For `j in {0,1}`:

```text
rho_new[j] =
    PoseidonHash1(
        "postfiat.asset_orchard.output_rho.v1",
        [
            pool_domain,
            anchor,
            nf_old[0],
            nf_old[1],
            rk[0].x,
            rk[0].y,
            rk[1].x,
            rk[1].y,
            field(j)
        ]
    )
```

Because nullifiers are unique and consensus rejects duplicate nullifiers, this gives deterministic per-action output rho. `rho` is not public in the note plaintext sense, but it is derivable from public action fields; this does not reveal asset or value because the note commitment remains hiding under `rcm`.

### 4.8 Encrypted output hash

The circuit does not hash encrypted output bytes directly. Instead, consensus computes fixed public hash limbs for each encrypted output.

For output index `j`:

```text
eo_digest[j] = SHA3-384(
    "postfiat.asset_orchard.encrypted_output_hash.v1" ||
    u8(j) ||
    len_bytes(encrypted_output[j])
)

eo_hash[j][0] = little_endian_u128(eo_digest[j][0..16])
eo_hash[j][1] = little_endian_u128(eo_digest[j][16..32])
eo_hash[j][2] = little_endian_u128(eo_digest[j][32..48])
```

Each limb is embedded as a field element and range-constrained to 128 bits in the circuit. Consensus MUST recompute these limbs from the parsed encrypted output bytes and MUST NOT accept user-supplied hash limbs.

### 4.9 `H_action` and `swap_binding_hash`

`H_action` is the in-circuit action binding hash. It binds the proof to the parsed public action fields.

Let:

```text
ActionBindingFieldsV1 = [
    ConstField("proof_system:postfiat.privacy.asset-orchard-halo2.v1"),
    ConstField("circuit:asset_orchard.swap.v1"),
    ConstField("schema:postfiat-asset-orchard-swap-action-v1"),
    ConstField("pool:asset-orchard-v1"),
    ConstField("note_version:1"),
    pool_domain,
    anchor,
    nf_old[0],
    nf_old[1],
    rk[0].x,
    rk[0].y,
    rk[1].x,
    rk[1].y,
    cmx_new[0],
    cmx_new[1],
    eo_hash[0][0],
    eo_hash[0][1],
    eo_hash[0][2],
    eo_hash[1][0],
    eo_hash[1][1],
    eo_hash[1][2],
    fee_field
]
```

Then:

```text
(action_ctx_0, action_ctx_1) =
    PoseidonHash2(
        "postfiat.asset_orchard.h_action.v1",
        ActionBindingFieldsV1
    )

swap_binding_hash = FieldEnc(action_ctx_0) || FieldEnc(action_ctx_1)
```

The circuit MUST recompute `(action_ctx_0, action_ctx_1)` from the public inputs and constrain them equal to the public action context fields. Consensus MUST recompute the same values before proof verification and MUST reject any action whose serialized `swap_binding_hash` differs.

`H_action` covers the consensus-semantic action transcript: all public state-changing fields and encrypted output bytes through `eo_hash`. It intentionally excludes proof bytes and spend authorization signatures to avoid circularity. Replacing one valid proof with another valid proof for the same public instance does not change the authorized state transition.

### 4.10 `H_sig`

Spend authorization signatures are over a conventional byte hash of the parsed action. Define:

```text
SigPreimageV1 =
    "postfiat.asset_orchard.swap.sighash.v1" ||
    u16le(1) ||                                  // action version
    len_bytes(chain_id) ||
    genesis_hash[32] ||
    u32le(protocol_version) ||
    len_bytes(pool_id) ||
    len_bytes("postfiat.privacy.asset-orchard-halo2.v1") ||
    len_bytes("asset_orchard.swap.v1") ||
    len_bytes("postfiat-asset-orchard-swap-action-v1") ||
    FieldEnc(pool_domain) ||
    FieldEnc(anchor) ||
    FieldEnc(nf_old[0]) ||
    FieldEnc(nf_old[1]) ||
    PointEnc(rk[0]) ||
    PointEnc(rk[1]) ||
    FieldEnc(cmx_new[0]) ||
    FieldEnc(cmx_new[1]) ||
    len_bytes(encrypted_output[0]) ||
    len_bytes(encrypted_output[1]) ||
    swap_binding_hash ||
    u64le(fee)

asset_orchard_swap_sighash = SHA3-256(SigPreimageV1)
```

Consensus verifies:

```text
VerifyRedPallas(rk[i], asset_orchard_swap_sighash, spend_auth_sig[i]) == true
```

for `i = 0, 1`.

`H_sig` signs the raw encrypted output bytes, while `H_action` binds their SHA3-384 hash limbs inside the proof. Changing ciphertext bytes after authorization requires either a RedPallas forgery, a SHA3-256 sighash collision, or new spend signatures.

## 5. Asset-Typed Note Format

### 5.1 Consensus note opening

The consensus note commitment opening is:

```text
AssetNoteOpening {
    d: orchard diversifier,
    g_d: Pallas point,        // Orchard diversified base for d
    pk_d: Pallas point,       // Orchard transmission key
    asset_tag_lo: u128,
    asset_tag_hi: u128,
    value: u64_nonzero,
    rho: Fp,
    psi: Fp,
    rcm: Pallas scalar nonzero
}
```

`asset_id` is not part of the consensus note commitment. Wallets store `asset_id` in encrypted note plaintext and disclosure packets.

### 5.2 Address validity and spend authority

For every input note, the circuit MUST prove the Orchard address/spend-authority relation using the same Orchard key derivation domains and generators pinned for this release:

```text
g_d = OrchardDiversifyHash(d)
ivk = OrchardDeriveIvk(ak, nk, rivk)
pk_d = [ivk] g_d
rk = ak + [alpha] SpendAuthG
```

The private `nk` in this relation MUST be the same `nk` used by `AssetDeriveNullifier`.

The public `rk` is represented in the Halo2 public instance by affine coordinates `(rk.x, rk.y)` derived from canonical `PointEnc(rk)` in the action.

This relation is mandatory. Without it, a prover could open someone else’s note commitment and derive a nullifier under an unrelated key.

For output notes, the circuit MUST check that `g_d = OrchardDiversifyHash(d)` and that `pk_d` is a valid Pallas subgroup point. The circuit cannot prove the recipient knows a viewing or spending key for `pk_d`; malformed recipient data is an authorized burn risk, not a conservation failure.

### 5.3 Asset note commitment

Define:

```text
AssetNoteCommitDomain = "postfiat.asset_orchard.note_commit.v1"
```

Define `PointBits(P)` as the bit representation of canonical `PointEnc(P)`. Define `I2LEBSP_n(x)` as the `n`-bit little-endian bit string of integer `x`.

The commitment message is:

```text
asset_note_message =
    I2LEBSP_255(pool_domain) ||
    I2LEBSP_128(asset_tag_lo) ||
    I2LEBSP_128(asset_tag_hi) ||
    PointBits(g_d) ||
    PointBits(pk_d) ||
    I2LEBSP_64(value) ||
    I2LEBSP_255(rho) ||
    I2LEBSP_255(psi)
```

The asset note commitment is:

```text
cm  = SinsemillaCommit[AssetNoteCommitDomain](asset_note_message, rcm)
cmx = ExtractP(cm)
```

where `ExtractP` returns the Pallas x-coordinate field element of the commitment point.

This is a new commitment domain and message. Asset-typed notes are not spend-compatible with legacy value-only Orchard notes.

### 5.4 Note plaintext and wallet scan

`AssetNotePlaintextV1` extends Orchard note plaintext with asset metadata:

```text
AssetNotePlaintextV1 {
    plaintext_version,
    recipient data needed by Orchard note decryption,
    canonical asset_id,
    asset_tag_lo,
    asset_tag_hi,
    value,
    rho,
    rseed,
    memo
}
```

The plaintext encoding MUST be fixed-length or length-bounded and canonical. For v1:

```text
MAX_ASSET_ID_BYTES = 128
MEMO_BYTES         = 512
```

Asset id bytes inside plaintext are length-prefixed and zero-padded to `MAX_ASSET_ID_BYTES` before encryption.

Wallet scan MUST verify:

```text
AssetTag(asset_id) == (asset_tag_lo, asset_tag_hi)
(asset_tag_lo, asset_tag_hi) != (0, 0)
psi = OrchardPsi(rseed, rho)
rcm = OrchardRcm(rseed, rho)
recomputed_cmx(pool_domain, recipient, asset_tag, value, rho, psi, rcm) == public cmx
```

Consensus does not decrypt note ciphertext. Incorrect ciphertext can make a recipient unable to recover or spend an output, but it cannot create value or fake an asset because the SNARK constrains the public `cmx`.

## 6. Swap Action Format

The v1 action identifiers are:

```text
proof_system_id = "postfiat.privacy.asset-orchard-halo2.v1"
circuit_id      = "asset_orchard.swap.v1"
schema          = "postfiat-asset-orchard-swap-action-v1"
pool_id         = "asset-orchard-v1"
```

Serialized public action fields:

```text
version
pool_id
pool_domain
anchor
nullifiers[2]                    // nf_old[0..2]
randomized_verification_keys[2]   // canonical PointEnc(rk[i])
output_commitments[2]             // cmx_new[0..2]
encrypted_outputs[2]              // opaque bounded canonical bytes
swap_binding_hash                 // FieldEnc(action_ctx_0) || FieldEnc(action_ctx_1)
fee                               // v1 requires fee == 0
proof
spend_authorization_signatures[2]
```

The action MUST NOT expose:

```text
asset_id
asset_tag
input values
output values
sender identity
recipient identity
price
which output belongs to which party
```

The existing public `input_asset_commitments`, `input_value_commitments`, `output_asset_commitments`, and `output_value_commitments` from the scaffold are not consensus fields for this design and MUST NOT be used as a soundness boundary.

## 7. Exact Circuit Statement

### 7.1 Public instance layout

The Halo2 public instance for `asset_orchard.swap.v1` is exactly:

```text
0:  pool_domain
1:  anchor
2:  nf_old[0]
3:  nf_old[1]
4:  rk[0].x
5:  rk[0].y
6:  rk[1].x
7:  rk[1].y
8:  cmx_new[0]
9:  cmx_new[1]
10: eo_hash[0][0]
11: eo_hash[0][1]
12: eo_hash[0][2]
13: eo_hash[1][0]
14: eo_hash[1][1]
15: eo_hash[1][2]
16: fee_field
17: action_ctx_0
18: action_ctx_1
```

`fee_field` MUST be zero.

Consensus MUST pin a hash of this public-instance layout. Any length mismatch, ordering mismatch, or alternative layout is consensus-invalid.

### 7.2 Private witness

For each input note `i in {0,1}`:

```text
input_note_i = {
    d_i,
    g_d_i,
    pk_d_i,
    asset_tag_i.lo,
    asset_tag_i.hi,
    value_i,
    rho_i,
    psi_i,
    rcm_i,
    cm_i
}

spend_authority_i = {
    ak_i,
    nk_i,
    rivk_i,
    ivk_i,
    alpha_i
}

merkle_path_i
merkle_position_i
```

For each output note `j in {0,1}`:

```text
output_note_j = {
    d'_j,
    g_d'_j,
    pk_d'_j,
    asset_tag'_j.lo,
    asset_tag'_j.hi,
    value'_j,
    rho'_j,       // constrained to AssetOutputRho(...)
    psi'_j,
    rcm'_j,
    cm'_j
}
```

Additional private witness:

```text
s in {0,1}       // private permutation bit
```

### 7.3 Input constraints

For each input `i`:

1. Range-constrain `asset_tag_i.lo < 2^128` and `asset_tag_i.hi < 2^128`.
2. Enforce `(asset_tag_i.lo, asset_tag_i.hi) != (0, 0)`.
3. Range-constrain `1 <= value_i < 2^64`.
4. Validate the Orchard address relation:
   ```text
   g_d_i = OrchardDiversifyHash(d_i)
   ivk_i = OrchardDeriveIvk(ak_i, nk_i, rivk_i)
   pk_d_i = [ivk_i] g_d_i
   ```
5. Recompute:
   ```text
   cm_i  = AssetNoteCommit(pool_domain, g_d_i, pk_d_i, asset_tag_i, value_i, rho_i, psi_i, rcm_i)
   cmx_i = ExtractP(cm_i)
   ```
6. Verify the Merkle path for `cmx_i` opens to public `anchor`.
7. Recompute:
   ```text
   nf_i = AssetDeriveNullifier(pool_domain, nk_i, rho_i, psi_i, cmx_i)
   ```
8. Constrain:
   ```text
   nf_i == public nf_old[i]
   ```
9. Derive the randomized verification key:
   ```text
   rk_i = ak_i + [alpha_i] SpendAuthG
   ```
   and constrain its affine coordinates equal to public `(rk[i].x, rk[i].y)`.

The circuit MUST enforce:

```text
nf_old[0] != nf_old[1]
```

Consensus also rejects duplicate nullifiers.

### 7.4 Output constraints

For each output `j`:

1. Range-constrain `asset_tag'_j.lo < 2^128` and `asset_tag'_j.hi < 2^128`.
2. Enforce `(asset_tag'_j.lo, asset_tag'_j.hi) != (0, 0)`.
3. Range-constrain `1 <= value'_j < 2^64`.
4. Validate output recipient material:
   ```text
   g_d'_j = OrchardDiversifyHash(d'_j)
   pk_d'_j is a valid Pallas subgroup point
   ```
5. Constrain:
   ```text
   rho'_j = AssetOutputRho(pool_domain, anchor, nf_old, rk, j)
   ```
6. Recompute:
   ```text
   cm'_j  = AssetNoteCommit(pool_domain, g_d'_j, pk_d'_j, asset_tag'_j, value'_j, rho'_j, psi'_j, rcm'_j)
   cmx'_j = ExtractP(cm'_j)
   ```
7. Constrain:
   ```text
   cmx'_j == public cmx_new[j]
   ```

The circuit MUST enforce:

```text
cmx_new[0] != cmx_new[1]
```

Consensus also rejects duplicate output commitments.

### 7.5 Conservation constraints

`AssetOrchardSwapV1` proves multiset equality of the two private `(asset_tag, value)` input pairs and the two private output pairs.

There is no distinct-asset requirement. Same-asset swaps are allowed. If both input tags are equal, the circuit still enforces exact pairwise value preservation up to permutation; it does not permit merging, splitting, or aggregate-only reshaping.

Constrain `s` to be boolean:

```text
s * (1 - s) = 0
```

For each tag limb and value:

```text
asset_tag'_0.lo = select(s, asset_tag_1.lo, asset_tag_0.lo)
asset_tag'_0.hi = select(s, asset_tag_1.hi, asset_tag_0.hi)
value'_0        = select(s, value_1,        value_0)

asset_tag'_1.lo = select(s, asset_tag_0.lo, asset_tag_1.lo)
asset_tag'_1.hi = select(s, asset_tag_0.hi, asset_tag_1.hi)
value'_1        = select(s, value_0,        value_1)
```

where:

```text
select(s, a, b) = b + s * (a - b)
```

This proves per-asset conservation without revealing the asset tags or amounts.

### 7.6 Action binding constraints

The circuit MUST recompute:

```text
(action_ctx_0, action_ctx_1) = H_action(ActionBindingFieldsV1)
```

from the public inputs and fixed constants, then constrain:

```text
action_ctx_0 == public instance[17]
action_ctx_1 == public instance[18]
```

The circuit MUST range-constrain all `eo_hash[j][k]` public limbs to 128 bits.

### 7.7 Domain binding

Domain binding is enforced through all of:

1. `pool_domain` is included in every old and new asset note commitment.
2. `pool_domain` is included directly in every nullifier.
3. `pool_domain` is included in `H_action`.
4. `pool_domain` is included in `H_sig`.
5. Consensus recomputes `pool_domain` from local chain/genesis/protocol/pool parameters.
6. Consensus rejects any mismatch before proof verification.

## 8. Binding Model and Substitution Resistance

The binding model is deliberately two-layered:

1. Consensus parses a bounded canonical action and recomputes all derived public fields:
   - `pool_domain`,
   - encrypted-output hash limbs,
   - `H_action`,
   - `swap_binding_hash`,
   - `H_sig`.
2. The circuit recomputes `H_action` from the public instance.
3. The Halo2 verifier checks the proof against the exact public instance constructed by consensus.
4. RedPallas verifies spend authorization signatures over `H_sig`, which includes the raw encrypted output bytes and the serialized `swap_binding_hash`.

Consequences:

- Changing an anchor, nullifier, randomized verification key, output commitment, fee, encrypted-output hash limb, or action context changes the public instance. The old proof does not verify.
- Changing encrypted output bytes changes `eo_hash`, `H_action`, `swap_binding_hash`, and `H_sig`. The old proof and old signatures do not verify unless the attacker finds relevant SHA3/RedPallas breaks.
- Supplying a malformed or alternate serialization is rejected by the canonical parser before proof verification.
- Supplying a forged `swap_binding_hash` is rejected because consensus recomputes it and the circuit constrains it.
- Replaying a proof across chains, genesis hashes, protocol versions, pools, or circuit ids changes `pool_domain`, `H_action`, or `H_sig`, and fails verification.
- Replacing a valid proof with another valid proof for the same public instance does not change the state transition and is not a conservation or authorization break.

## 9. Spend Authorization

The SNARK proves that each public randomized verification key `rk_i` is derived from the private spend authority tied to the input note recipient.

Consensus verifies RedPallas signatures outside the SNARK:

```text
VerifyRedPallas(rk_i, asset_orchard_swap_sighash, sig_i) == true
```

for both inputs.

The signature message includes:

- chain id,
- genesis hash,
- protocol version,
- pool id,
- proof system id,
- circuit id,
- schema,
- pool domain,
- anchor,
- nullifiers,
- randomized verification keys,
- output commitments,
- encrypted output bytes,
- swap binding hash,
- fee.

This gives explicit owner consent for the exact public action and ciphertexts while keeping spend keys private.

## 10. Key Setup

### 10.1 Trusted setup

The Orchard/Halo2 backend used here is the Halo/IPA polynomial commitment scheme exposed through:

```text
halo2_proofs::poly::commitment::Params::new(K)
```

Those parameters are generated transparently from fixed domains. There is no toxic waste ceremony for this backend.

This statement applies only to the pinned Orchard/Zcash-style IPA backend. Any switch to KZG or another backend is a consensus cryptography change.

### 10.2 Proving key

The proving key is not consensus-trusted. Wallets/operators may generate or download it for:

```text
circuit_id = "asset_orchard.swap.v1"
K          = pinned circuit size selected after implementation benchmarking
params     = Params::new(K)
pk         = keygen_pk(params, vk, AssetOrchardSwapCircuit::empty())
```

A proving-key cache is untrusted. Implementations MUST be able to regenerate the proving key from source, pinned parameters, and circuit constants.

### 10.3 Verifying key pinning

The verifying key is consensus-critical. The activation release MUST pin:

```text
asset_orchard_swap_circuit_id
asset_orchard_swap_k
asset_orchard_swap_params_hash
asset_orchard_swap_vk_hash
asset_orchard_swap_public_instance_layout_hash
poseidon_constants_hash
sinsemilla_generator_hash
orchard_key_derivation_parameter_hash
merkle_tree_depth
merkle_hash_parameter_hash
activation_protocol_version
activation_height_or_epoch
```

At startup and before activation, a node MUST either:

1. rebuild the verifying key from source and assert all pinned hashes match, or
2. load a serialized verifying key and assert its canonical hash and metadata hashes match.

If any hash check fails, the node MUST refuse to accept `AssetOrchardSwapV1` actions. Silent verifying-key changes are consensus faults.

## 11. Consensus Integration

The new verifier replaces the fail-closed scaffold verifier with a versioned verifier:

```text
verify_serialized_asset_orchard_swap_action(action, local_domain):
    parse bounded canonical action encoding
    reject unknown version/schema/proof_system_id/circuit_id
    reject pool_id != "asset-orchard-v1"
    reject fee != 0
    reject non-canonical field, scalar, point, and signature encodings
    decompress and validate rk[0], rk[1]
    recompute pool_domain from local chain/genesis/protocol/pool
    reject action.pool_domain mismatch
    recompute encrypted-output hash limbs from encrypted_outputs
    recompute H_action and swap_binding_hash
    reject serialized swap_binding_hash mismatch
    compute H_sig over the canonical parsed action
    verify both RedPallas spend authorization signatures
    construct the public instance vector in the pinned order
    verify Halo2 proof against the pinned AssetOrchardSwap verifying key
    return VerifiedAssetOrchardSwap
```

The apply path then performs public-state checks:

1. Reject duplicate nullifiers inside the action.
2. Reject nullifiers already present in the pool.
3. Reject duplicate output commitments inside the action.
4. Reject output commitments already present in the pool.
5. Reject unretained anchors.
6. Append nullifiers, output commitments, encrypted outputs, and accepted anchor.
7. Recompute and append the new note tree root.
8. Emit an accepted receipt indexed for disclosure lookup.

No raw asset ids or values enter consensus for internal swaps.

### 11.1 Verifying-key pinning failure-mode checklist

Consensus implementation MUST include explicit tests and runtime checks for:

- wrong `K`,
- wrong IPA parameter hash,
- wrong verifying-key hash,
- wrong public-instance length,
- public-instance field order mismatch,
- wrong Poseidon constants,
- wrong Sinsemilla generators,
- wrong Orchard key-derivation parameters,
- wrong Merkle depth or Merkle hash parameters,
- wrong circuit id or proof system id,
- accidental fallback to the old transcript-scaffold `ShieldedSwapAction`,
- accepting a proof generated for a different circuit with the same serialized action,
- accepting a serialized verifying key downloaded from the network without hash validation.

Any such condition MUST fail closed.

## 12. Pool Versioning and Edge Actions

Asset-typed notes are not spend-compatible with legacy value-only Orchard notes.

Deployment uses one versioned asset-typed pool:

```text
pool_id = "asset-orchard-v1"
```

This is a single pool for all assets. It reveals only that the transaction uses the asset-typed shielded pool; it does not reveal which NAVCoin or asset is being swapped.

Public edge actions are responsible for introducing and removing asset-typed notes:

- shield deposits expose public `asset_id`, `asset_tag`, and value;
- the current disclosed egress path exposes public `asset_id`, `asset_tag`, value, note opening, nullifier, and destination;
- edge consensus MUST check `AssetTag(asset_id) == asset_tag`;
- edge accounting MUST update transparent reserves/liabilities;
- edge actions MUST use the same `AssetNoteCommit` domain and `pool_domain`.

Migration from existing value-only Orchard notes MUST be explicit:

- transparent exit and asset-typed redeposit, or
- a reviewed migration circuit/action that proves ownership of a value-only note and mints an asset-typed note only when the asset identity is known through a public or disclosed edge.

Do not infer asset identity for old value-only notes. They do not carry asset identity in their commitments.

## 13. Conjectured Soundness Pending External Review

The intended theorem is:

> Under the assumptions below, no polynomial-time adversary can cause consensus to accept an `AssetOrchardSwapV1` action that spends unauthorized notes, creates value, destroys value outside the exact two-note permutation, relabels an asset, or binds a proof to a different public action transcript.

This theorem is an implemented-circuit claim pending external cryptographic review. Public-network privacy claims MUST treat it as conjectural until review and the breaking-test suite remain green on the release candidate.

### 13.1 Assumptions

1. Halo2 proof-system soundness for the pinned circuit, parameters, and verifying key.
2. Binding and collision resistance of Sinsemilla commitments and the Orchard Merkle tree hash.
3. Collision resistance and PRF-like behavior of the pinned Poseidon instantiations.
4. Unforgeability of RedPallas spend authorization signatures.
5. Correctness and soundness of the pinned Orchard address/key-derivation gadgets.
6. Collision and second-preimage resistance of `AssetTag`.
7. Collision resistance of SHA3-384 encrypted-output hashes and SHA3-256 signature hashes.
8. Canonical consensus serialization and deterministic public-instance construction.
9. Edge actions correctly enforce public asset-id/tag mapping and transparent reserve accounting.

### 13.2 Soundness sketch

- To spend an input, the prover must open an asset-typed note commitment under the public anchor. That commitment includes `pool_domain`, `asset_tag`, `value`, recipient material, `rho`, and `psi`.
- The Merkle path is checked against `cmx = ExtractP(cm)`, so the witness must correspond to an existing note leaf.
- The nullifier is recomputed from the same `cmx`, `pool_domain`, `rho`, `psi`, and private `nk`. A public nullifier cannot be attached to a different hidden asset/value witness without breaking the circuit constraints or commitment binding.
- The Orchard authority relation ties `nk` and `ak` to the note recipient. A prover cannot spend someone else’s note by substituting an unrelated nullifier key.
- The public randomized verification key is constrained from `ak`, and consensus verifies a RedPallas signature under that key over the action sighash.
- Each output `cmx` is recomputed inside the circuit from a private output asset tag and value.
- The permutation constraints force the two output `(asset_tag, value)` pairs to equal the two input pairs, possibly reordered. This is sufficient for per-asset conservation; no distinctness assumption is needed.
- Nonzero value constraints prevent zero-value degenerate notes.
- All-zero asset tags are rejected in-circuit and at public edges.
- Consensus rejects duplicate nullifiers and duplicate output commitments.
- `H_action` is recomputed inside the circuit, while consensus recomputes both `H_action` and `H_sig` from canonical serialized action fields. Transcript substitution changes public inputs or signatures and fails.

### 13.3 Failure-mode analysis

The implementation MUST explicitly guard against these failure modes:

| Failure mode | Consequence | v1 mitigation |
| --- | --- | --- |
| Omit asset tag from note commitment | Asset relabeling possible | `AssetNoteCommit` includes both tag limbs |
| Omit `cmx` from nullifier | Nullifier not bound to committed asset/value | `AssetDeriveNullifier` includes `cmx` |
| Omit `pool_domain` from commitments/nullifiers | Cross-chain or cross-pool replay | Included directly in both |
| Omit Orchard address authority relation | Anyone could spend any opened note | Mandatory `pk_d = [ivk]g_d` and `rk` constraints |
| Treat `swap_binding_hash` as unconstrained public data | Proof transplant/substitution | Circuit recomputes `H_action` |
| Let users supply encrypted-output hashes | Ciphertext substitution | Consensus recomputes hashes from bytes |
| Accept non-canonical encodings | Malleability or instance mismatch | Canonical parser rejects before proof |
| Allow zero values | Degenerate swaps and audit ambiguity | Values constrained `1..2^64-1` |
| Rely on input asset distinctness for soundness | Same-asset edge cases unclear | Distinctness removed; permutation is the invariant |
| Silently change verifying key or layout | Consensus split or invalid proof acceptance | Pinned hashes and startup refusal |
| Edge action skips `AssetTag(asset_id)` check | Bogus tags can enter pool | Edge consensus mandatory check and registry |

## 14. Privacy Argument

For internal `AssetOrchardSwapV1` actions, the public chain sees:

- the single asset-typed pool id,
- anchor,
- two nullifiers,
- two randomized verification keys,
- two output commitments,
- two encrypted outputs,
- encrypted-output hash limbs derived by consensus,
- swap binding hash,
- proof bytes,
- spend authorization signatures,
- fixed zero shielded fee,
- timing and block inclusion.

The public chain does not see:

- input asset ids,
- output asset ids,
- input amounts,
- output amounts,
- price,
- sender identities,
- recipient identities,
- which NAVCoin was selected,
- which output belongs to which party.

The action shape leaks that a two-input/two-output private settlement occurred in the asset-typed pool. That fixed-shape leak is intentional in v1.

Remaining privacy limits:

- Bridge-in and bridge-out edges are public.
- Primary mint and redemption edges that change NAVCoin reserves are public at aggregate level.
- Timing and amount correlation can be inferred from public edge flows, especially for large flows into small NAVCoins.
- Voluntary disclosure reveals disclosed note facts to the verifier.
- Bad ciphertext can burn funds or reveal information to intended recipients, but does not affect conservation.

## 15. Auditable Reserves

The shielded swap circuit does not hide backing reserves and does not mint backing assets.

Reserve accounting remains transparent at the edges:

- Bridge deposits custody assets transparently and mint shielded claims through reviewed bridge/turnstile actions.
- NAVCoin primary mints and exits update transparent reserve packets and NAV accounting.
- Shielded swaps transfer already-issued shielded claims between private holders.

The invariant is inductive:

1. Public edge ingress creates asset-typed notes only after transparent asset-id/tag/value checks and reserve accounting.
2. Internal swaps preserve the exact multiset of two `(asset_tag, value)` pairs.
3. Current public edge egress nullifies asset-typed notes only after disclosed validation proves the spent private tag/value equals the public withdrawal asset/value. This is functional egress, not private egress.

Therefore a private swap cannot create extra a651, destroy pfUSDC accounting, or convert one asset into another. If a user acquires a shielded a651 note through a swap, the backing for that a651 was established before the swap when that note or its ancestor entered the pool.

This preserves the product boundary:

```text
private transfer layer, transparent backing layer
```

Auditors can verify bridge custody, NAVCoin reserve packets, primary mint events, redemption events, and shielded-pool ingress/egress totals. They cannot see every private holder rotation inside the pool, and that is the intended privacy property.

## 16. Viewing Keys and ShieldDisclose

`ShieldDisclose` MUST support asset-typed outputs.

A disclosure packet for a swap-created output contains:

```text
disclosure_version
chain_id
genesis_hash
protocol_version
pool_id
action_id / receipt_id
output_index
cmx
recipient disclosure data
asset_id
asset_tag_lo
asset_tag_hi
value
rho
rseed
psi
rcm
memo disclosure policy
accepted action evidence
inclusion proof or archived receipt evidence
viewer signature or owner authorization
```

A verifier checks:

1. The referenced action was accepted by consensus.
2. The output index belongs to that accepted action.
3. The action’s public output commitment at `output_index` equals `cmx`.
4. `AssetTag(asset_id)` equals the disclosed tag.
5. The tag is not all-zero.
6. `psi == OrchardPsi(rseed, rho)`.
7. `rcm == OrchardRcm(rseed, rho)`.
8. For swap outputs, `rho == AssetOutputRho(pool_domain, anchor, nf_old, rk, output_index)`.
9. Recomputed `AssetNoteCommit(...)` equals public `cmx`.
10. The packet hash and owner/viewer authorization match the current `ShieldDisclose` policy.

Incoming viewing keys remain the normal recipient-side scan mechanism. A viewing key can reveal a specific output without revealing unrelated notes or the whole pool.

Compliance implication: this remains in the Orchard/Zcash lineage. Default chain state hides asset/value/party data, while holders can selectively disclose note facts using viewing-key material and archived inclusion evidence.

## 17. Circuit Size and Performance Estimate

The following is an implementation budget, not a consensus parameter. Actual activation values MUST be measured and pinned.

Expected circuit components:

| Component | Rough row budget |
| --- | ---: |
| Two input note commitments and openings | 70k–130k |
| Two Orchard authority relations and randomized keys | 60k–140k |
| Two Merkle paths at Orchard depth | 120k–220k |
| Two asset nullifiers | 10k–30k |
| Two output note commitments | 70k–130k |
| Output rho, permutation, nonzero/range checks | 20k–50k |
| In-circuit `H_action` Poseidon binding | 5k–20k |
| Equality, duplicate, and public-input plumbing | 5k–20k |
| **Estimated total** | **360k–740k rows** |

Expected proving parameter:

```text
target K: 20
acceptable after benchmarking: K <= 21
new review required if K > 21
```

Indicative costs on contemporary server hardware:

```text
proof size:       approximately 20–80 KiB for the IPA backend
proving time:     approximately 2–15 seconds per swap proof
verification:     approximately 10–50 ms per proof, batchable where supported
prover memory:    approximately 1–4 GiB
```

Activation MUST include benchmark results for the exact release build, CPU class, proving key, verifying key, and `K`.

## 18. Required Tests Before Implementation Gate

The implementation MUST include executable CI tests for at least the following.

### 18.1 Good paths

1. Valid two-asset swap accepted.
2. Valid same-asset exact two-note settlement accepted.
3. Valid output order with `s = 0`.
4. Valid output order with `s = 1`.
5. Disclosure lookup and note reconstruction for both swap outputs.

### 18.2 Conservation and permutation breaking tests

6. Value non-conservation rejected: construct outputs with one value incremented by one; `MockProver` must report unsatisfied conservation constraints and any serialized proof must fail consensus verification.
7. Asset non-conservation rejected: relabel one output tag while preserving values; proof verification must fail.
8. Same-asset aggregate split rejected: inputs `(A, 10)` and `(A, 20)` with outputs `(A, 15)` and `(A, 15)` must fail.
9. Invalid permutation bit rejected: witness `s` not in `{0,1}` must fail.
10. Zero input value rejected.
11. Zero output value rejected.
12. All-zero input asset tag rejected.
13. All-zero output asset tag rejected.

### 18.3 Spend and membership breaking tests

14. Unauthorized spend rejected by RedPallas signature verification.
15. Foreign-note spend rejected: attempt to open a valid note with unrelated `(ak, nk, rivk)`; authority constraints must fail.
16. Wrong Merkle path rejected.
17. Wrong anchor rejected.
18. Duplicate input nullifiers rejected in-circuit and by consensus.
19. Nullifier replay rejected by state.
20. Duplicate output commitments rejected in-circuit and by consensus.
21. Existing output commitment replay rejected by state.

### 18.4 Binding and serialization breaking tests

22. Tampered `swap_binding_hash` rejected before proof verification.
23. Tampered encrypted output bytes rejected with old proof/signatures.
24. Tampered encrypted-output hash limbs rejected because consensus recomputes them.
25. Tampered randomized verification key rejected.
26. Tampered output commitment rejected.
27. Wrong chain id rejected.
28. Wrong genesis hash rejected.
29. Wrong protocol version rejected.
30. Wrong pool id rejected.
31. Wrong `pool_domain` rejected.
32. Non-canonical field encodings rejected before proof verification.
33. Non-canonical point encodings rejected before proof verification.
34. Extra trailing bytes or alternate action encodings rejected by the canonical parser.

### 18.5 Forged non-conserving proof test

The key regression test MUST be executable as follows:

1. Build two valid input notes under a retained anchor.
2. Build a malicious witness whose output commitments encode non-conserving `(asset_tag, value)` pairs.
3. Run the real `AssetOrchardSwapCircuit` with `MockProver`; it MUST fail at named conservation constraints.
4. Attempt to verify:
   - a proof generated by the old SHA3 transcript scaffold,
   - a proof generated for a test circuit with conservation constraints removed,
   - a valid proof whose public output commitment fields are mutated after proving.
5. All three MUST fail against the pinned production verifying key.

### 18.6 Verifying-key and parameter pinning tests

35. Wrong `K` rejected.
36. Wrong IPA params hash rejected.
37. Wrong verifying-key hash rejected.
38. Wrong public-instance layout hash rejected.
39. Wrong Poseidon constants rejected.
40. Wrong Sinsemilla generators rejected.
41. Wrong Orchard key-derivation parameter hash rejected.
42. Wrong Merkle depth/hash parameters rejected.
43. Node refuses to activate if any pinned hash is missing.
44. Node refuses to fall back to the scaffold verifier.

### 18.7 Asset tag and edge tests

45. Public deposit with mismatched `asset_id`/`AssetTag` rejected.
46. Public withdrawal with mismatched `asset_id`/`AssetTag` rejected.
47. Non-canonical asset id rejected before tag derivation.
48. Asset registry duplicate tag for different canonical asset id rejected.
49. Malformed asset tag limbs outside 128-bit range rejected by edge circuits and by private circuit range checks.

### 18.8 State and archive tests

50. Accepted swap output commitments are appended to the note tree.
51. New note tree root recomputation matches archive.
52. Archive/state mismatch rejected.
53. Accepted receipts are indexed for disclosure lookup.
54. Disclosure verifier rejects packets referencing unaccepted actions or wrong output indexes.

## 19. Normative v1 Boundary and Future Work

The following are v1 normative requirements, not open questions:

- in-circuit `H_action` binding,
- two-limb `AssetTag` construction,
- all-zero asset tag rejection,
- direct `pool_domain` inclusion in commitments and nullifiers,
- deterministic `AssetOutputRho`,
- exact two-input/two-output permutation conservation,
- `fee == 0`,
- no asset-distinctness requirement.

Future circuit ids may add:

- shielded fees,
- split/change outputs,
- merge/split private transfers,
- public policy constraints such as NAV-band checks,
- encryption correctness proofs,
- alternative asset-tag encodings,
- batched or aggregated swap proofs.

Any such change is outside `asset_orchard.swap.v1` and requires a new circuit id, new verifying key, new public-instance layout, and new cryptographic review.
