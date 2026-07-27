#!/usr/bin/env bash
set -euo pipefail

BUNDLE_DIR='docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/pftl-relay/relay-bundle'
PFTL_DATA_DIR=${PFTL_DATA_DIR:-postfiat_data}
: "${PROPOSER_KEY_FILE:?set PROPOSER_KEY_FILE}"
: "${FINALIZER_KEY_FILE:?set FINALIZER_KEY_FILE}"
: "${CLAIMER_KEY_FILE:?set CLAIMER_KEY_FILE}"

postfiat-node asset-fee-quote --data-dir "$PFTL_DATA_DIR" --source "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8" --operation-json "$(cat "$BUNDLE_DIR/propose.operation.json")" > "$BUNDLE_DIR/propose.quote.json"
postfiat-node wallet-sign-asset-transaction --key-file "$PROPOSER_KEY_FILE" --quote-file "$BUNDLE_DIR/propose.quote.json" > "$BUNDLE_DIR/propose.signed.json"
postfiat-node mempool-submit-signed-asset-transaction --data-dir "$PFTL_DATA_DIR" --signed-asset-transaction-json "$(cat "$BUNDLE_DIR/propose.signed.json")"
postfiat-node asset-fee-quote --data-dir "$PFTL_DATA_DIR" --source "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8" --operation-json "$(cat "$BUNDLE_DIR/finalize.operation.json")" > "$BUNDLE_DIR/finalize.quote.json"
postfiat-node wallet-sign-asset-transaction --key-file "$FINALIZER_KEY_FILE" --quote-file "$BUNDLE_DIR/finalize.quote.json" > "$BUNDLE_DIR/finalize.signed.json"
postfiat-node mempool-submit-signed-asset-transaction --data-dir "$PFTL_DATA_DIR" --signed-asset-transaction-json "$(cat "$BUNDLE_DIR/finalize.signed.json")"
postfiat-node asset-fee-quote --data-dir "$PFTL_DATA_DIR" --source "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8" --operation-json "$(cat "$BUNDLE_DIR/claim.operation.json")" > "$BUNDLE_DIR/claim.quote.json"
postfiat-node wallet-sign-asset-transaction --key-file "$CLAIMER_KEY_FILE" --quote-file "$BUNDLE_DIR/claim.quote.json" > "$BUNDLE_DIR/claim.signed.json"
postfiat-node mempool-submit-signed-asset-transaction --data-dir "$PFTL_DATA_DIR" --signed-asset-transaction-json "$(cat "$BUNDLE_DIR/claim.signed.json")"
