'use strict';

// W-1 S3.3: the proxy's legacy shielded certification paths behind one
// operator switch, `WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION` (default on).
// When off, the routes below answer 410 and the proxy reads no validator key
// file, no validator data directory and no local Asset-Orchard service.
//   1. transport-peer-certified-batch-round with --key-file
//      (navswap-shielded.js buildShieldedCertifiedRoundArgs)
//   2. the local Asset-Orchard /asset-orchard/swap-batch call
//      (navswap-shielded.js createShieldedSwapBatchViaLocalService)
//   3. the HTTP-side certifying executions behind the routes below
//      (navswap-shielded.js executeShieldedNavswapIngress/Swap/Egress)

const SWITCH = 'WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION';
const RETIRED_CODE = 'legacy_shielded_certification_retired';
const REPLACEMENT = '/api/private/v1/';
const LEGACY_ROUTES = new Set([
    '/api/shielded-nav-swap/ingress',
    '/api/shielded-nav-swap/swap',
    '/api/shielded-nav-swap/egress',
]);

function legacyShieldedCertificationEnabled(env = process.env) {
    const raw = String(env[SWITCH] ?? '').trim().toLowerCase();
    if (raw === '' || ['on', 'true', '1'].includes(raw)) return true;
    if (['off', 'false', '0'].includes(raw)) return false;
    throw new Error(`${SWITCH} must be on or off`);
}

// Answers the legacy routes with 410 when the switch is off.
function handleRetiredLegacyRoute(req, res, url, env = process.env) {
    if (!LEGACY_ROUTES.has(url.pathname) || legacyShieldedCertificationEnabled(env)) return false;
    res.writeHead(410, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
    res.end(JSON.stringify({
        ok: false,
        code: RETIRED_CODE,
        message: `The proxy's legacy shielded certification is retired; submit user-proven actions to ${REPLACEMENT}.`,
        replacement: REPLACEMENT,
    }));
    return true;
}

// Startup: the mode line, and a warning (never a refusal) when the switch is on,
// a legacy path is enabled and its validator key file or data directory is
// missing; those routes then answer 409 as before. Only an invalid switch
// value stops startup.
function legacyShieldedCertificationStartup(ingressConfig, env = process.env) {
    if (!legacyShieldedCertificationEnabled(env)) {
        return {
            ok: true,
            line: `Legacy shielded certification: off (${[...LEGACY_ROUTES].join(', ')} answer 410; `
                + 'no validator key file, data directory or local Asset-Orchard service is read).',
        };
    }
    const fs = require('fs');
    const legacyEnabled = ['NAVSWAP_ENABLE_SHIELDED_INGRESS', 'NAVSWAP_ENABLE_SHIELDED_SWAPS', 'NAVSWAP_ENABLE_SHIELDED_EGRESS']
        .some((name) => ['true', '1'].includes(String(env[name] || '').trim().toLowerCase()));
    const missing = [];
    if (legacyEnabled) {
        if (!ingressConfig?.key_file || !fs.existsSync(ingressConfig.key_file)) missing.push('the validator key file');
        if (!ingressConfig?.data_dir || !fs.existsSync(ingressConfig.data_dir)) missing.push('the validator data directory');
    }
    if (missing.length) {
        return {
            ok: true,
            warning: true,
            line: `Warning: legacy shielded certification is on and enabled, but ${missing.join(' and ')} is missing; `
                + `its routes answer 409 until it is configured or ${SWITCH}=off.`,
        };
    }
    return {
        ok: true,
        line: `Legacy shielded certification: on (${legacyEnabled ? 'enabled' : 'routes present, not enabled'}).`,
    };
}

module.exports = {
    LEGACY_ROUTES,
    REPLACEMENT,
    RETIRED_CODE,
    SWITCH,
    handleRetiredLegacyRoute,
    legacyShieldedCertificationEnabled,
    legacyShieldedCertificationStartup,
};
