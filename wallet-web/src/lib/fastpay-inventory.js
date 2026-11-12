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

// The FastPay inventory a wallet screen selects from. Only a complete answer
// is published: the bounded all-pages read (`read`), or a live-feed answer
// the gateway marks complete. A partial feed page is never published; when
// its view_digest differs from the last complete view, selection becomes
// unavailable (`loading`) and `read` runs again (one at a time) until a read
// that started after the last invalidation completes.
//
// Fencing: every publication and invalidation advances `generation`. A read
// completing after a newer complete view was published is discarded; one
// completing after an invalidation is read again (unless it already shows
// the invalidating view). Feed answers carry the gateway's `observed_at_ms`
// as `order`; an answer no newer than the last one is ignored, so an older
// complete view neither replaces a newer one nor clears an error. `dispose`
// fences a replaced coordinator: its outstanding reads publish nothing.
// After a failed read partial pages do not retry on their own: the error
// stays until a newer complete answer or the user's retry.
export function createFastpayInventory({ read, publish }) {
  let complete = null;
  let inflight = null;
  let failed = false;
  let disposed = false;
  let generation = 0;
  let publishedAt = 0;
  let awaitedView = null;
  let lastFeed = null;
  let lastOrder = null;

  function emit(state) {
    if (!disposed) publish(state);
  }

  function publishComplete(snapshot) {
    complete = snapshot;
    failed = false;
    awaitedView = null;
    generation += 1;
    publishedAt = generation;
    emit({ status: 'ok', snapshot, error: '' });
  }

  async function readCurrent() {
    for (;;) {
      const started = generation;
      let snapshot;
      try {
        snapshot = await read();
      } catch (error) {
        if (disposed || (generation !== started && generation === publishedAt)) return complete;
        failed = true;
        emit({ status: 'error', reason: REASONS[error?.code] || 'unavailable', error: fastpayInventoryErrorMessage(error) });
        throw error;
      }
      if (disposed) return snapshot;
      const invalidated = generation !== publishedAt;
      if (generation === started || (invalidated && awaitedView && snapshot?.view_digest === awaitedView)) {
        publishComplete(snapshot);
        return snapshot;
      }
      if (!invalidated) return complete; // a newer complete view was published meanwhile
    }
  }

  function refresh() {
    if (inflight) return inflight;
    inflight = readCurrent().finally(() => { inflight = null; });
    return inflight;
  }

  // Idempotent per feed answer, so a re-rendering caller may repeat it.
  async function applyFeed(owned, ownedComplete, order) {
    if (disposed || !owned || owned === lastFeed) return;
    if (order != null) {
      if (lastOrder != null && !(order > lastOrder)) return;
      lastOrder = order;
    }
    lastFeed = owned;
    const whole = ownedComplete ?? owned.truncated !== true;
    if (whole) {
      publishComplete(owned);
      return;
    }
    const view = owned.view_digest ?? null;
    if (complete && view && complete.view_digest === view && generation === publishedAt) return;
    if (failed) return;
    generation += 1;
    awaitedView = view;
    emit({ status: 'loading', reason: 'refreshing', error: '' });
    if (inflight) return; // the running read sees the invalidation and reads again
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
