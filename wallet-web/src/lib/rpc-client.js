// RPC client for PostFiat L1 — WebSocket transport to the proxy.

import { assertNoCustodyMaterial } from './custody-boundary.js';

export const FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT = 2048;

const sleep = (ms) => new Promise(resolve => setTimeout(resolve, ms));

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

export function rpcErrorMessage(resp, fallback = 'RPC request failed') {
  if (!resp) return fallback;
  if (resp.error?.message) return resp.error.message;
  if (resp.error?.code) return resp.error.code;
  return fallback;
}

export function humanRpcErrorMessage(error, fallback = 'Wallet RPC request failed') {
  const raw = String(error?.message || error || fallback);
  if (raw.includes('owned_objects')) {
    return 'FastPay object lookup is unavailable from this RPC endpoint. Check wallet network status and retry.';
  }
  if (raw.includes('account')) {
    return 'Account state is unavailable from this RPC endpoint. Check wallet network status and retry.';
  }
  if (raw.includes('mempool_submit_signed') || raw.includes('finality')) {
    return 'Finality submit is unavailable from this RPC endpoint. Use a finality-enabled wallet endpoint and retry.';
  }
  if (raw.startsWith('RPC send failed:')) {
    return 'Wallet RPC connection dropped while sending the request. Retry after the network status refreshes.';
  }
  return raw;
}

export function parseAccountResult(resp) {
  if (!resp?.ok) {
    throw new Error(rpcErrorMessage(resp, 'account RPC failed'));
  }
  if (!isObject(resp.result)) {
    throw new Error('account RPC response missing result object');
  }

  const account = isObject(resp.result.account) ? resp.result.account : resp.result;
  if (account.balance === undefined || account.balance === null) {
    throw new Error('account RPC response missing balance');
  }

  return {
    ...account,
    balance: account.balance,
    sequence: account.sequence ?? null,
  };
}

export function parseOwnedObjectsResult(resp) {
  if (!resp?.ok) {
    throw new Error(rpcErrorMessage(resp, 'owned_objects RPC failed'));
  }
  if (!isObject(resp.result)) {
    throw new Error('owned_objects RPC response missing result object');
  }

  const objects = Array.isArray(resp.result.objects) ? resp.result.objects : [];
  const totalValue = resp.result.total_value ?? resp.result.totalValue ?? objects.reduce((sum, obj) => {
    const value = obj?.value ?? obj?.amount ?? 0;
    return sum + BigInt(value);
  }, 0n);

  return {
    ...resp.result,
    objects,
    total_value: totalValue,
    totalValue,
  };
}

export async function fetchOwnedObjectsSnapshot(rpc, ownerPublicKeyHex, opts = {}) {
  if (!rpc) throw new Error('RPC client is not connected');
  if (!ownerPublicKeyHex) throw new Error('Wallet public key is missing');

  const resp = await rpc.ownedObjects(ownerPublicKeyHex, {
    asset: opts.asset || 'PFT',
    limit: opts.limit ?? FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,
  });
  return parseOwnedObjectsResult(resp);
}

export async function pollOwnedObjectsTotal(rpc, ownerPublicKeyHex, opts = {}) {
  const timeoutMs = opts.timeoutMs ?? 10000;
  const intervalMs = opts.intervalMs ?? 500;
  const minTotal = BigInt(opts.minTotal ?? 0);
  const startedAt = Date.now();
  let lastSnapshot = null;

  while (Date.now() - startedAt <= timeoutMs) {
    lastSnapshot = await fetchOwnedObjectsSnapshot(rpc, ownerPublicKeyHex, opts);
    opts.onSnapshot?.(lastSnapshot);
    if (BigInt(lastSnapshot.totalValue ?? 0) >= minTotal) {
      return { ok: true, snapshot: lastSnapshot };
    }

    const elapsed = Date.now() - startedAt;
    if (elapsed >= timeoutMs) break;
    await sleep(Math.min(intervalMs, timeoutMs - elapsed));
  }

  return { ok: false, snapshot: lastSnapshot };
}

export class RpcClient {
  constructor(url, proxyAuthToken = '') {
    if (url && !url.startsWith('ws://') && !url.startsWith('wss://')) {
      throw new Error('RPC endpoint must use ws:// or wss:// scheme');
    }
    this.url = url || `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}/rpc`;
    this.proxyAuthToken = String(proxyAuthToken || '');
    this.ws = null;
    this.requestId = 0;
    this.pending = new Map();
    this.notificationHandlers = new Map();
    this.connectionCloseHandlers = new Set();
    this.connectPromise = null;
    this.heartbeatInterval = null;
  }

  _emitNotification(method, params, message) {
    const handlers = this.notificationHandlers.get(method);
    if (!handlers) return;
    for (const handler of [...handlers]) {
      try {
        handler(params, message);
      } catch (e) {
        console.error('RPC notification handler failed:', e);
      }
    }
  }

  onNotification(method, handler) {
    if (!this.notificationHandlers.has(method)) {
      this.notificationHandlers.set(method, new Set());
    }
    const handlers = this.notificationHandlers.get(method);
    handlers.add(handler);
    return () => {
      handlers.delete(handler);
      if (handlers.size === 0) this.notificationHandlers.delete(method);
    };
  }

  onConnectionClose(handler) {
    this.connectionCloseHandlers.add(handler);
    return () => {
      this.connectionCloseHandlers.delete(handler);
    };
  }

  _emitConnectionClose() {
    for (const handler of [...this.connectionCloseHandlers]) {
      try {
        handler();
      } catch (e) {
        console.error('RPC close handler failed:', e);
      }
    }
  }

  _forceReconnect() {
    this._stopHeartbeat();
    if (this.ws) {
      try { this.ws.close(); } catch (_) {}
    }
    this.ws = null;
    this.connectPromise = null;
  }

  _startHeartbeat() {
    this._stopHeartbeat();
    // Send a status ping every 30 seconds to keep the WebSocket alive
    this.heartbeatInterval = setInterval(() => {
      if (this.ws && this.ws.readyState === WebSocket.OPEN) {
        try {
          this.ws.send(JSON.stringify({
            version: 'postfiat-local-rpc-v1',
            id: `heartbeat-${++this.requestId}`,
            method: 'status',
            params: {},
          }));
        } catch (_) {
          // Send failed — connection is stale, will be cleaned up on next call()
          this.ws = null;
          this.connectPromise = null;
        }
      }
    }, 30000);
  }

  _stopHeartbeat() {
    if (this.heartbeatInterval) {
      clearInterval(this.heartbeatInterval);
      this.heartbeatInterval = null;
    }
  }

  setUrl(url) {
    if (url && !url.startsWith('ws://') && !url.startsWith('wss://')) {
      throw new Error('RPC endpoint must use ws:// or wss:// scheme');
    }
    this.close();
    this.url = url;
  }

  setProxyAuthToken(token) {
    this.proxyAuthToken = String(token || '');
  }

  async connect() {
    // Already connected
    if (this.ws && this.ws.readyState === WebSocket.OPEN) return;
    // If socket exists but is in CLOSING or CLOSED state, clear it so we can reconnect
    if (this.ws && (this.ws.readyState === WebSocket.CLOSING || this.ws.readyState === WebSocket.CLOSED)) {
      this.ws = null;
      this.connectPromise = null;
    }
    // Connection already in progress — wait for it (with timeout)
    if (this.ws && this.ws.readyState === WebSocket.CONNECTING && this.connectPromise) {
      return Promise.race([
        this.connectPromise,
        new Promise((_, reject) => setTimeout(() => reject(new Error('WebSocket connect timeout')), 5000)),
      ]);
    }
    // Reset and create new connection
    this.connectPromise = new Promise((resolve, reject) => {
      // Connection timeout — if the WebSocket doesn't open within 5 seconds, reject
      const connectTimeout = setTimeout(() => {
        try { this.ws?.close(); } catch (_) {}
        this.ws = null;
        this.connectPromise = null;
        reject(new Error('WebSocket connection timeout'));
      }, 5000);
      this.ws = new WebSocket(this.url);
      this.ws.onopen = () => {
        clearTimeout(connectTimeout);
        this.connectPromise = null;
        this._startHeartbeat();
        resolve();
      };
      this.ws.onmessage = (event) => {
        try {
          const resp = JSON.parse(event.data);
          if (this.pending.has(resp.id)) {
            const { resolve: res, timeout } = this.pending.get(resp.id);
            clearTimeout(timeout);
            this.pending.delete(resp.id);
            res(resp);
          } else if (resp.method) {
            this._emitNotification(resp.method, resp.params || {}, resp);
          }
        } catch (e) { console.error('RPC parse error:', e); }
      };
      this.ws.onerror = (e) => {
        clearTimeout(connectTimeout);
        this.connectPromise = null;
        reject(e);
      };
      this.ws.onclose = () => {
        clearTimeout(connectTimeout);
        this._stopHeartbeat();
        this.ws = null;
        this.connectPromise = null;
        for (const [, { reject }] of this.pending) reject(new Error('connection closed'));
        this.pending.clear();
        this._emitConnectionClose();
      };
    });
    return this.connectPromise;
  }

  async call(method, params = {}, timeoutMs = 10000) {
    assertNoCustodyMaterial(params, `wallet RPC ${method}`);
    const maxAttempts = 3;
    // Try a few times — if the first attempt fails due to a stale
    // WebSocket connection, force-close it and retry with a fresh one.
    for (let attempt = 0; attempt < maxAttempts; attempt++) {
      try {
        await this.connect();
      } catch (e) {
        // Connection failed — force close and retry once
        this._forceReconnect();
        if (attempt < maxAttempts - 1) {
          await sleep(100 * (attempt + 1));
          continue;
        }
        return {
          version: 'postfiat-local-rpc-v1',
          id: 'connection-failed',
          ok: false,
          result: null,
          error: { code: 'connection_error', message: e.message || 'WebSocket connection failed' },
          events: [],
        };
      }

      const id = `web-${++this.requestId}`;
      const request = { version: 'postfiat-local-rpc-v1', id, method, params };
      if (this.proxyAuthToken) request.proxy_auth_token = this.proxyAuthToken;
      try {
        const result = await new Promise((resolve, reject) => {
          const timeout = setTimeout(() => {
            this.pending.delete(id);
            reject(new Error(`RPC timeout: ${method}`));
          }, timeoutMs);
          this.pending.set(id, { resolve, reject, timeout });
          try {
            if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
              throw new Error('WebSocket is not open');
            }
            this.ws.send(JSON.stringify(request));
          } catch (e) {
            clearTimeout(timeout);
            this.pending.delete(id);
            reject(new Error(`RPC send failed: ${method}`));
          }
        });
        return result;
      } catch (e) {
        // Send failed — the WebSocket is probably stale. Force close and retry.
        this._forceReconnect();
        if (attempt < maxAttempts - 1) {
          await sleep(100 * (attempt + 1));
          continue;
        }
        return {
          version: 'postfiat-local-rpc-v1',
          id: 'send-failed',
          ok: false,
          result: null,
          error: { code: 'connection_error', message: `wallet RPC connection dropped while sending ${method}` },
          events: [],
        };
      }
    }
  }

  async status() { return this.call('status'); }
  async fee() { return this.call('fee'); }
  async account(address) { return this.call('account', { address }); }
  async accountTx(address, opts = {}) {
    const params = { address };
    if (opts.fromHeight !== undefined) params.from_height = opts.fromHeight;
    if (opts.toHeight !== undefined) params.to_height = opts.toHeight;
    if (opts.limit !== undefined) params.limit = opts.limit;
    return this.call('account_tx', params);
  }
  async transferFeeQuote(from, to, amount, sequenceOrOptions) {
    const params = { from, to, amount };
    if (sequenceOrOptions !== undefined) {
      if (sequenceOrOptions && typeof sequenceOrOptions === 'object') {
        const opts = sequenceOrOptions;
        if (opts.sequence !== undefined) params.sequence = opts.sequence;
        if (Object.prototype.hasOwnProperty.call(opts, 'memo_type') && opts.memo_type !== undefined) {
          params.memo_type = opts.memo_type;
        }
        if (Object.prototype.hasOwnProperty.call(opts, 'memo_format') && opts.memo_format !== undefined) {
          params.memo_format = opts.memo_format;
        }
        if (Object.prototype.hasOwnProperty.call(opts, 'memo_data') && opts.memo_data !== undefined) {
          params.memo_data = opts.memo_data;
        }
      } else {
        params.sequence = sequenceOrOptions;
      }
    }
    return this.call('transfer_fee_quote', params);
  }
  async assetFeeQuote(source, operationJson) {
    return this.call('asset_fee_quote', { source, operation_json: operationJson });
  }
  async escrowFeeQuote(source, operationJson, sequence = undefined) {
    const params = { source, operation_json: operationJson };
    if (sequence !== undefined && sequence !== null) params.sequence = sequence;
    return this.call('escrow_fee_quote', params);
  }
  async assetInfo(assetId) { return this.call('asset_info', { asset_id: assetId }); }
  async vaultBridgeRoute(assetId) { return this.call('vault_bridge_route', { asset_id: assetId }); }
  async accountAssets(address) { return this.call('account_assets', { account: address }); }
  async escrowInfo(escrowId) { return this.call('escrow_info', { escrow_id: escrowId }); }
  async ownedObjects(ownerPublicKeyHex, opts = {}) {
    const params = { owner_public_key_hex: ownerPublicKeyHex };
    if (opts.asset) params.asset = opts.asset;
    if (opts.limit !== undefined) params.limit = opts.limit;
    return this.call('owned_objects', params);
  }
  async ownedSign(orderJson, validatorId) {
    return this.call('owned_sign', { order_json: orderJson, validator_id: validatorId }, 30000);
  }
  async ownedApply(certJson) {
    return this.call('owned_apply', { cert_json: certJson }, 30000);
  }
  async ownedRecoveryCapabilities() {
    return this.call('owned_recovery_capabilities');
  }
  async ownedSignV3(orderJson, validatorId) {
    return this.call('owned_sign_v3', { order_json: orderJson, validator_id: validatorId }, 30000);
  }
  async ownedApplyV3(certJson) {
    return this.call('owned_apply_v3', { cert_json: certJson }, 30000);
  }
  async ownedUnwrapSign(orderJson, validatorId) {
    return this.call('owned_unwrap_sign', { order_json: orderJson, validator_id: validatorId }, 30000);
  }
  async ownedUnwrapApply(certJson) {
    return this.call('owned_unwrap_apply', { cert_json: certJson }, 30000);
  }
  async ownedUnwrapSignV3(orderJson, validatorId) {
    return this.call('owned_unwrap_sign_v3', { order_json: orderJson, validator_id: validatorId }, 30000);
  }
  async ownedUnwrapApplyV3(certJson) {
    return this.call('owned_unwrap_apply_v3', { cert_json: certJson }, 30000);
  }
  async ownedCertificate(selector) {
    const params = selector?.certificate_digest
      ? { certificate_digest: selector.certificate_digest }
      : { lock_id: selector?.lock_id };
    return this.call('owned_certificate', params);
  }
  async ownedRecoveryStatus(lockId) {
    return this.call('owned_recovery_status', { lock_id: lockId });
  }
  async accountLines(address) { return this.call('account_lines', { account: address }); }
  async issuerAssets(issuer) { return this.call('issuer_assets', { issuer }); }
  async offerFeeQuote(source, operationJson) {
    return this.call('offer_fee_quote', { source, operation_json: operationJson });
  }
  async offerInfo(offerId) { return this.call('offer_info', { offer_id: offerId }); }
  async accountOffers(address) { return this.call('account_offers', { account: address }); }
  async bookOffers(paysAsset, getsAsset) {
    return this.call('book_offers', { pays_asset: paysAsset, gets_asset: getsAsset });
  }
  async submitSignedTransfer(signedTransferJson) {
    return this.call('mempool_submit_signed_transfer', { signed_transfer_json: signedTransferJson }, 30000);
  }
  async submitSignedTransferFinality(signedTransferJson) {
    return this.call('mempool_submit_signed_transfer_finality', { signed_transfer_json: signedTransferJson }, 30000);
  }
  async submitSignedPaymentV2(signedPaymentV2Json) {
    return this.call('mempool_submit_signed_payment_v2', { signed_payment_v2_json: signedPaymentV2Json }, 30000);
  }
  async submitSignedPaymentV2Finality(signedPaymentV2Json) {
    return this.call('mempool_submit_signed_payment_v2_finality', { signed_payment_v2_json: signedPaymentV2Json }, 30000);
  }
  async submitSignedAssetTransaction(signedAssetJson) {
    return this.call('mempool_submit_signed_asset_transaction', { signed_asset_transaction_json: signedAssetJson }, 30000);
  }
  async submitSignedAssetTransactionFinality(signedAssetJson) {
    return this.call('mempool_submit_signed_asset_transaction_finality', { signed_asset_transaction_json: signedAssetJson }, 30000);
  }
  async submitSignedEscrowTransaction(signedEscrowJson) {
    return this.call('mempool_submit_signed_escrow_transaction', { signed_escrow_transaction_json: signedEscrowJson }, 30000);
  }
  async submitSignedEscrowTransactionFinality(signedEscrowJson) {
    return this.call('mempool_submit_signed_escrow_transaction_finality', { signed_escrow_transaction_json: signedEscrowJson }, 30000);
  }
  async submitSignedOfferTransaction(signedOfferJson) {
    return this.call('mempool_submit_signed_offer_transaction', { signed_offer_json: signedOfferJson }, 30000);
  }
  async submitFastlanePrimary(fastlanePrimaryJson) {
    return this.call('mempool_submit_fastlane_primary', { fastlane_primary_json: fastlanePrimaryJson }, 30000);
  }
  async receipts(txId) { return this.call('receipts', { tx_id: txId }); }
  async tx(txId) { return this.call('tx', { tx_id: txId }); }
  async serverInfo() { return this.call('server_info'); }
  async serverCapabilities() {
    const [info, status] = await Promise.all([
      this.call('server_info').catch(() => null),
      this.call('status').catch(() => null),
    ]);
    const rpc = (info && info.result && info.result.rpc) || {};
    const st = (status && status.result) || {};
    return {
      ok: !!(info && info.ok) || !!(status && status.ok),
      read_only: rpc.read_only ?? true,
      mempool_submit_enabled: rpc.mempool_submit_enabled ?? false,
      mempool_submit_finality_enabled: rpc.mempool_submit_finality_enabled ?? false,
      mempool_submit_asset_transaction_finality_enabled: rpc.mempool_submit_asset_transaction_finality_enabled ?? false,
      owned_lane_enabled: rpc.owned_lane_enabled ?? false,
      owned_certificate_domain: rpc.owned_certificate_domain ?? null,
      block_height: st.block_height ?? 0,
      genesis_hash: st.genesis_hash ?? '',
      protocol_version: st.protocol_version ?? 0,
      mempool_pending: st.mempool_pending ?? 0,
      chain_id: st.chain_id ?? '',
      validator_count: st.validator_count ?? 0,
      last_run_unix: st.last_run_unix ?? 0,
    };
  }
  async blocks(opts = {}) {
    const params = {};
    if (opts.fromHeight !== undefined) params.from_height = opts.fromHeight;
    if (opts.limit !== undefined) params.limit = opts.limit;
    return this.call('blocks', params);
  }
  async validators() { return this.call('validators'); }
  async walletSubscribe(params, handler) {
    const resp = await this.call('wallet_subscribe', params, 10000);
    if (!resp.ok) {
      throw new Error(rpcErrorMessage(resp, 'wallet feed subscribe failed'));
    }
    const subscriptionId = resp.result?.subscription_id;
    if (!subscriptionId) {
      throw new Error('wallet feed subscribe response missing subscription id');
    }
    const off = this.onNotification('wallet_update', (notificationParams, message) => {
      if (notificationParams?.subscription_id !== subscriptionId) return;
      handler(notificationParams.snapshot, notificationParams, message);
    });
    return {
      subscriptionId,
      intervalMs: resp.result?.interval_ms ?? params?.interval_ms ?? null,
      drop: off,
      unsubscribe: async () => {
        off();
        try {
          await this.walletUnsubscribe(subscriptionId);
        } catch (_) {
          // Connection may already be closed; local handler removal is enough.
        }
      },
    };
  }
  async walletUnsubscribe(subscriptionId) {
    return this.call('wallet_unsubscribe', { subscription_id: subscriptionId }, 5000);
  }

  close() {
    this._stopHeartbeat();
    if (this.ws) { this.ws.close(); this.ws = null; }
    this.connectPromise = null;
  }
}
