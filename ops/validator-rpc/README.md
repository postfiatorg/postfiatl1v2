# Packaged Read-Replica RPC Profiles (`postfiat-node`)

Single binary invariant: there is **no separate RPC codebase**. Private and public RPC nodes run the exact same `postfiat-node` binary in non-voting read-replica mode (`--rpc-profile private-replica` or `--rpc-profile public-replica`), installed and upgraded from the exact same 2-of-3 signed `postfiat.release_manifest.v3` release channel as validators.

## Supported Topologies (Consensus Anti-DoS Isolation)

1. **Topology A — Same-host cgroup-isolated read-replica:**
   - Validator consensus runs as UID `postfiat` with elevated cgroup shares (`consensus-cgroup-override.conf`: `CPUWeight=200`, `IOWeight=500`).
   - Read-replica RPC runs as distinct unprivileged UID `postfiat-rpc` (`postfiat-rpc-private.service` or `postfiat-rpc-public.service`) against its own synced replica store (`/var/lib/postfiat-rpc/replica`), capped to `CPUWeight=25..50`, `IOWeight=50..100`, `MemoryMax=3G..4G`, `TasksMax=512`.
   - Public TLS 1.3 terminates at a third unprivileged UID (`postfiat-edge`) running `postfiat-rpc-public-edge.service` (`Caddyfile.public-rpc`).
2. **Topology B — Dedicated non-voting follower host:**
   - Runs `postfiat-rpc-public.service` (or `docker-compose.rpc-public.yml`) on a standalone host synced from release-pinned finalized checkpoints via `postfiat-rpc-catchup.timer` (`postfiat-node rpc-catch-up-certified-delta`).

## Non-Voting Follower Invariants (Enforced Fail-Closed at Startup)

- `node_role` reports `"rpc_follower"` in `status` and `server_info`.
- `voting_enabled` is `false`; `validator_signer` is `"none"`.
- Passing `--finality-key-file`, `--finality-proposal-key-file`, or `--finality-signer-socket` fails closed (`rpc_replica_signer_forbidden`).
- Plaintext RPC binds strictly to loopback (`127.0.0.1` / `::1`); public or wildcard binds fail closed (`rpc_replica_bind_forbidden`).
- Standard read-only RPCs (`status`, `server_info`, `metrics`, `account`, `account_lines`, `account_assets`, `validators`, `manifests`, `fee`, `blocks`, `receipts`, `tx`, `vault_bridge_route`, `vault_bridge_status`, `bridge_status`, `ledger`) run in-process under a shared `Arc<TransactionalStore>` lease + bounded height-stamped response cache (`< 250ms` p95 SLA).

## Files

| File | Purpose |
| --- | --- |
| `postfiat-rpc-private.service` | Private loopback read-replica systemd unit (`127.0.0.1:27650`) |
| `postfiat-rpc-public.service` | Public read-replica systemd unit (`127.0.0.1:27651`, behind TLS edge) |
| `postfiat-rpc-public-edge.service` | Dedicated TLS 1.3 reverse proxy systemd unit (`postfiat-edge`) |
| `Caddyfile.public-rpc` | TLS 1.3 edge config (`MAX_RPC_REQUEST_BYTES=65536`, zero body rewrite) |
| `postfiat-rpc-catchup.service` / `.timer` | Continuous non-voting certified-delta follower sync |
| `consensus-cgroup-override.conf` | Consensus priority override (`CPUWeight=200`, `IOWeight=500`) |
| `docker-compose.rpc-private.yml` | Private loopback Docker Compose profile |
| `docker-compose.rpc-public.yml` | Public TLS read-replica Docker Compose profile |
| `rpc-private.env.example` / `rpc-public.env.example` | Typed environment templates |

## Quick Install (Systemd)

```bash
# 1. Create dedicated unprivileged users
sudo useradd --system --home /nonexistent --shell /usr/sbin/nologin postfiat-rpc
sudo useradd --system --home /nonexistent --shell /usr/sbin/nologin postfiat-edge

# 2. Prepare replica directories
sudo install -d -m 0700 -o postfiat-rpc -g postfiat-rpc /var/lib/postfiat-rpc/replica /var/log/postfiat-rpc

# 3. Install unit + environment file (private or public)
sudo install -m 0644 ops/validator-rpc/postfiat-rpc-public.service /etc/systemd/system/
sudo install -m 0644 ops/validator-rpc/postfiat-rpc-catchup.service /etc/systemd/system/
sudo install -m 0644 ops/validator-rpc/postfiat-rpc-catchup.timer /etc/systemd/system/
sudo install -m 0640 -o root -g postfiat-rpc ops/validator-rpc/rpc-public.env.example /etc/postfiat/rpc-public.env
sudo systemctl daemon-reload
sudo systemctl enable --now postfiat-rpc-public.service postfiat-rpc-catchup.timer

# 4. Probe loopback RPC health and inspect SSH dashboard
postfiat-node rpc-probe --host 127.0.0.1 --port 27651 --timeout-ms 5000
postfiat-node top --rpc-host 127.0.0.1 --rpc-port 27651 --once
```
