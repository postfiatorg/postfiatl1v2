# Validator CLI (`upgrade`, `top`), 2-of-3 Release Manifest, and Read-Replica RPC Profiles Specification (`v1`)

**Status:** Frozen Phase 0 Contract C2
**Related:** [`validator-uptime-feed-v1.md`](validator-uptime-feed-v1.md), [`fast-validator-reads-20261124.md`](fast-validator-reads-20261124.md), [`public-rpc-operator-policy.md`](../runbooks/public-rpc-operator-policy.md), [`signed-deployment-manifest.md`](../runbooks/signed-deployment-manifest.md), [`release-process.md`](../release-process.md), [`validator-doctor.md`](../runbooks/validator-doctor.md)

## 1. Architectural Invariant: Single Binary (`postfiat-node`)

There is **no separate RPC codebase**. Public and private RPC nodes run the exact same `postfiat-node` binary in non-voting read-replica / RPC mode, installed and upgraded from the exact same 2-of-3 signed release artifact channel as validators.

This specification freezes four contracts required for Pillar 3 milestones M1, M3, and M4:

1. **2-of-3 signed release manifest format** (`postfiat.release_manifest.v3`) and pinned keyset (`postfiat.release_keyset.v1`);
2. **`postfiat-node upgrade`** CLI interface, disk-space preflight, atomic rollback, and double-sign guard isolation;
3. **`postfiat-node top`** read-only SSH terminal dashboard CLI interface and JSON snapshot schema;
4. **Systemd (and Docker Compose) read-replica RPC profiles** (`private` and `public`), anti-DoS consensus isolation, and `< 250ms` p95 read-path SLO targets.

---

## 2. 2-of-3 Signed Release Manifest Format (`postfiat.release_manifest.v3`)

### 2.1 Release Channel Artifacts

Every tagged public release (`linux-x86_64` and `linux-aarch64`) publishes an immutable bundle containing:

- `postfiat-node-linux-x86_64` and `postfiat-node-linux-aarch64` binaries;
- Packaged systemd unit templates (validator, two-user signer, private RPC replica, public RPC replica) and Docker Compose profiles;
- Deterministic CycloneDX SBOM (`postfiat-cargo-sbom.cdx.json`);
- `SHA256SUMS`;
- Pinned finalized catch-up checkpoint (`checkpoint_height`, `checkpoint_block_hash`, `checkpoint_state_root`, `certificate_id`, `committee_epoch`);
- `release-manifest.json` (`postfiat.release_manifest.v3`), co-signed by **2-of-3 offline ML-DSA-65 release keys** (covering the independent second builder so no single publisher key is a single point of failure).

### 2.2 Pinned Release Keyset (`postfiat.release_keyset.v1`)

Installed at `/etc/postfiat/release-keyset.json` (mode `0644`, owned by `root:root`):

```json
{
  "schema": "postfiat.release_keyset.v1",
  "keyset_id": "postfiat-release-keyset-2026-v1",
  "threshold": 2,
  "total_signers": 3,
  "algorithm_id": "ML-DSA-65",
  "signing_context": "postfiat-l1-v2/release-manifest/v3",
  "signers": [
    {
      "signer_id": "release-lead-1",
      "role": "release_lead",
      "publisher": "<bech32-or-hex-publisher-address>",
      "public_key_hex": "<3904-lowercase-hex-mldsa65-pubkey>"
    },
    {
      "signer_id": "independent-builder-2",
      "role": "independent_builder",
      "publisher": "<bech32-or-hex-publisher-address>",
      "public_key_hex": "<3904-lowercase-hex-mldsa65-pubkey>"
    },
    {
      "signer_id": "security-custodian-3",
      "role": "security_custodian",
      "publisher": "<bech32-or-hex-publisher-address>",
      "public_key_hex": "<3904-lowercase-hex-mldsa65-pubkey>"
    }
  ]
}
```

Key rotation requires publishing a successor `postfiat.release_keyset.v1` co-signed by 2-of-3 of the **prior** keyset and documenting the fingerprint transition in the release notes.

### 2.3 Release Manifest Schema (`postfiat.release_manifest.v3`)

This evolves `postfiat.deployment_manifest.v2` (see [`signed-deployment-manifest.md`](../runbooks/signed-deployment-manifest.md)) to a 2-of-3 multi-architecture public release manifest:

```json
{
  "schema": "postfiat.release_manifest.v3",
  "release_id": "v0.3.0-testnet",
  "created_unix": 1795651200,
  "valid_from_unix": 1795651200,
  "valid_until_unix": 1803427200,
  "chain_id": "postfiat-testnet-1",
  "genesis_hash": "<96-lowercase-hex-sha3-384>",
  "git_revision": "<40-lowercase-hex-commit-sha>",
  "build_profile": "release",
  "build_features": ["privacy", "rpc", "transport"],
  "protocol_version": 2,
  "min_compatible_protocol_version": 1,
  "rpc_schema": "postfiat-local-rpc-v1",
  "rollout": {
    "mode": "activation_height",
    "activation_height": 500000,
    "advance_warning_blocks": 28800,
    "halt_if_missed": true
  },
  "checkpoint": {
    "height": 432000,
    "block_hash": "<96-lowercase-hex-sha3-384>",
    "state_root": "<96-lowercase-hex-sha3-384>",
    "certificate_id": "<96-lowercase-hex-sha3-384>",
    "committee_epoch": 12
  },
  "artifacts": {
    "linux-x86_64": {
      "binary_url": "https://releases.postfiat.org/v0.3.0-testnet/postfiat-node-linux-x86_64",
      "binary_sha256": "<64-lowercase-hex-sha256>",
      "binary_size_bytes": 28412928
    },
    "linux-aarch64": {
      "binary_url": "https://releases.postfiat.org/v0.3.0-testnet/postfiat-node-linux-aarch64",
      "binary_sha256": "<64-lowercase-hex-sha256>",
      "binary_size_bytes": 27918336
    },
    "systemd_bundle_sha256": "<64-lowercase-hex-sha256>",
    "docker_profiles_sha256": "<64-lowercase-hex-sha256>",
    "sbom_sha256": "<64-lowercase-hex-sha256>",
    "checksums_sha256": "<64-lowercase-hex-sha256>",
    "swap_circuit_metadata_sha256": "<64-lowercase-hex-sha256>",
    "private_egress_circuit_metadata_sha256": "<64-lowercase-hex-sha256>"
  },
  "signing_policy": {
    "keyset_id": "postfiat-release-keyset-2026-v1",
    "threshold": 2,
    "total_signers": 3,
    "algorithm_id": "ML-DSA-65",
    "signing_context": "postfiat-l1-v2/release-manifest/v3"
  },
  "signatures": [
    {
      "signer_id": "release-lead-1",
      "publisher": "<publisher-address>",
      "algorithm_id": "ML-DSA-65",
      "public_key_hex": "<3904-lowercase-hex>",
      "signed_at_unix": 1795651260,
      "signature_hex": "<6618-lowercase-hex>"
    },
    {
      "signer_id": "independent-builder-2",
      "publisher": "<publisher-address>",
      "algorithm_id": "ML-DSA-65",
      "public_key_hex": "<3904-lowercase-hex>",
      "signed_at_unix": 1795651320,
      "signature_hex": "<6618-lowercase-hex>"
    }
  ]
}
```

### 2.4 Signature Verification Rules (Fail Closed)

1. Canonical signing bytes are the UTF-8 compact sorted-key JSON of the manifest object with `"signatures": []`.
2. Each entry in `signatures` MUST reference a distinct `signer_id` present in the pinned `postfiat.release_keyset.v1`, with `public_key_hex` matching the pinned key byte-for-byte.
3. At least `2` distinct entries MUST verify under pure FIPS 204 **ML-DSA-65** with context `postfiat-l1-v2/release-manifest/v3`.
4. `valid_from_unix <= now_unix <= valid_until_unix` MUST hold.
5. `rollout.mode` MUST be either `"rolling"` (`activation_height: null`, `halt_if_missed: false`) or `"activation_height"` (`activation_height >= 1`, `advance_warning_blocks >= 1`, `halt_if_missed: true`).
6. Any missing, duplicate, expired, wrong-chain, or invalid signature is a hard refusal.

---

## 3. `postfiat-node upgrade` CLI Contract

Applying an upgrade is **always manual** via `postfiat-node upgrade` so consensus nodes never auto-reboot mid-round.

### 3.1 Synopsis and Flags

```text
postfiat-node upgrade [OPTIONS]
```

| Flag | Default | Description |
| --- | --- | --- |
| `--check` | `false` | Read-only preflight: verify 2-of-3 release manifest, report rollout mode (`rolling` vs `activation_height H`, advance warning window, halt-if-missed), check disk headroom, and exit without mutating files or services. |
| `--manifest-file PATH` | none | Local path to a `postfiat.release_manifest.v3` JSON file. Mutually exclusive with `--channel-url`. |
| `--channel-url URL` | `https://releases.postfiat.org/latest/release-manifest.json` | Fixed HTTPS release channel URL (TLS verified, no untrusted redirects, 1 MiB max manifest body). |
| `--keyset-file PATH` | `/etc/postfiat/release-keyset.json` | Pinned 2-of-3 `postfiat.release_keyset.v1` trust file. |
| `--data-dir PATH` | `/var/lib/postfiat/validator-0` | Node chain data directory (backed up during upgrade; NEVER contains signer guard state). |
| `--signer-guard-dir PATH` | `/var/lib/postfiat/signer-guard` | Immutable double-sign guard directory. Verified disjoint from `--data-dir` and `--backup-dir`; NEVER copied, backed up, or rolled back. |
| `--binary-path PATH` | `/usr/local/bin/postfiat-node` | Installed binary path to swap atomically. |
| `--backup-dir PATH` | `/var/lib/postfiat/backups` | Destination directory for pre-upgrade binary and chain-state backups. |
| `--service-units UNITS` | `postfiat-validator-0.service,postfiat-validator-0-rpc.service` | Comma-separated systemd units stopped before backup and started after swap. |
| `--rpc-host HOST` | `127.0.0.1` | Loopback RPC host used for post-upgrade health probe. |
| `--rpc-port PORT` | `27650` | Loopback RPC port used for post-upgrade health probe. |
| `--health-timeout-ms MS` | `60000` | Maximum wait for post-upgrade health convergence before triggering automatic rollback. |
| `--yes` | `false` | Non-interactive confirmation (required when stdin is not a TTY unless `--check`). |
| `--json` | `false` | Emit structured `postfiat-node-upgrade-report-v1` JSON to stdout. |

### 3.2 Mandatory Ordered Execution Sequence

1. **Manifest & artifact verification:** Verify 2-of-3 ML-DSA-65 signatures against `--keyset-file`, validate `chain_id` and `genesis_hash` against local `status`, select the host architecture (`linux-x86_64` or `linux-aarch64`), download/stage the candidate binary, and verify `binary_sha256` and `binary_size_bytes`.
2. **Double-sign guard isolation check (HARD INVARIANT):** Canonicalize `--signer-guard-dir`, `--data-dir`, and `--backup-dir`. Refuse immediately (`exit 3`, error code `guard_path_overlap`) if `--signer-guard-dir` is equal to, nested inside, or an ancestor of `--data-dir` or `--backup-dir`. **The double-sign guard directory (`/var/lib/postfiat/signer-guard/`) is NEVER copied, backed up, modified, or rolled back.**
3. **Disk-space preflight:** Compute total byte size `D` of `--data-dir` and `B` of the candidate binary. Verify free bytes on `--backup-dir`'s filesystem satisfy `free_bytes >= 2 * D + B + 536870912` (`2x` chain data + binary + 512 MiB headroom). Refuse with `exit 4` (`insufficient_disk_space`) before stopping any service.
4. **In `--check` mode:** Stop here and emit the report (`exit 0` if already at `release_id`; `exit 10` if upgrade available; `exit 11` if `activation_height` is within `advance_warning_blocks`).
5. **Graceful service stop:** Stop `--service-units` via systemd so `redb` closes cleanly with zero writes in flight.
6. **Chain-state-only backup:** Create `<backup-dir>/<release-id>-pre-<unix>/` containing a verified copy of **chain state only** (`--data-dir`, excluding runtime sockets and strictly excluding `--signer-guard-dir`) plus the prior `postfiat-node` binary and a `backup-manifest.json` with SHA-256 digests.
7. **Atomic binary swap:** Write staged binary to `<binary-path>.staging.<pid>`, `chmod 0755`, `fsync`, atomically `rename(2)` onto `<binary-path>`, and `fsync` parent directory.
8. **Start and post-upgrade health gate:** Start `--service-units` and verify within `--health-timeout-ms`:
   - `rpc-probe` returns `ok: true`;
   - reported `git_revision` equals manifest `git_revision`;
   - `block_height >= pre_upgrade_height`;
   - when two-user signer socket mode is configured, `private_readiness` reports `validator_signer = "socket"` and `signer_double_sign_protection = true`.
9. **Atomic rollback on failure:** If any check in Step 8 fails:
   - Stop `--service-units`;
   - Restore the backed-up prior binary AND pre-upgrade chain-state snapshot together atomically (still **never** touching `/var/lib/postfiat/signer-guard/`, whose monotonic round floors prevent rollback-vs-guard deadlocks and prevent double-signing);
   - Restart `--service-units` and run `rpc-probe`;
   - Exit `5` (`rolled_back`) if the prior version recovers cleanly, or `6` (`rollback_failed`) for immediate operator intervention.

### 3.3 Exit Codes

| Exit Code | Meaning |
| --- | --- |
| `0` | Upgrade applied and healthy (or already up-to-date in `--check` mode). |
| `2` | Invalid CLI arguments or configuration error. |
| `3` | Security refusal: invalid 2-of-3 manifest signature, checksum mismatch, chain mismatch, or signer-guard path overlap. |
| `4` | Preflight failure: insufficient free disk space (`< 2x` chain data) or unhealthy pre-upgrade state. |
| `5` | Post-upgrade health check failed; automatic binary + chain-state rollback succeeded. |
| `6` | Critical: post-upgrade health check failed AND rollback verification failed. |
| `10` | `--check` mode: newer verified rolling or non-urgent upgrade is available. |
| `11` | `--check` mode: `activation_height` upgrade is inside `advance_warning_blocks` (action required before halt height). |

---

## 4. `postfiat-node top` SSH Dashboard CLI Contract

`postfiat-node top` provides a read-only terminal dashboard over SSH with **no listening web port**, no write RPC calls, and no access to private key files or signer guard files.

### 4.1 Synopsis and Flags

```text
postfiat-node top [OPTIONS]
```

| Flag | Default | Description |
| --- | --- | --- |
| `--rpc-host HOST` | `127.0.0.1` | Target RPC listener host (loopback enforced unless `--allow-remote-rpc` is explicitly set). |
| `--rpc-port PORT` | `27650` | Target RPC listener port. |
| `--allow-remote-rpc` | `false` | Explicit opt-in to query a non-loopback read-only RPC endpoint. |
| `--data-dir PATH` | none | Optional local data-dir path for read-only `statvfs`/size inspection (never opens `redb`). |
| `--uptime-feed-url URL` | none | HTTPS URL of the signed `postfiat-validator-uptime-v1` feed. |
| `--uptime-feed-file PATH` | none | Local file path to a `postfiat-validator-uptime-v1` JSON feed (for offline testing/rehearsal). |
| `--observer-keyset-file PATH` | `/etc/postfiat/observer-keyset.json` | Pinned 3-observer ML-DSA-65 public keys used to verify 2-of-3 signatures on the uptime feed locally. |
| `--release-manifest-url URL` | none | Optional signed `postfiat.release_manifest.v3` URL for upgrade/activation-height alerts. |
| `--release-keyset-file PATH` | `/etc/postfiat/release-keyset.json` | Pinned 2-of-3 release keyset used to verify the release manifest. |
| `--refresh-ms MS` | `2000` | Poll interval in milliseconds (`500..=60000`). |
| `--once` | `false` | Render one snapshot and exit `0` (safe for SSH one-liners and scripts). |
| `--json` | `false` | Emit structured `postfiat-node-top-snapshot-v1` JSON (single object with `--once`, NDJSON stream otherwise). |
| `--no-color` | auto | Disable ANSI colors (automatically enabled when `NO_COLOR=1` or stdout is not a TTY). |

### 4.2 Required Dashboard Panels and Data Sources

| Panel | Fields Displayed | Source |
| --- | --- | --- |
| **1. Node & Release Banner** | `node_id`, `role` (`validator` / `rpc_follower`), `chain_id`, installed `git_revision`, uptime, upgrade alert (`UP TO DATE`, `ROLLING UPGRADE AVAILABLE`, or `ACTIVATION HEIGHT H IN N BLOCKS`) | Local `status` + `server_info` + verified `postfiat.release_manifest.v3` |
| **2. Height & Sync Lag** | `finalized_height`, 12-char `block_tip_hash`, `state_root` prefix, `committee_epoch`, `height_lag` vs peer/observer median, median block interval | Local `status` + `metrics` + verified uptime feed |
| **3. Peers & Vote Signing** | `connected_peers / configured_peers`, `quorum_size`, recent vote signing rate (last 100 heights + latest height-day bps + 30d mean bps), `last_voted_height` | Local `status` + `validators` + verified uptime feed |
| **4. Signer Guard Health** | `signer_mode` (`socket` / `plaintext_key_file` / `none`), `signer_reachable`, `signer_double_sign_protection`, `signer_key_matches_registry`, `separate_uid_enforced`, `custody_reasons` | Local `private_readiness` |
| **5. Disk & RPC Latency** | `data_dir_bytes`, `fs_free_bytes`, `upgrade_headroom_ok` (`free >= 2x data`), `rpc_read_p95_ms`, `rss_mib`, `history_retention_mode` | `statvfs` + local `metrics` + `history_status` |
| **6. Task Node Reward Eligibility** | `access_state` (`pending`/`active`/`lapsed`/`revoked`), `observer_quorum` (`3/3`, `2/3`, `stale`), `feed_age_seconds`, `observed_height_days` (`X/30`), `mean_daily_participation_bps`, `grace_remaining` & `recovery_streak` (`k/3`) if `lapsed`, `finance_hub_access`, `reward_eligible` | Locally verified 2-of-3 `postfiat-validator-uptime-v1` feed |
| **7. Active Warnings** | Bounded list of typed warning codes (`FEED_STALE`, `FEED_UNAVAILABLE`, `LOW_DISK_FOR_UPGRADE`, `ACTIVATION_IMMINENT`, `PARTICIPATION_BELOW_95`, `SIGNER_SAME_USER`, `HEIGHT_LAG_HIGH`) | Derived deterministically |

### 4.3 Interactive Controls and Security Invariants

- **Interactive keys:** `q` / `Ctrl-C` quit, `r` immediate refresh, `p` pause/resume refresh, `j`/`k` scroll warnings.
- **Fail-soft external inputs:** If `--uptime-feed-url` or `--release-manifest-url` is unreachable or fails signature verification, `postfiat-node top` MUST display `FEED_UNAVAILABLE` / `RELEASE_CHECK_UNAVAILABLE` in the warnings panel and continue rendering live local node metrics without crashing.
- **Strict read-only posture:** `postfiat-node top` MUST NEVER open private keys, write files under `--data-dir` or `/var/lib/postfiat/signer-guard/`, or bind any listening network port.

---

## 5. Systemd Read-Replica Public RPC Profile Contract

### 5.1 Consensus Isolation (Anti-DoS) Requirement

Validators opting into public RPC or `--allow-private-action-admission` MUST isolate public RPC and admission traffic from consensus voting using one of two supported topologies:

1. **Topology A — Dedicated same-host cgroup-isolated read-replica service:** `postfiat-rpc-replica.service` runs under its own unprivileged UID (`postfiat-rpc`, distinct from consensus `postfiat` and signer `postfiat-signer`), serves read traffic from a non-blocking read-replica store/snapshot (never contending on the consensus writer's exclusive `redb` lock), and is capped with strict systemd CPU, memory, I/O, and task cgroups so consensus voting retains priority headroom.
2. **Topology B — Dedicated non-voting follower host:** A separate host runs `postfiat-node` in non-voting follower mode, catching up from release-pinned checkpoints via `postfiat-node rpc-catch-up-certified-delta`, and optionally forwarding pre-checked private-action proofs to an approved validator admission edge.

### 5.2 Non-Voting Follower Mode Invariants

A node running under any RPC read-replica profile MUST enforce all of the following at startup:

- `node_role` MUST report `"rpc_follower"` in `status` and `server_info`;
- `voting_enabled` MUST be `false`;
- `validator_signer` MUST be `"none"`;
- Passing `--finality-key-file`, `--finality-proposal-key-file`, or `--finality-signer-socket` to a read-replica profile MUST fail closed immediately (`rpc_replica_signer_forbidden`);
- Read-replica processes MUST NEVER bind plaintext RPC to a public or wildcard address (`0.0.0.0`, `::`, or public IP); loopback (`127.0.0.1` / `::1`) or a permission-scoped Unix socket is mandatory.

### 5.3 Packaged Profiles (`private` and `public`)

Both profiles ship as generated systemd units (extending [`systemd/postfiat-rpc.service.example`](../../systemd/postfiat-rpc.service.example)) and matching Docker Compose profiles (`docker-compose.rpc-private.yml`, `docker-compose.rpc-public.yml`):

| Parameter | Private Loopback Profile (`postfiat-rpc-private.service`) | Public TLS Read-Replica Profile (`postfiat-rpc-public.service` + TLS edge) |
| --- | --- | --- |
| **OS User / Group** | `postfiat-rpc:postfiat-rpc` | `postfiat-rpc:postfiat-rpc` (node), `postfiat-edge:postfiat-edge` (TLS reverse proxy) |
| **Bind Address** | `127.0.0.1:27650` | `127.0.0.1:27651` (or `/run/postfiat-rpc-public/rpc.sock`); TLS 1.3 terminated on `:443` by edge |
| **Node Mode Flag** | `--rpc-profile private-replica` | `--rpc-profile public-replica` |
| **Write Surface** | Disabled (read allowlist only) | Disabled by default; `--allow-private-action-admission` is an explicit bounded opt-in |
| **Checkpoint Catch-Up** | `rpc-catch-up-certified-delta` from release-pinned checkpoint | `rpc-catch-up-certified-delta` from release-pinned checkpoint |
| **Max Replica Lag Before Unready** | `5` blocks (`ready = false` when exceeded) | `5` blocks (`ready = false` -> edge drains host) |
| **`CPUQuota` / `CPUWeight`** | `100%` / `50` | `200%` / `25` (consensus unit runs at `CPUWeight=200`) |
| **`MemoryHigh` / `MemoryMax`** | `2G` / `3G` | `3G` / `4G` |
| **`IOWeight`** | `100` | `50` (consensus unit runs at `IOWeight=500`) |
| **`TasksMax` / `LimitNOFILE`** | `512` / `65536` | `512` / `65536` |
| **`--max-requests`** | `100000` (`Restart=always`, `RestartSec=1`) | `100000` (`Restart=always`, `RestartSec=1`) |
| **`--timeout-ms` / `--child-timeout-ms`** | `30000` / `30000` | `15000` / `15000` |
| **`--max-child-dispatch-concurrent` / `per-peer`** | `8` / `4` | `8` / `4` |
| **Edge Limits (Public Profile)** | N/A | TLS 1.3 only, `MAX_RPC_REQUEST_BYTES=65536`, per-IP rate limit + burst cap, zero JSON body rewriting, optional x402 payment header gate |
| **Systemd Hardening (Mandatory on Both)** | `NoNewPrivileges=true`, `ProtectSystem=strict`, `ProtectHome=true`, `PrivateTmp=true`, `ProtectKernelTunables=true`, `ProtectKernelModules=true`, `ProtectControlGroups=true`, `LockPersonality=true`, `RestrictSUIDSGID=true`, `CapabilityBoundingSet=`, `AmbientCapabilities=`, `LimitCORE=0`, `UMask=0077` | Same mandatory hardening on node unit; edge unit holds zero capabilities except `CAP_NET_BIND_SERVICE` if binding `:443` directly |

### 5.4 Canonical Public Read-Replica Systemd Unit Contract

```ini
[Unit]
Description=PostFiat Public Read-Replica RPC (non-voting follower)
After=network-online.target
Wants=network-online.target
StartLimitIntervalSec=300
StartLimitBurst=5

[Service]
Type=simple
User=postfiat-rpc
Group=postfiat-rpc
ExecStartPre=/usr/local/bin/postfiat-node deployment-manifest-verify \
  --manifest-file /etc/postfiat/release-manifest.json \
  --trusted-publisher-key-file /etc/postfiat/release-keyset.json \
  --runtime-binary-file /usr/local/bin/postfiat-node
ExecStart=/usr/local/bin/postfiat-node rpc-serve \
  --rpc-profile public-replica \
  --data-dir /var/lib/postfiat-rpc/replica \
  --spool-dir /run/postfiat-rpc-public/spool \
  --ready-file /run/postfiat-rpc-public/ready.json \
  --bind-host 127.0.0.1 \
  --port 27651 \
  --max-requests 100000 \
  --timeout-ms 15000 \
  --child-timeout-ms 15000 \
  --max-child-dispatch-concurrent 8 \
  --max-child-dispatch-per-peer 4 \
  --max-replica-lag-blocks 5 \
  --event-log /var/log/postfiat-rpc/rpc-events.ndjson \
  --keep-alive
RuntimeDirectory=postfiat-rpc-public
RuntimeDirectoryMode=0700
Restart=always
RestartSec=1
TimeoutStopSec=30s
KillMode=control-group
CPUQuota=200%
CPUWeight=25
MemoryHigh=3G
MemoryMax=4G
IOWeight=50
TasksMax=512
LimitNOFILE=65536
LimitCORE=0
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ProtectControlGroups=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectKernelLogs=true
RestrictSUIDSGID=true
LockPersonality=true
RestrictRealtime=true
SystemCallArchitectures=native
CapabilityBoundingSet=
AmbientCapabilities=
ReadWritePaths=/var/lib/postfiat-rpc/replica /var/log/postfiat-rpc /run/postfiat-rpc-public
UMask=0077

[Install]
WantedBy=multi-user.target
```

---

## 6. `< 250ms` Read-Path SLO Targets and Verification Contract

### 6.1 Architecture Requirements to Eliminate 7–10s Latency

As measured in [`fast-validator-reads-20261124.md`](fast-validator-reads-20261124.md), devnet 7–10s read stalls were caused by per-request child process spawning, exclusive `redb` file locks (`flock(LOCK_EX)`) contending with validator block commits, `fdatasync` on read close, and full-history scans. To satisfy the Pillar 3 `< 250ms` p95 SLA, the M3 read-replica profile MUST:

1. **Serve reads off the consensus writer lock:** Read-replica `rpc-serve` serves from non-blocking read snapshots or a resident read-only replica store, never taking the consensus writer's `redb` exclusive lock.
2. **Use keyed lookups for all history queries:** `blocks` (`blocks_window`), `receipts` (by `tx_id`), `tx` (by `tx_id`), and `pfusdc_egress_witness` MUST use keyed index reads rather than full-history scans.
3. **Height-stamped in-process response cache:** Every cacheable read method MUST embed its committed `height` in the response payload and only populate the bounded in-process cache when `committed_height_before == committed_height_after`.
4. **Byte-for-byte equivalence:** Differential tests MUST prove cached and resident/replica replies equal canonical direct store replies byte-for-byte at identical committed heights (`cmp`).

### 6.2 Frozen Read-Path SLO Targets

Measured at the loopback RPC boundary on reference validator/replica hardware under steady concurrent load (`50 req/s` mixed read workload while the chain commits blocks continuously):

| Method Class | Methods | p50 Target | **p95 Target (SLA)** | p99 Target | Consistency Rule |
| --- | --- | ---: | ---: | ---: | --- |
| **In-process status & readiness** | `status`, `server_info`, `metrics`, `private_readiness` | `<= 10 ms` | **`<= 25 ms`** | `<= 50 ms` | Committed tip height |
| **Account, registry & fee reads** | `account`, `account_lines`, `account_assets`, `validators`, `manifests`, `mempool_status`, `fee`, `transfer_fee_quote` | `<= 25 ms` | **`<= 100 ms`** | `<= 200 ms` | Height-stamped snapshot |
| **Keyed history reads** | `blocks` (`limit <= 8`), `receipts` (by `tx_id`), `tx` (by `tx_id`) | `<= 35 ms` | **`<= 150 ms`** | `<= 250 ms` | Immutable finalized height |
| **Bridge & route state reads** | `vault_bridge_route`, `vault_bridge_status`, `bridge_status` | `<= 50 ms` | **`<= 200 ms`** | `<= 350 ms` | Height-stamped snapshot |
| **Keyed egress witness & bounded scans** | `pfusdc_egress_witness` (keyed), `shield_turnstile`, `shield_disclose` | `<= 100 ms` | **`<= 250 ms`** | `<= 500 ms` | Isolated concurrency slot |
| **Global Standard Read SLA** | All standard read methods above combined | `<= 50 ms` | **`< 250 ms`** | `<= 500 ms` | Availability `>= 99.9%` |

### 6.3 Freshness and Zero-Consensus-Interference SLOs

| Dimension | Frozen Target | Enforcement |
| --- | --- | --- |
| **Replica freshness lag (p95)** | `<= 1 block` (`<= 2s` behind finalized tip) | Continuous follower sync from local validator/peers |
| **Max stale lag before unready** | `5` blocks | `private_readiness` and `ready.json` flip `ready = false`; TLS edge stops routing |
| **Consensus vote latency impact under 10x RPC flood** | `<= 5 ms` p99 delta, **`0` missed consensus rounds** | Separate UID + cgroup caps (`CPUWeight=25`, `IOWeight=50`) + no shared `redb` writer lock |
| **Bounded heavy-query isolation** | `0` child timeouts degrading `status` p95 above `25 ms` | `--max-child-dispatch-concurrent 8`, `--max-child-dispatch-per-peer 4`, `--child-timeout-ms 15000` |

### 6.4 M3 Release Gate Verification

Before a release advertises public RPC compliance, CI / rehearsal runs MUST record:

1. **Differential equivalence (`cmp`)** comparing read-replica responses against direct store reads across tip, base, out-of-range, and malformed inputs;
2. **Concurrent read benchmark** (`50 req/s` mixed workload for `>= 5 minutes` during active block production) demonstrating global p95 `< 250 ms` and every method class within Table 6.2;
3. **10x flood anti-DoS drill** verifying `0` missed consensus rounds and `<= 5 ms` p99 consensus vote latency delta on the isolated validator.
