# Packaged RPC profiles

There is one binary. A read replica and the HTTP edge are `postfiat-node` in another mode, installed from the same signed release as a validator.

The contract is [`docs/specs/validator-rpc-profiles-v1.md`](../../docs/specs/validator-rpc-profiles-v1.md).

## The pieces

| Piece | Unit | What it does |
| --- | --- | --- |
| Read replica | `postfiat-rpc-private.service` (port 27650) or `postfiat-rpc-public.service` (port 27651) | `rpc-serve --rpc-profile ...-replica` on loopback. Serves reads from its own copy of the chain. Holds no key and refuses to start in a data directory that has one. Serves exactly the `read_only_public` methods of the [method inventory](../../docs/runbooks/rpc-method-inventory.json). |
| Catch-up | `postfiat-rpc-catchup@.service` + `.timer` | Every few seconds, `rpc-catch-up --follow` pulls new blocks from the source RPC, applies each only with its certificate, checks the tip and state root after every block, and records the source's height. A replica more than 5 blocks behind, or one whose catch-up has stopped, refuses reads with `replica_lagging`. The managed follower runs a full audit at startup without an audit stamp and every 24 hours thereafter. A failed catch-up invalidates its readiness record. Without `--follow` the command re-verifies the whole state on every run, which takes minutes. |
| HTTP edge | `postfiat-rpc-http-edge.service` (port 8650) | `rpc-serve` speaks one JSON request per line over TCP. The edge takes `POST /rpc` and `GET /healthz` on loopback and forwards to it, with the same method list and a request budget per client address. |
| TLS proxy | `postfiat-rpc-public-edge.service` + `Caddyfile.public-rpc` | TLS 1.3 on 443, forwards `/rpc` and `/healthz` to the HTTP edge, stops routing when `/healthz` is not 200. |
| Validator priority | `consensus-cgroup-override.conf` | Drop-in for the validator unit on a host that also runs a replica: the validator gets CPU and I/O first. |

A replica orders nothing. Clients submit transactions to a validator.

## Two ways to serve the public

1. **Reads only:** TLS proxy -> HTTP edge -> public replica. Use this for explorers, wallets reading balances, and observers.
2. **Reads and private-payment admission:** TLS proxy -> HTTP edge with `--allow-private-action-admission` -> the validator's own `rpc-serve` (which runs with the same switch). Admission needs the validator's key, so a replica cannot serve it. The edge still refuses every transaction-submitting and operator method.

## Install a public read replica

```bash
sudo useradd --system --home /nonexistent --shell /usr/sbin/nologin postfiat-rpc
sudo useradd --system --home /nonexistent --shell /usr/sbin/nologin postfiat-edge
sudo install -d -m 0700 -o postfiat-rpc -g postfiat-rpc /var/lib/postfiat-rpc/replica /var/log/postfiat-rpc
sudo install -d -m 0700 -o postfiat-edge -g postfiat-edge /var/lib/postfiat-edge /var/log/postfiat-edge

# The replica's own copy of the chain starts from a signed snapshot of the network.
sudo -u postfiat-rpc postfiat-node snapshot-import-signed --data-dir /var/lib/postfiat-rpc/replica \
  --snapshot-dir /srv/postfiat/snapshot --trusted-publisher-key-file /etc/postfiat/snapshot-publisher.public.json \
  --node-id rpc-replica-1

sudo install -m 0644 postfiat-rpc-public.service postfiat-rpc-catchup@.service postfiat-rpc-catchup@.timer \
  postfiat-rpc-http-edge.service postfiat-rpc-public-edge.service /etc/systemd/system/
sudo install -m 0640 -o root -g postfiat-rpc rpc-replica.env.example /etc/postfiat/rpc-replica.env
sudo install -m 0640 -o root -g postfiat-edge rpc-http-edge.env.example /etc/postfiat/rpc-http-edge.env
sudo install -m 0640 -o root -g postfiat-edge rpc-public.env.example /etc/postfiat/rpc-public.env
sudo install -m 0644 Caddyfile.public-rpc /etc/postfiat/Caddyfile.public-rpc
sudo systemctl daemon-reload
sudo systemctl enable --now postfiat-rpc-catchup@public.timer postfiat-rpc-public.service \
  postfiat-rpc-http-edge.service postfiat-rpc-public-edge.service

postfiat-node rpc-probe --host 127.0.0.1 --port 27651 --timeout-ms 5000
curl -s http://127.0.0.1:8650/healthz
curl -s https://$POSTFIAT_PUBLIC_RPC_DOMAIN/rpc -d '{"version":"postfiat-local-rpc-v1","id":"1","method":"status","params":{}}'
```

On `postfiat-wan-devnet-2` the snapshot import does not work at the moment: the block history file of a snapshot is 369 MB and the import accepts 256 MB. Until that is fixed, a Foundation replica there is seeded from a copy of a validator's store without its key files. See the measured read numbers in the [contract](../../docs/specs/validator-rpc-profiles-v1.md).

The replica unit checks before every start that the installed binary is the one a 2-of-3 signed release manifest names (`release-manifest-verify`). It needs `/etc/postfiat/release-manifest.json` and `/etc/postfiat/release-keyset.json` from the release.

Set the same randomly generated `POSTFIAT_RPC_EDGE_PROXY_TOKEN` in the TLS proxy and HTTP edge environment files, readable only by the administrators of those services. The edge charges the socket peer unless the proxy supplied this credential. Reload both services when rotating it.
