# Cobalt activate-or-retire corpus

This directory freezes the decision contract for the terminal Cobalt evaluation. It does not contain an activation recommendation and it does not authorize the decisive run. Execution is governed by Section 2 of `docs/plans/active/cobalt-activate-or-retire-milestone.md`.

## What is frozen

- `oracle-contract.md`: the independent compatible/incompatible rules.
- `generate_inputs.py`: 18 unscored scenario inputs.
- `scenario-manifest.json`: oracle-scored per-validator outcomes and source hashes.
- `diagnosis.md`: the code-level explanation of the obsolete 90%-overlap false halt.
- `rippled/DecisiveGovernanceBenchmark_test.cpp`: the pinned RippleD 3.1.3 adapter.
- `crates/cobalt_decision_oracle`: the standalone Rust oracle.
- `crates/node/src/bin/postfiat_cobalt_decisive_benchmark.rs`: the production Cobalt adapter.

The manifest contains 13 compatible and 5 incompatible cases. Every correct validator has an explicit Cobalt and RippleD outcome. It contains no `characterize` mode. RippleD validator-governance admission and native RippleD ledger consensus are separate report fields.

Canonical manifest ID: `78fc3f92d460f45a4941d40ef705af6c761e3782155a5b599dbd78c90396bde3`.

Raw `scenario-manifest.json` SHA-256: `3df59da71f0f52553bfa1d4919a50a180a4ec2aaf88a250bfb320c438932a14d`.

## Rebuild the frozen manifest

```bash
python3 benchmarks/cobalt-activate-or-retire/generate_inputs.py \
  --output .tih/cobalt-activate-or-retire-input.json

cargo run -p postfiat-cobalt-decision-oracle --locked -- \
  --input .tih/cobalt-activate-or-retire-input.json \
  --output benchmarks/cobalt-activate-or-retire/scenario-manifest.json \
  --oracle-source crates/cobalt_decision_oracle/src/lib.rs \
  --contract benchmarks/cobalt-activate-or-retire/oracle-contract.md \
  --adapter cobalt=crates/node/src/bin/postfiat_cobalt_decisive_benchmark.rs \
  --adapter rippled=benchmarks/cobalt-activate-or-retire/rippled/DecisiveGovernanceBenchmark_test.cpp
```

The repository's pinned Zig compiler environment is required when Cargo links executables on this host.

## Verification completed before freeze

```bash
python3 benchmarks/cobalt-activate-or-retire/verify_manifest.py
cargo test -p postfiat-cobalt-decision-oracle --locked
cargo check -p postfiat-node --bin postfiat-cobalt-decisive-benchmark --locked
```

The RippleD adapter was syntax-compiled using the exact compile flags from the pinned `3.1.3` build at commit `46b241ace8b30d9c9775d60ffba7d24b21903896`.

The decisive adapters must not be run and then used to edit expected outcomes. If the frozen oracle is wrong, the campaign is invalid. If an adapter differs from the oracle, the bounded remediation rules in the milestone apply.
