# Upstream Halo2 Source And Local Patch Provenance

This directory retains the exact source of upstream `halo2_proofs 0.3.2` from
<https://github.com/zcash/halo2> at immutable upstream commit
`f6200adaa6ca064d8d2eaa6fcc5e2671232d7249` (`path_in_vcs`:
`halo2_proofs`). The upstream license is `MIT OR Apache-2.0`; exact copies of
the upstream `COPYING.md`, `LICENSE-MIT`, and `LICENSE-APACHE` are included in
this directory. PostFiat does not reimplement the Halo2 proof system; Cargo uses
this in-tree upstream snapshot so the bounded local compatibility patch below
is explicit, reproducible, and reviewable.

The local source patch is intentional and limited to:

- bounded serialization and reconstruction of pinned verifier-key assembly;
- the associated key-generation, permutation-commitment, and typed-error
  support;
- one crate-level lint compatibility attribute;
- one whitespace-only cleanup in `src/multicore.rs`;
- the optional `halo2_legacy_pdqsort` compatibility feature in the packaged
  manifest.

No proving algorithm, verifier equation, transcript, field, curve, or proof
encoding is intentionally changed. The normalized upstream-to-vendor patch is
361 lines / 12,958 bytes with SHA-256
`d51e2e6edaa55be0910f4a72b1fd66ef9f634f9037437247ab3d25f6eb0d7a73`.

Run this from the repository root to reproduce the file inventory, license
hashes, and normalized patch hash against a freshly fetched upstream tree:

```bash
scripts/verify-vendored-halo2
```

Any changed file, extra file, upstream commit, license, or patch byte makes the
verifier fail. Updating this vendor requires a new cryptography review and
updated proof/circuit compatibility vectors; editing the expected hash alone is
not review.
