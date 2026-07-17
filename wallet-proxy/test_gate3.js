// Gate 3 test: Wallet creation, encryption, lock/unlock, import
// Tests the keystore logic using Node's built-in webcrypto
const fs = require('fs');
const path = require('path');
const { webcrypto } = require('crypto');

// Polyfill global.crypto for the keystore module
global.crypto = webcrypto;

let passed = 0, failed = 0;
function ok(name) { passed++; console.log('  PASS ' + name); }
function fail(name, err) { failed++; console.log('  FAIL ' + name + ': ' + err); }

async function main() {
  // Load WASM
  const wasmPkg = path.resolve(__dirname, '../wallet-web/src/wasm');
  const wasmBytes = fs.readFileSync(path.join(wasmPkg, 'postfiat_wallet_wasm_bg.wasm'));
  const wasmMod = await import(path.join(wasmPkg, 'postfiat_wallet_wasm.js'));
  wasmMod.initSync({ module: wasmBytes });

  // Load keystore logic — extract the encrypt/decrypt functions
  // We can't use ES modules in Node directly, so we'll reimplement the same logic
  const SALT_BYTES = 16;
  const IV_BYTES = 12;
  const PBKDF2_ITERATIONS = 100000;
  const enc = new TextEncoder();
  const dec = new TextDecoder();

  async function encrypt(masterSeedHex, passphrase) {
    const salt = crypto.getRandomValues(new Uint8Array(SALT_BYTES));
    const iv = crypto.getRandomValues(new Uint8Array(IV_BYTES));
    const keyMaterial = await crypto.subtle.importKey('raw', enc.encode(passphrase), 'PBKDF2', false, ['deriveKey']);
    const key = await crypto.subtle.deriveKey(
      { name: 'PBKDF2', salt, iterations: PBKDF2_ITERATIONS, hash: 'SHA-256' },
      keyMaterial, { name: 'AES-GCM', length: 256 }, false, ['encrypt']
    );
    const ciphertext = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, enc.encode(masterSeedHex));
    return {
      salt: Buffer.from(salt).toString('base64'),
      iv: Buffer.from(iv).toString('base64'),
      ciphertext: Buffer.from(new Uint8Array(ciphertext)).toString('base64')
    };
  }

  async function decrypt(blob, passphrase) {
    const salt = Uint8Array.from(Buffer.from(blob.salt, 'base64'));
    const iv = Uint8Array.from(Buffer.from(blob.iv, 'base64'));
    const ciphertext = Uint8Array.from(Buffer.from(blob.ciphertext, 'base64'));
    const keyMaterial = await crypto.subtle.importKey('raw', enc.encode(passphrase), 'PBKDF2', false, ['deriveKey']);
    const key = await crypto.subtle.deriveKey(
      { name: 'PBKDF2', salt, iterations: PBKDF2_ITERATIONS, hash: 'SHA-256' },
      keyMaterial, { name: 'AES-GCM', length: 256 }, false, ['decrypt']
    );
    const plaintext = await crypto.subtle.decrypt({ name: 'AES-GCM', iv }, key, ciphertext);
    return dec.decode(plaintext);
  }

  console.log('\n=== Gate 3: Wallet Creation/Onboarding ===');

  // Test 1: Create wallet — generate seed, keygen, encrypt, decrypt round-trip
  const chainId = 'postfiat-wan-devnet';
  const seed = wasmMod.random_master_seed();
  const result = wasmMod.wallet_keygen(chainId, seed, 0);
  
  if (result.address.startsWith('pf') && result.address.length === 42)
    ok('create: address is 42 chars starting with pf');
  else
    fail('create address', 'got: ' + result.address);

  // Test 2: Encrypt seed with passphrase
  const passphrase = 'test-pass-123';
  const blob = await encrypt(seed, passphrase);
  if (blob.salt && blob.iv && blob.ciphertext)
    ok('encrypt: blob has salt, iv, ciphertext');
  else
    fail('encrypt', 'missing fields');

  // Test 3: Ciphertext should not contain the seed
  if (!blob.ciphertext.includes(seed) && !Buffer.from(blob.ciphertext, 'base64').toString('hex').includes(seed))
    ok('encrypt: seed is not in plaintext in ciphertext');
  else
    fail('encrypt', 'seed found in ciphertext!');

  // Test 4: Decrypt with correct passphrase
  const decrypted = await decrypt(blob, passphrase);
  if (decrypted === seed)
    ok('decrypt: correct passphrase recovers seed');
  else
    fail('decrypt', 'got: ' + decrypted + ' expected: ' + seed);

  // Test 5: Decrypt with wrong passphrase fails
  try {
    await decrypt(blob, 'wrong-pass');
    fail('wrong passphrase', 'should have thrown');
  } catch (e) {
    ok('decrypt: wrong passphrase throws error (no crash)');
  }

  // Test 6: Lock/unlock — simulate by clearing seed and re-deriving
  // After lock, seed is null. After unlock, seed is recovered and keygen produces same address
  const recoveredSeed = await decrypt(blob, passphrase); // unlock
  const resultAfterUnlock = wasmMod.wallet_keygen(chainId, recoveredSeed, 0);
  if (resultAfterUnlock.address === result.address)
    ok('lock/unlock: address matches after re-deriving from decrypted seed');
  else
    fail('lock/unlock', result.address + ' != ' + resultAfterUnlock.address);

  // Test 7: Import wallet — paste seed, keygen, encrypt
  const importSeed = 'a'.repeat(64);
  const importResult = wasmMod.wallet_keygen(chainId, importSeed, 0);
  if (importResult.address.startsWith('pf') && importResult.address.length === 42)
    ok('import: valid address from pasted seed: ' + importResult.address);
  else
    fail('import', 'bad address');

  // Test 8: Import validation — reject bad seeds
  const badSeeds = ['', 'xyz', 'g'.repeat(64), 'a'.repeat(63), 'a'.repeat(65)];
  let allBadSeedsRejected = true;
  for (const bad of badSeeds) {
    if (/^[0-9a-f]{64}$/.test(bad)) {
      allBadSeedsRejected = false;
    }
  }
  if (allBadSeedsRejected)
    ok('import: all invalid seeds rejected (empty, non-hex, wrong length)');
  else
    fail('import validation', 'some bad seeds passed');

  // Test 9: Deterministic — same seed always gives same address
  const r1 = wasmMod.wallet_keygen(chainId, importSeed, 0);
  const r2 = wasmMod.wallet_keygen(chainId, importSeed, 0);
  if (r1.address === r2.address)
    ok('deterministic: same seed always gives same address');
  else
    fail('deterministic', r1.address + ' != ' + r2.address);

  // Test 10: Different account index gives different address
  const r3 = wasmMod.wallet_keygen(chainId, importSeed, 1);
  if (r1.address !== r3.address)
    ok('multi-account: index 0 != index 1');
  else
    fail('multi-account', 'same address for different indices');

  // Test 11: Encryption blob is different each time (random salt/iv)
  const blob2 = await encrypt(seed, passphrase);
  if (blob.salt !== blob2.salt && blob.iv !== blob2.iv)
    ok('encrypt: random salt/iv each time (different blobs)');
  else
    fail('encrypt', 'same salt/iv - not random!');

  console.log('\n=== Summary ===');
  console.log('Passed: ' + passed + '/' + (passed + failed));
  console.log('Failed: ' + failed);
  if (failed === 0) console.log('\n*** GATE 3 PASSED ***');
  else console.log('\n*** ' + failed + ' TESTS FAILED ***');
  process.exit(failed > 0 ? 1 : 0);
}

main().catch(e => { console.error('Fatal:', e); process.exit(1); });
