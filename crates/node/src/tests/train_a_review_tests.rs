//! Regression tests for the review points of Upgrade Train A (PR 295).

use super::*;
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign,
    ML_DSA_65_ALGORITHM,
};
use postfiat_execution::{execute_offer_transaction_with_compatibility, AssetExecutionCompatibility};
use postfiat_types::{
    Account, OfferPruneIneligibleOperation, OfferTransactionOperation, SignedOfferTransaction,
    UnsignedOfferTransaction, ADDRESS_NAMESPACE, GOVERNANCE_KIND_ASSET_GATE_V1_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_FASTPAY_FEE_V1_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_NFT_SCALING_V1_ACTIVATION_HEIGHT, OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND,
};

fn review_test_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "postfiat-train-a-review-{label}-{}-{nanos}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn signed_offer_prune(
    genesis: &Genesis,
    seed: u8,
    sequence: u64,
) -> (Account, SignedOfferTransaction) {
    let key = ml_dsa_65_keygen_from_seed(&[seed; 32]);
    let address = address_from_public_key(&key.public_key);
    let public_key_hex = bytes_to_hex(&key.public_key);
    let unsigned = UnsignedOfferTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: address.clone(),
        fee: 100_000,
        sequence,
        operation: OfferTransactionOperation::OfferPruneIneligible(
            OfferPruneIneligibleOperation {
                pruner: address.clone(),
                offer_ids: vec!["ab".repeat(48)],
            },
        ),
    };
    let signature = ml_dsa_65_sign(&key.private_key, &unsigned.signing_bytes()).expect("sign");
    let account = Account {
        address,
        balance: 10_000_000,
        sequence: 0,
        public_key_hex: Some(public_key_hex.clone()),
        flags: 0,
    };
    (
        account,
        SignedOfferTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex,
            signature_hex: bytes_to_hex(&signature),
        },
    )
}

/// Point 1 (A1, B14): each Train A activation kind is ratified through the
/// node's governance path, certified into a governance batch, applied, and
/// the rule switches exactly at the scheduled height.
#[test]
fn train_a_activation_kinds_ratify_certify_apply_and_switch_at_the_height() {
    let dir = review_test_dir("ratify");
    init(InitOptions {
        data_dir: dir.clone(),
        chain_id: "postfiat-train-a-ratify".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize store");
    let store = NodeStore::new(&dir);
    let genesis = store.read_genesis().expect("read genesis");
    let ratify = |kind: &str, value: u32, file: &str| {
        ratify_governance(RatifyGovernanceOptions {
            data_dir: dir.clone(),
            validators: vec!["validator-0".to_string()],
            support: vec!["validator-0".to_string()],
            kind: kind.to_string(),
            value,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: dir.join(file),
        })
    };
    for kind in [
        GOVERNANCE_KIND_FASTPAY_FEE_V1_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_NFT_SCALING_V1_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_ASSET_GATE_V1_ACTIVATION_HEIGHT,
    ] {
        assert!(
            ratify(kind, 0, "zero.json").is_err(),
            "{kind} must refuse a zero activation height"
        );
        let amendment = ratify(kind, 10, "ten.json").unwrap_or_else(|error| panic!("{kind}: {error}"));
        let batch = build_governance_action_batch(&genesis, vec![amendment.clone()], Vec::new())
            .expect("certified governance batch");

        // A past and the current height are refused when the block applies them.
        for block_height in [10, 11] {
            let mut governance = store.read_governance().expect("read governance");
            let mut ledger = store.read_ledger().expect("read ledger");
            let receipts =
                execute_governance_batch(&mut governance, Some(&mut ledger), None, &batch, block_height);
            assert!(!receipts[0].accepted, "{kind} at {block_height}: {receipts:?}");
            assert!(receipts[0].code.starts_with("invalid_"), "{receipts:?}");
            assert_eq!(governance_amendment_current_value(&governance, kind), 0);
        }

        let mut governance = store.read_governance().expect("read governance");
        let mut ledger = store.read_ledger().expect("read ledger");
        let receipts = execute_governance_batch(&mut governance, Some(&mut ledger), None, &batch, 1);
        assert!(receipts[0].accepted, "{kind}: {receipts:?}");
        assert_eq!(governance_amendment_current_value(&governance, kind), 10);
        let compatibility =
            asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
        let active = |height| match kind {
            GOVERNANCE_KIND_FASTPAY_FEE_V1_ACTIVATION_HEIGHT => {
                compatibility.fastpay_fee_v1_active(height)
            }
            GOVERNANCE_KIND_NFT_SCALING_V1_ACTIVATION_HEIGHT => {
                compatibility.nft_scaling_v1_active(height)
            }
            _ => compatibility.asset_gate_v1_active(height),
        };
        assert!(!active(9) && active(10), "{kind} must switch at height 10");

        if kind == GOVERNANCE_KIND_ASSET_GATE_V1_ACTIVATION_HEIGHT {
            // The real rule: offer_prune_ineligible is refused before the
            // height and runs (finds no such offer) from the height on.
            let (account, prune) = signed_offer_prune(&genesis, 42, 1);
            let mut prune_ledger = ledger.clone();
            prune_ledger.accounts.push(account);
            let before = execute_offer_transaction_with_compatibility(
                &genesis,
                &mut prune_ledger.clone(),
                &prune,
                9,
                compatibility,
            );
            assert_eq!(before.code, "asset_gate_not_active", "{before:?}");
            let unprimed = prune_ledger.clone();
            let after = execute_offer_transaction_with_compatibility(
                &genesis,
                &mut prune_ledger,
                &prune,
                10,
                compatibility,
            );
            assert_eq!(after.code, "missing_offer", "{after:?}");
            assert_eq!(
                execute_offer_transaction_with_compatibility(
                    &genesis,
                    &mut unprimed.clone(),
                    &prune,
                    10,
                    AssetExecutionCompatibility::strict(),
                )
                .code,
                "asset_gate_not_active"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
