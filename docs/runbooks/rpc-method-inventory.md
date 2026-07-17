# RPC Method Inventory

Status: code-derived inventory
Date: 2026-05-17
Report: `reports/testnet-rpc-method-inventory/testnet-rpc-method-inventory-20260517T035236Z.json`

This inventory is generated from Rust SDK method constants, node RPC dispatch,
the remote `rpc-serve` allowlist, and the Python RPC client. It is a launch
surface map, not a feature wishlist.

## Summary

- Total methods observed: 54
- Read-only public methods: 27
- Controlled write gated methods: 1
- Privacy-alpha gated methods: 3
- Operator/local-only methods: 23

Latest refresh was run on clean revision `d352f04` after the RPC boolean flag
handling fix. The method counts and public/write-gated classifications were
unchanged.

## 2026-06-29 FastPay Addendum

The generated inventory above predates the wallet-facing FastPay repair. The
current WAN devnet and wallet proxy also use these FastPay RPC methods:

| Method | Posture | Notes |
| --- | --- | --- |
| `owned_objects` | read_only_public | FastPay owned-object lookup. Wallet and Python defaults request up to 2048 objects for fragmented wallets. |
| `wrap_owned` | controlled_write_gated | Account-lane PFT to FastPay owned object. |
| `owned_sign` | controlled_write_gated | Validator vote for signed FastPay owned-transfer orders. |
| `owned_apply` | controlled_write_gated | Quorum-certified owned-transfer apply. The wallet proxy succeeds at BFT quorum, 5 of 6 on the current devnet. |
| `owned_unwrap_sign` | controlled_write_gated | Validator vote for signed FastPay unwrap orders. |
| `owned_unwrap_apply` | controlled_write_gated | Quorum-certified standard unwrap apply; supports amount-based multi-input unwrap up to 2048 inputs with automatic FastPay change. |
| `unwrap_owned` | disabled_compatibility | Public wallet flows must not call this unsigned whole-object path. |

Redeploy evidence:
`reports/transaction-improvement/20260629T012710Z-fastpay-owned-objects-read-cap2048-deploy/post-deploy-preflight.json`.

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
| account_tx | covered | account_tx, account_tx_index_status, python account_tx, python account_tx_index_status | Server-side bounded account_tx exists for transparent history and prefers a disk-backed per-account index when current, then aggregate index fallback, then bounded retained-history scan. |

## Gaps

| Gap | Severity | Description |
| --- | --- | --- |
| account_tx_index_aggregate_compaction | scale_follow_up | account_tx index refreshes automatically after ordered commits and disk-backed per-account reads exist; making the aggregate JSON index optional/metadata-only remains a scale follow-up. |
| public_write_edge_not_default | intentional_gate | mempool_submit_signed_transfer and Orchard batch creation are disabled on read-only RPC unless operator flags explicitly enable them. |

## Methods

| Method | Posture | SDK | Node | Python | Notes |
| --- | --- | --- | --- | --- | --- |
| account | read_only_public | yes | yes | yes | XRP-like transparent account read |
| account_tx | read_only_public | yes | yes | yes | XRP-like bounded transparent account transaction history read; uses disk-backed per-account index when current |
| account_tx_index_status | read_only_public | no | yes | yes | read-only account_tx index freshness/status report |
| apply_batch | operator_or_local_only | yes | yes | no | operator/state-apply path; not public RPC |
| apply_bridge_batch | operator_or_local_only | yes | yes | no | operator/state-apply path; not public RPC |
| apply_shield_batch | operator_or_local_only | yes | yes | no | operator/state-apply path; not public RPC |
| archive_window | read_only_public | yes | yes | no | bounded archive-window bundle read |
| batch_archive | read_only_public | yes | yes | yes | bounded batch payload archive read |
| batch_transfer | operator_or_local_only | no | yes | no |  |
| blocks | read_only_public | yes | yes | yes | bounded block query; node/Rust SDK/Python support from_height |
| bridge_batch_domain | operator_or_local_only | yes | yes | no |  |
| bridge_batch_pause | operator_or_local_only | yes | yes | no |  |
| bridge_batch_resume | operator_or_local_only | yes | yes | no |  |
| bridge_batch_transfer | operator_or_local_only | yes | yes | no |  |
| bridge_domain | operator_or_local_only | no | yes | no |  |
| bridge_pause | operator_or_local_only | no | yes | no |  |
| bridge_resume | operator_or_local_only | no | yes | no |  |
| bridge_status | read_only_public | yes | yes | yes | bridge simulation state read |
| bridge_transfer | operator_or_local_only | no | yes | no |  |
| faucet | operator_or_local_only | no | yes | no |  |
| fee | read_only_public | yes | yes | yes | XRP-like fee policy read |
| ledger | read_only_public | yes | yes | yes | XRP-like latest ledger alias with bounded block sample |
| manifests | read_only_public | yes | yes | yes | operator manifest read |
| market_ops_status | read_only_public | yes | yes | yes | NAVSwap market-ops public status and capped capacity read |
| mempool_batch | operator_or_local_only | yes | yes | no | operator/proposer batch creation |
| mempool_status | read_only_public | yes | yes | yes | mempool read |
| mempool_submit_signed_transfer | controlled_write_gated | yes | yes | no | controlled write edge; disabled unless explicitly enabled |
| mempool_submit_transfer | operator_or_local_only | yes | yes | no | local wallet/debug helper; uses local key file |
| metrics | read_only_public | yes | yes | yes | public node metrics read |
| navcoin_bridge_claims | read_only_public | yes | yes | yes | bounded PFTL-to-Uniswap export and return claim status |
| navcoin_bridge_packet | read_only_public | yes | yes | yes | PFTL-to-Uniswap export packet status by route and packet hash |
| navcoin_bridge_packet_preflight | operator_or_local_only | yes | yes | no | local pre-relay PFTL-to-Uniswap mint-and-swap packet validation against route ledger and launch config |
| navcoin_bridge_receipt_replay | read_only_public | yes | yes | yes | deterministic replay verification of persisted PFTL-to-Uniswap bridge receipts against the route ledger |
| navcoin_bridge_routes | read_only_public | yes | yes | yes | PFTL-to-Uniswap route status read with distinct handoff controller and settlement adapter fields |
| navcoin_bridge_supply_status | read_only_public | yes | yes | yes | PFTL-to-Uniswap bridge supply invariant read with per-wallet native NAV balance rows |
| orchard_pool_report | read_only_public | no | yes | yes | public Orchard pool counters only |
| receipts | read_only_public | yes | yes | yes | bounded receipt lookup |
| server_info | read_only_public | yes | yes | yes | XRP-like server info alias |
| shield_batch_migrate | operator_or_local_only | yes | yes | no |  |
| shield_batch_mint | operator_or_local_only | yes | yes | no |  |
| shield_batch_orchard | privacy_alpha_gated | yes | yes | no | privacy-alpha batch-create edge; disabled unless explicitly enabled |
| shield_batch_orchard_deposit | privacy_alpha_gated | yes | yes | no | privacy-alpha direct-deposit batch-create edge; disabled unless explicitly enabled |
| shield_batch_orchard_withdraw | privacy_alpha_gated | yes | yes | no | privacy-alpha withdraw batch-create edge; disabled unless explicitly enabled |
| shield_batch_spend | operator_or_local_only | yes | yes | no |  |
| shield_disclose | read_only_public | yes | yes | no | legacy shielded disclosure; not Orchard privacy |
| shield_mint | operator_or_local_only | no | yes | no |  |
| shield_scan | read_only_public | yes | yes | no | legacy shielded owner scan; not Orchard privacy |
| shield_spend | operator_or_local_only | no | yes | no |  |
| shield_turnstile | read_only_public | yes | yes | yes | shielded turnstile accounting read |
| status | read_only_public | yes | yes | yes | public chain/node health read |
| transfer | operator_or_local_only | no | yes | no |  |
| transfer_fee_quote | read_only_public | yes | yes | no | transparent wallet quote read |
| tx | read_only_public | yes | yes | no | transaction finality lookup |
| validate_local_keys | operator_or_local_only | yes | yes | no | operator-only local key validation |
| vault_bridge_status | read_only_public | yes | yes | yes | vault bridge public asset status, receipts, and capacity read |
| validators | read_only_public | yes | yes | yes | active validator registry read |
| verify_blocks | read_only_public | no | yes | no | local verification surfaced through read RPC |
| verify_bridge | read_only_public | no | yes | no | local verification surfaced through read RPC |
| verify_mempool | read_only_public | no | yes | no | local verification surfaced through read RPC |
| verify_shielded | read_only_public | no | yes | no | local verification surfaced through read RPC |
| verify_state | read_only_public | yes | yes | no | local verification surfaced through read RPC |
