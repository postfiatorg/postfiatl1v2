import assert from 'node:assert/strict';
import test from 'node:test';

import {
  loadFastPayRecoveries,
  removeFastPayRecovery,
  saveFastPayRecovery,
} from './fastpay-recovery-store.js';

function memoryStorage() {
  const values = new Map();
  return {
    getItem: key => values.get(key) || null,
    setItem: (key, value) => values.set(key, value),
  };
}

function pending(owner = 'owner-pk', lockId = '1'.repeat(96)) {
  return {
    signed_order: {
      operation: 'transfer',
      signed_order: {
        order: { recovery: { lock_id: lockId } },
        owner_pubkey_hex: owner,
        owner_signature_hex: 'public-order-signature',
      },
    },
  };
}

test('FastPay recovery store is wallet-scoped, durable, and removable', () => {
  const storage = memoryStorage();
  const lockId = saveFastPayRecovery(storage, 'owner-pk', pending());
  assert.equal(loadFastPayRecoveries(storage, 'owner-pk').length, 1);
  assert.equal(loadFastPayRecoveries(storage, 'other-owner').length, 0);
  removeFastPayRecovery(storage, lockId);
  assert.deepEqual(loadFastPayRecoveries(storage, 'owner-pk'), []);
});

test('FastPay recovery store refuses a record owned by another wallet', () => {
  const storage = memoryStorage();
  assert.throws(
    () => saveFastPayRecovery(storage, 'owner-pk', pending('attacker-pk')),
    /does not match this wallet/,
  );
});
