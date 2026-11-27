'use strict';

// Standalone CLI entrypoint for the Post Fiat x402 v2 facilitator.
// Defaults to loopback 127.0.0.1:4020; configure via environment variables.

const {
  FileSpentStore,
  MemorySpentStore,
  createFacilitatorCore,
  createFacilitatorServer,
} = require('./facilitator');

function main() {
  const listenHost = process.env.X402_LISTEN_HOST || '127.0.0.1';
  const listenPort = Number(process.env.X402_LISTEN_PORT || 4020);
  const spentStorePath = String(process.env.X402_SPENT_STORE_PATH || '').trim();
  const spentStore = spentStorePath ? new FileSpentStore(spentStorePath) : new MemorySpentStore();

  const core = createFacilitatorCore({
    facilitatorId: process.env.X402_FACILITATOR_ID || 'postfiat-facilitator-v1',
    chainId: process.env.X402_CHAIN_ID || 'postfiat-devnet-2',
    network: process.env.X402_NETWORK || 'postfiat:devnet-2',
    facilitatorUrl: process.env.X402_PUBLIC_URL || `http://${listenHost}:${listenPort}`,
    spentStore,
    ipRateLimit: Number(process.env.X402_IP_RATE_LIMIT || 60),
    keyRateLimit: Number(process.env.X402_KEY_RATE_LIMIT || 60),
    rateWindowMs: Number(process.env.X402_RATE_WINDOW_MS || 60_000),
  });

  const server = createFacilitatorServer(core);
  server.listen(listenPort, listenHost, () => {
    console.log(
      JSON.stringify({
        event: 'x402_facilitator_started',
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
