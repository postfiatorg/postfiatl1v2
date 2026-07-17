# Release Process

PostFiat L1 is currently controlled pre-testnet software. A tagged release is
an immutable engineering artifact, not a mainnet-readiness claim.

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
