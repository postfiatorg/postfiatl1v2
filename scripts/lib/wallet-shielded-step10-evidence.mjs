export const STEP10_FORBIDDEN_PRIVATE_KEYS = [
  'backup',
  'backup_json',
  'decrypted_backup',
  'diversifier',
  'full_viewing_key',
  'full_viewing_key_hex',
  'g_d',
  'input_note',
  'input_notes',
  'mnemonic',
  'nk',
  'note',
  'note_file',
  'note_files',
  'note_opening',
  'note_openings',
  'opening',
  'passphrase',
  'private_key',
  'private_seed',
  'pk_d',
  'psi',
  'rcm',
  'rho',
  'rseed',
  'seed',
  'seed_hex',
  'secret_key',
  'spend_auth_signing_key',
  'spend_key',
  'spending_key',
  'rivk',
  'wallet_note',
];

const FORBIDDEN_KEY_SET = new Set(STEP10_FORBIDDEN_PRIVATE_KEYS);

export function normalizeEvidenceKey(key) {
  return String(key || '')
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[^A-Za-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
    .toLowerCase();
}

function tryParseJsonString(value) {
  const text = String(value || '').trim();
  if (!text || !/^[{[]/.test(text)) return null;
  try {
    return JSON.parse(text);
  } catch (_) {
    return null;
  }
}

function sensitiveLabelsFromOptions(sensitiveLabels = []) {
  return sensitiveLabels
    .map(item => ({
      label: String(item?.label || '').trim(),
      value: String(item?.value || ''),
    }))
    .filter(item => item.label && item.value.length >= 8);
}

export function scanPrivateMaterial(value, {
  path = '$',
  hits = [],
  sensitiveLabels = [],
  parseJsonStrings = true,
  depth = 0,
  maxDepth = 32,
} = {}) {
  if (depth > maxDepth) {
    hits.push({ path, type: 'max_depth_exceeded' });
    return hits;
  }
  if (typeof value === 'string') {
    for (const item of sensitiveLabelsFromOptions(sensitiveLabels)) {
      if (value.includes(item.value)) {
        hits.push({ path, type: 'sensitive_value', label: item.label });
      }
    }
    if (parseJsonStrings) {
      const parsed = tryParseJsonString(value);
      if (parsed !== null) {
        scanPrivateMaterial(parsed, {
          path: `${path}<json>`,
          hits,
          sensitiveLabels,
          parseJsonStrings,
          depth: depth + 1,
          maxDepth,
        });
      }
    }
    return hits;
  }
  if (!value || typeof value !== 'object') return hits;
  if (Array.isArray(value)) {
    value.forEach((child, idx) => {
      scanPrivateMaterial(child, {
        path: `${path}[${idx}]`,
        hits,
        sensitiveLabels,
        parseJsonStrings,
        depth: depth + 1,
        maxDepth,
      });
    });
    return hits;
  }
  for (const [key, child] of Object.entries(value)) {
    const childPath = `${path}.${key}`;
    const normalized = normalizeEvidenceKey(key);
    if (FORBIDDEN_KEY_SET.has(normalized)) {
      hits.push({ path: childPath, type: 'forbidden_private_key', key });
    }
    scanPrivateMaterial(child, {
      path: childPath,
      hits,
      sensitiveLabels,
      parseJsonStrings,
      depth: depth + 1,
      maxDepth,
    });
  }
  return hits;
}

export function redactPrivateMaterial(value, {
  sensitiveLabels = [],
  depth = 0,
  maxDepth = 32,
} = {}) {
  if (depth > maxDepth) return '[REDACTED_MAX_DEPTH]';
  if (typeof value === 'string') {
    let redacted = value;
    for (const item of sensitiveLabelsFromOptions(sensitiveLabels)) {
      if (redacted.includes(item.value)) {
        redacted = redacted.split(item.value).join(`[REDACTED_${item.label}]`);
      }
    }
    return redacted;
  }
  if (!value || typeof value !== 'object') return value;
  if (Array.isArray(value)) {
    return value.map(child => redactPrivateMaterial(child, {
      sensitiveLabels,
      depth: depth + 1,
      maxDepth,
    }));
  }
  const out = {};
  for (const [key, child] of Object.entries(value)) {
    const normalized = normalizeEvidenceKey(key);
    out[key] = FORBIDDEN_KEY_SET.has(normalized)
      ? '[REDACTED_FORBIDDEN_PRIVATE_KEY]'
      : redactPrivateMaterial(child, {
          sensitiveLabels,
          depth: depth + 1,
          maxDepth,
        });
  }
  return out;
}

function requestShape(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return { type: value === null ? 'null' : typeof value };
  }
  const keys = Object.keys(value).sort();
  return {
    type: 'object',
    keys,
    json_string_fields: keys.filter(key => typeof value[key] === 'string' && /^[{[]/.test(value[key].trim())),
  };
}

export function proxyBoundRequestEntries(entries = []) {
  return entries.filter(entry => String(entry?.url || '').includes('/api/shielded-nav-swap/'));
}

export function buildNoPrivateMaterialRequestLog({
  entries = [],
  runIndex = null,
  schema = 'postfiat-wallet-private-swap-step10-no-private-material-request-log-v1',
  sensitiveLabels = [],
  capturedAt = new Date().toISOString(),
} = {}) {
  const proxyEntries = proxyBoundRequestEntries(entries);
  const rows = proxyEntries.map((entry, idx) => {
    const request = entry?.request ?? null;
    const hits = scanPrivateMaterial(request, { sensitiveLabels });
    return {
      index: idx,
      at: entry?.at || null,
      url: entry?.url || null,
      status: entry?.status ?? null,
      ok: entry?.ok ?? null,
      request_shape: requestShape(request),
      request: redactPrivateMaterial(request, { sensitiveLabels }),
      hits,
    };
  });
  const hits = rows.flatMap(row => row.hits.map(hit => ({
    ...hit,
    request_index: row.index,
    url: row.url,
  })));
  return {
    schema,
    run_index: runIndex,
    captured_at: capturedAt,
    ok: hits.length === 0,
    scanned_request_count: rows.length,
    proxy_bound_only: true,
    forbidden_keys: STEP10_FORBIDDEN_PRIVATE_KEYS,
    sensitive_value_labels_checked: sensitiveLabelsFromOptions(sensitiveLabels).map(item => item.label),
    hits,
    entries: rows,
  };
}

export function buildRunPassFailSummary(run, requestLog = null, reloadProof = null) {
  const assertions = run?.assertions || {};
  return {
    run_index: run?.run_index ?? null,
    direction: run?.direction || null,
    quote_ok: run?.quote?.ok === true,
    local_action_ok: run?.local_action?.ok === true,
    relay_ok: run?.relay?.ok === true,
    finalize_ok: run?.finalize?.ok === true,
    wire_privacy_ok: run?.wire_privacy?.ok === true,
    no_private_material_ok: requestLog?.ok === true,
    reload_rescan_ok: reloadProof ? reloadProof.ok === true : null,
    wallet_input_spent: assertions.wallet_input_spent === true,
    expected_wallet_input_spent: assertions.expected_wallet_input_spent === true,
    wallet_output_spendable: assertions.wallet_output_spendable === true,
    pool_input_spent: assertions.pool_input_spent === true,
    zero_repair: assertions.zero_repair === true,
  };
}

export function passFailFieldNames(summary) {
  return Object.keys(summary).sort();
}

export function summariesHaveIdenticalFields(summaries = []) {
  if (summaries.length <= 1) return true;
  const first = JSON.stringify(passFailFieldNames(summaries[0]));
  return summaries.every(summary => JSON.stringify(passFailFieldNames(summary)) === first);
}

export function buildStep10EvidenceSummary({
  evidenceDir,
  files,
  operatorDemoCommand = null,
  generatedAt = new Date().toISOString(),
} = {}) {
  if (!files || typeof files.readJson !== 'function') {
    throw new Error('files.readJson is required');
  }
  const runs = [1, 2]
    .map(index => files.readJson(`${evidenceDir}/run-${index}-evidence.json`, null))
    .filter(Boolean);
  const requestLogs = [1, 2]
    .map(index => files.readJson(`${evidenceDir}/run-${index}-no-private-material-request-log.json`, null));
  const reloadProof = files.readJson(`${evidenceDir}/reload-rescan-proof.json`, null);
  const stakehubSlot = files.readJson(`${evidenceDir}/stakehub-operator-demo-command.json`, null);
  const summaries = runs.map(run => buildRunPassFailSummary(
    run,
    requestLogs[(run.run_index || 1) - 1] || null,
    reloadProof,
  ));
  const requestLogsOk = requestLogs.filter(Boolean).length === 2
    && requestLogs.every(log => log?.ok === true);
  const canonicalDirections = runs.length === 2
    && runs[0]?.direction === 'a651->a652'
    && runs[1]?.direction === 'a652->a651';
  const stakehubPrepared = Boolean(stakehubSlot?.command || operatorDemoCommand);
  const runsOk = summaries.length === 2 && summaries.every(summary => (
    summary.quote_ok
    && summary.local_action_ok
    && summary.relay_ok
    && summary.finalize_ok
    && summary.wire_privacy_ok
    && summary.no_private_material_ok
    && summary.wallet_input_spent
    && summary.expected_wallet_input_spent
    && summary.wallet_output_spendable
    && summary.pool_input_spent
    && summary.zero_repair
  ));
  return {
    schema: 'postfiat-wallet-private-swap-step10-evidence-summary-v1',
    generated_at: generatedAt,
    evidence_dir: evidenceDir,
    run_count: runs.length,
    directions: runs.map(run => run.direction || null),
    canonical_pair: canonicalDirections,
    identical_pass_fail_summary_fields: summariesHaveIdenticalFields(summaries),
    pass_fail_field_names: summaries[0] ? passFailFieldNames(summaries[0]) : [],
    pass_fail_summaries: summaries,
    request_logs_ok: requestLogsOk,
    reload_rescan_ok: reloadProof?.ok === true,
    stakehub_operator_demo: {
      prepared: stakehubPrepared,
      command: stakehubSlot?.command || operatorDemoCommand || null,
      evidence_slot: stakehubSlot?.evidence_slot || null,
      live_window_required: true,
    },
    operator_approval_required_for_can_run: true,
    package_ok: runsOk
      && canonicalDirections
      && requestLogsOk
      && reloadProof?.ok === true
      && stakehubPrepared
      && summariesHaveIdenticalFields(summaries),
  };
}
