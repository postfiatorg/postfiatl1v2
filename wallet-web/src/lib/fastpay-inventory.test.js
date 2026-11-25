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
  const rpc = scripted([page('v1', [small], 'c1'), page('v1', [big], null), page('v1', [small], 'c1'), page('v1', [big], null)]);
  const { inventory, published } = inventoryOver(rpc);
  await inventory.refresh();
  assert.equal(rpc.calls.length, 4, 'a second all-pages read confirms the view');
  assert.deepEqual(published.at(-1).snapshot.objects.map(o => o.id), ['small', 'big']);

  // The feed's one truncated page of the same view: object worth 1, total 101.
  await inventory.applyFeed(result('v1', [small], 'c1'), false);
  assert.equal(published.length, 1, 'a partial page is never published');
  assert.equal(rpc.calls.length, 4, 'the same view needs no reread');
  assert.ok(published.at(-1).snapshot.objects.find(o => o.value >= 50), 'an unwrap of 50 finds a covering object');
});

test('a partial feed page of a new view rereads every page and publishes once', async () => {
  const rpc = scripted([
    page('v1', [small], null),
    page('v1', [small], null),
    page('v2', [small], 'c1'),
    page('v2', [big], null),
    page('v2', [small], 'c1'),
    page('v2', [big], null),
  ]);
  const { inventory, published } = inventoryOver(rpc);
  await inventory.refresh();
  await inventory.applyFeed(result('v2', [small], 'c1'), false);
  assert.deepEqual(published.map(p => p.status), ['ok', 'loading', 'ok'], 'selection is unavailable until the reread');
  assert.deepEqual(published[2].snapshot.objects.map(o => o.id), ['small', 'big']);
});

test('a complete feed answer is not published; the confirmed read of its view is', async () => {
  const rpc = scripted([page('v3', [big], null), page('v3', [big], null)]);
  const { inventory, published } = inventoryOver(rpc);
  await inventory.applyFeed(result('v3', [big], null), true);
  assert.deepEqual(published.map(p => p.status), ['loading', 'ok']);
  assert.equal(rpc.calls.length, 2);
  assert.deepEqual(published[1].snapshot.objects.map(o => o.id), ['big']);
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

test('reads that keep returning two different views end in view_unstable', async () => {
  const answers = [];
  for (let i = 0; i < 6; i += 1) answers.push(page(`v${i}`, [small], null));
  const rpc = scripted(answers);
  const { inventory, published } = inventoryOver(rpc);
  await assert.rejects(inventory.refresh(), { code: 'owned_objects_view_unstable' });
  assert.equal(rpc.calls.length, 6, 'three attempts of two reads');
  assert.deepEqual(published.map(p => p.status), ['error']);
  assert.equal(published[0].reason, 'view_unstable');
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

test('an in-flight read of an older view is not published after a newer feed view', async () => {
  const { inventory, reads, views } = racing();
  const refreshed = inventory.refresh();
  await inventory.applyFeed(result('new', [big], null), true, 2);
  reads[0].resolve(result('old', [small], null));
  await flush();
  reads[1].resolve(result('old', [small], null));
  await flush();
  assert.deepEqual(views(), ['loading'], 'the old view is read again, not published');
  reads[2].resolve(result('new', [big], null));
  await flush();
  reads[3].resolve(result('new', [big], null));
  assert.equal((await refreshed).view_digest, 'new');
  assert.deepEqual(views(), ['loading', 'new']);
});

test('a read that already returns the invalidating view needs only its confirmation', async () => {
  const { inventory, reads, views } = racing();
  const refreshed = inventory.refresh();
  await inventory.applyFeed(result('v2', [small], 'c1'), false, 1);
  reads[0].resolve(result('v2', [small, big], null));
  await flush();
  reads[1].resolve(result('v2', [small, big], null));
  await refreshed;
  assert.equal(reads.length, 2);
  assert.deepEqual(views(), ['loading', 'v2']);
});

test('no feed answer clears a read error; the user retry does', async () => {
  const { inventory, reads, views } = racing();
  const refreshed = inventory.refresh();
  reads[0].reject({ code: 'owned_objects_view_unstable' });
  await assert.rejects(refreshed);
  await inventory.applyFeed(result('v2', [small, big], null), true, 6);
  assert.deepEqual(views(), ['error'], 'a feed answer neither clears the warning nor publishes');
  assert.equal(reads.length, 1, 'nor reads on its own');
  const retried = inventory.refresh();
  reads[1].resolve(result('v2', [small, big], null));
  await flush();
  reads[2].resolve(result('v2', [small, big], null));
  await retried;
  assert.deepEqual(views(), ['error', 'v2'], 'the retry must reach the latest feed view');
});

test('a replaced coordinator publishes nothing from its outstanding read', async () => {
  const { inventory, reads, views } = racing();
  const refreshed = inventory.refresh();
  inventory.dispose();
  reads[0].resolve(result('v1', [small], null));
  await refreshed;
  await inventory.applyFeed(result('v2', [big], null), true, 1);
  assert.deepEqual(views(), []);
  assert.equal(reads.length, 1);
  inventory.attach();
  const fed = inventory.applyFeed(result('v3', [big], null), true, 2);
  reads[1].resolve(result('v3', [big], null));
  await flush();
  reads[2].resolve(result('v3', [big], null));
  await fed;
  assert.deepEqual(views(), ['loading', 'v3'], 'a re-attached (StrictMode) coordinator publishes again');
});

// Second reading R1-R5: feed answers cannot be ordered against reads (the
// node gives no comparable inventory revision), so no feed answer may
// republish an older inventory or re-enable one it invalidated.
// docs/wallets/fastpay-inventory-ordering.md.
const shared = { id: 'shared', version: 1, value: 5, asset: 'PFT' };
const spent = { id: 'spent', version: 1, value: 50, asset: 'PFT' };
const replacement = { id: 'spent', version: 2, value: 49, asset: 'PFT' };
const OLD = result('old', [shared, spent], null);
const NEW = result('new', [shared, replacement], null);
const NEW_PARTIAL = result('new', [shared], 'c-new');

// The node answers every read issued while `serve` runs with `snapshot`.
function served() {
  const harness = racing();
  harness.serve = async (snapshot) => {
    for (let round = 0; round < 50; round += 1) {
      await flush();
      const pending = harness.reads.filter(d => !d.settled);
      if (pending.length === 0) return;
      for (const d of pending) { d.settled = true; d.resolve(structuredClone(snapshot)); }
    }
  };
  return harness;
}

const selectable = (published) => {
  const last = published.at(-1);
  return last?.status === 'ok' ? last.snapshot.objects.map(o => `${o.id}@${o.version}`) : [];
};
const after = (views, view) => views.slice(views.lastIndexOf(view) + 1);

test('R1: a delayed complete feed answer does not republish the inventory a newer read replaced', async () => {
  const { inventory, published, views, serve } = served();
  const refreshed = inventory.refresh();
  await serve(NEW);
  await refreshed;
  assert.equal(views().at(-1), 'new');
  const fed = inventory.applyFeed(structuredClone(OLD), true, 100);
  await serve(NEW);
  await fed;
  assert.ok(!after(views(), 'new').includes('old'), `history ${views()}`);
  assert.ok(!selectable(published).includes('spent@1'), 'the spent coin is not selectable');
});

test('R2: an older complete feed answer stamped later than the feed watermark does not replace a newer read', async () => {
  const { inventory, published, views, serve } = served();
  const initial = result('initial', [shared], null);
  await Promise.all([inventory.applyFeed(initial, true, 50), serve(initial)]);
  // A plain refresh no longer moves past the latest feed view (RB1-N1); the
  // user's Retry reaches the newer read.
  const refreshed = inventory.retry().catch(() => {});
  await serve(NEW);
  await refreshed;
  assert.equal(views().at(-1), 'new');
  const fed = inventory.applyFeed(structuredClone(OLD), true, 100);
  await serve(NEW);
  await fed;
  assert.ok(!after(views(), 'new').includes('old'), `history ${views()}`);
  assert.ok(!selectable(published).includes('spent@1'));
});

test('R3: a reread that reaches a lagging node does not re-enable the invalidated inventory', async () => {
  const { inventory, published, views, serve } = served();
  const refreshed = inventory.refresh().catch(() => {});
  const fed = inventory.applyFeed(NEW_PARTIAL, false, 200);
  await serve(OLD); // every read, including the reread, reaches a lagging node
  await Promise.all([refreshed, fed]);
  assert.ok(!views().includes('old'), `history ${views()}`);
  assert.notEqual(published.at(-1).status, 'ok', 'selection stays unavailable');
  assert.ok(!selectable(published).includes('spent@1'));

  // Once reads reach the invalidating view, the user's retry publishes it.
  const retried = inventory.refresh().catch(() => {});
  await serve(NEW);
  await retried;
  assert.equal(views().at(-1), 'new');
});

test('R4: a newer partial view stamped earlier (reconnect, clock rollback) still invalidates and is read', async () => {
  const { inventory, reads, published, views, serve } = served();
  await Promise.all([inventory.applyFeed(structuredClone(OLD), true, 10000), serve(OLD)]);
  assert.equal(views().at(-1), 'old');
  const before = reads.length;
  const fed = inventory.applyFeed(NEW_PARTIAL, false, 9000);
  assert.equal(published.at(-1).status, 'loading', 'selection is unavailable at once');
  await serve(NEW);
  await fed;
  assert.ok(reads.length > before, 'the wallet reads again');
  assert.equal(views().at(-1), 'new');
  assert.ok(!selectable(published).includes('spent@1'));
});

test('R5: an older complete inventory from a faster gateway clock does not replace a newer one', async () => {
  const { inventory, published, views, serve } = served();
  await Promise.all([inventory.applyFeed(structuredClone(NEW), true, 10000), serve(NEW)]);
  assert.equal(views().at(-1), 'new');
  const fed = inventory.applyFeed(structuredClone(OLD), true, 11000);
  await serve(NEW);
  await fed;
  assert.ok(!after(views(), 'new').includes('old'), `history ${views()}`);
  assert.ok(!selectable(published).includes('spent@1'));
});

// Re-read RB1-N1/RB1-N2: the latest feed view is a standing constraint, a
// replaced view never returns, and only the user's Retry resynchronizes a
// stale feed constraint.
test('RB1-N1 E1: a later refresh that reaches a lagging node does not roll back a feed-confirmed view', async () => {
  const { inventory, published, views, serve } = served();
  const feed = structuredClone(NEW);
  await Promise.all([inventory.applyFeed(feed), serve(NEW)]);
  assert.equal(views().at(-1), 'new');
  const refreshed = inventory.refresh().catch(() => {});
  await serve(OLD); // the feed still names `new`
  await refreshed;
  await inventory.applyFeed(feed); // a re-render repeats the same feed answer
  assert.ok(!after(views(), 'new').includes('old'), `history ${views()}`);
  assert.notEqual(published.at(-1).status, 'ok');
  assert.ok(!selectable(published).includes('spent@1'));
});

test('RB1-N1 E2: an inventory replaced after its invalidation is never published again, even by Retry', async () => {
  const { inventory, published, views, serve } = served();
  await Promise.all([inventory.applyFeed(structuredClone(OLD)), serve(OLD)]);
  await Promise.all([inventory.applyFeed(structuredClone(NEW)), serve(NEW)]);
  assert.deepEqual(views(), ['loading', 'old', 'loading', 'new']);
  const refreshed = inventory.refresh().catch(() => {});
  await serve(OLD);
  await refreshed;
  const retried = inventory.retry().catch(() => {});
  await serve(OLD);
  await retried;
  assert.ok(!after(views(), 'new').includes('old'), `history ${views()}`);
  assert.ok(!selectable(published).includes('spent@1'));
});

test('RB1-N1 compact reproduction: two lagging reads with the feed current publish nothing old', async () => {
  const s = v => ({ view_digest: v, objects: [{ id: 'coin', version: v === 'old' ? 1 : 2, asset: 'PFT', value: 10 }], total_value: 10, truncated: false, next_cursor: null });
  let upstream = s('new');
  const history = [];
  const inventory = createFastpayInventory({
    read: async () => structuredClone(upstream),
    publish: x => history.push(x.status === 'ok' ? x.snapshot.view_digest : x.status),
  });
  const feed = s('new');
  await inventory.applyFeed(feed);
  upstream = s('old');
  await inventory.refresh().catch(() => {});
  await inventory.applyFeed(feed);
  assert.deepEqual(history.slice(0, 2), ['loading', 'new']);
  assert.ok(!history.includes('old'), `history ${history}`);
});

test('RB1-N1 E12: paged reads of a lagging node do not republish the older inventory', async () => {
  const objs = (version) => Array.from({ length: 2049 }, (_, i) => ({ id: `o${i}`, version: i === 0 ? version : 1, value: 1, asset: 'PFT' }));
  const pages = (view, version) => { const o = objs(version); return [page(view, o.slice(0, 2048), 'c'), page(view, o.slice(2048), null)]; };
  const rpc = scripted([...pages('new', 2), ...pages('new', 2), ...Array.from({ length: 6 }, () => pages('old', 1)).flat()]);
  const { inventory, published } = inventoryOver(rpc);
  const feed = { ...result('new', objs(2).slice(0, 2048), 'c'), view_digest: 'new' };
  await inventory.applyFeed(feed);
  assert.equal(published.at(-1).snapshot.view_digest, 'new');
  assert.equal(published.at(-1).snapshot.objects.length, 2049);
  await inventory.refresh().catch(() => {});
  await inventory.applyFeed(feed);
  assert.notEqual(published.at(-1).status, 'ok', 'the old paged inventory is not published');
  assert.ok(!published.some(p => p.status === 'ok' && p.snapshot.view_digest === 'old'));
});

test('RB1-N2: Retry resynchronizes a stale feed constraint after two confirmed reads of one unreplaced view', async () => {
  const { inventory, reads, published, views, serve } = served();
  const first = inventory.refresh();
  await serve(NEW);
  await first;
  const fed = inventory.applyFeed(structuredClone(OLD)); // delayed old feed, then the feed stalls
  await serve(NEW);
  await fed;
  assert.equal(published.at(-1).status, 'error');
  const refreshed = inventory.refresh().catch(() => {}); // a visibility refresh does not resynchronize
  await serve(NEW);
  await refreshed;
  assert.equal(published.at(-1).status, 'error');
  const before = reads.length;
  const retried = inventory.retry();
  await serve(NEW);
  assert.equal((await retried).view_digest, 'new');
  assert.equal(reads.length - before, 4, 'two confirmed reads');
  assert.equal(views().at(-1), 'new');
  // The resynchronized view is the constraint now: a lagging node cannot roll it back.
  const lagging = inventory.refresh().catch(() => {});
  await serve(OLD);
  await lagging;
  assert.ok(!after(views(), 'new').includes('old'), `history ${views()}`);
});

test('RB1-N2: Retry publishes an honest newer view the feed has not reached yet', async () => {
  const { inventory, views, serve } = served();
  const mid = result('mid', [shared], null);
  const next = result('next', [shared, replacement], null);
  const fed = inventory.applyFeed(mid);
  await serve(next);
  await fed;
  assert.deepEqual(views(), ['loading', 'error']);
  const retried = inventory.retry();
  await serve(next);
  await retried;
  assert.equal(views().at(-1), 'next');
});
