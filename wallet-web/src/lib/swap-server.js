import {
  assertNoShieldedPrivateMaterial,
  isShieldedNavswapRequest,
  SHIELDED_NAVSWAP_ROUTE,
} from './shielded-navswap.js';
import { assertNoCustodyMaterial } from './custody-boundary.js';

// Swap server API client - HTTP client for the companion swap server.
// The swap server never receives the user's seed, passphrase, private keys,
// shielded note openings, or Asset-Orchard spend authority.

export class SwapServer {
  constructor(baseUrl, proxyAuthToken = '') {
    this.baseUrl = normalizeSwapServerUrl(baseUrl || defaultSwapServerUrl());
    this.proxyAuthToken = String(proxyAuthToken || '');
  }

  setUrl(url) {
    this.baseUrl = normalizeSwapServerUrl(url);
  }

  setProxyAuthToken(token) {
    this.proxyAuthToken = String(token || '');
  }

  async _request(method, path, body) {
    if (isShieldedNavswapRequest(path, body) && body) {
      assertNoShieldedPrivateMaterial(body);
    }
    if (body) assertNoCustodyMaterial(body, `wallet HTTP ${method} ${path}`);
    const url = this.baseUrl.replace(/\/+$/, '') + path;
    const options = {
      method,
      headers: { 'Accept': 'application/json' },
    };
    if (this.proxyAuthToken) {
      options.headers.Authorization = `Bearer ${this.proxyAuthToken}`;
    }
    if (body) {
      options.headers['Content-Type'] = 'application/json';
      options.body = JSON.stringify(body);
    }
    const resp = await fetch(url, options);
    let data;
    try {
      data = await resp.json();
    } catch (e) {
      throw new Error(`Swap server returned non-JSON: ${resp.status}`);
    }
    if (!resp.ok && !data.ok) {
      const err = new Error(data.error || data.message || `Swap server error: ${resp.status}`);
      err.status = resp.status;
      err.data = data;
      throw err;
    }
    return data;
  }

  async getStatus() {
    return this._request('GET', '/api/swap/status');
  }

  async getBalances() {
    return this._request('GET', '/api/swap/balances');
  }

  async getNav(phase) {
    return this._request('GET', `/api/swap/nav?phase=${phase}`);
  }

  async action(body) {
    return this._request('POST', '/api/swap/action', body);
  }

  async getNavswapCapabilities() {
    return this._request('GET', '/api/navswap/capabilities');
  }

  async getNavswapNavProof({ assetId, phase = 'current' } = {}) {
    const params = new URLSearchParams();
    if (assetId) params.set('asset_id', assetId);
    if (phase) params.set('phase', phase);
    const query = params.toString();
    return this._request('GET', `/api/navswap/nav-proof${query ? `?${query}` : ''}`);
  }

  async quoteNavswap(body) {
    assertNotShieldedValueMovingRoute(body, 'quote');
    return this._request('POST', '/api/navswap/quotes', body);
  }

  async planNavswapInputs(body) {
    assertNotShieldedValueMovingRoute(body, 'planner input');
    return this._request('POST', '/api/navswap/planner-inputs', body);
  }

  async getNavswapReadiness(body) {
    assertNotShieldedValueMovingRoute(body, 'readiness');
    return this._request('POST', '/api/navswap/readiness', body);
  }

  async fundNavswapPfusdc(body) {
    return this._request('POST', '/api/navswap/devnet-fund-pfusdc', withNavswapIdempotency(body, 'navswap-funding'));
  }

  async runNavswap(body) {
    assertNotShieldedValueMovingRoute(body, 'run');
    return this._request('POST', '/api/navswap/runs', withNavswapIdempotency(body, 'navswap-run'));
  }

  async prepareNavswapAction(body) {
    assertNotShieldedValueMovingRoute(body, 'action prepare');
    return this._request('POST', '/api/navswap/actions/prepare', body);
  }

  async prepareNavswapActionBatch(body) {
    assertNotShieldedValueMovingRoute(body, 'action batch prepare');
    return this._request('POST', '/api/navswap/actions/prepare-batch', body);
  }

  async getShieldedNavswapStatus() {
    return this._request('GET', '/api/shielded-nav-swap/status');
  }

  async getShieldedNavswapBalances() {
    return this._request('GET', '/api/shielded-nav-swap/balances');
  }

  async getShieldedNavswapNoteCapability() {
    return this._request('GET', '/api/shielded-nav-swap/note-capability');
  }

  async getShieldedNavswapProverReadiness() {
    return this._request('GET', '/api/shielded-nav-swap/prover-readiness');
  }

  async getShieldedNavswapQuote(body) {
    return this._request('POST', '/api/shielded-nav-swap/quote', body);
  }

  async getShieldedNavswapPreflight(body) {
    return this._request('POST', '/api/shielded-nav-swap/preflight', body);
  }

  async submitShieldedNavswapIngress(body) {
    return this._request('POST', '/api/shielded-nav-swap/ingress', withNavswapIdempotency(body, 'shielded-navswap-ingress'));
  }

  async submitShieldedNavswapSwap(body) {
    return this._request('POST', '/api/shielded-nav-swap/swap', withNavswapIdempotency(body, 'shielded-navswap-swap'));
  }

  async submitShieldedNavswapEgress(body) {
    return this._request('POST', '/api/shielded-nav-swap/egress', withNavswapIdempotency(body, 'shielded-navswap-egress'));
  }

  async runPrivateSwapWorkflow(body) {
    return this._request('POST', '/api/private-swap-workflow', body);
  }

  async getNavswapRun(runId) {
    return this._request('GET', `/api/navswap/runs/${encodeURIComponent(runId)}`);
  }

  async getNavswapRuns({ walletAddress, wallet_address, route, includeTerminal = false, limit } = {}) {
    const params = new URLSearchParams();
    const resolvedWallet = walletAddress || wallet_address;
    if (resolvedWallet) params.set('wallet_address', resolvedWallet);
    if (route) params.set('route', route);
    if (includeTerminal) params.set('include_terminal', 'true');
    if (limit !== undefined && limit !== null) params.set('limit', String(limit));
    const query = params.toString();
    return this._request('GET', `/api/navswap/runs${query ? `?${query}` : ''}`);
  }

  async getNavswapRunEvents(runId) {
    return this._request('GET', `/api/navswap/runs/${encodeURIComponent(runId)}/events`);
  }

  navswapRunStreamUrl(runId) {
    return `${this.baseUrl.replace(/\/+$/, '')}/api/navswap/runs/${encodeURIComponent(runId)}/stream`;
  }

  async getNavswapRunReceipts(runId) {
    return this._request('GET', `/api/navswap/runs/${encodeURIComponent(runId)}/receipts`);
  }

  async buildAtomicSettlementTemplate(body) {
    return this._request('POST', '/api/navswap/atomic-templates', body);
  }

  async pollAction(action, statusSet, timeoutMs = 600000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      await new Promise(r => setTimeout(r, 3000));
      try {
        const status = await this.getStatus();
        if (status.status && statusSet.has(status.status)) {
          return status;
        }
      } catch (e) {
        // keep polling
      }
    }
    throw new Error(`Swap action ${action} timed out`);
  }
}

function isLoopbackHost(hostname) {
  const host = String(hostname || '').toLowerCase();
  return host === 'localhost' || host === '127.0.0.1' || host === '::1' || host === '[::1]';
}

export function defaultSwapServerUrl() {
  if (typeof window === 'undefined' || !window.location) {
    return 'http://localhost:8080';
  }
  const { protocol, hostname, host, port } = window.location;
  if (port === '5173') {
    if (protocol === 'https:') {
      return `${protocol}//${host}`;
    }
    return `http://${hostname || '127.0.0.1'}:8080`;
  }
  return `${protocol}//${host}`;
}

export function normalizeSwapServerUrl(url) {
  const value = String(url || '').trim();
  if (!value) return defaultSwapServerUrl();

  let parsed;
  try {
    parsed = new URL(value);
  } catch (_) {
    return value;
  }

  if (typeof window === 'undefined' || !window.location) return value;

  const pageIsHttps = window.location.protocol === 'https:';
  const pageIsLoopback = isLoopbackHost(window.location.hostname);
  if (isLoopbackHost(parsed.hostname) && (pageIsHttps || !pageIsLoopback)) {
    return defaultSwapServerUrl();
  }

  return value.replace(/\/+$/, '');
}

function navswapIdempotencyToken(prefix) {
  const random = typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function'
    ? crypto.randomUUID()
    : `${Date.now().toString(36)}-${Math.random().toString(16).slice(2)}`;
  return `${prefix}:${random}`;
}

function withNavswapIdempotency(body, prefix) {
  const payload = body && typeof body === 'object' && !Array.isArray(body) ? { ...body } : {};
  if (!payload.idempotency_key && !payload.idempotencyKey) {
    payload.idempotency_key = navswapIdempotencyToken(prefix);
  }
  return payload;
}

function assertNotShieldedValueMovingRoute(body, label) {
  if (body?.route === SHIELDED_NAVSWAP_ROUTE || body?.route_id === SHIELDED_NAVSWAP_ROUTE) {
    throw new Error(`Shielded NAVSwap ${label} is disabled until the Step 7 private swap submit gate`);
  }
}
