const WebSocket = require('ws');
const fs = require('fs');
const path = require('path');
const { startProxyFixture } = require('./proxy-test-fixture');

const repoRoot = path.resolve(__dirname, '..');
const wasmPath = path.join(repoRoot, 'wallet-web/src/wasm/postfiat_wallet_wasm_bg.wasm');
const wasmBytes = fs.readFileSync(wasmPath);

let passed = 0, failed = 0;
function ok(name) { passed++; console.log('  PASS ' + name); }
function fail(name, err) { failed++; console.log('  FAIL ' + name + ': ' + err); }

async function main() {
  const wasmMod = await import(path.join(repoRoot, 'wallet-web/src/wasm/postfiat_wallet_wasm.js'));
  wasmMod.initSync({ module: wasmBytes });

  console.log('\n=== Gate 0: WASM Core ===');

  // 1. Random seed
  const seed = wasmMod.random_master_seed();
  if (seed.length === 64 && /^[0-9a-f]{64}$/.test(seed)) ok('random_master_seed: 64 hex chars');
  else fail('random_master_seed', 'got: ' + seed);

  // 2. Keygen address
  const chainId = 'postfiat-wan-devnet';
  const result = wasmMod.wallet_keygen(chainId, seed, 0);
  if (result.address && result.address.startsWith('pf') && result.address.length === 42)
    ok('wallet_keygen: 42-char address: ' + result.address);
  else
    fail('wallet_keygen address', 'got: ' + result.address + ' len=' + (result.address?.length || 0));

  // 3. Public key size
  if (result.public_key_hex && result.public_key_hex.length === 3904)
    ok('public_key_hex: 3904 hex chars (1952 bytes)');
  else
    fail('public_key_hex', 'len=' + (result.public_key_hex?.length || 0));

  // 4. Backup JSON valid
  let backup;
  try {
    backup = JSON.parse(result.backup_json);
    if (backup.chain_id === chainId) ok('backup_json: valid, chain_id matches');
    else fail('backup chain_id', 'got: ' + backup.chain_id);
  } catch (e) { fail('backup_json parse', e.message); }

  // 5. Deterministic
  const result2 = wasmMod.wallet_keygen(chainId, seed, 0);
  if (result.address === result2.address) ok('deterministic: same seed = same address');
  else fail('deterministic', result.address + ' != ' + result2.address);

  // 6. Different seed
  const seed2 = wasmMod.random_master_seed();
  const result3 = wasmMod.wallet_keygen(chainId, seed2, 0);
  if (result.address !== result3.address) ok('different seeds = different addresses');
  else fail('different seeds', 'same address!');

  // 7. make_rpc_request
  const reqJson = wasmMod.make_rpc_request('status', '{}');
  const req = JSON.parse(reqJson);
  if (req.version === 'postfiat-local-rpc-v1' && typeof req.id === 'string' && req.method === 'status')
    ok('make_rpc_request: version=v1, id=string, method=status');
  else
    fail('make_rpc_request', JSON.stringify(req));

  // 8. parse_rpc_response
  const mockResp = JSON.stringify({
    version: 'postfiat-local-rpc-v1', id: 'test', ok: true,
    result: { block_height: 470 }, error: null, events: []
  });
  const parsed = wasmMod.parse_rpc_response(mockResp);
  if (parsed.ok === true && parsed.result && parsed.result.block_height === 470 && parsed.error === null)
    ok('parse_rpc_response: ok=true, block_height=470, error=null');
  else
    fail('parse_rpc_response', JSON.stringify(parsed));

  // 9. Signing with correct TransferFeeQuoteSummary fields
  console.log('\n=== Gate 0: WASM Signing ===');
  const quote = {
    chain_id: chainId,
    genesis_hash: '231b1cfb63439c23bdcc3f7ea2f7f3ce7a53f9abffef8f720f47421b575f16e7f2d9ad5e61298207be2e9ce08743f870',
    protocol_version: 1,
    from: result.address,
    to: 'pf00000000000000000000000000000000000000',
    amount: 1,
    sequence: 1,
    sequence_source: 'account',
    sender_balance: 1000000,
    sender_sequence: 1,
    mempool_pending_for_sender: 0,
    recipient_exists: false,
    will_create_recipient_account: true,
    base_transfer_fee: 1,
    state_expansion_fee: 0,
    minimum_fee: 1,
    account_reserve: 0,
    transfer_account_creation_fee: 0,
    transfer_fee_byte_quantum: 1,
    transfer_fee_per_quantum: 0,
    transfer_weight_bytes: 100,
    sender_balance_after_amount_and_fee: 999998,
    sender_meets_reserve_after_transfer: true,
    recipient_balance_after_amount: null,
    recipient_meets_reserve_after_transfer: false
  };
  const signed = wasmMod.wallet_sign_transfer(result.backup_json, JSON.stringify(quote));
  if (!signed) { fail('wallet_sign_transfer', 'returned null'); }
  else {
    const sigHex = signed.signature_hex || signed.signature;
    if (sigHex && sigHex.length === 6618) ok('signature: 6618 hex chars (3309 bytes)');
    else fail('signature size', 'len=' + (sigHex?.length || 0) + ' expected 6618');

    const pubKey = signed.public_key_hex || signed.public_key;
    if (pubKey && pubKey.length === 3904) ok('signed public_key: 3904 hex chars');
    else fail('signed pub_key', 'len=' + (pubKey?.length || 0));

    if (signed.unsigned && signed.unsigned.from === result.address) ok('signed from address matches keygen');
    else fail('from match', 'got: ' + (signed.unsigned?.from || 'undefined'));
  }

  // Gate 1: RPC via proxy
  console.log('\n=== Gate 1: RPC Proxy ===');
  const fixture = await startProxyFixture();
  const ws = new WebSocket(`ws://127.0.0.1:${fixture.port}`);

  await new Promise((resolve) => {
    let settled = false;
    const finish = () => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve();
    };
    ws.on('open', async () => {
      ws.send(JSON.stringify({version:'postfiat-local-rpc-v1',id:'t-status',method:'status',params:{}}));
      const statusResp = await new Promise(r => ws.once('message', d => r(JSON.parse(d.toString()))));
      if (statusResp.ok && statusResp.result && statusResp.result.block_height >= 470)
        ok('RPC status: height=' + statusResp.result.block_height + ' chain=' + statusResp.result.chain_id);
      else
        fail('RPC status', JSON.stringify(statusResp).slice(0, 200));

      ws.send(JSON.stringify({version:'postfiat-local-rpc-v1',id:'t-fee',method:'fee',params:{}}));
      const feeResp = await new Promise(r => ws.once('message', d => r(JSON.parse(d.toString()))));
      if (feeResp.ok && feeResp.result && (feeResp.result.account_reserve !== undefined || feeResp.result.minimum_fee !== undefined))
        ok('RPC fee: account_reserve=' + feeResp.result.account_reserve + ' burned=' + feeResp.result.burned_fee_total);
      else
        fail('RPC fee', JSON.stringify(feeResp).slice(0, 200));

      ws.send(JSON.stringify({version:'postfiat-local-rpc-v1',id:'t-vals',method:'validators',params:{}}));
      const valsResp = await new Promise(r => ws.once('message', d => r(JSON.parse(d.toString()))));
      if (valsResp.ok && valsResp.result) {
        const count = valsResp.result.validator_count || (Array.isArray(valsResp.result.validators) ? valsResp.result.validators.length : 0);
        ok('RPC validators: ' + count + ' validators, chain=' + valsResp.result.chain_id);
      } else
        fail('RPC validators', JSON.stringify(valsResp).slice(0, 200));

      ws.close();
      finish();
    });
    ws.on('error', (e) => { fail('WS proxy', e.message); finish(); });
    const timer = setTimeout(() => { fail('proxy timeout', ''); ws.close(); finish(); }, 15000);
  });
  await fixture.close();

  // Gate 2: Extension skeleton
  console.log('\n=== Gate 2: Extension Skeleton ===');
  const extDir = path.join(repoRoot, 'wallet-extension');
  const requiredFiles = ['manifest.json', 'background.js', 'popup/popup.html', 'popup/popup.js',
    'lib/rpc-client.js', 'lib/keystore.js', 'lib/tx-builder.js',
    'wasm/postfiat_wallet_wasm_bg.wasm', 'wasm/postfiat_wallet_wasm.js',
    'icons/icon16.png', 'icons/icon48.png', 'icons/icon128.png'];
  let allPresent = true;
  for (const f of requiredFiles) {
    if (!fs.existsSync(path.join(extDir, f))) {
      fail('file exists: ' + f, 'missing');
      allPresent = false;
    }
  }
  if (allPresent) ok('all ' + requiredFiles.length + ' extension files present');

  // Manifest checks
  const manifest = JSON.parse(fs.readFileSync(path.join(extDir, 'manifest.json')));
  if (manifest.manifest_version === 3 && manifest.permissions.includes('storage') &&
      manifest.content_security_policy.extension_pages.includes('wasm-unsafe-eval'))
    ok('manifest.json: MV3, storage permission, wasm-unsafe-eval CSP');
  else
    fail('manifest.json', JSON.stringify(manifest));

  if (manifest.web_accessible_resources && manifest.web_accessible_resources[0].resources.includes('wasm/postfiat_wallet_wasm_bg.wasm'))
    ok('web_accessible_resources: WASM files accessible');
  else
    fail('web_accessible_resources', 'missing wasm entries');

  // JS syntax checks
  const { execSync } = require('child_process');
  const jsFiles = ['background.js', 'popup/popup.js', 'lib/rpc-client.js', 'lib/keystore.js', 'lib/tx-builder.js'];
  let allSyntaxOk = true;
  for (const f of jsFiles) {
    try {
      execSync('node --check ' + path.join(extDir, f), { stdio: 'pipe' });
    } catch (e) {
      fail('syntax: ' + f, (e.stderr?.toString() || e.message));
      allSyntaxOk = false;
    }
  }
  if (allSyntaxOk) ok('all 5 JS files pass syntax check');

  console.log('\n=== Summary ===');
  console.log('Passed: ' + passed + '/' + (passed + failed));
  console.log('Failed: ' + failed);
  if (failed === 0) console.log('\n*** ALL TESTS PASSED ***');
  else console.log('\n*** ' + failed + ' TESTS FAILED ***');
  process.exit(failed > 0 ? 1 : 0);
}

main().catch(e => { console.error('Fatal:', e); process.exit(1); });
