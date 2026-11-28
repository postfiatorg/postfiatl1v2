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

**Who holds the keys today.** The format is two of three signers: release lead,
independent builder, security custodian. Post Fiat is one developer today, and
all three keys are held by that one person. The two signatures on a devnet
release therefore show that the release came from the key holder and was not
altered; they do not show that two people checked it. Release notes say so.
When there are separate people, each makes a key on a machine of its own, and
the keyset is replaced by a rotation (below).

**Once: the keys.** A signer makes a key and hands over only the public entry
the command prints:

```sh
postfiat-node release-key-create --signer-id release-lead-1 --role release_lead --out-file release-lead-1.key.json
```

The three entries, with `threshold: 2`, are the keyset file
(`postfiat.release_keyset.v1`) that operators install at
`/etc/postfiat/release-keyset.json`. The private halves never leave the
key holder's machine and are never in this repository.

**Every release:**

1. Tag the commit on `main`. The `release-build` workflow builds
   `postfiat-node` for `linux-x86_64` and `linux-aarch64` and assembles the
   release directory with `scripts/release-assemble`: the unit bundle, the
   SBOM and `SHA256SUMS`. Each binary is built on a runner of its own
   architecture and run there once (`release-build-info`); the assembly
   refuses a binary whose ELF machine is not the one its name says. It signs
   nothing and publishes nothing.
2. Build the same tag a second time on a clean machine and compare
   `postfiat-node release-build-info` with `SHA256SUMS`. A different hash stops
   the release. (With one developer this is a second clean build, not a second
   person.)
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
4. Sign with two of the three keys, after checking the manifest against what
   was built and verified:

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

**A first install** has no trusted `postfiat-node` yet, and a binary must not be
the one to say it is genuine. The operator checks the download with
`scripts/verify-release` (Python and `dilithium-py`; it runs nothing from the
release): the two signatures against the keyset, and the binary against the
manifest. The keyset comes from somewhere other than the download, and its
SHA-256 is published with the release.

After that, an operator runs `postfiat-node upgrade --check` and `postfiat-node
upgrade`: the installed binary checks the next one. A manifest with a lower `release_sequence` than the installed release
is refused.

**Replacing a key.** Two signers of the current keyset sign the new keyset
(`release-keyset-rotation-sign`), and operators apply it with `postfiat-node
upgrade --rotate-keyset`. A manifest signed under the new keyset is refused
until then.

`python/tests/test_release_flow.py` runs these commands end to end.

## Branch rules as they are

This section describes what is enforced today, not a target.

- `postfiatorg/postfiatl1v2` `main` has no branch protection. A pull request
  can be merged by its author without review. With one developer there is no
  second reviewer; the checks below are the gate, and a written review (an
  agent or an outside reader) is requested for changes to consensus, state
  transitions, cryptography, custody, bridge, release or upgrade code. **Every
  finding of such a review is fixed, or named as open in the pull request,
  before the merge.**
- Pull-request jobs that must be green before a merge: `rust-ci / check`,
  `rust-ci / test`, `docs-build / build`, and the `product-security-ci` jobs
  (`public-tree-hygiene`, `rust-supply-chain`, `python-sdk`,
  `wallet-and-proxy`, `evm-contracts`). GitHub does not enforce this on `main`;
  the person merging does.
- `build-lane-export-identity` fails on `main`: the committed SP1 program and
  verification key do not match a rebuild. It is a known open item.

The secret-backed `product-security-ci / official-mainnet-fork` job is required
on the exact post-merge release revision before tagging; it is deliberately not
treated as an offline PR success. Publication also requires the exact clean
candidate to pass `scripts/verify-publication-candidate` and
`scripts/test-productionization-closure-table --require-closed`. A skipped,
neutral, cancelled or stale check is not release evidence.
