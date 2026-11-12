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
  assert.deepEqual(published.map(p => p.status), ['ok', 'loading', 'ok'], 'selection is unavailable until the reread');
  assert.deepEqual(published[2].snapshot.objects.map(o => o.id), ['small', 'big']);
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

// Races between the live feed and the all-pages read (second reading, B1).
function deferred() {
  let resolve; let reject;
  const promise = new Promise((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function racing() {
  const reads = [];
  const published = [];
  const inventory = createFastpayInventory({
    read: () => { const d = deferred(); reads.push(d); return d.promise; },
    publish: (state) => published.push(state),
  });
  const views = () => published.map(p => (p.status === 'ok' ? p.snapshot.view_digest : p.status));
  return { inventory, reads, published, views };
}

const flush = () => new Promise(resolve => setImmediate(resolve));

test('a read that completes after a newer complete feed answer is discarded', async () => {
  const { inventory, reads, views } = racing();
  const refreshed = inventory.refresh();
  await inventory.applyFeed(result('new', [big], null), true, 2);
  reads[0].resolve(result('old', [small], null));
  assert.equal((await refreshed).view_digest, 'new');
  assert.deepEqual(views(), ['new']);
});

test('a new partial view during a read makes selection unavailable and is read again', async () => {
  const { inventory, reads, views } = racing();
  const refreshed = inventory.refresh();
  await inventory.applyFeed(result('v2', [small], 'c1'), false, 1);
  assert.deepEqual(views(), ['loading'], 'selection is unavailable until a current view is published');
  reads[0].resolve(result('v1', [small], null));
  await flush();
  assert.equal(reads.length, 2, 'the invalidation recorded during the read causes one more read');
  reads[1].resolve(result('v2', [small, big], null));
  assert.equal((await refreshed).view_digest, 'v2');
  assert.deepEqual(views(), ['loading', 'v2']);
});

test('a read that already returns the invalidating view is published without another read', async () => {
  const { inventory, reads, views } = racing();
  const refreshed = inventory.refresh();
  await inventory.applyFeed(result('v2', [small], 'c1'), false, 1);
  reads[0].resolve(result('v2', [small, big], null));
  await refreshed;
  assert.equal(reads.length, 1);
  assert.deepEqual(views(), ['loading', 'v2']);
});

test('an older complete feed answer does not replace a newer one', async () => {
  const { inventory, views } = racing();
  await inventory.applyFeed(result('new', [big], null), true, 5);
  await inventory.applyFeed(result('old', [small], null), true, 4);
  assert.deepEqual(views(), ['new']);
});

test('an older complete feed answer does not clear a read error; a newer one does', async () => {
  const { inventory, reads, views } = racing();
  await inventory.applyFeed(result('v1', [big], null), true, 5);
  const refreshed = inventory.refresh();
  reads[0].reject({ code: 'owned_objects_view_unstable' });
  await assert.rejects(refreshed);
  await inventory.applyFeed(result('v0', [small], null), true, 4);
  assert.deepEqual(views(), ['v1', 'error'], 'the stale answer neither clears the warning nor publishes');
  await inventory.applyFeed(result('v2', [small, big], null), true, 6);
  assert.deepEqual(views(), ['v1', 'error', 'v2'], 'a current complete answer may clear it');
});

test('a replaced coordinator publishes nothing from its outstanding read', async () => {
  const { inventory, reads, views } = racing();
  const refreshed = inventory.refresh();
  inventory.dispose();
  reads[0].resolve(result('v1', [small], null));
  await refreshed;
  await inventory.applyFeed(result('v2', [big], null), true, 1);
  assert.deepEqual(views(), []);
  inventory.attach();
  await inventory.applyFeed(result('v3', [big], null), true, 2);
  assert.deepEqual(views(), ['v3'], 'a re-attached (StrictMode) coordinator publishes again');
});
