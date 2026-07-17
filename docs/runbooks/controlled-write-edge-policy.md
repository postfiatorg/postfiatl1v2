# Controlled Write Edge Policy

Status: controlled-testnet policy, 2026-05-14.

This runbook defines the safe write-admission boundary for the controlled
testnet. Validator RPC services remain read-only by default. Signed transfer
submission is admitted only through a bounded controlled write edge.

## Current State

The live wallet-finality proof used a temporary SSH-local write edge:

```text
reports/testnet-live-wallet-finality/current-rerun3-20260514T161147Z/testnet-live-wallet-finality.json
```

That edge was not publicly exposed, accepted one
`mempool_submit_signed_transfer` request, used bounded per-peer and total submit
limits, and then exited. It is valid finality evidence, not the permanent public
write service.

The release package validator RPC units are intentionally read-only. They must
not include `--allow-mempool-submit`.

## Controlled-Testnet Target

The persistent controlled write edge is a separate role from validator read RPC.
It may run on a validator host or an operator-controlled edge host, but it must
be treated as an ingress service with stricter exposure controls than read RPC.

Minimum service shape:

```bash
postfiat-node rpc-serve \
  --data-dir /var/lib/postfiat/<write-edge-source> \
  --bind-host 127.0.0.1 \
  --port <write_edge_port> \
  --max-requests ${POSTFIAT_WRITE_EDGE_MAX_REQUESTS} \
  --timeout-ms ${POSTFIAT_TIMEOUT_MS} \
  --child-timeout-ms ${POSTFIAT_RPC_CHILD_TIMEOUT_MS} \
  --event-log /var/log/postfiat/write-edge/rpc-events.ndjson \
  --allow-mempool-submit \
  --max-mempool-submit-per-peer ${POSTFIAT_WRITE_EDGE_MAX_SUBMIT_PER_PEER} \
  --max-mempool-submit-total ${POSTFIAT_WRITE_EDGE_MAX_SUBMIT_TOTAL}
```

For public Internet exposure, bind the node process to `127.0.0.1` and expose
it through an operator-managed proxy or load balancer that enforces TLS,
request-size limits, source-rate limits, access logging, and firewall policy.
Do not expose validator transport ports as part of write-edge access.

## Admission Rules

Allowed write method:

- `mempool_submit_signed_transfer`

Disallowed on the controlled write edge:

- unsigned transfer submission;
- direct `apply_batch`;
- shielded/bridge apply methods;
- governance mutation methods;
- any request containing private key material.

The node must preserve these controls:

- `--allow-mempool-submit` is present only on the controlled write-edge service.
- `--max-mempool-submit-per-peer` is positive and intentionally bounded.
- `--max-mempool-submit-total` is positive and intentionally bounded.
- `--max-requests` is finite, with systemd or the process supervisor rotating
  the service before exhaustion.
- `postfiat-rpc-serve-v1` reports `invalid_signature_count`,
  `mempool_submit_rate_limited_count`, and
  `mempool_submit_global_rate_limited_count`.
- A valid signed submit after invalid-signature pressure is accepted in local
  smoke evidence before the edge is advertised.

## Verification

Run the policy audit after release-package changes and before operator handoff:

```bash
scripts/testnet-controlled-write-edge-policy-audit
```

The audit verifies that packaged validator RPC units remain read-only, the live
wallet-finality write proof was bounded and SSH-local, the write-edge pressure
smoke passed, and this runbook contains the required service boundary.

Run the pressure smoke after behavior-changing RPC edits:

```bash
scripts/testnet-rpc-write-edge-load-smoke
```

## Claim Boundary

Safe language:

> PostFiat has a bounded controlled write-edge policy and evidence for signed
> transfer admission under pressure. Validator RPC remains read-only by default.

Do not claim unrestricted public write RPC, production API authentication, or
public gateway availability until the persistent edge is deployed behind the
operator-managed exposure controls above and has live load evidence.
