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
//   second read returns the same view and, while an invalidation stands,
//   that view is the latest feed view. An invalidation is lifted only by
//   publishing a read of the view that caused it, never by a later read of
//   another view (a lagging node, or a delayed feed answer of an old view).
// - A refresh that cannot confirm such a view in
//   FASTPAY_INVENTORY_READ_ATTEMPTS attempts reports `view_unstable`.
//   After a failure feed answers only move the awaited view; the error
//   stays until the user's retry publishes.
// One refresh runs at a time; `dispose` fences a replaced coordinator, so
// its outstanding reads publish nothing.
export function createFastpayInventory({ read, publish, attempts = FASTPAY_INVENTORY_READ_ATTEMPTS }) {
  let publishedView = null;
  let awaitedView = null;
  let inflight = null;
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

  async function readConfirmed() {
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
      if (viewOf(first) === view && (awaitedView === null || view === awaitedView)) {
        publishedView = view;
        awaitedView = null;
        failed = false;
        loading = false;
        emit({ status: 'ok', snapshot: second, error: '' });
        return second;
      }
    }
    return fail({ code: OWNED_OBJECTS_VIEW_UNSTABLE, message: 'no confirmed FastPay view' });
  }

  function refresh() {
    if (inflight) return inflight;
    inflight = readConfirmed().finally(() => { inflight = null; });
    return inflight;
  }

  // Idempotent per feed answer, so a re-rendering caller may repeat it.
  // Extra arguments (completeness, gateway timestamps) are ignored: neither
  // orders a feed answer against a read.
  async function applyFeed(owned) {
    if (disposed || !owned || owned === lastFeed) return;
    lastFeed = owned;
    const view = viewOf(owned);
    if (awaitedView === null && view === publishedView) return;
    awaitedView = view;
    if (failed) return;
    if (!loading) {
      loading = true;
      emit({ status: 'loading', reason: 'refreshing', error: '' });
    }
    if (inflight) return; // the running refresh checks the new awaited view
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

  return { refresh, applyFeed, attach, dispose };
}
