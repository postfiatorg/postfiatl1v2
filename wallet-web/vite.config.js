import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { existsSync, readFileSync } from 'fs';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);
const pkg = require('./package.json');
const fastSwapDemoToken = process.env.FASTSWAP_DEMO_API_TOKEN || '';
const fastSwapDemoBackend = process.env.FASTSWAP_DEMO_BACKEND_URL || 'http://127.0.0.1:18830';

const CSP_VALUE = "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'; script-src 'self' 'wasm-unsafe-eval'; object-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ws://127.0.0.1:8080 ws://localhost:8080 http://127.0.0.1:8789 http://localhost:8789;";

// Inject CSP meta tag ONLY in production builds.
// In dev mode Vite uses inline scripts (react-refresh, HMR client) that
// would be blocked by CSP, causing a black screen.
function cspMetaPlugin() {
  return {
    name: 'csp-meta-tag',
    apply: 'build',
    transformIndexHtml(html) {
      return html.replace(
        '<head>',
        `<head>\n    <meta http-equiv="Content-Security-Policy" content="${CSP_VALUE}" />`,
      );
    },
  };
}

// Optional self-signed cert for HTTPS. Enable only when both env vars are set;
// the default local wallet dev path is HTTP on 5173 plus WS on 8080.
const httpsKeyPath = process.env.VITE_HTTPS_KEY;
const httpsCertPath = process.env.VITE_HTTPS_CERT;
const httpsCert = httpsKeyPath && httpsCertPath && existsSync(httpsKeyPath) && existsSync(httpsCertPath)
  ? {
      key: readFileSync(httpsKeyPath),
      cert: readFileSync(httpsCertPath),
    }
  : undefined;

export default defineConfig({
  plugins: [react(), cspMetaPlugin()],
  define: {
    'import.meta.env.VITE_APP_VERSION': JSON.stringify(pkg.version),
  },
  server: {
    port: 5173,
    host: '127.0.0.1',
    strictPort: true,
    ...(httpsCert ? { https: httpsCert } : {}),
    // Proxy WebSocket RPC through the Vite HTTPS server to avoid
    // mixed-content blocking (wss:// → ws:// tunnel to the proxy).
    proxy: {
      '/rpc': {
        target: 'ws://127.0.0.1:8080',
        ws: true,
        changeOrigin: true,
      },
      '/api/navswap': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
      '/api/shielded-nav-swap': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
      '/api/fastswap-demo': {
        target: fastSwapDemoBackend,
        changeOrigin: true,
        headers: { 'x-fastswap-demo-token': fastSwapDemoToken },
      },
    },
    // No CSP header in dev — Vite needs inline scripts for HMR/react-refresh
  },
  build: {
    outDir: 'dist',
    assetsInlineLimit: 0,
  },
  preview: {
    port: 5173,
    host: '127.0.0.1',
    strictPort: true,
    headers: {
      'Content-Security-Policy': CSP_VALUE,
    },
  },
  optimizeDeps: {
    exclude: ['src/wasm/postfiat_wallet_wasm.js'],
  },
});
