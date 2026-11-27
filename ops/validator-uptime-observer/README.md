# Validator uptime observer

Three independent observers watch the chain and each signs what it saw. Anybody can bundle the three signed attestations into the feed that Task Node, `postfiat-node top` and the validator directory read. The contract is [`docs/specs/validator-uptime-feed-v1.md`](../../docs/specs/validator-uptime-feed-v1.md).

An observer only reads. It holds its own signing key and nothing else: no validator key, no governance key. It must not run inside a validator's data directory and refuses to.

## What an observer needs

- `postfiat-node` from a signed release (it signs and verifies).
- `scripts/validator-uptime-observer.py` and `scripts/validator_uptime_contract.py`, installed together in `/usr/local/lib/postfiat/`. Python 3.11 or newer, no packages.
- Read access to a trusted RPC on the chain. Certificate votes determine the signed score. Other validator RPCs can provide diagnostic late-vote records.

## Set up one observer

```bash
sudo useradd --system --home /nonexistent --shell /usr/sbin/nologin postfiat-observer
sudo install -d -m 0750 -o postfiat-observer -g postfiat-observer /var/lib/postfiat-observer
sudo install -d -m 0755 -o postfiat-observer -g postfiat-observer /var/lib/postfiat-observer/public
sudo install -d /usr/local/lib/postfiat
sudo install -m 0755 scripts/validator-uptime-observer.py /usr/local/lib/postfiat/
sudo install -m 0644 scripts/validator_uptime_contract.py /usr/local/lib/postfiat/

# The observer's own key. The private half stays in this file, mode 0600.
sudo postfiat-node observer-key-create --key-file /etc/postfiat/uptime-observer.key.json \
  --observer-id observer-us-east-1 --region us-east --operator-group my-organisation
sudo chown postfiat-observer: /etc/postfiat/uptime-observer.key.json

sudo install -m 0644 ops/validator-uptime-observer/postfiat-uptime-observer.service \
  ops/validator-uptime-observer/postfiat-uptime-observer.timer /etc/systemd/system/
sudo install -m 0640 -o root -g postfiat-observer ops/validator-uptime-observer/uptime-observer.env.example /etc/postfiat/uptime-observer.env
sudo systemctl daemon-reload && sudo systemctl enable --now postfiat-uptime-observer.timer
```

`observer-key-create` prints the public entry (`observer_id`, `region`, `operator_group`, `public_key_hex`). The three entries, with the chain, make the keyset file every consumer pins ([schema](../../docs/specs/schemas/postfiat-observer-keyset-v1.schema.json)). The three observers have three different regions and three different operator groups.

The observer writes `/var/lib/postfiat-observer/public/attestation.json` every run. Serve that one file over HTTPS, or copy it to whoever bundles.

## What costs an observer a day

An observer dates blocks by when it saw them. If two reads are more than 15 minutes apart, or the head moved between two reads that straddle UTC midnight, it cannot date those blocks and marks the day incomplete: that day does not count for or against any validator in this observer's attestation. The other two observers still carry the feed. The timer runs every 5 minutes for that reason.

It stops, and writes nothing, when the source is on another chain, behind what it already saw, or shows a block that does not follow the last one it saw.

## Bundle the feed

On any host, with the pinned keyset at `/etc/postfiat/observer-keyset.json`:

```bash
sudo install -m 0644 ops/validator-uptime-observer/postfiat-uptime-bundle.service \
  ops/validator-uptime-observer/postfiat-uptime-bundle.timer /etc/systemd/system/
sudo install -m 0640 -o root -g postfiat-observer ops/validator-uptime-observer/uptime-bundle.env.example /etc/postfiat/uptime-bundle.env
sudo systemctl daemon-reload && sudo systemctl enable --now postfiat-uptime-bundle.timer
```

It writes `/var/lib/postfiat-observer/public/validator-uptime-feed.json`. Bundling needs no key and adds no trust: a consumer verifies the three signatures itself and rebuilds every row.

## Read a feed

```bash
postfiat-node observer-uptime-verify --feed-file validator-uptime-feed.json --keyset-file /etc/postfiat/observer-keyset.json
```

It prints who signed, whose attestation was dropped and why, and the rows. With fewer than two attestations that count, `indeterminate` is true: new bindings and new credit wait, and nobody is revoked.

For a cold start with known lifecycle history, supply both `--bootstrap-feed` and `--bootstrap-keyset`. The latter must be pinned independently. Without a verified checkpoint, activation dates remain unknown and the observer grants no ramp-in.
