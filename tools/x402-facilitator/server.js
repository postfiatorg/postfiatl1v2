'use strict';

// Standalone CLI entrypoint for the Post Fiat x402 v2 facilitator.
// Defaults to loopback 127.0.0.1:4020; configure via environment variables.
//
// In `X402_MODE=production`, the facilitator requires BOTH:
//  - durable spent-payment storage (`X402_SPENT_STORE_PATH`), AND
//  - either an injected settlement adapter OR `X402_RPC_URLS` paired with pinned
//    `X402_COMMITTEE_JSON` + `X402_DOMAIN_JSON` so FastPay quorum votes and apply
//    acknowledgements are cryptographically authenticated against the active committee.

const { CommitteeRegistry, FileSpentStore, createFacilitatorCore, createFacilitatorServer } = require('./facilitator');
const { FileCertificateStore, createChainSettlementAdapter } = require('./chain-adapter');

const MODE_PROTOTYPE = 'prototype';
const MODE_PRODUCTION = 'production';

function isSettlementAdapter(adapter) {
  return typeof adapter?.settleFastPay === 'function' && typeof adapter?.settleConsensus === 'function';
}

function parseJsonEnv(raw, name) {
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch (err) {
    throw new Error(`${name} must be valid JSON: ${err.message}`);
  }
}

function createFacilitatorFromEnv(env = process.env, { settlementAdapter = null } = {}) {
  const mode = String(env.X402_MODE || MODE_PROTOTYPE).trim().toLowerCase();
  if (mode !== MODE_PROTOTYPE && mode !== MODE_PRODUCTION) {
    throw new Error(`X402_MODE must be "${MODE_PROTOTYPE}" or "${MODE_PRODUCTION}", got "${mode}"`);
  }

  const committeeCfg = parseJsonEnv(String(env.X402_COMMITTEE_JSON || '').trim(), 'X402_COMMITTEE_JSON');
  const expectedDomain = parseJsonEnv(String(env.X402_DOMAIN_JSON || '').trim(), 'X402_DOMAIN_JSON');
  const committeeRegistry = new CommitteeRegistry(
    committeeCfg ? { ...committeeCfg, domain: expectedDomain || committeeCfg.domain || null } : null
  );

  let resolvedAdapter = settlementAdapter;
  const rpcUrlsRaw = String(env.X402_RPC_URLS || '').trim();
  const quorum = env.X402_QUORUM_THRESHOLD ? Number(env.X402_QUORUM_THRESHOLD) : null;
  const finalityMode = String(env.X402_SETTLEMENT_FINALITY_MODE || '').trim().toLowerCase() || null;
  const certStorePath = String(env.X402_CERT_STORE_PATH || '').trim();
  const coLocatedValidatorId = String(env.X402_COLOCATED_VALIDATOR_ID || '').trim() || null;
  const certificateStore = certStorePath ? new FileCertificateStore(certStorePath) : null;

  if (!resolvedAdapter && rpcUrlsRaw) {
    if (mode === MODE_PRODUCTION) {
      if (committeeCfg && expectedDomain) {
        if (finalityMode === 'certificate' && !certificateStore) {
          throw new Error(
            'X402_SETTLEMENT_FINALITY_MODE=certificate in production requires X402_CERT_STORE_PATH for durable certificate persistence'
          );
        }
        resolvedAdapter = createChainSettlementAdapter({
          rpcUrls: rpcUrlsRaw,
          quorumThreshold: quorum,
          committeeRegistry,
          expectedDomain,
          requireAuthenticatedCommittee: true,
          settlementFinalityMode: finalityMode,
          certificateStore,
          coLocatedValidatorId,
          rpcTimeoutMs: env.X402_RPC_TIMEOUT_MS ? Number(env.X402_RPC_TIMEOUT_MS) : undefined,
          finalityTimeoutMs: env.X402_FINALITY_TIMEOUT_MS ? Number(env.X402_FINALITY_TIMEOUT_MS) : undefined,
        });
      }
    } else {
      resolvedAdapter = createChainSettlementAdapter({
        rpcUrls: rpcUrlsRaw,
        quorumThreshold: quorum,
        committeeRegistry: committeeCfg ? committeeRegistry : null,
        expectedDomain,
        settlementFinalityMode: finalityMode,
        certificateStore,
        coLocatedValidatorId,
        rpcTimeoutMs: env.X402_RPC_TIMEOUT_MS ? Number(env.X402_RPC_TIMEOUT_MS) : undefined,
        finalityTimeoutMs: env.X402_FINALITY_TIMEOUT_MS ? Number(env.X402_FINALITY_TIMEOUT_MS) : undefined,
      });
    }
  }

  if (mode === MODE_PRODUCTION && !isSettlementAdapter(resolvedAdapter)) {
    throw new Error(
      `X402_MODE=${MODE_PRODUCTION} requires an injected settlement adapter or X402_RPC_URLS paired with pinned ` +
        'X402_COMMITTEE_JSON and X402_DOMAIN_JSON for verified chain certificates'
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
    chainId: env.X402_CHAIN_ID || expectedDomain?.chain_id || 'postfiat-devnet-2',
    network: env.X402_NETWORK || 'postfiat:devnet-2',
    facilitatorUrl: env.X402_PUBLIC_URL || `http://${listenHost}:${listenPort}`,
    spentStore: new FileSpentStore(spentStorePath),
    committeeRegistry,
    expectedDomain,
    requireNativePayloads: mode === MODE_PRODUCTION,
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

  if (typeof core.settlementAdapter?.replayPendingCertificates === 'function') {
    Promise.resolve(core.settlementAdapter.replayPendingCertificates()).catch((err) => {
      console.error(
        JSON.stringify({
          event: 'x402_pending_certificate_replay_error',
          error: String(err?.message || err),
        })
      );
    });
  }

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
