#!/usr/bin/env node
"use strict";

/*
 * Immutable, loopback-only production UI edge for the real-value
 * Lightning/NAVcoin demo.
 *
 * The browser never receives either local bearer.  This process injects the
 * coordinator bearer into the narrow HTTP API and the wallet-proxy bearer
 * into a narrow WebSocket RPC allowlist.  It serves only bytes whose hashes
 * are recorded in an operator-pinned release manifest.
 */

const crypto = require("crypto");
const fs = require("fs");
const http = require("http");
const net = require("net");
const path = require("path");
const { spawnSync } = require("child_process");
const { WebSocket, WebSocketServer } = require("ws");

const RELEASE_SCHEMA = "postfiat.lightning_wallet_ui_release.v1";
const SOURCE_RELEASE_SCHEMA = "postfiat.lightning_coordinator_source_release.v1";
const MAX_MANIFEST_BYTES = 512 * 1024;
const MAX_STATIC_FILE_BYTES = 32 * 1024 * 1024;
const MAX_STATIC_TOTAL_BYTES = 96 * 1024 * 1024;
const MAX_STATIC_FILES = 4096;
const MAX_API_REQUEST_BYTES = 64 * 1024;
const MAX_API_RESPONSE_BYTES = 256 * 1024;
const MAX_RPC_MESSAGE_BYTES = 256 * 1024;
const COORDINATOR_PREFIX = "/api/lightning-navcoin/";
const REQUIRED_SOURCE_TARGETS = Object.freeze([
  "tools/lightning_navcoin_demo",
  "wallet-web",
  "scripts/lightning-navcoin-mainnet-ui",
]);
const PFTL_RPC_ALLOWLIST = new Set([
  "status",
  "server_info",
  "escrow_info",
  "escrow_fee_quote",
  "mempool_submit_signed_escrow_transaction_finality",
  "receipts",
]);
const CSP = [
  "default-src 'self'",
  "base-uri 'none'",
  "frame-ancestors 'none'",
  "form-action 'self'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "object-src 'none'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data:",
  "connect-src 'self'",
  "font-src 'self'",
  "manifest-src 'self'",
  "media-src 'none'",
  "worker-src 'none'",
].join("; ") + ";";

class UiServerError extends Error {}

function sha256(value) {
  return crypto.createHash("sha256").update(value).digest("hex");
}

function canonicalJson(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  return `{${Object.keys(value).sort().map(
    key => `${JSON.stringify(key)}:${canonicalJson(value[key])}`,
  ).join(",")}}`;
}

function assertPlainObject(value, label) {
  if (
    value === null
    || typeof value !== "object"
    || Array.isArray(value)
    || Object.getPrototypeOf(value) !== Object.prototype
  ) {
    throw new UiServerError(`${label} must be one JSON object`);
  }
  return value;
}

function parseStrictJson(bytes, label) {
  let value;
  try {
    const text = bytes.toString("ascii");
    if (!Buffer.from(text, "ascii").equals(bytes)) {
      throw new Error("non-ASCII bytes");
    }
    value = JSON.parse(text);
  } catch (error) {
    throw new UiServerError(`${label} is not valid ASCII JSON`);
  }
  return assertPlainObject(value, label);
}

function parseCanonicalJson(bytes, label) {
  const value = parseStrictJson(bytes, label);
  const canonical = Buffer.from(`${canonicalJson(value)}\n`, "ascii");
  if (!canonical.equals(bytes)) {
    throw new UiServerError(`${label} is not canonical JSON`);
  }
  return value;
}

function assertHex(value, bytes, label) {
  if (
    typeof value !== "string"
    || !new RegExp(`^[0-9a-f]{${bytes * 2}}$`).test(value)
  ) {
    throw new UiServerError(`${label} must be canonical lowercase hex`);
  }
  return value;
}

function assertOwnerSafeStat(
  stat,
  label,
  { privateFile = false, allowGroupWrite = false } = {},
) {
  if (!stat.isFile()) throw new UiServerError(`${label} must be a regular file`);
  const uid = typeof process.getuid === "function" ? process.getuid() : stat.uid;
  if (stat.uid !== uid && stat.uid !== 0) {
    throw new UiServerError(`${label} has an untrusted owner`);
  }
  const forbiddenWriteBits = allowGroupWrite ? 0o002 : 0o022;
  if (stat.mode & forbiddenWriteBits) {
    throw new UiServerError(`${label} must not be group/world writable`);
  }
  if (privateFile && (stat.mode & 0o077)) {
    throw new UiServerError(`${label} must be mode 0600`);
  }
}

function canonicalExistingFile(filePath, label, options = {}) {
  const absolute = path.resolve(filePath);
  let canonical;
  let stat;
  try {
    canonical = fs.realpathSync.native(absolute);
    stat = fs.lstatSync(absolute);
  } catch (error) {
    throw new UiServerError(`${label} is unavailable`);
  }
  if (canonical !== absolute || stat.isSymbolicLink()) {
    throw new UiServerError(`${label} must be a canonical non-symlink path`);
  }
  assertOwnerSafeStat(stat, label, options);
  return { path: absolute, stat };
}

function readBoundedFile(filePath, label, maximum, options = {}) {
  const checked = canonicalExistingFile(filePath, label, options);
  if (checked.stat.size < 1 || checked.stat.size > maximum) {
    throw new UiServerError(`${label} has an invalid size`);
  }
  const bytes = fs.readFileSync(checked.path);
  if (bytes.length !== checked.stat.size) {
    throw new UiServerError(`${label} changed while being read`);
  }
  return bytes;
}

function readCoordinatorToken(filePath) {
  const token = readBoundedFile(
    filePath,
    "coordinator API token",
    32,
    { privateFile: true },
  );
  if (token.length !== 32) {
    throw new UiServerError("coordinator API token must be exactly 32 bytes");
  }
  return token;
}

function readPftlProxyToken(filePath) {
  const raw = readBoundedFile(
    filePath,
    "PFTL wallet-proxy token",
    4096,
    { privateFile: true },
  );
  let token;
  try {
    token = raw.toString("ascii");
  } catch (_) {
    throw new UiServerError("PFTL wallet-proxy token must be ASCII");
  }
  if (
    Buffer.from(token, "ascii").length !== raw.length
    || token.length < 32
    || token.trim() !== token
    || /\s/.test(token)
  ) {
    throw new UiServerError(
      "PFTL wallet-proxy token must be 32-4096 non-whitespace ASCII bytes",
    );
  }
  return token;
}

function runGit(repoRoot, args) {
  const result = spawnSync("git", ["-C", repoRoot, ...args], {
    encoding: "utf8",
    maxBuffer: 2 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) {
    throw new UiServerError("git source-release verification failed");
  }
  return result.stdout.trim();
}

function verifyCleanSourceRelease(repoRoot, sourceReleasePath) {
  const root = fs.realpathSync.native(path.resolve(repoRoot));
  const source = parseStrictJson(
    readBoundedFile(
      sourceReleasePath,
      "coordinator source-release pin",
      MAX_MANIFEST_BYTES,
    ),
    "coordinator source-release pin",
  );
  if (
    source.schema !== SOURCE_RELEASE_SCHEMA
    || source.clean !== true
    || !Array.isArray(source.targets)
  ) {
    throw new UiServerError("coordinator source-release pin is not clean");
  }
  for (const target of ["tools/lightning_navcoin_demo", "wallet-web"]) {
    if (!source.targets.includes(target)) {
      throw new UiServerError(`coordinator source-release omits ${target}`);
    }
  }
  const commit = runGit(root, ["rev-parse", "--verify", "HEAD^{commit}"]);
  const tree = runGit(root, ["rev-parse", "--verify", "HEAD^{tree}"]);
  if (source.git_commit !== commit || source.git_tree !== tree) {
    throw new UiServerError("coordinator source-release does not match HEAD");
  }
  const dirty = runGit(root, [
    "status",
    "--porcelain=v1",
    "--untracked-files=all",
    "--",
    ...REQUIRED_SOURCE_TARGETS,
  ]);
  if (dirty) {
    throw new UiServerError("production UI sources are not a clean release");
  }
  return Object.freeze({ git_commit: commit, git_tree: tree });
}

function walkRegularFiles(rootPath, { rejectDevArtifacts = false } = {}) {
  const root = fs.realpathSync.native(path.resolve(rootPath));
  const rootStat = fs.lstatSync(root);
  if (!rootStat.isDirectory() || rootStat.isSymbolicLink()) {
    throw new UiServerError("release root must be a real directory");
  }
  const files = [];
  let totalBytes = 0;

  function visit(directory, prefix) {
    const entries = fs.readdirSync(directory, { withFileTypes: true })
      .sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
    for (const entry of entries) {
      if (
        !entry.name
        || entry.name === "."
        || entry.name === ".."
        || entry.name.includes("\\")
        || entry.name.includes("/")
      ) {
        throw new UiServerError("release contains an unsafe path");
      }
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      const absolute = path.join(directory, entry.name);
      const stat = fs.lstatSync(absolute);
      if (entry.isSymbolicLink() || stat.isSymbolicLink()) {
        throw new UiServerError(`release contains a symlink: ${relative}`);
      }
      if (entry.isDirectory()) {
        visit(absolute, relative);
        continue;
      }
      if (!entry.isFile() || !stat.isFile()) {
        throw new UiServerError(`release contains a non-file: ${relative}`);
      }
      if (
        rejectDevArtifacts
        && (
          relative.endsWith(".map")
          || relative.startsWith("src/")
          || relative.startsWith("node_modules/")
          || relative.startsWith("@vite/")
          || relative === "@vite/client"
          || relative.split("/").some(segment => segment.startsWith("."))
        )
      ) {
        throw new UiServerError(`production dist contains a dev artifact: ${relative}`);
      }
      if (stat.size > MAX_STATIC_FILE_BYTES) {
        throw new UiServerError(`release file is oversized: ${relative}`);
      }
      totalBytes += stat.size;
      if (totalBytes > MAX_STATIC_TOTAL_BYTES || files.length >= MAX_STATIC_FILES) {
        throw new UiServerError("release exceeds the static file limits");
      }
      const bytes = fs.readFileSync(absolute);
      if (bytes.length !== stat.size) {
        throw new UiServerError(`release file changed while read: ${relative}`);
      }
      files.push({
        path: relative,
        size: bytes.length,
        sha256: sha256(bytes),
        bytes,
      });
    }
  }

  visit(root, "");
  return files;
}

function fileTreeHash(files) {
  const hash = crypto.createHash("sha256");
  for (const file of files) {
    hash.update(file.path, "utf8");
    hash.update(Buffer.from([0]));
    hash.update(String(file.size), "ascii");
    hash.update(Buffer.from([0]));
    hash.update(file.sha256, "ascii");
    hash.update(Buffer.from("\n"));
  }
  return hash.digest("hex");
}

function installedWsEvidence() {
  const packagePath = require.resolve("ws/package.json");
  const packageValue = parseStrictJson(
    fs.readFileSync(packagePath),
    "installed ws package metadata",
  );
  if (packageValue.name !== "ws" || packageValue.version !== "8.21.1") {
    throw new UiServerError("installed WebSocket runtime is not ws 8.21.1");
  }
  const root = path.dirname(packagePath);
  const files = walkRegularFiles(root);
  return Object.freeze({
    name: "ws",
    version: packageValue.version,
    tree_sha256: fileTreeHash(files),
  });
}

function assertProductionIndex(files) {
  const index = files.find(file => file.path === "index.html");
  if (!index) throw new UiServerError("production dist has no index.html");
  const text = index.bytes.toString("utf8");
  for (const marker of ["/@vite/client", "react-refresh", "/src/main."]) {
    if (text.includes(marker)) {
      throw new UiServerError(`production index contains dev marker: ${marker}`);
    }
  }
  if (!files.some(file => /^assets\/.+-[A-Za-z0-9_-]{8,}\./.test(file.path))) {
    throw new UiServerError("production dist has no content-hashed asset");
  }
}

function releaseManifestValue({
  source,
  distFiles,
  packageLockSha256,
  serverSha256,
  wsEvidence,
}) {
  return {
    schema: RELEASE_SCHEMA,
    source_release: source,
    wallet_package_lock_sha256: packageLockSha256,
    production_ui_server_sha256: serverSha256,
    websocket_runtime: wsEvidence,
    dist_tree_sha256: fileTreeHash(distFiles),
    files: distFiles.map(({ path: filePath, size, sha256: digest }) => ({
      path: filePath,
      size,
      sha256: digest,
    })),
  };
}

function ensurePrivateDirectory(directoryPath) {
  const absolute = path.resolve(directoryPath);
  fs.mkdirSync(absolute, { recursive: true, mode: 0o700 });
  const canonical = fs.realpathSync.native(absolute);
  const stat = fs.lstatSync(absolute);
  if (canonical !== absolute || !stat.isDirectory() || stat.isSymbolicLink()) {
    throw new UiServerError("release output root must be a canonical real directory");
  }
  const uid = typeof process.getuid === "function" ? process.getuid() : stat.uid;
  if ((stat.uid !== uid && stat.uid !== 0) || (stat.mode & 0o077)) {
    throw new UiServerError("release output root must be owner-only");
  }
  return absolute;
}

function createRelease({
  repoRoot,
  sourceReleasePath,
  distPath,
  outputRoot,
}) {
  const source = verifyCleanSourceRelease(repoRoot, sourceReleasePath);
  const distFiles = walkRegularFiles(distPath, { rejectDevArtifacts: true });
  assertProductionIndex(distFiles);
  const packageLockPath = path.join(repoRoot, "wallet-web", "package-lock.json");
  const serverPath = path.join(
    repoRoot,
    "tools/lightning_navcoin_demo/mainnet/ui-server/production-ui-server.cjs",
  );
  const manifestValue = releaseManifestValue({
    source,
    distFiles,
    packageLockSha256: sha256(
      readBoundedFile(
        packageLockPath,
        "wallet package lock",
        8 * 1024 * 1024,
        { allowGroupWrite: true },
      ),
    ),
    serverSha256: sha256(
      readBoundedFile(
        serverPath,
        "production UI server",
        2 * 1024 * 1024,
        { allowGroupWrite: true },
      ),
    ),
    wsEvidence: installedWsEvidence(),
  });
  const manifestBytes = Buffer.from(`${canonicalJson(manifestValue)}\n`, "ascii");
  const manifestSha256 = sha256(manifestBytes);
  const root = ensurePrivateDirectory(outputRoot);
  const releasesRoot = ensurePrivateDirectory(
    path.join(root, "wallet-ui-releases"),
  );
  // The source-release pin is part of the manifest even when the built UI
  // bytes are unchanged.  Keying only by the dist tree would collide when an
  // unrelated reviewed source change advances HEAD, and would either discard
  // provenance or require overwriting the prior release.  The full manifest
  // digest is immutable and uniquely binds both source and served bytes.
  const releasePath = path.join(releasesRoot, manifestSha256);

  if (!fs.existsSync(releasePath)) {
    const temporary = fs.mkdtempSync(path.join(releasesRoot, ".release-"));
    fs.chmodSync(temporary, 0o700);
    const outputDist = path.join(temporary, "dist");
    fs.mkdirSync(outputDist, { mode: 0o700 });
    try {
      for (const file of distFiles) {
        const target = path.join(outputDist, ...file.path.split("/"));
        fs.mkdirSync(path.dirname(target), { recursive: true, mode: 0o700 });
        fs.writeFileSync(target, file.bytes, { mode: 0o600, flag: "wx" });
      }
      fs.writeFileSync(
        path.join(temporary, "manifest.json"),
        manifestBytes,
        { mode: 0o600, flag: "wx" },
      );
      fs.renameSync(temporary, releasePath);
    } catch (error) {
      fs.rmSync(temporary, { recursive: true, force: true });
      throw error;
    }
  }
  const manifestPath = path.join(releasePath, "manifest.json");
  const observed = readBoundedFile(
    manifestPath,
    "existing production UI manifest",
    MAX_MANIFEST_BYTES,
  );
  if (!observed.equals(manifestBytes)) {
    throw new UiServerError("existing release directory does not match this build");
  }
  return Object.freeze({
    schema: "postfiat.lightning_wallet_ui_release_created.v1",
    manifest_path: manifestPath,
    manifest_sha256: manifestSha256,
    dist_tree_sha256: manifestValue.dist_tree_sha256,
    source_release: source,
    value_moved: false,
  });
}

function validateManifestShape(manifest) {
  if (
    manifest.schema !== RELEASE_SCHEMA
    || !manifest.source_release
    || !Array.isArray(manifest.files)
    || manifest.files.length < 2
    || manifest.files.length > MAX_STATIC_FILES
  ) {
    throw new UiServerError("production UI manifest has an invalid schema");
  }
  assertHex(manifest.dist_tree_sha256, 32, "dist tree hash");
  assertHex(manifest.wallet_package_lock_sha256, 32, "wallet package lock hash");
  assertHex(manifest.production_ui_server_sha256, 32, "UI server hash");
  const paths = new Set();
  for (const file of manifest.files) {
    assertPlainObject(file, "manifest file");
    if (
      typeof file.path !== "string"
      || !file.path
      || file.path.startsWith("/")
      || file.path.includes("\\")
      || file.path.split("/").some(
        segment => !segment || segment === "." || segment === ".." || segment.startsWith("."),
      )
      || paths.has(file.path)
      || !Number.isSafeInteger(file.size)
      || file.size < 0
      || file.size > MAX_STATIC_FILE_BYTES
    ) {
      throw new UiServerError("production UI manifest has an unsafe file entry");
    }
    assertHex(file.sha256, 32, "manifest file hash");
    paths.add(file.path);
  }
  if (!paths.has("index.html")) {
    throw new UiServerError("production UI manifest does not pin index.html");
  }
}

function loadVerifiedRelease({
  repoRoot,
  sourceReleasePath,
  manifestPath,
  expectedManifestSha256,
}) {
  const expected = assertHex(
    expectedManifestSha256,
    32,
    "expected production UI manifest hash",
  );
  const manifestBytes = readBoundedFile(
    manifestPath,
    "production UI manifest",
    MAX_MANIFEST_BYTES,
  );
  if (sha256(manifestBytes) !== expected) {
    throw new UiServerError("production UI manifest hash does not match the operator pin");
  }
  const manifest = parseCanonicalJson(manifestBytes, "production UI manifest");
  validateManifestShape(manifest);
  const source = verifyCleanSourceRelease(repoRoot, sourceReleasePath);
  if (
    manifest.source_release.git_commit !== source.git_commit
    || manifest.source_release.git_tree !== source.git_tree
  ) {
    throw new UiServerError("production UI manifest is for a different source release");
  }
  const packageLockPath = path.join(repoRoot, "wallet-web", "package-lock.json");
  const serverPath = path.join(
    repoRoot,
    "tools/lightning_navcoin_demo/mainnet/ui-server/production-ui-server.cjs",
  );
  if (
    sha256(readBoundedFile(
      packageLockPath,
      "wallet package lock",
      8 * 1024 * 1024,
      { allowGroupWrite: true },
    ))
      !== manifest.wallet_package_lock_sha256
    || sha256(readBoundedFile(
      serverPath,
      "production UI server",
      2 * 1024 * 1024,
      { allowGroupWrite: true },
    ))
      !== manifest.production_ui_server_sha256
  ) {
    throw new UiServerError("production UI manifest does not match reviewed sources");
  }
  const wsEvidence = installedWsEvidence();
  if (
    !manifest.websocket_runtime
    || manifest.websocket_runtime.name !== wsEvidence.name
    || manifest.websocket_runtime.version !== wsEvidence.version
    || manifest.websocket_runtime.tree_sha256 !== wsEvidence.tree_sha256
  ) {
    throw new UiServerError("WebSocket runtime does not match the UI release");
  }
  const releaseRoot = path.dirname(path.resolve(manifestPath));
  const distRoot = path.join(releaseRoot, "dist");
  const observed = walkRegularFiles(distRoot, { rejectDevArtifacts: true });
  assertProductionIndex(observed);
  const expectedFiles = new Map(manifest.files.map(file => [file.path, file]));
  if (observed.length !== expectedFiles.size) {
    throw new UiServerError("production dist file inventory changed");
  }
  for (const file of observed) {
    const pin = expectedFiles.get(file.path);
    if (!pin || pin.size !== file.size || pin.sha256 !== file.sha256) {
      throw new UiServerError(`production dist file changed: ${file.path}`);
    }
  }
  if (fileTreeHash(observed) !== manifest.dist_tree_sha256) {
    throw new UiServerError("production dist tree hash changed");
  }
  return Object.freeze({
    manifestSha256: expected,
    distTreeSha256: manifest.dist_tree_sha256,
    source,
    // Bytes are intentionally retained in memory. Changes on disk after
    // startup cannot alter the release served by this process.
    files: new Map(observed.map(file => [file.path, Buffer.from(file.bytes)])),
  });
}

function parseLoopbackUrl(value, scheme, label) {
  let parsed;
  try {
    parsed = new URL(value);
  } catch (_) {
    throw new UiServerError(`${label} is not a valid URL`);
  }
  if (
    parsed.protocol !== `${scheme}:`
    || !["127.0.0.1", "::1"].includes(parsed.hostname)
    || !parsed.port
    || parsed.username
    || parsed.password
    || parsed.search
    || parsed.hash
    || (parsed.pathname !== "/" && parsed.pathname !== "")
  ) {
    throw new UiServerError(`${label} must be an explicit loopback ${scheme} URL`);
  }
  return parsed;
}

function contentType(filePath) {
  switch (path.extname(filePath).toLowerCase()) {
    case ".html": return "text/html; charset=utf-8";
    case ".js": return "text/javascript; charset=utf-8";
    case ".css": return "text/css; charset=utf-8";
    case ".json": return "application/json; charset=utf-8";
    case ".svg": return "image/svg+xml";
    case ".png": return "image/png";
    case ".ico": return "image/x-icon";
    case ".wasm": return "application/wasm";
    default: return "application/octet-stream";
  }
}

function setSecurityHeaders(response) {
  response.setHeader("Content-Security-Policy", CSP);
  response.setHeader("X-Frame-Options", "DENY");
  response.setHeader("X-Content-Type-Options", "nosniff");
  response.setHeader("Referrer-Policy", "no-referrer");
  response.setHeader(
    "Permissions-Policy",
    "camera=(), geolocation=(), microphone=(), payment=(), usb=()",
  );
  response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
  response.setHeader("Cross-Origin-Resource-Policy", "same-origin");
}

function sendJson(response, status, value) {
  const encoded = Buffer.from(canonicalJson(value), "ascii");
  response.statusCode = status;
  setSecurityHeaders(response);
  response.setHeader("Content-Type", "application/json; charset=ascii");
  response.setHeader("Cache-Control", "no-store");
  response.setHeader("Content-Length", encoded.length);
  response.end(encoded);
}

async function readRequestBody(request) {
  if (request.headers["transfer-encoding"] !== undefined) {
    throw new UiServerError("chunked request bodies are not accepted");
  }
  const lengthText = request.headers["content-length"];
  if (lengthText === undefined || !/^(0|[1-9][0-9]*)$/.test(lengthText)) {
    throw new UiServerError("request must have one canonical Content-Length");
  }
  const length = Number(lengthText);
  if (!Number.isSafeInteger(length) || length > MAX_API_REQUEST_BYTES) {
    throw new UiServerError("request body is oversized");
  }
  const chunks = [];
  let seen = 0;
  for await (const chunk of request) {
    seen += chunk.length;
    if (seen > length || seen > MAX_API_REQUEST_BYTES) {
      throw new UiServerError("request body length changed");
    }
    chunks.push(chunk);
  }
  if (seen !== length) throw new UiServerError("request body is incomplete");
  return Buffer.concat(chunks, seen);
}

function validateHost(request, expectedHost) {
  return request.headers.host === expectedHost;
}

async function proxyCoordinator(request, response, config, requestUrl) {
  if (!["GET", "POST"].includes(request.method)) {
    sendJson(response, 405, { ok: false, error: "method not allowed" });
    return;
  }
  if (requestUrl.search || requestUrl.hash) {
    sendJson(response, 400, { ok: false, error: "query strings are not accepted" });
    return;
  }
  if (request.method === "POST" && request.headers.origin !== config.uiOrigin) {
    sendJson(response, 403, { ok: false, error: "origin rejected" });
    return;
  }
  let body = Buffer.alloc(0);
  try {
    if (request.method === "POST") {
      const contentTypeValue = String(request.headers["content-type"] || "");
      if (!/^application\/json(?:;\s*charset=utf-8)?$/i.test(contentTypeValue)) {
        throw new UiServerError("coordinator mutations require application/json");
      }
      body = await readRequestBody(request);
    } else if (
      request.headers["content-length"] !== undefined
      && request.headers["content-length"] !== "0"
    ) {
      throw new UiServerError("GET request body is not accepted");
    }
  } catch (error) {
    sendJson(response, 400, { ok: false, error: error.message });
    return;
  }

  const headers = {
    accept: "application/json",
    authorization: `Bearer ${config.coordinatorToken.toString("hex")}`,
    host: config.coordinatorUrl.host,
  };
  if (request.method === "POST") {
    headers["content-type"] = "application/json";
    headers["content-length"] = String(body.length);
    headers.origin = config.uiOrigin;
    if (typeof request.headers["x-postfiat-csrf"] === "string") {
      headers["x-postfiat-csrf"] = request.headers["x-postfiat-csrf"];
    }
    if (typeof request.headers["x-requested-with"] === "string") {
      headers["x-requested-with"] = request.headers["x-requested-with"];
    }
  }

  await new Promise(resolve => {
    let settled = false;
    const finish = () => {
      if (settled) return false;
      settled = true;
      resolve();
      return true;
    };
    const fail = () => {
      if (settled) return;
      if (!response.headersSent) {
        sendJson(response, 502, { ok: false, error: "coordinator unavailable" });
      } else {
        response.destroy();
      }
      finish();
    };
    const upstream = http.request({
      protocol: "http:",
      hostname: config.coordinatorUrl.hostname,
      port: Number(config.coordinatorUrl.port),
      method: request.method,
      path: requestUrl.pathname,
      headers,
      timeout: 15_000,
    }, upstreamResponse => {
      const chunks = [];
      let seen = 0;
      upstreamResponse.on("error", fail);
      upstreamResponse.on("data", chunk => {
        if (settled) return;
        seen += chunk.length;
        if (seen > MAX_API_RESPONSE_BYTES) {
          upstreamResponse.destroy(new UiServerError("coordinator response is oversized"));
          return;
        }
        chunks.push(chunk);
      });
      upstreamResponse.on("end", () => {
        if (settled) return;
        const encoded = Buffer.concat(chunks, seen);
        const upstreamType = String(upstreamResponse.headers["content-type"] || "");
        if (
          !/^application\/json(?:;|$)/i.test(upstreamType)
          || encoded.includes(config.coordinatorToken)
          || encoded.includes(Buffer.from(config.coordinatorToken.toString("hex"), "ascii"))
        ) {
          sendJson(response, 502, { ok: false, error: "unsafe coordinator response" });
          finish();
          return;
        }
        response.statusCode = Number(upstreamResponse.statusCode) || 502;
        setSecurityHeaders(response);
        response.setHeader("Content-Type", "application/json; charset=ascii");
        response.setHeader("Cache-Control", "no-store");
        response.setHeader("Content-Length", encoded.length);
        response.end(encoded);
        finish();
      });
    });
    upstream.on("timeout", () => upstream.destroy(new Error("timeout")));
    upstream.on("error", fail);
    if (body.length) upstream.write(body);
    upstream.end();
  });
}

function rejectUpgrade(socket, status, message) {
  const body = Buffer.from(message, "ascii");
  socket.write(
    `HTTP/1.1 ${status}\r\n`
    + "Connection: close\r\n"
    + "Content-Type: text/plain; charset=ascii\r\n"
    + `Content-Length: ${body.length}\r\n\r\n`,
  );
  socket.end(body);
}

function safeCloseCode(code) {
  return [1000, 1001, 1008, 1009, 1011].includes(code) ? code : 1011;
}

function pairWebSockets(browser, upstream, pftlToken) {
  let closed = false;
  const closeBoth = (code = 1011) => {
    if (closed) return;
    closed = true;
    const safe = safeCloseCode(code);
    if (browser.readyState === WebSocket.OPEN) browser.close(safe);
    if (upstream.readyState === WebSocket.OPEN) upstream.close(safe);
  };

  browser.on("message", (data, isBinary) => {
    if (isBinary || data.length > MAX_RPC_MESSAGE_BYTES) {
      closeBoth(1009);
      return;
    }
    let request;
    try {
      request = assertPlainObject(JSON.parse(data.toString("utf8")), "PFTL RPC request");
      if (
        typeof request.version !== "string"
        || typeof request.id !== "string"
        || typeof request.method !== "string"
        || !PFTL_RPC_ALLOWLIST.has(request.method)
      ) {
        throw new UiServerError("PFTL RPC method is outside the UI allowlist");
      }
      // Discard any browser-provided credential, then inject the private local
      // dispatch credential immediately before the trusted loopback hop.
      delete request.proxy_auth_token;
      request.proxy_auth_token = pftlToken;
      const encoded = JSON.stringify(request);
      if (Buffer.byteLength(encoded) > MAX_RPC_MESSAGE_BYTES) {
        throw new UiServerError("PFTL RPC request is oversized");
      }
      upstream.send(encoded);
    } catch (_) {
      if (browser.readyState === WebSocket.OPEN) {
        browser.send(canonicalJson({
          version: "postfiat-local-rpc-v1",
          id: typeof request?.id === "string" ? request.id : "ui-proxy-error",
          ok: false,
          result: null,
          error: {
            code: "ui_rpc_rejected",
            message: "request is outside the production Lightning UI RPC surface",
          },
          events: [],
        }));
      }
    }
  });

  upstream.on("message", (data, isBinary) => {
    if (isBinary || data.length > MAX_RPC_MESSAGE_BYTES) {
      closeBoth(1009);
      return;
    }
    const encoded = data.toString("utf8");
    try {
      assertPlainObject(JSON.parse(encoded), "PFTL RPC response");
      if (encoded.includes(pftlToken)) {
        throw new UiServerError("PFTL proxy token appeared in an upstream response");
      }
    } catch (_) {
      closeBoth(1011);
      return;
    }
    if (browser.readyState === WebSocket.OPEN) browser.send(encoded);
  });

  browser.on("close", code => closeBoth(code));
  upstream.on("close", code => closeBoth(code));
  browser.on("error", () => closeBoth(1011));
  upstream.on("error", () => closeBoth(1011));
}

function createProductionServer(config) {
  if (!["127.0.0.1", "::1"].includes(config.host)) {
    throw new UiServerError("production UI must bind an explicit loopback address");
  }
  if (!Number.isInteger(config.port) || config.port < 1 || config.port > 65535) {
    throw new UiServerError("production UI port is invalid");
  }
  const hostLiteral = config.host === "::1" ? "[::1]" : config.host;
  const expectedHost = `${hostLiteral}:${config.port}`;
  const uiOrigin = `http://${expectedHost}`;
  const coordinatorUrl = parseLoopbackUrl(
    config.coordinatorUrl,
    "http",
    "coordinator URL",
  );
  const pftlProxyUrl = parseLoopbackUrl(
    config.pftlProxyUrl,
    "ws",
    "PFTL wallet-proxy URL",
  );
  const runtimeConfig = {
    ...config,
    expectedHost,
    uiOrigin,
    coordinatorUrl,
    pftlProxyUrl,
  };

  const server = http.createServer(async (request, response) => {
    if (!validateHost(request, expectedHost)) {
      sendJson(response, 421, { ok: false, error: "host rejected" });
      return;
    }
    let requestUrl;
    try {
      requestUrl = new URL(request.url || "/", uiOrigin);
    } catch (_) {
      sendJson(response, 400, { ok: false, error: "invalid URL" });
      return;
    }
    if (requestUrl.pathname.startsWith(COORDINATOR_PREFIX)) {
      await proxyCoordinator(request, response, runtimeConfig, requestUrl);
      return;
    }
    if (requestUrl.pathname === "/rpc") {
      response.statusCode = 426;
      setSecurityHeaders(response);
      response.setHeader("Connection", "close");
      response.end();
      return;
    }
    if (requestUrl.pathname === "/healthz" && request.method === "GET") {
      sendJson(response, 200, {
        schema: "postfiat.lightning_wallet_ui_health.v1",
        status: "READY",
        manifest_sha256: config.release.manifestSha256,
        dist_tree_sha256: config.release.distTreeSha256,
        source_release: config.release.source,
        value_moved: false,
      });
      return;
    }
    if (request.method !== "GET" && request.method !== "HEAD") {
      sendJson(response, 405, { ok: false, error: "method not allowed" });
      return;
    }
    if (requestUrl.search || requestUrl.hash || requestUrl.pathname.startsWith("/api/")) {
      sendJson(response, 404, { ok: false, error: "not found" });
      return;
    }
    let relative;
    try {
      relative = decodeURIComponent(requestUrl.pathname).replace(/^\/+/, "");
    } catch (_) {
      sendJson(response, 400, { ok: false, error: "invalid path" });
      return;
    }
    if (!relative) relative = "index.html";
    if (
      relative.includes("\\")
      || relative.split("/").some(
        segment => !segment || segment === "." || segment === ".." || segment.startsWith("."),
      )
    ) {
      sendJson(response, 404, { ok: false, error: "not found" });
      return;
    }
    const bytes = config.release.files.get(relative);
    if (!bytes) {
      sendJson(response, 404, { ok: false, error: "not found" });
      return;
    }
    response.statusCode = 200;
    setSecurityHeaders(response);
    response.setHeader("Content-Type", contentType(relative));
    response.setHeader(
      "Cache-Control",
      /^assets\/.+-[A-Za-z0-9_-]{8,}\./.test(relative)
        ? "public, max-age=31536000, immutable"
        : "no-store",
    );
    response.setHeader("Content-Length", bytes.length);
    if (request.method === "HEAD") response.end();
    else response.end(bytes);
  });

  server.on("upgrade", (request, socket, head) => {
    if (
      !validateHost(request, expectedHost)
      || request.url !== "/rpc"
      || request.headers.origin !== uiOrigin
    ) {
      rejectUpgrade(socket, "403 Forbidden", "WebSocket origin rejected");
      return;
    }
    const upstream = new WebSocket(pftlProxyUrl.href, {
      origin: uiOrigin,
      perMessageDeflate: false,
      maxPayload: MAX_RPC_MESSAGE_BYTES,
      handshakeTimeout: 5_000,
    });
    const onSocketClose = () => {
      if (upstream.readyState === WebSocket.CONNECTING) upstream.terminate();
    };
    socket.once("close", onSocketClose);
    const timeout = setTimeout(() => {
      upstream.terminate();
      if (!socket.destroyed) {
        rejectUpgrade(socket, "502 Bad Gateway", "PFTL proxy unavailable");
      }
    }, 5_500);
    const handshakeError = () => {
      clearTimeout(timeout);
      if (!socket.destroyed) {
        rejectUpgrade(socket, "502 Bad Gateway", "PFTL proxy unavailable");
      }
    };
    upstream.once("error", handshakeError);
    upstream.once("open", () => {
      clearTimeout(timeout);
      socket.off("close", onSocketClose);
      upstream.off("error", handshakeError);
      const wss = new WebSocketServer({
        noServer: true,
        perMessageDeflate: false,
        maxPayload: MAX_RPC_MESSAGE_BYTES,
      });
      try {
        wss.handleUpgrade(request, socket, head, browser => {
          pairWebSockets(browser, upstream, config.pftlProxyToken);
        });
      } catch (_) {
        upstream.close(1011);
        socket.destroy();
      }
    });
  });
  server.requestTimeout = 20_000;
  server.headersTimeout = 10_000;
  server.keepAliveTimeout = 5_000;
  return Object.freeze({ server, uiOrigin, expectedHost });
}

function parseArgs(argv) {
  if (argv.length < 1) throw new UiServerError("command is required");
  const command = argv[0];
  const options = {};
  for (let index = 1; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined || value.startsWith("--")) {
      throw new UiServerError("options must be explicit --name value pairs");
    }
    const name = key.slice(2);
    if (Object.hasOwn(options, name)) {
      throw new UiServerError(`duplicate option: --${name}`);
    }
    options[name] = value;
  }
  return { command, options };
}

function requiredOption(options, name) {
  const value = options[name];
  if (!value) throw new UiServerError(`--${name} is required`);
  return value;
}

function outputJson(value, stream = process.stdout) {
  stream.write(`${canonicalJson(value)}\n`);
}

async function main(argv = process.argv.slice(2)) {
  const { command, options } = parseArgs(argv);
  if (command === "release") {
    const result = createRelease({
      repoRoot: requiredOption(options, "repo-root"),
      sourceReleasePath: requiredOption(options, "source-release"),
      distPath: requiredOption(options, "dist"),
      outputRoot: requiredOption(options, "output-root"),
    });
    outputJson(result);
    return 0;
  }
  if (command !== "serve") throw new UiServerError("unknown command");
  const portText = requiredOption(options, "port");
  if (!/^[1-9][0-9]{0,4}$/.test(portText)) {
    throw new UiServerError("--port must be a canonical positive integer");
  }
  const release = loadVerifiedRelease({
    repoRoot: requiredOption(options, "repo-root"),
    sourceReleasePath: requiredOption(options, "source-release"),
    manifestPath: requiredOption(options, "manifest"),
    expectedManifestSha256: requiredOption(options, "manifest-sha256"),
  });
  const coordinatorToken = readCoordinatorToken(
    requiredOption(options, "coordinator-token-file"),
  );
  const pftlProxyToken = readPftlProxyToken(
    requiredOption(options, "pftl-proxy-token-file"),
  );
  const production = createProductionServer({
    host: requiredOption(options, "host"),
    port: Number(portText),
    release,
    coordinatorToken,
    pftlProxyToken,
    coordinatorUrl: requiredOption(options, "coordinator-url"),
    pftlProxyUrl: requiredOption(options, "pftl-proxy-url"),
  });
  production.server.listen(Number(portText), requiredOption(options, "host"), () => {
    outputJson({
      schema: "postfiat.lightning_wallet_ui_server.v1",
      status: "READY",
      origin: production.uiOrigin,
      manifest_sha256: release.manifestSha256,
      dist_tree_sha256: release.distTreeSha256,
      coordinator_auth: "SERVER_SIDE",
      pftl_proxy_auth: "SERVER_SIDE",
      value_moved: false,
    }, process.stderr);
  });
  const stop = () => production.server.close(() => process.exitCode = 0);
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
  return new Promise(resolve => production.server.once("close", () => resolve(0)));
}

if (require.main === module) {
  main().then(
    code => { process.exitCode = code; },
    error => {
      const message = error instanceof UiServerError
        ? error.message
        : "production UI server failed closed";
      outputJson({
        schema: "postfiat.lightning_wallet_ui_error.v1",
        status: "HOLD",
        error: message,
        value_moved: false,
      }, process.stderr);
      process.exitCode = 1;
    },
  );
}

module.exports = {
  CSP,
  PFTL_RPC_ALLOWLIST,
  UiServerError,
  assertProductionIndex,
  canonicalJson,
  createProductionServer,
  createRelease,
  fileTreeHash,
  loadVerifiedRelease,
  pairWebSockets,
  readCoordinatorToken,
  readPftlProxyToken,
  releaseManifestValue,
  setSecurityHeaders,
  sha256,
  verifyCleanSourceRelease,
  walkRegularFiles,
};
