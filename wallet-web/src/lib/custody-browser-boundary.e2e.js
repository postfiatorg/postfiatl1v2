import assert from 'node:assert/strict';
import { createHash, randomBytes } from 'node:crypto';
import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join, normalize, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { chromium } from 'playwright';

const WALLET_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const WEBSOCKET_GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';

function websocketFrame(text) {
  const payload = Buffer.from(text);
  let header;
  if (payload.length < 126) {
    header = Buffer.from([0x81, payload.length]);
  } else if (payload.length <= 0xffff) {
    header = Buffer.alloc(4);
    header[0] = 0x81;
    header[1] = 126;
    header.writeUInt16BE(payload.length, 2);
  } else {
    header = Buffer.alloc(10);
    header[0] = 0x81;
    header[1] = 127;
    header.writeBigUInt64BE(BigInt(payload.length), 2);
  }
  return Buffer.concat([header, payload]);
}

function consumeClientFrames(buffer, onText) {
  let offset = 0;
  while (buffer.length - offset >= 2) {
    const first = buffer[offset];
    const second = buffer[offset + 1];
    let length = second & 0x7f;
    let cursor = offset + 2;
    if (length === 126) {
      if (buffer.length - cursor < 2) break;
      length = buffer.readUInt16BE(cursor);
      cursor += 2;
    } else if (length === 127) {
      if (buffer.length - cursor < 8) break;
      const wide = buffer.readBigUInt64BE(cursor);
      if (wide > BigInt(Number.MAX_SAFE_INTEGER)) throw new Error('WebSocket test frame too large');
      length = Number(wide);
      cursor += 8;
    }
    const masked = Boolean(second & 0x80);
    if (!masked || buffer.length - cursor < 4 + length) break;
    const mask = buffer.subarray(cursor, cursor + 4);
    cursor += 4;
    const payload = Buffer.from(buffer.subarray(cursor, cursor + length));
    for (let index = 0; index < payload.length; index++) payload[index] ^= mask[index % 4];
    const opcode = first & 0x0f;
    if (opcode === 0x1) onText(payload.toString('utf8'));
    offset = cursor + length;
  }
  return buffer.subarray(offset);
}

async function readRequestBody(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  return Buffer.concat(chunks).toString('utf8');
}

function fixtureHtml() {
  return `<!doctype html><meta charset="utf-8"><title>custody boundary capture</title>
<script type="module">
import { RpcClient } from '/src/lib/rpc-client.js';
import { SwapServer } from '/src/lib/swap-server.js';
import { relayVaultDeposit } from '/src/lib/bridge-relay.js';
import { fastSwapDemoApi } from '/src/lib/fastswap-demo.js';
import { approveUsdc, depositToBridge } from '/src/lib/evm.js';
import { clearSensitiveMemory, setDecryptedState } from '/src/lib/vault.js';

window.runCustodyCapture = async ({ seed, backupJson, privateNote }) => {
  if (!privateNote || Object.keys(privateNote).length !== 11) {
    throw new Error('complete private-note fixture is required');
  }
  const publicKey = 'ab'.repeat(1952);
  const publicSignature = 'PUBLIC_SIGNATURE_CAPTURE_MARKER_' + 'cd'.repeat(3309);
  const signed = JSON.stringify({ public_key_hex: publicKey, signature_hex: publicSignature });
  const cert = JSON.stringify({ votes: [{ validator_id: 'validator-0', signature_hex: publicSignature }] });
  const ethereumRequests = [];
  window.ethereum = {
    async request(request) {
      ethereumRequests.push(request);
      if (request.method === 'eth_chainId') return '0xa4b1';
      if (request.method === 'eth_accounts') return ['0x1111111111111111111111111111111111111111'];
      if (request.method === 'eth_sendTransaction') return '0x' + '22'.repeat(32);
      return null;
    },
  };

  setDecryptedState(seed, backupJson);
  localStorage.setItem('wallet-address', 'pf-browser-capture');

  const rpc = new RpcClient('ws://' + location.host + '/rpc', 'session-only-token');
  await rpc.submitSignedTransferFinality(signed);
  await rpc.submitSignedPaymentV2Finality(signed);
  await rpc.submitSignedAssetTransactionFinality(signed);
  await rpc.submitSignedEscrowTransactionFinality(signed);
  await rpc.submitSignedOfferTransaction(signed);
  await rpc.submitFastlanePrimary(signed);
  await rpc.ownedSign(JSON.stringify({ order: { object_id: 'object-1' }, owner_pubkey_hex: publicKey, owner_signature_hex: publicSignature }), 'validator-0');
  await rpc.ownedApply(cert);
  await rpc.ownedUnwrapSign(JSON.stringify({ order: { object_id: 'object-1' }, owner_pubkey_hex: publicKey, owner_signature_hex: publicSignature }), 'validator-0');
  await rpc.ownedUnwrapApply(cert);
  rpc.close();

  const server = new SwapServer(location.origin, 'session-only-token');
  await server.fundNavswapPfusdc({ wallet_address: 'pf-browser-capture', amount_atoms: '1' });
  await server.runNavswap({ route: 'transparent_navswap', signed_transaction_json: signed });
  await server.prepareNavswapAction({ route: 'transparent_navswap', wallet_address: 'pf-browser-capture' });
  await server.runPrivateSwapWorkflow({ wallet_address: 'pf-browser-capture', proof_hex: 'aa55' });
  await server.submitShieldedNavswapIngress({ route: 'shielded_navswap', ingress_action_json: signed });
  await server.submitShieldedNavswapSwap({ route: 'shielded_navswap', swap_action_json: signed });
  await server.submitShieldedNavswapEgress({ route: 'shielded_navswap', egress_json: signed, disclosure_ack: true });

  await relayVaultDeposit({
    depositTxHash: '0x' + '33'.repeat(32),
    pftlRecipient: 'pf-browser-capture',
    amountAtoms: '1',
    routeProfileHash: '44'.repeat(48),
    routeEpoch: 1,
    routeBinding: '55'.repeat(32),
  });
  await fastSwapDemoApi.faucet('pf-browser-capture');
  await fastSwapDemoApi.swap('quote-browser-capture');
  await approveUsdc('0x2222222222222222222222222222222222222222', 1n);
  await depositToBridge(
    '0x3333333333333333333333333333333333333333',
    1n,
    'pf-browser-capture',
    '0x' + '66'.repeat(32),
    '0x' + '77'.repeat(32),
  );

  const storage = {
    local: Object.fromEntries(Object.keys(localStorage).map(key => [key, localStorage.getItem(key)])),
    session: Object.fromEntries(Object.keys(sessionStorage).map(key => [key, sessionStorage.getItem(key)])),
  };
  clearSensitiveMemory();
  return { storage, ethereumRequests };
};
</script>`;
}

async function startCaptureServer(capture) {
  const upgradeSockets = new Set();
  const server = createServer(async (request, response) => {
    const url = new URL(request.url, 'http://127.0.0.1');
    if (url.pathname === '/') {
      response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
      response.end(fixtureHtml());
      return;
    }
    if (url.pathname.startsWith('/src/lib/')) {
      const candidate = normalize(join(WALLET_ROOT, url.pathname));
      if (!candidate.startsWith(join(WALLET_ROOT, 'src/lib/'))) {
        response.writeHead(403).end();
        return;
      }
      try {
        response.writeHead(200, { 'content-type': 'text/javascript; charset=utf-8' });
        response.end(await readFile(candidate));
      } catch (_) {
        response.writeHead(404).end();
      }
      return;
    }
    const body = await readRequestBody(request);
    capture.http.push({ method: request.method, path: url.pathname, body });
    response.writeHead(200, { 'content-type': 'application/json' });
    response.end(JSON.stringify({ ok: true, result: { accepted: true }, receipt: { accepted: true, code: 'accepted' } }));
  });

  server.on('upgrade', (request, socket) => {
    upgradeSockets.add(socket);
    socket.once('close', () => upgradeSockets.delete(socket));
    const key = request.headers['sec-websocket-key'];
    const accept = createHash('sha1').update(`${key}${WEBSOCKET_GUID}`).digest('base64');
    socket.write([
      'HTTP/1.1 101 Switching Protocols',
      'Upgrade: websocket',
      'Connection: Upgrade',
      `Sec-WebSocket-Accept: ${accept}`,
      '',
      '',
    ].join('\r\n'));
    let pending = Buffer.alloc(0);
    socket.on('data', chunk => {
      pending = Buffer.concat([pending, chunk]);
      pending = consumeClientFrames(pending, text => {
        capture.websocket.push(text);
        const requestBody = JSON.parse(text);
        socket.write(websocketFrame(JSON.stringify({
          version: requestBody.version,
          id: requestBody.id,
          ok: true,
          result: { accepted: true },
          error: null,
          events: [],
        })));
      });
    });
  });

  await new Promise(resolveListen => server.listen(0, '127.0.0.1', resolveListen));
  server.destroyUpgradeSockets = () => {
    for (const socket of upgradeSockets) socket.destroy();
    upgradeSockets.clear();
  };
  return server;
}

async function scanTreeForValues(root, values) {
  const hits = [];
  async function visit(path) {
    const info = await stat(path);
    if (info.isDirectory()) {
      for (const entry of await readdir(path)) await visit(join(path, entry));
      return;
    }
    if (!info.isFile() || info.size > 16 * 1024 * 1024) return;
    const content = await readFile(path).catch(() => null);
    if (!content) return;
    for (const value of values) {
      if (content.includes(Buffer.from(value))) hits.push(path);
    }
  }
  await visit(root);
  return [...new Set(hits)];
}

async function scanChromiumProcessArguments(profileDir, values) {
  const procEntries = await readdir('/proc').catch(() => []);
  const chromiumCmdlines = [];
  for (const entry of procEntries) {
    if (!/^\d+$/.test(entry)) continue;
    const cmdline = await readFile(`/proc/${entry}/cmdline`, 'utf8').catch(() => '');
    if (cmdline.includes(profileDir)) chromiumCmdlines.push(cmdline);
  }
  const secretHits = chromiumCmdlines.filter(cmdline => values.some(value => cmdline.includes(value)));
  return { processCount: chromiumCmdlines.length, secretHits };
}

test('Chromium captures every wallet money boundary without custody material', { timeout: 60_000 }, async () => {
  const capture = { http: [], websocket: [] };
  const consoleLines = [];
  const profileDir = await mkdtemp(join(tmpdir(), 'postfiat-custody-browser-'));
  const seed = randomBytes(32).toString('hex');
  const backupJson = JSON.stringify({ schema: 'wallet-backup-v1', master_seed_hex: seed });
  const privateNote = Object.fromEntries([
    'diversifier',
    'g_d',
    'pk_d',
    'rho',
    'psi',
    'rcm',
    'nk',
    'rivk',
    'rseed',
    'spend_auth_signing_key',
    'full_viewing_key_hex',
  ].map(key => [key, randomBytes(32).toString('hex')]));
  const sensitiveValues = [seed, backupJson, ...Object.values(privateNote)];
  const server = await startCaptureServer(capture);
  const address = server.address();
  const context = await chromium.launchPersistentContext(profileDir, { headless: true });
  let acceptance = null;
  try {
    const page = await context.newPage();
    page.on('console', message => consoleLines.push(message.text()));
    await page.goto(`http://127.0.0.1:${address.port}/`);
    await page.waitForFunction(() => typeof window.runCustodyCapture === 'function');
    const browserResult = await page.evaluate(
      input => window.runCustodyCapture(input),
      { seed, backupJson, privateNote },
    );

    assert.equal(capture.websocket.length, 10, 'complete WebSocket money-operation catalog');
    assert.equal(capture.http.length, 10, 'complete HTTP money-operation catalog');
    assert.equal(browserResult.ethereumRequests.filter(item => item.method === 'eth_sendTransaction').length, 2);

    const outbound = JSON.stringify({ capture, ethereum: browserResult.ethereumRequests });
    assert.equal(outbound.includes(seed), false);
    assert.equal(outbound.includes(backupJson), false);
    for (const privateValue of Object.values(privateNote)) {
      assert.equal(outbound.includes(privateValue), false);
    }
    assert.match(outbound, /PUBLIC_SIGNATURE_CAPTURE_MARKER_/);
    assert.equal(JSON.stringify(browserResult.storage).includes(seed), false);
    assert.equal(JSON.stringify(browserResult.storage).includes(backupJson), false);
    for (const privateValue of Object.values(privateNote)) {
      assert.equal(JSON.stringify(browserResult.storage).includes(privateValue), false);
    }
    assert.equal(consoleLines.join('\n').includes(seed), false);
    assert.equal(consoleLines.join('\n').includes(backupJson), false);
    for (const privateValue of Object.values(privateNote)) {
      assert.equal(consoleLines.join('\n').includes(privateValue), false);
    }
    assert.equal(process.argv.join('\0').includes(seed), false);
    for (const privateValue of Object.values(privateNote)) {
      assert.equal(process.argv.join('\0').includes(privateValue), false);
    }
    const chromiumArguments = await scanChromiumProcessArguments(profileDir, sensitiveValues);
    assert.ok(chromiumArguments.processCount > 0, 'Chromium process command lines were found');
    assert.deepEqual(chromiumArguments.secretHits, []);
    acceptance = {
      schema: 'postfiat-browser-custody-boundary-acceptance-v2',
      accepted: true,
      browser: 'chromium-headless',
      websocket_money_operations: capture.websocket.map(raw => JSON.parse(raw).method),
      http_money_operations: capture.http.map(item => `${item.method} ${item.path}`),
      ethereum_money_operations: browserResult.ethereumRequests
        .filter(item => item.method === 'eth_sendTransaction')
        .map(item => item.method),
      public_signature_observed: outbound.includes('PUBLIC_SIGNATURE_CAPTURE_MARKER_'),
      custody_seed_observed: false,
      custody_backup_observed: false,
      private_note_marker_count: Object.keys(privateNote).length,
      private_note_marker_observed: false,
      browser_storage_secret_hits: 0,
      browser_console_secret_hits: 0,
      process_argv_secret_hits: 0,
      chromium_processes_scanned: chromiumArguments.processCount,
      proxy_ingress_secret_hits: 0,
    };
  } finally {
    await context.close();
    server.destroyUpgradeSockets();
    await new Promise(resolveClose => server.close(resolveClose));
  }

  const profileHits = await scanTreeForValues(profileDir, sensitiveValues);
  assert.deepEqual(profileHits, []);
  const crashArtifacts = (await readdir(profileDir, { recursive: true }))
    .filter(path => /(^|\/)(crash|crashes|crashpad)(\/|$)|\.(dmp|core)$/i.test(path));
  assert.deepEqual(crashArtifacts, []);
  acceptance.chromium_profile_secret_hits = profileHits.length;
  acceptance.crash_artifact_count = crashArtifacts.length;
  const reportDir = process.env.POSTFIAT_CUSTODY_REPORT_DIR;
  if (reportDir) {
    await mkdir(reportDir, { recursive: true });
    await writeFile(join(reportDir, 'ACCEPTANCE.json'), `${JSON.stringify(acceptance, null, 2)}\n`, {
      mode: 0o600,
    });
  }
  await rm(profileDir, { recursive: true, force: true });
});
