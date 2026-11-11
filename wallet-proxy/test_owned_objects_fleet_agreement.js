'use strict';

// The fleet majority for owned_objects groups validators by the object view
// and the returned page: equal answers at different heights agree; the same
// page from two different views (view_digest) or two different object sets
// never agree.
const assert = require('assert');
const { readGroupKey } = require('./rpc-routing').create({});

const answer = (overrides = {}) => ({
  schema: 'postfiat-owned-objects-v1',
  objects: [{ id: 'aa'.repeat(32), version: 1, value: 10, asset: 'PFT' }],
  total_value: 30,
  truncated: true,
  cursor: null,
  next_cursor: `ooc2.tag.-.${'cc'.repeat(32)}.505654.${'aa'.repeat(32)}.1`,
  view_digest: 'cc'.repeat(32),
  height: 40,
  state_root: 'bb'.repeat(48),
  ...overrides,
});
const key = (result) => readGroupKey('owned_objects', result);

assert.strictEqual(key(answer()), key(answer({ height: 41, state_root: 'dd'.repeat(48) })));
assert.notStrictEqual(key(answer()), key(answer({ view_digest: 'ee'.repeat(32) })));
assert.notStrictEqual(
  key(answer()),
  key(answer({ objects: [{ id: 'ab'.repeat(32), version: 1, value: 10, asset: 'PFT' }] })),
);
assert.notStrictEqual(key(answer()), key(answer({ total_value: 40 })));
console.log('PASS owned_objects fleet agreement compares the view digest and the objects');
