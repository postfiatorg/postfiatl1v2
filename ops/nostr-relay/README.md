# First-Party Nostr Relay (`relay.postfiat.org`) Packaging

Deployment packaging, write-admission policy plugin, mirror topology, and
automated health/rate-limit verification harness for Post Fiat's first-party
Hive Chat Nostr relay (`relay.postfiat.org`, Pillar 6).

## Architecture

- **Edge (`Caddyfile`):** Terminates TLS for `relay.postfiat.org`, serves
  `/healthz`, enforces HSTS and a 256 KiB request-body cap, and proxies
  WebSocket (`wss://`) plus NIP-11 (`Accept: application/nostr+json`) traffic to
  the internal `strfry` process.
- **Relay (`strfry.conf`):** Lightweight C++ Nostr relay bound internally to
  port `7777` with strict event/filter/subscription limits and a mandatory
  write-admission plugin (`write-policy.py`).
- **Write-Admission Policy (`write-policy.py`):**
  - Enforces NIP-42 authentication and registered Post Fiat member allowlist
    (synced from on-chain `identity_set` / Task Node member snapshot).
  - Enforces the frozen Contract C3 Nostr event kind allowlist
    (`[0, 1, 13, 1059, 10050]`; ephemeral `22242` auth events are never stored;
    unlisted kinds such as `30078`, `24133`, `27235` are rejected).
  - Blocks `kind: 0` profile publishing in all non-production environments
    (`NOSTR_RELAY_ENV != "production"`).
  - Enforces per-pubkey sliding-window rate limits (`rate-limited:`).
  - Filters inbound mirrored events (`sourceType` `Stream`/`Sync`/`Import`) so
    public-relay spam floods cannot enter `relay.postfiat.org`.
- **Public Relay Mirroring (`mirror-sync.json`):** Mirrors registered-member
  Hive events with fallback public relays (`wss://relay.primal.net`,
  `wss://nos.lol`, `wss://relay.damus.io`) while filtering inbound spam and
  blocking outbound `kind: 0` outside production.

## Quick Start

```bash
# Run automated offline verification harness (HTTP /healthz, NIP-11, NIP-42 AUTH,
# registered-member admission, kind allowlist, mirror filter, and rate limiting):
python3 ops/nostr-relay/verify_relay.py --self-test

# Run unit & deployment-config test suite:
python3 -m unittest discover -s ops/nostr-relay

# Verify a live deployment:
python3 ops/nostr-relay/verify_relay.py \
  --http-url https://relay.postfiat.org \
  --ws-url wss://relay.postfiat.org
```
