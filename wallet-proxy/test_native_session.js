'use strict';

// W-2: per-install native sessions carry exactly the browser relay's caps and
// are bound to the gateway's approved origin and network.

const assert = require('assert');
const crypto = require('crypto');
const fs = require('fs');
const http = require('http');
const net = require('net');
const os = require('os');
const path = require('path');
const { Readable } = require('stream');
const WebSocket = require('ws');

const BROWSER_TOKEN = 'browser-relay-test-token-32-bytes-min-01';
const NATIVE_ORIGIN = 'https://gateway.example.test';
const WALLET_ORIGIN = 'https://wallet.example.test';
const CHAIN_ID = 'postfiat-wan-devnet-2';
const GENESIS = 'ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9';
const SECRET = 'native-session-test-secret-32-bytes-minimum-01';
const NATIVE_ENV = {
  WALLET_PROXY_NATIVE_SESSION_ORIGIN: NATIVE_ORIGIN,
  WALLET_PROXY_NATIVE_SESSION_SECRET: SECRET,
  WALLET_PROXY_NATIVE_SESSION_CHAIN_ID: CHAIN_ID,
  WALLET_PROXY_NATIVE_SESSION_GENESIS_HASH: GENESIS,
};

delete process.env.LISTEN_HOST;
delete process.env.WALLET_PROXY_API_TOKENS_JSON;
delete process.env.WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE;
Object.assign(process.env, NATIVE_ENV, {
  WALLET_PROXY_API_TOKEN: BROWSER_TOKEN,
  WALLET_PROXY_LOCAL_SESSION_PRINCIPAL: 'default',
  ALLOWED_ORIGINS: `${NATIVE_ORIGIN},${WALLET_ORIGIN}`,
  WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_LIMIT: '1000',
  ENABLE_PROPOSER_ROUTING: 'false',
  WALLET_PROXY_MUTATION_RATE_LIMIT: '3',
  WALLET_PROXY_MUTATION_RATE_WINDOW_MS: '60000',
  WALLET_PROXY_MAX_HTTP_BODY_BYTES: '256',
  NAVSWAP_IDEMPOTENCY_STORE_PATH: 'off',
  NAVSWAP_RUN_STORE_PATH: 'off',
});

// Loaded in main() once the fake validator RPC is listening.
let clearMutationAdmissionForTest;
let server;
const {
  ALGORITHM_ID, DEFAULT_MAX_ACTIVE_INSTALLS, INFO_PATH, MAX_LIST_FILE_BYTES, OPEN_PATH, OPEN_SCHEMA, SIGNATURE_CONTEXT,
  createNativeSessions, installId, mlDsa65, openMessage,
} = require('./native-session');

function request(port, method, pathname, { headers = {}, body = null } = {}) {
  return new Promise((resolve, reject) => {
    const payload = body === null ? '' : (typeof body === 'string' ? body : JSON.stringify(body));
    const req = http.request({
      host: '127.0.0.1', port, path: pathname, method,
      headers: { 'content-type': 'application/json', 'content-length': Buffer.byteLength(payload), ...headers },
    }, (res) => {
      let raw = '';
      res.setEncoding('utf8');
      res.on('data', (chunk) => { raw += chunk; });
      res.on('end', () => {
        let parsed = null;
        try { parsed = JSON.parse(raw); } catch (_) { /* asserted by callers */ }
        resolve({ statusCode: res.statusCode, headers: res.headers, body: parsed });
      });
    });
    req.on('error', reject);
    req.end(payload);
  });
}

async function keypair() {
  const ml = await mlDsa65();
  const keys = ml.keygen();
  return { ...keys, publicKeyHex: Buffer.from(keys.publicKey).toString('hex') };
}

async function openBody(keys, overrides = {}, now = Date.now()) {
  const body = {
    schema: OPEN_SCHEMA, algorithm_id: ALGORITHM_ID, public_key_hex: keys.publicKeyHex,
    client_nonce_hex: crypto.randomBytes(32).toString('hex'), origin: NATIVE_ORIGIN,
    chain_id: CHAIN_ID, genesis_hash_hex: GENESIS, issued_at_ms: now, ...overrides,
  };
  const ml = await mlDsa65();
  body.signature_hex = Buffer.from(ml.sign(openMessage(body), keys.secretKey,
    { context: Buffer.from(SIGNATURE_CONTEXT, 'utf8') })).toString('hex');
  return body;
}

// Drives another gateway instance (different secret, origin, network or clock).
async function openVia(instance, body, remoteAddress = undefined, method = 'POST') {
  const req = Readable.from([Buffer.from(typeof body === 'string' ? body : JSON.stringify(body))]);
  req.method = method;
  req.headers = {};
  if (remoteAddress) req.socket = { remoteAddress };
  return instance.open(req);
}

function infoVia(instance, method = 'GET') {
  return instance.info({ method, headers: {} });
}

function websocketSubmit(port, token, origin) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${port}`, { origin });
    const timer = setTimeout(() => { ws.terminate(); reject(new Error('websocket response timed out')); }, 5_000);
    ws.on('open', () => ws.send(JSON.stringify({
      version: 'postfiat-local-rpc-v1', id: 'native-ws', method: 'mempool_submit_signed_transfer',
      params: { signed_transfer_json: '{}' }, proxy_auth_token: token,
    })));
    ws.on('message', (message) => { clearTimeout(timer); ws.close(); resolve(JSON.parse(message.toString('utf8'))); });
    ws.on('error', reject);
  });
}

async function main() {
  // Fake validator RPC: answers every forwarded call so WebSocket auth can be observed.
  const upstream = net.createServer((socket) => {
    socket.setEncoding('utf8');
    socket.once('data', (line) => {
      const forwarded = JSON.parse(line.trim());
      socket.end(`${JSON.stringify({ version: 'postfiat-local-rpc-v1', id: forwarded.id, ok: true, result: { forwarded: forwarded.method }, error: null, events: [] })}\n`);
    });
  });
  await new Promise((resolve) => upstream.listen(0, '127.0.0.1', resolve));
  process.env.RPC_HOST = '127.0.0.1';
  process.env.RPC_PORT = String(upstream.address().port);
  ({ clearMutationAdmissionForTest, server } = require('./server'));
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  const fund = (token, key, origin = NATIVE_ORIGIN) => request(port, 'POST', '/api/navswap/devnet-fund-pfusdc', {
    headers: { authorization: `Bearer ${token}`, origin },
    body: { idempotency_key: key, recipient: 'pf-test', amount_atoms: '1' },
  });
  try {
    const alpha = await keypair();
    const beta = await keypair();

    // Open succeeds; the token authenticates as native:<install id> with the relay's caps.
    const opened = await request(port, 'POST', OPEN_PATH, { body: await openBody(alpha) });
    assert.strictEqual(opened.statusCode, 200, JSON.stringify(opened.body));
    assert.strictEqual(opened.headers.location, undefined);
    assert.strictEqual(opened.headers['cache-control'], 'no-store');
    const native = opened.body;
    assert.strictEqual(native.principal, `native:${installId(alpha.publicKeyHex)}`);
    assert.strictEqual(native.origin, NATIVE_ORIGIN);
    assert.strictEqual(native.genesis_hash_hex, GENESIS);
    assert.ok(native.expires_at_ms - Date.now() > 14 * 60_000 && native.expires_at_ms - Date.now() <= 15 * 60_000);
    assert.deepStrictEqual(native.caps, {
      same_as: 'browser_relay', mutation_rate_limit: 3, mutation_rate_window_ms: 60000,
      mutation_concurrency: 16, max_http_body_bytes: 256, max_ws_message_bytes: 1024 * 1024,
    });
    clearMutationAdmissionForTest();
    const used = await fund(native.token, 'native-key-0001');
    assert.notStrictEqual(used.statusCode, 401, JSON.stringify(used.body));
    assert.notStrictEqual(used.statusCode, 403, JSON.stringify(used.body));
    assert.strictEqual(used.headers.location, undefined);
    assert.strictEqual(used.body.idempotency.principal_id, native.principal);

    // Open refusals.
    const refused = async (body, status, code, raw = false) => {
      const response = await request(port, 'POST', OPEN_PATH, { body: raw ? body : body });
      assert.strictEqual(response.statusCode, status, `${code}: ${JSON.stringify(response.body)}`);
      assert.strictEqual(response.body.code, code);
    };
    const tampered = await openBody(alpha);
    tampered.signature_hex = `${tampered.signature_hex.slice(0, 10)}${tampered.signature_hex[10] === '0' ? '1' : '0'}${tampered.signature_hex.slice(11)}`;
    await refused(tampered, 401, 'native_session_bad_signature');
    const otherKeySigned = await openBody(beta);
    await refused({ ...otherKeySigned, public_key_hex: alpha.publicKeyHex }, 401, 'native_session_bad_signature');
    await refused(await openBody(alpha, { origin: 'https://attacker.example' }), 403, 'native_session_wrong_origin');
    await refused(await openBody(alpha, { genesis_hash_hex: 'ab'.repeat(48) }), 403, 'native_session_wrong_network');
    await refused(await openBody(alpha, { chain_id: 'postfiat-other' }), 403, 'native_session_wrong_network');
    await refused(await openBody(alpha, {}, Date.now() - 10 * 60_000), 401, 'native_session_stale');
    const once = await openBody(alpha);
    assert.strictEqual((await request(port, 'POST', OPEN_PATH, { body: once })).statusCode, 200);
    await refused(once, 409, 'native_session_nonce_replayed');
    await refused({ ...(await openBody(alpha)), secret_key_hex: '00' }, 400, 'native_session_request_invalid');
    await refused(JSON.stringify({ padding: 'x'.repeat(17 * 1024) }), 413, 'request_body_too_large', true);
    const foreignHeader = await request(port, 'POST', OPEN_PATH, { body: await openBody(alpha), headers: { origin: WALLET_ORIGIN } });
    assert.strictEqual(foreignHeader.statusCode, 403);
    assert.strictEqual(foreignHeader.body.code, 'native_session_wrong_origin');

    // Tokens refused on use: expired, other origin, other network, other gateway secret.
    const shifted = createNativeSessions({ env: NATIVE_ENV, now: () => Date.now() - 20 * 60_000 });
    const expired = await openVia(shifted, await openBody(alpha, {}, Date.now() - 20 * 60_000));
    assert.strictEqual(expired.statusCode, 200);
    const tokenCases = [
      [expired.body.token, 401, 'native_session_expired'],
      [(await openVia(createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ORIGIN: 'https://other.example.test' } }),
        await openBody(alpha, { origin: 'https://other.example.test' }))).body.token, 403, 'native_session_wrong_origin'],
      [(await openVia(createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_GENESIS_HASH: 'cd'.repeat(48) } }),
        await openBody(alpha, { genesis_hash_hex: 'cd'.repeat(48) }))).body.token, 403, 'native_session_wrong_network'],
      [(await openVia(createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_SECRET: 'another-gateway-secret-32-bytes-minimum-99' } }),
        await openBody(alpha))).body.token, 401, 'native_session_invalid'],
      [`${native.token.slice(0, -2)}AA`, 401, 'native_session_invalid'],
    ];
    for (const [token, status, code] of tokenCases) {
      assert.ok(token, code);
      const response = await fund(token, `refused-${code}`);
      assert.strictEqual(response.statusCode, status, `${code}: ${JSON.stringify(response.body)}`);
      assert.strictEqual(response.body.code, code);
    }
    // A valid token used from another allowed browser origin is not this session's origin.
    const crossOrigin = await fund(native.token, 'cross-origin', WALLET_ORIGIN);
    assert.strictEqual(crossOrigin.statusCode, 403);
    assert.strictEqual(crossOrigin.body.code, 'native_session_wrong_origin');

    // Tokens never travel in query strings (native or proxy token).
    for (const query of [`?session=${encodeURIComponent(native.token)}`, `?token=${BROWSER_TOKEN}`]) {
      const response = await request(port, 'GET', `/healthz${query}`);
      assert.strictEqual(response.statusCode, 400);
      assert.strictEqual(response.body.code, 'proxy_token_in_query');
    }

    // Disabled when WALLET_PROXY_NATIVE_SESSION_ORIGIN is unset: 404, no session surface.
    const disabled = createNativeSessions({ env: {} });
    assert.strictEqual(disabled.enabled, false);
    const disabledOpen = await openVia(disabled, await openBody(alpha));
    assert.strictEqual(disabledOpen.statusCode, 404);
    assert.strictEqual(disabledOpen.body.code, 'native_session_disabled');
    assert.strictEqual(disabled.authenticate(native.token).principal, null);
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_SECRET: 'short' } }), /SECRET/);
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ORIGIN: 'http://gateway.example.test' } }), /https origin/);

    // Allowlist: only listed installs may open; operator-file checks apply.
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'pft-native-allowlist-'));
    const allowlistFile = path.join(dir, 'allowlist.json');
    fs.writeFileSync(allowlistFile, JSON.stringify({
      schema: 'postfiat-native-session-allowlist-v1', install_public_keys_hex: [alpha.publicKeyHex],
    }), { mode: 0o600 });
    const listed = createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE: allowlistFile } });
    assert.strictEqual((await openVia(listed, await openBody(alpha))).statusCode, 200);
    const unlisted = await openVia(listed, await openBody(beta));
    assert.strictEqual(unlisted.statusCode, 403);
    assert.strictEqual(unlisted.body.code, 'native_session_install_not_allowed');
    const betaToken = (await openVia(createNativeSessions({ env: NATIVE_ENV, log: () => {} }), await openBody(beta))).body.token;
    // A token minted by another gateway process is refused before any list check.
    assert.strictEqual(listed.authenticate(betaToken).failure.code, 'native_session_restarted');
    const link = path.join(dir, 'link.json');
    fs.symlinkSync(allowlistFile, link);
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE: link } }), /owner-controlled/);
    fs.chmodSync(allowlistFile, 0o666);
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE: allowlistFile } }), /owner-controlled/);
    fs.chmodSync(allowlistFile, 0o600);

    // Revocation without a restart: both lists are re-read when their mtime
    // changes, checked at most once per second; a bad file never falls open.
    const denylistFile = path.join(dir, 'denylist.json');
    const writeList = (file, value, mtimeSeconds) => {
      fs.writeFileSync(file, typeof value === 'string' ? value : JSON.stringify(value), { mode: 0o600 });
      fs.utimesSync(file, mtimeSeconds, mtimeSeconds);
    };
    const allow = (...keys) => ({ schema: 'postfiat-native-session-allowlist-v1', install_public_keys_hex: keys.map((k) => k.publicKeyHex) });
    const deny = (...keys) => ({ schema: 'postfiat-native-session-denylist-v1', install_ids_hex: keys.map((k) => installId(k.publicKeyHex)) });
    writeList(allowlistFile, allow(alpha, beta), 1_000);
    writeList(denylistFile, deny(), 1_000);
    let listClock = Date.now();
    const listErrors = [];
    const revocable = createNativeSessions({
      env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE: allowlistFile, WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE: denylistFile },
      now: () => listClock, log: () => {}, logError: (line) => listErrors.push(line),
    });
    const openAt = async (instance, keys, clockMs) => openVia(instance, await openBody(keys, {}, clockMs));
    const alphaToken = (await openAt(revocable, alpha, listClock)).body.token;
    const betaListedToken = (await openAt(revocable, beta, listClock)).body.token;
    const revoked = { statusCode: 403, code: 'native_session_install_revoked' };
    writeList(denylistFile, deny(alpha), 2_000);
    listClock += 500;
    assert.strictEqual(revocable.authenticate(alphaToken).failure, null, 'lists are checked at most once per second');
    listClock += 600;
    assert.deepStrictEqual(revocable.authenticate(alphaToken).failure, revoked);
    assert.deepStrictEqual((await openAt(revocable, alpha, listClock)).body.code, revoked.code);
    assert.strictEqual(revocable.authenticate(betaListedToken).failure, null);
    writeList(allowlistFile, allow(alpha), 3_000);
    listClock += 1_000;
    assert.deepStrictEqual(revocable.authenticate(betaListedToken).failure, revoked);
    const betaReopen = await openAt(revocable, beta, listClock);
    assert.strictEqual(betaReopen.statusCode, 403);
    assert.strictEqual(betaReopen.body.code, revoked.code);
    // Malformed files keep the last good lists and log one error line each.
    writeList(denylistFile, '{not json', 4_000);
    writeList(allowlistFile, { schema: 'postfiat-native-session-allowlist-v1', install_public_keys_hex: 'all' }, 4_000);
    for (let step = 0; step < 3; step += 1) {
      listClock += 1_000;
      assert.deepStrictEqual(revocable.authenticate(alphaToken).failure, revoked);
      assert.deepStrictEqual(revocable.authenticate(betaListedToken).failure, revoked);
    }
    assert.strictEqual(listErrors.length, 2, listErrors.join('\n'));
    assert.ok(listErrors.some((line) => line.includes('denylist')) && listErrors.some((line) => line.includes('allowlist')));
    writeList(denylistFile, deny(), 5_000);
    listClock += 1_000;
    assert.strictEqual(revocable.authenticate(alphaToken).failure, null, 'un-denied install works again');
    writeList(denylistFile, deny(), 6_000);
    fs.chmodSync(denylistFile, 0o666);
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE: denylistFile } }), /owner-controlled/);
    fs.chmodSync(denylistFile, 0o600);
    writeList(denylistFile, { schema: 'postfiat-native-session-denylist-v1', install_ids_hex: ['zz'] }, 7_000);
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE: denylistFile } }), /denylist/);

    // An in-place edit of the same length with the mtime restored is seen, and a
    // load refused for its permissions is retried once they are repaired.
    writeList(allowlistFile, allow(alpha, beta), 8_000);
    writeList(denylistFile, deny(beta), 8_000);
    let hashClock = Date.now();
    const hashErrors = [];
    const hashed = createNativeSessions({
      env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE: allowlistFile, WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE: denylistFile },
      now: () => hashClock, log: () => {}, logError: (line) => hashErrors.push(line),
    });
    const hashedAlpha = (await openAt(hashed, alpha, hashClock)).body.token;
    const denyInode = fs.statSync(denylistFile).ino;
    writeList(denylistFile, deny(alpha), 8_000);
    assert.strictEqual(fs.statSync(denylistFile).ino, denyInode);
    hashClock += 1_000;
    assert.deepStrictEqual(hashed.authenticate(hashedAlpha).failure, revoked, 'same inode, size and mtime');
    writeList(denylistFile, deny(), 9_000);
    hashClock += 1_000;
    assert.strictEqual(hashed.authenticate(hashedAlpha).failure, null);
    writeList(denylistFile, deny(alpha), 10_000);
    fs.chmodSync(denylistFile, 0o666);
    for (let step = 0; step < 3; step += 1) {
      hashClock += 1_000;
      assert.strictEqual(hashed.authenticate(hashedAlpha).failure, null, 'a group-writable file is not loaded');
    }
    assert.strictEqual(hashErrors.length, 1, hashErrors.join('\n'));
    fs.chmodSync(denylistFile, 0o600);
    hashClock += 1_000;
    assert.deepStrictEqual(hashed.authenticate(hashedAlpha).failure, revoked, 'a permission-only repair is retried');
    assert.strictEqual(hashErrors.length, 1, hashErrors.join('\n'));

    // An id nested in an array is malformed, not another spelling of the id: the
    // revocation stands on reload and the file is refused at startup.
    const alphaId = installId(alpha.publicKeyHex);
    writeList(denylistFile, { schema: 'postfiat-native-session-denylist-v1', install_ids_hex: [[alphaId]] }, 11_000);
    hashClock += 1_000;
    assert.deepStrictEqual(hashed.authenticate(hashedAlpha).failure, revoked, 'a nested denylist id keeps the last good list');
    assert.strictEqual((await openAt(hashed, alpha, hashClock)).body.code, revoked.code);
    assert.strictEqual(hashErrors.length, 2, hashErrors.join('\n'));
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE: denylistFile } }), /denylist/);
    writeList(allowlistFile, { schema: 'postfiat-native-session-allowlist-v1', install_public_keys_hex: [[alpha.publicKeyHex]] }, 11_000);
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE: allowlistFile } }), /allowlist/);

    // A reload frees the slot of every install it revokes, without that install's help.
    writeList(allowlistFile, allow(alpha, beta), 12_000);
    writeList(denylistFile, deny(), 12_000);
    let slotClock = Date.now();
    const slots = createNativeSessions({
      env: {
        ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_MAX_ACTIVE_INSTALLS: '1',
        WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE: allowlistFile, WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE: denylistFile,
      },
      now: () => slotClock, log: () => {}, logError: () => {},
    });
    assert.strictEqual((await openAt(slots, alpha, slotClock)).statusCode, 200);
    assert.strictEqual((await openAt(slots, beta, slotClock)).statusCode, 503);
    writeList(denylistFile, deny(alpha), 13_000);
    slotClock += 1_000;
    const betaSlot = await openAt(slots, beta, slotClock);
    assert.strictEqual(betaSlot.statusCode, 200, 'a silent denied occupant frees its slot');
    writeList(denylistFile, deny(), 14_000);
    writeList(allowlistFile, allow(alpha), 14_000);
    slotClock += 1_000;
    assert.strictEqual((await openAt(slots, alpha, slotClock)).statusCode, 200, 'a silent delisted occupant frees its slot');
    // Listed again, beta's token may not take back a slot past the cap.
    writeList(allowlistFile, allow(alpha, beta), 15_000);
    slotClock += 1_000;
    assert.deepStrictEqual(slots.authenticate(betaSlot.body.token).failure, { statusCode: 503, code: 'native_session_busy' });

    // A list file past MAX_LIST_FILE_BYTES is refused before it is read, like a
    // malformed one: at startup, and on reload, where the last good list stays.
    const padTo = (value, bytes) => {
      const text = JSON.stringify(value);
      return text + ' '.repeat(bytes - Buffer.byteLength(text));
    };
    const bothLists = { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE: allowlistFile, WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE: denylistFile };
    writeList(allowlistFile, padTo(allow(alpha, beta), MAX_LIST_FILE_BYTES), 16_000);
    writeList(denylistFile, padTo(deny(beta), MAX_LIST_FILE_BYTES), 16_000);
    assert.strictEqual(fs.statSync(allowlistFile).size, MAX_LIST_FILE_BYTES);
    let sizeClock = Date.now();
    const sizeErrors = [];
    const sized = createNativeSessions({
      env: bothLists, now: () => sizeClock, log: () => {}, logError: (line) => sizeErrors.push(line),
    });
    const sizedAlpha = await openAt(sized, alpha, sizeClock);
    assert.strictEqual(sizedAlpha.statusCode, 200, 'an allowlist of exactly the limit loads');
    assert.strictEqual((await openAt(sized, beta, sizeClock)).body.code, revoked.code, 'a denylist of exactly the limit loads');
    writeList(allowlistFile, padTo(allow(beta), MAX_LIST_FILE_BYTES + 1), 17_000);
    writeList(denylistFile, padTo(deny(alpha), MAX_LIST_FILE_BYTES + 1), 17_000);
    for (let step = 0; step < 3; step += 1) {
      sizeClock += 1_000;
      assert.strictEqual(sized.authenticate(sizedAlpha.body.token).failure, null, 'oversize lists keep the last good lists');
    }
    assert.strictEqual(sizeErrors.length, 2, sizeErrors.join('\n'));
    for (const label of ['allowlist', 'denylist']) {
      assert.ok(sizeErrors.some((line) => line.includes(label) && line.includes(`${MAX_LIST_FILE_BYTES}-byte limit`)), sizeErrors.join('\n'));
    }
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE: allowlistFile } }), /ALLOWLIST_FILE .*byte limit/);
    assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE: denylistFile } }), /DENYLIST_FILE .*byte limit/);

    // A restart ends every session: a token of an earlier process is refused, so
    // surviving tokens cannot fill the live-install table past the cap.
    const capOne = { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_MAX_ACTIVE_INSTALLS: '1' };
    const beforeRestart = createNativeSessions({ env: capOne, log: () => {} });
    const survivor = (await openVia(beforeRestart, await openBody(alpha))).body.token;
    assert.strictEqual(beforeRestart.authenticate(survivor).failure, null);
    const afterRestart = createNativeSessions({ env: capOne, log: () => {} });
    assert.strictEqual((await openVia(afterRestart, await openBody(beta))).statusCode, 200);
    assert.deepStrictEqual(afterRestart.authenticate(survivor).failure, { statusCode: 401, code: 'native_session_restarted' });
    assert.strictEqual((await openVia(afterRestart, await openBody(alpha))).body.code, 'native_session_busy');

    // A bound across installs: distinct installs with unexpired tokens.
    let capClock = Date.now();
    const capped = createNativeSessions({
      env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_MAX_ACTIVE_INSTALLS: '1', WALLET_PROXY_NATIVE_SESSION_TTL_MS: '60000' },
      now: () => capClock, log: () => {},
    });
    assert.strictEqual((await openAt(capped, alpha, capClock)).statusCode, 200);
    const busy = await openAt(capped, beta, capClock);
    assert.strictEqual(busy.statusCode, 503);
    assert.strictEqual(busy.body.code, 'native_session_busy');
    assert.strictEqual((await openAt(capped, alpha, capClock)).statusCode, 200, 'a live install renews');
    capClock += 60_001;
    assert.strictEqual((await openAt(capped, beta, capClock)).statusCode, 200, 'an expired install frees its slot');
    assert.strictEqual((await openAt(capped, alpha, capClock)).statusCode, 503);
    for (const bad of ['0', '1000001', 'many']) {
      assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_MAX_ACTIVE_INSTALLS: bad } }), /MAX_ACTIVE_INSTALLS/);
    }

    // One audit line per open: install id prefix, source address, outcome; never a credential.
    const audit = [];
    const audited = createNativeSessions({ env: NATIVE_ENV, log: (line) => audit.push(line) });
    const auditedBody = await openBody(alpha);
    const auditedOpen = await openVia(audited, auditedBody, '192.0.2.9');
    assert.strictEqual(auditedOpen.statusCode, 200);
    await openVia(audited, tampered, '192.0.2.10');
    await openVia(audited, 'not json', '192.0.2.11');
    assert.strictEqual(audit.length, 3);
    assert.deepStrictEqual(audit.map((line) => JSON.parse(line)), [
      { event: 'native_session_open', install_id_prefix: installId(alpha.publicKeyHex).slice(0, 16), source: '192.0.2.9', outcome: 'ok' },
      { event: 'native_session_open', install_id_prefix: null, source: '192.0.2.10', outcome: 'native_session_bad_signature' },
      { event: 'native_session_open', install_id_prefix: null, source: '192.0.2.11', outcome: 'invalid_json' },
    ]);
    const auditText = audit.join('\n');
    const [, tokenPayload, tokenMac] = auditedOpen.body.token.split('.');
    for (const secretish of [auditedOpen.body.token, tokenPayload, tokenMac, auditedBody.signature_hex.slice(0, 64),
      SECRET, installId(alpha.publicKeyHex), alpha.publicKeyHex.slice(0, 64)]) {
      assert.ok(!auditText.includes(secretish), 'audit line leaks a credential or a full identity');
    }

    // Discovery: the session settings, no secret; disabled answers 404 like open.
    const info = await request(port, 'GET', INFO_PATH);
    assert.strictEqual(info.statusCode, 200, JSON.stringify(info.body));
    assert.strictEqual(info.headers['cache-control'], 'no-store');
    assert.deepStrictEqual(info.body, {
      ok: true, schema: 'postfiat-native-session-info-v1', enabled: true, origin: NATIVE_ORIGIN, chain_id: CHAIN_ID,
      genesis_hash_hex: GENESIS, ttl_ms: 15 * 60_000, allowlist: false, max_active_installs: DEFAULT_MAX_ACTIVE_INSTALLS,
    });
    assert.ok(!JSON.stringify(info.body).includes(SECRET));
    assert.strictEqual((await request(port, 'POST', INFO_PATH, { body: {} })).statusCode, 405);
    assert.strictEqual((await infoVia(revocable)).body.allowlist, true);
    const disabledInfo = await infoVia(createNativeSessions({ env: {} }));
    assert.strictEqual(disabledInfo.statusCode, 404);
    assert.strictEqual(disabledInfo.body.code, 'native_session_disabled');

    // Caps: the native principal hits the same rate limit and body limit as the browser principal.
    clearMutationAdmissionForTest();
    for (const [label, token] of [['browser', BROWSER_TOKEN], ['native', native.token]]) {
      for (let index = 1; index <= 3; index += 1) {
        const response = await fund(token, `${label}-rate-${index}`);
        assert.notStrictEqual(response.statusCode, 429, `${label} ${index}`);
      }
      const limited = await fund(token, `${label}-rate-4`);
      assert.strictEqual(limited.statusCode, 429, label);
      assert.strictEqual(limited.body.code, 'proxy_mutation_rate_limited');
    }
    clearMutationAdmissionForTest();
    for (const token of [BROWSER_TOKEN, native.token]) {
      const oversized = await request(port, 'POST', '/api/navswap/devnet-fund-pfusdc', {
        headers: { authorization: `Bearer ${token}`, origin: NATIVE_ORIGIN },
        body: { padding: 'x'.repeat(512) },
      });
      assert.strictEqual(oversized.statusCode, 413);
      assert.strictEqual(oversized.body.code, 'request_body_too_large');
    }

    // WebSocket mutations accept the native token as proxy_auth_token, under the same origin binding.
    clearMutationAdmissionForTest();
    const wsAccepted = await websocketSubmit(port, native.token, NATIVE_ORIGIN);
    assert.strictEqual(wsAccepted.ok, true, JSON.stringify(wsAccepted));
    assert.strictEqual(wsAccepted.result.forwarded, 'mempool_submit_signed_transfer');
    for (const [token, origin, code] of [
      [`${native.token.slice(0, -2)}AA`, NATIVE_ORIGIN, 'native_session_invalid'],
      [native.token, WALLET_ORIGIN, 'native_session_wrong_origin'],
      [expired.body.token, NATIVE_ORIGIN, 'native_session_expired'],
    ]) {
      const refusedWs = await websocketSubmit(port, token, origin);
      assert.strictEqual(refusedWs.ok, false, code);
      assert.strictEqual(refusedWs.error.code, code);
    }

    // Open rate limit: per source address, before the body is read or a signature verified.
    let clock = Date.now();
    const limitedInstance = createNativeSessions({
      env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_LIMIT: '2', WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_WINDOW_MS: '60000' },
      now: () => clock,
    });
    assert.strictEqual((await openVia(limitedInstance, await openBody(alpha, {}, clock), '203.0.113.7')).statusCode, 200);
    assert.strictEqual((await openVia(limitedInstance, 'not json', '203.0.113.7')).statusCode, 400);
    clock += 15_000;
    const limited = await openVia(limitedInstance, await openBody(alpha, {}, clock), '203.0.113.7');
    assert.strictEqual(limited.statusCode, 429);
    assert.strictEqual(limited.body.code, 'native_session_rate_limited');
    assert.strictEqual(limited.body.retry_after_ms, 45_000);
    assert.strictEqual((await openVia(limitedInstance, 'not json', '203.0.113.7')).statusCode, 429);
    assert.strictEqual((await openVia(limitedInstance, await openBody(beta, {}, clock), '203.0.113.8')).statusCode, 200);
    clock += 45_000;
    assert.strictEqual((await openVia(limitedInstance, await openBody(alpha, {}, clock), '203.0.113.7')).statusCode, 200);
    assert.strictEqual(createNativeSessions({ env: NATIVE_ENV }).enabled, true);
    for (const bad of ['0', '100001', '1.5']) {
      assert.throws(() => createNativeSessions({ env: { ...NATIVE_ENV, WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_LIMIT: bad } }), /OPEN_RATE_LIMIT/);
    }

    // Cross-implementation: an open request signed by crates/crypto_provider verifies here.
    const vector = JSON.parse(fs.readFileSync(path.join(__dirname, 'fixtures', 'native-session-rust-vector.json'), 'utf8'));
    assert.strictEqual(vector.context, SIGNATURE_CONTEXT);
    assert.strictEqual(vector.request.origin, NATIVE_ORIGIN);
    assert.strictEqual(vector.request.chain_id, CHAIN_ID);
    assert.strictEqual(vector.request.genesis_hash_hex, GENESIS);
    const atVectorTime = () => createNativeSessions({ env: NATIVE_ENV, now: () => vector.request.issued_at_ms + 1_000 });
    const vectorGateway = atVectorTime();
    const rustOpened = await openVia(vectorGateway, vector.request);
    assert.strictEqual(rustOpened.statusCode, 200, JSON.stringify(rustOpened.body));
    assert.strictEqual(rustOpened.body.principal, `native:${installId(vector.request.public_key_hex)}`);
    assert.strictEqual(vectorGateway.authenticate(rustOpened.body.token).principal, rustOpened.body.principal);
    const rustTampered = await openVia(atVectorTime(), { ...vector.request, issued_at_ms: vector.request.issued_at_ms + 1 });
    assert.strictEqual(rustTampered.statusCode, 401);
    assert.strictEqual(rustTampered.body.code, 'native_session_bad_signature');

    // The browser local-session path is unchanged.
    const local = await request(port, 'GET', '/api/bridge/local-session', {
      headers: { host: `127.0.0.1:${port}`, 'sec-fetch-site': 'same-origin' },
    });
    assert.strictEqual(local.statusCode, 200);
    assert.strictEqual(local.body.token, BROWSER_TOKEN);
    const remoteNative = await request(port, 'GET', '/api/bridge/local-session', { headers: { host: `127.0.0.1:${port}` } });
    assert.strictEqual(remoteNative.statusCode, 403);
    assert.strictEqual(remoteNative.body.code, 'local_session_forbidden');

    console.log('native session regression passed');
  } finally {
    clearMutationAdmissionForTest();
    await new Promise((resolve) => server.close(resolve));
    await new Promise((resolve) => upstream.close(resolve));
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
