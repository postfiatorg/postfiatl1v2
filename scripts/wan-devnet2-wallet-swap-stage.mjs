#!/usr/bin/env node
import { execFile } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { promisify } from 'node:util';
import { tmpdir } from 'node:os';

const execFileAsync = promisify(execFile);

const REPO_ROOT = path.resolve(new URL('..', import.meta.url).pathname);
const RESET_ROOT = process.env.POSTFIAT_WAN_RESET_ROOT || '';
const DEFAULT_CHAIN_ID = 'postfiat-wan-devnet-2';
const DEFAULT_GENESIS_HASH = '46da6c340d27d9140bd9d9a2fc0cb81064b0bfa662d5981d2e2b2de6960f06cd22ef4f790cb35f8d2e20f771f595ff10';
const DEFAULT_TOPOLOGY_ID = '7670f1db668fb61df40b89f93160681fe61cb1de242fc2c675cf9e793843a1a846f6cc80c76831b5475b8640fe715c48';
const GO_TOKEN = 'POSTFIAT_WAN_DEVNET2_WALLET_SWAP_GO';

function usage() {
  return `Usage:
  node scripts/wan-devnet2-wallet-swap-stage.mjs [--out-dir DIR]
  ${GO_TOKEN}=I_UNDERSTAND_THIS_SUBMITS_TO_THE_SHARED_FLEET node scripts/wan-devnet2-wallet-swap-stage.mjs --go --out-dir DIR

Default mode is dry-run staging only: it writes env files and GO commands, copies local seed data, runs local verifier checks,
and performs read-only public RPC preflight. It never submits to the fleet.

Options:
  --out-dir DIR          Run artifact directory. Default: temporary local directory
  --reset-root DIR       Reset artifact root. Required unless POSTFIAT_WAN_RESET_ROOT is set
  --node-bin FILE        postfiat-node binary. Default: target/release/postfiat-node
  --topology FILE        Remote topology JSON. Default: <reset-root>/remote-topology.json
  --local-validator N    Local seed validator to copy. Default: 0
  --skip-rpc             Skip read-only public RPC preflight.
  --go                  Only writes the guarded GO packet. Still does not execute live commands in this script.
`;
}

function utcStamp() {
  return new Date().toISOString().replace(/[-:.]/g, '').replace('Z', 'Z');
}

function parseArgs(argv) {
  const args = {
    outDir: path.join(tmpdir(), `wan_devnet2_wallet_swap_stage_${utcStamp()}`),
    resetRoot: RESET_ROOT,
    nodeBin: path.join(REPO_ROOT, 'target', 'release', 'postfiat-node'),
    localValidator: '0',
    skipRpc: false,
    go: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--help' || arg === '-h') {
      args.help = true;
    } else if (arg === '--skip-rpc') {
      args.skipRpc = true;
    } else if (arg === '--go') {
      args.go = true;
    } else if (arg.startsWith('--')) {
      const key = arg.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase());
      const value = argv[i + 1];
      if (!value || value.startsWith('--')) throw new Error(`${arg} requires a value`);
      args[key] = value;
      i += 1;
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }
  if (!args.resetRoot) throw new Error('--reset-root or POSTFIAT_WAN_RESET_ROOT is required');
  args.topology = args.topology || path.join(args.resetRoot, 'remote-topology.json');
  return args;
}

function jsonReplacer(_key, value) {
  return typeof value === 'bigint' ? value.toString() : value;
}

async function writeJson(file, value) {
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, `${JSON.stringify(value, jsonReplacer, 2)}\n`);
}

async function writeText(file, text, mode = 0o644) {
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, text, { mode });
}

async function readJson(file) {
  return JSON.parse(await fs.readFile(file, 'utf8'));
}

async function exists(file) {
  try {
    await fs.access(file);
    return true;
  } catch (_) {
    return false;
  }
}

async function runCommand(label, command, args, options = {}) {
  const started = Date.now();
  const record = {
    label,
    command,
    args,
    cwd: options.cwd || REPO_ROOT,
    ok: false,
    duration_ms: null,
    stdout: '',
    stderr: '',
  };
  try {
    const result = await execFileAsync(command, args, {
      cwd: record.cwd,
      env: { ...process.env, ...(options.env || {}) },
      timeout: options.timeoutMs || 120000,
      maxBuffer: options.maxBuffer || 16 * 1024 * 1024,
    });
    record.ok = true;
    record.stdout = result.stdout;
    record.stderr = result.stderr;
  } catch (error) {
    record.ok = false;
    record.exit_code = error.code ?? null;
    record.signal = error.signal ?? null;
    record.stdout = error.stdout || '';
    record.stderr = error.stderr || error.message || '';
  } finally {
    record.duration_ms = Date.now() - started;
  }
  return record;
}

function validatorsArg(topology) {
  return topology.peers
    .map(peer => `${peer.node_id}=${peer.host}:${peer.rpc_port}`)
    .join(',');
}

function hostList(topology) {
  return topology.peers.map(peer => peer.host).join(',');
}

function rpcPortList(topology) {
  return topology.peers.map(peer => peer.rpc_port).join(',');
}

function shellQuote(value) {
  return `'${String(value).replace(/'/g, "'\\''")}'`;
}

async function copyLocalSeed(args, outDir) {
  const source = path.join(args.resetRoot, 'seed', `validator-${args.localValidator}`);
  const dest = path.join(outDir, 'local-stack', `validator-${args.localValidator}`);
  if (!(await exists(source))) {
    throw new Error(`missing local seed source: ${source}`);
  }
  await fs.rm(dest, { recursive: true, force: true });
  await fs.mkdir(path.dirname(dest), { recursive: true });
  await fs.cp(source, dest, { recursive: true, dereference: false });
  return { source, dest };
}

function envText(entries) {
  return `${Object.entries(entries)
    .map(([key, value]) => `${key}=${shellQuote(value)}`)
    .join('\n')}\n`;
}

function buildEnvPackets({ args, outDir, topology, localDataDir }) {
  const artifactRoot = path.join(outDir, 'wallet-proxy-artifacts');
  const certifierRoot = path.join(artifactRoot, 'certifier-loop');
  const topologyFile = path.join(outDir, 'remote-topology.json');
  const validatorKey = path.join(localDataDir, 'validator_keys.json');
  const common = {
    POSTFIAT_CHAIN_ID: topology.chain_id,
    VITE_POSTFIAT_CHAIN_ID: topology.chain_id,
    POSTFIAT_GENESIS_HASH: topology.genesis_hash,
    VITE_POSTFIAT_GENESIS_HASH: topology.genesis_hash,
    POSTFIAT_TOPOLOGY: topologyFile,
    NAVSWAP_SHIELDED_INGRESS_TOPOLOGY: topologyFile,
    POSTFIAT_DATA_DIR: localDataDir,
    NAVSWAP_SHIELDED_INGRESS_DATA_DIR: localDataDir,
    POSTFIAT_VALIDATOR_KEY_FILE: validatorKey,
    NAVSWAP_SHIELDED_INGRESS_KEY_FILE: validatorKey,
    POSTFIAT_NODE_BIN: args.nodeBin,
    NAVSWAP_SHIELDED_INGRESS_NODE_BIN: args.nodeBin,
    ASSET_ORCHARD_LOCAL_SERVICE_URL: 'http://127.0.0.1:8789',
    ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS: '60000',
    NAVSWAP_ENABLE_SHIELDED_QUOTES: 'true',
    NAVSWAP_ENABLE_SHIELDED_INGRESS: 'true',
    NAVSWAP_SHIELDED_LIQUIDITY_MODE: 'pool_managed_note',
    NAVSWAP_SHIELDED_LIQUIDITY_PROVIDER: 'controlled_pool_operator',
    NAVSWAP_SHIELDED_QUOTE_TTL_MS: '300000',
    NAVSWAP_SHIELDED_INGRESS_TIMEOUT_MS: '120000',
    NAVSWAP_SHIELDED_INGRESS_ARTIFACT_ROOT: path.join(artifactRoot, 'ingress'),
    NAVSWAP_SHIELDED_SWAP_ARTIFACT_ROOT: path.join(artifactRoot, 'swaps'),
    NAVSWAP_SHIELDED_CERTIFIER_LOOP_ROOT: certifierRoot,
    NAVSWAP_SHIELDED_CERTIFIER_LOOP_BATCH_DIR: path.join(certifierRoot, 'batches'),
    NAVSWAP_SHIELDED_CERTIFIER_LOOP_ARTIFACT_ROOT: path.join(certifierRoot, 'artifacts'),
    NAVSWAP_SHIELDED_CERTIFIER_LOOP_PROCESSED_DIR: path.join(certifierRoot, 'processed'),
    NAVSWAP_SHIELDED_CERTIFIER_READY_FILE: path.join(certifierRoot, 'ready.json'),
    NAVSWAP_SHIELDED_CERTIFIER_REPORT_FILE: path.join(certifierRoot, 'loop-report.json'),
    NAVSWAP_SHIELDED_CERTIFIER_LOOP_POLL_MS: '250',
    NAVSWAP_SHIELDED_ROUND_PREWARM: 'true',
    POSTFIAT_SHIELDED_ROUND_PREWARM: 'true',
    SHIELDED_EARLY_QUORUM: 'true',
    VALIDATOR_HOSTS: hostList(topology),
    VALIDATOR_RPC_PORTS: rpcPortList(topology),
    ORCHARD_SWAP_E2E_RPC: process.env.ORCHARD_SWAP_E2E_RPC || 'ws://127.0.0.1:8080/rpc',
  };
  const dryRun = {
    ...common,
    NAVSWAP_ENABLE_SHIELDED_SWAPS: 'false',
    NAVSWAP_ENABLE_SHIELDED_EGRESS: 'false',
    NAVSWAP_SHIELDED_CERTIFIER_LOOP: 'false',
    CAN_RUN_GUARD: 'dry-run-only; shielded swap submit stays disabled',
  };
  const go = {
    ...common,
    NAVSWAP_ENABLE_SHIELDED_SWAPS: 'true',
    NAVSWAP_ENABLE_SHIELDED_EGRESS: 'true',
    NAVSWAP_SHIELDED_CERTIFIER_LOOP: 'true',
    ORCHARD_SWAP_E2E_STEP: 'step10-pair',
    ORCHARD_SWAP_E2E_LIVE_WINDOW: 'true',
    ORCHARD_SWAP_E2E_RUNS: '1',
    ORCHARD_SWAP_E2E_PLAN: 'a651->a652',
    ORCHARD_SWAP_E2E_ZERO_REPAIR: 'true',
    ORCHARD_SWAP_E2E_OUT_DIR: path.join(outDir, 'live-warm-swap-evidence'),
    CAN_RUN_GUARD: `${GO_TOKEN}=I_UNDERSTAND_THIS_SUBMITS_TO_THE_SHARED_FLEET required before sourcing this file`,
  };
  return { dryRun, go };
}

function buildGoCommands(outDir) {
  const goEnv = path.join(outDir, 'wallet-proxy-devnet2-go.env');
  const liveOut = path.join(outDir, 'live-warm-swap-evidence');
  return {
    guard: `${GO_TOKEN}=I_UNDERSTAND_THIS_SUBMITS_TO_THE_SHARED_FLEET`,
    warm_certifier_loop: `cd ${shellQuote(REPO_ROOT)} && set -a && . ${shellQuote(goEnv)} && set +a && node wallet-proxy/server.js`,
    live_warm_swap: `cd ${shellQuote(REPO_ROOT)} && set -a && . ${shellQuote(goEnv)} && set +a && node scripts/wallet-shielded-swap-step7-e2e.mjs`,
    package_evidence: `cd ${shellQuote(REPO_ROOT)} && STEP10_EVIDENCE_DIR=${shellQuote(liveOut)} node scripts/wallet-shielded-swap-step10-package.mjs`,
  };
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write(usage());
    return;
  }
  if (args.go && process.env[GO_TOKEN] !== 'I_UNDERSTAND_THIS_SUBMITS_TO_THE_SHARED_FLEET') {
    throw new Error(`--go requires ${GO_TOKEN}=I_UNDERSTAND_THIS_SUBMITS_TO_THE_SHARED_FLEET`);
  }

  const outDir = path.resolve(args.outDir);
  await fs.mkdir(outDir, { recursive: true });
  await writeText(path.join(outDir, 'README.txt'), [
    'postfiat-wan-devnet-2 wallet/swap staging packet',
    '',
    'Default dry-run packet only. No live fleet submissions were made by this script.',
    `Live commands require ${GO_TOKEN}=I_UNDERSTAND_THIS_SUBMITS_TO_THE_SHARED_FLEET and an operator GO line.`,
    '',
  ].join('\n'));

  const topology = await readJson(args.topology);
  if (topology.chain_id !== DEFAULT_CHAIN_ID) throw new Error(`unexpected chain_id ${topology.chain_id}`);
  if (topology.genesis_hash !== DEFAULT_GENESIS_HASH) throw new Error(`unexpected genesis_hash ${topology.genesis_hash}`);
  if (topology.topology_id !== DEFAULT_TOPOLOGY_ID) throw new Error(`unexpected topology_id ${topology.topology_id}`);
  await writeJson(path.join(outDir, 'remote-topology.json'), topology);

  const localSeed = await copyLocalSeed(args, outDir);
  const envPackets = buildEnvPackets({ args, outDir, topology, localDataDir: localSeed.dest });
  await writeText(path.join(outDir, 'wallet-proxy-devnet2-dry-run.env'), envText(envPackets.dryRun));
  await writeText(path.join(outDir, 'wallet-proxy-devnet2-go.env'), envText(envPackets.go), 0o600);

  const preflightFile = path.join(outDir, 'public-rpc-preflight-readonly.json');
  const checks = [];
  if (!args.skipRpc) {
    checks.push(await runCommand(
      'read-only public RPC preflight',
      'python3',
      [
        'scripts/wan-devnet-transaction-preflight',
        '--validators',
        validatorsArg(topology),
        '--output',
        preflightFile,
      ],
      { cwd: REPO_ROOT, timeoutMs: 120000 },
    ));
  }

  for (const command of ['status', 'verify-blocks', 'verify-state', 'verify-shielded', 'orchard-frontier-cache-warm']) {
    checks.push(await runCommand(
      `local seed ${command}`,
      args.nodeBin,
      [command, '--data-dir', localSeed.dest],
      { cwd: REPO_ROOT, timeoutMs: 120000 },
    ));
  }
  checks.push(await runCommand(
    'wallet-proxy warm certifier config test',
    'node',
    ['wallet-proxy/test_shielded_round_prewarm.js'],
    { cwd: REPO_ROOT, timeoutMs: 120000 },
  ));

  let preflight = null;
  if (await exists(preflightFile)) {
    preflight = await readJson(preflightFile);
  }
  const chainRows = (preflight?.validators || preflight?.entries || [])
    .map(entry => entry.status?.result)
    .filter(Boolean);
  const chainOk = chainRows.length === topology.peers.length
    && chainRows.every(row => row.chain_id === DEFAULT_CHAIN_ID && row.genesis_hash === DEFAULT_GENESIS_HASH);
  const mempoolOk = chainRows.every(row => Number(row.mempool_pending || 0) === 0);
  const allChecksOk = checks.every(check => check.ok);
  const commands = buildGoCommands(outDir);
  const plan = {
    schema: 'postfiat-wan-devnet2-wallet-swap-stage-v1',
    captured_at: new Date().toISOString(),
    repo_root: REPO_ROOT,
    out_dir: outDir,
    dry_run_validated: allChecksOk && (args.skipRpc || chainOk),
    live_devnet_rounds_executed: false,
    fleet_mutation: false,
    rpc_reads_only: !args.skipRpc,
    go_guard: {
      env: GO_TOKEN,
      required_value: 'I_UNDERSTAND_THIS_SUBMITS_TO_THE_SHARED_FLEET',
      go_mode_requested: args.go,
      go_mode_authorized: args.go && process.env[GO_TOKEN] === 'I_UNDERSTAND_THIS_SUBMITS_TO_THE_SHARED_FLEET',
    },
    chain: {
      chain_id: topology.chain_id,
      genesis_hash: topology.genesis_hash,
      topology_id: topology.topology_id,
      peers: topology.peers,
      read_only_chain_ok: chainOk,
      read_only_mempool_zero: mempoolOk,
      read_only_mempool_zero_is_go_window_check: true,
    },
    local_stack: {
      seed_source: localSeed.source,
      local_data_dir: localSeed.dest,
      topology_file: path.join(outDir, 'remote-topology.json'),
    },
    wallet_proxy_env: {
      dry_run_env: path.join(outDir, 'wallet-proxy-devnet2-dry-run.env'),
      go_env: path.join(outDir, 'wallet-proxy-devnet2-go.env'),
      dry_run_can_run: false,
      go_can_run_after_operator_line: true,
      warm_certifier_loop_configured_in_go_env: true,
      optional_transport_ready_file_envs: [
        'POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE',
        'POSTFIAT_TRANSPORT_BLOCK_VOTE_READY_FILE',
      ],
    },
    fresh_chain_milestone_plan: [
      'asset creation by guarded issuer/funder transaction in the Step 10 harness',
      'shielded pool bootstrap by guarded pool note ingress',
      'wallet note ingress by guarded wallet burn + ingress relay',
      'wallet-proxy pointed at postfiat-wan-devnet-2 topology and local validator seed',
      'warm certifier loop enabled only in GO env',
      'one warm private swap measurement and packaging after explicit GO',
    ],
    commands,
    checks: checks.map(check => ({
      label: check.label,
      ok: check.ok,
      duration_ms: check.duration_ms,
      exit_code: check.exit_code ?? 0,
      stdout_tail: check.stdout.slice(-2000),
      stderr_tail: check.stderr.slice(-2000),
    })),
  };
  await writeJson(path.join(outDir, 'stage-plan.json'), plan);
  await writeJson(path.join(outDir, 'stage-status.json'), {
    schema: 'postfiat-wan-devnet2-wallet-swap-stage-status-v1',
    captured_at: new Date().toISOString(),
    ok: plan.dry_run_validated,
    out_dir: outDir,
    dry_run_validated: plan.dry_run_validated,
    live_devnet_rounds_executed: false,
    fleet_mutation: false,
    next_gate: 'await founder GO after cbdc clears fleet',
    stage_plan: path.join(outDir, 'stage-plan.json'),
  });
  process.stdout.write(`${JSON.stringify({
    ok: plan.dry_run_validated,
    out_dir: outDir,
    live_devnet_rounds_executed: false,
    fleet_mutation: false,
  }, null, 2)}\n`);
  if (!plan.dry_run_validated) {
    process.exitCode = 1;
  }
}

main().catch(error => {
  console.error(error.stack || error.message || String(error));
  process.exitCode = 1;
});
