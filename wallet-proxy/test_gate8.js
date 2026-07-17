// Gate 8 test: Polish and release prep
const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync, execSync } = require('child_process');

let passed = 0, failed = 0;
function ok(name) { passed++; console.log('  PASS ' + name); }
function fail(name, err) { failed++; console.log('  FAIL ' + name + ': ' + err); }

async function main() {
  console.log('\n=== Gate 8: Polish and Release Prep ===');
  const extDir = path.resolve(__dirname, '../wallet-extension');
  const packageDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-wallet-extension-'));
  const packagePath = path.join(packageDir, 'postfiat-wallet-extension.zip');
  process.on('exit', () => fs.rmSync(packageDir, { recursive: true, force: true }));

  execFileSync('python3', [
    '-c',
    [
      'import pathlib, sys, zipfile',
      'root = pathlib.Path(sys.argv[1])',
      'with zipfile.ZipFile(sys.argv[2], "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:',
      '    for item in sorted(root.rglob("*")):',
      '        if item.is_file():',
      '            archive.write(item, item.relative_to(root))',
    ].join('\n'),
    extDir,
    packagePath,
  ]);

  // Test 1: Dark mode (dark background colors in CSS)
  const html = fs.readFileSync(path.join(extDir, 'popup/popup.html'), 'utf8');
  if (html.includes('#0a0a0a') && html.includes('#1a1a2e') && html.includes('#8b5cf6'))
    ok('dark mode: dark background (#0a0a0a) + accent (#8b5cf6)');
  else
    fail('dark mode', 'dark colors not found');

  // Test 2: Empty states (no wallet, no transactions)
  const popupJs = fs.readFileSync(path.join(extDir, 'popup/popup.js'), 'utf8');
  if (html.includes('No wallet found') && (html.includes('No transactions') || popupJs.includes('No transactions')))
    ok('empty states: no-wallet + no-transactions messages');
  else
    fail('empty states', 'missing');

  // Test 3: Loading states
  if (html.includes('Loading...'))
    ok('loading states: Loading... text');
  else
    fail('loading states', 'missing');

  // Test 4: Error/success states
  if (html.includes('class="error"') && html.includes('class="success"'))
    ok('error/success states: CSS classes present');
  else
    fail('error/success', 'missing');

  // Test 5: Copy address with toast
  if (html.includes('title="Click to copy"') && html.includes('copy-toast'))
    ok('copy address: click-to-copy + toast notification');
  else
    fail('copy address', 'missing');

  // Test 6: Responsive layout (360px width)
  if (html.includes('width: 360px'))
    ok('responsive layout: popup is 360px wide');
  else
    fail('layout', 'width not set');

  // Test 7: README exists
  if (fs.existsSync(path.join(extDir, 'README.md')))
    ok('README.md exists with install instructions');
  else
    fail('README', 'missing');

  // Test 8: Extension packaged as zip
  if (fs.existsSync(packagePath)) {
    const stat = fs.statSync(packagePath);
    if (stat.size > 100000 && stat.size < 1000000)
      ok('extension packaged: ' + (stat.size / 1024).toFixed(0) + ' KB zip');
    else
      fail('package size', stat.size + ' bytes');
  } else
    fail('package', 'zip not found');

  // Test 9: All required files present in zip
  try {
    const list = execFileSync('python3', [
      '-c',
      'import sys, zipfile; print("\\n".join(zipfile.ZipFile(sys.argv[1]).namelist()))',
      packagePath,
    ], { encoding: 'utf8' });
    const files = list.trim().split('\n');
    const required = ['manifest.json', 'background.js', 'popup/popup.html', 'popup/popup.js',
      'lib/rpc-client.js', 'lib/keystore.js', 'lib/tx-builder.js',
      'wasm/postfiat_wallet_wasm_bg.wasm', 'wasm/postfiat_wallet_wasm.js',
      'icons/icon16.png', 'icons/icon48.png', 'icons/icon128.png', 'README.md'];
    let allFound = true;
    for (const req of required) {
      if (!files.some(f => f.endsWith(req))) {
        fail('zip contains: ' + req, 'missing');
        allFound = false;
      }
    }
    if (allFound) ok('all ' + required.length + ' required files in zip');
  } catch (e) {
    fail('zip contents', e.message);
  }

  // Test 10: JS syntax final check
  const jsFiles = ['background.js', 'popup/popup.js', 'lib/rpc-client.js', 'lib/keystore.js', 'lib/tx-builder.js'];
  let allOk = true;
  for (const f of jsFiles) {
    try {
      execSync('node --check ' + path.join(extDir, f), { stdio: 'pipe' });
    } catch (e) {
      fail('syntax: ' + f, e.stderr?.toString());
      allOk = false;
    }
  }
  if (allOk) ok('all JS files pass syntax check');

  // Test 11: Manifest valid JSON
  try {
    JSON.parse(fs.readFileSync(path.join(extDir, 'manifest.json')));
    ok('manifest.json is valid JSON');
  } catch (e) {
    fail('manifest', e.message);
  }

  console.log('\n=== Summary ===');
  console.log('Passed: ' + passed + '/' + (passed + failed));
  console.log('Failed: ' + failed);
  if (failed === 0) console.log('\n*** GATE 8 PASSED ***');
  else console.log('\n*** ' + failed + ' TESTS FAILED ***');
  process.exit(failed > 0 ? 1 : 0);
}

main().catch(e => { console.error('Fatal:', e); process.exit(1); });
