'use strict';

// The fleet majority for owned_objects groups validators by the object view
// and the returned page: equal answers at different heights agree; the same
// page from two different views (view_digest) or two different object sets
// never agree. The answering node's id and its inventory revision are left
// out of the grouping key and returned beside the answer, one entry per
// agreeing validator that sent a well-formed pair, with the configured
// endpoint's validator id beside the node's own (unauthenticated) id.
const assert = require('assert');
const net = require('net');
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
const revised = (node, sequence, overrides = {}) => answer({
  node_id: node,
  inventory_revision: { finalized_height: 40, fastpay_sequence: sequence },
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
assert.strictEqual(key(revised('validator-0', 7)), key(revised('validator-1', 9)));
assert.strictEqual(key(revised('validator-0', 7)), key(answer()), 'older nodes group with newer ones');
assert.notStrictEqual(
  key(revised('validator-0', 7)),
  key(revised('validator-1', 7, { view_digest: 'ee'.repeat(32) })),
);
// The incarnation and the unavailable reason never split a group.
const INCARNATION = '0f'.repeat(16);
assert.strictEqual(
  key(revised('validator-0', 7, {
    inventory_revision: { finalized_height: 40, fastpay_sequence: 7, incarnation: INCARNATION },
  })),
  key(answer({ node_id: 'validator-1', inventory_revision_unavailable: 'legacy_owned_applies_enabled' })),
);
assert.strictEqual(
  key(answer({ inventory_revision_unavailable: 'legacy_owned_applies_enabled' })),
  key(answer()),
);
console.log('PASS owned_objects fleet agreement compares the view digest and the objects');

function validator(result) {
  const server = net.createServer((socket) => {
    socket.on('data', (chunk) => {
      const request = JSON.parse(chunk.toString('utf8').split('\n')[0]);
      socket.write(`${JSON.stringify({ id: request.id, ok: true, result })}\n`);
    });
  });
  return new Promise((resolve) => server.listen(0, '127.0.0.1', () => resolve(server)));
}

async function fleetRead(results) {
  const servers = await Promise.all(results.map(validator));
  const routing = require('./rpc-routing').create({
    net,
    RPC_FLEET: servers.map((server, index) => ({
      validatorId: `v${index}`,
      host: '127.0.0.1',
      port: server.address().port,
    })),
    WALLET_SUBSCRIPTION_READ_TIMEOUT_MS: 2000,
    TCP_TIMEOUT_MS: 2000,
    ENABLE_UPSTREAM_KEEPALIVE: false,
  });
  try {
    return await routing.readFleetRpcMajority('owned_objects', { owner_public_key_hex: 'ab'.repeat(32) });
  } finally {
    servers.forEach((server) => server.close());
  }
}

const expectedAnswer = (() => {
  const { height: _height, state_root: _stateRoot, ...rest } = answer();
  return rest;
})();

(async () => {
  // Six nodes, six ids, two fastpay sequences, one view: one majority.
  const six = await fleetRead([0, 1, 2, 3, 4, 5].map((index) => revised(`node-${index}`, index < 3 ? 7 : 8)));
  assert.strictEqual(six.ok, true);
  assert.strictEqual(six.source.consensus_count, 6);
  assert.deepStrictEqual(six.result, expectedAnswer, 'the agreed answer keeps its fields');
  assert.deepStrictEqual(six.source.fleet_revisions, [0, 1, 2, 3, 4, 5].map((index) => ({
    endpoint_validator_id: `v${index}`,
    node_id: `node-${index}`,
    inventory_revision: { finalized_height: 40, fastpay_sequence: index < 3 ? 7 : 8 },
  })));

  // A different view splits; the list carries only the agreeing validators.
  const split = await fleetRead([
    revised('node-0', 7),
    revised('node-1', 7, { view_digest: 'ee'.repeat(32) }),
    revised('node-2', 8),
    answer(),
  ]);
  assert.strictEqual(split.source.consensus_count, 3);
  assert.deepStrictEqual(split.source.validators, ['v0', 'v2', 'v3']);
  assert.deepStrictEqual(split.source.fleet_revisions, [
    { endpoint_validator_id: 'v0', node_id: 'node-0', inventory_revision: { finalized_height: 40, fastpay_sequence: 7 } },
    { endpoint_validator_id: 'v2', node_id: 'node-2', inventory_revision: { finalized_height: 40, fastpay_sequence: 8 } },
  ]);

  // Older nodes without the fields group as before and have no entries.
  const older = await fleetRead([answer(), answer({ height: 41 }), answer()]);
  assert.strictEqual(older.source.consensus_count, 3);
  assert.deepStrictEqual(older.result, expectedAnswer);
  assert.deepStrictEqual(older.source.fleet_revisions, []);

  // Malformed pairs are omitted, never repaired; the page still agrees.
  const malformed = [
    { node_id: '' },
    { node_id: ' node-x' },
    { node_id: 'node-x\n' },
    { node_id: ['node-x'] },
    { node_id: {} },
    { inventory_revision: [] },
    { inventory_revision: [40, 7] },
    { inventory_revision: {} },
    { inventory_revision: null },
    { inventory_revision: { finalized_height: 40 } },
    { inventory_revision: { finalized_height: 40, fastpay_sequence: -1 } },
    { inventory_revision: { finalized_height: 40, fastpay_sequence: 1.5 } },
    { inventory_revision: { finalized_height: 40, fastpay_sequence: '7' } },
    { inventory_revision: { finalized_height: 40, fastpay_sequence: 2 ** 64 } },
    // Above 2^53 - 1 the parsed JSON number may be rounded: omit, not pass on.
    { inventory_revision: { finalized_height: 40, fastpay_sequence: 2 ** 53 } },
    { inventory_revision: { finalized_height: 39, fastpay_sequence: 7 } },
    { height: 41 },
    { height: undefined },
  ];
  const bad = await fleetRead([revised('node-ok', 7), ...malformed.map((fields) => revised('node-x', 7, fields))]);
  assert.strictEqual(bad.source.consensus_count, malformed.length + 1);
  assert.deepStrictEqual(bad.source.fleet_revisions, [
    { endpoint_validator_id: 'v0', node_id: 'node-ok', inventory_revision: { finalized_height: 40, fastpay_sequence: 7 } },
  ]);
  // Only the two revision fields pass on.
  const extra = await fleetRead([revised('node-0', 7, {
    inventory_revision: { finalized_height: 40, fastpay_sequence: 7, note: 'x' },
  })]);
  assert.deepStrictEqual(extra.source.fleet_revisions, [
    { endpoint_validator_id: 'v0', node_id: 'node-0', inventory_revision: { finalized_height: 40, fastpay_sequence: 7 } },
  ]);
  // The incarnation passes on when well formed; a malformed one drops the
  // entry; a node that says why it has no revision has no entry and still
  // agrees.
  const incarnated = await fleetRead([
    revised('node-0', 7, { inventory_revision: { finalized_height: 40, fastpay_sequence: 7, incarnation: INCARNATION } }),
    revised('node-1', 7, { inventory_revision: { finalized_height: 40, fastpay_sequence: 7, incarnation: '0F'.repeat(16) } }),
    revised('node-2', 7, { inventory_revision: { finalized_height: 40, fastpay_sequence: 7, incarnation: 7 } }),
    answer({ node_id: 'node-3', inventory_revision_unavailable: 'legacy_owned_applies_enabled' }),
  ]);
  assert.strictEqual(incarnated.source.consensus_count, 4);
  assert.deepStrictEqual(incarnated.result, expectedAnswer);
  assert.deepStrictEqual(incarnated.source.fleet_revisions, [
    {
      endpoint_validator_id: 'v0',
      node_id: 'node-0',
      inventory_revision: { finalized_height: 40, fastpay_sequence: 7, incarnation: INCARNATION },
    },
  ]);
  console.log('PASS the fleet feed groups owned_objects pages without node ids and returns their revisions');
})().catch((error) => {
  console.error(error);
  process.exit(1);
});
