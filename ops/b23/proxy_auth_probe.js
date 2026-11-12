// Read-only: call methods through the wallet-proxy WS with the local-demo token read from
// the token FILE (never printed). Usage: node proxy_auth_probe.js URL TOKENS_FILE METHOD:PARAMSJSON...
const fs = require('fs'); const WebSocket = require('ws');
const [url, tokenFile, ...calls] = process.argv.slice(2);
const token = JSON.parse(fs.readFileSync(tokenFile, 'utf8'))['local-demo'];
const ws = new WebSocket(url, process.env.PROBE_ORIGIN ? { origin: process.env.PROBE_ORIGIN } : {}); const out = {}; let done = 0;
ws.on('open', () => calls.forEach((c, i) => { const k = c.indexOf(':'); const method = c.slice(0, k);
  const params = JSON.parse(c.slice(k + 1));
  ws.send(JSON.stringify({ version: 'postfiat-local-rpc-v1', id: `a-${i}`, method, params })); // no token
  ws.send(JSON.stringify({ version: 'postfiat-local-rpc-v1', id: `t-${i}`, method, params, proxy_auth_token: token })); }));
ws.on('message', (d) => { const r = JSON.parse(d.toString()); const [mode, i] = String(r.id).split('-');
  const m = calls[Number(i)].split(':')[0]; out[m] = out[m] || {};
  out[m][mode === 'a' ? 'no_token' : 'with_token'] = { ok: r.ok, error: r.error ? (r.error.code || r.error) : null, result_keys: r.result ? Object.keys(r.result).slice(0, 8) : null };
  if (++done === calls.length * 2) { console.log(JSON.stringify(out, null, 1)); process.exit(0); } });
setTimeout(() => { console.log(JSON.stringify(out, null, 1)); process.exit(3); }, 90000);
