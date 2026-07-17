struct AtomicExecutionFixture {
    genesis: Genesis,
    ledger: LedgerState,
    owner_0_key: MlDsa65KeyPair,
    owner_1_key: MlDsa65KeyPair,
    transaction: SignedAtomicSwapTransaction,
}

fn atomic_test_line(
    account: &str,
    asset: &AssetDefinition,
    balance: u64,
) -> TrustLine {
    let mut line = TrustLine::new(
        account,
        &asset.issuer,
        &asset.asset_id,
        1_000_000,
        TRUSTLINE_STATE_EXPANSION_FEE,
    )
    .expect("atomic test trustline");
    line.authorized = true;
    line.balance = balance;
    line
}

fn sign_atomic_swap(
    mut unsigned: postfiat_types::UnsignedAtomicSwapTransaction,
    owner_0_key: &MlDsa65KeyPair,
    owner_1_key: &MlDsa65KeyPair,
) -> SignedAtomicSwapTransaction {
    let owner_0 = address_from_public_key(&owner_0_key.public_key);
    let owner_1 = address_from_public_key(&owner_1_key.public_key);
    unsigned.leg_0.owner = owner_0.clone();
    unsigned.leg_0.recipient = owner_1.clone();
    unsigned.leg_1.owner = owner_1.clone();
    unsigned.leg_1.recipient = owner_0.clone();

    let signing_bytes = unsigned.signing_bytes();
    SignedAtomicSwapTransaction {
        unsigned,
        authorization_0: AtomicSwapAuthorization {
            owner: owner_0,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_0_key.public_key),
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign(&owner_0_key.private_key, &signing_bytes)
                    .expect("final sign atomic owner 0"),
            ),
        },
        authorization_1: AtomicSwapAuthorization {
            owner: owner_1,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_1_key.public_key),
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign(&owner_1_key.private_key, &signing_bytes)
                    .expect("final sign atomic owner 1"),
            ),
        },
    }
}

fn resign_atomic_swap(
    transaction: &mut SignedAtomicSwapTransaction,
    owner_0_key: &MlDsa65KeyPair,
    owner_1_key: &MlDsa65KeyPair,
) {
    let signing_bytes = transaction.unsigned.signing_bytes();
    transaction.authorization_0.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign(&owner_0_key.private_key, &signing_bytes)
            .expect("resign atomic owner 0"),
    );
    transaction.authorization_1.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign(&owner_1_key.private_key, &signing_bytes)
            .expect("resign atomic owner 1"),
    );
}

fn atomic_market_envelope_record(
    leg: &AtomicSwapLeg,
    epoch: u64,
    reserve_packet_hash: &str,
) -> FinalizedMarketOpsEnvelope {
    let mut operation =
        market_ops_operation_fixture(&leg.issuer, &leg.asset_id, reserve_packet_hash);
    operation.envelope.epoch = epoch;
    operation.envelope.supply_packet_hash = market_ops_supply_packet_hash(
        &leg.asset_id,
        epoch,
        operation.envelope.valid_global_supply_atoms,
    )
    .expect("atomic market-ops supply packet hash");
    operation.envelope_hash = bytes_to_hex(&operation.envelope.envelope_hash());
    FinalizedMarketOpsEnvelope {
        asset_id: leg.asset_id.clone(),
        epoch,
        envelope_hash: operation.envelope_hash,
        envelope: operation.envelope,
        policy_inputs: Some(operation.policy_inputs),
        finalized_at_height: 9,
    }
}

fn configure_pf_usdc_a651_market_binding(
    fixture: &mut AtomicExecutionFixture,
    a651_leg_index: usize,
) {
    let (a651_leg, pf_usdc_leg) = match a651_leg_index {
        0 => (
            fixture.transaction.unsigned.leg_0.clone(),
            fixture.transaction.unsigned.leg_1.clone(),
        ),
        1 => (
            fixture.transaction.unsigned.leg_1.clone(),
            fixture.transaction.unsigned.leg_0.clone(),
        ),
        _ => panic!("atomic fixture price-NAV leg index must be 0 or 1"),
    };
    let pf_usdc = NavTrackedAsset::new(
        pf_usdc_leg.asset_id.clone(),
        pf_usdc_leg.issuer.clone(),
        pf_usdc_leg.issuer.clone(),
        "pfusdc-bridge-accounting-profile",
        "usd_e8",
        pf_usdc_leg.issuer.clone(),
    )
    .expect("pfUSDC bridge-accounting NAV record");
    let mut a651 = NavTrackedAsset::new(
        a651_leg.asset_id.clone(),
        a651_leg.issuer.clone(),
        a651_leg.issuer.clone(),
        "a651-price-nav-profile",
        "usd_e8",
        a651_leg.issuer.clone(),
    )
    .expect("a651 price-NAV record");
    a651.finalized_epoch = 1;
    let envelope = atomic_market_envelope_record(&a651_leg, 1, &"ab".repeat(48));

    fixture.ledger.nav_assets = vec![pf_usdc, a651];
    fixture.ledger.market_ops_policies = vec![market_ops_policy_fixture()];
    fixture.ledger.market_ops_envelopes = vec![envelope.clone()];
    fixture.transaction.unsigned.nav_epoch = envelope.epoch;
    fixture.transaction.unsigned.market_envelope_hash = envelope.envelope_hash;
    resign_atomic_swap(
        &mut fixture.transaction,
        &fixture.owner_0_key,
        &fixture.owner_1_key,
    );
}

fn atomic_execution_fixture() -> AtomicExecutionFixture {
    let genesis = Genesis::new("postfiat-local");
    let owner_0_key = ml_dsa_65_keygen().expect("owner 0 keygen");
    let owner_1_key = ml_dsa_65_keygen().expect("owner 1 keygen");
    let issuer_a_key = ml_dsa_65_keygen().expect("issuer a keygen");
    let issuer_b_key = ml_dsa_65_keygen().expect("issuer b keygen");
    let owner_0 = address_from_public_key(&owner_0_key.public_key);
    let owner_1 = address_from_public_key(&owner_1_key.public_key);
    let issuer_a = address_from_public_key(&issuer_a_key.public_key);
    let issuer_b = address_from_public_key(&issuer_b_key.public_key);
    let asset_a = AssetDefinition::new(&genesis.chain_id, &issuer_a, "ASWAPA", 1, 6)
        .expect("asset a");
    let asset_b = AssetDefinition::new(&genesis.chain_id, &issuer_b, "ASWAPB", 1, 6)
        .expect("asset b");
    let (asset_0, asset_1) = if asset_a.asset_id < asset_b.asset_id {
        (asset_a, asset_b)
    } else {
        (asset_b, asset_a)
    };
    let mut ledger = LedgerState::new(vec![
        Account::new(
            owner_0.clone(),
            10_000,
            Some(bytes_to_hex(&owner_0_key.public_key)),
        ),
        Account::new(
            owner_1.clone(),
            10_000,
            Some(bytes_to_hex(&owner_1_key.public_key)),
        ),
        Account::new(
            asset_0.issuer.clone(),
            10_000,
            Some(bytes_to_hex(if asset_0.issuer == issuer_a {
                &issuer_a_key.public_key
            } else {
                &issuer_b_key.public_key
            })),
        ),
        Account::new(
            asset_1.issuer.clone(),
            10_000,
            Some(bytes_to_hex(if asset_1.issuer == issuer_a {
                &issuer_a_key.public_key
            } else {
                &issuer_b_key.public_key
            })),
        ),
    ]);
    ledger.asset_definitions = vec![asset_0.clone(), asset_1.clone()];
    ledger.trustlines = vec![
        atomic_test_line(&owner_0, &asset_0, 20_000),
        atomic_test_line(&owner_1, &asset_1, 164_020),
    ];
    let mut fee_0 = MIN_TRANSFER_FEE;
    let mut fee_1 = MIN_TRANSFER_FEE;
    let mut transaction = None;
    for _ in 0..8 {
        let unsigned = postfiat_types::UnsignedAtomicSwapTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            rfq_hash: "11".repeat(48),
            market_envelope_hash: "00".repeat(48),
            nav_epoch: 0,
            expires_at_height: 100,
            swap_nonce: "33".repeat(48),
            leg_0: AtomicSwapLeg {
                owner: owner_0.clone(),
                recipient: owner_1.clone(),
                issuer: asset_0.issuer.clone(),
                asset_id: asset_0.asset_id.clone(),
                amount: 20_000,
                sequence: 1,
                fee: fee_0,
            },
            leg_1: AtomicSwapLeg {
                owner: owner_1.clone(),
                recipient: owner_0.clone(),
                issuer: asset_1.issuer.clone(),
                asset_id: asset_1.asset_id.clone(),
                amount: 164_020,
                sequence: 1,
                fee: fee_1,
            },
        };
        let candidate = sign_atomic_swap(unsigned, &owner_0_key, &owner_1_key);
        let minimum_0 = minimum_atomic_swap_leg_fee_for_ledger(
            &ledger,
            &candidate,
            &candidate.unsigned.leg_0,
        );
        let minimum_1 = minimum_atomic_swap_leg_fee_for_ledger(
            &ledger,
            &candidate,
            &candidate.unsigned.leg_1,
        );
        transaction = Some(candidate);
        if fee_0 >= minimum_0 && fee_1 >= minimum_1 {
            break;
        }
        fee_0 = minimum_0;
        fee_1 = minimum_1;
    }
    let mut fixture = AtomicExecutionFixture {
        genesis,
        ledger,
        owner_0_key,
        owner_1_key,
        transaction: transaction.expect("atomic transaction converged"),
    };
    configure_pf_usdc_a651_market_binding(&mut fixture, 1);
    fixture
}

fn atomic_line_balance(ledger: &LedgerState, account: &str, asset_id: &str) -> u64 {
    ledger
        .trustline_for_account_asset(account, asset_id)
        .map_or(0, |line| line.balance)
}

fn execute_active_atomic_swap(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAtomicSwapTransaction,
    block_height: u64,
) -> Receipt {
    execute_atomic_swap_transaction_with_compatibility(
        genesis,
        ledger,
        transaction,
        block_height,
        AssetExecutionCompatibility::strict(),
    )
}

#[test]
fn atomic_swap_applies_both_legs_fees_sequences_and_receipt_once() {
    let mut fixture = atomic_execution_fixture();
    for leg in [
        &fixture.transaction.unsigned.leg_0,
        &fixture.transaction.unsigned.leg_1,
    ] {
        assert!(fixture
            .ledger
            .trustline_for_account_asset(&leg.recipient, &leg.asset_id)
            .is_none());
    }
    let before = fixture.ledger.clone();
    let supply_0_before = issued_asset_supply(
        &before,
        &fixture.transaction.unsigned.leg_0.asset_id,
    )
    .expect("asset 0 supply before");
    let supply_1_before = issued_asset_supply(
        &before,
        &fixture.transaction.unsigned.leg_1.asset_id,
    )
    .expect("asset 1 supply before");
    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut fixture.ledger,
        &fixture.transaction,
        10,
    );
    assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
    let leg_0 = &fixture.transaction.unsigned.leg_0;
    let leg_1 = &fixture.transaction.unsigned.leg_1;
    assert_eq!(atomic_line_balance(&fixture.ledger, &leg_0.owner, &leg_0.asset_id), 0);
    assert_eq!(
        atomic_line_balance(&fixture.ledger, &leg_0.recipient, &leg_0.asset_id),
        leg_0.amount
    );
    assert_eq!(atomic_line_balance(&fixture.ledger, &leg_1.owner, &leg_1.asset_id), 0);
    assert_eq!(
        atomic_line_balance(&fixture.ledger, &leg_1.recipient, &leg_1.asset_id),
        leg_1.amount
    );
    for leg in [leg_0, leg_1] {
        assert_eq!(fixture.ledger.account(&leg.owner).expect("owner").sequence, 1);
        assert_eq!(
            fixture.ledger.account(&leg.owner).expect("owner").balance,
            before.account(&leg.owner).expect("owner before").balance - leg.fee
        );
    }
    assert_eq!(receipt.fee_charged, leg_0.fee + leg_1.fee);
    assert_eq!(receipt.fee_burned, receipt.fee_charged);
    assert_eq!(
        receipt.minimum_fee,
        minimum_atomic_swap_leg_fee_for_ledger(&before, &fixture.transaction, leg_0)
            + minimum_atomic_swap_leg_fee_for_ledger(&before, &fixture.transaction, leg_1)
    );
    assert_eq!(receipt.atomic_swap_legs.as_ref().map(Vec::len), Some(2));
    assert_eq!(
        issued_asset_supply(&fixture.ledger, &leg_0.asset_id).expect("asset 0 supply after"),
        supply_0_before
    );
    assert_eq!(
        issued_asset_supply(&fixture.ledger, &leg_1.asset_id).expect("asset 1 supply after"),
        supply_1_before
    );

    let after = fixture.ledger.clone();
    let replay = execute_active_atomic_swap(
        &fixture.genesis,
        &mut fixture.ledger,
        &fixture.transaction,
        11,
    );
    assert!(!replay.accepted);
    assert_eq!(replay.code, "bad_sequence");
    assert_eq!(fixture.ledger, after);
}

#[test]
fn fastswap_prefunded_effects_match_w6_asset_deltas_for_the_same_trade() {
    use crate::fastswap::validate_fastswap_admission;
    use crate::fastswap_bridge::asset_definition_hash;
    use postfiat_types::{
        FastAssetControlStateV1, FastAssetIdV1, FastAssetObjectV1,
        FastAssetRuleHashV1, FastAssetRuleV1, FastLaneStateV1, FastObjectIdV1, FastObjectKeyV1,
        FastObjectOriginV1, FastSwapAuthorizationV1, FastSwapChainDomainV1,
        FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1, FastSwapMarketEnvelopeHashV1,
        FastSwapOpaqueHashV1, FastSwapPartyV1, FastSwapPolicyHashV1, FastSwapPolicySnapshotV1,
        FastSwapQuoteRoundingV1, FastSwapRfqHashV1, SignedFastSwapIntentV1,
        FASTSWAP_INTENT_CONTEXT_V1, FASTSWAP_ML_DSA_65, FASTSWAP_SCHEMA_VERSION_V1,
    };
    use std::collections::{BTreeMap, BTreeSet};

    let mut fixture = atomic_execution_fixture();
    let before = fixture.ledger.clone();
    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut fixture.ledger,
        &fixture.transaction,
        10,
    );
    assert!(receipt.accepted, "W6 fixture must settle");
    let w6 = &fixture.transaction.unsigned;
    let definitions = [&before.asset_definitions[0], &before.asset_definitions[1]];
    let fast_asset = |asset_id: &str| {
        FastAssetIdV1(
            postfiat_crypto_provider::hex_to_bytes(asset_id)
                .expect("asset hex")
                .try_into()
                .expect("asset width"),
        )
    };
    let asset_0 = fast_asset(&w6.leg_0.asset_id);
    let asset_1 = fast_asset(&w6.leg_1.asset_id);
    let definition_0 = definitions
        .iter()
        .find(|definition| definition.asset_id == w6.leg_0.asset_id)
        .expect("definition 0");
    let definition_1 = definitions
        .iter()
        .find(|definition| definition.asset_id == w6.leg_1.asset_id)
        .expect("definition 1");
    let rule = |asset_id: FastAssetIdV1,
                definition: &AssetDefinition,
                issuer_pubkey: Vec<u8>| FastAssetRuleV1 {
        asset_id,
        asset_definition_hash: asset_definition_hash(definition).expect("definition hash"),
        issuer_address: definition.issuer.clone(),
        issuer_control_pubkey: issuer_pubkey,
        requires_authorization: false,
        freeze_enabled: false,
        clawback_enabled: false,
        fast_lane_enabled: true,
        valid_from_height: 1,
        valid_through_height: 100,
    };
    let rule_0 = rule(asset_0, definition_0, vec![31; 64]);
    let rule_1 = rule(asset_1, definition_1, vec![32; 64]);
    let rule_hash_0 = rule_0.rule_hash().expect("rule 0 hash");
    let rule_hash_1 = rule_1.rule_hash().expect("rule 1 hash");
    let domain = FastSwapCommitteeDomainV1 {
        chain: FastSwapChainDomainV1 {
            chain_id: fixture.genesis.chain_id.clone(),
            genesis_hash: FastSwapOpaqueHashV1(
                postfiat_crypto_provider::hex_to_bytes(&genesis_hash(&fixture.genesis))
                    .expect("genesis hex")
                    .try_into()
                    .expect("genesis width"),
            ),
            protocol_version: fixture.genesis.protocol_version,
        },
        fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
        committee_epoch: 1,
        committee_root: FastSwapCommitteeRootV1([40; 48]),
        validator_count: 6,
        quorum: 5,
    };
    let envelope = FastSwapMarketEnvelopeHashV1([41; 48]);
    let mut policy = FastSwapPolicySnapshotV1 {
        domain: domain.chain.clone(),
        policy_epoch: 1,
        policy_hash: FastSwapPolicyHashV1::ZERO,
        pair_asset_0: asset_0,
        pair_asset_1: asset_1,
        asset_rule_hash_0: rule_hash_0,
        asset_rule_hash_1: rule_hash_1,
        price_numerator: u128::from(w6.leg_1.amount),
        price_denominator: u128::from(w6.leg_0.amount),
        rounding: FastSwapQuoteRoundingV1::Exact,
        nav_epoch: w6.nav_epoch,
        market_envelope_hash: envelope,
        valid_from_height: 1,
        valid_through_height: 100,
        fee_schedule_hash: FastSwapOpaqueHashV1([42; 48]),
        max_inputs_per_party: 8,
        max_outputs: 8,
        paused: false,
    };
    policy.policy_hash = policy.computed_hash().expect("policy hash");
    let native = FastAssetIdV1::native_pft();
    let object = |id: u8,
                  owner_pubkey: Vec<u8>,
                  asset_id: FastAssetIdV1,
                  asset_rule_hash: FastAssetRuleHashV1,
                  amount_atoms: u64| FastAssetObjectV1 {
        key: FastObjectKeyV1 {
            object_id: FastObjectIdV1([id; 32]),
            version: 1,
        },
        owner_pubkey,
        asset_id,
        asset_rule_hash,
        amount_atoms,
        control_state: FastAssetControlStateV1::Spendable,
        origin: FastObjectOriginV1::Deposit {
            deposit_id: postfiat_types::FastSwapDepositIdV1([id; 48]),
        },
    };
    let objects = [
        object(
            1,
            fixture.owner_0_key.public_key.clone(),
            asset_0,
            rule_hash_0,
            w6.leg_0.amount,
        ),
        object(
            2,
            fixture.owner_0_key.public_key.clone(),
            native,
            FastAssetRuleHashV1::ZERO,
            1,
        ),
        object(
            3,
            fixture.owner_1_key.public_key.clone(),
            asset_1,
            rule_hash_1,
            w6.leg_1.amount,
        ),
        object(
            4,
            fixture.owner_1_key.public_key.clone(),
            native,
            FastAssetRuleHashV1::ZERO,
            1,
        ),
    ];
    let party_0 = FastSwapPartyV1 {
        owner_address: w6.leg_0.owner.clone(),
        owner_pubkey: fixture.owner_0_key.public_key.clone(),
        offered_asset_id: asset_0,
        offered_asset_rule_hash: rule_hash_0,
        offered_amount: w6.leg_0.amount,
        receives_asset_id: asset_1,
        receives_asset_rule_hash: rule_hash_1,
        receives_holder_permit_id: None,
        receives_amount: w6.leg_1.amount,
        asset_inputs: vec![objects[0].key],
        fee_inputs: vec![objects[1].key],
        asset_change: 0,
        fee_change: 0,
        fee_burn_pft: 1,
    };
    let party_1 = FastSwapPartyV1 {
        owner_address: w6.leg_1.owner.clone(),
        owner_pubkey: fixture.owner_1_key.public_key.clone(),
        offered_asset_id: asset_1,
        offered_asset_rule_hash: rule_hash_1,
        offered_amount: w6.leg_1.amount,
        receives_asset_id: asset_0,
        receives_asset_rule_hash: rule_hash_0,
        receives_holder_permit_id: None,
        receives_amount: w6.leg_0.amount,
        asset_inputs: vec![objects[2].key],
        fee_inputs: vec![objects[3].key],
        asset_change: 0,
        fee_change: 0,
        fee_burn_pft: 1,
    };
    let intent = postfiat_types::FastSwapIntentV1 {
        domain: domain.clone(),
        policy_hash: policy.policy_hash,
        rfq_hash: FastSwapRfqHashV1([43; 48]),
        market_envelope_hash: envelope,
        nav_epoch: w6.nav_epoch,
        expires_at_height: 100,
        nonce: [44; 32],
        party_0,
        party_1,
    };
    let signing_bytes = intent.canonical_bytes().expect("intent bytes");
    let signed = SignedFastSwapIntentV1 {
        intent,
        authorization_0: FastSwapAuthorizationV1 {
            role: 0,
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            public_key: fixture.owner_0_key.public_key.clone(),
            signature: postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                &fixture.owner_0_key.private_key,
                &signing_bytes,
                FASTSWAP_INTENT_CONTEXT_V1,
            )
            .expect("fast signature 0"),
        },
        authorization_1: FastSwapAuthorizationV1 {
            role: 1,
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            public_key: fixture.owner_1_key.public_key.clone(),
            signature: postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                &fixture.owner_1_key.private_key,
                &signing_bytes,
                FASTSWAP_INTENT_CONTEXT_V1,
            )
            .expect("fast signature 1"),
        },
    };
    let state = FastLaneStateV1 {
        schema_version: 1,
        committee: domain,
        objects: objects.into_iter().map(|object| (object.key, object)).collect(),
        reservations: BTreeMap::new(),
        swaps: BTreeMap::new(),
        imported_deposits: BTreeSet::new(),
        exit_claims: BTreeMap::new(),
        terminal_tombstones: BTreeMap::new(),
        asset_rules: BTreeMap::from([(rule_hash_0, rule_0), (rule_hash_1, rule_1)]),
        holder_permits: BTreeMap::new(),
        policy_snapshots: BTreeMap::from([(policy.policy_hash, policy)]),
        prepare_fences: BTreeMap::new(),
        pending_fee_burns: BTreeMap::new(),
        anchored_checkpoints: BTreeSet::new(),
    };
    let effects = validate_fastswap_admission(&state, &signed, 10)
        .expect("FastSwap equivalent admission")
        .effects;

    let fast_received_0 = effects
        .created
        .iter()
        .filter(|output| {
            output.owner_pubkey == fixture.owner_0_key.public_key && output.asset_id == asset_1
        })
        .map(|output| output.amount_atoms)
        .sum::<u64>();
    let fast_received_1 = effects
        .created
        .iter()
        .filter(|output| {
            output.owner_pubkey == fixture.owner_1_key.public_key && output.asset_id == asset_0
        })
        .map(|output| output.amount_atoms)
        .sum::<u64>();
    let w6_received_0 = atomic_line_balance(&fixture.ledger, &w6.leg_1.recipient, &w6.leg_1.asset_id)
        - atomic_line_balance(&before, &w6.leg_1.recipient, &w6.leg_1.asset_id);
    let w6_received_1 = atomic_line_balance(&fixture.ledger, &w6.leg_0.recipient, &w6.leg_0.asset_id)
        - atomic_line_balance(&before, &w6.leg_0.recipient, &w6.leg_0.asset_id);
    assert_eq!((fast_received_0, fast_received_1), (w6_received_0, w6_received_1));
    assert_eq!((fast_received_0, fast_received_1), (w6.leg_1.amount, w6.leg_0.amount));
    assert!(effects.receipt.accepted);
    assert_eq!(effects.receipt.code, "fastswap_applied");
}

#[test]
fn atomic_swap_rejects_domain_activation_expiry_and_issuer_endpoints_without_mutation() {
    let fixture = atomic_execution_fixture();
    let cases: Vec<(&str, SignedAtomicSwapTransaction, u64, AssetExecutionCompatibility)> = vec![
        {
            let mut tx = fixture.transaction.clone();
            tx.unsigned.chain_id.push_str("-wrong");
            resign_atomic_swap(&mut tx, &fixture.owner_0_key, &fixture.owner_1_key);
            ("wrong_chain", tx, 10, AssetExecutionCompatibility::strict())
        },
        {
            let mut tx = fixture.transaction.clone();
            tx.unsigned.genesis_hash = "44".repeat(48);
            resign_atomic_swap(&mut tx, &fixture.owner_0_key, &fixture.owner_1_key);
            ("wrong_genesis", tx, 10, AssetExecutionCompatibility::strict())
        },
        {
            let mut tx = fixture.transaction.clone();
            tx.unsigned.protocol_version += 1;
            resign_atomic_swap(&mut tx, &fixture.owner_0_key, &fixture.owner_1_key);
            ("wrong_protocol_version", tx, 10, AssetExecutionCompatibility::strict())
        },
        {
            let mut tx = fixture.transaction.clone();
            tx.unsigned.address_namespace.push_str(".wrong");
            resign_atomic_swap(&mut tx, &fixture.owner_0_key, &fixture.owner_1_key);
            ("wrong_address_namespace", tx, 10, AssetExecutionCompatibility::strict())
        },
        (
            "atomic_swap_not_active",
            fixture.transaction.clone(),
            10,
            AssetExecutionCompatibility::strict().with_atomic_swap_activation_height(None),
        ),
        (
            "atomic_swap_paused",
            fixture.transaction.clone(),
            10,
            AssetExecutionCompatibility::strict().with_atomic_swap_paused(true),
        ),
        ("swap_expired", fixture.transaction.clone(), 101, AssetExecutionCompatibility::strict()),
        {
            let mut tx = fixture.transaction.clone();
            tx.unsigned.leg_0.issuer = tx.unsigned.leg_0.owner.clone();
            ("issuer_leg_not_supported", tx, 10, AssetExecutionCompatibility::strict())
        },
    ];
    for (expected_code, transaction, height, compatibility) in cases {
        let mut ledger = fixture.ledger.clone();
        let before = ledger.clone();
        let receipt = execute_atomic_swap_transaction_with_compatibility(
            &fixture.genesis,
            &mut ledger,
            &transaction,
            height,
            compatibility,
        );
        assert_eq!(receipt.code, expected_code);
        assert_eq!(ledger, before, "{expected_code} mutated state");
    }

    let compatibility =
        AssetExecutionCompatibility::strict().with_atomic_swap_activation_height(Some(11));
    let mut before_activation = fixture.ledger.clone();
    let receipt = execute_atomic_swap_transaction_with_compatibility(
        &fixture.genesis,
        &mut before_activation,
        &fixture.transaction,
        10,
        compatibility,
    );
    assert_eq!(receipt.code, "atomic_swap_not_active");
    let mut at_activation = fixture.ledger.clone();
    let receipt = execute_atomic_swap_transaction_with_compatibility(
        &fixture.genesis,
        &mut at_activation,
        &fixture.transaction,
        11,
        compatibility,
    );
    assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
    assert!(!compatibility.with_atomic_swap_paused(true).atomic_swap_active(11));
}

#[test]
fn atomic_swap_public_entrypoint_without_activation_context_fails_closed() {
    let fixture = atomic_execution_fixture();
    let mut ledger = fixture.ledger.clone();
    let before = ledger.clone();

    let receipt = execute_atomic_swap_transaction(
        &fixture.genesis,
        &mut ledger,
        &fixture.transaction,
        10,
    );

    assert_eq!(receipt.code, "atomic_swap_not_active");
    assert_eq!(ledger, before);
}

#[test]
fn atomic_swap_rejects_each_owner_auth_sequence_fee_and_line_failure_atomically() {
    let fixture = atomic_execution_fixture();
    let mut cases: Vec<(&str, SignedAtomicSwapTransaction, LedgerState)> = Vec::new();
    for authorization in 0..2 {
        let mut tx = fixture.transaction.clone();
        if authorization == 0 {
            tx.authorization_0.signature_hex.replace_range(0..2, "00");
        } else {
            tx.authorization_1.signature_hex.replace_range(0..2, "00");
        }
        cases.push(("bad_signature", tx, fixture.ledger.clone()));
    }
    for authorization in 0..2 {
        let mut tx = fixture.transaction.clone();
        let signing_bytes = tx.unsigned.signing_bytes();
        if authorization == 0 {
            tx.authorization_0.public_key_hex = bytes_to_hex(&fixture.owner_1_key.public_key);
            tx.authorization_0.signature_hex = bytes_to_hex(
                &ml_dsa_65_sign(&fixture.owner_1_key.private_key, &signing_bytes)
                    .expect("wrong owner 0 signature"),
            );
        } else {
            tx.authorization_1.public_key_hex = bytes_to_hex(&fixture.owner_0_key.public_key);
            tx.authorization_1.signature_hex = bytes_to_hex(
                &ml_dsa_65_sign(&fixture.owner_0_key.private_key, &signing_bytes)
                    .expect("wrong owner 1 signature"),
            );
        }
        cases.push(("sender_mismatch", tx, fixture.ledger.clone()));
    }
    for leg in 0..2 {
        let mut tx = fixture.transaction.clone();
        if leg == 0 {
            tx.unsigned.leg_0.sequence += 1;
        } else {
            tx.unsigned.leg_1.sequence += 1;
        }
        resign_atomic_swap(&mut tx, &fixture.owner_0_key, &fixture.owner_1_key);
        cases.push(("bad_sequence", tx, fixture.ledger.clone()));
    }
    for leg in 0..2 {
        let mut tx = fixture.transaction.clone();
        if leg == 0 {
            tx.unsigned.leg_0.fee = 0;
        } else {
            tx.unsigned.leg_1.fee = 0;
        }
        resign_atomic_swap(&mut tx, &fixture.owner_0_key, &fixture.owner_1_key);
        cases.push(("fee_too_low", tx, fixture.ledger.clone()));
    }
    for leg in 0..2 {
        let tx = fixture.transaction.clone();
        let owner = if leg == 0 { &tx.unsigned.leg_0.owner } else { &tx.unsigned.leg_1.owner };
        let fee = if leg == 0 { tx.unsigned.leg_0.fee } else { tx.unsigned.leg_1.fee };
        let mut ledger = fixture.ledger.clone();
        ledger.account_mut(owner).expect("owner").balance = fee - 1;
        cases.push(("insufficient_funds", tx, ledger));
    }
    for leg in 0..2 {
        let tx = fixture.transaction.clone();
        let owner = if leg == 0 { &tx.unsigned.leg_0.owner } else { &tx.unsigned.leg_1.owner };
        let fee = if leg == 0 { tx.unsigned.leg_0.fee } else { tx.unsigned.leg_1.fee };
        let mut ledger = fixture.ledger.clone();
        ledger.account_mut(owner).expect("owner").balance = fee + ACCOUNT_RESERVE - 1;
        cases.push(("below_account_reserve", tx, ledger));
    }
    for leg in 0..2 {
        let tx = fixture.transaction.clone();
        let owner = if leg == 0 { &tx.unsigned.leg_0.owner } else { &tx.unsigned.leg_1.owner };
        let asset_id = if leg == 0 { &tx.unsigned.leg_0.asset_id } else { &tx.unsigned.leg_1.asset_id };
        let mut ledger = fixture.ledger.clone();
        ledger
            .trustlines
            .iter_mut()
            .find(|line| line.account == *owner && line.asset_id == *asset_id)
            .expect("line")
            .frozen = true;
        cases.push(("asset_balance_frozen", tx, ledger));
    }
    for leg in 0..2 {
        let tx = fixture.transaction.clone();
        let owner = if leg == 0 { &tx.unsigned.leg_0.owner } else { &tx.unsigned.leg_1.owner };
        let asset_id = if leg == 0 { &tx.unsigned.leg_0.asset_id } else { &tx.unsigned.leg_1.asset_id };
        let mut ledger = fixture.ledger.clone();
        ledger
            .asset_definitions
            .iter_mut()
            .find(|asset| asset.asset_id == *asset_id)
            .expect("asset")
            .requires_authorization = true;
        ledger
            .trustlines
            .iter_mut()
            .find(|line| line.account == *owner && line.asset_id == *asset_id)
            .expect("line")
            .authorized = false;
        cases.push(("asset_transfer_not_authorized", tx, ledger));
    }
    for leg in 0..2 {
        let tx = fixture.transaction.clone();
        let owner = if leg == 0 { &tx.unsigned.leg_0.owner } else { &tx.unsigned.leg_1.owner };
        let mut ledger = fixture.ledger.clone();
        ledger.account_mut(owner).expect("owner").public_key_hex = Some("00".to_string());
        cases.push(("sender_key_mismatch", tx, ledger));
    }
    for (expected_code, transaction, mut ledger) in cases {
        let before = ledger.clone();
        let receipt = execute_active_atomic_swap(
            &fixture.genesis,
            &mut ledger,
            &transaction,
            10,
        );
        assert_eq!(receipt.code, expected_code);
        assert!(!receipt.code.contains("trustline"));
        assert!(!receipt.message.contains("trustline"));
        assert_eq!(ledger, before, "{expected_code} mutated state");
    }
}

#[test]
fn atomic_swap_rejects_sequence_and_balance_overflow_without_mutation() {
    let fixture = atomic_execution_fixture();
    let mut sequence_ledger = fixture.ledger.clone();
    sequence_ledger
        .account_mut(&fixture.transaction.unsigned.leg_0.owner)
        .expect("owner")
        .sequence = u64::MAX;
    let before = sequence_ledger.clone();
    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut sequence_ledger,
        &fixture.transaction,
        10,
    );
    assert_eq!(receipt.code, "sequence_overflow");
    assert_eq!(sequence_ledger, before);

    let mut tx = fixture.transaction.clone();
    tx.unsigned.leg_0.amount = 1;
    resign_atomic_swap(&mut tx, &fixture.owner_0_key, &fixture.owner_1_key);
    let mut balance_ledger = fixture.ledger.clone();
    let recipient_asset = balance_ledger
        .asset_definitions
        .iter()
        .find(|asset| asset.asset_id == tx.unsigned.leg_0.asset_id)
        .cloned()
        .expect("recipient asset");
    let mut recipient_line =
        atomic_test_line(&tx.unsigned.leg_0.recipient, &recipient_asset, u64::MAX);
    recipient_line.balance = u64::MAX;
    recipient_line.limit = u64::MAX;
    balance_ledger.trustlines.push(recipient_line);
    let before = balance_ledger.clone();
    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut balance_ledger,
        &tx,
        10,
    );
    assert_eq!(receipt.code, "issued_balance_overflow");
    assert_eq!(balance_ledger, before);

    let mut fee_tx = fixture.transaction.clone();
    fee_tx.unsigned.leg_0.fee = u64::MAX / 2 + 1;
    fee_tx.unsigned.leg_1.fee = u64::MAX / 2 + 1;
    resign_atomic_swap(&mut fee_tx, &fixture.owner_0_key, &fixture.owner_1_key);
    let mut fee_ledger = fixture.ledger.clone();
    fee_ledger
        .account_mut(&fee_tx.unsigned.leg_0.owner)
        .expect("owner 0")
        .balance = u64::MAX;
    fee_ledger
        .account_mut(&fee_tx.unsigned.leg_1.owner)
        .expect("owner 1")
        .balance = u64::MAX;
    let before = fee_ledger.clone();
    let receipt =
        execute_active_atomic_swap(&fixture.genesis, &mut fee_ledger, &fee_tx, 10);
    assert_eq!(receipt.code, "fee_overflow");
    assert_eq!(fee_ledger, before);
}

#[test]
fn atomic_swap_fee_accounts_for_implicit_recipient_balance_row() {
    let mut fixture = atomic_execution_fixture();
    let leg = fixture.transaction.unsigned.leg_0.clone();
    let mut ledger = fixture.ledger.clone();
    ledger
        .trustlines
        .retain(|line| !(line.account == leg.recipient && line.asset_id == leg.asset_id));
    assert_eq!(
        atomic_swap_leg_state_expansion_fee(&ledger, &leg),
        TRUSTLINE_STATE_EXPANSION_FEE
    );
    assert_eq!(
        minimum_atomic_swap_leg_fee_for_ledger(&ledger, &fixture.transaction, &leg),
        minimum_atomic_swap_fee(&fixture.transaction) + TRUSTLINE_STATE_EXPANSION_FEE
    );

    fixture.transaction.unsigned.leg_0.fee = minimum_atomic_swap_leg_fee_for_ledger(
        &ledger,
        &fixture.transaction,
        &fixture.transaction.unsigned.leg_0,
    );
    resign_atomic_swap(
        &mut fixture.transaction,
        &fixture.owner_0_key,
        &fixture.owner_1_key,
    );
    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut ledger,
        &fixture.transaction,
        10,
    );
    assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
    assert_eq!(
        atomic_line_balance(&ledger, &leg.recipient, &leg.asset_id),
        leg.amount
    );
    let implicit_row = ledger
        .trustline_for_account_asset(&leg.recipient, &leg.asset_id)
        .expect("implicit recipient balance row");
    assert!(implicit_row.authorized);
    assert!(!implicit_row.frozen);
    assert_eq!(implicit_row.reserve_paid, 0);
    assert_eq!(
        receipt.state_expansion_fee,
        TRUSTLINE_STATE_EXPANSION_FEE * 2
    );
}

#[test]
fn atomic_swap_never_auto_creates_a_missing_sender_balance_row() {
    let fixture = atomic_execution_fixture();
    let leg = fixture.transaction.unsigned.leg_0.clone();
    let mut ledger = fixture.ledger.clone();
    ledger
        .trustlines
        .retain(|line| !(line.account == leg.owner && line.asset_id == leg.asset_id));
    let before = ledger.clone();

    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut ledger,
        &fixture.transaction,
        10,
    );

    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "missing_asset_balance");
    assert!(!receipt.code.contains("trustline"));
    assert!(!receipt.message.contains("trustline"));
    assert_eq!(ledger, before);
}

#[test]
fn atomic_swap_accepts_pf_usdc_a651_with_price_nav_in_either_canonical_leg() {
    for a651_leg_index in [0, 1] {
        let mut fixture = atomic_execution_fixture();
        configure_pf_usdc_a651_market_binding(&mut fixture, a651_leg_index);
        assert_eq!(fixture.ledger.nav_assets.len(), 2);
        assert_eq!(fixture.ledger.market_ops_envelopes.len(), 1);

        let price_leg = if a651_leg_index == 0 {
            &fixture.transaction.unsigned.leg_0
        } else {
            &fixture.transaction.unsigned.leg_1
        };
        assert_eq!(
            fixture.ledger.market_ops_envelopes[0].asset_id,
            price_leg.asset_id
        );

        let receipt = execute_active_atomic_swap(
            &fixture.genesis,
            &mut fixture.ledger,
            &fixture.transaction,
            10,
        );
        assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
    }
}

#[test]
fn atomic_swap_classifies_price_nav_once_per_leg_not_per_envelope_row() {
    let mut fixture = atomic_execution_fixture();
    let price_leg = fixture.transaction.unsigned.leg_1.clone();
    let current = atomic_market_envelope_record(&price_leg, 2, &"cd".repeat(48));
    fixture
        .ledger
        .nav_asset_mut(&price_leg.asset_id)
        .expect("a651 NAV record")
        .finalized_epoch = current.epoch;
    fixture.ledger.market_ops_envelopes.push(current.clone());
    fixture.transaction.unsigned.nav_epoch = current.epoch;
    fixture.transaction.unsigned.market_envelope_hash = current.envelope_hash;
    resign_atomic_swap(
        &mut fixture.transaction,
        &fixture.owner_0_key,
        &fixture.owner_1_key,
    );

    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut fixture.ledger,
        &fixture.transaction,
        10,
    );
    assert!(receipt.accepted, "{}: {}", receipt.code, receipt.message);
}

#[test]
fn atomic_swap_rejects_zero_or_two_price_nav_legs_without_mutation() {
    let fixture = atomic_execution_fixture();

    let mut zero_price_ledger = fixture.ledger.clone();
    zero_price_ledger.market_ops_envelopes.clear();
    let before = zero_price_ledger.clone();
    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut zero_price_ledger,
        &fixture.transaction,
        10,
    );
    assert_eq!(receipt.code, "wrong_market_envelope");
    assert!(receipt.message.contains("exactly one price-NAV leg"));
    assert_eq!(zero_price_ledger, before);

    let mut two_price_ledger = fixture.ledger.clone();
    let bridge_leg = fixture.transaction.unsigned.leg_0.clone();
    two_price_ledger
        .nav_asset_mut(&bridge_leg.asset_id)
        .expect("pfUSDC bridge-accounting NAV record")
        .finalized_epoch = fixture.transaction.unsigned.nav_epoch;
    two_price_ledger
        .market_ops_envelopes
        .push(atomic_market_envelope_record(
            &bridge_leg,
            fixture.transaction.unsigned.nav_epoch,
            &"ef".repeat(48),
        ));
    let before = two_price_ledger.clone();
    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut two_price_ledger,
        &fixture.transaction,
        10,
    );
    assert_eq!(receipt.code, "nav_pair_not_supported");
    assert_eq!(two_price_ledger, before);
}

#[test]
fn atomic_swap_requires_current_exact_price_nav_envelope_without_mutation() {
    let fixture = atomic_execution_fixture();

    let mut wrong_epoch = fixture.transaction.clone();
    wrong_epoch.unsigned.nav_epoch += 1;
    resign_atomic_swap(&mut wrong_epoch, &fixture.owner_0_key, &fixture.owner_1_key);
    let mut ledger = fixture.ledger.clone();
    let before = ledger.clone();
    let receipt = execute_active_atomic_swap(&fixture.genesis, &mut ledger, &wrong_epoch, 10);
    assert_eq!(receipt.code, "wrong_nav_epoch");
    assert_eq!(ledger, before);

    let mut wrong_hash = fixture.transaction.clone();
    wrong_hash.unsigned.market_envelope_hash = "ff".repeat(48);
    resign_atomic_swap(&mut wrong_hash, &fixture.owner_0_key, &fixture.owner_1_key);
    let mut ledger = fixture.ledger.clone();
    let before = ledger.clone();
    let receipt = execute_active_atomic_swap(&fixture.genesis, &mut ledger, &wrong_hash, 10);
    assert_eq!(receipt.code, "wrong_market_envelope");
    assert_eq!(ledger, before);

    let price_asset_id = fixture.transaction.unsigned.leg_1.asset_id.clone();
    let mut absent_exact_ledger = fixture.ledger.clone();
    absent_exact_ledger
        .nav_asset_mut(&price_asset_id)
        .expect("a651 NAV record")
        .finalized_epoch += 1;
    let mut absent_exact = fixture.transaction.clone();
    absent_exact.unsigned.nav_epoch += 1;
    resign_atomic_swap(
        &mut absent_exact,
        &fixture.owner_0_key,
        &fixture.owner_1_key,
    );
    let before = absent_exact_ledger.clone();
    let receipt = execute_active_atomic_swap(
        &fixture.genesis,
        &mut absent_exact_ledger,
        &absent_exact,
        10,
    );
    assert_eq!(receipt.code, "wrong_market_envelope");
    assert!(receipt.message.contains("has no finalized market-ops envelope"));
    assert_eq!(absent_exact_ledger, before);
}
