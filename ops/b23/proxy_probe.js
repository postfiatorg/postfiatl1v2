// Read-only probe of a wallet-proxy /rpc WebSocket: server_info, status, and one account balance.
// Usage: NODE_PATH=<dir with ws> node proxy_probe.js ws://127.0.0.1:PORT/rpc pfADDRESS
const WebSocket = require('ws');
const [url, address] = process.argv.slice(2);
const calls = [
  ['server_info', {}], ['status', {}], ['account', { address }],
  ['account_lines', { account: address, limit: 60 }],
];
const ws = new WebSocket(url);
const out = {}; let n = 0;
ws.on('open', () => calls.forEach(([method, params], i) =>
  ws.send(JSON.stringify({ version: 'postfiat-local-rpc-v1', id: `probe-${i}`, method, params }))));
ws.on('message', (d) => {
  const r = JSON.parse(d.toString()); const i = Number(String(r.id).split('-')[1]);
  if (!(i >= 0 && i < calls.length)) return; out[calls[i][0]] = r;
  if (Object.keys(out).length === calls.length && !n++) { console.log(JSON.stringify(out, null, 1)); process.exit(0); }
});
ws.on('error', (e) => { console.error('ERR', e.message); process.exit(2); });
setTimeout(() => { console.error('timeout; got', Object.keys(out)); if (!n++) console.log(JSON.stringify(out, null, 1)); process.exit(3); }, 60000);
