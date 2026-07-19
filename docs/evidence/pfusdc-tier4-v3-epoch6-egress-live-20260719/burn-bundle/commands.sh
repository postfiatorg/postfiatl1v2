#!/usr/bin/env bash
set -euo pipefail

BUNDLE_DIR='/home/postfiat/repos/postfiatl1v2-public-main-verification-20260717/docs/evidence/pfusdc-tier4-v3-epoch6-egress-live-20260719/burn-bundle'
PFTL_DATA_DIR=${PFTL_DATA_DIR:-postfiat_data}
: "${OWNER_KEY_FILE:?set OWNER_KEY_FILE}"

postfiat-node asset-fee-quote --data-dir "$PFTL_DATA_DIR" --source "pfab9b9228942e5c529633a13aa271d5297bec6353" --operation-json "$(cat "$BUNDLE_DIR/burn-to-redeem.operation.json")" > "$BUNDLE_DIR/burn-to-redeem.quote.json"
postfiat-node wallet-sign-asset-transaction --key-file "$OWNER_KEY_FILE" --quote-file "$BUNDLE_DIR/burn-to-redeem.quote.json" > "$BUNDLE_DIR/burn-to-redeem.signed.json"
postfiat-node mempool-submit-signed-asset-transaction --data-dir "$PFTL_DATA_DIR" --signed-asset-transaction-json "$(cat "$BUNDLE_DIR/burn-to-redeem.signed.json")"
