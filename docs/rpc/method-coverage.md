# RPC Dispatch Method Coverage

This page completes the hosted documentation for every method arm in
`crates/node/src/rpc_dispatch.rs`. The primary [RPC Methods](methods.md) page
covers the public wallet and protocol surface; this page covers the remaining
read, gated, operator, compatibility, and local request-file methods.

Posture names come from the code-derived
[RPC Method Inventory](../runbooks/rpc-method-inventory.md):

- **public read**: remotely enabled read-only method;
- **privacy-alpha gated**: remotely unavailable unless its exact privacy gate is
  enabled;
- **operator/local only**: dispatched by the local node command or request-file
  path, but absent from the remote RPC allowlist.

A method being listed here does not authorize exposing it remotely.

## Local keys, transfer construction, and state application

| Method | Posture | Current dispatch behavior |
| --- | --- | --- |
| `faucet` | operator/local only | Returns the complete local development-key record, including private key material. Keep this method off every remote RPC edge. |
| `validate_local_keys` | operator/local only | Validates the local validator key set and its local-only constraint. |
| `owned-sign` | operator/local only | Hyphenated local compatibility alias for `owned_sign`; loads the selected validator signing key and returns a vote. |
| `owned-unwrap-sign` | operator/local only | Hyphenated local compatibility alias for `owned_unwrap_sign`; loads the selected validator signing key and returns an unwrap vote. |
| `owned-unwrap-apply` | operator/local only | Hyphenated local compatibility alias for `owned_unwrap_apply`; verifies and applies a supplied unwrap certificate. |
| `transfer` | operator/local only | With direct-state mode enabled, signs a transparent transfer with a local key file and applies it directly. |
| `batch_transfer` | operator/local only | Builds a signed transparent-transfer batch file; it does not apply the batch. |
| `mempool_submit_transfer` | operator/local only | Signs with a local wallet key and admits a transparent transfer to the local mempool. |
| `mempool_batch` | operator/local only | Builds a proposer batch from admitted mempool transactions. |
| `apply_batch` | operator/local only | Applies a prepared transparent batch directly and returns its receipts. |

## Market, reserve, and bridge reads

| Method | Posture | Current dispatch behavior |
| --- | --- | --- |
| `nav_reserve_proof_status` | public read | Returns the provider-neutral reserve-proof/profile status for an asset. |
| `asset_orchard_action_status` | public read | Looks up finalized Asset-Orchard action elements by both nullifiers and both output commitments. |
| `fx_fix_list` | public read | Returns a bounded fixed-rate FX market directory, optionally filtered by asset pair and active state. |
| `fx_fix_info` | public read | Returns fixed-rate FX market detail for an exact fix-packet hash. |
| `fx_fix_reservation_info` | public read | Returns reservation status for an exact FX reservation ID. |
| `fx_fix_quote` | public read | Computes a deterministic quote for an exact fix-packet hash and base-atom amount. |
| `market_ops_status` | public read | Returns NAV market-operation state for an asset and optional epoch. |
| `vault_bridge_route` | public read | Verifies and returns the governed vault-bridge route for an asset. |
| `vault_bridge_status` | public read | Returns the vault-bridge operational and accounting status for an asset. |
| `pfusdc_ingress_preflight` | public read | Simulates an exact pfUSDC ingress claim against current route, supply-cap, and Orchard policy without mutating state. |
| `pfusdc_egress_witness` | public read | Exports a bounded proof-ready pfUSDC egress witness for a withdrawal and optional prior checkpoint. |
| `pfusdc_checkpoint_witness` | operator/local only | Exports a bounded checkpoint witness between exact prior and target block IDs; it is not remotely allowlisted. |
| `pftl_uniswap_receipt_witness` | operator/local only | Exports a bounded PFTL-Uniswap receipt witness for an exact packet hash and prior checkpoint; it is not remotely allowlisted. |

## Archive and local verification reads

| Method | Posture | Current dispatch behavior |
| --- | --- | --- |
| `batch_archive` | public read | Queries bounded entries from the local batch-archive index; it does not write an external archive. |
| `archive_window` | public read | Builds a bounded history handoff bundle for an inclusive height window; it does not publish the bundle. |
| `verify_blocks` | public read | Re-verifies the local block-log chain and signatures. |
| `verify_state` | public read | Re-verifies the aggregate local state, including its block-log integrity report. |
| `verify_bridge` | public read | Re-verifies local bridge-state invariants. |
| `verify_mempool` | public read | Re-verifies admitted mempool entries against local policy. |
| `verify_shielded` | public read | Re-verifies the local shielded-state commitment and accounting. |

## Shielded and Asset-Orchard methods

The legacy cleartext mint and spend builders fail closed. Orchard batch creation
uses its separate privacy-alpha remote gate; local application remains an
operator action.

| Method | Posture | Current dispatch behavior |
| --- | --- | --- |
| `shield_mint` | operator/local only | Disabled legacy cleartext mint endpoint; returns permission denied and directs callers to Asset-Orchard ingress. |
| `shield_spend` | operator/local only | Disabled legacy cleartext spend endpoint; returns permission denied and directs callers to an Asset-Orchard action. |
| `shield_batch_mint` | operator/local only | Disabled legacy cleartext mint-batch builder; returns permission denied. |
| `shield_batch_spend` | operator/local only | Disabled legacy cleartext spend-batch builder; returns permission denied. |
| `shield_batch_migrate` | operator/local only | Builds a local legacy-pool migration batch file for later application. |
| `shield_scan` | public read | Reads notes for an owner from the legacy shielded state; this is not an Orchard privacy scan. |
| `shield_disclose` | public read | Reads a disclosure for a note in the legacy shielded state; this is not an Orchard disclosure path. |
| `shield_batch_orchard` | privacy-alpha gated | Verifies a serialized Orchard action and builds a local action-batch file. |
| `shield_batch_orchard_deposit` | privacy-alpha gated | Verifies an Orchard deposit action and builds a local deposit-batch file. |
| `shield_batch_orchard_withdraw` | privacy-alpha gated | Verifies an Orchard action plus withdrawal terms and builds a local withdrawal-batch file. |
| `shield_batch_swap` | privacy-alpha gated | Verifies a shielded-swap action and builds a local swap-batch file. |
| `shield_batch_asset_orchard_ingress` | operator/local only | Verifies an Asset-Orchard ingress action and builds a local ingress-batch file; it is not remotely allowlisted. |
| `asset_orchard_swap_create` | operator/local only | Builds and verifies an Asset-Orchard swap action from two input-note files and explicit output artifacts. |
| `apply_shield_batch` | operator/local only | Applies a prepared shielded-action batch directly and returns its receipts. |

## NAVCoin bridge mutation methods

These methods are local/operator dispatch arms. They are not part of the remote
read surface.

| Method | Posture | Current dispatch behavior |
| --- | --- | --- |
| `navcoin_bridge_route_init` | operator/local only | Initializes a route from a typed route-configuration file. |
| `navcoin_bridge_launch_config_template` | operator/local only | Produces a launch-configuration template for an existing route. |
| `navcoin_bridge_launch_config_init` | operator/local only | Validates and initializes an exact launch configuration. |
| `navcoin_bridge_record_fork_rehearsal` | operator/local only | Records a route fork-rehearsal result from typed evidence. |
| `navcoin_bridge_packet_preflight` | operator/local only | Preflights a typed bridge packet against current route and launch policy without applying it. |
| `navcoin_bridge_primary_subscribe` | operator/local only | Applies a typed primary-subscription transition. |
| `navcoin_bridge_export_debit` | operator/local only | Applies a typed source-ledger export debit. |
| `navcoin_bridge_destination_consume` | operator/local only | Records typed destination consumption for an export packet. |
| `navcoin_bridge_refund_source` | operator/local only | Applies a typed source refund after validating the route transition. |
| `navcoin_bridge_record_return_burn` | operator/local only | Records a typed destination return-burn transition. |
| `navcoin_bridge_import_return` | operator/local only | Applies a typed return import on the source ledger. |

## Legacy bridge simulation methods

These are local simulation and batch-construction arms, separate from the
governed NAVCoin and vault-bridge surfaces.

| Method | Posture | Current dispatch behavior |
| --- | --- | --- |
| `bridge_domain` | operator/local only | With direct-state mode enabled, creates or updates a local bridge-domain record. |
| `bridge_transfer` | operator/local only | With direct-state mode enabled, applies a local simulated cross-domain transfer. |
| `bridge_pause` | operator/local only | With direct-state mode enabled, marks a local bridge domain paused. |
| `bridge_resume` | operator/local only | With direct-state mode enabled, marks a local bridge domain active. |
| `bridge_batch_domain` | operator/local only | Builds a bridge-domain action batch file without applying it. |
| `bridge_batch_transfer` | operator/local only | Builds a bridge-transfer action batch file without applying it. |
| `bridge_batch_pause` | operator/local only | Builds a bridge-pause action batch file without applying it. |
| `bridge_batch_resume` | operator/local only | Builds a bridge-resume action batch file without applying it. |
| `apply_bridge_batch` | operator/local only | Applies a prepared bridge-action batch directly and returns its receipts. |

## Coverage contract

`scripts/test-rpc-method-inventory` extracts the current dispatch arms and
requires every one to have an explicit table row in the hosted RPC
documentation. The generated inventory remains the source of truth for remote
default, gated, SDK, Python, and local-dispatch posture.
