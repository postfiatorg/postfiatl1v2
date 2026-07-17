# RPC Method Inventory

Status: code-derived inventory
Date: 2026-07-17
Report: `docs/runbooks/rpc-method-inventory.json`

This inventory is generated from Rust SDK method constants, node RPC dispatch,
the remote `rpc-serve` allowlist, and the Python RPC client. It is a launch
surface map, not a feature wishlist. “Public protocol mutation” means remotely
reachable by default but cryptographically authorized by a signed intent,
owner authorization, policy command, or quorum certificate; it does not mean
an unauthenticated state write. The generator fails closed if a newly observed
method lacks an explicit posture.

## Summary

- Total methods observed: 144
- Read-only public methods: 67
- Public cryptographically authorized protocol mutations: 12
- Controlled write gated methods: 15
- Privacy-alpha gated methods: 4
- Owned-lane gated methods: 8
- Operator/local-only methods: 38
- Unclassified methods: 0
- Classification checks: PASS

## XRP-Like Coverage

| Capability | State | Methods | Notes |
| --- | --- | --- | --- |
| server_info | covered | server_info | Node and Rust/Python clients expose a server_info alias. |
| latest ledger | covered | ledger, blocks | Latest ledger alias plus bounded block query exist. |
| ledger range | covered | blocks, archive_window | Node, Rust SDK, and Python client support bounded blocks.from_height; archive_window provides bounded handoff bundles. |
| fee | covered | fee, transfer_fee_quote | Static fee policy and wallet quote path exist. |
| validators | covered | validators, manifests | Active registry and operator manifests are public reads. |
| transaction lookup | covered | tx, receipts | Finality lookup and receipt reads exist. |
| account info | covered | account | Transparent account read exists. |
| account_tx | covered | account_tx, account_tx_index_status, python account_tx, python account_tx_index_status | Server-side bounded account_tx exists for transparent history and uses a rebuilt account_tx index when current, with scan fallback for absent or stale index data. |

## Gaps

| Gap | Severity | Description |
| --- | --- | --- |
| account_tx_index_incremental_store | scale_follow_up | account_tx index refreshes automatically after ordered commits using the rebuilt JSON cache; incremental append/update and a disk-backed store remain scale follow-ups. |
| public_write_edge_not_default | intentional_gate | All signed-mempool, Orchard batch-create, and owned-lane methods are disabled unless their exact operator flag enables them. |
| fastswap_public_protocol_mutations | authenticated_protocol_surface | FastSwap lock/vote/apply and FastLane control methods are remotely enabled protocol mutations, not reads; request signatures and quorum certificates are their authorization boundary. |

## Methods

| Method | Posture | Remote default | Remote gated | Local CLI | SDK | Python | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| account | read_only_public | yes | no | yes | yes | yes | XRP-like transparent account read |
| account_assets | read_only_public | yes | no | yes | yes | yes |  |
| account_escrows | read_only_public | yes | no | yes | yes | yes |  |
| account_lines | read_only_public | yes | no | yes | yes | yes |  |
| account_nfts | read_only_public | yes | no | yes | yes | yes |  |
| account_offers | read_only_public | yes | no | yes | yes | yes |  |
| account_tx | read_only_public | yes | no | yes | yes | yes | XRP-like bounded transparent account transaction history read; uses rebuilt index when current |
| account_tx_index_status | read_only_public | yes | no | yes | no | yes | read-only account_tx index freshness/status report |
| apply_batch | operator_or_local_only | no | no | yes | yes | no | operator/state-apply path; not public RPC |
| apply_bridge_batch | operator_or_local_only | no | no | yes | yes | no | operator/state-apply path; not public RPC |
| apply_shield_batch | operator_or_local_only | no | no | yes | yes | no | operator/state-apply path; not public RPC |
| archive_window | read_only_public | yes | no | yes | yes | no | bounded archive-window bundle read |
| asset_fee_quote | read_only_public | yes | no | yes | yes | yes |  |
| asset_info | read_only_public | yes | no | yes | yes | yes |  |
| asset_orchard_swap_create | operator_or_local_only | no | no | yes | no | no |  |
| atomic_settlement_template | read_only_public | yes | no | yes | yes | yes |  |
| atomic_swap_fee_quote | read_only_public | yes | no | yes | yes | no |  |
| batch_archive | read_only_public | yes | no | yes | yes | yes | bounded batch payload archive read |
| batch_transfer | operator_or_local_only | no | no | yes | no | no |  |
| blocks | read_only_public | yes | no | yes | yes | yes | bounded block query; node/Rust SDK/Python support from_height |
| book_offers | read_only_public | yes | no | yes | yes | yes |  |
| bridge_batch_domain | operator_or_local_only | no | no | yes | yes | no |  |
| bridge_batch_pause | operator_or_local_only | no | no | yes | yes | no |  |
| bridge_batch_resume | operator_or_local_only | no | no | yes | yes | no |  |
| bridge_batch_transfer | operator_or_local_only | no | no | yes | yes | no |  |
| bridge_domain | operator_or_local_only | no | no | yes | no | no |  |
| bridge_pause | operator_or_local_only | no | no | yes | no | no |  |
| bridge_resume | operator_or_local_only | no | no | yes | no | no |  |
| bridge_status | read_only_public | yes | no | yes | yes | yes | bridge simulation state read |
| bridge_transfer | operator_or_local_only | no | no | yes | no | no |  |
| escrow_fee_quote | read_only_public | yes | no | yes | yes | yes |  |
| escrow_info | read_only_public | yes | no | yes | yes | yes |  |
| fastlane_asset_control_apply | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastlane_asset_control_catch_up | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastlane_asset_control_prepare | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastlane_asset_control_preview | read_only_public | yes | no | no | yes | no |  |
| fastlane_exit | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastswap_apply | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastswap_cancel_apply | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastswap_capabilities | read_only_public | yes | no | no | yes | no |  |
| fastswap_catch_up | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastswap_checkpoint_status | read_only_public | yes | no | no | yes | no |  |
| fastswap_commit | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastswap_commit_round | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastswap_effects | read_only_public | yes | no | no | yes | no |  |
| fastswap_new_round_vote | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastswap_objects | read_only_public | yes | no | no | yes | no |  |
| fastswap_policy | read_only_public | yes | no | no | yes | no |  |
| fastswap_precommit | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastswap_prepare | authorized_protocol_mutation_public | yes | no | no | yes | no |  |
| fastswap_preview | read_only_public | yes | no | no | yes | no |  |
| fastswap_propose_round | read_only_public | yes | no | no | yes | no |  |
| fastswap_status | read_only_public | yes | no | no | yes | no |  |
| fastswap_votes | read_only_public | yes | no | no | yes | no |  |
| faucet | operator_or_local_only | no | no | yes | no | no |  |
| fee | read_only_public | yes | no | yes | yes | yes | XRP-like fee policy read |
| issuer_assets | read_only_public | yes | no | yes | yes | yes |  |
| issuer_nfts | read_only_public | yes | no | yes | yes | yes |  |
| ledger | read_only_public | yes | no | yes | yes | yes | XRP-like latest ledger alias with bounded block sample |
| manifests | read_only_public | yes | no | yes | yes | yes | operator manifest read |
| market_ops_status | read_only_public | yes | no | yes | no | no |  |
| mempool_batch | operator_or_local_only | no | no | yes | yes | no | operator/proposer batch creation |
| mempool_status | read_only_public | yes | no | yes | yes | yes | mempool read |
| mempool_submit_fastlane_primary | controlled_write_gated | no | yes | yes | yes | no |  |
| mempool_submit_fastlane_primary_finality | controlled_write_gated | no | yes | no | yes | no |  |
| mempool_submit_signed_asset_transaction | controlled_write_gated | no | yes | yes | yes | no |  |
| mempool_submit_signed_asset_transaction_finality | controlled_write_gated | no | yes | no | no | no |  |
| mempool_submit_signed_atomic_swap_transaction | controlled_write_gated | no | yes | yes | yes | no |  |
| mempool_submit_signed_atomic_swap_transaction_finality | controlled_write_gated | no | yes | no | yes | no |  |
| mempool_submit_signed_escrow_transaction | controlled_write_gated | no | yes | yes | yes | no |  |
| mempool_submit_signed_escrow_transaction_finality | controlled_write_gated | no | yes | no | no | no |  |
| mempool_submit_signed_nft_transaction | controlled_write_gated | no | yes | yes | yes | no |  |
| mempool_submit_signed_offer_transaction | controlled_write_gated | no | yes | yes | yes | no |  |
| mempool_submit_signed_payment_v2 | controlled_write_gated | no | yes | yes | yes | no |  |
| mempool_submit_signed_payment_v2_finality | controlled_write_gated | no | yes | no | no | no |  |
| mempool_submit_signed_transfer | controlled_write_gated | no | yes | yes | yes | no | controlled write edge; disabled unless explicitly enabled |
| mempool_submit_signed_transfer_finality | controlled_write_gated | no | yes | no | no | no | controlled in-process write/finality edge; disabled unless explicitly enabled |
| mempool_submit_transfer | operator_or_local_only | no | no | yes | yes | no | local wallet/debug helper; uses local key file |
| metrics | read_only_public | yes | no | yes | yes | yes | public node metrics read |
| navcoin_bridge_claims | read_only_public | yes | no | yes | yes | yes |  |
| navcoin_bridge_destination_consume | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_export_debit | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_import_return | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_launch_config_init | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_launch_config_template | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_packet | read_only_public | yes | no | yes | yes | no |  |
| navcoin_bridge_packet_preflight | operator_or_local_only | no | no | yes | yes | no |  |
| navcoin_bridge_primary_subscribe | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_receipt_replay | read_only_public | yes | no | yes | yes | yes |  |
| navcoin_bridge_record_fork_rehearsal | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_record_return_burn | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_refund_source | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_route_init | operator_or_local_only | no | no | yes | no | no |  |
| navcoin_bridge_routes | read_only_public | yes | no | yes | yes | yes |  |
| navcoin_bridge_supply_status | read_only_public | yes | no | yes | yes | yes |  |
| nft_fee_quote | read_only_public | yes | no | yes | yes | yes |  |
| nft_info | read_only_public | yes | no | yes | yes | yes |  |
| offer_fee_quote | read_only_public | yes | no | yes | yes | yes |  |
| offer_info | read_only_public | yes | no | yes | yes | yes |  |
| orchard_pool_report | read_only_public | yes | no | yes | no | yes | public Orchard pool counters only |
| owned-sign | operator_or_local_only | no | no | yes | no | no |  |
| owned-unwrap-apply | operator_or_local_only | no | no | yes | no | no |  |
| owned-unwrap-sign | operator_or_local_only | no | no | yes | no | no |  |
| owned_apply | owned_lane_gated | no | yes | no | no | yes |  |
| owned_apply_v3 | owned_lane_gated | no | yes | no | no | yes |  |
| owned_certificate | read_only_public | yes | no | no | no | no |  |
| owned_objects | read_only_public | yes | no | yes | no | yes |  |
| owned_recovery_capabilities | read_only_public | yes | no | no | no | yes |  |
| owned_recovery_status | read_only_public | yes | no | no | no | no |  |
| owned_sign | owned_lane_gated | no | yes | yes | yes | no |  |
| owned_sign_v3 | owned_lane_gated | no | yes | no | no | no |  |
| owned_unwrap_apply | owned_lane_gated | no | yes | yes | no | yes |  |
| owned_unwrap_apply_v3 | owned_lane_gated | no | yes | no | no | yes |  |
| owned_unwrap_sign | owned_lane_gated | no | yes | yes | yes | no |  |
| owned_unwrap_sign_v3 | owned_lane_gated | no | yes | no | no | no |  |
| receipts | read_only_public | yes | no | yes | yes | yes | bounded receipt lookup |
| server_info | read_only_public | yes | no | yes | yes | yes | XRP-like server info alias |
| shield_batch_asset_orchard_ingress | operator_or_local_only | no | no | yes | no | no |  |
| shield_batch_finality | controlled_write_gated | no | yes | no | no | yes |  |
| shield_batch_migrate | operator_or_local_only | no | no | yes | yes | no |  |
| shield_batch_mint | operator_or_local_only | no | no | yes | yes | no |  |
| shield_batch_orchard | privacy_alpha_gated | no | yes | yes | yes | no | privacy-alpha batch-create edge; disabled unless explicitly enabled |
| shield_batch_orchard_deposit | privacy_alpha_gated | no | yes | yes | yes | no | privacy-alpha direct-deposit batch-create edge; disabled unless explicitly enabled |
| shield_batch_orchard_withdraw | privacy_alpha_gated | no | yes | yes | yes | no | privacy-alpha withdraw batch-create edge; disabled unless explicitly enabled |
| shield_batch_spend | operator_or_local_only | no | no | yes | yes | no |  |
| shield_batch_swap | privacy_alpha_gated | no | yes | yes | yes | no |  |
| shield_disclose | read_only_public | yes | no | yes | yes | no | legacy shielded disclosure; not Orchard privacy |
| shield_mint | operator_or_local_only | no | no | yes | no | no |  |
| shield_scan | read_only_public | yes | no | yes | yes | no | legacy shielded owner scan; not Orchard privacy |
| shield_spend | operator_or_local_only | no | no | yes | no | no |  |
| shield_turnstile | read_only_public | yes | no | yes | yes | yes | shielded turnstile accounting read |
| status | read_only_public | yes | no | yes | yes | yes | public chain/node health read |
| transfer | operator_or_local_only | no | no | yes | no | no |  |
| transfer_fee_quote | read_only_public | yes | no | yes | yes | yes | transparent wallet quote read |
| tx | read_only_public | yes | no | yes | yes | yes | transaction finality lookup |
| validate_local_keys | operator_or_local_only | no | no | yes | yes | no | operator-only local key validation |
| validators | read_only_public | yes | no | yes | yes | yes | active validator registry read |
| vault_bridge_route | read_only_public | yes | no | yes | no | no |  |
| vault_bridge_status | read_only_public | yes | no | yes | no | no |  |
| verify_blocks | read_only_public | yes | no | yes | no | no | local verification surfaced through read RPC |
| verify_bridge | read_only_public | yes | no | yes | no | no | local verification surfaced through read RPC |
| verify_mempool | read_only_public | yes | no | yes | no | no | local verification surfaced through read RPC |
| verify_shielded | read_only_public | yes | no | yes | no | no | local verification surfaced through read RPC |
| verify_state | read_only_public | yes | no | yes | yes | no | local verification surfaced through read RPC |
