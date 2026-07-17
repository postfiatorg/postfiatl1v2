import assert from 'node:assert/strict';
import test from 'node:test';

import {
  CHAIN_ID,
  GENESIS_HASH,
  LEGACY_CHAIN_IDS,
} from './utils.js';

test('public wallet defaults are bound to the live WAN devnet domain', () => {
  assert.equal(CHAIN_ID, 'postfiat-wan-devnet-2');
  assert.equal(
    GENESIS_HASH,
    '46da6c340d27d9140bd9d9a2fc0cb81064b0bfa662d5981d2e2b2de6960f06cd22ef4f790cb35f8d2e20f771f595ff10',
  );
  assert.deepEqual(LEGACY_CHAIN_IDS, ['postfiat-wan-devnet']);
});
