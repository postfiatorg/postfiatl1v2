#!/usr/bin/env node
// Simple HTTP server that serves the wallet extension ZIP for download.
// Also serves a simple landing page with instructions.

const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = parseInt(process.env.DOWNLOAD_PORT || '8091', 10);
const ZIP_PATH = process.env.ZIP_PATH || '/tmp/postfiat-wallet-extension.zip';

const landingHtml = `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>PostFiat Wallet Extension</title>
  <style>
    body { font-family: system-ui, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px; background: #0a0a0a; color: #e0e0e0; }
    h1 { color: #8b5cf6; }
    a { color: #8b5cf6; }
    .download-btn { display: inline-block; padding: 12px 24px; background: #8b5cf6; color: #fff; text-decoration: none; border-radius: 8px; font-size: 16px; margin: 16px 0; }
    .step { margin: 8px 0; padding: 8px; background: #1a1a1a; border-radius: 4px; }
    code { background: #1a1a1a; padding: 2px 6px; border-radius: 3px; color: #fbbf24; }
  </style>
</head>
<body>
  <h1>PostFiat Wallet Extension</h1>
  <p>Post-quantum self-custody wallet for PostFiat L1 — ML-DSA-65 signing in your browser.</p>
  
  <a href="/postfiat-wallet-extension.zip" class="download-btn">Download Extension ZIP</a>
  
  <h2>Install Instructions</h2>
  <div class="step"><strong>1.</strong> Download and unzip the file above</div>
  <div class="step"><strong>2.</strong> Open Chrome and go to <code>chrome://extensions</code></div>
  <div class="step"><strong>3.</strong> Enable "Developer mode" (top right toggle)</div>
  <div class="step"><strong>4.</strong> Click "Load unpacked" and select the unzipped folder</div>
  <div class="step"><strong>5.</strong> The PostFiat Wallet icon should appear in your toolbar</div>
  
  <h2>RPC Connection</h2>
  <p>The wallet needs a WebSocket proxy to talk to the PostFiat L1 chain. Configure the RPC endpoint in the wallet's Settings tab.</p>
  <p>Default: <code>ws://localhost:8080</code></p>
  <p>If running the proxy on this server: use <code>ws://SERVER_IP:8080</code> (check below)</p>
</body>
</html>`;

const server = http.createServer((req, res) => {
  if (req.url === '/' || req.url === '/index.html') {
    res.writeHead(200, { 'Content-Type': 'text/html' });
    res.end(landingHtml);
  } else if (req.url === '/postfiat-wallet-extension.zip') {
    if (!fs.existsSync(ZIP_PATH)) {
      res.writeHead(404, { 'Content-Type': 'text/plain' });
      res.end('ZIP not found. Run the packaging step first.');
      return;
    }
    const stat = fs.statSync(ZIP_PATH);
    res.writeHead(200, {
      'Content-Type': 'application/zip',
      'Content-Disposition': 'attachment; filename="postfiat-wallet-extension.zip"',
      'Content-Length': stat.size,
    });
    fs.createReadStream(ZIP_PATH).pipe(res);
  } else {
    res.writeHead(404, { 'Content-Type': 'text/plain' });
    res.end('Not found. Visit / for the download page.');
  }
});

server.listen(PORT, '127.0.0.1', () => {
  console.log(`Wallet extension download server on http://127.0.0.1:${PORT}/`);
  console.log(`ZIP: ${ZIP_PATH} (${fs.existsSync(ZIP_PATH) ? fs.statSync(ZIP_PATH).size + ' bytes' : 'NOT FOUND'})`);
});
