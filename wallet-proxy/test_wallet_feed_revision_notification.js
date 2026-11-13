'use strict';

// The wallet feed sends an update when the agreed snapshot changes. A newer
// inventory revision with identical objects is a change: a wallet must be
// able to learn it without waiting for an object to move.
const assert = require('assert');
const crypto = require('crypto');
const { walletSnapshotDigest } = require('./rpc-routing').create({ crypto });

const owned = { objects: [{ id: 'aa'.repeat(32), version: 1, value: 1, asset: 'PFT' }], view_digest: 'cc'.repeat(32) };
const snapshot = (sequence) => ({
  account: { balance: 1 },
  account_error: null,
  owned,
  owned_error: null,
  assets: null,
  assets_error: null,
  observed_at_ms: sequence,
  sources: {
    account: { validators: ['v0'] },
    owned: {
      validators: ['v0'],
      fleet_revisions: [{
        endpoint_validator_id: 'v0',
        node_id: 'node-0',
        inventory_revision: { finalized_height: 40, fastpay_sequence: sequence },
      }],
    },
    assets: null,
  },
});

assert.strictEqual(walletSnapshotDigest(snapshot(7)), walletSnapshotDigest(snapshot(7)), 'same revision, no update');
assert.notStrictEqual(walletSnapshotDigest(snapshot(7)), walletSnapshotDigest(snapshot(8)), 'a newer revision is an update');
const older = { ...snapshot(7), sources: { account: null, owned: { validators: ['v0'] }, assets: null } };
assert.strictEqual(walletSnapshotDigest(older), walletSnapshotDigest({ ...older, sources: { owned: null } }),
  'feeds without revisions digest as before');
console.log('PASS the wallet feed notifies a revision-only change');
