#!/usr/bin/env bash
set -euo pipefail

BUNDLE_DIR='docs/evidence/pfusdc-tier4-v3-epoch3-nav-profile-20260719/bundle'
PFTL_DATA_DIR=${PFTL_DATA_DIR:-postfiat_data}
: "${ISSUER_KEY_FILE:?set ISSUER_KEY_FILE}"

postfiat-node asset-fee-quote --data-dir "$PFTL_DATA_DIR" --source "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8" --operation-json "$(cat "$BUNDLE_DIR/profile-register.operation.json")" > "$BUNDLE_DIR/profile-register.quote.json"
postfiat-node wallet-sign-asset-transaction --key-file "$ISSUER_KEY_FILE" --quote-file "$BUNDLE_DIR/profile-register.quote.json" > "$BUNDLE_DIR/profile-register.signed.json"
postfiat-node mempool-submit-signed-asset-transaction --data-dir "$PFTL_DATA_DIR" --signed-asset-transaction-json "$(cat "$BUNDLE_DIR/profile-register.signed.json")"
postfiat-node asset-fee-quote --data-dir "$PFTL_DATA_DIR" --source "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8" --operation-json "$(cat "$BUNDLE_DIR/asset-create.operation.json")" > "$BUNDLE_DIR/asset-create.quote.json"
postfiat-node wallet-sign-asset-transaction --key-file "$ISSUER_KEY_FILE" --quote-file "$BUNDLE_DIR/asset-create.quote.json" > "$BUNDLE_DIR/asset-create.signed.json"
postfiat-node mempool-submit-signed-asset-transaction --data-dir "$PFTL_DATA_DIR" --signed-asset-transaction-json "$(cat "$BUNDLE_DIR/asset-create.signed.json")"
postfiat-node asset-fee-quote --data-dir "$PFTL_DATA_DIR" --source "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8" --operation-json "$(cat "$BUNDLE_DIR/nav-asset-register.operation.json")" > "$BUNDLE_DIR/nav-asset-register.quote.json"
postfiat-node wallet-sign-asset-transaction --key-file "$ISSUER_KEY_FILE" --quote-file "$BUNDLE_DIR/nav-asset-register.quote.json" > "$BUNDLE_DIR/nav-asset-register.signed.json"
postfiat-node mempool-submit-signed-asset-transaction --data-dir "$PFTL_DATA_DIR" --signed-asset-transaction-json "$(cat "$BUNDLE_DIR/nav-asset-register.signed.json")"
