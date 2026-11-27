'use strict';

// Standalone CLI entrypoint for the Post Fiat x402 v2 facilitator.
// Defaults to loopback 127.0.0.1:4020; configure via environment variables.
//
// The daemon started from this file is a prototype: it has no settlement adapter, so it can
// issue and check challenges but settles nothing on the chain (see the top of facilitator.js).

const { FileSpentStore, createFacilitatorCore, createFacilitatorServer } = require('./facilitator');
const { createChainSettlementAdapter } = require('./chain-adapter');

const MODE_PROTOTYPE = 'prototype';
const MODE_PRODUCTION = 'production';

function isSettlementAdapter(adapter) {
  return typeof adapter?.settleFastPay === 'function' && typeof adapter?.settleConsensus === 'function';
}

// Throws when the environment does not describe a facilitator that may run.
// When `X402_RPC_URLS` is configured and no custom `settlementAdapter` is injected,
// builds the live postfiatl1v2 JSON-RPC settlement adapter automatically.
function createFacilitatorFromEnv(env = process.env, { settlementAdapter = null } = {}) {
  const mode = String(env.X402_MODE || MODE_PROTOTYPE).trim().toLowerCase();
  if (mode !== MODE_PROTOTYPE && mode !== MODE_PRODUCTION) {
    throw new Error(`X402_MODE must be "${MODE_PROTOTYPE}" or "${MODE_PRODUCTION}", got "${mode}"`);
  }
  let resolvedAdapter = settlementAdapter;
  const rpcUrlsRaw = String(env.X402_RPC_URLS || '').trim();
  if (!resolvedAdapter && rpcUrlsRaw) {
    const quorum = env.X402_QUORUM_THRESHOLD ? Number(env.X402_QUORUM_THRESHOLD) : null;
    resolvedAdapter = createChainSettlementAdapter({
      rpcUrls: rpcUrlsRaw,
      quorumThreshold: quorum,
      rpcTimeoutMs: env.X402_RPC_TIMEOUT_MS ? Number(env.X402_RPC_TIMEOUT_MS) : undefined,
      finalityTimeoutMs: env.X402_FINALITY_TIMEOUT_MS ? Number(env.X402_FINALITY_TIMEOUT_MS) : undefined,
    });
  }
  if (mode === MODE_PRODUCTION && !isSettlementAdapter(resolvedAdapter)) {
    throw new Error(
      `X402_MODE=${MODE_PRODUCTION} requires a settlement adapter (set X402_RPC_URLS or inject settlementAdapter) ` +
        'backed by postfiatl1v2 validator JSON-RPC endpoints'
    );
  }

  const listenHost = env.X402_LISTEN_HOST || '127.0.0.1';
  const listenPort = Number(env.X402_LISTEN_PORT || 4020);
  const spentStorePath = String(env.X402_SPENT_STORE_PATH || '').trim();
  if (!spentStorePath) {
    throw new Error(
      'X402_SPENT_STORE_PATH is required: the daemon keeps used payments in that file, so a restart does not forget them'
    );
  }

  const core = createFacilitatorCore({
    facilitatorId: env.X402_FACILITATOR_ID || 'postfiat-facilitator-v1',
    chainId: env.X402_CHAIN_ID || 'postfiat-devnet-2',
    network: env.X402_NETWORK || 'postfiat:devnet-2',
    facilitatorUrl: env.X402_PUBLIC_URL || `http://${listenHost}:${listenPort}`,
    spentStore: new FileSpentStore(spentStorePath),
    settlementAdapter: resolvedAdapter,
    ipRateLimit: Number(env.X402_IP_RATE_LIMIT || 60),
    keyRateLimit: Number(env.X402_KEY_RATE_LIMIT || 60),
    rateWindowMs: Number(env.X402_RATE_WINDOW_MS || 60_000),
  });
  return { core, listenHost, listenPort, mode };
}

function main() {
  let facilitator;
  try {
    facilitator = createFacilitatorFromEnv();
  } catch (err) {
    console.error(JSON.stringify({ event: 'x402_facilitator_not_started', error: err.message }));
    process.exitCode = 1;
    return;
  }
  const { core, listenHost, listenPort, mode } = facilitator;

  const server = createFacilitatorServer(core);
  server.listen(listenPort, listenHost, () => {
    console.log(
      JSON.stringify({
        event: 'x402_facilitator_started',
        mode,
        listen: `http://${listenHost}:${listenPort}`,
        facilitator_id: core.facilitatorId,
        chain_id: core.chainId,
        network: core.network,
      })
    );
  });
}

if (require.main === module) {
  main();
}

module.exports = { createFacilitatorFromEnv };
