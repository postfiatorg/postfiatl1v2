#!/usr/bin/env bash
# Focused suites of this release's changes, on the release tip (main checkout at ae70d66d). Local only.
set -uo pipefail
cd /home/postfiatchad/repos/postfiatl1v2-dravlic
test "$(git rev-parse HEAD)" = ae70d66d8a5083cc1a33aaf4a3e17f6901f7bc34 || { echo "checkout is not the tip"; exit 1; }
export CARGO_BUILD_JOBS=6 CARGO_TERM_COLOR=never
for spec in "--lib block_vote_records" "--bin postfiat-node block_vote_records" "--bin postfiat-node transport_batch_payload_tests" \
  "--lib private_action_admission" "--bin postfiat-node rpc_serve_exposes_private_actions_only_with_the_switch" "--bin postfiat-node private_action_methods_exist_only_behind_the_admission_switch" \
  "--lib asset_orchard_pricing_enforcement" "--lib asset_orchard_egress_policy" \
  "--lib private_pool_snapshot_points" "--lib private_pool_tokens_outlive_new_blocks" "--lib private_pool_receipts_serve_a_retained_older_token" \
  "--lib account_tx"; do
  echo "=== $(date -u +%T) cargo test -p postfiat-node $spec"
  cargo test -q -p postfiat-node $spec 2>&1 | grep -E "^test result|panicked|^error|FAILED|^test .* \.\.\. "
  echo "exit ${PIPESTATUS[0]}"
done
echo "=== DONE $(date -u +%T)"
