#!/usr/bin/env bash
# Focused suites of this release's changes, on the release tip (main checkout at d728f895). Local only.
set -uo pipefail
cd /home/postfiatchad/repos/postfiatl1v2
test "$(git rev-parse HEAD)" = d728f895ee6c6d9910c6e0c0f3b9d92f30e0f3f2 || { echo "checkout is not the tip"; exit 1; }
for spec in "--lib account_tx" "--lib transport_protocol" "--bin postfiat-node transport_protocol" "--bin postfiat-node transport_batch_payload_tests" "--lib block_vote_records" "--bin postfiat-node block_vote_records" "--test fastpay_default_rpc"; do
  echo "=== cargo test -p postfiat-node $spec"
  cargo test -q -p postfiat-node $spec 2>&1 | grep -E "^test result|panicked|^error|FAILED" 
  echo "exit ${PIPESTATUS[0]}"
done
echo "=== DONE"
