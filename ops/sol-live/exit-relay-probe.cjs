// Record only the non-secret correlation ID for a disabled cutover probe.
const { channel } = require('node:diagnostics_channel');
channel('http.server.response.finish').subscribe(({ request, response }) => {
  const path = /^\/api\/l1\/bridge\/solana\/withdrawals\/(0{16}[0-9a-f]{80})$/.exec(request.url);
  if (path && request.method === 'GET' && response.statusCode === 503)
    console.info('sol-exit-disabled-probe ' + path[1]);
});
