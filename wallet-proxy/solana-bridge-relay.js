'use strict';

const fs = require('fs');
const ROUTE = process.env.PFT_SOL_RELAY_ROUTE_ID || 'solana-mainnet-usdc-v1';

// Called only behind the proxy's normal wallet mutation and private-list authorization.
function solanaRelay() {
    const base = process.env.PFT_SOL_RELAY_URL;
    if (!base) return null;
    if (!/^solana-(mainnet|devnet|local)-usdc-v1$/.test(ROUTE)) {
        throw new Error('Unknown Solana relay route ID');
    }
    return async function relay(path, body) {
        const token = fs.readFileSync(process.env.PFT_SOL_RELAY_TOKEN_FILE, 'utf8').trim();
        const response = await fetch(new URL(path, base), {
            method: body ? 'POST' : 'GET',
            headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + token },
            body: body ? JSON.stringify(body) : undefined,
            redirect: 'error',
            signal: AbortSignal.timeout(5000),
        });
        return { status: response.status, body: await response.json() };
    };
}
module.exports = { ROUTE, solanaRelay };
