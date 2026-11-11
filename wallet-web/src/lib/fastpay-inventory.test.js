import test from 'node:test';
import assert from 'node:assert/strict';
import { createFastpayInventory, fastpayInventoryErrorMessage } from './fastpay-inventory.js';
import { fetchOwnedObjectsSnapshot } from './rpc-client.js';

const small = { id: 'small', version: 1, value: 1, asset: 'PFT' };
const big = { id: 'big', version: 1, value: 100, asset: 'PFT' };
const result = (view, objects, nextCursor) => ({
  schema: 'postfiat-owned-objects-v1',
  objects,
  object_count: objects.length,
  total_value: 101,
  truncated: nextCursor !== null,
  cursor: null,
  next_cursor: nextCursor,
  view_digest: view,
});
const page = (view, objects, nextCursor) => ({ ok: true, result: result(view, objects, nextCursor) });

function scripted(answers) {
  const calls = [];
  return { calls, async ownedObjects(_owner, opts) { calls.push(opts); return answers.shift(); } };
}

function inventoryOver(rpc) {
  const published = [];
  const inventory = createFastpayInventory({
    read: () => fetchOwnedObjectsSnapshot(rpc, 'owner-pk', { asset: 'PFT' }),
    publish: (state) => published.push(state),
  });
  return { inventory, published };
}

test('a partial feed page after a complete read keeps the page-2 object selectable', async () => {
  const rpc = scripted([page('v1', [small], 'c1'), page('v1', [big], null)]);
  const { inventory, published } = inventoryOver(rpc);
  await inventory.refresh();
  assert.deepEqual(published.at(-1).snapshot.objects.map(o => o.id), ['small', 'big']);

  // The feed's one truncated page of the same view: object worth 1, total 101.
  await inventory.applyFeed(result('v1', [small], 'c1'), false);
  assert.equal(published.length, 1, 'a partial page is never published');
  assert.equal(rpc.calls.length, 2, 'the same view needs no reread');
  assert.ok(published.at(-1).snapshot.objects.find(o => o.value >= 50), 'an unwrap of 50 finds a covering object');
});

test('a partial feed page of a new view rereads every page and publishes once', async () => {
  const rpc = scripted([
    page('v1', [small], null),
    page('v2', [small], 'c1'),
    page('v2', [big], null),
  ]);
  const { inventory, published } = inventoryOver(rpc);
  await inventory.refresh();
  await inventory.applyFeed(result('v2', [small], 'c1'), false);
  assert.equal(published.length, 2);
  assert.equal(published[1].status, 'ok');
  assert.deepEqual(published[1].snapshot.objects.map(o => o.id), ['small', 'big']);
});

test('a complete feed answer replaces the inventory', async () => {
  const { inventory, published } = inventoryOver(scripted([]));
  await inventory.applyFeed(result('v3', [big], null), true);
  assert.deepEqual(published, [{ status: 'ok', snapshot: result('v3', [big], null), error: '' }]);
});

test('a failed reread is shown and a partial feed page does not clear it', async () => {
  const changed = { ok: false, error: { code: 'owned_objects_view_changed', message: 'moved' } };
  const answers = [];
  for (let i = 0; i < 4; i += 1) answers.push(page(`v${i}`, [small], 'c'), changed);
  const rpc = scripted(answers);
  const { inventory, published } = inventoryOver(rpc);
  await assert.rejects(inventory.refresh());
  assert.equal(published.at(-1).status, 'error');
  assert.equal(published.at(-1).reason, 'view_unstable');
  assert.match(published.at(-1).error, /kept changing/);

  await inventory.applyFeed(result('v9', [small], 'c'), false);
  assert.equal(published.length, 1, 'the partial page neither clears the error nor publishes');
});

test('the failure reasons distinguish an old node from a changing inventory', async () => {
  const old = scripted([{ ok: true, result: { ...result('v1', [small], null), truncated: true, view_digest: undefined } }]);
  const error = await fetchOwnedObjectsSnapshot(old, 'owner-pk').catch(e => e);
  assert.equal(error.code, 'owned_objects_paging_unsupported');
  assert.match(fastpayInventoryErrorMessage(error), /too old/);
  assert.match(fastpayInventoryErrorMessage({ code: 'owned_objects_view_unstable' }), /kept changing/);
  assert.doesNotMatch(fastpayInventoryErrorMessage({ code: 'owned_objects_view_unstable' }), /too old/);
});
