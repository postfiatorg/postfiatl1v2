#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';

const ROOT = path.resolve(new URL('..', import.meta.url).pathname);
const DEFAULT_DOC_TARGETS = [
  'docs/plans/trustless-navswap-wallet-integration-spec.md',
  'docs/plans/proper-private-nav-swap-plan.md',
  'docs/status/navswap-wallet-overnight-2026-06-29.md',
];
const DEFAULT_TMP_PREFIX = '/tmp/navswap-';
const MAX_FILE_BYTES = 5 * 1024 * 1024;

const DENY_PATTERNS = [
  {
    id: 'pem_private_key',
    regex: /BEGIN [A-Z0-9 ]*PRIVATE KEY/g,
    message: 'PEM private key block',
  },
  {
    id: 'secret_json_field',
    regex: /["']?(master_seed_hex|spending_key_hex|full_viewing_key_hex|spend_auth_signing_key|secret_key|private_key|mnemonic|passphrase|backup_json|rseed)["']?\s*[:=]\s*["'][^"'\r\n]{8,}["']/gi,
    message: 'secret-bearing JSON/key field with value',
  },
  {
    id: 'private_note_opening',
    regex: /["']?(diversifier|g_d|pk_d|rho|psi|rcm|nk|rivk)["']?\s*[:=]\s*["'][0-9a-f]{22,}["']/gi,
    message: 'private note-opening field with value',
  },
  {
    id: 'secret_path_value',
    regex: /["']?(key_file|key-file|wallet_backup_file|wallet-backup-file|backup_file|backup-file)["']?\s*[:=]\s*["'][^"'\r\n]*(\.key\.json|wallet\.backup\.json|master-seed\.hex)[^"'\r\n]*["']/gi,
    message: 'secret-bearing file path value',
  },
  {
    id: 'absolute_secret_artifact_path',
    regex: /\/(?:home|tmp)\/[^\s"'`<>]*(?:wallet\.backup\.json|wallet\.key\.json|master-seed\.hex|\.key\.json)/g,
    message: 'absolute path to wallet/key artifact',
  },
];

function usage() {
  return `Usage:
  node scripts/navswap-redaction-check.mjs [--repo-only] [--include-private] [--artifact-dir DIR] [--json]

Scans NAVSwap public docs and public-readable NAVSwap evidence artifacts for
secret-bearing fields. Non-public directories are skipped by default.

Options:
  --repo-only          Scan only committed NAVSwap docs.
  --include-private   Also scan private directories such as chmod 700 evidence.
  --artifact-dir DIR   Additional artifact directory or file to scan. Repeatable.
  --json              Emit a machine-readable report.
`;
}

function parseArgs(argv) {
  const args = {
    repoOnly: false,
    includePrivate: false,
    artifactDirs: [],
    json: false,
    help: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else if (arg === '--repo-only') {
      args.repoOnly = true;
    } else if (arg === '--include-private') {
      args.includePrivate = true;
    } else if (arg === '--json') {
      args.json = true;
    } else if (arg === '--artifact-dir') {
      const value = argv[i + 1];
      if (!value || value.startsWith('--')) throw new Error('--artifact-dir requires a value');
      args.artifactDirs.push(value);
      i += 1;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  return args;
}

function isPublicMode(mode) {
  return (mode & 0o077) !== 0;
}

function statOrNull(file) {
  try {
    return fs.statSync(file);
  } catch (_) {
    return null;
  }
}

function walk(target, options, out = []) {
  const st = statOrNull(target);
  if (!st) return out;
  if (st.isDirectory()) {
    if (!options.includePrivate && !isPublicMode(st.mode)) {
      options.skippedPrivate.push(target);
      return out;
    }
    for (const entry of fs.readdirSync(target).sort()) {
      walk(path.join(target, entry), options, out);
    }
    return out;
  }
  if (st.isFile()) {
    if (!options.includePrivate && !isPublicMode(st.mode)) {
      options.skippedPrivate.push(target);
      return out;
    }
    if (st.size <= MAX_FILE_BYTES) out.push(target);
  }
  return out;
}

function defaultTargets(args) {
  const targets = DEFAULT_DOC_TARGETS
    .map(file => path.join(ROOT, file))
    .filter(file => statOrNull(file)?.isFile());
  if (!args.repoOnly) {
    for (const entry of fs.readdirSync('/tmp').sort()) {
      if (entry.startsWith(path.basename(DEFAULT_TMP_PREFIX))) {
        targets.push(path.join('/tmp', entry));
      }
    }
  }
  for (const item of args.artifactDirs) {
    targets.push(path.resolve(item));
  }
  return targets;
}

function lineNumberAt(text, index) {
  let line = 1;
  for (let i = 0; i < index; i += 1) {
    if (text.charCodeAt(i) === 10) line += 1;
  }
  return line;
}

function scanFile(file) {
  let text;
  try {
    text = fs.readFileSync(file, 'utf8');
  } catch (_) {
    return [];
  }
  const findings = [];
  for (const pattern of DENY_PATTERNS) {
    pattern.regex.lastIndex = 0;
    let match;
    while ((match = pattern.regex.exec(text)) !== null) {
      findings.push({
        file,
        line: lineNumberAt(text, match.index),
        rule: pattern.id,
        message: pattern.message,
      });
    }
  }
  return findings;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write(usage());
    return 0;
  }

  const options = {
    includePrivate: args.includePrivate,
    skippedPrivate: [],
  };
  const files = [];
  for (const target of defaultTargets(args)) {
    walk(target, options, files);
  }
  const uniqueFiles = [...new Set(files)].sort();
  const findings = uniqueFiles.flatMap(scanFile);
  const report = {
    ok: findings.length === 0,
    schema: 'postfiat-navswap-redaction-check-v2',
    scanned_file_count: uniqueFiles.length,
    skipped_private_count: options.skippedPrivate.length,
    skipped_private_paths: options.skippedPrivate,
    finding_count: findings.length,
    findings,
  };

  if (args.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else if (report.ok) {
    process.stdout.write(
      `NAVSwap redaction check passed (${report.scanned_file_count} files scanned, `
      + `${report.skipped_private_count} private paths skipped)\n`,
    );
  } else {
    for (const finding of findings) {
      process.stderr.write(`${finding.file}:${finding.line}: ${finding.rule}: ${finding.message}\n`);
    }
    process.stderr.write(`NAVSwap redaction check failed with ${findings.length} finding(s)\n`);
  }
  return report.ok ? 0 : 1;
}

try {
  process.exitCode = main();
} catch (error) {
  process.stderr.write(`${error.message || error}\n`);
  process.exitCode = 1;
}
