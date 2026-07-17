## Outcome

Describe the user/protocol outcome and why this change is needed.

## Safety impact

- [ ] Consensus, state root, serialization, cryptography, custody, bridge, RPC, or storage behavior is unchanged.
- [ ] If changed, the affected invariant, migration/rollback behavior, and adversarial tests are described below.

## Verification

List exact commands and results. The normal release guards are:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
scripts/public-secret-scan
scripts/docs-site-build
```

## Release and rollback

State compatibility, rollout ordering, canary signal, and rollback threshold:
