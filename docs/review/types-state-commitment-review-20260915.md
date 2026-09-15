# Canonical types and state commitment review — 2026-09-15

This is A2 of the [burn 4 campaign](qa-campaign-20260915-burn4-brief.md). The review examined focused serialization, hashing, ordering, arithmetic, and version-handling paths in the eleven named `crates/types/src/` modules and `crates/node/src/state_commitment.rs`. The pfUSDC and Ethereum bridge type files were read only for schema context. This is a focused review, not a whole-file audit; the exact limits appear below. No other source was reviewed as a burn 4 surface.

## Findings

### 1. P2 — FastPay committee root omits its new-order admission window

**Source:** `crates/types/src/fastpay_recovery_types.rs:159-205`.

`FastPayRecoveryCommitteeV1::root_preimage` hashes the chain, genesis, version, epoch, and ordered roster, but not `valid_from_height` or `new_orders_through_height`. Two otherwise identical validated committees at the same epoch with different admission windows therefore compute the same `registry_root`. The ledger lookup by epoch and registry root can select the first committee even when its window differs from the intended one, while signatures using that registry identity cannot distinguish the schedules. The state commitment does contain the two heights, so the defect is specifically in the committee identity used to select and bind authorization.

The minimal repair is to length-preservingly append both big-endian heights to the v1 root preimage and test that changing either admission bound changes the computed root. This changes hashed committee identities and associated authorization domains and is **consensus-affecting**; a source repair is not live behavior and needs a separate activation/replay qualification before deployment.

### 2. P2 — FastPay recovery-reveal commitment omits its retained certificate

**Source:** `crates/types/src/fastpay_recovery_types.rs:697-730` and `crates/node/src/state_commitment.rs:1700-1712`.

`FastPayRecoveryRevealV1::validate_shape` checks that the certificate names the lock, but its `state_commitment_bytes` includes only the supplied order and certificate digest strings, height, schema, lock ID, and operation. It never checks or commits the retained certificate bytes. A restored or locally altered ledger can replace the certificate's owner signature or validator votes while retaining the same digest strings and root; the state root then identifies two distinct retained recovery evidence objects as one state. A later consumer of the retained certificate can see different evidence despite matching state roots.

The minimal repair is to bind a deterministic, length-prefixed serialization of the retained certificate in the reveal's state commitment, with a regression that changes a retained signature while preserving the other fields and checks different commitments. This changes state-root bytes and is **consensus-affecting**; the source change is not live or deployed and needs separate activation/replay qualification.

### 3. P3 — the public genesis digest helper narrows long domain-label lengths

**Source:** `crates/types/src/genesis_registry.rs:163-173`.

`genesis_domain_digest` uses `debug_assert!` for the `u16` label length and casts a supplied label length to `u16`. In a release build, a 65,536-byte label gets the same length prefix as an empty label. Calling the helper with the long label and a short payload versus the empty label and a payload beginning with the long label produces the same preimage. Existing genesis-registry callers use short constant labels, so the demonstrated collision needs an external caller supplying a longer label and is not a claim about existing genesis registry objects.

The minimal future repair is to reject overlong labels before narrowing, or return an error from a checked public helper while keeping existing short-label domain bytes intact. This P3 is recorded and not fixed in A2.

## Areas with no findings

- `crates/types/src/consensus_v2_types.rs` and `crates/types/src/core_chain.rs`: the reviewed versioned consensus-v2 field definitions, genesis fields, canonical identity constants, and transparent account/block/receipt schema had no additional findings.
- `crates/types/src/ledger_assets.rs`, `crates/types/src/account_owned_asset_types.rs`, and `crates/types/src/market_nav_asset_types.rs`: the reviewed issued-asset, owned-object, NAV amount arithmetic, bounded asset-state inventory, and receipt-merkle proof paths had no additional findings.
- `crates/types/src/genesis_registry.rs:240-380,415-500,744-880`, `crates/types/src/fastswap_types.rs:1-225,1000-1115,1540-1715,1790-2160`: the reviewed closed-label canonical-CBOR reader, sorted genesis entries, bounded FastSwap codecs, ordered committee roster, and ordered checkpoint fields had no additional findings beyond finding 3.
- `crates/types/src/fx_fix_types.rs`, `crates/types/src/nav_reserve_public_values.rs`, and `crates/types/src/shielded_bridge_governance.rs`: the reviewed bounded FX quote arithmetic, fixed-width NAV public-values round trip, and storage-commitment activation record hashes and height checks had no additional findings.
- `crates/types/src/fastpay_recovery_types.rs:30-107,365-510,735-945` and `crates/node/src/state_commitment.rs:1-105,515-1048,1180-1725,1729-1930,5590-5680`: the reviewed recovery policy/window bounds, ordered version-fence inputs, issued-supply custody checks, canonical length prefixes, inventory exhaustiveness, and sorted state-root vectors had no additional findings beyond findings 1 and 2.

## Review limits and skips

Large account-owned, market/NAV, FastSwap, governance/shielded, and state-commitment modules were sampled around the A2 focus rather than audited line by line. Account/owned asset bridge, pfUSDC/Arc, Orchard proof, and Ethereum routing implementations were not reviewed. In `crates/node/src/state_commitment.rs`, bridge-specific and excluded pfUSDC/Arc commitment bodies and historical exception machinery were not audited. `crates/types/src/pfusdc_tier4_types.rs`, `pfusdc_bonded_ingress_types.rs`, and `ethereum_bridge_types.rs` supplied read-only schema context only. No excluded or already-reviewed crate, other A surface, or B inventory was reviewed; no Task Node, fleet, chain, deployment, spend, signup, or frozen-artifact action occurred.
