#!/usr/bin/env node
import { execFile } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';

const execFileAsync = promisify(execFile);
const ROOT = path.resolve(new URL('..', import.meta.url).pathname);
const SCRIPT = path.join(ROOT, 'scripts', 'navswap-redaction-check.mjs');

async function run(args) {
  try {
    const result = await execFileAsync('node', [SCRIPT, ...args], {
      cwd: ROOT,
      maxBuffer: 2 * 1024 * 1024,
    });
    return { ok: true, stdout: result.stdout, stderr: result.stderr };
  } catch (error) {
    return {
      ok: false,
      stdout: error.stdout || '',
      stderr: error.stderr || '',
      code: error.code,
    };
  }
}

async function main() {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'navswap-redaction-test-'));
  try {
    const cleanDir = path.join(root, 'clean-public');
    await fs.mkdir(cleanDir, { mode: 0o755 });
    await fs.writeFile(
      path.join(cleanDir, 'summary.json'),
      `${JSON.stringify({ ok: true, wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468' })}\n`,
    );
    const clean = await run(['--repo-only', '--artifact-dir', cleanDir, '--json']);
    if (!clean.ok) throw new Error(`expected clean fixture to pass: ${clean.stderr}`);
    const cleanReport = JSON.parse(clean.stdout);
    if (cleanReport.finding_count !== 0) throw new Error('clean fixture had redaction findings');

    const publicSecretDir = path.join(root, 'public-secret');
    await fs.mkdir(publicSecretDir, { mode: 0o755 });
    await fs.writeFile(
      path.join(publicSecretDir, 'wallet.backup.json'),
      '{"master_seed_hex":"0123456789abcdef0123456789abcdef"}\n',
    );
    const publicSecret = await run(['--repo-only', '--artifact-dir', publicSecretDir, '--json']);
    if (publicSecret.ok) throw new Error('expected public secret fixture to fail');
    const publicReport = JSON.parse(publicSecret.stdout);
    if (publicReport.finding_count < 1) throw new Error('public secret fixture did not report findings');
    if (publicSecret.stdout.includes('0123456789abcdef0123456789abcdef')) {
      throw new Error('redaction report echoed the detected secret value');
    }

    const privateOpeningDir = path.join(root, 'private-note-opening');
    await fs.mkdir(privateOpeningDir, { mode: 0o755 });
    const noteOpening = 'ab'.repeat(32);
    await fs.writeFile(
      path.join(privateOpeningDir, 'note.json'),
      `${JSON.stringify({ rho: noteOpening, psi: 'cd'.repeat(32), rcm: 'ef'.repeat(32) })}\n`,
    );
    const privateOpening = await run(['--repo-only', '--artifact-dir', privateOpeningDir, '--json']);
    if (privateOpening.ok) throw new Error('expected private note opening fixture to fail');
    if (privateOpening.stdout.includes(noteOpening)) {
      throw new Error('redaction report echoed the detected note opening');
    }

    const completeOpeningDir = path.join(root, 'complete-note-opening');
    await fs.mkdir(completeOpeningDir, { mode: 0o755 });
    await fs.writeFile(
      path.join(completeOpeningDir, 'complete-note.json'),
      `${JSON.stringify({
        diversifier: '01'.repeat(11),
        g_d: '02'.repeat(32),
        pk_d: '03'.repeat(32),
        nk: '04'.repeat(32),
        rivk: '05'.repeat(32),
      })}\n`,
    );
    const completeOpening = await run(['--repo-only', '--artifact-dir', completeOpeningDir, '--json']);
    if (completeOpening.ok) throw new Error('expected complete private-note fixture to fail');

    const privateSecretDir = path.join(root, 'private-secret');
    await fs.mkdir(privateSecretDir, { mode: 0o700 });
    await fs.writeFile(
      path.join(privateSecretDir, 'wallet.backup.json'),
      '{"master_seed_hex":"fedcba9876543210fedcba9876543210"}\n',
    );
    const privateSecret = await run(['--repo-only', '--artifact-dir', privateSecretDir, '--json']);
    if (!privateSecret.ok) throw new Error(`expected private fixture to be skipped: ${privateSecret.stderr}`);
    const privateReport = JSON.parse(privateSecret.stdout);
    if (privateReport.skipped_private_count !== 1) throw new Error('private fixture was not skipped');

    const privateIncluded = await run(['--repo-only', '--include-private', '--artifact-dir', privateSecretDir, '--json']);
    if (privateIncluded.ok) throw new Error('expected include-private fixture to fail');

    process.stdout.write('navswap redaction check tests passed\n');
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
}

main().catch((error) => {
  process.stderr.write(`${error.message || error}\n`);
  process.exitCode = 1;
});
