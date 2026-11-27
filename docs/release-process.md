# Release Process

PostFiat L1 is currently controlled pre-testnet software. A tagged release is
an immutable engineering artifact, not a mainnet-readiness claim.

Since 21 November 2026 a release is built from a commit on `main` only
([Master State](status/master-state.md)). A short-lived release branch cut from
`main` may set an activation height (for example `release/sol-exit-r2`); until
it is merged back, the running build is not on `main`. Validators are rolled
one at a time with the release kit, and after each host an accepted transfer
and six-way agreement on height, tip and state root are checked (working rules
of the [23 November handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/handoff-2026-11-23.md)).

1. Start from a clean, reviewed commit with no open P0/P1 publication blocker.
2. Run formatting, locked workspace check/test/Clippy, wallet/proxy tests,
   offline and explicitly configured fork contract suites, strict docs build,
   dependency/license policy, vendored-source verification, and tracked-tree
   plus full-public-history secret scans.
3. Generate the CycloneDX SBOM, signed deployment manifest, checksums, and
   reproducible-build records. A second clean builder must reproduce the node
   hash before promotion.
4. Exercise snapshot migration, rollback compatibility, and a rolling canary.
   Stop on state-root divergence, rejected expected receipts, committee-roster
   mismatch, or any failed conservation/security invariant.
5. Sign the annotated tag and release checksums. Publish only redacted evidence;
   never upload node data, keys, wallet backups, private notes, or raw provider
   captures.
6. Retain the staged prior binary and schema-compatible snapshot until the
   post-release observation window closes.

Production/mainnet promotion additionally requires external cryptography,
circuits, contracts, and protocol review plus HSM/remote-signer custody and a
production storage engine. Those are not implied by this process.

## Public release, signed by two of three

A release that outside operators install is described by one manifest
(`postfiat.release_manifest.v3`) that two of three release signers have signed.
`postfiat-node upgrade` and the pre-start check of the packaged units accept
nothing else. The format and the rules are in
[`validator-rpc-profiles-v1.md`](specs/validator-rpc-profiles-v1.md).

**Once: the keys.** Each of the three signers (release lead, independent
builder, security custodian) makes a key on a machine of its own and hands over
only the public entry the command prints:

```sh
postfiat-node release-key-create --signer-id release-lead-1 --role release_lead --out-file release-lead-1.key.json
```

The three entries, with `threshold: 2`, are the keyset file
(`postfiat.release_keyset.v1`) that operators install at
`/etc/postfiat/release-keyset.json`. The private halves never leave the
signers' machines and are never in this repository.

**Every release:**

1. Tag the commit on `main`. The `release-build` workflow builds
   `postfiat-node` for `linux-x86_64` and `linux-aarch64` and assembles the
   release directory with `scripts/release-assemble`: the unit bundle, the
   SBOM and `SHA256SUMS`. It signs nothing and publishes nothing.
2. The independent builder builds the same tag on a clean machine and compares
   `postfiat-node release-build-info` with `SHA256SUMS`. A different hash stops
   the release.
3. The release lead builds the unsigned manifest from the release directory.
   `--release-sequence` is one more than the last release of the chain; the
   checkpoint is a finalized block the signers have checked on their own nodes.

   ```sh
   postfiat-node release-manifest-create --keyset-file release-keyset.json \
     --release-id v0.3.0-testnet --release-sequence 7 \
     --chain-id ... --genesis-hash ... --git-revision <40 hex> \
     --build-features privacy,rpc,transport --protocol-version 2 --min-compatible-protocol-version 1 \
     --checkpoint-height ... --checkpoint-block-hash ... --checkpoint-state-root ... \
     --checkpoint-certificate-id ... --checkpoint-committee-epoch ... \
     --url-base https://releases.postfiat.org \
     --binary-x86-64 release/postfiat-node-linux-x86_64 --binary-aarch64 release/postfiat-node-linux-aarch64 \
     --systemd-bundle release/postfiat-units.tar --docker-profiles release/postfiat-docker-profiles.tar \
     --sbom release/postfiat-cargo-sbom.cdx.json --checksums release/SHA256SUMS \
     --swap-circuit-metadata ... --private-egress-circuit-metadata ... \
     --output release/release-manifest.json
   ```

   For a release that activates at a height, add `--activation-height H
   --advance-warning-blocks N`.
4. Two signers sign, each on its own machine, each after checking the manifest
   against what it built or verified:

   ```sh
   postfiat-node release-manifest-sign --manifest-file release-manifest.json --key-file release-lead-1.key.json
   ```

5. Check the result, then publish the release directory under
   `<url-base>/<release-id>/` and the manifest also as
   `<url-base>/latest/release-manifest.json`:

   ```sh
   postfiat-node release-manifest-verify --manifest-file release-manifest.json --keyset-file release-keyset.json \
     --runtime-binary-file release/postfiat-node-linux-x86_64 --arch linux-x86_64
   ```

An operator then runs `postfiat-node upgrade --check` and `postfiat-node
upgrade`. A manifest with a lower `release_sequence` than the installed release
is refused.

**Replacing a key.** Two signers of the current keyset sign the new keyset
(`release-keyset-rotation-sign`), and operators apply it with `postfiat-node
upgrade --rotate-keyset`. A manifest signed under the new keyset is refused
until then.

`python/tests/test_release_flow.py` runs these commands end to end.

## Protected-branch requirements

The public `main` branch must reject force-pushes and deletion, require review
of CODEOWNERS paths, require all review conversations to be resolved, and apply
the same rules to administrators. At least one approving review is required;
changes to consensus, state transitions, cryptography/proofs, custody, bridge,
release or workflow code require two approvals including the owning security or
protocol reviewer.

The following pull-request jobs are required and may not be bypassed by a label
or a maintainer-authored merge:

- `rust-ci / check`
- `rust-ci / test`
- `docs-build / build`
- `product-security-ci / public-tree-hygiene`
- `product-security-ci / rust-supply-chain`
- `product-security-ci / python-sdk`
- `product-security-ci / wallet-and-proxy`
- `product-security-ci / evm-contracts`

The secret-backed `product-security-ci / official-mainnet-fork` job is required
on the exact post-merge release revision before tagging; it is deliberately not
treated as an offline PR success. Publication also requires the exact clean
candidate to pass `scripts/verify-publication-candidate` and
`scripts/test-productionization-closure-table --require-closed`. A skipped,
neutral, cancelled or stale check is not release evidence.
