import { humanRpcErrorMessage } from './rpc-client.js';

// The FastPay inventory a wallet screen selects from. Only a complete answer
// is published: the bounded all-pages read (`read`), or a live-feed answer
// the gateway marks complete. A partial feed page is never published; when
// its view_digest differs from the last complete read, `read` runs again
// (one at a time) and its result is published in one step. After a failed
// read partial pages do not retry on their own: the error stays until a
// complete answer or the user's retry.
export function createFastpayInventory({ read, publish }) {
  let complete = null;
  let inflight = null;
  let failed = false;

  function refresh() {
    if (inflight) return inflight;
    inflight = (async () => {
      try {
        const snapshot = await read();
        complete = snapshot;
        failed = false;
        publish({ status: 'ok', snapshot, error: '' });
        return snapshot;
      } catch (error) {
        failed = true;
        publish({ status: 'error', error: `FastPay balance unavailable: ${humanRpcErrorMessage(error)}` });
        throw error;
      } finally {
        inflight = null;
      }
    })();
    return inflight;
  }

  let lastFeed = null;

  // Idempotent per feed answer, so a re-rendering caller may repeat it.
  async function applyFeed(owned, ownedComplete) {
    if (!owned || owned === lastFeed) return;
    lastFeed = owned;
    const whole = ownedComplete ?? owned.truncated !== true;
    if (whole) {
      complete = owned;
      failed = false;
      publish({ status: 'ok', snapshot: owned, error: '' });
      return;
    }
    const view = owned.view_digest ?? null;
    if (complete && view && complete.view_digest === view) return;
    if (inflight || failed) return;
    await refresh().catch(() => {});
  }

  return { refresh, applyFeed };
}
