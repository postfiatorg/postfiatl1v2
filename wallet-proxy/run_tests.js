'use strict';

const { readdirSync } = require('node:fs');
const { spawnSync } = require('node:child_process');
const { join } = require('node:path');

const root = __dirname;
const tests = readdirSync(root)
  .filter((name) => /^test_.*\.js$/.test(name))
  .sort();

if (tests.length === 0) {
  throw new Error('wallet-proxy test discovery found no test_*.js files');
}

for (const test of tests) {
  const result = spawnSync(process.execPath, [join(root, test)], {
    cwd: join(root, '..'),
    encoding: 'utf8',
    stdio: 'pipe',
    timeout: 120_000,
  });
  if (result.status !== 0 || result.error) {
    process.stderr.write(result.stdout || '');
    process.stderr.write(result.stderr || '');
    throw result.error || new Error(`${test} failed with status ${result.status}`);
  }
  process.stdout.write(`PASS ${test}\n`);
}

process.stdout.write(`wallet-proxy regression suite passed (${tests.length}/${tests.length})\n`);
