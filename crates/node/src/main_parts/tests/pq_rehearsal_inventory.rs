use postfiat_execution::genesis_hash;
use postfiat_node::{global_issued_asset_supply, native_pft_live_total};
use postfiat_storage::NodeStore;
use std::path::Path;

pub fn inventory(dir: &Path) {
    let store = NodeStore::new(dir);
    let ledger = store.read_ledger().unwrap();
    let shielded = store.read_shielded().unwrap();
    let genesis = store.read_genesis().unwrap();
    let live = native_pft_live_total(&ledger, &shielded).unwrap();
    let fees: u128 = store
        .read_receipts()
        .unwrap()
        .iter()
        .map(|r| u128::from(r.fee_burned))
        .sum();
    let fastpay_fees: u128 = ledger
        .fastpay_version_fences
        .iter()
        .filter(|f| f.origin == postfiat_types::FastPayFenceOriginV1::Consensusless)
        .filter_map(|f| f.certificate.as_ref())
        .map(|c| {
            u128::from(match c {
                postfiat_types::FastPayCertificateV1::Transfer(c) => c.order.fee,
                postfiat_types::FastPayCertificateV1::Unwrap(c) => c.order.fee,
            })
        })
        .sum();
    println!(
        "SUPPLY genesis={} PFT_live={live} receipt_fee_burn={fees} fastpay_fee_burn={fastpay_fees} genesis_native={}",
        genesis_hash(&genesis),
        genesis.expected_native_supply_atoms()
    );
    if let Some(p) = &shielded.pq_pool_v1 {
        for (asset, state) in &p.head.assets {
            println!(
                "POOL asset={asset} epoch={} reserve={} root={}",
                state.epoch,
                state.reserve,
                state.writable.anchors.back().unwrap()
            );
        }
    }
    println!(
        "PQ_PAIRS {}",
        shielded.pq_pool_v1.as_ref().map_or(0, |p| p.head.pairs)
    );
    for a in &ledger.asset_definitions {
        println!(
            "ASSET {} {} {} {}",
            a.code,
            a.asset_id,
            if a.asset_family_id.is_empty() {
                "family"
            } else {
                "series"
            },
            global_issued_asset_supply(&ledger, &shielded, &a.asset_id).unwrap()
        );
    }
}
