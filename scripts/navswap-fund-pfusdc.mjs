#!/usr/bin/env node
import { execFile } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { promisify } from 'node:util';
import WebSocket from '../wallet-proxy/node_modules/ws/index.js';

import { RpcClient } from '../wallet-web/src/lib/rpc-client.js';
import { PFUSDC_ASSET_ID } from '../wallet-web/src/lib/utils.js';

globalThis.WebSocket = WebSocket;

const execFileAsync = promisify(execFile);
const DEFAULT_RPC = 'ws://127.0.0.1:8080/rpc';
const DEFAULT_AMOUNT_ATOMS = '10000000';
const DEFAULT_MAX_AMOUNT_ATOMS = '100000000';

function usage() {
  return `Usage:
  node scripts/navswap-fund-pfusdc.mjs --recipient pf... [--amount-atoms 10000000]
  node scripts/navswap-fund-pfusdc.mjs --execute --recipient pf... --issuer-key-file issuer.key.json [--amount-atoms 10000000]

Dry-run mode checks whether the recipient has a usable canonical pfUSDC trustline.
Execution mode signs an issuer-owned issued_payment with postfiat-node and submits through the finality-enabled wallet proxy.

Options:
  --recipient ADDR            Recipient wallet address.
  --issuer-key-file FILE      Issuer .key.json. Defaults to NAVSWAP_OPERATOR_ISSUER_KEY_FILE.
  --execute                   Move live devnet funds. Refuses to run without issuer key and trustline.
  --amount-atoms N            pfUSDC atoms to issue. Default: ${DEFAULT_AMOUNT_ATOMS}.
  --max-amount-atoms N        Safety cap. Default: ${DEFAULT_MAX_AMOUNT_ATOMS}.
  --rpc URL                   Wallet proxy WebSocket RPC. Default: ${DEFAULT_RPC}
  --node-bin PATH             postfiat-node binary. Default: target/release/postfiat-node.
  --out-dir DIR               Evidence directory. Default: /tmp/navswap-fund-pfusdc-<timestamp>
  --timeout-ms N              Submit/poll timeout. Default: 60000.
`;
}

function parseArgs(argv) {
  const args = {
    rpc: DEFAULT_RPC,
    amountAtoms: DEFAULT_AMOUNT_ATOMS,
    maxAmountAtoms: DEFAULT_MAX_AMOUNT_ATOMS,
    nodeBin: './target/release/postfiat-node',
    timeoutMs: 60000,
    issuerKeyFile: process.env.NAVSWAP_OPERATOR_ISSUER_KEY_FILE || '',
    execute: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else if (arg === '--execute') {
      args.execute = true;
    } else if (arg.startsWith('--')) {
      const key = arg.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      const value = argv[i + 1];
      if (value === undefined || value.startsWith('--')) throw new Error(`${arg} requires a value`);
      args[key] = value;
      i += 1;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }
  if (args.help) return args;
  if (!/^pf[0-9a-f]{40}$/.test(String(args.recipient || ''))) {
    throw new Error('--recipient must be a lowercase PostFiat account address');
  }
  for (const key of ['amountAtoms', 'maxAmountAtoms']) {
    if (!/^[1-9][0-9]*$/.test(String(args[key] || ''))) {
      throw new Error(`--${key.replace(/[A-Z]/g, c => `-${c.toLowerCase()}`)} must be a positive integer`);
    }
  }
  args.timeoutMs = Number.parseInt(args.timeoutMs, 10);
  if (!Number.isFinite(args.timeoutMs) || args.timeoutMs <= 0) {
    throw new Error('--timeout-ms must be a positive integer');
  }
  if (BigInt(args.amountAtoms) > BigInt(args.maxAmountAtoms)) {
    throw new Error(`amount ${args.amountAtoms} exceeds safety cap ${args.maxAmountAtoms}`);
  }
  return args;
}

function jsonReplacer(_key, value) {
  return typeof value === 'bigint' ? value.toString() : value;
}

async function writeJson(file, value) {
  await fs.writeFile(file, `${JSON.stringify(value, jsonReplacer, 2)}\n`);
}

function assetItems(result) {
  if (Array.isArray(result)) return result;
  if (Array.isArray(result?.assets)) return result.assets;
  return [];
}

function accountLines(result) {
  if (Array.isArray(result)) return result;
  if (Array.isArray(result?.lines)) return result.lines;
  return [];
}

function canonicalPfusdcBalance(assetResult) {
  let total = 0n;
  for (const item of assetItems(assetResult)) {
    if ((item.asset_id || item.id) === PFUSDC_ASSET_ID) {
      total += BigInt(item.balance ?? item.amount ?? 0);
    }
  }
  return total;
}

function findPfusdcTrustline(linesResult, issuer) {
  return accountLines(linesResult).find(line => (
    line.asset_id === PFUSDC_ASSET_ID
    && (!issuer || line.issuer === issuer)
  )) || null;
}

function usableTrustline(line, amountAtoms) {
  if (!line) return false;
  if (line.frozen === true) return false;
  if (line.authorized === false) return false;
  const limit = BigInt(line.limit ?? 0);
  const balance = BigInt(line.balance ?? 0);
  return limit - balance >= BigInt(amountAtoms);
}

function assetIssuer(assetInfo) {
  const asset = assetInfo?.asset || assetInfo?.asset_definition || assetInfo?.definition || assetInfo;
  return asset?.issuer || asset?.owner || null;
}

async function readIssuerAddress(keyFile) {
  if (!keyFile) return null;
  const parsed = JSON.parse(await fs.readFile(keyFile, 'utf8'));
  return parsed.address || null;
}

async function pollPfusdcBalance(rpc, recipient, minBalance, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let last = null;
  while (Date.now() <= deadline) {
    const resp = await rpc.accountAssets(recipient);
    if (!resp.ok) throw new Error(resp.error?.message || 'account_assets failed while polling funding result');
    last = resp.result;
    const balance = canonicalPfusdcBalance(last);
    if (balance >= minBalance) return { ok: true, result: last, balance };
    await new Promise(resolve => setTimeout(resolve, 1500));
  }
  return { ok: false, result: last, balance: canonicalPfusdcBalance(last) };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write(usage());
    return;
  }

  const outDir = args.outDir || path.join('/tmp', `navswap-fund-pfusdc-${new Date().toISOString().replace(/[:.]/g, '')}`);
  await fs.mkdir(outDir, { recursive: true });
  const rpc = new RpcClient(args.rpc);
  try {
    const [assetInfoResp, linesResp, assetsResp] = await Promise.all([
      rpc.assetInfo(PFUSDC_ASSET_ID),
      rpc.accountLines(args.recipient),
      rpc.accountAssets(args.recipient),
    ]);
    if (!assetInfoResp.ok) throw new Error(assetInfoResp.error?.message || 'asset_info failed for canonical pfUSDC');
    if (!linesResp.ok) throw new Error(linesResp.error?.message || 'account_lines failed for recipient');
    if (!assetsResp.ok) throw new Error(assetsResp.error?.message || 'account_assets failed for recipient');

    const issuer = assetIssuer(assetInfoResp.result);
    const issuerKeyAddress = await readIssuerAddress(args.issuerKeyFile).catch(() => null);
    const trustline = findPfusdcTrustline(linesResp.result, issuer);
    const beforeBalance = canonicalPfusdcBalance(assetsResp.result);
    const readiness = {
      ok: true,
      recipient: args.recipient,
      execute_requested: args.execute,
      amount_atoms: BigInt(args.amountAtoms),
      max_amount_atoms: BigInt(args.maxAmountAtoms),
      asset_id: PFUSDC_ASSET_ID,
      issuer,
      issuer_key_address: issuerKeyAddress,
      issuer_key_matches_asset: issuerKeyAddress ? issuerKeyAddress === issuer : null,
      before_balance_atoms: beforeBalance,
      trustline_found: Boolean(trustline),
      trustline,
      trustline_usable: usableTrustline(trustline, args.amountAtoms),
    };
    await writeJson(path.join(outDir, 'readiness.json'), readiness);

    if (!args.execute) {
      const summary = {
        ok: true,
        mode: 'dry-run',
        out_dir: outDir,
        readiness,
        message: readiness.trustline_usable
          ? 'Recipient is ready for guarded pfUSDC funding. Re-run with --execute to move live devnet funds.'
          : 'Recipient is not ready for pfUSDC funding. Open a canonical pfUSDC trustline first.',
      };
      await writeJson(path.join(outDir, 'summary.json'), summary);
      process.stdout.write(`${JSON.stringify(summary, jsonReplacer, 2)}\n`);
      return;
    }

    if (!args.issuerKeyFile) throw new Error('--execute requires --issuer-key-file or NAVSWAP_OPERATOR_ISSUER_KEY_FILE');
    if (issuerKeyAddress !== issuer) {
      throw new Error(`issuer key address ${issuerKeyAddress || 'unknown'} does not match canonical pfUSDC issuer ${issuer}`);
    }
    if (!readiness.trustline_usable) {
      throw new Error('recipient does not have a usable canonical pfUSDC trustline for the requested amount');
    }

    const operation = {
      operation: 'issued_payment',
      from: issuer,
      to: args.recipient,
      issuer,
      asset_id: PFUSDC_ASSET_ID,
      amount: Number.parseInt(args.amountAtoms, 10),
    };
    await writeJson(path.join(outDir, 'operation.json'), operation);
    const quoteResp = await rpc.assetFeeQuote(issuer, JSON.stringify(operation));
    await writeJson(path.join(outDir, 'asset-fee-quote-response.json'), quoteResp);
    if (!quoteResp.ok) throw new Error(quoteResp.error?.message || 'asset_fee_quote failed for funding operation');
    const quoteFile = path.join(outDir, 'asset-fee-quote.json');
    await writeJson(quoteFile, quoteResp.result);

    const tmpDir = await fs.mkdtemp(path.join(os.tmpdir(), 'navswap-fund-pfusdc-sign-'));
    const signedFile = path.join(tmpDir, 'signed.json');
    try {
      const { stdout } = await execFileAsync(
        args.nodeBin,
        [
          'wallet-sign-asset-transaction',
          '--key-file',
          args.issuerKeyFile,
          '--quote-file',
          quoteFile,
        ],
        {
          timeout: args.timeoutMs,
          maxBuffer: 2 * 1024 * 1024,
        },
      );
      await fs.writeFile(signedFile, stdout, { mode: 0o600 });
      const signed = JSON.parse(stdout);
      await writeJson(path.join(outDir, 'signed-asset-transaction.json'), signed);

      const submitResp = await rpc.submitSignedAssetTransactionFinality(JSON.stringify(signed));
      await writeJson(path.join(outDir, 'submit-response.json'), submitResp);
      if (!submitResp.ok) throw new Error(submitResp.error?.message || 'funding finality submit failed');

      const minBalance = beforeBalance + BigInt(args.amountAtoms);
      const after = await pollPfusdcBalance(rpc, args.recipient, minBalance, args.timeoutMs);
      await writeJson(path.join(outDir, 'after-assets.json'), after.result);
      if (!after.ok) {
        throw new Error(`funding submit accepted but pfUSDC balance did not reach ${minBalance} before timeout`);
      }
      const summary = {
        ok: true,
        mode: 'execute',
        out_dir: outDir,
        recipient: args.recipient,
        asset_id: PFUSDC_ASSET_ID,
        issuer,
        amount_atoms: BigInt(args.amountAtoms),
        before_balance_atoms: beforeBalance,
        after_balance_atoms: after.balance,
        tx_id: submitResp.result?.tx_id || null,
      };
      await writeJson(path.join(outDir, 'summary.json'), summary);
      process.stdout.write(`${JSON.stringify(summary, jsonReplacer, 2)}\n`);
    } finally {
      await fs.rm(tmpDir, { recursive: true, force: true }).catch(() => {});
    }
  } finally {
    rpc.close();
  }
}

main().catch((error) => {
  process.stderr.write(`${error.stack || error.message}\n`);
  process.exit(1);
});
