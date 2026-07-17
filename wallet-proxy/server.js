#!/usr/bin/env node
// WebSocket-to-TCP proxy for PostFiat L1 RPC.
// Bridges browser WebSocket connections to the raw TCP JSON-RPC server.

const net = require('net');
const http = require('http');
const crypto = require('crypto');
const zlib = require('zlib');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFile, execFileSync, spawn } = require('child_process');
const { promisify } = require('util');
const { WebSocketServer } = require('ws');
const atomicNavswapModule = require('./navswap-atomic');
const { FastpayCertificateOutbox } = require('./fastpay-certificate-outbox');

const execFileAsync = promisify(execFile);

function compactFastpayVoteRequest(request) {
    if (![
        'owned_sign',
        'owned_unwrap_sign',
        'owned_sign_v3',
        'owned_unwrap_sign_v3',
    ].includes(request?.method)) return request;
    const orderJson = request?.params?.order_json;
    if (typeof orderJson !== 'string' || orderJson.length === 0) return request;
    const compressed = zlib.gzipSync(Buffer.from(orderJson, 'utf8'), { level: 1 }).toString('base64');
    if (compressed.length >= Buffer.byteLength(orderJson, 'utf8')) return request;
    const params = { ...request.params, order_json_gzip_base64: compressed };
    delete params.order_json;
    return { ...request, params };
}

const RPC_HOST = process.env.RPC_HOST || '127.0.0.1';
const RPC_PORT = parseInt(process.env.RPC_PORT || '27650', 10);
const LISTEN_PORT = parseInt(process.env.LISTEN_PORT || '8080', 10);
const LISTEN_HOST = parseListenHost(process.env.LISTEN_HOST);
const ALLOWED_ORIGINS = (process.env.ALLOWED_ORIGINS
    ?? 'http://127.0.0.1:5173,http://localhost:5173')
    .split(',')
    .map((origin) => origin.trim())
    .filter(Boolean);
const WALLET_PROXY_API_TOKEN = (process.env.WALLET_PROXY_API_TOKEN || '').trim();
const WALLET_PROXY_API_TOKENS = parseProxyApiTokens(
    WALLET_PROXY_API_TOKEN,
    process.env.WALLET_PROXY_API_TOKENS_JSON,
    process.env.WALLET_PROXY_API_TOKENS_FILE,
);
const WALLET_STATIC_DIR = path.resolve(
    process.env.WALLET_STATIC_DIR || path.join(__dirname, '..', 'wallet-web', 'dist'),
);
const WALLET_CSP = "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'; script-src 'self' 'wasm-unsafe-eval'; object-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ws://127.0.0.1:8080 ws://localhost:8080 http://127.0.0.1:8789 http://localhost:8789;";
const DEFAULT_RPC_FLEET = [
    'validator-0=127.0.0.1:27650',
    'validator-1=127.0.0.1:27651',
    'validator-2=127.0.0.1:27652',
    'validator-3=127.0.0.1:27653',
    'validator-4=127.0.0.1:27654',
    'validator-5=127.0.0.1:27655',
].join(',');
const RPC_FLEET = parseRpcFleet(process.env.RPC_FLEET || DEFAULT_RPC_FLEET);
const ENABLE_PROPOSER_ROUTING = process.env.ENABLE_PROPOSER_ROUTING !== 'false';

function parseListenHost(value) {
    const host = (value || '127.0.0.1').trim();
    if (net.isIP(host) === 0) {
        throw new Error('LISTEN_HOST must be an explicit IPv4 or IPv6 address');
    }
    return host;
}

function isLoopbackHost(host) {
    return host === '127.0.0.1' || host === '::1';
}

function parseProxyApiTokens(singleToken, tokensJson, tokensFile) {
    const single = String(singleToken || '').trim();
    const inlineJson = String(tokensJson || '').trim();
    const filePath = String(tokensFile || '').trim();
    const configuredSources = [single, inlineJson, filePath].filter(Boolean).length;
    if (configuredSources > 1) {
        throw new Error(
            'configure exactly one wallet proxy token source: single token, JSON, or token file',
        );
    }
    let encoded = inlineJson;
    if (filePath) {
        const stat = fs.statSync(filePath);
        if (!stat.isFile() || stat.size === 0 || stat.size > 64 * 1024) {
            throw new Error('WALLET_PROXY_API_TOKENS_FILE must be a nonempty regular file no larger than 64 KiB');
        }
        encoded = fs.readFileSync(filePath, 'utf8').trim();
    }
    let entries = [];
    if (encoded) {
        let parsed;
        try {
            parsed = JSON.parse(encoded);
        } catch (_) {
            throw new Error('WALLET_PROXY_API_TOKENS_JSON must be a JSON object');
        }
        if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
            throw new Error('WALLET_PROXY_API_TOKENS_JSON must be a JSON object');
        }
        entries = Object.entries(parsed);
    } else if (single) {
        entries = [['default', single]];
    }
    const principals = new Map();
    const tokenValues = new Set();
    for (const [rawPrincipal, rawToken] of entries) {
        const principal = String(rawPrincipal || '').trim();
        const token = String(rawToken || '').trim();
        if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(principal)) {
            throw new Error('wallet proxy principal IDs must be 1-64 safe characters');
        }
        if (Buffer.byteLength(token, 'utf8') < 32) {
            throw new Error(`wallet proxy token for principal ${principal} must contain at least 32 bytes`);
        }
        if (tokenValues.has(token)) {
            throw new Error('wallet proxy tokens must be unique per principal');
        }
        principals.set(principal, token);
        tokenValues.add(token);
    }
    return principals;
}

function validateProxyExposureConfig(host, allowedOrigins, apiTokens) {
    const tokenCount = apiTokens instanceof Map
        ? apiTokens.size
        : (String(apiTokens || '').trim() ? 1 : 0);
    if (!(apiTokens instanceof Map) && tokenCount > 0
        && Buffer.byteLength(String(apiTokens), 'utf8') < 32) {
        throw new Error('WALLET_PROXY_API_TOKEN must contain at least 32 bytes');
    }
    if (!isLoopbackHost(host) && (tokenCount === 0 || allowedOrigins.length === 0)) {
        throw new Error(
            'non-loopback LISTEN_HOST requires wallet proxy API tokens and an explicit ALLOWED_ORIGINS allowlist',
        );
    }
}

validateProxyExposureConfig(LISTEN_HOST, ALLOWED_ORIGINS, WALLET_PROXY_API_TOKENS);

function constantTimeTokenEqual(candidate, expected) {
    if (!candidate || !expected) return false;
    const candidateBytes = Buffer.from(String(candidate), 'utf8');
    const expectedBytes = Buffer.from(String(expected), 'utf8');
    return candidateBytes.length === expectedBytes.length
        && crypto.timingSafeEqual(candidateBytes, expectedBytes);
}

function authenticateProxyToken(candidate, principals = WALLET_PROXY_API_TOKENS) {
    if (!candidate || Buffer.byteLength(String(candidate), 'utf8') > 4096) return null;
    let authenticatedPrincipal = null;
    for (const [principal, expected] of principals.entries()) {
        if (constantTimeTokenEqual(candidate, expected)) authenticatedPrincipal = principal;
    }
    return authenticatedPrincipal;
}

function requestBearerToken(req) {
    const authorization = String(req?.headers?.authorization || '');
    const match = authorization.match(/^Bearer ([^\s]+)$/);
    return match ? match[1] : '';
}

function httpMutationPrincipal(req) {
    return authenticateProxyToken(requestBearerToken(req));
}

function httpMutationAuthorized(req) {
    return httpMutationPrincipal(req) !== null;
}

const PUBLIC_READ_ONLY_POST_PATHS = new Set([
    '/api/shielded-nav-swap/quote',
    '/api/shielded-nav-swap/preflight',
    '/api/navswap/planner-inputs',
    '/api/navswap/quotes',
    '/api/navswap/readiness',
    '/api/navswap/actions/prepare',
    '/api/navswap/actions/prepare-batch',
]);

function httpRequestRequiresAuth(method, pathname) {
    // POST is fail-closed: new endpoints require authentication until they are
    // deliberately classified as side-effect-free and added to this list.
    return method === 'POST' && !PUBLIC_READ_ONLY_POST_PATHS.has(pathname);
}

const PUBLIC_READ_RPC_METHODS = new Set([
    'status',
    'fee',
    'validators',
    'server_info',
    'account',
    'account_assets',
    'account_lines',
    'account_tx',
    'asset_info',
    'vault_bridge_route',
    'escrow_info',
    'owned_objects',
    'owned_recovery_capabilities',
    'owned_certificate',
    'owned_recovery_status',
    'blocks',
    'receipt',
    'mempool',
    'transfer_fee_quote',
    'asset_fee_quote',
    'escrow_fee_quote',
    'nft_fee_quote',
    'offer_fee_quote',
    'atomic_swap_fee_quote',
    atomicNavswapModule.ATOMIC_QUOTE_METHOD,
]);

const REMOVED_PUBLIC_RPC_METHODS = new Set([
    'wallet_sign_owned_transfer',
    'wallet_sign_owned_unwrap',
]);

function rpcRequestRequiresAuth(method) {
    return !PUBLIC_READ_RPC_METHODS.has(method);
}

function rpcRequestPrincipal(request) {
    return authenticateProxyToken(request?.proxy_auth_token);
}

function rpcRequestAuthorized(request) {
    return rpcRequestPrincipal(request) !== null;
}

function walletStaticContentType(filePath) {
    switch (path.extname(filePath)) {
    case '.html': return 'text/html; charset=utf-8';
    case '.js': return 'text/javascript; charset=utf-8';
    case '.css': return 'text/css; charset=utf-8';
    case '.json': return 'application/json; charset=utf-8';
    case '.svg': return 'image/svg+xml';
    case '.png': return 'image/png';
    case '.ico': return 'image/x-icon';
    case '.wasm': return 'application/wasm';
    default: return 'application/octet-stream';
    }
}

function setWalletSecurityHeaders(res) {
    res.setHeader('Content-Security-Policy', WALLET_CSP);
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('X-Frame-Options', 'DENY');
    res.setHeader('Referrer-Policy', 'no-referrer');
    res.setHeader('Permissions-Policy', 'camera=(), geolocation=(), microphone=()');
}

function walletStaticPathAllowed(relativePath) {
    const normalized = relativePath.replaceAll('\\', '/');
    const segments = normalized.split('/');
    if (segments.some((segment) => segment.startsWith('.'))) return false;
    if (
        normalized === '@vite/client'
        || normalized.startsWith('@vite/')
        || normalized.startsWith('@fs/')
        || normalized.startsWith('src/')
        || normalized.startsWith('node_modules/')
        || normalized.endsWith('.map')
    ) {
        return false;
    }
    return true;
}

function walletStaticAssetIsContentHashed(relativePath) {
    return /^assets\/[A-Za-z0-9_.-]+-[A-Za-z0-9_-]{8,}\.[A-Za-z0-9]+$/.test(relativePath);
}

async function serveWalletStatic(req, res, url, staticDir = WALLET_STATIC_DIR) {
    if (!['GET', 'HEAD'].includes(req.method || 'GET')) return false;
    let pathname;
    try {
        pathname = decodeURIComponent(url.pathname);
    } catch (_) {
        return false;
    }
    if (pathname.startsWith('/api/') || pathname === '/rpc') return false;
    const relativePath = pathname === '/' ? 'index.html' : pathname.replace(/^\/+/, '');
    if (!walletStaticPathAllowed(relativePath)) return false;
    const root = path.resolve(staticDir);
    const filePath = path.resolve(root, relativePath);
    if (filePath !== root && !filePath.startsWith(`${root}${path.sep}`)) return false;

    let stat;
    let canonicalRoot;
    let canonicalFile;
    try {
        [canonicalRoot, canonicalFile] = await Promise.all([
            fs.promises.realpath(root),
            fs.promises.realpath(filePath),
        ]);
        if (
            canonicalFile !== canonicalRoot
            && !canonicalFile.startsWith(`${canonicalRoot}${path.sep}`)
        ) {
            return false;
        }
        stat = await fs.promises.stat(canonicalFile);
    } catch (error) {
        if (error?.code === 'ENOENT') return false;
        throw error;
    }
    if (!stat.isFile()) return false;

    setWalletSecurityHeaders(res);
    res.statusCode = 200;
    res.setHeader('Content-Type', walletStaticContentType(filePath));
    res.setHeader(
        'Cache-Control',
        walletStaticAssetIsContentHashed(relativePath)
            ? 'public, max-age=31536000, immutable'
            : 'no-store',
    );
    res.setHeader('Content-Length', stat.size);
    if (req.method === 'HEAD') {
        res.end();
    } else {
        res.end(await fs.promises.readFile(canonicalFile));
    }
    return true;
}

// Capability injection: the wallet-facing proxy narrows the upstream WAN RPC
// surface to proposer-routed finality submits plus controlled-devnet FastPay
// broadcasts. Keep the exposed rate limits aligned with the upstream validator
// RPC limit; overstating them makes wallet/tooling latency runs fail with
// server-side rate-limit errors after clients have already trusted server_info.
const INJECT_RPC_CAPS = process.env.INJECT_RPC_CAPS !== 'false';
const RPC_CAPS = {
    read_only: false,
    mempool_submit_enabled: false,
    mempool_submit_finality_enabled: true,
    mempool_submit_asset_transaction_finality_enabled: true,
    mempool_submit_escrow_transaction_finality_enabled: true,
    atomic_swap_fee_quote_enabled: true,
    mempool_submit_atomic_swap_finality_enabled: true,
    mempool_submit_atomic_swap_enabled: false,
    fastpay_bridge_enabled: true,
    fastpay_bridge_mode: 'proxy_broadcast_devnet',
    fastpay_owned_apply_broadcast_enabled: true,
    max_mempool_submit_per_peer: 16,
    max_mempool_submit_total: 64,
    mempool_submit_rate_limit_window_secs: 60,
};

const PFUSDC_ASSET_ID = process.env.PFUSDC_ASSET_ID
    || '8751c2d04b993eb54f751b0f130c420fdb089548ec2f2a53837d11d1c397a1252e74bcc24616527e9c79b968635fae90';
const VAULT_BRIDGE_RELAY_SCHEMA = 'postfiat-vault-bridge-relay-v1';
const VAULT_BRIDGE_RELAY_SOURCE_RPC_URL = process.env.VAULT_BRIDGE_SOURCE_RPC_URL
    || 'https://arb1.arbitrum.io/rpc';
// Money-destination fields are deliberately absent here. The relay resolves
// chain ID, vault/token addresses and runtime hashes, policy hash, route epoch,
// and route binding from `vault_bridge_route` replicated state for each relay.
// Environment variables are transport configuration, never route authority.
const VAULT_BRIDGE_RELAY_SOURCE_CHAIN_ID = 0;
const VAULT_BRIDGE_RELAY_VAULT_ADDRESS = '';
const VAULT_BRIDGE_RELAY_TOKEN_ADDRESS = '';
const VAULT_BRIDGE_RELAY_POLICY_HASH = '';
const VAULT_BRIDGE_RELAY_DEFAULT_ACCOUNT = process.env.VAULT_BRIDGE_RELAY_ACCOUNT
    || 'pf65c9783ceafc0f519a74195e78cc7909f92429c3';
const VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT = Number.parseInt(
    process.env.VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT || '1000000',
    10,
);
const VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT = Number.parseInt(
    process.env.VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT || '10',
    10,
);
const VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS = (() => {
    try {
        const value = BigInt(process.env.VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS || '1000000');
        return value > 0n ? value : 1000000n;
    } catch (_) {
        return 1000000n;
    }
})();
const A651_ASSET_ID = process.env.A651_ASSET_ID
    || 'dcddbf56e7e15f7893d0038e8e0e6089d5a41418dead75353aabb8c016cf626beeb93bc802929f29883c078d910f59d5';
const A652_ASSET_ID = process.env.A652_ASSET_ID || '';
const LEGACY_A651_ETH_TOKEN = '0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e';
const LEGACY_A651_UNISWAP_POOL_ID = '0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84';
const ETHEREUM_USDC_TOKEN = '0xA0b86991c6218b36c1d19D4A2e9Eb0cE3606eB48';
const NAVSWAP_MAX_LIVE_USD = Number.parseFloat(process.env.NAVSWAP_MAX_LIVE_USD || '100');
const NAVSWAP_CAPABILITIES_SCHEMA = 'postfiat-navswap-capabilities-v1';
const NAVSWAP_QUOTE_SCHEMA = 'postfiat-navswap-quote-v1';
const NAVSWAP_RUN_SCHEMA = 'postfiat-navswap-run-v1';
const NAVSWAP_NAV_PROOF_SCHEMA = 'postfiat-navswap-nav-proof-v1';
const NAVSWAP_RUN_STATUS_SCHEMA = 'postfiat-navswap-run-status-v1';
const NAVSWAP_RUN_LIST_SCHEMA = 'postfiat-navswap-run-list-v1';
const NAVSWAP_RUN_EVENTS_SCHEMA = 'postfiat-navswap-run-events-v1';
const NAVSWAP_RUN_RECEIPTS_SCHEMA = 'postfiat-navswap-run-receipts-v1';
const NAVSWAP_RUN_STREAM_SCHEMA = 'postfiat-navswap-run-stream-v1';
const NAVSWAP_RUN_STREAM_EVENT_SCHEMA = 'postfiat-navswap-run-stream-event-v1';
const NAVSWAP_RUN_STORE_SCHEMA = 'postfiat-navswap-run-store-v1';
const NAVSWAP_IDEMPOTENCY_STORE_SCHEMA = 'postfiat-navswap-idempotency-store-v2';
const NAVSWAP_READINESS_SCHEMA = 'postfiat-navswap-readiness-v1';
const NAVSWAP_DEVNET_FUNDING_SCHEMA = 'postfiat-navswap-devnet-funding-v1';
const NAVSWAP_WALLET_ACTION_SCHEMA = 'postfiat-navswap-wallet-action-request-v1';
const NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA = 'postfiat-navswap-wallet-action-prepare-v1';
const NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA = 'postfiat-navswap-wallet-action-batch-prepare-v1';
const NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA = 'postfiat-navswap-transparent-planner-inputs-v1';
const NAVSWAP_STAKEHUB_TRANSPARENT_ACTION = 'transparent_roundtrip';
const NAVSWAP_ROUTE_TRUST_CLASSES = new Set(['CONTROLLED', 'OPTIMISTIC', 'TRUSTLESS_FINALITY', 'DISABLED']);
const NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY = 'primary_pftl_mint';
const SHIELDED_NAVSWAP_STATUS_SCHEMA = 'postfiat-shielded-navswap-status-v1';
const SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA = 'postfiat-shielded-navswap-ingress-preflight-v1';
const SHIELDED_NAVSWAP_INGRESS_SCHEMA = 'postfiat-shielded-navswap-ingress-v1';
const SHIELDED_NAVSWAP_QUOTE_SCHEMA = 'postfiat-shielded-navswap-quote-v1';
const SHIELDED_NAVSWAP_SWAP_SCHEMA = 'postfiat-shielded-navswap-swap-v1';
const SHIELDED_NAVSWAP_EGRESS_SCHEMA = 'postfiat-shielded-navswap-egress-v1';
const SHIELDED_NAVSWAP_LIQUIDITY_MODES = new Set([
    'bilateral_rfq',
    'operator_inventory',
    'pool_managed_note',
    'issuer_reserve_source',
]);
const ASSET_ORCHARD_INGRESS_FILE_SCHEMA = 'postfiat-asset-orchard-ingress-file-v2';
const ASSET_ORCHARD_SWAP_ACTION_SCHEMA = 'postfiat-asset-orchard-swap-action-v1';
const ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA = 'postfiat-asset-orchard-private-egress-file-v1';
const ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA = 'postfiat-asset-orchard-private-egress-action-v1';
const ASSET_ORCHARD_POOL_ID = 'asset-orchard-v1';
const DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_URL = 'http://127.0.0.1:8789';
const DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS = 1500;
const SHIELDED_NAVSWAP_EGRESS_POLICY_ID = process.env.NAVSWAP_SHIELDED_EGRESS_POLICY_ID
    || 'wallet_private_egress_public_exit_v1';
const SHIELDED_ROUND_TIMEOUT_DEFAULT_MS = 2_400_000;
const VAULT_BRIDGE_BUCKET_STATUS_ACTIVE = 'active';
const VAULT_BRIDGE_RECEIPT_STATUS_COUNTED = 'counted';
const VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY = 'vault_bridge_supply';
const VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION = 'nav_subscription';
const NAVSWAP_RUN_STORE_DEFAULT_PATH = path.join(
    os.homedir(),
    '.local',
    'share',
    'postfiat',
    'wallet-proxy',
    'navswap-runs.jsonl',
);
const NAVSWAP_IDEMPOTENCY_STORE_DEFAULT_PATH = path.join(
    os.homedir(),
    '.local',
    'share',
    'postfiat',
    'wallet-proxy',
    'navswap-idempotency.jsonl',
);

// S3.1: Limit WS message size to 1 MB (RPC max is 8 MB, but wallet requests are small)
const MAX_WS_MESSAGE_BYTES = 1024 * 1024;
const MAX_HTTP_BODY_BYTES = parseBoundedPositiveIntegerEnv(
    'WALLET_PROXY_MAX_HTTP_BODY_BYTES',
    16 * 1024 * 1024,
    16 * 1024 * 1024,
);
const MUTATION_RATE_LIMIT = parseBoundedPositiveIntegerEnv(
    'WALLET_PROXY_MUTATION_RATE_LIMIT',
    120,
    100_000,
);
const MUTATION_RATE_WINDOW_MS = parseBoundedPositiveIntegerEnv(
    'WALLET_PROXY_MUTATION_RATE_WINDOW_MS',
    60_000,
    86_400_000,
);
const MUTATION_CONCURRENCY = parseBoundedPositiveIntegerEnv(
    'WALLET_PROXY_MUTATION_CONCURRENCY',
    16,
    4096,
);
const mutationAdmissionByPrincipal = new Map();
let activeMutationAdmissions = 0;

function parseBoundedPositiveIntegerEnv(name, fallback, maximum) {
    const raw = process.env[name];
    if (raw === undefined || String(raw).trim() === '') return fallback;
    const value = Number.parseInt(String(raw), 10);
    if (!Number.isSafeInteger(value) || value <= 0 || value > maximum) {
        throw new Error(`${name} must be an integer between 1 and ${maximum}`);
    }
    return value;
}

function boundedHttpBodyLimit(routeLimit) {
    const requested = Number.isSafeInteger(routeLimit) && routeLimit > 0
        ? routeLimit
        : MAX_HTTP_BODY_BYTES;
    return Math.min(requested, MAX_HTTP_BODY_BYTES);
}

function acquireMutationAdmission(principalId, nowMs = Date.now()) {
    const principal = String(principalId || '').trim();
    if (!principal) {
        return { ok: false, code: 'proxy_auth_required', retry_after_ms: 0 };
    }
    let window = mutationAdmissionByPrincipal.get(principal);
    if (!window || nowMs - window.started_at_ms >= MUTATION_RATE_WINDOW_MS) {
        window = { started_at_ms: nowMs, count: 0 };
        mutationAdmissionByPrincipal.set(principal, window);
    }
    if (window.count >= MUTATION_RATE_LIMIT) {
        return {
            ok: false,
            code: 'proxy_mutation_rate_limited',
            retry_after_ms: Math.max(1, MUTATION_RATE_WINDOW_MS - (nowMs - window.started_at_ms)),
        };
    }
    if (activeMutationAdmissions >= MUTATION_CONCURRENCY) {
        return {
            ok: false,
            code: 'proxy_mutation_concurrency_limited',
            retry_after_ms: 100,
        };
    }
    window.count += 1;
    activeMutationAdmissions += 1;
    let released = false;
    return {
        ok: true,
        principal_id: principal,
        release() {
            if (released) return;
            released = true;
            activeMutationAdmissions = Math.max(0, activeMutationAdmissions - 1);
        },
    };
}

function clearMutationAdmissionForTest() {
    mutationAdmissionByPrincipal.clear();
    activeMutationAdmissions = 0;
}
// S3.4: TCP connection timeout
const TCP_TIMEOUT_MS = 30000;
const PROPOSER_ROUTE_TIMEOUT_MS = parseInt(process.env.PROPOSER_ROUTE_TIMEOUT_MS || '10000', 10);
const PROPOSER_ROUTE_RETRY_MS = parseInt(process.env.PROPOSER_ROUTE_RETRY_MS || '250', 10);
const PROPOSER_ROUTE_CACHE_MS = parseInt(process.env.PROPOSER_ROUTE_CACHE_MS || '10000', 10);
const PROPOSER_READY_RETRY_MS = parseInt(process.env.PROPOSER_READY_RETRY_MS || '50', 10);
const ENABLE_FIRST_READY_SEQUENCED_READ = process.env.ENABLE_FIRST_READY_SEQUENCED_READ !== 'false';
const ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE = process.env.ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE !== 'false';
const OPTIMISTIC_CACHED_FINALITY_ROUTE = process.env.OPTIMISTIC_CACHED_FINALITY_ROUTE === 'true';
const ENABLE_FINALITY_RESPONDER_READ_CACHE = process.env.ENABLE_FINALITY_RESPONDER_READ_CACHE === 'true';
const ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE = process.env.ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE !== 'false';
const PREFERRED_SEQUENCED_READ_VALIDATORS = (process.env.PREFERRED_SEQUENCED_READ_VALIDATORS || 'validator-2,validator-5')
    .split(',')
    .map((part) => part.trim())
    .filter(Boolean);
const FIRST_READY_SEQUENCED_READ_PROPOSERS = new Set(
    (process.env.FIRST_READY_SEQUENCED_READ_PROPOSERS || '')
        .split(',')
        .map((part) => part.trim())
        .filter(Boolean),
);
const FASTPAY_FLEET_STATUS_CACHE_MS = parseInt(process.env.FASTPAY_FLEET_STATUS_CACHE_MS || '10000', 10);
const FASTPAY_ROUTE_WARMUP_ENABLED = process.env.FASTPAY_ROUTE_WARMUP_ENABLED !== 'false';
const FASTPAY_ROUTE_REFRESH_MS = parseInt(
    process.env.FASTPAY_ROUTE_REFRESH_MS
        || String(Math.max(250, Math.floor(FASTPAY_FLEET_STATUS_CACHE_MS / 2))),
    10,
);
const FASTPAY_ROUTE_TIMEOUT_MS = parseInt(process.env.FASTPAY_ROUTE_TIMEOUT_MS || String(PROPOSER_ROUTE_TIMEOUT_MS), 10);
const FASTPAY_ROUTE_RETRY_MS = parseInt(process.env.FASTPAY_ROUTE_RETRY_MS || String(PROPOSER_READY_RETRY_MS), 10);
const FASTPAY_REQUIRE_PRIMARY_SUCCESS = process.env.FASTPAY_REQUIRE_PRIMARY_SUCCESS === 'true';
const ENABLE_UPSTREAM_KEEPALIVE = process.env.ENABLE_UPSTREAM_KEEPALIVE !== 'false';
const FASTPAY_CERTIFICATE_FINALITY_ENABLED = process.env.FASTPAY_CERTIFICATE_FINALITY_ENABLED !== 'false';
const FASTPAY_CERTIFICATE_RETRY_MS = parseInt(process.env.FASTPAY_CERTIFICATE_RETRY_MS || '2000', 10);
const FASTPAY_CERTIFICATE_OUTBOX_PATH = process.env.FASTPAY_CERTIFICATE_OUTBOX_PATH
    || path.join(os.homedir(), '.postfiat', 'wallet-proxy', 'fastpay-certificate-outbox.json');
const fastpayCertificateOutbox = new FastpayCertificateOutbox(FASTPAY_CERTIFICATE_OUTBOX_PATH);
const WALLET_SUBSCRIPTION_INTERVAL_MS = parseInt(process.env.WALLET_SUBSCRIPTION_INTERVAL_MS || '1500', 10);
const WALLET_SUBSCRIPTION_MIN_INTERVAL_MS = parseInt(process.env.WALLET_SUBSCRIPTION_MIN_INTERVAL_MS || '750', 10);
const WALLET_SUBSCRIPTION_READ_TIMEOUT_MS = parseInt(process.env.WALLET_SUBSCRIPTION_READ_TIMEOUT_MS || '5000', 10);
const FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT = 2048;
const NAVSWAP_IDEMPOTENCY_TTL_MS = parseInt(process.env.NAVSWAP_IDEMPOTENCY_TTL_MS || '86400000', 10);
const NAVSWAP_QUOTE_FRESHNESS_TTL_MS = parseInt(process.env.NAVSWAP_QUOTE_FRESHNESS_TTL_MS || '300000', 10);
const NAVSWAP_SETTLEMENT_RECEIPT_MAX_SNAPSHOT_AGE_BLOCKS = parseInt(
    process.env.NAVSWAP_SETTLEMENT_RECEIPT_MAX_SNAPSHOT_AGE_BLOCKS || '100',
    10,
);
const NAVSWAP_SETTLEMENT_RECEIPT_SAFETY_BLOCKS = parseInt(
    process.env.NAVSWAP_SETTLEMENT_RECEIPT_SAFETY_BLOCKS || '5',
    10,
);
// S3.4: Max concurrent TCP connections per WS client. Dev React hydration can
// briefly duplicate read effects; keep the bound explicit without stalling the
// first user action behind startup reads.
const MAX_TCP_PER_WS = parseInt(process.env.MAX_TCP_PER_WS || '32', 10);

const FINALITY_METHODS = new Set([
    'mempool_submit_signed_transfer_finality',
    'mempool_submit_signed_payment_v2_finality',
    'mempool_submit_signed_asset_transaction_finality',
    'mempool_submit_signed_escrow_transaction_finality',
    'mempool_submit_fastlane_primary_finality',
    atomicNavswapModule.ATOMIC_FINALITY_METHOD,
]);

const SEQUENCED_ACCOUNT_METHODS = new Set([
    'transfer_fee_quote',
    'asset_fee_quote',
    'escrow_fee_quote',
    'nft_fee_quote',
    'offer_fee_quote',
    atomicNavswapModule.ATOMIC_QUOTE_METHOD,
    'account',
    'account_assets',
    'account_lines',
]);

const FASTPAY_BROADCAST_METHODS = new Set([
    'owned_apply',
    'owned_unwrap_apply',
    'owned_apply_v3',
    'owned_unwrap_apply_v3',
]);

let proposerRouteCache = null;
let latestFinalizedReadCache = null;
let fastpayFleetStatusCache = null;
let fastpayFleetStatusInFlight = null;
let preferredSequencedReadIndex = 0;
const navswapRuns = new Map();
const navswapRunStreams = new Map();
const navswapDevnetFundingUsage = new Map();
const navswapIdempotencyRecords = new Map();

function parseRpcFleet(value) {
    return value.split(',')
        .map((part) => part.trim())
        .filter(Boolean)
        .map((part) => {
            const eq = part.indexOf('=');
            if (eq <= 0) throw new Error(`invalid RPC_FLEET entry: ${part}`);
            const validatorId = part.slice(0, eq);
            const endpoint = part.slice(eq + 1);
            const colon = endpoint.lastIndexOf(':');
            if (colon <= 0) throw new Error(`invalid RPC_FLEET endpoint: ${part}`);
            const host = endpoint.slice(0, colon);
            const port = Number.parseInt(endpoint.slice(colon + 1), 10);
            if (!host || !Number.isInteger(port) || port <= 0 || port > 65535) {
                throw new Error(`invalid RPC_FLEET endpoint: ${part}`);
            }
            return { validatorId, host, port };
        });
}

const walletProxyRuntime = { A651_ASSET_ID,A652_ASSET_ID,ALLOWED_ORIGINS,ASSET_ORCHARD_INGRESS_FILE_SCHEMA,ASSET_ORCHARD_POOL_ID,ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA,ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA,ASSET_ORCHARD_SWAP_ACTION_SCHEMA,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_URL,DEFAULT_RPC_FLEET,ENABLE_FINALITY_RESPONDER_READ_CACHE,ENABLE_FIRST_READY_SEQUENCED_READ,ENABLE_PROPOSER_ROUTING,ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE,ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE,ENABLE_UPSTREAM_KEEPALIVE,ETHEREUM_USDC_TOKEN,FASTPAY_BROADCAST_METHODS,FASTPAY_CERTIFICATE_FINALITY_ENABLED,FASTPAY_CERTIFICATE_RETRY_MS,FASTPAY_FLEET_STATUS_CACHE_MS,FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,FASTPAY_REQUIRE_PRIMARY_SUCCESS,FASTPAY_ROUTE_RETRY_MS,FASTPAY_ROUTE_TIMEOUT_MS,FINALITY_METHODS,FIRST_READY_SEQUENCED_READ_PROPOSERS,INJECT_RPC_CAPS,LEGACY_A651_ETH_TOKEN,LEGACY_A651_UNISWAP_POOL_ID,LISTEN_PORT,MAX_TCP_PER_WS,MAX_WS_MESSAGE_BYTES,NAVSWAP_CAPABILITIES_SCHEMA,NAVSWAP_DEVNET_FUNDING_SCHEMA,NAVSWAP_IDEMPOTENCY_STORE_DEFAULT_PATH,NAVSWAP_IDEMPOTENCY_STORE_SCHEMA,NAVSWAP_IDEMPOTENCY_TTL_MS,NAVSWAP_MAX_LIVE_USD,NAVSWAP_NAV_PROOF_SCHEMA,NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY,NAVSWAP_QUOTE_FRESHNESS_TTL_MS,NAVSWAP_QUOTE_SCHEMA,NAVSWAP_READINESS_SCHEMA,NAVSWAP_ROUTE_TRUST_CLASSES,NAVSWAP_RUN_EVENTS_SCHEMA,NAVSWAP_RUN_LIST_SCHEMA,NAVSWAP_RUN_RECEIPTS_SCHEMA,NAVSWAP_RUN_SCHEMA,NAVSWAP_RUN_STATUS_SCHEMA,NAVSWAP_RUN_STORE_DEFAULT_PATH,NAVSWAP_RUN_STORE_SCHEMA,NAVSWAP_RUN_STREAM_EVENT_SCHEMA,NAVSWAP_RUN_STREAM_SCHEMA,NAVSWAP_SETTLEMENT_RECEIPT_MAX_SNAPSHOT_AGE_BLOCKS,NAVSWAP_SETTLEMENT_RECEIPT_SAFETY_BLOCKS,NAVSWAP_STAKEHUB_TRANSPARENT_ACTION,NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA,NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_SCHEMA,OPTIMISTIC_CACHED_FINALITY_ROUTE,PFUSDC_ASSET_ID,PREFERRED_SEQUENCED_READ_VALIDATORS,PROPOSER_READY_RETRY_MS,PROPOSER_ROUTE_CACHE_MS,PROPOSER_ROUTE_RETRY_MS,PROPOSER_ROUTE_TIMEOUT_MS,RPC_CAPS,RPC_FLEET,RPC_HOST,RPC_PORT,SEQUENCED_ACCOUNT_METHODS,SHIELDED_NAVSWAP_EGRESS_POLICY_ID,SHIELDED_NAVSWAP_EGRESS_SCHEMA,SHIELDED_NAVSWAP_INGRESS_SCHEMA,SHIELDED_NAVSWAP_LIQUIDITY_MODES,SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,SHIELDED_NAVSWAP_QUOTE_SCHEMA,SHIELDED_NAVSWAP_STATUS_SCHEMA,SHIELDED_NAVSWAP_SWAP_SCHEMA,SHIELDED_ROUND_TIMEOUT_DEFAULT_MS,TCP_TIMEOUT_MS,VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,VAULT_BRIDGE_BUCKET_STATUS_ACTIVE,VAULT_BRIDGE_RECEIPT_STATUS_COUNTED,VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT,VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS,VAULT_BRIDGE_RELAY_DEFAULT_ACCOUNT,VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT,VAULT_BRIDGE_RELAY_POLICY_HASH,VAULT_BRIDGE_RELAY_SCHEMA,VAULT_BRIDGE_RELAY_SOURCE_CHAIN_ID,VAULT_BRIDGE_RELAY_SOURCE_RPC_URL,VAULT_BRIDGE_RELAY_TOKEN_ADDRESS,VAULT_BRIDGE_RELAY_VAULT_ADDRESS,WALLET_SUBSCRIPTION_INTERVAL_MS,WALLET_SUBSCRIPTION_MIN_INTERVAL_MS,WALLET_SUBSCRIPTION_READ_TIMEOUT_MS,acquireMutationAdmission,boundedHttpBodyLimit,crypto,execFileAsync,fastpayCertificateOutbox,fastpayFleetStatusCache,fastpayFleetStatusInFlight,fs,http,httpMutationAuthorized,httpMutationPrincipal,httpRequestRequiresAuth,latestFinalizedReadCache,navswapDevnetFundingUsage,navswapIdempotencyRecords,navswapRunStreams,navswapRuns,net,os,parseRpcFleet,path,preferredSequencedReadIndex,proposerRouteCache };
Object.assign(walletProxyRuntime, {
    execFile,
    execFileSync,
    spawn,
    FASTPAY_ROUTE_WARMUP_ENABLED,
    FASTPAY_ROUTE_REFRESH_MS,
});
Object.assign(walletProxyRuntime, require('./rpc-routing').create(walletProxyRuntime));
const { UpstreamRpcConnection,addProxyRouteEvent,bftQuorumThreshold,broadcastFastpayMutation,cachedSelection,canonicalReadResult,chooseOwnedVoteEndpoint,chooseProposerEndpointCached,chooseProposerEndpointFromStatuses,chooseProposerEndpointWithRetry,chooseSequencedAccountReadEndpoint,clearFastpayFleetStatusCache,closeUpstreamRpcConnections,collectFastpayFleetStatuses,collectFleetStatuses,conciseRpcError,convergedFleetGroup,deterministicProposer,endpointStatusMeetsRoute,endpointStatusMeetsSequencedReadRoute,fetchWalletSnapshot,firstReadyEndpointForRoute,firstStructuredFastpayResult,invalidateProposerRouteCache,isFastpayBroadcastMethod,isFinalityMethod,isSequencedAccountMethod,normalizeFastpayBroadcastRequest,normalizeWalletSubscriptionParams,preferredSequencedReadEndpoint,primeNextProposerRouteCache,primeNextProposerRouteCacheFromResponse,proposerEndpointForHeight,readFleetRpcMajority,readGroupKey,rememberFinalizedReadEndpoint,requestWithProxyReadiness: requestWithProxyReadinessBase,resolveRpcTarget,responseEnvelope,rpcTcpRequest,rpcTcpRequestLine,rpcTcpRequestOneShotLine,sendWalletNotification,shouldUseFirstReadySequencedRead,sleep,startCachedSelectionReadinessProbe,startFastpayCertificateRecovery,startFastpayRouteWarmup,startWalletSubscription,stopWalletSubscription,upstreamEndpointKey,upstreamRpcConnection,upstreamRpcConnections,waitForCachedSelectionReady,waitForFastpayConvergedGroup,walletSnapshotDigest } = walletProxyRuntime;
const finalityFailureCanAdvanceView = walletProxyRuntime.finalityFailureCanAdvanceView;
const recoverFinalityAcrossViews = walletProxyRuntime.recoverFinalityAcrossViews;
function requestWithProxyReadiness(request, route) {
    if (request?.method === atomicNavswapModule.ATOMIC_FINALITY_METHOD) {
        return request;
    }
    return requestWithProxyReadinessBase(request, route);
}
walletProxyRuntime.requestWithProxyReadiness = requestWithProxyReadiness;
Object.assign(walletProxyRuntime, require('./navswap-config-bridge').create(walletProxyRuntime));
const { SHIELDED_PRIVATE_KEY_PATTERNS,assertNoShieldedPrivateMaterial,assertVaultBridgeEvidenceMatches,assetOrchardLocalServiceConfig,buildUniswapHandoffQuoteBinding,buildVaultBridgeRelayBundle,clearNavswapDevnetFundingUsageForTest,currentA652AssetId,ensureVaultBridgeRecipientAccount,executeNavswapCapabilities,executeVaultBridgeRelay,findShieldedPrivateMaterialPaths,governedVaultBridgeRelayConfig,isBadSequenceSubmitResponse,isReplayableVaultBridgeRelayDuplicate,lower,navswapBridgeConfig,navswapCapabilities,navswapDevnetFundingUsageSnapshot,navswapDevnetFundingWindowUsage,navswapDevnetPfusdcFundingConfig,navswapInferTrustClass,navswapNormalizeTrustClass,navswapRoutePrivacy,navswapStakehubTransparentConfig,navswapTransparentOperatorConfig,navswapTrustlessFinalityAgreement,navswapUniswapBetaRouteState,normalizeShieldedKey,normalizeShieldedLiquidityMode,normalizeVaultBridgeAddress,normalizeVaultBridgeBytes32,normalizeVaultBridgeTxHash,parseUniswapHandoffBytes32,parseUniswapHandoffPositiveInteger,presentEnv,presentPositiveSafeIntegerEnv,readNavswapKeyFileAddress,releaseNavswapDevnetFundingUsage,reserveNavswapDevnetFundingUsage,routedRpcRead,shieldedLiquidityModeLabel,shieldedNavswapEgressConfig,shieldedNavswapIngressConfig,shieldedNavswapQuoteConfig,shieldedNavswapSwapConfig,shieldedQuotePolicyHash,signAndSubmitVaultBridgeRecipientSponsor,signAndSubmitVaultBridgeRelayOperation,vaultBridgeAccountAssets,vaultBridgeBodyTxHash,vaultBridgeEvidenceFromPlan,vaultBridgeExpectedField,vaultBridgePftlAccountExists,vaultBridgeRelayConfig } = walletProxyRuntime;
Object.assign(walletProxyRuntime, require('./navswap-transparent').create(walletProxyRuntime));
const { assetIdForNavswapSymbol,buildNavswapNavProofResponse,buildNavswapQuoteResponse,buildPftlUniswapReceiptVerification,buildStakehubTransparentPreflight,buildTransparentNavswapReceiptVerification,buildTransparentNavswapRedeemReceiptVerification,buildUrl,completePftlUniswapHandoffRun,completeTransparentNavswapRun,executeNavswapDevnetPfusdcFunding,executeNavswapQuote,executePftlUniswapHandoffRun,executePftlUniswapWalletQuote,executeTransparentNavswapQuote,executeTransparentNavswapReadiness,executeTransparentNavswapRun,fetchJsonWithTimeout,isIssuedAsset,isPftAsset,loadPftlUniswapWalletActionContext,navswapAccountAssetItems,navswapAccountBalanceAtoms,navswapActionAutoPlanRequested,navswapActionPrepareError,navswapAllocationRemainingAtoms,navswapAssetInfoAsset,navswapAssetInfoIssuer,navswapAssetIssuer,navswapAssetPrecision,navswapCompletionConsumerIds,navswapCompletionOperationTemplate,navswapCompletionSubmittedChainId,navswapCompletionSubmittedSequence,navswapConsumerMatchesRecipient,navswapDecimalAmountToAtoms,navswapFreshnessFromBody,navswapFreshnessPayload,navswapHashHexDomain,navswapNativeAccountBalanceAtoms,navswapNavProofStub,navswapNavRedemptionId,navswapPftlUniswapControlledAttestationTxHash,navswapPftlUniswapDefaultDeadlineSeconds,navswapPftlUniswapDefaultEthereumRecipient,navswapPftlUniswapDefaultRefundDelayBlocks,navswapPftlUniswapDestinationHeights,navswapPftlUniswapPacketHash,navswapPftlUniswapRouteRow,navswapPlannerCurrentHeight,navswapPlannerError,navswapPlannerNumber,navswapPlannerPositiveNumber,navswapPlannerRemainingAtoms,navswapPrimaryMintIntentFields,navswapProofIsFresh,navswapRandomHex,navswapReceiptFreshness,navswapRedeemCompletionOperationTemplate,navswapRequiredVaultBridgeSettlementAtoms,navswapRouteFromBody,navswapRpcRead,navswapSafeU64Number,navswapSettlementReceiptFreshnessConfig,navswapSettlementReceiptHash,navswapSubscriptionId,navswapValuationUnitScale,navswapWalletActionBatchItems,navswapWalletActionId,normalizePftlUniswapPacketStatus,parseAtomicInteger,parseNavswapActionInteger,parseNavswapDisplayOrAtomAmount,parseNavswapEvmAddress,parseNavswapHexId,parseNavswapWalletAddress,parseStakehubTransparentAmount,pftlUniswapCompletionError,pftlUniswapCompletionQuote,pftlUniswapPreparedAction,planTransparentNavswapWalletActions,preflightNavswapPreparedActionFees,prepareNavswapWalletAction,prepareNavswapWalletActionBatch,prepareNavswapWalletNavRedeemAtNavAction,prepareNavswapWalletNavSubscriptionAllocateAction,preparePftlUniswapWalletActionBatch,selectNavswapIssuedSettlementSource,selectTransparentRedeemSettlementAllocation,signAndSubmitNavswapOperatorAssetTransaction,stakehubTransparentAmountError,transparentCompletionError,transparentCompletionPreparedAction,transparentCompletionQuote,transparentCompletionStage,transparentCompletionSubmission,transparentCompletionWalletResult,validateNavswapPlannerMarketStatus,verifyPftlUniswapExportPacket,verifyPftlUniswapWalletCompletionInput,verifyTransparentNavRedeemSettlement,verifyTransparentNavSubscriptionAllocation,verifyTransparentWalletCompletionInput } = walletProxyRuntime;
Object.assign(walletProxyRuntime, atomicNavswapModule.create(walletProxyRuntime));
const { atomicRpcProxyError,executeAtomicNavswapQuote,executeAtomicNavswapRun,executeNavswapCapabilitiesWithAtomic,executeNavswapQuoteWithAtomic,executeTransparentNavswapRunWithAtomic } = walletProxyRuntime;
walletProxyRuntime.executeNavswapCapabilities = executeNavswapCapabilitiesWithAtomic;
walletProxyRuntime.executeNavswapQuote = executeNavswapQuoteWithAtomic;
walletProxyRuntime.executeTransparentNavswapRun = executeTransparentNavswapRunWithAtomic;
Object.assign(walletProxyRuntime, require('./navswap-shielded').create(walletProxyRuntime));
const { ASSET_ORCHARD_ACTION_CLEAR_KEYS,buildShieldedCertifiedRoundArgs,certifiedRoundFailure,certifiedRoundHasQuorumCertificate,certifiedRoundHeight,certifiedRoundReceipts,certifyShieldedBatchViaWarmLoop,chooseShieldedCatchUpSource,cloneJson,collectShieldedTopologyStatuses,createShieldedSwapBatchViaLocalService,executeShieldedNavswapBalances,executeShieldedNavswapEgress,executeShieldedNavswapIngress,executeShieldedNavswapIngressPreflight,executeShieldedNavswapNoteCapability,executeShieldedNavswapProverReadiness,executeShieldedNavswapQuote,executeShieldedNavswapStatus,executeShieldedNavswapSwap,fileMtimeUnixMs,findAssetOrchardActionCleartext,loadShieldedTopologyPeers,majorityRootAtHeight,maxMtimeUnixMs,msSpan,navswapIdempotencyKeyFromRequest,navswapIdempotencyStorePath,navswapRunStorePath,navswapStableJson,navswapValidateIdempotencyKey,newNavswapRunId,parseShieldedPrivateEgressJson,parseShieldedSwapActionJson,runShieldedLaggardCatchUp,runShieldedRpcCatchUp,shellQuote,shieldedBatchExplicitActionIds,shieldedCatchUpLaggards,shieldedCatchUpSourceCandidates,shieldedCertifiedRoundEnv,shieldedCertifierLoopBatchFile,shieldedCertifierLoopStartHeight,shieldedCertifierLoopState,shieldedConvergenceSummary,shieldedEarlyQuorumEnabled,shieldedIngressSupportedAsset,shieldedLaggardCatchUpConfig,shieldedPrivateEgressDisclosureFields,shieldedPrivateEgressDisclosureHash,shieldedQuoteAssetByInput,shieldedQuoteFromSubmitBody,shieldedQuotePairEnabled,shieldedRemoteDataDir,shieldedRemoteWorkDir,shieldedRoundBatchIds,shieldedRoundPhaseTimings,shieldedRoundReceiptIds,shieldedSwapProxyTimingReport,startShieldedCertifierLoop,validateShieldedCertifierLoopReportForBatch,validateShieldedEgressSubmit,validateShieldedIngressPayload,validateShieldedPrivateEgressFile,validateShieldedSwapAction,validateShieldedSwapSubmit } = walletProxyRuntime;
Object.assign(walletProxyRuntime, require('./navswap-persistence-http').create(walletProxyRuntime));
const { annotateNavswapIdempotency,buildNavswapRunResponse,clearNavswapIdempotencyForTest,clearNavswapRunsForTest,compareNavswapRunsNewestFirst,createNavswapRun,executeNavswapAtomicTemplate,executeNavswapIdempotentRequest,executeNavswapRun,finishNavswapRun,forwardStakehubTransparentRun,handleNavswapHttp,jsonHeaders,loadNavswapIdempotencyStore,loadNavswapRunStore,markStoredNavswapRunInterrupted,navswapAsyncRunRequested,navswapIdempotencyHashBody,navswapIdempotencyStoreSnapshot,navswapListLimit,navswapRunEvents,navswapRunIsTerminal,navswapRunList,navswapRunPublic,navswapRunReceipts,navswapRunSortTime,navswapRunStoreSnapshot,navswapRunStreamSnapshot,navswapTruthyParam,normalizeAtomicTemplateParams,normalizeStoredNavswapIdempotencyRecord,normalizeStoredNavswapRun,originAllowed,persistNavswapIdempotencyRecord,persistNavswapRun,pruneNavswapIdempotencyRecords,publishNavswapRunUpdate,readJsonBody,recordNavswapRunEvent,removeNavswapRunStreamSubscriber,sanitizeNavswapRunRequest,sendJson,sendNavswapRunStream,sseHeaders,swapAtomicTemplateParams,verifyAtomicTemplateResult,verifyAtomicTemplateSymmetry,writeSseEvent } = walletProxyRuntime;
const server = http.createServer(async (req, res) => {
    const url = new URL(req.url || '/', `http://${req.headers.host || 'localhost'}`);
    if (await handleNavswapHttp(req, res, url)) return;
    if (await serveWalletStatic(req, res, url)) return;
    if (req.method === 'GET' && url.pathname === '/healthz') {
        sendJson(req, res, 200, { status: 'ok' });
        return;
    }
    sendJson(req, res, 404, { ok: false, error: 'not found' });
});

// Validate browser origins before completing the WebSocket upgrade. Originless
// non-browser clients may connect for reads, but mutation dispatch below still
// requires both an authenticated principal and an allowed browser Origin.
const wss = new WebSocketServer({ noServer: true, maxPayload: MAX_WS_MESSAGE_BYTES });
server.on('upgrade', (req, socket, head) => {
    const origin = req.headers.origin || '';
    if (origin && !ALLOWED_ORIGINS.includes(origin)) {
        socket.write(
            'HTTP/1.1 403 Forbidden\r\n'
            + 'Connection: close\r\n'
            + 'Content-Length: 0\r\n\r\n',
        );
        socket.destroy();
        return;
    }
    wss.handleUpgrade(req, socket, head, (ws) => {
        wss.emit('connection', ws, req);
    });
});

wss.on('connection', (ws, req) => {
    // Browser WebSockets always carry Origin. Non-browser clients may omit it,
    // but all mutation methods still require the per-request bearer below.
    const origin = req.headers.origin || '';
    if (origin && !ALLOWED_ORIGINS.includes(origin)) {
        ws.close(1008, 'origin not allowed');
        return;
    }

    // Track pending upstream requests for this WS client.
    let activeTcpConnections = 0;
    const walletSubscriptions = new Map();

    ws.on('close', () => {
        for (const subscription of walletSubscriptions.values()) {
            stopWalletSubscription(subscription);
        }
        walletSubscriptions.clear();
    });

    ws.on('error', () => {
        // The ws library emits connection-level errors for rejected frames
        // such as maxPayload violations. Treat them as client disconnects
        // instead of letting an unhandled event terminate the proxy.
    });

    ws.on('message', async (data) => {
        const msg = data.toString('utf8');

        // S3.2: Validate JSON before forwarding to TCP
        let parsed;
        try {
            parsed = JSON.parse(msg);
        } catch (e) {
            ws.send(JSON.stringify({
                version: 'postfiat-local-rpc-v1',
                id: 'proxy-error',
                ok: false,
                result: null,
                error: { code: 'proxy_invalid_json', message: 'message is not valid JSON' },
                events: []
            }));
            return;
        }

        // S3.2: Validate required RPC fields
        if (!parsed.version || !parsed.id || !parsed.method) {
            ws.send(JSON.stringify({
                version: 'postfiat-local-rpc-v1',
                id: parsed.id || 'proxy-error',
                ok: false,
                result: null,
                error: { code: 'proxy_invalid_request', message: 'missing required RPC fields' },
                events: []
            }));
            return;
        }

        if (REMOVED_PUBLIC_RPC_METHODS.has(parsed.method)) {
            ws.send(JSON.stringify(responseEnvelope(
                parsed.id,
                false,
                null,
                {
                    code: 'proxy_method_removed',
                    message: 'remote wallet custody signing is not available; sign locally in the wallet',
                },
                [],
            )));
            return;
        }

        if (rpcRequestRequiresAuth(parsed.method) && !origin) {
            ws.send(JSON.stringify(responseEnvelope(
                parsed.id,
                false,
                null,
                {
                    code: 'proxy_origin_required',
                    message: 'browser wallet mutations require an allowed Origin',
                },
                [],
            )));
            return;
        }

        const requiresAuth = rpcRequestRequiresAuth(parsed.method);
        const principalId = requiresAuth ? rpcRequestPrincipal(parsed) : null;
        if (requiresAuth && !principalId) {
            ws.send(JSON.stringify(responseEnvelope(
                parsed.id,
                false,
                null,
                {
                    code: 'proxy_auth_required',
                    message: 'authenticated wallet proxy mutation required',
                },
                [],
            )));
            return;
        }
        if (requiresAuth) {
            const admission = acquireMutationAdmission(principalId);
            if (!admission.ok) {
                ws.send(JSON.stringify(responseEnvelope(
                    parsed.id,
                    false,
                    null,
                    {
                        code: admission.code,
                        message: admission.code === 'proxy_mutation_rate_limited'
                            ? 'authenticated mutation rate limit exceeded'
                            : 'authenticated mutation concurrency limit exceeded',
                        retry_after_ms: admission.retry_after_ms,
                    },
                    [],
                )));
                return;
            }
            // The existing per-WebSocket active-TCP counter holds the routed
            // work slot. Shared admission records this principal's rate and
            // observes process-wide HTTP mutation pressure at admission time.
            admission.release();
        }
        // The proxy credential is a local dispatch capability and must never be
        // forwarded to validators, logged in route evidence, or persisted.
        delete parsed.proxy_auth_token;

        if (parsed.method === 'consensus_v2_timeout_vote') {
            ws.send(JSON.stringify(responseEnvelope(
                parsed.id,
                false,
                null,
                {
                    code: 'proxy_internal_method',
                    message: 'consensus timeout votes are internal to finality recovery',
                },
                [],
            )));
            return;
        }

        const atomicProxyError = atomicRpcProxyError(parsed);
        if (atomicProxyError) {
            ws.send(JSON.stringify(responseEnvelope(
                parsed.id,
                false,
                null,
                atomicProxyError,
                [],
            )));
            return;
        }

        if (parsed.method === 'wallet_subscribe') {
            try {
                const subscription = startWalletSubscription(ws, parsed, walletSubscriptions);
                ws.send(JSON.stringify(responseEnvelope(
                    parsed.id,
                    true,
                    {
                        schema: 'postfiat-wallet-subscription-v1',
                        subscription_id: subscription.id,
                        interval_ms: subscription.params.interval_ms,
                        push_method: 'wallet_update',
                    },
                    null,
                    [{
                        event_type: 'proxy_wallet_feed',
                        subject: subscription.id,
                        message: 'wallet feed subscription started',
                    }],
                )));
            } catch (error) {
                ws.send(JSON.stringify(responseEnvelope(
                    parsed.id,
                    false,
                    null,
                    {
                        code: 'proxy_wallet_subscribe_failed',
                        message: error?.message || 'could not start wallet feed',
                    },
                    [],
                )));
            }
            return;
        }

        if (parsed.method === 'wallet_unsubscribe') {
            const subscriptionId = parsed.params?.subscription_id;
            const subscription = walletSubscriptions.get(subscriptionId);
            if (subscription) {
                stopWalletSubscription(subscription);
                walletSubscriptions.delete(subscriptionId);
            }
            ws.send(JSON.stringify(responseEnvelope(
                parsed.id,
                true,
                {
                    subscription_id: subscriptionId || null,
                    unsubscribed: !!subscription,
                },
                null,
                [],
            )));
            return;
        }

        // S3.4: Limit concurrent connections. Use the request id so browser
        // clients resolve the pending RPC immediately instead of waiting for
        // their own timeout and retrying.
        if (activeTcpConnections >= MAX_TCP_PER_WS) {
            ws.send(JSON.stringify({
                version: 'postfiat-local-rpc-v1',
                id: parsed.id,
                ok: false,
                result: null,
                error: { code: 'proxy_rate_limited', message: 'too many concurrent requests' },
                events: []
            }));
            return;
        }

        if (isFastpayBroadcastMethod(parsed.method)) {
            try {
                const response = await broadcastFastpayMutation(parsed);
                ws.send(JSON.stringify(response));
            } catch (e) {
                ws.send(JSON.stringify(responseEnvelope(
                    parsed.id,
                    false,
                    null,
                    {
                        code: 'proxy_fastpay_broadcast_unavailable',
                        message: e.message || 'FastPay broadcast unavailable',
                    },
                    [],
                )));
            }
            return;
        }

        let target;
        try {
            target = ['owned_sign', 'owned_unwrap_sign', 'owned_sign_v3', 'owned_unwrap_sign_v3'].includes(parsed.method)
                ? await chooseOwnedVoteEndpoint(parsed, parsed.method)
                : await resolveRpcTarget(parsed.method);
            if (['owned_sign', 'owned_unwrap_sign', 'owned_sign_v3', 'owned_unwrap_sign_v3'].includes(parsed.method)) {
                console.log(`[fastpay-${parsed.method}] routed to ${target.endpoint.validatorId} (${target.endpoint.host}:${target.endpoint.port})`);
            }
        } catch (e) {
            ws.send(JSON.stringify({
                version: 'postfiat-local-rpc-v1',
                id: parsed.id,
                ok: false,
                result: null,
                error: {
                    code: ['owned_sign', 'owned_unwrap_sign', 'owned_sign_v3', 'owned_unwrap_sign_v3'].includes(parsed.method)
                        ? 'proxy_fastpay_vote_route_unavailable'
                        : 'proxy_proposer_route_unavailable',
                    message: e.message || 'could not route request',
                },
                events: []
            }));
            return;
        }

        activeTcpConnections++;
        try {
            const originalOutbound = requestWithProxyReadiness(parsed, target.route);
            const outbound = compactFastpayVoteRequest(originalOutbound);
            let line = await rpcTcpRequestLine(
                target.endpoint.host,
                target.endpoint.port,
                outbound,
                TCP_TIMEOUT_MS,
                ['owned_sign', 'owned_unwrap_sign', 'owned_sign_v3', 'owned_unwrap_sign_v3'].includes(parsed.method)
                    ? 'fastpay-vote'
                    : 'default',
            );
            if (outbound !== originalOutbound) {
                try {
                    const compactResponse = JSON.parse(line);
                    if (
                        compactResponse?.error?.code === 'rpc_protocol_error'
                        && String(compactResponse.error.message || '').includes('order_json_gzip_base64')
                    ) {
                        line = await rpcTcpRequestLine(
                            target.endpoint.host,
                            target.endpoint.port,
                            originalOutbound,
                            TCP_TIMEOUT_MS,
                            'fastpay-vote',
                        );
                    }
                } catch (_) { /* preserve the original response */ }
            }
            if (isFinalityMethod(parsed.method) && finalityFailureCanAdvanceView(line)) {
                const recovered = await recoverFinalityAcrossViews(parsed, target.route, {
                    initialLine: line,
                });
                line = recovered.line;
                target = { endpoint: recovered.endpoint, route: recovered.route };
            }
            // Inject RPC capability fields into server_info responses
            if (INJECT_RPC_CAPS && parsed.method === 'server_info') {
                try {
                    const resp = JSON.parse(line);
                    if (resp.ok && resp.result) {
                        if (!resp.result.rpc) resp.result.rpc = {};
                        Object.assign(resp.result.rpc, RPC_CAPS);
                        ws.send(JSON.stringify(resp));
                        return;
                    }
                } catch (e) { /* fall through to send raw line */ }
            }
            if (isFinalityMethod(parsed.method)) {
                clearFastpayFleetStatusCache();
                rememberFinalizedReadEndpoint(line, target);
                primeNextProposerRouteCacheFromResponse(line, target.route, {
                    warmReadiness: true,
                });
            }
            ws.send(addProxyRouteEvent(line, target.route));
        } catch (initialError) {
            if (isFinalityMethod(parsed.method) && target?.route) {
                try {
                    const recovered = await recoverFinalityAcrossViews(parsed, target.route, {
                        initialError,
                    });
                    clearFastpayFleetStatusCache();
                    rememberFinalizedReadEndpoint(recovered.line, recovered);
                    primeNextProposerRouteCacheFromResponse(recovered.line, recovered.route, {
                        warmReadiness: true,
                    });
                    ws.send(addProxyRouteEvent(recovered.line, recovered.route));
                    return;
                } catch (_) { /* return the stable public connection error below */ }
            }
            // S3.3: Don't leak internal error details — send generic message
            ws.send(JSON.stringify({
                version: 'postfiat-local-rpc-v1',
                id: parsed.id,
                ok: false,
                result: null,
                error: { code: 'proxy_connection_error', message: 'could not connect to RPC server' },
                events: []
            }));
        } finally {
            activeTcpConnections = Math.max(0, activeTcpConnections - 1);
        }
    });
});

if (require.main === module) {
    startFastpayRouteWarmup();
    startFastpayCertificateRecovery();
    const navswapStoreLoad = loadNavswapRunStore();
    const navswapIdempotencyStoreLoad = loadNavswapIdempotencyStore();
    const startupShieldedSwapConfig = shieldedNavswapSwapConfig();
    const startupCertifierLoop = startupShieldedSwapConfig.certifier_loop?.enabled
        ? startShieldedCertifierLoop(startupShieldedSwapConfig)
        : null;
    if (startupCertifierLoop) {
        startupCertifierLoop.done.catch((error) => {
            console.error(`Shielded certifier loop exited before use: ${error.message || error}`);
        });
    }
    server.listen(LISTEN_PORT, LISTEN_HOST, () => {
        console.log(`PostFiat RPC proxy listening on ${LISTEN_HOST}:${LISTEN_PORT} -> ${RPC_HOST}:${RPC_PORT}`);
        if (startupCertifierLoop) {
            console.log(
                `Shielded certifier loop: warming pid=${startupCertifierLoop.child.pid} `
                + `ready=${startupCertifierLoop.ready_file} start_height=${startupCertifierLoop.start_height}`,
            );
        }
        if (navswapStoreLoad.enabled) {
            console.log(
                `NAVSwap run store: ${navswapStoreLoad.path} `
                + `(loaded ${navswapStoreLoad.loaded_count}, interrupted ${navswapStoreLoad.interrupted_count || 0})`,
            );
        } else {
            console.log('NAVSwap run store: disabled');
        }
        if (navswapIdempotencyStoreLoad.enabled) {
            console.log(
                `NAVSwap idempotency store: ${navswapIdempotencyStoreLoad.path} `
                + `(loaded ${navswapIdempotencyStoreLoad.loaded_count}, expired ${navswapIdempotencyStoreLoad.expired_count || 0})`,
            );
        } else {
            console.log('NAVSwap idempotency store: disabled');
        }
        if (ENABLE_PROPOSER_ROUTING) {
            console.log(`Finality proposer routing: enabled across ${RPC_FLEET.length} validators`);
            console.log(
                `First-ready sequenced reads: ${ENABLE_FIRST_READY_SEQUENCED_READ ? 'enabled' : 'disabled'}`,
            );
            console.log(
                `Upstream RPC keep-alive: ${ENABLE_UPSTREAM_KEEPALIVE ? 'enabled' : 'disabled'}`,
            );
            console.log(
                `Finality responder read cache: ${ENABLE_FINALITY_RESPONDER_READ_CACHE ? 'enabled' : 'disabled'}`,
            );
            console.log(
                `Sequenced read RPC parent-wait: ${ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE ? 'enabled' : 'disabled'}`,
            );
            console.log(
                `Preferred sequenced read validators: ${PREFERRED_SEQUENCED_READ_VALIDATORS.join(',') || 'fleet order'}`,
            );
            console.log(
                `Adaptive first-ready proposers: ${[...FIRST_READY_SEQUENCED_READ_PROPOSERS].join(',') || 'none'}`,
            );
        } else {
            console.log('Finality proposer routing: disabled');
        }
        if (ALLOWED_ORIGINS.length > 0) {
            console.log(`Allowed origins: ${ALLOWED_ORIGINS.join(', ')}`);
        } else {
            console.log('Origin checking: disabled (all origins allowed)');
        }
    });
}

module.exports = {
    DEFAULT_RPC_FLEET,
    LISTEN_HOST,
    RPC_FLEET,
    RPC_HOST,
    RPC_CAPS,
    WALLET_STATIC_DIR,
    acquireMutationAdmission,
    atomicRpcProxyError,
    addProxyRouteEvent,
    assertVaultBridgeEvidenceMatches,
    bftQuorumThreshold,
    broadcastFastpayMutation,
    chooseProposerEndpointCached,
    chooseProposerEndpointFromStatuses,
    chooseProposerEndpointWithRetry,
    chooseOwnedVoteEndpoint,
    chooseSequencedAccountReadEndpoint,
    closeUpstreamRpcConnections,
    clearFastpayFleetStatusCache,
    clearMutationAdmissionForTest,
    compactFastpayVoteRequest,
    collectFastpayFleetStatuses,
    collectFinalityTimeoutVotes: walletProxyRuntime.collectFinalityTimeoutVotes,
    deterministicProposer,
    endpointStatusMeetsRoute,
    endpointStatusMeetsSequencedReadRoute,
    isFastpayBroadcastMethod,
    isFinalityMethod,
    isSequencedAccountMethod,
    finalityFailureCanAdvanceView,
    httpRequestRequiresAuth,
    httpMutationPrincipal,
    rpcRequestRequiresAuth,
    serveWalletStatic,
    validateProxyExposureConfig,
    buildNavswapQuoteResponse,
    buildNavswapRunResponse,
    buildNavswapNavProofResponse,
    buildStakehubTransparentPreflight,
    buildShieldedCertifiedRoundArgs,
    clearNavswapDevnetFundingUsageForTest,
    clearNavswapIdempotencyForTest,
    clearNavswapRunsForTest,
    executeNavswapAtomicTemplate,
    executeNavswapDevnetPfusdcFunding,
    executeNavswapIdempotentRequest,
    executeAtomicNavswapQuote,
    executeAtomicNavswapRun,
    executeNavswapQuote: executeNavswapQuoteWithAtomic,
    executeNavswapRun,
    executeNavswapCapabilities: executeNavswapCapabilitiesWithAtomic,
    executeShieldedNavswapEgress,
    executeShieldedNavswapIngress,
    executeShieldedNavswapIngressPreflight,
    executeShieldedNavswapProverReadiness,
    executeShieldedNavswapQuote,
    executeShieldedNavswapSwap,
    executeShieldedNavswapStatus,
    executeVaultBridgeRelay,
    governedVaultBridgeRelayConfig,
    executeTransparentNavswapReadiness,
    executeTransparentNavswapRun: executeTransparentNavswapRunWithAtomic,
    loadNavswapIdempotencyStore,
    loadNavswapRunStore,
    navswapIdempotencyStorePath,
    navswapRunEvents,
    navswapRunList,
    navswapRunPublic,
    navswapRunReceipts,
    navswapRunStorePath,
    navswapRunStreamSnapshot,
    navswapTransparentOperatorConfig,
    shieldedNavswapEgressConfig,
    shieldedNavswapIngressConfig,
    shieldedNavswapSwapConfig,
    shieldedCertifiedRoundEnv,
    certifyShieldedBatchViaWarmLoop,
    startShieldedCertifierLoop,
    validateShieldedCertifierLoopReportForBatch,
    shieldedCertifierLoopBatchFile,
    shieldedPrivateEgressDisclosureFields,
    shieldedPrivateEgressDisclosureHash,
    runShieldedLaggardCatchUp,
    planTransparentNavswapWalletActions,
    prepareNavswapWalletAction,
    prepareNavswapWalletActionBatch,
    server,
    normalizeFastpayBroadcastRequest,
    normalizeAtomicTemplateParams,
    navswapBridgeConfig,
    navswapStakehubTransparentConfig,
    navswapCapabilities,
    vaultBridgeRelayConfig,
    parseRpcFleet,
    parseProxyApiTokens,
    parseListenHost,
    primeNextProposerRouteCache,
    primeNextProposerRouteCacheFromResponse,
    proposerEndpointForHeight,
    recoverFinalityAcrossViews,
    rememberFinalizedReadEndpoint,
    requestWithProxyReadiness,
    rpcTcpRequest,
    shouldUseFirstReadySequencedRead,
    startFastpayRouteWarmup,
    startFastpayCertificateRecovery,
    verifyAtomicTemplateResult,
    verifyAtomicTemplateSymmetry,
    verifyTransparentNavSubscriptionAllocation,
    waitForFastpayConvergedGroup,
};
