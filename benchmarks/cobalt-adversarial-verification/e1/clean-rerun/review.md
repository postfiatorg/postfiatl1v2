# Clean-state E1 rerun

The worktree was clean at `b9da3405`. Cargo artifacts for only `postfiat-cobalt-adversarial-oracle` and `postfiat-cobalt-e1-harness` were removed, and the unchanged corpus was rebuilt and rerun in optimized mode.

- Corpus SHA-256: `42ed266ba207136eec560f8be14c904c2e63ffe305e188860e4ff04731cd5fd2`
- Classification SHA-256: `66ed6e8b2f7fd33927448b5b2e866ae4275263840128cbf1842d7460f3ca19cd`
- Cases: 10,240
- Disagreements: 0
- Hash comparison: corpus and classification hashes are byte-identical to the reconciled full pass.
- Result: PASS

Focused tests passed after the rerun:

- `cargo test -p postfiat-cobalt-adversarial-oracle -p postfiat-cobalt-e1-harness`
- `cargo test -p postfiat-consensus-cobalt strong_support`
- `cargo test -p postfiat-consensus-cobalt verifies_nonuniform_governance_certificate_against_local_views`
- `cargo test -p postfiat-consensus-cobalt reports_unsafe_nonuniform_graph_with_counterexample_pair`

No Orchard, Halo2, node, or full-workspace suite was run.
