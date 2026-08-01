# Generic NAVCoin Ethereum Export and Return Relay

**Date:** 2026-08-01
**Status:** implemented and locally qualified; A666 production cutover requires
the governed successor-profile migration and a separate deployment approval

## Purpose

This is the public operational boundary for moving a registered PFTL NAVCoin
to its governed Ethereum wrapper and back. It replaces per-asset relay code
and any direct dependency on an internal portfolio or signing product.

```text
finalized PFTL route + user-signed source operation
  -> durable route-scoped relay job
  -> proof construction and local verification
  -> standalone constrained signer
  -> governed Ethereum wrapper/controller
  -> finalized destination acknowledgement
```

The relay never calculates NAV, fetches brokerage positions, holds a user's
PFTL spending key, or decides which proof profile is valid. Those facts and
permissions come from finalized PFTL state and the reviewed route deployment.

## Components

| Component | Responsibility |
|---|---|
| `wallet-proxy/navcoin-export-jobs.js` | Durable, idempotent export supervision for up to 64 configured routes. |
| `wallet-proxy/navcoin-return-jobs.js` | Durable, idempotent Ethereum-return supervision for up to 64 configured routes. |
| `wallet-proxy/navcoin-export-relay-driver.js` | Route-pinned source inspection, proof generation, Ethereum mint, and PFTL acknowledgement. |
| `wallet-proxy/navcoin-return-relay-driver.js` | Route-pinned burn inspection, proof generation, and PFTL native-asset restoration. |
| `tools/postfiat-signer` | Open constrained Ethereum signer with chain, contract, selector, value, fee, route, and idempotency policy. |
| `/api/navcoin/<route-id>/...` | Route-scoped readiness, submission, and status API used by the browser. |

Route IDs are case-sensitive and must match `^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$`.
This includes the deployed `pftl-a666-ethereum-wA666-usdc-v1` identity. A
request for an unconfigured or differently-cased route fails closed.

## Configuration

Configure one JSON service file per route and point the proxy at either one
file or a comma-separated bounded list:

```text
NAVCOIN_EXPORT_RELAY_CONFIG_FILE=/path/export-service.json
NAVCOIN_EXPORT_RELAY_CONFIG_FILES=/path/a.json,/path/b.json
NAVCOIN_RETURN_RELAY_CONFIG_FILE=/path/return-service.json
NAVCOIN_RETURN_RELAY_CONFIG_FILES=/path/a.json,/path/b.json
```

At most 64 routes may be configured in each direction. Duplicate route IDs
are rejected. Each service config pins the exact route, native asset,
route/deployment digest, driver path and hash, state root, timeout, and retry
policy. Each driver config pins the expected chain, controller, token,
verifier, proof vkey, asset metadata, data directory, and constrained-signer
endpoint. A hash or identity mismatch blocks readiness before source value is
moved.

The checked A666 examples are under
`deployments/a666-export-relay-mainnet-20260731/`. They are deployment records,
not a universal default and not authorization to modify the live route.

## HTTP lifecycle

For route `<route>`:

```text
GET  /api/navcoin/<route>/export-readiness
POST /api/navcoin/<route>/export-jobs
GET  /api/navcoin/<route>/export-jobs/<job-id>
GET  /api/navcoin/<route>/return-readiness
POST /api/navcoin/<route>/return-jobs
GET  /api/navcoin/<route>/return-jobs/<job-id>
```

Job identity commits to the exact case-preserving route ID and source packet
or Ethereum transaction. A retry returns the same job. A request cannot query
or resume another route's job. Atomic state files allow restart recovery; do
not edit or delete a nonterminal job directory.

## New NAVCoin onboarding

An operator does not fork the wallet or relay code. The operator must:

1. register the NAVCoin, provider-neutral proof profile, and source manifest;
2. finalize a valid reserve packet and governed primary-market policy;
3. deploy and verify the wrapper, controller, verifier, and return contracts;
4. register the exact PFTL/Ethereum route and deployment digest;
5. create route-specific export/return configs and a least-privilege signer
   policy;
6. confirm the browser discovers the route from finalized PFTL RPC state;
7. run issue, redeem, export, return, replay, restart, and conservation gates;
8. publish the immutable identities and operational trust classification; and
9. activate value only through the asset's governed rollout procedure.

No step requires StakeHub. A private source collector may hold brokerage
credentials, but it emits the public bounded adapter artifact consumed by the
open reserve-proof kit and has no authority over consensus semantics.

## Qualification and production boundary

Automated tests cover malformed config, duplicate routes, wrong-route access,
job idempotency, concurrent distinct routes, worker failure/retry, restart,
and the case-sensitive live A666 route. The controlled qNAV lifecycle covers
the provider-neutral proof through issue/redeem/export/return consensus logic.

Production activation additionally requires audited contracts, a reviewed
signer policy, genuine proof infrastructure, chain finality, deployed-route
readback, fleet convergence, operational monitoring, and the asset's explicit
governance action. The Monday A666 demonstration remains pinned to its frozen
qualified release; this runbook does not authorize upgrading that fleet.
