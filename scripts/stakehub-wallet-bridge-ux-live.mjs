#!/usr/bin/env node
import { createRequire } from 'node:module';
import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const require = createRequire(new URL('../wallet-web/package.json', import.meta.url));
const { chromium } = require('playwright');

const APP_URL = process.env.BRIDGE_UX_URL || 'http://127.0.0.1:5173/';
const RUNS = Number.parseInt(process.env.BRIDGE_UX_RUNS || '5', 10);
const AMOUNT = process.env.BRIDGE_UX_AMOUNT || '1';
const AMOUNT_ATOMS = BigInt(Math.round(Number(AMOUNT) * 1_000_000));
const NAVSWAP_RUNS = Number.parseInt(process.env.NAVSWAP_UX_RUNS || '0', 10);
const NAVSWAP_REDEEM_RUNS = Number.parseInt(process.env.NAVSWAP_UX_REDEEM_RUNS || '0', 10);
const NAVSWAP_AMOUNT = process.env.NAVSWAP_UX_AMOUNT || '1';
const OUT_DIR = process.env.BRIDGE_UX_OUT_DIR || `/tmp/postfiat-stakehub-ux-bridge-${Date.now()}`;
const PASSPHRASE = process.env.BRIDGE_UX_WALLET_PASSPHRASE || `stakehub-live-ux-${Date.now()}`;
const IMPORT_SEED = (process.env.BRIDGE_UX_IMPORT_SEED || '').trim().toLowerCase();

const PYTHON = String(process.env.STAKEHUB_PYTHON || '').trim();
if (!PYTHON) throw new Error('STAKEHUB_PYTHON must explicitly select the StakeHub Python interpreter');
const STAKEHUB_ADDRESS = '0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0';
const ARBITRUM_CHAIN_ID = 42161;
const ARBITRUM_CHAIN_HEX = '0xa4b1';
const ARBITRUM_RPC = 'https://arb1.arbitrum.io/rpc';
const MAINNET_RPC = 'https://ethereum-rpc.publicnode.com';
const ETH_MAINNET_CHAIN_ID = 1;
const ETH_MAINNET_USDC = '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48';
const ARBITRUM_USDC = '0xaf88d065e77c8cC2239327C5EDb3A432268e5831';
const RETIRED_BRIDGE_VAULTS = new Set(['0x1a15e6103d6af4e88924f748e13b829d3948dea9']);
const VAULT = String(process.env.BRIDGE_UX_VAULT_ADDRESS || '').trim();
const VAULT_CODE_HASH = String(process.env.BRIDGE_UX_VAULT_CODE_HASH || '').trim().toLowerCase();
const CAST = process.env.BRIDGE_UX_CAST_BIN || 'cast';
assertConfiguredVault(VAULT, VAULT_CODE_HASH);
verifyVaultDeployment(VAULT, VAULT_CODE_HASH);
const CCTP_V1_MAINNET_TOKEN_MESSENGER = '0xBd3fa81B58Ba92a82136038B25aDec7066af3155';
const CCTP_V2_MAINNET_TOKEN_MESSENGER = '0x28b5a0e9C621a5BadaA536219b3a228C8168cf5d';
const CCTP_V1_ARBITRUM_MESSAGE_TRANSMITTER = '0xC30362313FBBA5cf9163F0bb16a0e01f01A896ca';
const CCTP_V2_ARBITRUM_MESSAGE_TRANSMITTER = '0x81D40F21F12A8F0E3252Bccb954D722d4c464B64';

function assertConfiguredVault(address, codeHash) {
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) {
    throw new Error('BRIDGE_UX_VAULT_ADDRESS must explicitly name a reviewed 20-byte EVM bridge vault');
  }
  if (RETIRED_BRIDGE_VAULTS.has(address.toLowerCase())) {
    throw new Error('BRIDGE_UX_VAULT_ADDRESS identifies a retired bridge vault');
  }
  if (!/^0x[0-9a-f]{64}$/.test(codeHash)) {
    throw new Error('BRIDGE_UX_VAULT_CODE_HASH must explicitly bind the reviewed vault bytecode');
  }
}

function verifyVaultDeployment(address, expectedCodeHash) {
  const result = spawnSync(
    CAST,
    ['codehash', address, '--rpc-url', ARBITRUM_RPC],
    { encoding: 'utf8', timeout: 30_000 },
  );
  assertOk(result.status === 0, 'failed to read the configured bridge vault bytecode hash');
  const actual = String(result.stdout || '').trim().toLowerCase();
  assertOk(actual === expectedCodeHash, 'configured bridge vault bytecode hash mismatch');
}

mkdirSync(OUT_DIR, { recursive: true });

function assertOk(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function runAgentd(request, timeout = 900) {
  const helper = `
import json
import sys
from stakehub import agentd
from web3 import Web3

req = json.load(sys.stdin)

def emit(payload):
    print(json.dumps(payload, default=str))

if req.get("_helper") == "send_tx":
    session_id = req["session_id"]
    session_action = req["session_action"]
    dummy = b"\\x60\\x00"
    try:
        agentd.call({"op": "close_launch_session"}, timeout=30)
    except Exception:
        pass
    open_req = {
        "op": "open_launch_session",
        "session_id": session_id,
        "chain_id": req["chain_id"],
        "owner": Web3.to_checksum_address(req["owner"]),
        "policy_name": req.get("policy_name", "codex-wallet-ux-live"),
        "usdc_address": Web3.to_checksum_address(req["usdc_address"]),
        "usdc_budget": str(req["usdc_budget"]),
        "allowlist": [Web3.to_checksum_address(x) for x in req["allowlist"]],
        "expected_deploys": [{
            "label": "codex-empty-session",
            "bytecode_hash": Web3.to_hex(Web3.keccak(dummy)),
            "bytecode_len": len(dummy),
            "bytecode": Web3.to_hex(dummy),
        }],
        "close_after_action": session_action,
    }
    opened = agentd.call(open_req, timeout=60)
    if not opened.get("ok"):
        emit({"ok": False, "stage": "open_launch_session", "response": opened})
        sys.exit(0)
    tx_req = {
        "op": "evm_contract_tx",
        "chain_id": req["chain_id"],
        "rpc_url": req["rpc_url"],
        "to": Web3.to_checksum_address(req["to"]),
        "data": req.get("data") or "0x",
        "session_id": session_id,
        "session_action": session_action,
        "label": req.get("label", session_action),
        "gas_usd": req.get("gas_usd", 0.1),
    }
    if req.get("value"):
        tx_req["value"] = req["value"]
    sent = agentd.call(tx_req, timeout=req.get("timeout", 900))
    emit({"ok": bool(sent.get("ok")), "stage": "evm_contract_tx", "response": sent, "session_id": session_id, "session_action": session_action})
else:
    emit(agentd.call(req, timeout=req.get("timeout", 60)))
`;
  const proc = spawnSync(PYTHON, ['-c', helper], {
    input: JSON.stringify({ ...request, timeout }),
    encoding: 'utf8',
    maxBuffer: 1024 * 1024 * 16,
  });
  if (proc.error) {
    throw proc.error;
  }
  if (proc.status !== 0) {
    throw new Error(`stakehub agent helper failed (${proc.status}): ${proc.stderr || proc.stdout}`);
  }
  const stdout = proc.stdout.trim();
  if (!stdout) {
    throw new Error(`stakehub agent helper returned no output: ${proc.stderr}`);
  }
  const lastLine = stdout.split(/\r?\n/).at(-1);
  return JSON.parse(lastLine);
}

async function rpcCall(chainIdHex, method, params = []) {
  if (method === 'eth_estimateGas') {
    return '0x7a120';
  }
  const chainId = Number.parseInt(chainIdHex, 16);
  const rpcUrl = chainId === ARBITRUM_CHAIN_ID ? ARBITRUM_RPC : MAINNET_RPC;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 45_000);
  const response = await fetch(rpcUrl, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    signal: controller.signal,
    body: JSON.stringify({ jsonrpc: '2.0', id: Date.now(), method, params }),
  }).finally(() => clearTimeout(timeout));
  const payload = await response.json();
  if (payload.error) {
    throw new Error(`${method} failed: ${payload.error.message || JSON.stringify(payload.error)}`);
  }
  return payload.result;
}

function stakehubSendTransaction(tx, chainIdHex) {
  const chainId = Number.parseInt(chainIdHex, 16);
  assertOk(
    chainId === ETH_MAINNET_CHAIN_ID || chainId === ARBITRUM_CHAIN_ID,
    `unexpected live transaction chain ${chainId}`,
  );
  assertOk(tx && tx.to, 'transaction missing target');
  const selector = (tx.data || '0x').slice(0, 10);
  const target = tx.to.toLowerCase();
  const allowedTargets = chainId === ETH_MAINNET_CHAIN_ID
    ? [
        ETH_MAINNET_USDC,
        CCTP_V1_MAINNET_TOKEN_MESSENGER,
        CCTP_V2_MAINNET_TOKEN_MESSENGER,
      ]
    : [
        ARBITRUM_USDC,
        VAULT,
        CCTP_V1_ARBITRUM_MESSAGE_TRANSMITTER,
        CCTP_V2_ARBITRUM_MESSAGE_TRANSMITTER,
      ];
  const isAllowedTarget = allowedTargets.some((allowed) => target === allowed.toLowerCase());
  assertOk(isAllowedTarget, `unexpected live transaction target ${tx.to}`);
  const actionId = `ux-${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
  const usdcAddress = chainId === ETH_MAINNET_CHAIN_ID ? ETH_MAINNET_USDC : ARBITRUM_USDC;
  const label = chainId === ETH_MAINNET_CHAIN_ID
    ? selector === '0x095ea7b3'
      ? 'wallet-ux-cctp-usdc-approve'
      : 'wallet-ux-cctp-burn'
    : target === VAULT.toLowerCase()
      ? 'wallet-ux-vault-deposit'
      : target === ARBITRUM_USDC.toLowerCase()
        ? 'wallet-ux-usdc-approve'
        : 'wallet-ux-cctp-mint';
  const result = runAgentd(
    {
      _helper: 'send_tx',
      session_id: actionId,
      session_action: actionId,
      chain_id: chainId,
      rpc_url: chainId === ETH_MAINNET_CHAIN_ID ? MAINNET_RPC : ARBITRUM_RPC,
      owner: STAKEHUB_ADDRESS,
      usdc_address: usdcAddress,
      usdc_budget: (AMOUNT_ATOMS * 10n).toString(),
      allowlist: [STAKEHUB_ADDRESS, usdcAddress, ...allowedTargets],
      to: tx.to,
      data: tx.data || '0x',
      value: tx.value || '0x0',
      label,
      gas_usd: Number(process.env.BRIDGE_UX_GAS_USD || (chainId === ETH_MAINNET_CHAIN_ID ? '12' : '0.25')),
    },
    900,
  );
  if (!result.ok) {
    throw new Error(`StakeHub tx rejected at ${result.stage}: ${JSON.stringify(result.response)}`);
  }
  const txHash = result.response.tx || result.response.tx_hash || result.response.transactionHash;
  assertOk(txHash, `StakeHub tx response missing hash: ${JSON.stringify(result.response)}`);
  return {
    hash: txHash.startsWith('0x') ? txHash : `0x${txHash}`,
    selector,
    to: tx.to,
    raw: result.response,
  };
}

async function installStakehubProvider(page) {
  await page.exposeFunction('__stakehubRpcCall', rpcCall);
  await page.exposeFunction('__stakehubSendTransaction', stakehubSendTransaction);
  await page.addInitScript(
    ({ address, chainIdHex }) => {
      const listeners = new Map();
      const emit = (event, payload) => {
        const handlers = listeners.get(event) || [];
        handlers.forEach((handler) => {
          try {
            handler(payload);
          } catch {
            // Browser event handlers are best-effort.
          }
        });
      };
      window.__bridgeUx = { sent: [], relayResponses: [], navswapResponses: [] };
      const originalFetch = window.fetch.bind(window);
      window.fetch = async (...args) => {
        const response = await originalFetch(...args);
        const url = typeof args[0] === 'string' ? args[0] : args[0]?.url || '';
        if (url.includes('/api/bridge/relay')) {
          response
            .clone()
            .json()
            .then((body) => {
              window.__bridgeUx.relayResponses.push({
                at: new Date().toISOString(),
                status: response.status,
                ok: response.ok,
                body,
              });
            })
            .catch((error) => {
              window.__bridgeUx.relayResponses.push({
                at: new Date().toISOString(),
                status: response.status,
                ok: response.ok,
                error: String(error),
              });
            });
        }
        if (url.includes('/api/navswap/')) {
          response
            .clone()
            .json()
            .then((body) => {
              window.__bridgeUx.navswapResponses.push({
                at: new Date().toISOString(),
                url,
                status: response.status,
                ok: response.ok,
                body,
              });
            })
            .catch((error) => {
              window.__bridgeUx.navswapResponses.push({
                at: new Date().toISOString(),
                url,
                status: response.status,
                ok: response.ok,
                error: String(error),
              });
            });
        }
        return response;
      };
      let currentChainId = chainIdHex;
      window.ethereum = {
        isMetaMask: true,
        selectedAddress: address,
        chainId: currentChainId,
        on(event, handler) {
          listeners.set(event, [...(listeners.get(event) || []), handler]);
          return this;
        },
        removeListener(event, handler) {
          listeners.set(
            event,
            (listeners.get(event) || []).filter((item) => item !== handler),
          );
          return this;
        },
        async request({ method, params = [] }) {
          if (method === 'eth_requestAccounts' || method === 'eth_accounts') {
            return [address];
          }
          if (method === 'eth_chainId') {
            return currentChainId;
          }
          if (method === 'wallet_switchEthereumChain') {
            currentChainId = params?.[0]?.chainId || currentChainId;
            this.chainId = currentChainId;
            emit('chainChanged', currentChainId);
            return null;
          }
          if (method === 'wallet_addEthereumChain') {
            currentChainId = params?.[0]?.chainId || currentChainId;
            this.chainId = currentChainId;
            emit('chainChanged', currentChainId);
            return null;
          }
          if (method === 'eth_sendTransaction') {
            const tx = params?.[0] || {};
            if (tx.from && tx.from.toLowerCase() !== address.toLowerCase()) {
              throw new Error(`unexpected sender ${tx.from}`);
            }
            const sent = await window.__stakehubSendTransaction(tx, currentChainId);
            window.__bridgeUx.sent.push({
              at: new Date().toISOString(),
              chainId: currentChainId,
              hash: sent.hash,
              selector: sent.selector,
              to: sent.to,
            });
            return sent.hash;
          }
          if (
            method === 'eth_call' ||
            method === 'eth_getBalance' ||
            method === 'eth_getTransactionReceipt' ||
            method === 'eth_getTransactionByHash' ||
            method === 'eth_blockNumber' ||
            method === 'eth_gasPrice' ||
            method === 'eth_estimateGas' ||
            method === 'eth_feeHistory'
          ) {
            return window.__stakehubRpcCall(currentChainId, method, params);
          }
          throw new Error(`unsupported provider method ${method}`);
        },
      };
      emit('accountsChanged', [address]);
    },
    { address: STAKEHUB_ADDRESS, chainIdHex: ARBITRUM_CHAIN_HEX },
  );
}

async function maybeClick(page, locator, timeout = 5000) {
  try {
    await locator.click({ timeout });
    return true;
  } catch {
    return false;
  }
}

async function clickButtonByText(page, text, timeout = 60_000) {
  const button = page.locator('button').filter({ hasText: text }).first();
  await button.waitFor({ timeout });
  const state = await button.evaluate((node) => ({
    text: node.innerText,
    disabled: Boolean(node.disabled),
    ariaDisabled: node.getAttribute('aria-disabled'),
  }));
  console.log(`clicking button: ${JSON.stringify(state)}`);
  await button.click({ timeout });
}

async function waitForBodyMatch(page, pattern, timeout = 120_000) {
  await page.waitForFunction(
    (source) => {
      const re = new RegExp(source, 'i');
      return re.test(document.body.innerText || '');
    },
    pattern.source,
    { timeout },
  );
  return page.locator('body').innerText();
}

async function createWalletInUx(page) {
  await page.goto(APP_URL, { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 30_000 }).catch(() => {});
  await page.screenshot({ path: join(OUT_DIR, '00-onboard.png'), fullPage: true });
  const createButton = page.getByRole('button', { name: /^Create Wallet$/ }).first();
  await createButton.click({ timeout: 30_000 });
  await page.waitForSelector('.pf-seed-display', { timeout: 30_000 });
  const seed = (await page.locator('.pf-seed-display').innerText()).trim();
  assertOk(/^[0-9a-f]{64}$/i.test(seed), 'generated wallet seed was not displayed');
  const accountAddress = (await page.locator('text=/pf[a-f0-9]{40}/i').first().innerText()).trim();
  writeFileSync(join(OUT_DIR, 'wallet-generated-sensitive.json'), JSON.stringify({
    warning: 'Sensitive test wallet seed. Do not publish.',
    accountAddress,
    seed,
    passphrase: PASSPHRASE,
  }, null, 2));
  await page.locator('input[type="checkbox"]').first().check();
  const passphraseInputs = page.locator('input[type="password"]');
  await passphraseInputs.nth(0).fill(PASSPHRASE);
  await passphraseInputs.nth(1).fill(PASSPHRASE);
  await page.getByRole('button', { name: /^Create Wallet$/ }).last().click();
  await page.waitForTimeout(1500);
  await page.screenshot({ path: join(OUT_DIR, '01-wallet-created.png'), fullPage: true });
  return { accountAddress };
}

async function importWalletInUx(page) {
  assertOk(/^[0-9a-f]{64}$/.test(IMPORT_SEED), 'BRIDGE_UX_IMPORT_SEED must be a 64-char hex seed');
  await page.goto(APP_URL, { waitUntil: 'domcontentloaded' });
  await page.waitForLoadState('networkidle', { timeout: 30_000 }).catch(() => {});
  await page.screenshot({ path: join(OUT_DIR, '00-import-onboard.png'), fullPage: true });
  await page.getByRole('button', { name: /^Import Wallet$/ }).click({ timeout: 30_000 });
  await page.locator('input[placeholder*="64 hex"]').fill(IMPORT_SEED);
  await page.getByRole('button', { name: /^Validate Seed$/ }).click({ timeout: 30_000 });
  await page.waitForSelector('text=/Imported seed derives to address/i', { timeout: 30_000 });
  const accountAddress = (await page.locator('text=/pf[a-f0-9]{40}/i').first().innerText()).trim();
  const passphraseInputs = page.locator('input[type="password"]');
  await passphraseInputs.nth(0).fill(PASSPHRASE);
  await passphraseInputs.nth(1).fill(PASSPHRASE);
  await page.getByRole('button', { name: /^Confirm Import$/ }).click({ timeout: 30_000 });
  await page.waitForTimeout(1500);
  await page.screenshot({ path: join(OUT_DIR, '01-wallet-imported.png'), fullPage: true });
  return { accountAddress };
}

async function openBridgeAndConnect(page) {
  await page.locator('button').filter({ hasText: 'Bridge' }).first().click({ timeout: 10_000 });
  await page.waitForTimeout(500);
  await page.screenshot({ path: join(OUT_DIR, '02-bridge-before-connect.png'), fullPage: true });
  await maybeClick(page, page.getByRole('button', { name: /Connect MetaMask/i }).first(), 10_000);
  await page.waitForSelector('text=/Approve the vault|Deposit to the vault|Bridge USDC to Arbitrum/i', {
    timeout: 45_000,
  });
  await page.screenshot({ path: join(OUT_DIR, '03-bridge-connected.png'), fullPage: true });
}

async function setAmount(page, amount) {
  const amountInput = page.locator('.pfb-amount input').first();
  await amountInput.waitFor({ timeout: 30_000 });
  await amountInput.fill(amount);
}

async function runBridgeCycle(page, index) {
  await setAmount(page, AMOUNT);
  const before = await page.evaluate(() => ({
    sent: window.__bridgeUx?.sent?.length || 0,
    relay: window.__bridgeUx?.relayResponses?.length || 0,
  }));
  await page.screenshot({ path: join(OUT_DIR, `cycle-${index}-ready.png`), fullPage: true });

  const bridgeButton = page.locator('button').filter({ hasText: 'Bridge via Circle CCTP' }).first();
  if (await bridgeButton.isVisible({ timeout: 10_000 }).catch(() => false)) {
    await clickButtonByText(page, 'Bridge via Circle CCTP', 60_000);
    const afterBridgeText = await waitForBodyMatch(
      page,
      /Approve Arbitrum USDC|Deposit to vault|Circle CCTP Fast Transfer failed|Bridge action needs attention/,
      1_200_000,
    );
    await page.screenshot({ path: join(OUT_DIR, `cycle-${index}-after-cctp.png`), fullPage: true });
    assertOk(
      !/Circle CCTP Fast Transfer failed|Bridge action needs attention/i.test(afterBridgeText),
      `cycle ${index} CCTP bridge failed in UX:\n${afterBridgeText}`,
    );
  }

  const approveButton = page.locator('button').filter({ hasText: 'Approve Arbitrum USDC' }).first();
  if (await approveButton.isVisible({ timeout: 10_000 }).catch(() => false)) {
    await clickButtonByText(page, 'Approve Arbitrum USDC', 60_000);
    const afterApproveText = await waitForBodyMatch(
      page,
      /Deposit to the vault|Bridge action needs attention|USDC approval failed/,
      240_000,
    );
    await page.screenshot({ path: join(OUT_DIR, `cycle-${index}-after-approve.png`), fullPage: true });
    assertOk(!/USDC approval failed|Bridge action needs attention/i.test(afterApproveText), `cycle ${index} approval failed in UX:\n${afterApproveText}`);
  }

  await clickButtonByText(page, 'Deposit to vault', 60_000);
  const afterDepositText = await waitForBodyMatch(
    page,
    /pfUSDC minted|Relay complete|Vault deposit confirmed, but relay failed|Vault deposit failed|Bridge action needs attention/,
    900_000,
  );
  await page.screenshot({ path: join(OUT_DIR, `cycle-${index}-complete.png`), fullPage: true });
  assertOk(
    !/Vault deposit confirmed, but relay failed|Vault deposit failed|Bridge action needs attention/i.test(afterDepositText),
    `cycle ${index} deposit/relay failed in UX:\n${afterDepositText}`,
  );

  const details = await page.evaluate((snapshot) => {
    const state = window.__bridgeUx || { sent: [], relayResponses: [] };
    return {
      text: document.body.innerText,
      sent: (state.sent || []).slice(snapshot.sent),
      relayResponses: (state.relayResponses || []).slice(snapshot.relay),
    };
  }, before);
  const relayOk = details.relayResponses.find((entry) => entry.ok && entry.body?.ok);
  assertOk(relayOk, `cycle ${index} did not capture a successful relay response`);
  assertOk(/pfUSDC minted|Relay complete|Bridge complete/i.test(details.text), `cycle ${index} did not show completion`);

  if (index < RUNS) {
    await page.getByRole('button', { name: /Start another bridge/i }).first().click({ timeout: 30_000 });
    await page.waitForSelector('text=/Approve the vault|Deposit to the vault/i', { timeout: 60_000 });
  }

  return {
    index,
    sent: details.sent,
    relay: relayOk.body,
  };
}

async function verifyWalletBalance(page, expected) {
  await page.locator('button').filter({ hasText: 'Wallet' }).first().click({ timeout: 15_000 });
  await page.waitForTimeout(2500);
  await page.screenshot({ path: join(OUT_DIR, '99-wallet-balance.png'), fullPage: true });
  const body = await page.locator('body').innerText();
  assertOk(/pfUSDC/i.test(body), 'wallet page does not show pfUSDC');
  const displayed = [...body.matchAll(/([0-9]+(?:\.[0-9]+)?)\s+pfUSDC/gi)]
    .map((match) => Number(match[1]))
    .filter(Number.isFinite);
  const actual = displayed.length ? Math.max(...displayed) : Number.NaN;
  assertOk(
    Number.isFinite(actual) && actual >= Number(expected),
    `wallet page did not show at least ${expected} pfUSDC; displayed values: ${displayed.join(', ') || 'none'}`,
  );
  return body;
}

function parseUiAmount(value) {
  const text = String(value || '').replace(/,/g, '').trim();
  const match = text.match(/[0-9]+(?:\.[0-9]+)?/);
  return match ? Number(match[0]) : Number.NaN;
}

async function readSwapBalances(page) {
  return page.evaluate(() => {
    const rows = Array.from(document.querySelectorAll('.pfs-balance-list div'));
    const balances = {};
    for (const row of rows) {
      const parts = (row.innerText || '').split(/\n+/).map((part) => part.trim()).filter(Boolean);
      if (parts.length >= 2) balances[parts[0]] = parts[1];
    }
    return balances;
  });
}

async function waitForSwapBalance(page, asset, minimum, timeout = 300_000) {
  await page.waitForFunction(
    ({ asset: wantedAsset, minimum: wantedMinimum }) => {
      const rows = Array.from(document.querySelectorAll('.pfs-balance-list div'));
      for (const row of rows) {
        const parts = (row.innerText || '').split(/\n+/).map((part) => part.trim()).filter(Boolean);
        if (parts[0] !== wantedAsset) continue;
        const match = String(parts[1] || '').replace(/,/g, '').match(/[0-9]+(?:\.[0-9]+)?/);
        if (match && Number(match[0]) >= wantedMinimum) return true;
      }
      return false;
    },
    { asset, minimum },
    { timeout },
  );
}

async function waitForSwapBalanceAtMost(page, asset, maximum, timeout = 300_000) {
  await page.waitForFunction(
    ({ asset: wantedAsset, maximum: wantedMaximum }) => {
      const rows = Array.from(document.querySelectorAll('.pfs-balance-list div'));
      for (const row of rows) {
        const parts = (row.innerText || '').split(/\n+/).map((part) => part.trim()).filter(Boolean);
        if (parts[0] !== wantedAsset) continue;
        const match = String(parts[1] || '').replace(/,/g, '').match(/[0-9]+(?:\.[0-9]+)?/);
        if (match && Number(match[0]) <= wantedMaximum) return true;
      }
      return false;
    },
    { asset, maximum },
    { timeout },
  );
}

async function openSwap(page) {
  await page.locator('button').filter({ hasText: 'Swap' }).first().click({ timeout: 15_000 });
  await page.waitForSelector('text=/Move between assets/i', { timeout: 45_000 });
  await page.screenshot({ path: join(OUT_DIR, '10-swap-open.png'), fullPage: true });
}

async function waitForSwapButton(page, labelPattern, timeout = 180_000) {
  const button = page.locator('button').filter({ hasText: labelPattern }).first();
  await button.waitFor({ timeout });
  return button;
}

async function clickVisibleSwapAction(page, patterns, timeout = 240_000) {
  const deadline = Date.now() + timeout;
  let lastText = '';
  while (Date.now() < deadline) {
    lastText = await page.locator('body').innerText().catch(() => '');
    if (!patterns.some((pattern) => pattern.test('Submit swap') || pattern.test('Submit redemption')) && /Submit swap|Submit redemption/i.test(lastText)) {
      return 'submit_ready';
    }
    for (const pattern of patterns) {
      const button = page.locator('button').filter({ hasText: pattern }).first();
      const visible = await button.isVisible({ timeout: 1000 }).catch(() => false);
      if (!visible) continue;
      const disabled = await button.evaluate((node) => Boolean(node.disabled)).catch(() => true);
      if (!disabled) {
        await button.click({ timeout: 30_000 });
        return pattern.toString();
      }
    }
    await page.waitForTimeout(1500);
  }
  throw new Error(`timed out waiting for swap action ${patterns.map(String).join(', ')}; last body:\n${lastText}`);
}

async function runNavswapCycle(page, index, baseline = {}, { direction = 'subscribe', resetAfter = false } = {}) {
  if (direction === 'redeem') {
    const redeemButton = page.locator('.pfs-mode button').filter({ hasText: /^Redeem$/ }).first();
    await redeemButton.waitFor({ timeout: 60_000 });
    await redeemButton.click({ timeout: 30_000 });
    await page.waitForSelector('text=/You redeem/i', { timeout: 45_000 });
  } else {
    const mintButton = page.locator('.pfs-mode button').filter({ hasText: /^Mint$/ }).first();
    await mintButton.waitFor({ timeout: 60_000 });
    await mintButton.click({ timeout: 30_000 });
    await page.waitForSelector('text=/You mint/i', { timeout: 45_000 });
  }
  const amountInput = page.locator('input[aria-label^="Amount of"]').first();
  await amountInput.waitFor({ timeout: 60_000 });
  await amountInput.fill(NAVSWAP_AMOUNT);
  const label = direction === 'redeem' ? `redeem-${index}` : `swap-${index}`;
  await page.screenshot({ path: join(OUT_DIR, `${label}-ready.png`), fullPage: true });

  const bodyBefore = await page.locator('body').innerText();
  assertOk(!/Bridge fresh pfUSDC/i.test(bodyBefore), `swap ${index} asked for fresh bridge before submit:\n${bodyBefore}`);

  const submitPattern = direction === 'redeem' ? /Submit redemption/i : /Submit swap/i;
  if (!submitPattern.test(bodyBefore)) {
    const clicked = await clickVisibleSwapAction(page, [/Get quote/i, /Refresh quote/i], 240_000);
    if (clicked !== 'submit_ready') await waitForSwapButton(page, submitPattern, 240_000);
  }

  const before = await page.evaluate(() => ({
    navswap: window.__bridgeUx?.navswapResponses?.length || 0,
  }));
  await clickVisibleSwapAction(page, [submitPattern], 240_000);
  await page.waitForFunction(
    () => /Swap submitted|Swap needs attention|Make another swap/i.test(document.body.innerText || ''),
    null,
    { timeout: 900_000 },
  );
  const finalText = await page.locator('body').innerText();
  await page.screenshot({ path: join(OUT_DIR, `${label}-complete.png`), fullPage: true });
  assertOk(!/Swap needs attention/i.test(finalText), `swap ${index} failed in UX:\n${finalText}`);

  if (direction === 'redeem') {
    await waitForSwapBalanceAtMost(
      page,
      'a651',
      Number(baseline.a651 || 0) - (Number(NAVSWAP_AMOUNT) * index),
      300_000,
    );
  } else {
    await waitForSwapBalance(
      page,
      'a651',
      Number(baseline.a651 || 0) + (Number(NAVSWAP_AMOUNT) * index),
      300_000,
    );
  }
  const balances = await readSwapBalances(page);
  const details = await page.evaluate((snapshot) => {
    const state = window.__bridgeUx || { navswapResponses: [] };
    return {
      navswapResponses: (state.navswapResponses || []).slice(snapshot.navswap),
      text: document.body.innerText,
    };
  }, before);
  const runResponse = details.navswapResponses.find((entry) => entry.url.includes('/api/navswap/runs') && entry.body?.ok !== false);

  if (resetAfter) {
    await clickVisibleSwapAction(page, [/Make another swap/i], 120_000);
    await waitForSwapButton(page, /Submit swap|Submit redemption|Refresh quote|Get quote/i, 240_000);
  }

  return {
    index,
    balances,
    navswapResponses: details.navswapResponses,
    runResponse: runResponse?.body || null,
  };
}

async function runNavswapCycles(page) {
  if (!NAVSWAP_RUNS && !NAVSWAP_REDEEM_RUNS) return { mintCycles: [], redeemCycles: [] };
  assertOk(Number.isFinite(NAVSWAP_RUNS) && NAVSWAP_RUNS >= 0, 'NAVSWAP_UX_RUNS must be non-negative when set');
  assertOk(Number.isFinite(NAVSWAP_REDEEM_RUNS) && NAVSWAP_REDEEM_RUNS >= 0, 'NAVSWAP_UX_REDEEM_RUNS must be non-negative when set');
  assertOk(Number.isFinite(Number(NAVSWAP_AMOUNT)) && Number(NAVSWAP_AMOUNT) > 0, 'NAVSWAP_UX_AMOUNT must be positive');
  await openSwap(page);
  if (NAVSWAP_RUNS > 0) await waitForSwapBalance(page, 'pfUSDC', Number(NAVSWAP_AMOUNT) * NAVSWAP_RUNS * 6, 240_000).catch(async () => {
    const balances = await readSwapBalances(page);
    const pfusdc = parseUiAmount(balances.pfUSDC);
    assertOk(
      Number.isFinite(pfusdc) && pfusdc > 0,
      `swap page did not show a usable pfUSDC balance: ${JSON.stringify(balances)}`,
    );
  });
  const initialBalances = await readSwapBalances(page);
  const baseline = {
    pfUSDC: parseUiAmount(initialBalances.pfUSDC),
    a651: parseUiAmount(initialBalances.a651),
    pft: parseUiAmount(initialBalances['PFT fees']),
  };
  const cycles = [];
  for (let i = 1; i <= NAVSWAP_RUNS; i += 1) {
    console.log(`running UX NAVSwap cycle ${i}/${NAVSWAP_RUNS}`);
    cycles.push(await runNavswapCycle(page, i, baseline, {
      direction: 'subscribe',
      resetAfter: i < NAVSWAP_RUNS || NAVSWAP_REDEEM_RUNS > 0,
    }));
  }
  const redeemInitialBalances = await readSwapBalances(page);
  const redeemBaseline = {
    pfUSDC: parseUiAmount(redeemInitialBalances.pfUSDC),
    a651: parseUiAmount(redeemInitialBalances.a651),
    pft: parseUiAmount(redeemInitialBalances['PFT fees']),
  };
  assertOk(
    !NAVSWAP_REDEEM_RUNS || redeemBaseline.a651 >= Number(NAVSWAP_AMOUNT) * NAVSWAP_REDEEM_RUNS,
    `not enough a651 to run ${NAVSWAP_REDEEM_RUNS} redeem cycle(s): ${JSON.stringify(redeemInitialBalances)}`,
  );
  const redeemCycles = [];
  for (let i = 1; i <= NAVSWAP_REDEEM_RUNS; i += 1) {
    console.log(`running UX NAVSwap redeem cycle ${i}/${NAVSWAP_REDEEM_RUNS}`);
    redeemCycles.push(await runNavswapCycle(page, i, redeemBaseline, {
      direction: 'redeem',
      resetAfter: i < NAVSWAP_REDEEM_RUNS,
    }));
  }
  await page.locator('button').filter({ hasText: 'Wallet' }).first().click({ timeout: 15_000 });
  await page.waitForTimeout(2500);
  await page.screenshot({ path: join(OUT_DIR, '98-wallet-after-navswap.png'), fullPage: true });
  return { mintCycles: cycles, redeemCycles };
}

async function main() {
  assertOk(Number.isFinite(RUNS) && RUNS >= 0, 'BRIDGE_UX_RUNS must be non-negative');
  assertOk(
    RUNS > 0 || NAVSWAP_RUNS > 0 || NAVSWAP_REDEEM_RUNS > 0,
    'nothing to run: set BRIDGE_UX_RUNS, NAVSWAP_UX_RUNS, or NAVSWAP_UX_REDEEM_RUNS',
  );
  assertOk(RUNS > 0 || IMPORT_SEED, 'BRIDGE_UX_IMPORT_SEED is required when BRIDGE_UX_RUNS=0');
  assertOk(AMOUNT_ATOMS > 0n, 'BRIDGE_UX_AMOUNT must be positive');

  const status = runAgentd({ op: 'status' }, 30);
  assertOk(status.ok, `StakeHub agent is not ready: ${JSON.stringify(status)}`);

  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
  });
  const context = await browser.newContext({
    ignoreHTTPSErrors: true,
    viewport: { width: 1440, height: 1100 },
  });
  const page = await context.newPage();
  page.setDefaultTimeout(60_000);
  page.on('console', (message) => {
    const text = message.text();
    if (/bridge|relay|vault|MetaMask|USDC/i.test(text)) {
      console.log(`[browser:${message.type()}] ${text}`);
    }
  });

  await installStakehubProvider(page);
  const wallet = IMPORT_SEED ? await importWalletInUx(page) : await createWalletInUx(page);

  const cycles = [];
  let walletText = '';
  if (RUNS > 0) {
    await openBridgeAndConnect(page);
    for (let i = 1; i <= RUNS; i += 1) {
      console.log(`running UX bridge cycle ${i}/${RUNS}`);
      cycles.push(await runBridgeCycle(page, i));
    }
    const expectedPfusdc = String(Number(AMOUNT) * RUNS);
    walletText = await verifyWalletBalance(page, expectedPfusdc);
  }

  const navswapResult = await runNavswapCycles(page);
  const report = {
    url: APP_URL,
    amount: AMOUNT,
    runs: RUNS,
    navswapAmount: NAVSWAP_AMOUNT,
    navswapRuns: NAVSWAP_RUNS,
    navswapRedeemRuns: NAVSWAP_REDEEM_RUNS,
    stakehubAddress: STAKEHUB_ADDRESS,
    pftlRecipient: wallet.accountAddress,
    outputDirectory: OUT_DIR,
    cycles,
    navswapCycles: navswapResult.mintCycles,
    navswapRedeemCycles: navswapResult.redeemCycles,
    walletText,
  };
  writeFileSync(join(OUT_DIR, 'report.json'), JSON.stringify(report, null, 2));
  await browser.close();
  console.log(JSON.stringify(report, null, 2));
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
