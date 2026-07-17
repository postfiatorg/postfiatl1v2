// RPC client for PostFiat L1 — WebSocket transport to the proxy.

export class RpcClient {
  constructor(url) {
    // Validate URL scheme — only ws:// and wss:// allowed
    if (url && !url.startsWith('ws://') && !url.startsWith('wss://')) {
      throw new Error('RPC endpoint must use ws:// or wss:// scheme');
    }
    this.url = url || 'ws://localhost:8080';
    this.ws = null;
    this.requestId = 0;
    this.pending = new Map();
    this.connectPromise = null;
  }

  async connect() {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) return;
    // Clear stale connect promise from a failed/closed connection
    this.connectPromise = null;
    this.connectPromise = new Promise((resolve, reject) => {
      this.ws = new WebSocket(this.url);
      this.ws.onopen = () => { this.connectPromise = null; resolve(); };
      this.ws.onmessage = (event) => {
        try {
          const resp = JSON.parse(event.data);
          if (this.pending.has(resp.id)) {
            const { resolve: res, timeout } = this.pending.get(resp.id);
            clearTimeout(timeout);
            this.pending.delete(resp.id);
            res(resp);
          }
        } catch (e) { console.error('RPC parse error:', e); }
      };
      this.ws.onerror = (e) => { this.connectPromise = null; reject(e); };
      this.ws.onclose = () => {
        this.ws = null;
        this.connectPromise = null;
        for (const [, { reject }] of this.pending) reject(new Error('connection closed'));
        this.pending.clear();
      };
    });
    return this.connectPromise;
  }

  async call(method, params = {}, timeoutMs = 10000) {
    await this.connect();
    const id = `ext-${++this.requestId}`;
    const request = { version: 'postfiat-local-rpc-v1', id, method, params };
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`RPC timeout: ${method}`));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timeout });
      try {
        this.ws.send(JSON.stringify(request));
      } catch (e) {
        clearTimeout(timeout);
        this.pending.delete(id);
        reject(new Error(`RPC send failed: ${method}`));
      }
    });
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
  async transferFeeQuote(from, to, amount, sequence) {
    const params = { from, to, amount };
    if (sequence !== undefined) params.sequence = sequence;
    return this.call('transfer_fee_quote', params);
  }
  async submitSignedTransfer(signedTransferJson) {
    return this.call('mempool_submit_signed_transfer', { signed_transfer_json: signedTransferJson }, 30000);
  }
  async receipts(txId) { return this.call('receipts', { tx_id: txId }); }
  async blocks(opts = {}) {
    const params = {};
    if (opts.fromHeight !== undefined) params.from_height = opts.fromHeight;
    if (opts.limit !== undefined) params.limit = opts.limit;
    return this.call('blocks', params);
  }
  async validators() { return this.call('validators'); }

  close() {
    if (this.ws) { this.ws.close(); this.ws = null; }
  }
}
