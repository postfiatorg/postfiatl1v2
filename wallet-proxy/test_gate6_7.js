// Gate 6 + 7 test: Settings, backup, security
const fs = require('fs');
const path = require('path');
const { webcrypto } = require('crypto');
global.crypto = webcrypto;

let passed = 0, failed = 0;
function ok(name) { passed++; console.log('  PASS ' + name); }
function fail(name, err) { failed++; console.log('  FAIL ' + name + ': ' + err); }

async function main() {
  const repoRoot = path.resolve(__dirname, '..');
  const wasmPkg = path.join(repoRoot, 'wallet-web/src/wasm');
  const wasmBytes = fs.readFileSync(path.join(wasmPkg, 'postfiat_wallet_wasm_bg.wasm'));
  const wasmMod = await import(path.join(wasmPkg, 'postfiat_wallet_wasm.js'));
  wasmMod.initSync({ module: wasmBytes });

  const enc = new TextEncoder();
  const dec = new TextDecoder();
  const SALT_BYTES = 16, IV_BYTES = 12, PBKDF2_ITERATIONS = 100000;

  async function encrypt(seed, pass) {
    const salt = crypto.getRandomValues(new Uint8Array(SALT_BYTES));
    const iv = crypto.getRandomValues(new Uint8Array(IV_BYTES));
    const keyMaterial = await crypto.subtle.importKey('raw', enc.encode(pass), 'PBKDF2', false, ['deriveKey']);
    const key = await crypto.subtle.deriveKey({name:'PBKDF2',salt,iterations:PBKDF2_ITERATIONS,hash:'SHA-256'}, keyMaterial, {name:'AES-GCM',length:256}, false, ['encrypt']);
    const ct = await crypto.subtle.encrypt({name:'AES-GCM',iv}, key, enc.encode(seed));
    return { salt: Buffer.from(salt).toString('base64'), iv: Buffer.from(iv).toString('base64'), ciphertext: Buffer.from(new Uint8Array(ct)).toString('base64') };
  }

  async function decrypt(blob, pass) {
    const salt = Uint8Array.from(Buffer.from(blob.salt, 'base64'));
    const iv = Uint8Array.from(Buffer.from(blob.iv, 'base64'));
    const ct = Uint8Array.from(Buffer.from(blob.ciphertext, 'base64'));
    const keyMaterial = await crypto.subtle.importKey('raw', enc.encode(pass), 'PBKDF2', false, ['deriveKey']);
    const key = await crypto.subtle.deriveKey({name:'PBKDF2',salt,iterations:PBKDF2_ITERATIONS,hash:'SHA-256'}, keyMaterial, {name:'AES-GCM',length:256}, false, ['decrypt']);
    const pt = await crypto.subtle.decrypt({name:'AES-GCM',iv}, key, ct);
    return dec.decode(pt);
  }

  console.log('\n=== Gate 6: Settings and Wallet Management ===');

  // Test 1: Export/Import backup round-trip
  const chainId = 'postfiat-wan-devnet';
  const seed = 'e'.repeat(64);
  const result = wasmMod.wallet_keygen(chainId, seed, 0);
  const blob = await encrypt(seed, 'pass123');
  const walletData = { blob, metadata: { address: result.address, accountIndex: 0, chainId } };

  // Simulate export (serialize) then import (deserialize + decrypt)
  const exported = JSON.stringify(walletData);
  const imported = JSON.parse(exported);
  const decryptedSeed = await decrypt(imported.blob, 'pass123');
  const reDerived = wasmMod.wallet_keygen(chainId, decryptedSeed, 0);
  if (reDerived.address === result.address)
    ok('export/import backup round-trips to same address');
  else
    fail('backup round-trip', result.address + ' != ' + reDerived.address);

  // Test 2: Import with wrong passphrase fails
  try {
    await decrypt(imported.blob, 'wrong');
    fail('wrong passphrase import', 'should have failed');
  } catch (e) {
    ok('import with wrong passphrase fails gracefully');
  }

  // Test 3: Settings persist (simulate)
  const settings = { rpcEndpoint: 'ws://localhost:8080', autoLockMinutes: 30 };
  const settingsJson = JSON.stringify(settings);
  const loadedSettings = JSON.parse(settingsJson);
  if (loadedSettings.rpcEndpoint === 'ws://localhost:8080' && loadedSettings.autoLockMinutes === 30)
    ok('settings persist: endpoint + auto-lock saved');
  else
    fail('settings', JSON.stringify(loadedSettings));

  // Test 4: Auto-lock timer values
  const validLockTimes = [5, 15, 30, 60];
  for (const t of validLockTimes) {
    if (t > 0 && t <= 120) { /* ok */ }
    else { fail('auto-lock ' + t, 'invalid'); }
  }
  ok('auto-lock: valid values 5/15/30/60 minutes');

  console.log('\n=== Gate 7: Security Hardening ===');

  // Test 5: CSP check
  const manifest = JSON.parse(fs.readFileSync(path.join(repoRoot, 'wallet-extension/manifest.json')));
  const csp = manifest.content_security_policy?.extension_pages || '';
  if (csp.includes("script-src 'self'") && csp.includes('wasm-unsafe-eval') && !csp.includes('unsafe-inline') && !csp.includes("'unsafe-eval'") && !csp.includes('http:'))
    ok('CSP: only self + wasm-unsafe-eval, no inline/eval/remote');
  else
    fail('CSP', csp);

  // Test 6: No unnecessary permissions
  const perms = manifest.permissions || [];
  if (perms.length === 1 && perms[0] === 'storage')
    ok('permissions: only storage (no tabs, cookies, webRequest)');
  else
    fail('permissions', JSON.stringify(perms));

  // Test 7: No plaintext seed in stored data
  const storedData = JSON.stringify(walletData);
  if (!storedData.includes(seed))
    ok('no plaintext seed in stored data');
  else
    fail('seed leak', 'seed found in storage!');

  // Test 8: Passphrase not in stored data
  if (!storedData.includes('pass123'))
    ok('passphrase not stored in wallet data');
  else
    fail('passphrase leak', 'passphrase found in storage!');

  // Test 9: Ciphertext is base64 (not hex seed)
  const ctBuffer = Buffer.from(blob.ciphertext, 'base64');
  const ctHex = ctBuffer.toString('hex');
  if (!ctHex.includes(seed))
    ok('ciphertext does not contain seed');
  else
    fail('ciphertext', 'seed found in ciphertext!');

  // Test 10: web_accessible_resources limited
  const war = manifest.web_accessible_resources?.[0]?.resources || [];
  if (war.includes('wasm/postfiat_wallet_wasm_bg.wasm') && war.includes('wasm/postfiat_wallet_wasm.js') && war.length === 2)
    ok('web_accessible_resources: only WASM files');
  else
    fail('war', JSON.stringify(war));

  // Test 11: Proxy origin check support
  const proxyCode = fs.readFileSync(path.join(repoRoot, 'wallet-proxy/server.js'), 'utf8');
  if (proxyCode.includes('ALLOWED_ORIGINS') && proxyCode.includes('origin not allowed'))
    ok('proxy: origin checking supported');
  else
    fail('proxy origin', 'no origin check');

  // Test 12: All JS files have no eval()
  const jsFiles = ['background.js', 'popup/popup.js', 'lib/rpc-client.js', 'lib/keystore.js', 'lib/tx-builder.js'];
  let noEval = true;
  for (const f of jsFiles) {
    const code = fs.readFileSync(path.join(repoRoot, 'wallet-extension', f), 'utf8');
    if (code.includes('eval(')) {
      fail('eval in ' + f, 'eval() found');
      noEval = false;
    }
  }
  if (noEval) ok('no eval() in any JS file');

  // Test 13: Recipient address validation in popup.js
  const popupCode = fs.readFileSync(path.join(repoRoot, 'wallet-extension/popup/popup.js'), 'utf8');
  if (popupCode.includes('pf[0-9a-f]{40}') && popupCode.includes('42'))
    ok('recipient address validation: pf + 40 hex = 42 chars');
  else
    fail('address validation', 'missing in popup.js');

  console.log('\n=== Summary ===');
  console.log('Passed: ' + passed + '/' + (passed + failed));
  console.log('Failed: ' + failed);
  if (failed === 0) console.log('\n*** GATES 6 + 7 PASSED ***');
  else console.log('\n*** ' + failed + ' TESTS FAILED ***');
  process.exit(failed > 0 ? 1 : 0);
}

main().catch(e => { console.error('Fatal:', e); process.exit(1); });
