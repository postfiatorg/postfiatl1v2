//! Regression tests for the review points of Upgrade Train A (PR 295).

use super::*;
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign,
    ML_DSA_65_ALGORITHM,
};
use postfiat_execution::{
    execute_nft_transaction_with_compatibility, execute_offer_transaction_with_compatibility,
    AssetExecutionCompatibility,
};
use postfiat_types::{
    nft_id, Account, GovernanceAmendment, NftBurnOperation, NftMintOperation,
    NftTransactionOperation, OfferPruneIneligibleOperation, OfferTransactionOperation,
    SignedNftTransaction, SignedOfferTransaction, UnsignedNftTransaction,
    UnsignedOfferTransaction, ADDRESS_NAMESPACE, GOVERNANCE_KIND_ASSET_GATE_V1_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_FASTPAY_FEE_V1_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_NFT_SCALING_V1_ACTIVATION_HEIGHT, NFT_BURN_TRANSACTION_KIND,
    NFT_COLLECTION_FLAG_BURN_LOCKED, NFT_FLAG_ISSUER_BURNABLE, NFT_FLAG_TRANSFERABLE,
    NFT_MINT_TRANSACTION_KIND, OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND,
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

struct ReviewKey {
    address: String,
    public_key_hex: String,
    keypair: postfiat_crypto_provider::MlDsa65KeyPair,
}

fn review_key(seed: u8) -> ReviewKey {
    let keypair = ml_dsa_65_keygen_from_seed(&[seed; 32]);
    ReviewKey {
        address: address_from_public_key(&keypair.public_key),
        public_key_hex: bytes_to_hex(&keypair.public_key),
        keypair,
    }
}

fn review_account(key: &ReviewKey) -> Account {
    Account {
        address: key.address.clone(),
        balance: 10_000_000,
        sequence: 0,
        public_key_hex: Some(key.public_key_hex.clone()),
        flags: 0,
    }
}

fn review_nft_tx(
    genesis: &Genesis,
    key: &ReviewKey,
    sequence: u64,
    kind: &str,
    operation: NftTransactionOperation,
) -> SignedNftTransaction {
    let unsigned = UnsignedNftTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: kind.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: key.address.clone(),
        fee: 50_000,
        sequence,
        operation,
    };
    let signature = ml_dsa_65_sign(&key.keypair.private_key, &unsigned.signing_bytes())
        .expect("sign nft tx");
    SignedNftTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: key.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    }
}

fn review_mint(issuer: &ReviewKey, owner: &str, serial: u64, flags: u32, collection_flags: u32) -> NftTransactionOperation {
    NftTransactionOperation::NftMint(NftMintOperation {
        issuer: issuer.address.clone(),
        collection_id: "COL".to_string(),
        serial,
        owner: owner.to_string(),
        metadata_hash: "11".repeat(48),
        metadata_uri: "ipfs://review".to_string(),
        flags,
        collection_flags,
        issuer_transfer_fee: 0,
    })
}

/// A fresh store whose governance activates NFT scaling and the asset gate
/// at height 1, with its tip moved to height 5 (admission judges height 6).
fn store_with_train_a_active(label: &str) -> (PathBuf, NodeStore, Genesis) {
    let dir = review_test_dir(label);
    let genesis = Genesis::new_with_validator_count("postfiat-train-a-review", 1);
    let store = NodeStore::new(&dir);
    store
        .init(&genesis, &NodeState::initialized("val-0"))
        .expect("init store");
    let mut governance = store.read_governance().expect("read governance");
    for kind in [
        GOVERNANCE_KIND_NFT_SCALING_V1_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_ASSET_GATE_V1_ACTIVATION_HEIGHT,
    ] {
        governance.amendments.push(GovernanceAmendment {
            amendment_id: format!("amd-{kind}"),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            instance_id: "i".to_string(),
            proposal_id: "p".to_string(),
            certificate_id: "c".to_string(),
            proposer: "val-0".to_string(),
            validators: vec!["val-0".to_string()],
            quorum: 1,
            support: vec!["val-0".to_string()],
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
            kind: kind.to_string(),
            value: 1,
            activation_height: 1,
            veto_until_height: 0,
            paused: false,
        });
    }
    store.write_governance(&governance).expect("write governance");
    let mut tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis).expect("read tip");
    tip.height = 5;
    store.write_chain_tip(&tip).expect("write tip");
    (dir, store, genesis)
}

/// Point 3 (A3): mempool admission judges NFT, offer and escrow transactions
/// by the rules the next block applies.
#[test]
fn mempool_admission_uses_the_block_rules_after_train_a_activation() {
    let (dir, store, genesis) = store_with_train_a_active("mempool-rules");
    let governance = store.read_governance().expect("read governance");
    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
    assert!(compatibility.asset_gate_v1_active(6) && compatibility.nft_scaling_v1_active(6));

    let issuer = review_key(81);
    let owner = review_key(82);
    let spammer = review_key(83);
    let (pruner, mut prune) = signed_offer_prune(&genesis, 84, 1);
    let mut ledger = store.read_ledger().expect("read ledger");
    ledger.accounts.extend([
        review_account(&issuer),
        review_account(&owner),
        review_account(&spammer),
        pruner,
    ]);
    // An open native-PFT offer of `owner` that has expired: prunable once the
    // gate is active.
    let quote_asset = postfiat_types::AssetDefinition::new(&genesis.chain_id, &issuer.address, "RVW", 1, 6)
        .expect("quote asset");
    let offer = postfiat_types::Offer::new(
        &genesis.chain_id,
        owner.address.clone(),
        7,
        postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID,
        1_000,
        quote_asset.asset_id.clone(),
        1_000,
        1,
        2,
    )
    .expect("expired offer");
    ledger.asset_definitions.push(quote_asset);
    let owner_account = ledger
        .accounts
        .iter_mut()
        .find(|account| account.address == owner.address)
        .expect("owner account");
    owner_account.balance -= 1_000 + offer.reserve_paid;
    let offer_id = offer.offer_id.clone();
    ledger.offers.push(offer);
    let prune_key = ml_dsa_65_keygen_from_seed(&[84; 32]);
    if let OfferTransactionOperation::OfferPruneIneligible(operation) = &mut prune.unsigned.operation {
        operation.offer_ids = vec![offer_id];
    }
    prune.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign(&prune_key.private_key, &prune.unsigned.signing_bytes()).expect("re-sign prune"),
    );
    // An issuer-burnable NFT minted under the new rules.
    let mint = review_nft_tx(
        &genesis,
        &issuer,
        1,
        NFT_MINT_TRANSACTION_KIND,
        review_mint(&issuer, &owner.address, 1, NFT_FLAG_TRANSFERABLE | NFT_FLAG_ISSUER_BURNABLE, 0),
    );
    let minted =
        execute_nft_transaction_with_compatibility(&genesis, &mut ledger, &mint, 6, compatibility);
    assert!(minted.accepted, "{minted:?}");
    store.write_ledger(&ledger).expect("write ledger");

    // offer_prune_ineligible is admitted once the gate is active.
    submit_signed_offer_transaction_json_to_mempool(SignedOfferTransactionJsonSubmitOptions {
        data_dir: dir.clone(),
        signed_offer_transaction_json: serde_json::to_string(&prune).expect("encode prune"),
    })
    .expect("offer_prune_ineligible is admitted after activation");

    // An issuer burn is admitted once NFT scaling is active.
    let burn = review_nft_tx(
        &genesis,
        &issuer,
        2,
        NFT_BURN_TRANSACTION_KIND,
        NftTransactionOperation::NftBurn(NftBurnOperation {
            nft_id: nft_id(&genesis.chain_id, &issuer.address, "COL", 1).expect("nft id"),
            owner: owner.address.clone(),
            issuer: issuer.address.clone(),
        }),
    );
    submit_signed_nft_transaction_json_to_mempool(SignedNftTransactionJsonSubmitOptions {
        data_dir: dir.clone(),
        signed_nft_transaction_json: serde_json::to_string(&burn).expect("encode burn"),
    })
    .expect("issuer nft_burn is admitted after activation");

    // An unsolicited permanently locked mint is refused at admission, as the
    // block refuses it.
    let spam = review_nft_tx(
        &genesis,
        &spammer,
        1,
        NFT_MINT_TRANSACTION_KIND,
        NftTransactionOperation::NftMint(NftMintOperation {
            issuer: spammer.address.clone(),
            collection_id: "SPAM".to_string(),
            serial: 1,
            owner: owner.address.clone(),
            metadata_hash: "22".repeat(48),
            metadata_uri: "ipfs://spam".to_string(),
            flags: 0,
            collection_flags: NFT_COLLECTION_FLAG_BURN_LOCKED,
            issuer_transfer_fee: 0,
        }),
    );
    let block = execute_nft_transaction_with_compatibility(
        &genesis,
        &mut ledger.clone(),
        &spam,
        6,
        compatibility,
    );
    assert_eq!(block.code, "nft_unsolicited_permanent_lock_forbidden");
    let error = submit_signed_nft_transaction_json_to_mempool(SignedNftTransactionJsonSubmitOptions {
        data_dir: dir.clone(),
        signed_nft_transaction_json: serde_json::to_string(&spam).expect("encode spam"),
    })
    .expect_err("the block's refusal is an admission refusal");
    assert!(
        error.to_string().contains("nft_unsolicited_permanent_lock_forbidden")
            || error.to_string().contains("burn-locked and non-transferable"),
        "{error}"
    );

    // Escrow: on an authorization-required asset, finishing an escrow whose
    // owner's line is authorized is admitted; finishing one whose owner's
    // line is not is refused at admission, as the block refuses it.
    let escrow_issuer = review_key(85);
    let authorized_owner = review_key(86);
    let unauthorized_owner = review_key(87);
    let recipient = review_key(88);
    let mut escrow_asset = postfiat_types::AssetDefinition::new(
        &genesis.chain_id,
        &escrow_issuer.address,
        "EGT",
        1,
        6,
    )
    .expect("escrow asset");
    escrow_asset.requires_authorization = true;
    let mut ledger = store.read_ledger().expect("read ledger");
    ledger.accounts.extend([
        review_account(&escrow_issuer),
        review_account(&authorized_owner),
        review_account(&unauthorized_owner),
        review_account(&recipient),
    ]);
    let mut escrow_finishes = Vec::new();
    for (sequence, owner, authorized) in [
        (1, &authorized_owner, true),
        (2, &unauthorized_owner, false),
    ] {
        ledger.account_mut(&owner.address).expect("owner").sequence = 1;
        let mut line = postfiat_types::TrustLine::new(
            &owner.address,
            &escrow_issuer.address,
            &escrow_asset.asset_id,
            1_000,
            10,
        )
        .expect("owner line");
        line.authorized = authorized;
        ledger.trustlines.push(line);
        let escrow = postfiat_types::Escrow::new(
            &genesis.chain_id,
            &owner.address,
            1,
            &recipient.address,
            &escrow_asset.asset_id,
            10,
            100,
            "",
            1,
            20,
            1,
        )
        .expect("escrow");
        let unsigned = postfiat_types::UnsignedEscrowTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: postfiat_types::ESCROW_FINISH_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            source: recipient.address.clone(),
            fee: 50_000,
            sequence,
            operation: postfiat_types::EscrowTransactionOperation::EscrowFinish(
                postfiat_types::EscrowFinishOperation {
                    escrow_id: escrow.escrow_id.clone(),
                    owner: owner.address.clone(),
                    recipient: recipient.address.clone(),
                    fulfillment: String::new(),
                },
            ),
        };
        ledger.escrows.push(escrow);
        escrow_finishes.push(postfiat_types::SignedEscrowTransaction {
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign(&recipient.keypair.private_key, &unsigned.signing_bytes())
                    .expect("sign escrow finish"),
            ),
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: recipient.public_key_hex.clone(),
        });
    }
    let mut recipient_line = postfiat_types::TrustLine::new(
        &recipient.address,
        &escrow_issuer.address,
        &escrow_asset.asset_id,
        1_000,
        0,
    )
    .expect("recipient line");
    recipient_line.authorized = true;
    ledger.trustlines.push(recipient_line);
    ledger.asset_definitions.push(escrow_asset);
    store.write_ledger(&ledger).expect("write ledger");

    let admitted = &escrow_finishes[0];
    let mut block_ledger = ledger.clone();
    let block = postfiat_execution::execute_escrow_transaction_with_compatibility(
        &genesis,
        &mut block_ledger,
        admitted,
        6,
        compatibility,
    );
    assert!(block.accepted, "{block:?}");
    submit_signed_escrow_transaction_json_to_mempool(SignedEscrowTransactionJsonSubmitOptions {
        data_dir: dir.clone(),
        signed_escrow_transaction_json: serde_json::to_string(admitted).expect("encode finish"),
    })
    .expect("an eligible escrow finish is admitted after activation");

    let refused = &escrow_finishes[1];
    let legacy = postfiat_execution::execute_escrow_transaction(
        &genesis,
        &mut block_ledger.clone(),
        refused,
        6,
    );
    assert!(legacy.accepted, "the old rules accept it: {legacy:?}");
    let block = postfiat_execution::execute_escrow_transaction_with_compatibility(
        &genesis,
        &mut block_ledger,
        refused,
        6,
        compatibility,
    );
    assert_eq!(block.code, "asset_gate_sender_ineligible");
    let error =
        submit_signed_escrow_transaction_json_to_mempool(SignedEscrowTransactionJsonSubmitOptions {
            data_dir: dir.clone(),
            signed_escrow_transaction_json: serde_json::to_string(refused)
                .expect("encode finish"),
        })
        .expect_err("the block's escrow refusal is an admission refusal");
    assert!(
        error.to_string().contains("asset_gate_sender_ineligible")
            || error.to_string().contains("not eligible"),
        "{error}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Point 9 (A13, B12): once the asset gate is active, the asset fee quote
/// includes the implicit-line fee that execution charges.
#[test]
fn asset_fee_quote_includes_the_implicit_line_fee_after_the_asset_gate() {
    let (dir, store, genesis) = store_with_train_a_active("asset-fee-quote");
    let governance = store.read_governance().expect("read governance");
    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
    let issuer = review_key(91);
    let recipient = review_key(92);
    let asset = postfiat_types::AssetDefinition::new(&genesis.chain_id, &issuer.address, "QTE", 1, 6)
        .expect("asset");
    let mut ledger = store.read_ledger().expect("read ledger");
    ledger
        .accounts
        .extend([review_account(&issuer), review_account(&recipient)]);
    ledger.asset_definitions.push(asset.clone());
    store.write_ledger(&ledger).expect("write ledger");
    let operation =
        postfiat_types::AssetTransactionOperation::IssuedPayment(postfiat_types::IssuedPaymentOperation {
            from: issuer.address.clone(),
            to: recipient.address.clone(),
            asset_id: asset.asset_id.clone(),
            issuer: issuer.address.clone(),
            amount: 1,
        });
    let quote = asset_fee_quote(AssetFeeQuoteOptions {
        data_dir: dir.clone(),
        source: issuer.address.clone(),
        operation_json: serde_json::to_string(&operation).expect("operation json"),
        sequence: None,
    })
    .expect("asset fee quote");
    let signed = quote_signed_asset_transaction(
        &genesis,
        issuer.address.clone(),
        quote.minimum_fee,
        quote.sequence,
        operation,
    )
    .expect("quoted transaction");
    let legacy = postfiat_execution::asset_transaction_state_expansion_fee(&ledger, &signed);
    let enforced = postfiat_execution::asset_transaction_state_expansion_fee_with_compatibility(
        &ledger,
        &signed,
        6,
        compatibility,
    );
    assert!(enforced > legacy, "the gate adds an implicit-line fee");
    assert_eq!(quote.state_expansion_fee, enforced);
    assert_eq!(
        quote.minimum_fee,
        postfiat_execution::minimum_asset_transaction_fee_for_ledger_with_compatibility(
            &ledger,
            &signed,
            6,
            compatibility,
        )
    );
    let _ = std::fs::remove_dir_all(&dir);
}
