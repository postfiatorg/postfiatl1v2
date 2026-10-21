#!/usr/bin/env bash
# Focused suites of this release's changes, main checkout at the cut commit, default test profile.
set -u
R=$HOME/.cache/release-store-handle-20261021; L=$R/logs
cd $HOME/repos/postfiatl1v2
test "$(git rev-parse HEAD)" = 86706699f646405743385beabf609c6771a518a3 && test -z "$(git status --porcelain -- crates Cargo.lock)"
run() { name=$1; shift; echo "$(date -u +%FT%TZ) START $name: $*"; "$@" > $L/focused-$name.log 2>&1; echo "$(date -u +%FT%TZ) EXIT $? $name"; }
run storage-fastswap-store cargo test -p postfiat-storage --lib fastswap_store
run storage-transactional cargo test -p postfiat-storage --lib transactional
run node-lib-committee-migration cargo test -p postfiat-node --lib fastswap_committee_migration
run node-lib-fastswap-service cargo test -p postfiat-node --lib fastswap_service
run node-bin-store-handle-round cargo test -p postfiat-node --bin postfiat-node transactional_six_validator_round_opens_each_database_a_bounded_number_of_times
run node-bin-operator-attestation cargo test -p postfiat-node --bin postfiat-node operator_attestation_commands_reach_their_cli_handler
run node-bin-pftl-swapctl cargo test -p postfiat-node --bin pftl_swapctl
echo "$(date -u +%FT%TZ) DONE"
