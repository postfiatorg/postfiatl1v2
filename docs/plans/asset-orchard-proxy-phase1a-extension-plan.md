# Asset-Orchard Proxy Phase 1a Extension Plan

Status: plan only, not implemented
Date: 2026-07-03

## Headline

Prover latency is solved for the wallet-local path; the residual browser click-to-receipt latency is proxy-subprocess plus cross-continent transport.

## Evidence

Source table:

- `docs/evidence/wallet-private-swap-zk-latency-click-receipt-20260703T0548Z/zk-latency-before-after-table.json`
- `docs/evidence/wallet-private-swap-zk-latency-click-receipt-20260703T0548Z/phase0-click-receipt-report.json`

Accepted warm browser click-to-receipt:

- Total: `681.288s`
- Wallet-local resident service action: `6.064s`
- Proxy certified receipt path: `675.224s`

Proxy residual decomposition from `wallet-proxy/server.js` submit timings:

- `shield-batch-swap` subprocess: `327.683s`
- `transport-peer-certified-batch-round`: `347.513s`
- Proxy total: `675.201s`

## Classification

1. Proxy `shield-batch-swap` subprocess: FIXABLE, Phase 1a-extended.
   - This is a second proof/batch path launched as a fresh `postfiat-node` process by the proxy submit handler.
   - It bypasses the warmed resident local service and therefore repeats expensive cold process/prover setup.
   - Fix class: route proxy submit through a resident warmed service/API or keep a long-lived worker process with warmed keys.

2. Certified transport round: TRANSPORT-BOUND.
   - This is cross-continent consensus transport for a multi-MB Halo2 batch.
   - It is not fixed by more prover key prewarm.
   - Fix class: transport keep-alive, persistent peer channels, batch-size work, and production fleet geography.

## Scope

Primary files:

- `wallet-proxy/server.js`
- `crates/node/src/bin/asset_orchard_local_service.rs`
- `crates/node/src/main_parts/cli_dispatch_parts/group_05.rs`
- `crates/node/src/main_parts/runtime_helpers.rs`
- `scripts/wallet-shielded-swap-step7-e2e.mjs`
- `scripts/zk-latency-phase0-report.mjs`

Likely tests:

- `wallet-proxy/test_navswap_adapter.js`
- `scripts/zk-latency-phase0-report.test.mjs`
- Rust tests under `crates/node/src/bin/asset_orchard_local_service.rs` if the service gains submit/batch endpoints.

## Proposed Fix

### Option A - Resident Service Batch Endpoint

Add an Asset-Orchard local-service endpoint that accepts the public swap action JSON and produces the shielded batch JSON using the same resident warmed process:

- `POST /asset-orchard/swap-batch`
- input: public `swap_action_json`, no note private material
- output: batch JSON or batch file material suitable for certified transport
- invariant: no wallet seed, note opening, spend authority, or backup JSON crosses to the proxy

Then change `wallet-proxy/server.js::executeShieldedNavswapSwap`:

- replace `execFile(postfiat-node, ["shield-batch-swap", ...])`
- call the resident service batch endpoint
- write the returned batch to the existing artifact root
- keep the certified transport call unchanged for this slice

Expected impact:

- removes the `327.683s` warm proxy batch subprocess residual
- leaves the `347.513s` certified transport residual

### Option B - Long-Lived Proxy Worker

Run a persistent worker owned by the proxy that prewarms the same batch path and receives batch requests over local IPC.

Expected impact:

- similar performance goal as Option A
- more process supervision and operational complexity than Option A

Recommendation: Option A first. It reuses the existing resident service, keeps wallet-private material local, and has a smaller operational surface.

## Acceptance Criteria

- [ ] Browser e2e warm submit shows local action still near seconds.
- [ ] Proxy batch portion drops from about `327s` to near resident-service warm batch time.
- [ ] Certified transport remains separately reported and not mislabeled as proof.
- [ ] `proof` timing remains absent from wall-clock reports unless directly scoped to Halo2 proof generation.
- [ ] Request privacy scan proves no wallet private material is sent to proxy or remote endpoints.
- [ ] Additive report invariant remains green.
- [ ] No `can_run` promotion until the next live gate accepts the new measurement.

## Non-Goals

- Do not claim the certified transport round is prover-fixable.
- Do not co-locate or reconfigure the devnet fleet in this slice.
- Do not change consensus semantics or receipt validation.
- Do not batch or hide the transport time inside a `proof` bucket.

## Hold Point

This plan is for review only. Implementation waits for the next directive.
