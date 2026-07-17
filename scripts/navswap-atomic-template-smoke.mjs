#!/usr/bin/env node
import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';

import { A651_ASSET_ID, PFUSDC_ASSET_ID } from '../wallet-web/src/lib/utils.js';

const DEFAULT_PROXY = 'http://127.0.0.1:8080';
const DEFAULT_LEFT_OWNER = 'pf07381735ddb7de134e8be8402b465c9cd8ec7546';
const DEFAULT_RIGHT_OWNER = 'pf65c9783ceafc0f519a74195e78cc7909f92429c3';

function usage() {
  return `Usage:
  node scripts/navswap-atomic-template-smoke.mjs [--out-dir DIR]

Read-only NAVSwap ESCROW-009 smoke. It calls the wallet proxy atomic-template
adapter, verifies the returned schema/symmetry/escrow ids, and writes evidence.
It never signs, submits, approves, or transfers funds.

Defaults use the live NAVSwap runbook buyer/holder accounts and request a
1-atom PFT <-> a651 template.

Options:
  --proxy URL                Wallet proxy HTTP base. Default: ${DEFAULT_PROXY}
  --left-owner ADDR          Left escrow owner. Default: ${DEFAULT_LEFT_OWNER}
  --right-owner ADDR         Right escrow owner. Default: ${DEFAULT_RIGHT_OWNER}
  --left-asset ASSET         PFT, pfUSDC, a651, or raw asset id. Default: PFT
  --right-asset ASSET        PFT, pfUSDC, a651, or raw asset id. Default: a651
  --left-amount N            Positive integer atom amount. Default: 1
  --right-amount N           Positive integer atom amount. Default: 1
  --condition TEXT           Shared fulfillment text. Default: timestamped smoke label
  --cancel-after N           Positive ledger height. Default: 999999999
  --out-dir DIR              Evidence directory. Default: /tmp/navswap-atomic-template-smoke-<timestamp>
`;
}

function parseArgs(argv) {
  const args = {
    proxy: DEFAULT_PROXY,
    leftOwner: DEFAULT_LEFT_OWNER,
    rightOwner: DEFAULT_RIGHT_OWNER,
    leftAsset: 'PFT',
    rightAsset: 'a651',
    leftAmount: '1',
    rightAmount: '1',
    cancelAfter: '999999999',
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else if (arg.startsWith('--')) {
      const key = arg.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      const value = argv[i + 1];
      if (value === undefined || value.startsWith('--')) {
        throw new Error(`${arg} requires a value`);
      }
      args[key] = value;
      i += 1;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  if (args.help) return args;
  for (const [label, address] of [['--left-owner', args.leftOwner], ['--right-owner', args.rightOwner]]) {
    if (!/^pf[0-9a-f]{40}$/.test(String(address || ''))) {
      throw new Error(`${label} must be a lowercase PostFiat account address`);
    }
  }
  if (args.leftOwner === args.rightOwner) {
    throw new Error('--left-owner and --right-owner must differ');
  }
  for (const key of ['leftAmount', 'rightAmount', 'cancelAfter']) {
    if (!/^[1-9][0-9]*$/.test(String(args[key] || ''))) {
      throw new Error(`--${key.replace(/[A-Z]/g, c => `-${c.toLowerCase()}`)} must be a positive integer`);
    }
  }
  args.condition ||= `navswap-atomic-smoke-${new Date().toISOString()}`;
  return args;
}

function jsonReplacer(_key, value) {
  return typeof value === 'bigint' ? value.toString() : value;
}

async function writeJson(file, value) {
  await fs.writeFile(file, `${JSON.stringify(value, jsonReplacer, 2)}\n`);
}

function assetId(value) {
  if (value === 'PFT') return 'PFT';
  if (value === 'pfUSDC') return PFUSDC_ASSET_ID;
  if (value === 'a651') return A651_ASSET_ID;
  if (/^[0-9a-f]{96}$/.test(String(value || ''))) return value;
  throw new Error(`unsupported asset alias or id: ${value}`);
}

function assertAtomicTemplate(responseBody) {
  if (!responseBody || typeof responseBody !== 'object') {
    throw new Error('atomic template response is not an object');
  }
  if (responseBody.ok !== true) {
    throw new Error(responseBody.message || responseBody.error?.message || 'atomic template smoke failed');
  }
  if (responseBody.schema !== 'postfiat-navswap-atomic-template-v1') {
    throw new Error(`unexpected adapter schema: ${responseBody.schema || 'missing'}`);
  }
  const verification = responseBody.verification || {};
  if (verification.schema !== 'postfiat-atomic-settlement-template-v1') {
    throw new Error(`unexpected template schema: ${verification.schema || 'missing'}`);
  }
  const symmetry = responseBody.symmetry || {};
  if (symmetry.stable !== true) {
    throw new Error('atomic template symmetry was not verified');
  }
  if (!verification.settlement_id || verification.settlement_id !== symmetry.settlement_id) {
    throw new Error('settlement_id missing or not symmetric');
  }
  if (!verification.condition_hash || verification.condition_hash !== symmetry.condition_hash) {
    throw new Error('condition_hash missing or not symmetric');
  }
  if (!verification.left_escrow_id || !verification.right_escrow_id) {
    throw new Error('escrow ids missing from verification');
  }
  if (verification.left_escrow_id === verification.right_escrow_id) {
    throw new Error('left and right escrow ids must be distinct');
  }
  return { verification, symmetry };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write(usage());
    return;
  }

  const outDir = args.outDir || path.join('/tmp', `navswap-atomic-template-smoke-${new Date().toISOString().replace(/[:.]/g, '')}`);
  await fs.mkdir(outDir, { recursive: true });
  const request = {
    left_owner: args.leftOwner,
    left_recipient: args.rightOwner,
    left_asset_id: assetId(args.leftAsset),
    left_amount: args.leftAmount,
    right_owner: args.rightOwner,
    right_recipient: args.leftOwner,
    right_asset_id: assetId(args.rightAsset),
    right_amount: args.rightAmount,
    condition: args.condition,
    cancel_after: args.cancelAfter,
  };
  await writeJson(path.join(outDir, 'request.json'), request);

  const endpoint = new URL('/api/navswap/atomic-templates', args.proxy);
  const httpResponse = await fetch(endpoint, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(request),
  });
  const responseBody = await httpResponse.json().catch(error => ({
    ok: false,
    code: 'json_parse_failed',
    message: String(error?.message || error),
  }));
  await writeJson(path.join(outDir, 'response.json'), {
    status: httpResponse.status,
    ok: httpResponse.ok,
    body: responseBody,
  });

  if (!httpResponse.ok) {
    throw new Error(`atomic template endpoint returned HTTP ${httpResponse.status}`);
  }
  const { verification, symmetry } = assertAtomicTemplate(responseBody);
  const summary = {
    ok: true,
    schema: 'postfiat-navswap-atomic-template-smoke-v1',
    out_dir: outDir,
    endpoint: endpoint.toString(),
    left_owner: request.left_owner,
    right_owner: request.right_owner,
    left_asset_id: request.left_asset_id,
    right_asset_id: request.right_asset_id,
    settlement_id: verification.settlement_id,
    condition_hash: verification.condition_hash,
    left_escrow_id: verification.left_escrow_id,
    right_escrow_id: verification.right_escrow_id,
    symmetry_stable: symmetry.stable === true,
  };
  await writeJson(path.join(outDir, 'summary.json'), summary);
  process.stdout.write(`${JSON.stringify(summary, jsonReplacer, 2)}\n`);
}

main().catch(error => {
  process.stderr.write(`${error?.stack || error}\n`);
  process.exit(1);
});
