# Halo2 Dependency And Local Patch Boundary

PostFiat does **not** implement or replace the Halo2 proof system. The privacy
code uses Electric Coin Company's upstream Rust implementation,
`halo2_proofs 0.3.2`, from the Zcash `halo2` repository. The exact upstream
source is retained in this repository with a small compatibility patch so that
validators can load a release-pinned verifying-key assembly instead of
rebuilding it on the transaction path.

## Exact Source Boundary

| Item | Pinned value |
| --- | --- |
| Upstream project | [`zcash/halo2`](https://github.com/zcash/halo2) |
| Package | `halo2_proofs 0.3.2` |
| Upstream commit | `f6200adaa6ca064d8d2eaa6fcc5e2671232d7249` |
| Local source location | `third_party/halo2_proofs` |
| Cargo selection | Root `[patch.crates-io]` path override |
| Upstream license | `MIT OR Apache-2.0` |
| Normalized local patch | 361 lines / 12,958 bytes |
| Patch SHA-256 | `d51e2e6edaa55be0910f4a72b1fd66ef9f634f9037437247ab3d25f6eb0d7a73` |

Keeping the dependency source in-tree makes the exact bytes used by a build
inspectable and reproducible. It does not make Halo2 a PostFiat-designed proof
system.

## What The Patch Changes

The local patch adds a bounded interface for serializing and reconstructing the
pinned verifying-key assembly used by the PostFiat privacy verifier. It also
contains the directly associated key-generation, permutation-commitment, and
typed-error support, plus packaging and lint compatibility changes.

This interface exists because the upstream `VerifyingKey` fields required for
the pinned artifact are private and upstream `halo2_proofs 0.3.2` does not
expose the required deserialization or `from_parts` API.

## What The Patch Does Not Change

The patch does not intentionally change:

- the Halo2 proving algorithm;
- verifier equations;
- transcript construction;
- fields or curves;
- the polynomial commitment scheme; or
- proof encoding.

PostFiat circuits and their public-input bindings remain PostFiat's review
boundary. The underlying Halo2 proof-system implementation remains the pinned
upstream Zcash implementation with the compatibility patch described above.

## Reproducible Verification

From the repository root, run:

```bash
scripts/verify-vendored-halo2
```

The verifier compares the local tree with a fresh checkout of the immutable
upstream commit. It fails closed on an unexpected source file, changed license,
different upstream revision, or normalized patch mismatch.

The complete machine-checkable source and license inventory is in
`third_party/halo2_proofs/PROVENANCE.md`. A future dependency update or patch
change requires new compatibility vectors and cryptographic review; changing a
recorded hash alone is not review.

