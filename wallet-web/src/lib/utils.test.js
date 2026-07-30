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
    'ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9',
  );
  assert.deepEqual(LEGACY_CHAIN_IDS, ['postfiat-wan-devnet']);
});
