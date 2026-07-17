import assert from 'node:assert/strict';
import test from 'node:test';

import {
  assertNoCustodyMaterial,
  clearCustodyMaterialRegistry,
  findCustodyMaterial,
  registerCustodyMaterial,
} from './custody-boundary.js';

test.afterEach(() => clearCustodyMaterialRegistry());

test('custody boundary rejects secret fields recursively and inside JSON strings', () => {
  const payload = {
    action: {
      operation_json: JSON.stringify({ nested: { master_seed_hex: '11'.repeat(32) } }),
    },
  };

  const hits = findCustodyMaterial(payload);
  assert.deepEqual(hits.map(hit => hit.path), [
    '$.action.operation_json<json>.nested.master_seed_hex',
  ]);
  assert.throws(
    () => assertNoCustodyMaterial(payload, 'wallet RPC'),
    /wallet RPC contains forbidden custody material/,
  );

  const oversizedSerializedPayload = JSON.stringify({
    padding: 'x'.repeat(1_048_576),
    nested: { private_key_hex: '22'.repeat(32) },
  });
  assert.throws(
    () => assertNoCustodyMaterial({ operation_json: oversizedSerializedPayload }, 'wallet RPC'),
    /serialized-json-inspection-limit-exceeded/,
    'oversized JSON-looking network fields must fail closed before transport',
  );
});

test('custody boundary rejects an active seed hidden under an innocuous key', () => {
  const seed = 'a5'.repeat(32);
  registerCustodyMaterial({ seed, backupJson: JSON.stringify({ master_seed_hex: seed }) });

  assert.throws(
    () => assertNoCustodyMaterial({ metadata: { opaque: `prefix:${seed}:suffix` } }, 'swap request'),
    /registered-secret-value/,
  );
});

test('custody boundary permits public keys, signatures, and signed envelopes', () => {
  const payload = {
    owner_pubkey_hex: 'ab'.repeat(1952),
    owner_signature_hex: 'cd'.repeat(3309),
    signed_transfer_json: JSON.stringify({
      public_key_hex: 'ef'.repeat(1952),
      signature_hex: '12'.repeat(3309),
      transaction: { from: 'pfsource', to: 'pftarget', amount: 1 },
    }),
  };

  assert.deepEqual(findCustodyMaterial(payload), []);
  assert.doesNotThrow(() => assertNoCustodyMaterial(payload, 'signed transaction'));
});

test('custody registry clears all active secret sentinels on wallet lock', () => {
  const seed = '7f'.repeat(32);
  registerCustodyMaterial({ seed });
  assert.throws(() => assertNoCustodyMaterial({ opaque: seed }), /registered-secret-value/);
  clearCustodyMaterialRegistry();
  assert.doesNotThrow(() => assertNoCustodyMaterial({ opaque: seed }));
});
