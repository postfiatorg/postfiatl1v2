import {
  humanRpcErrorMessage,
  OWNED_OBJECTS_PAGING_UNSUPPORTED,
  OWNED_OBJECTS_VIEW_UNSTABLE,
} from './rpc-client.js';

const REASONS = {
  [OWNED_OBJECTS_PAGING_UNSUPPORTED]: 'paging_unsupported',
  [OWNED_OBJECTS_VIEW_UNSTABLE]: 'view_unstable',
};

export function fastpayInventoryErrorMessage(error) {
  if (error?.code === OWNED_OBJECTS_PAGING_UNSUPPORTED) {
    return 'This RPC node is too old to page FastPay objects, so the wallet cannot read them all. Switch to an upgraded node, then retry.';
  }
  if (error?.code === OWNED_OBJECTS_VIEW_UNSTABLE) {
    return 'FastPay objects kept changing between pages (the account is busy, or validators serve different views). Retry shortly.';
  }
  return `FastPay balance unavailable: ${humanRpcErrorMessage(error)}`;
}

// How many confirmed-read attempts one refresh makes before it reports the
// inventory unavailable.
export const FASTPAY_INVENTORY_READ_ATTEMPTS = 3;

// A view's identity: its view_digest, or (a node from before view digests)
// its objects.
function viewOf(snapshot) {
  if (snapshot?.view_digest) return snapshot.view_digest;
  const objects = Array.isArray(snapshot?.objects) ? snapshot.objects : [];
  return JSON.stringify(objects.map(o => [o?.asset, o?.id, o?.version, o?.value]));
}

// How many replaced views a session remembers (oldest forgotten first).
export const FASTPAY_REPLACED_VIEWS_KEPT = 64;

// The FastPay inventory a wallet screen selects from.
//
// Nodes expose no inventory revision a wallet can compare, and the gateway's
// feed and the wallet's reads are fleet answers that name no single node,
// so neither source is ranked by time (docs/wallets/fastpay-inventory-ordering.md).
// The interim rule:
// - A feed answer is never published. One whose view differs from the
//   published view (complete or partial) invalidates it: selection becomes
//   unavailable (`loading`) and the wallet reads.
// - Only the wallet's own all-pages read is published, and only when a
//   second read returns the same view, that view is the latest feed view
//   (or no feed view has been seen), and the coordinator has not already
//   replaced it. The latest feed view is a standing constraint for every
//   later refresh, so a lagging node cannot roll back a confirmed view.
// - A refresh that cannot confirm such a view in
//   FASTPAY_INVENTORY_READ_ATTEMPTS attempts reports `view_unstable`.
//   After a failure feed answers only move the feed view; the error stays
//   until a refresh publishes.
// - `retry` (the user's Retry) also resynchronizes a stale feed view: two
//   confirmed reads of one view that was never replaced become the feed
//   view and are published. It cannot tell a lagging node from a stalled
//   feed; it only refuses views the session already replaced.
// One refresh runs at a time; `dispose` fences a replaced coordinator, so
// its outstanding reads publish nothing.
export function createFastpayInventory({ read, publish, attempts = FASTPAY_INVENTORY_READ_ATTEMPTS }) {
  let publishedView = null;
  let feedView = null;
  const replaced = new Set();
  let inflight = null;
  let resync = false;
  let failed = false;
  let loading = false;
  let disposed = false;
  let lastFeed = null;

  function emit(state) {
    if (!disposed) publish(state);
  }

  function fail(error) {
    failed = true;
    loading = false;
    emit({ status: 'error', reason: REASONS[error?.code] || 'unavailable', error: fastpayInventoryErrorMessage(error) });
    throw error;
  }

  function replace(view) {
    if (publishedView === null || publishedView === view) return;
    replaced.delete(publishedView);
    replaced.add(publishedView);
    if (replaced.size > FASTPAY_REPLACED_VIEWS_KEPT) replaced.delete(replaced.values().next().value);
  }

  async function readConfirmed() {
    let candidate = null;
    for (let attempt = 0; attempt < attempts; attempt += 1) {
      let first;
      let second;
      try {
        first = await read();
        if (disposed) return first;
        second = await read();
      } catch (error) {
        if (disposed) return null;
        return fail(error);
      }
      if (disposed) return second;
      const view = viewOf(second);
      if (viewOf(first) !== view || replaced.has(view)) { candidate = null; continue; }
      if (feedView !== null && view !== feedView) {
        // Only a Retry may move a stale feed view, and only to a view two
        // confirmed reads agree on.
        if (!resync || candidate !== view) { candidate = resync ? view : null; continue; }
        feedView = view;
      }
      replace(view);
      publishedView = view;
      failed = false;
      loading = false;
      emit({ status: 'ok', snapshot: second, error: '' });
      return second;
    }
    return fail({ code: OWNED_OBJECTS_VIEW_UNSTABLE, message: 'no confirmed FastPay view' });
  }

  function refresh() {
    if (inflight) return inflight;
    inflight = readConfirmed().finally(() => { inflight = null; resync = false; });
    return inflight;
  }

  // The user's Retry: a refresh that may resynchronize a stale feed view.
  function retry() {
    resync = true;
    return refresh();
  }

  // Idempotent per feed answer, so a re-rendering caller may repeat it.
  // Extra arguments (completeness, gateway timestamps) are ignored: neither
  // orders a feed answer against a read.
  async function applyFeed(owned) {
    if (disposed || !owned || owned === lastFeed) return;
    lastFeed = owned;
    const view = viewOf(owned);
    feedView = view;
    if (failed || (view === publishedView && !loading)) return;
    if (!loading) {
      loading = true;
      emit({ status: 'loading', reason: 'refreshing', error: '' });
    }
    if (inflight) return; // the running refresh checks the new feed view
    await refresh().catch(() => {});
  }

  // A screen attaches its coordinator while mounted and disposes it when the
  // coordinator is replaced or unmounted (React StrictMode re-attaches).
  function attach() {
    disposed = false;
  }

  function dispose() {
    disposed = true;
  }

  return { refresh, retry, applyFeed, attach, dispose };
}
