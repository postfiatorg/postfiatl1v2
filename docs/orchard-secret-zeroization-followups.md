# Orchard secret zeroization follow-ups (#181, after #201)

This is a review of Orchard and Asset-Orchard secret lifetimes after
[#201](https://github.com/postfiatorg/postfiatl1v2/pull/201) merged. It lists
gaps that #201 and the
[#181](https://github.com/postfiatorg/postfiatl1v2/issues/181) gap comment do
not cover. Items already listed there (upstream `SigningKey<SpendAuth>`,
orchard `SpendingKey` and the ZIP32 intermediates, halo2 `Value<T>`, by-value
pasta arguments to `scalar_enc`, `orchard_commit_ivk` and
`asset_derive_nullifier`) are not repeated.

## Reviewed commit

- `main` at `78fc09ad8ff68da1b4a9dfdebac280207f0e658a`.
- No commits touch `crates/privacy_orchard` after `431231fc4`, so the line
  numbers in the #181 gap comment still match.
- Pinned versions: orchard 0.14.0, pasta_curves 0.5.1, halo2_proofs 0.3,
  zeroize 1.8.2, sha3 0.11.0, serde_json 1.0.149, chacha20poly1305 0.10.1.

## Scope and files inspected

Scope is memory lifetime and redaction of secret material in process memory:
creation, `Clone`/`Copy`, serde and `Debug`, temporary `Vec`/`String`/array
buffers, hex encoding, and drop on success, `?` and early-return paths.

- `crates/privacy_orchard/src/asset_orchard.rs`: secret owners, wallet-note
  derivation, hash-to-field, accounting blindings, note commitments, hex
  parsing, disclosed egress.
- `crates/privacy_orchard/src/asset_orchard_note_encryption.rs`: encrypt and
  decrypt of wallet notes.
- `crates/privacy_orchard/src/asset_orchard_action_builders.rs`: swap and
  private-egress builders, the point-3 note at lines 1603-1641.
- `crates/privacy_orchard/src/asset_orchard_circuit.rs`,
  `asset_orchard_sinsemilla.rs`: witness types and synthesis (no new local
  buffers found beyond the halo2 gap already listed).
- `crates/privacy_orchard/src/verify.rs`: plain Orchard scan, spend, withdraw
  and ZIP32 entry points.
- `crates/crypto_provider/src/lib.rs`: `bytes_to_hex`, `hex_to_bytes`.
- Workspace `Cargo.toml` and `Cargo.lock`: feature flags of the hash and AEAD
  crates.
- `crates/node/src/orchard_policy_actions.rs`: call sites only, to see which
  paths are reached.

## How the findings were checked

Each confirmed item below was seen in the code at the reviewed commit. Items
1, 2, 3, 5, 6 and 7 were also checked with a local probe that is not part of
this change. The probe is an integration test binary with a wrapping
`#[global_allocator]`. When a block is handed back to the allocator (`dealloc`,
or a `realloc` that moves the block), the wrapper scans the block for a known
canary byte string, then frees it. It reads only memory the program still
owns, so it does not inspect freed memory. A buffer that is wiped before drop
(`Zeroizing`, `AssetOrchardSecret`) does not match. The canaries are values
from the crate's own non-secret test notes or fixed marker strings, never real
keys. Results were the same in debug and release builds.

The same allocator pattern is the suggested way to observe the wipe in the
regression tests below. It needs its own test binary because a crate can have
only one global allocator.

## Confirmed defects (ranked)

### 1. Note encryption leaves partial plaintext copies when the buffer grows

- Where: `crates/privacy_orchard/src/asset_orchard_note_encryption.rs:107`,
  `encrypt_asset_orchard_wallet_note_with_rng`.
- How it survives: `Zeroizing::new(serde_json::to_vec(note))` wraps only the
  final buffer. `serde_json::to_vec` starts small and doubles the `Vec` as it
  writes. Each growth frees the old block without a wipe, and the old block
  holds a prefix of the wallet note JSON. The JSON field order puts the note
  opening, `nk` and `rivk` before `spend_auth_signing_key` and `rseed`.
- Probe: for a test note of 1117 JSON bytes, the freed growth blocks held
  `rcm` once, `nk` twice and `rivk` once. `spend_auth_signing_key` and `rseed`
  were only in the final buffer, which is wiped. Which fields land in a freed
  block depends on the JSON length (asset id length, value digits).
- Impact: every successful encryption leaves `nk`, `rivk` and the note opening
  in freed heap blocks. This is the plaintext #181 point 4 lists as already
  `Zeroizing`.
- Smallest fix: serialize into a buffer that cannot grow. For example, a
  `Zeroizing<Vec<u8>>` of `ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES` zeros,
  written through `std::io::Cursor<&mut [u8]>` with `serde_json::to_writer`,
  then encrypt `&buf[..cursor.position()]`. Size it at the largest plaintext
  the ciphertext bound allows (the bound minus header and tag), and map a
  write overflow to the existing bound error. The ciphertext bytes do not
  change.
- Regression test: `note_encryption_plaintext_is_never_freed_unwiped`. In an
  allocator-probe test binary, arm the canaries `nk`, `rivk`,
  `spend_auth_signing_key`, `rseed` and `rcm` of a test note, call
  `encrypt_asset_orchard_wallet_note`, and assert zero hits. Keep the existing
  ciphertext pin test unchanged.

### 2. `orchard_psi` and `orchard_rcm` copy `rseed` into a plain `Vec`

- Where: `crates/privacy_orchard/src/asset_orchard.rs:2217` (`orchard_psi`)
  and `:2227` (`orchard_rcm`).
- How it survives: `msg` is a plain `Vec<u8>` holding the 32 raw `rseed` bytes
  and `rho`. It is dropped without a wipe on return. The caller's `rseed` is a
  `Zeroizing` owner (`asset_orchard.rs:633`), but this copy is not.
- Probe: one freed block with the canary for each call, and two freed blocks
  with the note's raw `rseed` per `build_asset_orchard_wallet_note` call.
- Impact: every wallet-note build leaves the raw `rseed` twice in freed heap.
  This includes the `template` and `expected` builds inside
  `decrypt_asset_orchard_wallet_note`, so each successful scan adds more.
- Smallest fix: build `msg` as `Zeroizing<[u8; ASSET_ORCHARD_RSEED_BYTES +
  ASSET_ORCHARD_FIELD_BYTES]>` (or a `Zeroizing<Vec<u8>>` with the exact
  capacity) and pass `&msg[..]`. Values do not change.
- Regression test: `orchard_psi_and_rcm_wipe_the_rseed_message`. Arm a fixed
  marker array as `rseed`, call both functions, and assert zero hits. The
  existing `orchard_psi_and_rcm_bind_rseed_and_rho` keeps the values pinned.

### 3. The accounting blinding payload copies `rseed` hex into a growing `Vec`

- Where: `crates/privacy_orchard/src/asset_orchard.rs:2140`,
  `accounting_blinding_from_note`, reached from the public
  `asset_orchard_swap_accounting_records` (`:2069`).
- How it survives: `payload` starts as `Vec::new()` and grows while the role,
  index, output commitment and the note's `rseed` hex are appended. The final
  buffer and any grown-out buffers are freed without a wipe.
- Probe: one freed block with the input note's `rseed` hex and one with the
  output note's `rseed` hex, for one call.
- Impact: each swap build leaves the `rseed` of each blinded leg in freed heap.
- Smallest fix: `Zeroizing::new(Vec::with_capacity(n))` with the exact
  length, so it never grows and is wiped on every exit. The hash input is
  unchanged.
- Regression test: `accounting_blinding_payload_is_wiped`. Arm the `rseed` hex
  of each note, call `asset_orchard_swap_accounting_records`, and assert zero
  hits. Existing accounting pin tests keep the blindings unchanged.

### 4. SHA3 hasher state is dropped without a wipe

- Where: workspace `Cargo.toml:61` (`sha3 = "0.11"`, no `zeroize` feature).
  The hashers are in `asset_orchard.rs:757` (`derive_bytes`), `:4033`
  (`hash_to_field_digest`, used for `nk`, `rivk`, the spend scalar, `psi` and
  `rcm`) and `asset_orchard_note_encryption.rs:254` (`note_encryption_key`,
  which absorbs the shared point and `pk_d`).
- How it survives: sha3 0.11.0 wipes the Keccak state in `Drop` only under its
  optional `zeroize` feature (`sha3-0.11.0/src/block_api.rs:168-177`).
  `cargo tree -e features` shows only `alloc`, `default` and `oid` enabled.
  The block buffer and state, which hold absorbed seed and key bytes, are left
  on the stack.
- Impact: stack copies of seed and key-derivation input after every
  derivation and every note encryption key. No probe; the stack cannot be
  observed without undefined behavior.
- Smallest fix: `sha3 = { version = "0.11", features = ["zeroize"] }`. This
  enables `ZeroizeOnDrop` on `Sha3_256` and `Sha3_512`. Digest values do not
  change.
- Regression test: `sha3_hashers_zeroize_on_drop`, a compile-time check
  `fn assert_zeroize_on_drop<T: zeroize::ZeroizeOnDrop>() {}` called for
  `sha3::Sha3_256` and `sha3::Sha3_512`. It fails to build if the feature is
  dropped.

### 5. Plain Orchard `rseed` is a plain `String` with derived `Debug`

- Where: `crates/privacy_orchard/src/verify.rs:286-296`
  (`OrchardDecryptedOutput`) and `:315-326` (`OrchardSpendNote`); filled at
  `:2775` by `orchard_scan_encrypted_outputs_report_with_full_viewing_key`.
- How it survives: both types `#[derive(Debug, Clone, Serialize)]` with
  `rseed: String`. `Debug` prints `rseed`, and drop frees it without a wipe.
  This is the plain-Orchard twin of #181 point 2, which was fixed only for the
  Asset-Orchard opening.
- Probe: `format!("{:?}")` of an `OrchardSpendNote` contains a marker `rseed`,
  and dropping it frees one block with the marker.
- Impact: any `Debug` formatting of a scan result or spend note prints the
  note's `rseed`. With `rho` and the address, that is the full note opening.
- Smallest fix: make `rseed` an `AssetOrchardSecret<String>` (serde is
  `transparent`, so the wire form is unchanged), or give both types a
  redacting `Debug` and a `Drop` that wipes `rseed`.
- Regression test: `orchard_scan_and_spend_note_redact_and_wipe_rseed`.
  Assert `Debug` output has no marker `rseed`, assert zero allocator hits when
  the value drops, and assert the serde JSON is byte-identical to the current
  form.

### 6. Hex field types are not wiped on deserialize error paths

- Where: `crates/privacy_orchard/src/asset_orchard.rs:321-365`
  (`AssetOrchardFieldElement`), `:369-437` (`AssetOrchardPoint`) and `:4264`
  (`parse_fixed_lower_hex`).
- How it survives: both types implement `Zeroize` but not `Drop`; only the
  owner that embeds them wipes them. `parse_fixed_lower_hex` takes the input
  `String` by value and drops it unwiped when validation fails.
  1. A wallet note whose `nk` is valid hex but not a canonical field element
     fails in `TryFrom<String>`, and the `nk` string is freed unwiped.
  2. When `AssetOrchardPublicNoteOpening` fails part way (for example, a
     missing `rcm`), the fields already parsed (`diversifier`, `g_d`, `pk_d`,
     `rho`, `psi`) are freed unwiped. The opening's own `Drop` never runs
     because the struct was never built.
- Probe: case 1 freed one block with the rejected `nk`; case 2 freed one
  block with `rho`. Control: a successful parse that is then dropped freed no
  block with `nk`.
- Impact: error paths only. They are reached by reading a malformed wallet
  note file and by `decrypt_asset_orchard_wallet_note` on a plaintext that
  decrypts but does not parse.
- Smallest fix: wipe `value` before returning `Err` in `parse_fixed_lower_hex`
  and in both `parse_hex` functions. Add `Drop` (calling `zeroize`) to both
  types, and change the `From<..> for String` impls to `std::mem::take`.
  Public values are wiped too, which is harmless.
- Regression test: `wallet_note_parse_errors_wipe_parsed_fields`. Arm a
  marker `nk` and the opening `rho`, deserialize both malformed notes, assert
  the error codes are unchanged and assert zero hits.

### 7. Note commitment bit vectors hold the opening in plain `Vec<bool>`

- Where: `crates/privacy_orchard/src/asset_orchard.rs:3772`
  (`asset_note_message_bits`), called from `commitment_point` (`:3754`) on
  every `cmx()`; temporaries from `i2lebsp_from_field` (`:4069`) and
  `point_bits` (`:4064`).
- How it survives: the message bits of `g_d`, `pk_d`, value, asset tag, `rho`
  and `psi` are built in a plain `Vec<bool>`, plus one temporary `Vec<bool>`
  per field. All are freed unwiped after each commitment.
- Probe: one `cmx()` call freed three blocks holding the 255-bit `psi` run.
- Impact: every commitment check (wallet-note validation, swap build, decrypt)
  leaves the opening bits in freed heap. This is custody material, not a
  spend key.
- Smallest fix: `commitment_point` uses a private variant that returns
  `Zeroizing<Vec<bool>>` (zeroize implements `Zeroize` for `bool`) and writes
  each field's bits straight into the pre-sized vector. The public
  `asset_note_message_bits` signature can stay.
- Regression test: `note_commitment_bits_are_wiped`. Arm the `psi` bit run,
  call `cmx()`, and assert zero hits. The existing host/circuit commitment
  tests keep `cmx` pinned.

### 8. The #181 gap list misses three by-value spending-key entry points

- Where: `crates/privacy_orchard/src/verify.rs:2043`
  (`orchard_build_spend_action`), `:2174` (`orchard_build_withdraw_action`)
  and `:2631` (`orchard_default_address_from_spending_key`), each taking
  `spending_key_bytes: [u8; 32]` by value. The full viewing key also moves
  as a plain `[u8; 96]` (`:2685` return, `:2689`, `:2723`, `:2736`, `:2840`).
- How it survives: the same way as `:2681`, `:2695` and `:2708`, which are
  already listed. The callee's copy is a plain array, and node callers copy
  the key out of its `Zeroizing` owner (for example
  `crates/node/src/orchard_policy_actions.rs:502`).
- Impact: documentation is incomplete, so a later fix that only covers the
  listed functions would leave these.
- Smallest fix: add these lines to the point-3 note in
  `asset_orchard_action_builders.rs:1603-1641` and to the #181 list, or take
  `&[u8; 32]` / `&[u8; 96]` when the public signatures are next changed.
- Regression test: none for the doc change. A signature change is covered by
  the existing spend, withdraw and scan tests.

## Open questions

These were seen in the code but not proven to matter, or could not be
observed.

- Plain stack arrays of derived secrets: `derive_bytes` returns a plain
  `[u8; 32]` (`asset_orchard.rs:757`), and `hash_to_field_digest` returns a
  plain `[u8; 64]` whose 32-byte candidate is copied at `:3997` and `:4016`.
  Wrapping them in `Zeroizing` is cheap, but the wipe cannot be observed
  without reading the stack.
- Accounting blindings: `input_blindings`, `output_blindings` and the totals
  (`asset_orchard.rs:2079-2097`) are plain `pallas::Scalar` values in plain
  `Vec`s. Should value-commitment blindings be treated as custody material?
- `ShieldedSwapPrivateInput` (`verify.rs:328`) derives `Debug` over
  `authorization_secret` and both blindings. It is used by
  `shielded_swap_build_action_test_vector` and node tests. Is this type still
  in use outside test vectors?
- `decrypt_asset_orchard_wallet_note` (`asset_orchard_note_encryption.rs:220`)
  wraps the AEAD output after `decrypt` returns it. The probe did not cover
  whether `aead::Aead::decrypt` grows or copies that buffer internally.

## Not covered

- Upstream types already listed on #181 (orchard, reddsa, zip32, halo2).
- Process-level boundaries (swap, core dumps, worker isolation).
- Node and wallet storage of key files. That is a storage design question, not
  an in-memory lifetime gap.
