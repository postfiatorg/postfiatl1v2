export const PRODUCT_PRIVATE_SWAP_STEPS = Object.freeze([
  'preflight',
  'public_funding',
  'shield_ingress',
  'private_swap',
  'private_egress',
  'public_settlement',
  'final_verify',
]);

export function productPrivateSwapRunId(walletAddress, now = Date.now(), random = Math.random()) {
  const wallet = String(walletAddress || '').toLowerCase();
  if (!/^pf[0-9a-f]{40}$/.test(wallet)) throw new Error('A valid FastPay wallet is required');
  const suffix = Math.floor(random * 0x100000000).toString(16).padStart(8, '0');
  return `ux-${now.toString(36)}-${suffix}`;
}

export function normalizeProductPrivateSwapResult(result) {
  const steps = result?.steps && typeof result.steps === 'object' ? result.steps : {};
  return {
    ok: result?.ok === true,
    complete: result?.complete === true,
    runId: result?.wallet_ux?.run_id || null,
    runDir: result?.run_dir || null,
    finalHeight: steps?.final_verify?.expected_height ?? null,
    finalStateRoot: steps?.final_verify?.expected_state_root || null,
    steps: PRODUCT_PRIVATE_SWAP_STEPS.map(name => ({
      name,
      state: steps?.[name]?.state || 'not_started',
      artifactHash: steps?.[name]?.artifact_hash || null,
    })),
  };
}

export async function executeProductPrivateSwap({ swapServer, walletAddress, runId }) {
  if (!swapServer || typeof swapServer.runPrivateSwapWorkflow !== 'function') {
    throw new Error('Certified private-swap backend is not configured');
  }
  const resolvedRunId = runId || productPrivateSwapRunId(walletAddress);
  const result = await swapServer.runPrivateSwapWorkflow({
    action: 'execute',
    fresh_wallet: true,
    initiating_wallet_address: walletAddress,
    no_money: true,
    run_id: resolvedRunId,
  });
  const normalized = normalizeProductPrivateSwapResult(result);
  if (!normalized.ok || !normalized.complete) {
    throw new Error(result?.error || 'Certified private-swap workflow did not complete');
  }
  return normalized;
}
