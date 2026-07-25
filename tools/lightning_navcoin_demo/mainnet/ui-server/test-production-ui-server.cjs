"use strict";

const assert = require("assert/strict");
const crypto = require("crypto");
const fs = require("fs");
const http = require("http");
const os = require("os");
const path = require("path");
const { execFileSync } = require("child_process");
const { afterEach, test } = require("node:test");
const { WebSocket, WebSocketServer } = require("ws");

const {
  CSP,
  UiServerError,
  assertProductionIndex,
  createProductionServer,
  createRelease,
  loadVerifiedRelease,
  readCoordinatorToken,
  readPftlProxyToken,
  sha256,
  verifyCleanSourceRelease,
  walkRegularFiles,
} = require("./production-ui-server.cjs");

const cleanups = [];

afterEach(async () => {
  while (cleanups.length) {
    await cleanups.pop()();
  }
});

function temporaryDirectory() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "pftl-ln-ui-test-"));
  fs.chmodSync(directory, 0o700);
  cleanups.push(async () => fs.rmSync(directory, { recursive: true, force: true }));
  return directory;
}

function listen(server, host = "127.0.0.1", port = 0) {
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, () => {
      server.off("error", reject);
      resolve(server.address());
    });
  });
}

function closeServer(server) {
  return new Promise(resolve => server.close(() => resolve()));
}

async function freePort() {
  const server = http.createServer();
  const address = await listen(server);
  await closeServer(server);
  return address.port;
}

function httpCall({ port, method = "GET", pathname = "/", headers = {}, body = null }) {
  return new Promise((resolve, reject) => {
    const requestHeaders = { ...headers };
    if (body !== null && requestHeaders["Content-Length"] === undefined) {
      requestHeaders["Content-Length"] = Buffer.byteLength(body);
    }
    const request = http.request({
      hostname: "127.0.0.1",
      port,
      path: pathname,
      method,
      headers: requestHeaders,
    }, response => {
      const chunks = [];
      response.on("data", chunk => chunks.push(chunk));
      response.on("end", () => resolve({
        status: response.statusCode,
        headers: response.headers,
        body: Buffer.concat(chunks),
      }));
    });
    request.on("error", reject);
    if (body !== null) request.end(body);
    else request.end();
  });
}

function fakeRelease() {
  return Object.freeze({
    manifestSha256: "11".repeat(32),
    distTreeSha256: "22".repeat(32),
    source: Object.freeze({
      git_commit: "33".repeat(20),
      git_tree: "44".repeat(20),
    }),
    files: new Map([
      ["index.html", Buffer.from("<!doctype html><div id=\"root\"></div>")],
      ["assets/main-abcdefgh.js", Buffer.from("console.log('release');")],
    ]),
  });
}

function initializeFakeReleaseRepo() {
  const root = temporaryDirectory();
  fs.mkdirSync(path.join(root, "tools/lightning_navcoin_demo"), { recursive: true });
  fs.mkdirSync(path.join(root, "wallet-web"), { recursive: true });
  fs.mkdirSync(path.join(root, "scripts"), { recursive: true });
  fs.mkdirSync(
    path.join(root, "tools/lightning_navcoin_demo/mainnet/ui-server"),
    { recursive: true },
  );
  fs.writeFileSync(
    path.join(
      root,
      "tools/lightning_navcoin_demo/mainnet/ui-server/production-ui-server.cjs",
    ),
    "module.exports = {};\n",
  );
  fs.writeFileSync(path.join(root, "wallet-web/package-lock.json"), "{}\n");
  fs.writeFileSync(path.join(root, "wallet-web/source.js"), "export default 1;\n");
  fs.writeFileSync(
    path.join(root, "scripts/lightning-navcoin-mainnet-ui"),
    "#!/bin/sh\nexit 0\n",
  );
  execFileSync("git", ["init", "-q"], { cwd: root });
  execFileSync("git", ["config", "user.email", "test@example.invalid"], { cwd: root });
  execFileSync("git", ["config", "user.name", "Test"], { cwd: root });
  execFileSync("git", ["add", "."], { cwd: root });
  execFileSync("git", ["commit", "-qm", "release"], { cwd: root });
  const commit = execFileSync(
    "git",
    ["rev-parse", "HEAD^{commit}"],
    { cwd: root, encoding: "utf8" },
  ).trim();
  const tree = execFileSync(
    "git",
    ["rev-parse", "HEAD^{tree}"],
    { cwd: root, encoding: "utf8" },
  ).trim();
  const sourceRelease = path.join(temporaryDirectory(), "source-release.json");
  fs.writeFileSync(sourceRelease, JSON.stringify({
    schema: "postfiat.lightning_coordinator_source_release.v1",
    git_commit: commit,
    git_tree: tree,
    clean: true,
    targets: [
      "tools/lightning_navcoin_demo",
      "wallet-web",
      "scripts/lightning-navcoin-mainnet-coordinator",
    ],
  }));
  fs.chmodSync(sourceRelease, 0o600);
  return { root, sourceRelease, commit, tree };
}

test("secret files require owner-only exact coordinator and bounded PFTL tokens", () => {
  const root = temporaryDirectory();
  const coordinator = path.join(root, "coordinator.token");
  const pftl = path.join(root, "pftl.token");
  fs.writeFileSync(coordinator, crypto.randomBytes(32), { mode: 0o600 });
  fs.writeFileSync(pftl, "p".repeat(48), { mode: 0o600 });
  assert.equal(readCoordinatorToken(coordinator).length, 32);
  assert.equal(readPftlProxyToken(pftl), "p".repeat(48));

  fs.chmodSync(coordinator, 0o640);
  assert.throws(
    () => readCoordinatorToken(coordinator),
    /mode 0600/,
  );
  fs.chmodSync(coordinator, 0o600);
  fs.writeFileSync(pftl, `${"p".repeat(48)}\n`);
  assert.throws(() => readPftlProxyToken(pftl), /non-whitespace/);
});

test("source release binds clean HEAD and all production UI inputs", () => {
  const fixture = initializeFakeReleaseRepo();
  assert.deepEqual(
    verifyCleanSourceRelease(fixture.root, fixture.sourceRelease),
    { git_commit: fixture.commit, git_tree: fixture.tree },
  );
  fs.appendFileSync(path.join(fixture.root, "wallet-web/source.js"), "// dirty\n");
  assert.throws(
    () => verifyCleanSourceRelease(fixture.root, fixture.sourceRelease),
    /not a clean release/,
  );
});

test("release manifest pins every production byte and rejects tampering", () => {
  const fixture = initializeFakeReleaseRepo();
  const dist = path.join(temporaryDirectory(), "dist");
  fs.mkdirSync(path.join(dist, "assets"), { recursive: true });
  fs.writeFileSync(
    path.join(dist, "index.html"),
    "<!doctype html><script type=\"module\" src=\"/assets/main-abcdefgh.js\"></script>",
  );
  fs.writeFileSync(
    path.join(dist, "assets/main-abcdefgh.js"),
    "console.log('production');",
  );
  const output = path.join(temporaryDirectory(), "artifacts");
  fs.mkdirSync(output, { mode: 0o700 });
  const created = createRelease({
    repoRoot: fixture.root,
    sourceReleasePath: fixture.sourceRelease,
    distPath: dist,
    outputRoot: output,
  });
  const loaded = loadVerifiedRelease({
    repoRoot: fixture.root,
    sourceReleasePath: fixture.sourceRelease,
    manifestPath: created.manifest_path,
    expectedManifestSha256: created.manifest_sha256,
  });
  assert.equal(loaded.files.get("index.html").includes("doctype"), true);
  assert.equal(loaded.manifestSha256, created.manifest_sha256);

  const canonicalManifest = fs.readFileSync(created.manifest_path);
  const noncanonicalManifest = Buffer.from(
    `${JSON.stringify(JSON.parse(canonicalManifest), null, 2)}\n`,
  );
  fs.writeFileSync(created.manifest_path, noncanonicalManifest);
  assert.throws(
    () => loadVerifiedRelease({
      repoRoot: fixture.root,
      sourceReleasePath: fixture.sourceRelease,
      manifestPath: created.manifest_path,
      expectedManifestSha256: sha256(noncanonicalManifest),
    }),
    /not canonical JSON/,
  );
  fs.writeFileSync(created.manifest_path, canonicalManifest);

  const releasedAsset = path.join(
    path.dirname(created.manifest_path),
    "dist/assets/main-abcdefgh.js",
  );
  fs.chmodSync(releasedAsset, 0o600);
  fs.appendFileSync(releasedAsset, "\n// tampered");
  assert.throws(
    () => loadVerifiedRelease({
      repoRoot: fixture.root,
      sourceReleasePath: fixture.sourceRelease,
      manifestPath: created.manifest_path,
      expectedManifestSha256: created.manifest_sha256,
    }),
    /changed/,
  );
});

test("unchanged UI bytes can be released under a new clean source pin", () => {
  const fixture = initializeFakeReleaseRepo();
  const dist = path.join(temporaryDirectory(), "dist");
  fs.mkdirSync(path.join(dist, "assets"), { recursive: true });
  fs.writeFileSync(
    path.join(dist, "index.html"),
    "<!doctype html><script type=\"module\" src=\"/assets/main-abcdefgh.js\"></script>",
  );
  fs.writeFileSync(
    path.join(dist, "assets/main-abcdefgh.js"),
    "console.log('production');",
  );
  const output = path.join(temporaryDirectory(), "artifacts");
  fs.mkdirSync(output, { mode: 0o700 });
  const first = createRelease({
    repoRoot: fixture.root,
    sourceReleasePath: fixture.sourceRelease,
    distPath: dist,
    outputRoot: output,
  });

  fs.writeFileSync(
    path.join(fixture.root, "tools/lightning_navcoin_demo/audit-note.txt"),
    "reviewed source-only change\n",
  );
  execFileSync("git", ["add", "."], { cwd: fixture.root });
  execFileSync("git", ["commit", "-qm", "source-only change"], {
    cwd: fixture.root,
  });
  const commit = execFileSync(
    "git",
    ["rev-parse", "HEAD^{commit}"],
    { cwd: fixture.root, encoding: "utf8" },
  ).trim();
  const tree = execFileSync(
    "git",
    ["rev-parse", "HEAD^{tree}"],
    { cwd: fixture.root, encoding: "utf8" },
  ).trim();
  const source = JSON.parse(fs.readFileSync(fixture.sourceRelease, "utf8"));
  source.git_commit = commit;
  source.git_tree = tree;
  fs.writeFileSync(fixture.sourceRelease, JSON.stringify(source));
  fs.chmodSync(fixture.sourceRelease, 0o600);

  const second = createRelease({
    repoRoot: fixture.root,
    sourceReleasePath: fixture.sourceRelease,
    distPath: dist,
    outputRoot: output,
  });
  assert.equal(first.dist_tree_sha256, second.dist_tree_sha256);
  assert.notEqual(first.manifest_sha256, second.manifest_sha256);
  assert.notEqual(first.manifest_path, second.manifest_path);
  assert.equal(fs.existsSync(first.manifest_path), true);
  assert.equal(fs.existsSync(second.manifest_path), true);
});

test("production inventory rejects Vite dev and source-map artifacts", () => {
  const root = temporaryDirectory();
  fs.writeFileSync(path.join(root, "index.html"), "<script src=\"/@vite/client\"></script>");
  const files = walkRegularFiles(root, { rejectDevArtifacts: true });
  assert.throws(
    () => assertProductionIndex(files),
    /dev marker/,
  );
  fs.rmSync(path.join(root, "index.html"));
  fs.writeFileSync(path.join(root, "app.js.map"), "{}");
  assert.throws(
    () => walkRegularFiles(root, { rejectDevArtifacts: true }),
    /dev artifact/,
  );
});

test("loopback UI serves immutable bytes with framing and CSP headers", async () => {
  const coordinator = http.createServer((_request, response) => {
    response.setHeader("Content-Type", "application/json");
    response.end("{\"ok\":true}");
  });
  const coordinatorAddress = await listen(coordinator);
  cleanups.push(async () => closeServer(coordinator));
  const pftlPort = await freePort();
  const pftlServer = new WebSocketServer({ host: "127.0.0.1", port: pftlPort });
  cleanups.push(async () => new Promise(resolve => pftlServer.close(resolve)));
  const uiPort = await freePort();
  const production = createProductionServer({
    host: "127.0.0.1",
    port: uiPort,
    release: fakeRelease(),
    coordinatorToken: Buffer.alloc(32, 1),
    pftlProxyToken: "p".repeat(48),
    coordinatorUrl: `http://127.0.0.1:${coordinatorAddress.port}`,
    pftlProxyUrl: `ws://127.0.0.1:${pftlPort}`,
  });
  await listen(production.server, "127.0.0.1", uiPort);
  cleanups.push(async () => closeServer(production.server));

  const result = await httpCall({ port: uiPort });
  assert.equal(result.status, 200);
  assert.equal(result.body.toString(), "<!doctype html><div id=\"root\"></div>");
  assert.equal(result.headers["x-frame-options"], "DENY");
  assert.equal(result.headers["content-security-policy"], CSP);
  assert.match(result.headers["content-security-policy"], /frame-ancestors 'none'/);
  assert.equal(result.headers["cache-control"], "no-store");

  const asset = await httpCall({
    port: uiPort,
    pathname: "/assets/main-abcdefgh.js",
  });
  assert.equal(asset.headers["cache-control"], "public, max-age=31536000, immutable");
  const rebinding = await httpCall({
    port: uiPort,
    headers: { Host: `evil.invalid:${uiPort}` },
  });
  assert.equal(rebinding.status, 421);
});

test("coordinator auth is injected server-side and browser auth is discarded", async () => {
  const coordinatorToken = Buffer.alloc(32, 7);
  let observed = null;
  const coordinator = http.createServer((request, response) => {
    const chunks = [];
    request.on("data", chunk => chunks.push(chunk));
    request.on("end", () => {
      observed = {
        authorization: request.headers.authorization,
        origin: request.headers.origin,
        cookie: request.headers.cookie,
        body: Buffer.concat(chunks).toString(),
      };
      response.setHeader("Content-Type", "application/json; charset=ascii");
      response.end("{\"ok\":true,\"result\":{\"can_execute\":false}}");
    });
  });
  const coordinatorAddress = await listen(coordinator);
  cleanups.push(async () => closeServer(coordinator));
  const pftlPort = await freePort();
  const pftlServer = new WebSocketServer({ host: "127.0.0.1", port: pftlPort });
  cleanups.push(async () => new Promise(resolve => pftlServer.close(resolve)));
  const uiPort = await freePort();
  const uiOrigin = `http://127.0.0.1:${uiPort}`;
  const production = createProductionServer({
    host: "127.0.0.1",
    port: uiPort,
    release: fakeRelease(),
    coordinatorToken,
    pftlProxyToken: "p".repeat(48),
    coordinatorUrl: `http://127.0.0.1:${coordinatorAddress.port}`,
    pftlProxyUrl: `ws://127.0.0.1:${pftlPort}`,
  });
  await listen(production.server, "127.0.0.1", uiPort);
  cleanups.push(async () => closeServer(production.server));

  const body = "{\"direction\":\"lightning_to_pftl\"}";
  const result = await httpCall({
    port: uiPort,
    method: "POST",
    pathname: "/api/lightning-navcoin/v1/quotes",
    headers: {
      Origin: uiOrigin,
      "Content-Type": "application/json",
      Authorization: "Bearer browser-must-not-control-this",
      Cookie: "also=discarded",
      "X-PostFiat-CSRF": "44".repeat(32),
      "X-Requested-With": "postfiat-wallet",
    },
    body,
  });
  assert.equal(result.status, 200);
  assert.equal(observed.authorization, `Bearer ${coordinatorToken.toString("hex")}`);
  assert.equal(observed.origin, uiOrigin);
  assert.equal(observed.cookie, undefined);
  assert.equal(observed.body, body);
  assert.equal(result.body.includes(coordinatorToken), false);

  const rejected = await httpCall({
    port: uiPort,
    method: "POST",
    pathname: "/api/lightning-navcoin/v1/quotes",
    headers: {
      Origin: "http://evil.invalid",
      "Content-Type": "application/json",
    },
    body,
  });
  assert.equal(rejected.status, 403);
});

test("PFTL websocket proxy injects its token and narrows the RPC surface", async () => {
  const coordinator = http.createServer((_request, response) => {
    response.setHeader("Content-Type", "application/json");
    response.end("{\"ok\":true}");
  });
  const coordinatorAddress = await listen(coordinator);
  cleanups.push(async () => closeServer(coordinator));
  const pftlToken = "p".repeat(48);
  const pftlPort = await freePort();
  const observed = [];
  const origins = [];
  const pftlServer = new WebSocketServer({ host: "127.0.0.1", port: pftlPort });
  pftlServer.on("connection", (socket, request) => {
    origins.push(request.headers.origin);
    socket.on("message", data => {
      const value = JSON.parse(data.toString());
      observed.push(value);
      socket.send(JSON.stringify({
        version: "postfiat-local-rpc-v1",
        id: value.id,
        ok: true,
        result: { chain_id: "test" },
        error: null,
        events: [],
      }));
    });
  });
  cleanups.push(async () => new Promise(resolve => pftlServer.close(resolve)));
  const uiPort = await freePort();
  const uiOrigin = `http://127.0.0.1:${uiPort}`;
  const production = createProductionServer({
    host: "127.0.0.1",
    port: uiPort,
    release: fakeRelease(),
    coordinatorToken: Buffer.alloc(32, 1),
    pftlProxyToken: pftlToken,
    coordinatorUrl: `http://127.0.0.1:${coordinatorAddress.port}`,
    pftlProxyUrl: `ws://127.0.0.1:${pftlPort}`,
  });
  await listen(production.server, "127.0.0.1", uiPort);
  cleanups.push(async () => closeServer(production.server));

  const client = new WebSocket(`ws://127.0.0.1:${uiPort}/rpc`, { origin: uiOrigin });
  await new Promise((resolve, reject) => {
    client.once("open", resolve);
    client.once("error", reject);
  });
  cleanups.push(async () => new Promise(resolve => {
    if (client.readyState === WebSocket.CLOSED) resolve();
    else {
      client.once("close", resolve);
      client.close();
    }
  }));
  client.send(JSON.stringify({
    version: "postfiat-local-rpc-v1",
    id: "one",
    method: "status",
    params: {},
    proxy_auth_token: "attacker-controlled",
  }));
  const accepted = JSON.parse(await new Promise(resolve => {
    client.once("message", data => resolve(data.toString()));
  }));
  assert.equal(accepted.ok, true);
  assert.equal(observed.length, 1);
  assert.equal(observed[0].proxy_auth_token, pftlToken);
  assert.equal(origins[0], uiOrigin);

  client.send(JSON.stringify({
    version: "postfiat-local-rpc-v1",
    id: "two",
    method: "mempool_submit_signed_transfer_finality",
    params: {},
  }));
  const rejected = JSON.parse(await new Promise(resolve => {
    client.once("message", data => resolve(data.toString()));
  }));
  assert.equal(rejected.ok, false);
  assert.equal(rejected.error.code, "ui_rpc_rejected");
  assert.equal(observed.length, 1);
});

test("non-loopback binds and upstreams fail closed", () => {
  const base = {
    host: "127.0.0.1",
    port: 18832,
    release: fakeRelease(),
    coordinatorToken: Buffer.alloc(32, 1),
    pftlProxyToken: "p".repeat(48),
    coordinatorUrl: "http://127.0.0.1:18831",
    pftlProxyUrl: "ws://127.0.0.1:8080",
  };
  assert.throws(
    () => createProductionServer({ ...base, host: "0.0.0.0" }),
    /loopback/,
  );
  assert.throws(
    () => createProductionServer({
      ...base,
      coordinatorUrl: "http://example.com:18831",
    }),
    /loopback/,
  );
  assert.throws(
    () => createProductionServer({
      ...base,
      pftlProxyUrl: "ws://example.com:8080",
    }),
    /loopback/,
  );
});

test("manifest hash is stable over canonical release JSON", () => {
  assert.equal(
    sha256(Buffer.from("{\"a\":1}\n")),
    "e346432021b04179518d9614f3560ccd71354a4ee101ddcb893d6959a9d6301c",
  );
  assert.equal(CSP.includes("frame-ancestors 'none'"), true);
});

test("launcher verifies and copies reviewed code before Node execution", () => {
  const launcher = fs.readFileSync(
    path.resolve(__dirname, "../../../../scripts/lightning-navcoin-mainnet-ui"),
    "utf8",
  );
  assert.match(launcher, /preexec_verified_server/);
  assert.match(launcher, /production UI server does not match the reviewed manifest/);
  assert.match(launcher, /installed ws runtime does not match the reviewed manifest/);
  assert.match(launcher, /verified-production-ui-server-/);
  assert.match(launcher, /verified-ui-node-runtime-/);
  assert.doesNotMatch(launcher, /exec node "\$UI_SERVER" serve/);
  assert.match(launcher, /exec node "\$verified_server" serve/);
});
