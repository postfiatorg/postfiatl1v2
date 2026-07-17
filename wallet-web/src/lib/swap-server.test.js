import assert from 'node:assert/strict';
import test from 'node:test';

import { SwapServer, defaultSwapServerUrl, normalizeSwapServerUrl } from './swap-server.js';
import {
  clearCustodyMaterialRegistry,
  registerCustodyMaterial,
} from './custody-boundary.js';

function withLocation(location, fn) {
  const previousWindow = globalThis.window;
  Object.defineProperty(globalThis, 'window', {
    value: { location },
    configurable: true,
  });
  try {
    fn();
  } finally {
    if (previousWindow === undefined) {
      delete globalThis.window;
    } else {
      Object.defineProperty(globalThis, 'window', {
        value: previousWindow,
        configurable: true,
      });
    }
  }
}

test('defaultSwapServerUrl uses wallet proxy for local Vite dev server', () => {
  withLocation({
    protocol: 'http:',
    hostname: '127.0.0.1',
    host: '127.0.0.1:5173',
    port: '5173',
  }, () => {
    assert.equal(defaultSwapServerUrl(), 'http://127.0.0.1:8080');
  });
});

test('defaultSwapServerUrl uses same-origin adapter on HTTPS Vite dev server', () => {
  withLocation({
    protocol: 'https:',
    hostname: '192.0.2.1',
    host: '192.0.2.1:5173',
    port: '5173',
  }, () => {
  assert.equal(defaultSwapServerUrl(), 'https://192.0.2.1:5173');
  });
});

test('normalizeSwapServerUrl migrates stale loopback adapter on public HTTPS wallet', () => {
  withLocation({
    protocol: 'https:',
    hostname: '192.0.2.1',
    host: '192.0.2.1:5173',
    port: '5173',
  }, () => {
  assert.equal(normalizeSwapServerUrl('http://localhost:8787'), 'https://192.0.2.1:5173');
  });
});

test('SwapServer calls navswap capabilities endpoint', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return { ok: true, schema: 'postfiat-navswap-capabilities-v1' };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080/');
    const caps = await client.getNavswapCapabilities();
    assert.equal(caps.schema, 'postfiat-navswap-capabilities-v1');
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/navswap/capabilities');
    assert.equal(calls[0].options.method, 'GET');
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer calls the certified private-swap workflow endpoint', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return { ok: true, async json() { return { ok: true, complete: true }; } };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8790');
    await client.runPrivateSwapWorkflow({ run_id: 'ux-test', no_money: true });
    assert.equal(calls[0].url, 'http://127.0.0.1:8790/api/private-swap-workflow');
    assert.equal(calls[0].options.method, 'POST');
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer rejects active wallet seed material on every HTTP route', async () => {
  const seed = 'b8'.repeat(32);
  let fetchCalled = false;
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async () => {
    fetchCalled = true;
    throw new Error('fetch must not run');
  };
  registerCustodyMaterial({ seed });
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    await assert.rejects(
      () => client.runNavswap({ route: 'transparent_navswap', metadata: seed }),
      /registered-secret-value/,
    );
    assert.equal(fetchCalled, false);
  } finally {
    clearCustodyMaterialRegistry();
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer exposes disabled navswap route errors with response payload', async () => {
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async () => ({
    ok: false,
    status: 409,
    async json() {
      return {
        ok: false,
        code: 'legacy_pool_rejected',
        message: 'legacy pool rejected',
      };
    },
  });
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    await assert.rejects(
      () => client.quoteNavswap({ route: 'uniswap_atomic_handoff' }),
      (err) => {
        assert.equal(err.status, 409);
        assert.equal(err.data.code, 'legacy_pool_rejected');
        assert.match(err.message, /legacy pool rejected/);
        return true;
      },
    );
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer calls navswap run status endpoints', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return { ok: true, run_id: 'run/1' };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    await client.getNavswapRun('run/1');
    await client.getNavswapRunEvents('run/1');
    await client.getNavswapRunReceipts('run/1');
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/navswap/runs/run%2F1');
    assert.equal(calls[1].url, 'http://127.0.0.1:8080/api/navswap/runs/run%2F1/events');
    assert.equal(calls[2].url, 'http://127.0.0.1:8080/api/navswap/runs/run%2F1/receipts');
    assert.deepEqual(calls.map((call) => call.options.method), ['GET', 'GET', 'GET']);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer calls navswap run list endpoint', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return { ok: true, schema: 'postfiat-navswap-run-list-v1', runs: [] };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    const list = await client.getNavswapRuns({
      walletAddress: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      route: 'transparent_navswap',
      limit: 1,
    });
    assert.equal(list.schema, 'postfiat-navswap-run-list-v1');
    assert.equal(
      calls[0].url,
      'http://127.0.0.1:8080/api/navswap/runs?wallet_address=pf124071fd53a12ca4556b7aa1f5ec98b585e73468&route=transparent_navswap&limit=1',
    );
    assert.equal(calls[0].options.method, 'GET');
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer exposes navswap run stream URL', () => {
  const client = new SwapServer('http://127.0.0.1:8080/');
  assert.equal(
    client.navswapRunStreamUrl('run/1'),
    'http://127.0.0.1:8080/api/navswap/runs/run%2F1/stream',
  );
});

test('SwapServer calls navswap action prepare endpoint', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return {
          ok: true,
          schema: 'postfiat-navswap-wallet-action-prepare-v1',
          action: { schema: 'postfiat-navswap-wallet-action-request-v1' },
        };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    const body = {
      route: 'transparent_navswap',
      stage: 'trust_set',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      asset_id: 'a'.repeat(96),
      limit_atoms: '1000000',
    };
    const prepared = await client.prepareNavswapAction(body);
    assert.equal(prepared.schema, 'postfiat-navswap-wallet-action-prepare-v1');
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/navswap/actions/prepare');
    assert.equal(calls[0].options.method, 'POST');
    assert.deepEqual(JSON.parse(calls[0].options.body), body);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer calls navswap action batch prepare endpoint', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return {
          ok: true,
          schema: 'postfiat-navswap-wallet-action-batch-prepare-v1',
          actions: [{ schema: 'postfiat-navswap-wallet-action-request-v1' }],
        };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    const body = {
      route: 'transparent_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      actions: [
        {
          stage: 'trust_set',
          asset_id: 'a'.repeat(96),
          limit_atoms: '1000000',
        },
      ],
    };
    const prepared = await client.prepareNavswapActionBatch(body);
    assert.equal(prepared.schema, 'postfiat-navswap-wallet-action-batch-prepare-v1');
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/navswap/actions/prepare-batch');
    assert.equal(calls[0].options.method, 'POST');
    assert.deepEqual(JSON.parse(calls[0].options.body), body);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer calls navswap planner inputs endpoint', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return {
          ok: true,
          schema: 'postfiat-navswap-transparent-planner-inputs-v1',
          actions: [{ stage: 'trust_set' }],
        };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    const body = {
      route: 'transparent_navswap',
      from_asset: '8'.repeat(96),
      to_asset: 'd'.repeat(96),
      amount: '1000000',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    };
    const plan = await client.planNavswapInputs(body);
    assert.equal(plan.schema, 'postfiat-navswap-transparent-planner-inputs-v1');
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/navswap/planner-inputs');
    assert.equal(calls[0].options.method, 'POST');
    assert.deepEqual(JSON.parse(calls[0].options.body), body);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer calls navswap readiness endpoint', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return { ok: true, schema: 'postfiat-navswap-readiness-v1' };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    const readiness = await client.getNavswapReadiness({
      route: 'transparent_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    });
    assert.equal(readiness.schema, 'postfiat-navswap-readiness-v1');
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/navswap/readiness');
    assert.equal(calls[0].options.method, 'POST');
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer calls navswap devnet pfUSDC funding endpoint', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return { ok: true, schema: 'postfiat-navswap-devnet-funding-v1', tx_id: 'funding-tx' };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    const body = {
      route: 'transparent_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      amount: '1',
    };
    const funding = await client.fundNavswapPfusdc(body);
    assert.equal(funding.schema, 'postfiat-navswap-devnet-funding-v1');
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/navswap/devnet-fund-pfusdc');
    assert.equal(calls[0].options.method, 'POST');
    const submitted = JSON.parse(calls[0].options.body);
    assert.match(submitted.idempotency_key, /^navswap-funding:/);
    delete submitted.idempotency_key;
    assert.deepEqual(submitted, body);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer adds idempotency key to navswap run requests', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return { ok: true, schema: 'postfiat-navswap-run-v1', run_id: 'run-1' };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    const body = {
      route: 'transparent_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      async: true,
    };
    const run = await client.runNavswap(body);
    assert.equal(run.schema, 'postfiat-navswap-run-v1');
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/navswap/runs');
    assert.equal(calls[0].options.method, 'POST');
    const submitted = JSON.parse(calls[0].options.body);
    assert.match(submitted.idempotency_key, /^navswap-run:/);
    delete submitted.idempotency_key;
    assert.deepEqual(submitted, body);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer calls navswap atomic template endpoint', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return {
          ok: true,
          schema: 'postfiat-navswap-atomic-template-v1',
          verification: { settlement_id: 'settlement' },
        };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    const body = {
      left_owner: 'pf-left',
      left_recipient: 'pf-right',
      left_asset_id: 'PFT',
      left_amount: '1',
      right_owner: 'pf-right',
      right_recipient: 'pf-left',
      right_asset_id: 'a651',
      right_amount: '1',
      condition: 'condition',
      cancel_after: 100,
    };
    const template = await client.buildAtomicSettlementTemplate(body);
    assert.equal(template.schema, 'postfiat-navswap-atomic-template-v1');
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/navswap/atomic-templates');
    assert.equal(calls[0].options.method, 'POST');
    assert.deepEqual(JSON.parse(calls[0].options.body), body);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer attaches the session-only bearer to mutation requests', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return { ok: true, async json() { return { ok: true }; } };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080', 'session-token');
    await client.buildAtomicSettlementTemplate({ left_owner: 'pf-left' });
    assert.equal(calls[0].options.headers.Authorization, 'Bearer session-token');
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer exposes shielded NAVSwap read-only and preflight endpoints', async () => {
  const calls = [];
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async (url, options) => {
    calls.push({ url, options });
    return {
      ok: true,
      async json() {
        return { ok: true, schema: 'postfiat-shielded-navswap-preflight-v1' };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    await client.getShieldedNavswapStatus();
    await client.getShieldedNavswapBalances();
    await client.getShieldedNavswapNoteCapability();
    await client.getShieldedNavswapProverReadiness();
    await client.getShieldedNavswapQuote({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      from_asset: 'a651',
      to_asset: 'a652',
      amount_atoms: '1000000',
    });
    await client.getShieldedNavswapPreflight({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    });
    await client.submitShieldedNavswapIngress({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      ingress_payload: { asset_id: 'aa'.repeat(48) },
    });
    await client.submitShieldedNavswapSwap({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      quote: { ok: true },
      swap_action_json: '{"schema":"postfiat-asset-orchard-swap-action-v1"}',
    });
    await client.submitShieldedNavswapEgress({
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      to: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      asset_id: 'aa'.repeat(48),
      amount_atoms: '2000',
      note_commitment: 'bb'.repeat(32),
      policy_id: 'wallet_private_egress_public_exit_v1',
      disclosure_hash: 'cc'.repeat(32),
      disclosure_ack: true,
      egress_json: '{"schema":"postfiat-asset-orchard-private-egress-file-v1","payload":{}}',
    });
    assert.deepEqual(calls.map(call => call.options.method), ['GET', 'GET', 'GET', 'GET', 'POST', 'POST', 'POST', 'POST', 'POST']);
    assert.equal(calls[0].url, 'http://127.0.0.1:8080/api/shielded-nav-swap/status');
    assert.equal(calls[4].url, 'http://127.0.0.1:8080/api/shielded-nav-swap/quote');
    assert.equal(calls[5].url, 'http://127.0.0.1:8080/api/shielded-nav-swap/preflight');
    assert.equal(calls[6].url, 'http://127.0.0.1:8080/api/shielded-nav-swap/ingress');
    assert.equal(calls[7].url, 'http://127.0.0.1:8080/api/shielded-nav-swap/swap');
    assert.equal(calls[8].url, 'http://127.0.0.1:8080/api/shielded-nav-swap/egress');
    assert.deepEqual(JSON.parse(calls[5].options.body), {
      route: 'shielded_navswap',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
    });
    assert.match(JSON.parse(calls[6].options.body).idempotency_key, /^shielded-navswap-ingress:/);
    assert.match(JSON.parse(calls[8].options.body).idempotency_key, /^shielded-navswap-egress:/);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer rejects shielded private material before endpoint submission', async () => {
  let fetchCalled = false;
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async () => {
    fetchCalled = true;
    return {
      ok: true,
      async json() {
        return { ok: true };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    await assert.rejects(
      () => client.getShieldedNavswapPreflight({
        route: 'shielded_navswap',
        wallet_address: 'pfwallet',
        backup_json: { seed: 'secret' },
      }),
      /forbidden private wallet material/,
    );
    await assert.rejects(
      () => client.submitShieldedNavswapIngress({
        route: 'shielded_navswap',
        wallet_address: 'pfwallet',
        note_seed_hex: '00'.repeat(32),
      }),
      /forbidden private wallet material/,
    );
    await assert.rejects(
      () => client.getShieldedNavswapQuote({
        route: 'shielded_navswap',
        wallet_address: 'pfwallet',
        backup_json: { seed: 'secret' },
      }),
      /forbidden private wallet material/,
    );
    await assert.rejects(
      () => client.submitShieldedNavswapSwap({
        route: 'shielded_navswap',
        wallet_address: 'pfwallet',
        note_opening: 'secret',
      }),
      /forbidden private wallet material/,
    );
    await assert.rejects(
      () => client.submitShieldedNavswapSwap({
        route: 'shielded_navswap',
        wallet_address: 'pfwallet',
        swap_action_json: JSON.stringify({
          schema: 'postfiat-asset-orchard-swap-action-v1',
          private_witness: { diversifier: '01'.repeat(11) },
        }),
      }),
      /forbidden private wallet material/,
    );
    await assert.rejects(
      () => client.submitShieldedNavswapEgress({
        route: 'shielded_navswap',
        wallet_address: 'pfwallet',
        note_file: '/tmp/local-note.json',
      }),
      /forbidden private wallet material/,
    );
    assert.equal(fetchCalled, false);
  } finally {
    globalThis.fetch = previousFetch;
  }
});

test('SwapServer blocks shielded NAVSwap quote, run, and prepared action paths', async () => {
  let fetchCalled = false;
  const previousFetch = globalThis.fetch;
  globalThis.fetch = async () => {
    fetchCalled = true;
    return {
      ok: true,
      async json() {
        return { ok: true };
      },
    };
  };
  try {
    const client = new SwapServer('http://127.0.0.1:8080');
    await assert.rejects(
      () => client.quoteNavswap({ route: 'shielded_navswap', wallet_address: 'pfwallet' }),
      /disabled until the Step 7 private swap submit gate/,
    );
    await assert.rejects(
      () => client.runNavswap({ route: 'shielded_navswap', wallet_address: 'pfwallet' }),
      /disabled until the Step 7 private swap submit gate/,
    );
    await assert.rejects(
      () => client.prepareNavswapActionBatch({ route: 'shielded_navswap', wallet_address: 'pfwallet' }),
      /disabled until the Step 7 private swap submit gate/,
    );
    assert.equal(fetchCalled, false);
  } finally {
    globalThis.fetch = previousFetch;
  }
});
