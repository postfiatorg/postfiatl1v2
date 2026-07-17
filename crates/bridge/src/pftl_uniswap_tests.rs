use super::*;
use postfiat_crypto_provider::{
    bytes_to_hex, hash_bytes, ml_dsa_65_keygen, ml_dsa_65_sign_with_context_seed,
};

const TEST_CHAIN_ID: &str = "postfiat-local";
const TEST_GENESIS_HASH: &str =
        "97982d730c6adadfa21b7662bfe12d8ca69b4192bba0f4905e4090acc441d572fd17a81f0c23ff7bc8ccd7c4091aa04a";
const TEST_PROTOCOL_VERSION: u32 = 1;

fn test_chain_domain() -> BridgeWitnessChainDomain<'static> {
    BridgeWitnessChainDomain {
        chain_id: TEST_CHAIN_ID,
        genesis_hash: TEST_GENESIS_HASH,
        protocol_version: TEST_PROTOCOL_VERSION,
    }
}

fn attested_request(
    domain: &BridgeDomain,
    direction: &str,
    from: &str,
    to: &str,
    amount: u64,
    witness_id: &str,
    witness_epoch: u32,
) -> BridgeTransferRequest {
    let mut request = BridgeTransferRequest {
        domain_id: domain.domain_id.clone(),
        direction: direction.to_string(),
        from: from.to_string(),
        to: to.to_string(),
        asset_id: "XRP".to_string(),
        amount,
        witness_id: witness_id.to_string(),
        witness_epoch,
        witness_attestation: None,
    };
    let key_pair = ml_dsa_65_keygen().expect("witness keygen");
    let signer = "test-bridge-witness";
    let public_key_hex = bytes_to_hex(&key_pair.public_key);
    let message = bridge_witness_attestation_message(
        test_chain_domain(),
        domain,
        &request,
        signer,
        ML_DSA_65_ALGORITHM,
        &public_key_hex,
    )
    .expect("witness message");
    let signature_seed = bridge_witness_signature_seed(&message);
    let signature = ml_dsa_65_sign_with_context_seed(
        &key_pair.private_key,
        &message,
        BRIDGE_WITNESS_SIGNATURE_CONTEXT,
        &signature_seed,
    )
    .expect("witness sign");
    let attestation_id = bridge_witness_attestation_id(
        test_chain_domain(),
        domain,
        &request,
        signer,
        ML_DSA_65_ALGORITHM,
        &public_key_hex,
    )
    .expect("witness attestation id");
    request.witness_attestation = Some(BridgeWitnessAttestation {
        attestation_id,
        chain_id: TEST_CHAIN_ID.to_string(),
        genesis_hash: TEST_GENESIS_HASH.to_string(),
        protocol_version: TEST_PROTOCOL_VERSION,
        signer: signer.to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex,
        signature_hex: bytes_to_hex(&signature),
    });
    request
}

fn bridge_witness_signature_seed(message: &[u8]) -> [u8; 32] {
    let digest = hash_bytes("postfiat.bridge_witness.signature_seed.v1", message);
    digest[..32].try_into().expect("seed length")
}

fn pftl_uniswap_config() -> PftlUniswapRouteConfig {
    PftlUniswapRouteConfig {
        schema: "postfiat-pftl-uniswap-route-config-v1".to_string(),
        route_id: "pftl-a666-ethereum-wA666-usdc-v1".to_string(),
        route_family: PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
        native_nav_asset_id: "a".repeat(96),
        settlement_asset_id: "8".repeat(96),
        wrapped_navcoin_token: "0x1111111111111111111111111111111111111111".to_string(),
        handoff_controller: "0x2222222222222222222222222222222222222222".to_string(),
        settlement_adapter: "0x3333333333333333333333333333333333333333".to_string(),
        verifier_mode: "threshold-controlled".to_string(),
        route_trust_class: ROUTE_TRUST_CLASS_CONTROLLED.to_string(),
        uniswap_pool_id_or_path:
            "0x4444444444444444444444444444444444444444444444444444444444444444".to_string(),
        router: "0x5555555555555555555555555555555555555555".to_string(),
        failure_behavior: "refund_unconsumed_pftl_packet".to_string(),
        route_supply_cap_atoms: 10_000_000,
        packet_notional_cap_atoms: 1_000_000,
        seed_nav_epoch: 7,
        seed_usdc_atoms: 100_000_000,
        seed_wrapped_navcoin_atoms: 100_000,
        lp_recipient: "0x6666666666666666666666666666666666666666".to_string(),
        lp_custody_policy: "controlled_launch_lp".to_string(),
    }
}

fn official_uniswap_deployments() -> PftlUniswapOfficialUniswapV4Deployments {
    PftlUniswapOfficialUniswapV4Deployments {
        chain_id: 1,
        deployments_source_url: "https://developers.uniswap.org/docs/protocols/v4/deployments"
            .to_string(),
        deployments_table_hash: "1".repeat(64),
        checked_at_utc: "2026-07-01T00:00:00Z".to_string(),
        pool_manager: "0x7777777777777777777777777777777777777777".to_string(),
        position_manager: "0x8888888888888888888888888888888888888888".to_string(),
        universal_router: "0x9999999999999999999999999999999999999999".to_string(),
        permit2: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        state_view: "0xabababababababababababababababababababab".to_string(),
    }
}

fn pftl_uniswap_launch_config(route_config: &PftlUniswapRouteConfig) -> PftlUniswapLaunchConfig {
    PftlUniswapLaunchConfig {
        schema: "postfiat-pftl-uniswap-launch-config-v1".to_string(),
        route_id: route_config.route_id.clone(),
        route_config_digest: pftl_uniswap_route_config_digest(route_config)
            .expect("route config digest"),
        route_trust_class: route_config.route_trust_class.clone(),
        native_nav_asset_id: route_config.native_nav_asset_id.clone(),
        settlement_asset_id: route_config.settlement_asset_id.clone(),
        wrapped_navcoin_token: route_config.wrapped_navcoin_token.clone(),
        usdc_token: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
        handoff_controller: route_config.handoff_controller.clone(),
        receipt_verifier: "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        settlement_adapter: route_config.settlement_adapter.clone(),
        official_uniswap: official_uniswap_deployments(),
        uniswap_pool_key_hash: "2".repeat(64),
        uniswap_pool_id: route_config.uniswap_pool_id_or_path.clone(),
        seed: PftlUniswapPoolSeedConfig {
            pricing_nav_epoch: route_config.seed_nav_epoch,
            pricing_reserve_packet_hash: "e".repeat(96),
            seed_usdc_atoms: route_config.seed_usdc_atoms,
            seed_wrapped_navcoin_atoms: route_config.seed_wrapped_navcoin_atoms,
            nav_price_settlement_atoms_per_nav_atom: 1_000,
            tick_lower: -120,
            tick_upper: 120,
            fee_pips: 3_000,
            lp_recipient: route_config.lp_recipient.clone(),
            position_recipient: "0xcccccccccccccccccccccccccccccccccccccccc".to_string(),
            lp_custody_policy: route_config.lp_custody_policy.clone(),
        },
        fork_rehearsal_required: true,
    }
}

fn pftl_uniswap_fork_rehearsal_evidence(
    launch_config: &PftlUniswapLaunchConfig,
) -> PftlUniswapForkRehearsalEvidence {
    PftlUniswapForkRehearsalEvidence {
        schema: "postfiat-pftl-uniswap-fork-rehearsal-evidence-v1".to_string(),
        rehearsal_id: "gate-3-fork-rehearsal-001".to_string(),
        launch_config_digest: pftl_uniswap_launch_config_digest(launch_config)
            .expect("launch config digest"),
        route_config_digest: launch_config.route_config_digest.clone(),
        fork_chain_id: launch_config.official_uniswap.chain_id,
        fork_block_number: 22_000_000,
        official_uniswap: launch_config.official_uniswap.clone(),
        uniswap_pool_key_hash: launch_config.uniswap_pool_key_hash.clone(),
        uniswap_pool_id: launch_config.uniswap_pool_id.clone(),
        seed_export_packet_hash: "3".repeat(96),
        seed_receipt_root: "4".repeat(96),
        seed_mint_tx_hash: "5".repeat(64),
        seed_lp_tx_hash: "6".repeat(64),
        external_buy_tx_hash: "7".repeat(64),
        external_sell_tx_hash: "8".repeat(64),
        mint_only_packet_tx_hash: "9".repeat(64),
        mint_and_swap_packet_tx_hash: "a".repeat(64),
        state_view_liquidity_after_seed: 1_000_000,
        state_view_liquidity_after_buy: 1_000_100,
        state_view_liquidity_after_sell: 999_900,
        user_buy_usdc_spent_atoms: 10_000,
        user_buy_wrapped_received_atoms: 9,
        user_sell_wrapped_spent_atoms: 5,
        user_sell_usdc_received_atoms: 4_900,
        canonical_supply_before_external_trades_atoms: 100_000,
        canonical_supply_after_external_trades_atoms: 100_000,
        packet_consumed_without_manual_mint: true,
        min_output_failure_reverted_without_consume: true,
    }
}

fn pftl_uniswap_packet(config: &PftlUniswapRouteConfig) -> PftlUniswapMintAndSwapPacket {
    PftlUniswapMintAndSwapPacket {
        schema: "postfiat-pftl-uniswap-mint-and-swap-packet-v1".to_string(),
        route_id: config.route_id.clone(),
        config_digest: pftl_uniswap_route_config_digest(config).expect("config digest"),
        source_packet_hash: "d".repeat(96),
        source_receipt_hash: "c".repeat(96),
        source_receipt_root: "b".repeat(96),
        source_wallet: "pf124071fd53a12ca4556b7aa1f5ec98b585e73468".to_string(),
        settlement_asset_id: config.settlement_asset_id.clone(),
        native_nav_asset_id: config.native_nav_asset_id.clone(),
        wrapped_navcoin_token: config.wrapped_navcoin_token.clone(),
        ethereum_recipient: "0x6666666666666666666666666666666666666666".to_string(),
        token_out: "0x7777777777777777777777777777777777777777".to_string(),
        settlement_amount_atoms: 500_000,
        mint_amount_atoms: 50_000,
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: "e".repeat(96),
        uniswap_pool_id_or_path: config.uniswap_pool_id_or_path.clone(),
        swap_path_hash: "9".repeat(64),
        router: config.router.clone(),
        minimum_output_atoms: 490_000,
        deadline_seconds: 1_924_992_000,
        nonce: "f".repeat(64),
    }
}

fn pftl_uniswap_ledger(config: &PftlUniswapRouteConfig) -> PftlUniswapBridgeLedger {
    pftl_uniswap_bridge_ledger_from_config(config, 1, 7, 64).expect("bridge ledger")
}

fn primary_subscription_request(
    config: &PftlUniswapRouteConfig,
    nonce: &str,
    settlement_value_atoms: u64,
    nav_price_settlement_atoms_per_nav_atom: u64,
    pricing_nav_epoch: u64,
) -> PftlUniswapPrimarySubscriptionRequest {
    PftlUniswapPrimarySubscriptionRequest {
        route_id: config.route_id.clone(),
        source_wallet: "pf124071fd53a12ca4556b7aa1f5ec98b585e73468".to_string(),
        settlement_asset_id: config.settlement_asset_id.clone(),
        subscription_nonce: nonce.to_string(),
        quote: PrimarySubscriptionQuoteInput {
            settlement_value_atoms,
            nav_price_settlement_atoms_per_nav_atom,
            pricing_nav_epoch,
            pricing_reserve_packet_hash: "e".repeat(96),
        },
    }
}

fn export_request(
    config: &PftlUniswapRouteConfig,
    packet_hash: &str,
    nonce: &str,
    amount_atoms: u64,
    source_height: u64,
) -> PftlUniswapExportDebitRequest {
    PftlUniswapExportDebitRequest {
        route_id: config.route_id.clone(),
        packet_hash: packet_hash.to_string(),
        nonce: nonce.to_string(),
        source_wallet: "pf124071fd53a12ca4556b7aa1f5ec98b585e73468".to_string(),
        ethereum_recipient: "0x6666666666666666666666666666666666666666".to_string(),
        amount_atoms,
        source_height,
        destination_deadline_seconds: 1_924_992_000,
        refund_not_before_height: source_height + 10,
    }
}

fn return_burn_request(
    config: &PftlUniswapRouteConfig,
    burn_event_hash: &str,
    amount_atoms: u64,
    burn_height: u64,
) -> PftlUniswapReturnBurnRequest {
    let mut request = PftlUniswapReturnBurnRequest {
        burn_event_hash: "0".repeat(64),
        ethereum_chain_id: 1,
        bridge_controller: config.handoff_controller.clone(),
        wrapped_navcoin_token: config.wrapped_navcoin_token.clone(),
        native_nav_asset_id: config.native_nav_asset_id.clone(),
        ethereum_sender: "0x6666666666666666666666666666666666666666".to_string(),
        pftl_recipient: "pf124071fd53a12ca4556b7aa1f5ec98b585e73468".to_string(),
        amount_atoms,
        return_nonce: burn_event_hash.to_string(),
        burn_height,
        finalized_height: burn_height + 64,
    };
    request.burn_event_hash = pftl_uniswap_return_burn_id(&request).expect("return burn id");
    request
}

#[test]
fn primary_subscription_quote_uses_floor_and_reports_dust() {
    let quote = primary_subscription_quote(PrimarySubscriptionQuoteInput {
        settlement_value_atoms: 100_005,
        nav_price_settlement_atoms_per_nav_atom: 1_000,
        pricing_nav_epoch: 7,
        pricing_reserve_packet_hash: "e".repeat(96),
    })
    .expect("primary quote");

    assert_eq!(quote.route_family, "primary_pftl_mint");
    assert_eq!(quote.supply_effect, "mints_new_native_navcoin_supply");
    assert_eq!(quote.pricing_source, "finalized_pre_inflow_nav_snapshot");
    assert_eq!(
        quote.settlement_reserve_effect,
        "accepted_settlement_added_after_primary_fill"
    );
    assert_eq!(quote.requested_settlement_atoms, 100_005);
    assert_eq!(quote.accepted_settlement_atoms, 100_005);
    assert_eq!(quote.refund_settlement_atoms, 0);
    assert_eq!(quote.minted_nav_atoms, 100);
    assert_eq!(quote.dust_settlement_atoms, 5);
    assert_eq!(
        quote.rounding_rule,
        PRIMARY_SUBSCRIPTION_ROUNDING_RULE_FLOOR_RESERVE_KEEPS_DUST
    );
}

#[test]
fn primary_subscription_receipt_binds_requested_accepted_refund_and_rounding() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    let (quote, receipt) = pftl_uniswap_apply_primary_subscription_with_receipt(
        &mut ledger,
        primary_subscription_request(&config, &"8".repeat(64), 100_005, 1_000, 7),
    )
    .expect("primary subscription receipt");

    assert_eq!(quote.requested_settlement_atoms, 100_005);
    assert_eq!(quote.accepted_settlement_atoms, 100_005);
    assert_eq!(quote.refund_settlement_atoms, 0);
    assert_eq!(receipt.transition, "primary_subscription");
    assert_eq!(receipt.requested_settlement_atoms, Some(100_005));
    assert_eq!(receipt.accepted_settlement_atoms, Some(100_005));
    assert_eq!(receipt.refund_settlement_atoms, Some(0));
    assert_eq!(receipt.minted_nav_atoms, Some(100));
    assert_eq!(receipt.amount_atoms, receipt.minted_nav_atoms);
    assert_eq!(
        receipt.settlement_amount_atoms,
        receipt.accepted_settlement_atoms
    );
    assert_eq!(receipt.nav_price_settlement_atoms_per_nav_atom, Some(1_000));
    assert_eq!(
        receipt.rounding_rule.as_deref(),
        Some(PRIMARY_SUBSCRIPTION_ROUNDING_RULE_FLOOR_RESERVE_KEEPS_DUST)
    );

    let mut missing = receipt.clone();
    missing.requested_settlement_atoms = None;
    let error = validate_pftl_uniswap_transition_receipt(&missing)
        .expect_err("missing requested amount must fail");
    assert_eq!(error.code(), "missing_primary_receipt_field");

    let mut mismatched = receipt;
    mismatched.settlement_amount_atoms = Some(100_004);
    let error = validate_pftl_uniswap_transition_receipt(&mismatched)
        .expect_err("alias mismatch must fail");
    assert_eq!(error.code(), "primary_receipt_alias_mismatch");
}

#[test]
fn pftl_uniswap_route_config_digest_is_stable_and_validated() {
    let config = pftl_uniswap_config();
    let digest = pftl_uniswap_route_config_digest(&config).expect("digest");
    assert_eq!(digest.len(), 96);
    assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));

    let second = pftl_uniswap_route_config_digest(&config).expect("digest");
    assert_eq!(digest, second);
}

#[test]
fn pftl_uniswap_launch_config_binds_route_seed_and_official_deployments() {
    let route_config = pftl_uniswap_config();
    let launch_config = pftl_uniswap_launch_config(&route_config);

    let digest = pftl_uniswap_launch_config_digest(&launch_config).expect("launch digest");
    assert_eq!(digest.len(), 96);
    validate_pftl_uniswap_launch_config_against_route(&launch_config, &route_config)
        .expect("launch config matches route");

    let mut bad_seed = launch_config.clone();
    bad_seed.seed.seed_wrapped_navcoin_atoms += 1;
    let error = pftl_uniswap_launch_config_digest(&bad_seed).expect_err("bad seed math must fail");
    assert_eq!(error.code(), "bad_seed_nav_amount");

    let mut bad_route = route_config.clone();
    bad_route.lp_recipient = "0xdddddddddddddddddddddddddddddddddddddddd".to_string();
    let error = validate_pftl_uniswap_launch_config_against_route(&launch_config, &bad_route)
        .expect_err("route mismatch must fail");
    assert_eq!(error.code(), "launch_route_config_mismatch");
}

#[test]
fn pftl_uniswap_packet_binds_launch_pricing_epoch_and_reserve_packet() {
    let route_config = pftl_uniswap_config();
    let launch_config = pftl_uniswap_launch_config(&route_config);
    let mut packet = pftl_uniswap_packet(&route_config);
    packet.token_out = launch_config.usdc_token.clone();

    validate_pftl_uniswap_packet_against_launch_config(&packet, &launch_config)
        .expect("packet matches launch config");

    let mut wrong_epoch = packet.clone();
    wrong_epoch.pricing_nav_epoch += 1;
    let error = validate_pftl_uniswap_packet_against_launch_config(&wrong_epoch, &launch_config)
        .expect_err("wrong pricing epoch must fail");
    assert_eq!(error.code(), "launch_pricing_nav_epoch_mismatch");

    let mut wrong_reserve = packet.clone();
    wrong_reserve.pricing_reserve_packet_hash = "9".repeat(96);
    let error = validate_pftl_uniswap_packet_against_launch_config(&wrong_reserve, &launch_config)
        .expect_err("wrong pricing reserve packet must fail");
    assert_eq!(error.code(), "launch_pricing_reserve_packet_mismatch");

    let mut wrong_output = packet;
    wrong_output.token_out = "0x7777777777777777777777777777777777777777".to_string();
    let error = validate_pftl_uniswap_packet_against_launch_config(&wrong_output, &launch_config)
        .expect_err("wrong output token must fail");
    assert_eq!(error.code(), "launch_packet_config_mismatch");
}

#[test]
fn pftl_uniswap_fork_rehearsal_evidence_requires_tradeability_and_supply_invariant() {
    let route_config = pftl_uniswap_config();
    let launch_config = pftl_uniswap_launch_config(&route_config);
    let evidence = pftl_uniswap_fork_rehearsal_evidence(&launch_config);

    let digest = pftl_uniswap_fork_rehearsal_evidence_digest(&evidence, &launch_config)
        .expect("fork rehearsal evidence digest");
    assert_eq!(digest.len(), 96);

    let mut supply_changed = evidence.clone();
    supply_changed.canonical_supply_after_external_trades_atoms += 1;
    let error = validate_pftl_uniswap_fork_rehearsal_evidence(&supply_changed, &launch_config)
        .expect_err("external trade supply mutation must fail");
    assert_eq!(error.code(), "external_trade_supply_changed");

    let mut no_buy = evidence.clone();
    no_buy.user_buy_wrapped_received_atoms = 0;
    let error = validate_pftl_uniswap_fork_rehearsal_evidence(&no_buy, &launch_config)
        .expect_err("missing external buy must fail");
    assert_eq!(error.code(), "zero_external_trade_delta");

    let mut manual_seed = evidence;
    manual_seed.packet_consumed_without_manual_mint = false;
    let error = validate_pftl_uniswap_fork_rehearsal_evidence(&manual_seed, &launch_config)
        .expect_err("manual seed must fail");
    assert_eq!(error.code(), "seed_not_canonical_packet");

    let mut no_liquidity = pftl_uniswap_fork_rehearsal_evidence(&launch_config);
    no_liquidity.state_view_liquidity_after_buy = 0;
    let error = validate_pftl_uniswap_fork_rehearsal_evidence(&no_liquidity, &launch_config)
        .expect_err("missing StateView liquidity must fail");
    assert_eq!(error.code(), "zero_state_view_liquidity");

    let mut consumed_failed_swap = pftl_uniswap_fork_rehearsal_evidence(&launch_config);
    consumed_failed_swap.min_output_failure_reverted_without_consume = false;
    let error =
        validate_pftl_uniswap_fork_rehearsal_evidence(&consumed_failed_swap, &launch_config)
            .expect_err("min-output failure consuming packet must fail");
    assert_eq!(error.code(), "swap_failure_consume_not_reverted");

    let mut wrong_pool = pftl_uniswap_fork_rehearsal_evidence(&launch_config);
    wrong_pool.uniswap_pool_id =
        "0x7777777777777777777777777777777777777777777777777777777777777777".to_string();
    let error = validate_pftl_uniswap_fork_rehearsal_evidence(&wrong_pool, &launch_config)
        .expect_err("wrong pool binding must fail");
    assert_eq!(error.code(), "fork_rehearsal_pool_mismatch");
}

#[test]
fn pftl_uniswap_return_burn_id_matches_solidity_abi_vector() {
    let config = pftl_uniswap_config();
    let request = PftlUniswapReturnBurnRequest {
        burn_event_hash: "0".repeat(64),
        ethereum_chain_id: 1,
        bridge_controller: config.handoff_controller.clone(),
        wrapped_navcoin_token: config.wrapped_navcoin_token.clone(),
        native_nav_asset_id: config.native_nav_asset_id.clone(),
        ethereum_sender: "0x6666666666666666666666666666666666666666".to_string(),
        pftl_recipient: "pf124071fd53a12ca4556b7aa1f5ec98b585e73468".to_string(),
        amount_atoms: 125,
        return_nonce: "b".repeat(64),
        burn_height: 1_000,
        finalized_height: 1_064,
    };

    let burn_id = pftl_uniswap_return_burn_id(&request).expect("return burn id");
    assert_eq!(
        burn_id,
        "fc60c1be99179546fffe0c4b7ed0f6049eb73f0634a519631614b38749552f93"
    );
}

#[test]
fn pftl_uniswap_route_rejects_trustless_copy_without_verifier() {
    let mut config = pftl_uniswap_config();
    config.route_trust_class = ROUTE_TRUST_CLASS_TRUSTLESS_FINALITY.to_string();
    config.verifier_mode = "threshold-controlled".to_string();

    let error = pftl_uniswap_route_config_digest(&config).expect_err("must reject");
    assert_eq!(error.code(), "trustless_finality_verifier_missing");
}

#[test]
fn pftl_uniswap_packet_binds_config_and_caps() {
    let config = pftl_uniswap_config();
    let packet = pftl_uniswap_packet(&config);

    validate_pftl_uniswap_packet_against_config(&packet, &config).expect("valid packet");
    let packet_id = pftl_uniswap_packet_id(&packet).expect("packet id");
    assert_eq!(packet_id.len(), 96);

    let mut mismatched = packet.clone();
    mismatched.config_digest = "0".repeat(96);
    let error = validate_pftl_uniswap_packet_against_config(&mismatched, &config)
        .expect_err("mismatched digest must fail");
    assert_eq!(error.code(), "route_config_digest_mismatch");

    let mut over_cap = packet;
    over_cap.settlement_amount_atoms = config.packet_notional_cap_atoms + 1;
    let error = validate_pftl_uniswap_packet_against_config(&over_cap, &config)
        .expect_err("packet cap must fail");
    assert_eq!(error.code(), "packet_notional_cap_exceeded");
}

#[test]
fn pftl_uniswap_packet_requires_source_packet_hash() {
    let config = pftl_uniswap_config();
    let mut packet = pftl_uniswap_packet(&config);
    packet.source_packet_hash = "d".repeat(64);

    let error = validate_pftl_uniswap_packet_against_config(&packet, &config)
        .expect_err("source packet hash must be full PFTL hash");
    assert_eq!(error.code(), "bad_genesis_hash");
}

#[test]
fn pftl_uniswap_packet_requires_source_receipt_binding() {
    let config = pftl_uniswap_config();
    let mut packet = pftl_uniswap_packet(&config);
    packet.source_receipt_root = "b".repeat(64);

    let error = validate_pftl_uniswap_packet_against_config(&packet, &config)
        .expect_err("source receipt root must be full PFTL hash");
    assert_eq!(error.code(), "bad_genesis_hash");
}

#[test]
fn pftl_uniswap_bridge_ledger_exports_refunds_and_preserves_invariant() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    let quote = pftl_uniswap_apply_primary_subscription(
        &mut ledger,
        primary_subscription_request(&config, &"0".repeat(64), 100_005, 1_000, 7),
    )
    .expect("primary subscription");
    assert_eq!(quote.minted_nav_atoms, 100);
    assert_eq!(ledger.authorized_valid_supply_atoms, 100);
    assert_eq!(ledger.pftl_spendable_supply_atoms, 100);
    assert_eq!(
        ledger
            .native_spendable_balances_atoms
            .get("pf124071fd53a12ca4556b7aa1f5ec98b585e73468"),
        Some(&100)
    );
    assert_eq!(ledger.settlement_reserve_atoms, 100_005);
    assert_eq!(
        ledger
            .primary_subscription_nonces
            .get(&"0".repeat(64))
            .map(String::as_str),
        Some("pf124071fd53a12ca4556b7aa1f5ec98b585e73468")
    );
    validate_pftl_uniswap_bridge_ledger(&ledger).expect("invariant after primary mint");

    let packet_hash = "1".repeat(96);
    let nonce = "2".repeat(64);
    let packet = pftl_uniswap_export_debit(
        &mut ledger,
        export_request(&config, &packet_hash, &nonce, 40, 10),
    )
    .expect("export debit");
    assert_eq!(packet.status, PftlUniswapExportPacketStatus::SourceDebited);
    assert_eq!(ledger.pftl_spendable_supply_atoms, 60);
    assert_eq!(
        ledger
            .native_spendable_balances_atoms
            .get("pf124071fd53a12ca4556b7aa1f5ec98b585e73468"),
        Some(&60)
    );
    assert_eq!(ledger.outstanding_bridge_claims_atoms, 40);
    validate_pftl_uniswap_bridge_ledger(&ledger).expect("invariant after export");

    let duplicate_nonce = pftl_uniswap_export_debit(
        &mut ledger,
        export_request(&config, &"3".repeat(96), &nonce, 1, 11),
    )
    .expect_err("duplicate nonce must fail");
    assert_eq!(duplicate_nonce.code(), "duplicate_export_nonce");
    let non_consumption_proof_hash =
        pftl_uniswap_non_consumption_proof_hash(&config.route_id, &packet_hash, 20)
            .expect("non-consumption proof commitment");
    let mismatched_refund = pftl_uniswap_refund_source(
        &mut ledger,
        PftlUniswapRefundRequest {
            packet_hash: packet_hash.clone(),
            current_height: 20,
            non_consumption_proof_hash: "4".repeat(96),
        },
    )
    .expect_err("arbitrary non-consumption proof hash must fail");
    assert_eq!(mismatched_refund.code(), "non_consumption_proof_mismatch");

    let early_refund = pftl_uniswap_refund_source(
        &mut ledger,
        PftlUniswapRefundRequest {
            packet_hash: packet_hash.clone(),
            current_height: 19,
            non_consumption_proof_hash: non_consumption_proof_hash.clone(),
        },
    )
    .expect_err("refund before window must fail");
    assert_eq!(early_refund.code(), "refund_before_window");
    assert_eq!(ledger.outstanding_bridge_claims_atoms, 40);

    let refunded = pftl_uniswap_refund_source(
        &mut ledger,
        PftlUniswapRefundRequest {
            packet_hash: packet_hash.clone(),
            current_height: 20,
            non_consumption_proof_hash,
        },
    )
    .expect("refund after window");
    assert_eq!(
        refunded.status,
        PftlUniswapExportPacketStatus::SourceRefunded
    );
    assert_eq!(ledger.pftl_spendable_supply_atoms, 100);
    assert_eq!(
        ledger
            .native_spendable_balances_atoms
            .get("pf124071fd53a12ca4556b7aa1f5ec98b585e73468"),
        Some(&100)
    );
    assert_eq!(ledger.outstanding_bridge_claims_atoms, 0);
    validate_pftl_uniswap_bridge_ledger(&ledger).expect("invariant after refund");

    let consumed = pftl_uniswap_mark_destination_consumed(&mut ledger, &packet_hash)
        .expect_err("refunded packet must not be consumed");
    assert_eq!(consumed.code(), "export_packet_not_settleable");
}

#[test]
fn pftl_uniswap_bridge_ledger_rejects_export_from_wrong_native_wallet() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    pftl_uniswap_apply_primary_subscription(
        &mut ledger,
        primary_subscription_request(&config, &"0".repeat(64), 100_000, 1_000, 7),
    )
    .expect("primary subscription");

    let mut wrong_wallet = export_request(&config, &"1".repeat(96), &"2".repeat(64), 40, 10);
    wrong_wallet.source_wallet = "pfwrongwallet".to_string();
    let error = pftl_uniswap_export_debit(&mut ledger, wrong_wallet)
        .expect_err("wrong wallet must not export another wallet's native balance");
    assert_eq!(error.code(), "insufficient_native_wallet_balance");
    assert_eq!(ledger.pftl_spendable_supply_atoms, 100);
    assert_eq!(
        ledger
            .native_spendable_balances_atoms
            .get("pf124071fd53a12ca4556b7aa1f5ec98b585e73468"),
        Some(&100)
    );
    assert!(!ledger
        .native_spendable_balances_atoms
        .contains_key("pfwrongwallet"));
    validate_pftl_uniswap_bridge_ledger(&ledger).expect("invariant after rejected export");
}

#[test]
fn pftl_uniswap_bridge_ledger_rejects_stale_nav_wrong_route_and_caps() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    let stale = pftl_uniswap_apply_primary_subscription(
        &mut ledger,
        primary_subscription_request(&config, &"1".repeat(64), 100_000, 1_000, 6),
    )
    .expect_err("stale NAV must fail");
    assert_eq!(stale.code(), "stale_pricing_nav_epoch");

    let cap = pftl_uniswap_apply_primary_subscription(
        &mut ledger,
        primary_subscription_request(
            &config,
            &"2".repeat(64),
            config.route_supply_cap_atoms + 1,
            1,
            7,
        ),
    )
    .expect_err("route cap must fail");
    assert_eq!(cap.code(), "route_supply_cap_exceeded");

    let valid_primary = primary_subscription_request(&config, &"3".repeat(64), 100_000, 1_000, 7);
    pftl_uniswap_apply_primary_subscription(&mut ledger, valid_primary.clone())
        .expect("primary mint for export failures");

    let duplicate_primary = pftl_uniswap_apply_primary_subscription(&mut ledger, valid_primary)
        .expect_err("duplicate primary nonce must fail");
    assert_eq!(
        duplicate_primary.code(),
        "duplicate_primary_subscription_nonce"
    );

    let mut wrong_primary_route =
        primary_subscription_request(&config, &"4".repeat(64), 100_000, 1_000, 7);
    wrong_primary_route.route_id = "wrong-route".to_string();
    let error = pftl_uniswap_apply_primary_subscription(&mut ledger, wrong_primary_route)
        .expect_err("wrong primary route");
    assert_eq!(error.code(), "primary_subscription_config_mismatch");

    let mut wrong_primary_asset =
        primary_subscription_request(&config, &"5".repeat(64), 100_000, 1_000, 7);
    wrong_primary_asset.settlement_asset_id = "7".repeat(96);
    let error = pftl_uniswap_apply_primary_subscription(&mut ledger, wrong_primary_asset)
        .expect_err("wrong primary settlement asset");
    assert_eq!(error.code(), "primary_subscription_config_mismatch");

    let mut wrong_route = export_request(&config, &"5".repeat(96), &"6".repeat(64), 1, 10);
    wrong_route.route_id = "wrong-route".to_string();
    let error = pftl_uniswap_export_debit(&mut ledger, wrong_route).expect_err("wrong route");
    assert_eq!(error.code(), "route_packet_config_mismatch");

    let over_notional = pftl_uniswap_export_debit(
        &mut ledger,
        export_request(
            &config,
            &"7".repeat(96),
            &"8".repeat(64),
            config.packet_notional_cap_atoms + 1,
            10,
        ),
    )
    .expect_err("packet cap must fail");
    assert_eq!(over_notional.code(), "packet_notional_cap_exceeded");
}

#[test]
fn pftl_uniswap_primary_subscription_mints_fractional_supply_from_pre_inflow_nav() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    assert_eq!(ledger.authorized_valid_supply_atoms, 0);
    assert_eq!(ledger.pftl_spendable_supply_atoms, 0);
    assert_eq!(ledger.settlement_reserve_atoms, 0);

    let (quote, receipt) = pftl_uniswap_apply_primary_subscription_with_receipt(
        &mut ledger,
        primary_subscription_request(&config, &"d".repeat(64), 100_500, 1_000, 7),
    )
    .expect("large fractional primary subscription");

    assert_eq!(quote.route_family, "primary_pftl_mint");
    assert_eq!(quote.supply_effect, "mints_new_native_navcoin_supply");
    assert_eq!(quote.pricing_source, "finalized_pre_inflow_nav_snapshot");
    assert_eq!(
        quote.settlement_reserve_effect,
        "accepted_settlement_added_after_primary_fill"
    );
    assert_eq!(quote.requested_settlement_atoms, 100_500);
    assert_eq!(quote.accepted_settlement_atoms, 100_500);
    assert_eq!(quote.refund_settlement_atoms, 0);
    assert_eq!(quote.minted_nav_atoms, 100);
    assert_eq!(quote.dust_settlement_atoms, 500);

    assert_eq!(ledger.authorized_valid_supply_atoms, 100);
    assert_eq!(ledger.pftl_spendable_supply_atoms, 100);
    assert_eq!(ledger.settlement_reserve_atoms, 100_500);
    assert_eq!(
        ledger
            .native_spendable_balances_atoms
            .get("pf124071fd53a12ca4556b7aa1f5ec98b585e73468"),
        Some(&100)
    );
    assert_eq!(receipt.transition, "primary_subscription");
    assert_eq!(receipt.requested_settlement_atoms, Some(100_500));
    assert_eq!(receipt.accepted_settlement_atoms, Some(100_500));
    assert_eq!(receipt.refund_settlement_atoms, Some(0));
    assert_eq!(receipt.minted_nav_atoms, Some(100));
    assert_eq!(
        receipt.rounding_rule.as_deref(),
        Some(PRIMARY_SUBSCRIPTION_ROUNDING_RULE_FLOOR_RESERVE_KEEPS_DUST)
    );
}

#[test]
fn pftl_uniswap_primary_subscription_rejects_secondary_inventory_route() {
    let mut config = pftl_uniswap_config();
    config.route_family = PFTL_UNISWAP_ROUTE_FAMILY_SECONDARY_INVENTORY.to_string();
    let mut ledger = pftl_uniswap_ledger(&config);

    let error = pftl_uniswap_apply_primary_subscription(
        &mut ledger,
        primary_subscription_request(&config, &"f".repeat(64), 100_000, 1_000, 7),
    )
    .expect_err("secondary inventory route cannot primary mint");

    assert_eq!(error.code(), "primary_subscription_route_family_mismatch");
    assert_eq!(ledger.authorized_valid_supply_atoms, 0);
    assert_eq!(ledger.pftl_spendable_supply_atoms, 0);
    assert_eq!(ledger.settlement_reserve_atoms, 0);
}

#[test]
fn pftl_uniswap_bridge_ledger_round_trips_twice_without_manual_edits() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    pftl_uniswap_apply_primary_subscription(
        &mut ledger,
        primary_subscription_request(&config, &"7".repeat(64), 500_000, 1_000, 7),
    )
    .expect("primary subscription");

    for (index, hex) in [("first", 'a'), ("second", 'b')] {
        let packet_hash = hex.to_string().repeat(96);
        let nonce = hex.to_string().repeat(64);
        pftl_uniswap_export_debit(
            &mut ledger,
            export_request(&config, &packet_hash, &nonce, 100, 20),
        )
        .unwrap_or_else(|error| panic!("{index} export failed: {error}"));
        pftl_uniswap_mark_destination_consumed(&mut ledger, &packet_hash)
            .unwrap_or_else(|error| panic!("{index} destination consume failed: {error}"));
        assert_eq!(ledger.outstanding_bridge_claims_atoms, 0);
        assert_eq!(ledger.ethereum_spendable_supply_atoms, 100);

        let return_nonce = if index == "first" {
            "c".repeat(64)
        } else {
            "d".repeat(64)
        };
        let return_burn = return_burn_request(&config, &return_nonce, 100, 1_000);
        let burn_hash = return_burn.burn_event_hash.clone();
        pftl_uniswap_record_return_burn(&mut ledger, return_burn)
            .unwrap_or_else(|error| panic!("{index} return burn failed: {error}"));
        assert_eq!(ledger.ethereum_spendable_supply_atoms, 0);
        assert_eq!(ledger.pending_return_import_claims_atoms, 100);

        let imported = pftl_uniswap_import_return(
            &mut ledger,
            &burn_hash,
            "pf124071fd53a12ca4556b7aa1f5ec98b585e73468",
        )
        .unwrap_or_else(|error| panic!("{index} import failed: {error}"));
        assert_eq!(imported.status, PftlUniswapReturnBurnStatus::Imported);
        assert_eq!(ledger.pending_return_import_claims_atoms, 0);
        validate_pftl_uniswap_bridge_ledger(&ledger)
            .unwrap_or_else(|error| panic!("{index} invariant failed: {error}"));
    }

    assert_eq!(ledger.authorized_valid_supply_atoms, 500);
    assert_eq!(ledger.pftl_spendable_supply_atoms, 500);
    assert_eq!(
        ledger
            .native_spendable_balances_atoms
            .get("pf124071fd53a12ca4556b7aa1f5ec98b585e73468"),
        Some(&500)
    );
    assert_eq!(ledger.ethereum_spendable_supply_atoms, 0);
}

#[test]
fn pftl_uniswap_status_reports_expose_route_packet_claims_and_supply() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    pftl_uniswap_apply_primary_subscription(
        &mut ledger,
        primary_subscription_request(&config, &"1".repeat(64), 300_000, 1_000, 7),
    )
    .expect("primary subscription");

    let outstanding_hash = "2".repeat(96);
    pftl_uniswap_export_debit(
        &mut ledger,
        export_request(&config, &outstanding_hash, &"3".repeat(64), 100, 20),
    )
    .expect("outstanding export");

    let consumed_hash = "4".repeat(96);
    pftl_uniswap_export_debit(
        &mut ledger,
        export_request(&config, &consumed_hash, &"5".repeat(64), 50, 21),
    )
    .expect("second export");
    pftl_uniswap_mark_destination_consumed(&mut ledger, &consumed_hash)
        .expect("consume second export");

    let return_burn = return_burn_request(&config, &"6".repeat(64), 30, 1_000);
    pftl_uniswap_record_return_burn(&mut ledger, return_burn).expect("return burn");

    let supply = pftl_uniswap_bridge_supply_status(&ledger).expect("supply status");
    assert_eq!(supply.schema, "postfiat-pftl-uniswap-supply-status-v1");
    assert!(supply.invariant_holds);
    assert_eq!(supply.authorized_valid_supply_atoms, 300);
    assert_eq!(supply.pftl_spendable_supply_atoms, 150);
    assert_eq!(supply.native_spendable_balance_count, 1);
    assert_eq!(
        supply.native_spendable_balance_limit,
        PFTL_UNISWAP_STATUS_MAX_ROWS as u64
    );
    assert!(!supply.native_spendable_balances_truncated);
    assert_eq!(supply.native_spendable_balance_sum_atoms, 150);
    assert_eq!(supply.native_spendable_balances.len(), 1);
    assert_eq!(
        supply.native_spendable_balances[0].wallet,
        "pf124071fd53a12ca4556b7aa1f5ec98b585e73468"
    );
    assert_eq!(supply.native_spendable_balances[0].amount_atoms, 150);
    assert_eq!(supply.ethereum_spendable_supply_atoms, 20);
    assert_eq!(supply.outstanding_bridge_claims_atoms, 100);
    assert_eq!(supply.pending_return_import_claims_atoms, 30);
    assert_eq!(supply.live_supply_sum_atoms, 300);

    let packet =
        pftl_uniswap_bridge_packet_status(&ledger, &outstanding_hash).expect("packet status");
    assert_eq!(packet.packet.packet_hash, outstanding_hash);
    assert_eq!(packet.packet.claim_class, "outstanding_bridge_claim");
    assert_eq!(
        packet.packet.status,
        PftlUniswapExportPacketStatus::SourceDebited
    );

    let claims = pftl_uniswap_bridge_claims_status(&ledger, 2, false).expect("claims status");
    assert!(!claims.truncated);
    assert_eq!(claims.export_claim_count, 1);
    assert_eq!(claims.return_claim_count, 1);
    assert_eq!(claims.exports.len(), 1);
    assert_eq!(claims.returns.len(), 1);
    assert_eq!(claims.exports[0].packet_hash, outstanding_hash);
    assert_eq!(claims.returns[0].claim_class, "pending_return_import_claim");

    let truncated = pftl_uniswap_bridge_claims_status(&ledger, 1, false).expect("truncated claims");
    assert!(truncated.truncated);
    assert_eq!(truncated.export_claim_count, 1);
    assert_eq!(truncated.return_claim_count, 1);
    assert_eq!(truncated.exports.len(), 1);
    assert!(truncated.returns.is_empty());

    let with_terminal =
        pftl_uniswap_bridge_claims_status(&ledger, 3, true).expect("claims with terminal");
    assert_eq!(with_terminal.export_claim_count, 2);
    assert_eq!(with_terminal.exports[1].packet_hash, consumed_hash);
    assert_eq!(with_terminal.exports[1].claim_class, "destination_consumed");

    let mut other_config = config.clone();
    other_config.route_id = "aaa-pftl-a777-ethereum-wA777-usdc-v1".to_string();
    other_config.native_nav_asset_id = "b".repeat(96);
    other_config.wrapped_navcoin_token = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
    let other_ledger = pftl_uniswap_ledger(&other_config);
    let routes =
        pftl_uniswap_bridge_routes_status(&[ledger.clone(), other_ledger]).expect("routes status");
    assert_eq!(routes.schema, "postfiat-pftl-uniswap-routes-status-v1");
    assert_eq!(routes.route_count, 2);
    assert_eq!(routes.routes[0].route_id, other_config.route_id);
    assert_eq!(routes.routes[1].route_id, config.route_id);
    assert_eq!(routes.routes[1].outstanding_export_packet_count, 1);
    assert_eq!(routes.routes[1].pending_return_burn_count, 1);
}

#[test]
fn pftl_uniswap_supply_status_bounds_native_balance_rows() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    let wallet_count = PFTL_UNISWAP_STATUS_MAX_ROWS + 3;
    for index in 0..wallet_count {
        ledger
            .native_spendable_balances_atoms
            .insert(format!("pfwallet{index:04}"), 1);
    }
    ledger.authorized_valid_supply_atoms = wallet_count as u64;
    ledger.pftl_spendable_supply_atoms = wallet_count as u64;

    let supply = pftl_uniswap_bridge_supply_status(&ledger).expect("supply status");
    assert!(supply.invariant_holds);
    assert_eq!(supply.native_spendable_balance_count, wallet_count as u64);
    assert_eq!(
        supply.native_spendable_balance_limit,
        PFTL_UNISWAP_STATUS_MAX_ROWS as u64
    );
    assert!(supply.native_spendable_balances_truncated);
    assert_eq!(
        supply.native_spendable_balances.len(),
        PFTL_UNISWAP_STATUS_MAX_ROWS
    );
    assert_eq!(
        supply.native_spendable_balance_sum_atoms,
        wallet_count as u64
    );
    assert_eq!(supply.pftl_spendable_supply_atoms, wallet_count as u64);
    assert_eq!(supply.native_spendable_balances[0].wallet, "pfwallet0000");
    assert_eq!(
        supply
            .native_spendable_balances
            .last()
            .expect("last displayed balance")
            .wallet,
        format!("pfwallet{:04}", PFTL_UNISWAP_STATUS_MAX_ROWS - 1)
    );
}

#[test]
fn pftl_uniswap_transition_receipts_commit_ordered_root_and_mutations() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    let initial_ledger = ledger.clone();
    let state_zero = pftl_uniswap_bridge_ledger_hash(&ledger).expect("initial state hash");

    let (_, primary_receipt) = pftl_uniswap_apply_primary_subscription_with_receipt(
        &mut ledger,
        primary_subscription_request(&config, &"8".repeat(64), 500_000, 1_000, 7),
    )
    .expect("primary with receipt");
    assert_eq!(primary_receipt.transition, "primary_subscription");
    assert_eq!(primary_receipt.state_before_hash, state_zero);
    assert_eq!(primary_receipt.amount_atoms, Some(500));
    assert_eq!(
        primary_receipt.source_wallet.as_deref(),
        Some("pf124071fd53a12ca4556b7aa1f5ec98b585e73468")
    );
    assert_eq!(
        primary_receipt.settlement_asset_id.as_deref(),
        Some(config.settlement_asset_id.as_str())
    );
    assert_eq!(
        primary_receipt.nonce.as_deref(),
        Some("8888888888888888888888888888888888888888888888888888888888888888")
    );
    let primary_hash =
        pftl_uniswap_transition_receipt_hash(&primary_receipt).expect("primary hash");
    assert_eq!(primary_hash.len(), 96);

    let packet_hash = "a".repeat(96);
    let (_, export_receipt) = pftl_uniswap_export_debit_with_receipt(
        &mut ledger,
        export_request(&config, &packet_hash, &"b".repeat(64), 125, 20),
    )
    .expect("export with receipt");
    assert_eq!(export_receipt.transition, "export_debit");
    assert_eq!(
        export_receipt.state_before_hash,
        primary_receipt.state_after_hash
    );
    assert_eq!(
        export_receipt.packet_hash.as_deref(),
        Some(packet_hash.as_str())
    );

    let (_, consume_receipt) =
        pftl_uniswap_mark_destination_consumed_with_receipt(&mut ledger, &packet_hash)
            .expect("consume with receipt");
    assert_eq!(consume_receipt.transition, "destination_consumed");
    assert_eq!(
        consume_receipt.state_before_hash,
        export_receipt.state_after_hash
    );

    let return_nonce = "c".repeat(64);
    let return_burn = return_burn_request(&config, &return_nonce, 125, 1_000);
    let burn_hash = return_burn.burn_event_hash.clone();
    let (_, return_burn_receipt) =
        pftl_uniswap_record_return_burn_with_receipt(&mut ledger, return_burn)
            .expect("return burn with receipt");
    assert_eq!(return_burn_receipt.transition, "return_burn_observed");
    assert_eq!(
        return_burn_receipt.state_before_hash,
        consume_receipt.state_after_hash
    );
    assert_eq!(
        return_burn_receipt.bridge_controller.as_deref(),
        Some(config.handoff_controller.as_str())
    );
    assert_eq!(
        return_burn_receipt.return_burn_event_hash.as_deref(),
        Some(burn_hash.as_str())
    );
    assert_eq!(
        return_burn_receipt.ethereum_sender.as_deref(),
        Some("0x6666666666666666666666666666666666666666")
    );
    assert_eq!(
        return_burn_receipt.nonce.as_deref(),
        Some(return_nonce.as_str())
    );

    let (_, import_receipt) = pftl_uniswap_import_return_with_receipt(
        &mut ledger,
        &burn_hash,
        "pf124071fd53a12ca4556b7aa1f5ec98b585e73468",
    )
    .expect("import with receipt");
    assert_eq!(import_receipt.transition, "return_imported");
    assert_eq!(
        import_receipt.state_before_hash,
        return_burn_receipt.state_after_hash
    );
    assert_eq!(
        import_receipt.bridge_controller.as_deref(),
        Some(config.handoff_controller.as_str())
    );
    assert_eq!(
        import_receipt.return_burn_event_hash.as_deref(),
        Some(burn_hash.as_str())
    );
    assert_eq!(
        import_receipt.ethereum_sender.as_deref(),
        Some("0x6666666666666666666666666666666666666666")
    );
    assert_eq!(import_receipt.nonce.as_deref(), Some(return_nonce.as_str()));

    let receipts = vec![
        primary_receipt.clone(),
        export_receipt.clone(),
        consume_receipt.clone(),
        return_burn_receipt.clone(),
        import_receipt.clone(),
    ];
    let root = pftl_uniswap_receipt_root(&receipts).expect("receipt root");
    assert_eq!(root.len(), 96);
    let stable = pftl_uniswap_receipt_root(&receipts).expect("stable receipt root");
    assert_eq!(root, stable);

    let mut mutated = receipts.clone();
    mutated[1].amount_atoms = Some(126);
    let mutated_root = pftl_uniswap_receipt_root(&mutated).expect("mutated root");
    assert_ne!(root, mutated_root);
    let mutated_error =
        pftl_uniswap_verify_transition_receipt_replay(&initial_ledger, &mutated, &ledger)
            .expect_err("mutated receipt must fail replay");
    assert_eq!(mutated_error.code(), "receipt_replay_mismatch");

    let (replayed, replay_report) =
        pftl_uniswap_replay_transition_receipts(&initial_ledger, &receipts)
            .expect("receipt replay");
    assert_eq!(replayed, ledger);
    assert_eq!(
        replay_report.schema,
        "postfiat-pftl-uniswap-receipt-replay-report-v1"
    );
    assert_eq!(replay_report.route_id, config.route_id);
    assert_eq!(replay_report.initial_ledger_hash, state_zero);
    assert_eq!(
        replay_report.final_ledger_hash,
        pftl_uniswap_bridge_ledger_hash(&ledger).expect("final ledger hash")
    );
    assert_eq!(replay_report.receipt_root, root);
    assert_eq!(replay_report.receipt_count, 5);
    let verified =
        pftl_uniswap_verify_transition_receipt_replay(&initial_ledger, &receipts, &ledger)
            .expect("receipt replay verify");
    assert_eq!(verified, replay_report);

    let mut wrong_final = ledger.clone();
    wrong_final.paused = true;
    let wrong_final_error =
        pftl_uniswap_verify_transition_receipt_replay(&initial_ledger, &receipts, &wrong_final)
            .expect_err("wrong final ledger must fail replay verification");
    assert_eq!(
        wrong_final_error.code(),
        "receipt_replay_final_ledger_mismatch"
    );

    let mut reordered = receipts.clone();
    reordered.swap(0, 1);
    let reordered_root = pftl_uniswap_receipt_root(&reordered).expect("reordered root");
    assert_ne!(root, reordered_root);
    let reordered_error =
        pftl_uniswap_verify_transition_receipt_replay(&initial_ledger, &reordered, &ledger)
            .expect_err("reordered receipts must fail replay");
    assert_eq!(reordered_error.code(), "receipt_state_chain_mismatch");

    let empty_error = pftl_uniswap_replay_transition_receipts(&initial_ledger, &[])
        .expect_err("empty receipt replay must fail");
    assert_eq!(empty_error.code(), "empty_receipt_replay");
}

#[test]
fn pftl_uniswap_refund_receipt_commits_non_consumption_proof() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    pftl_uniswap_apply_primary_subscription(
        &mut ledger,
        primary_subscription_request(&config, &"9".repeat(64), 100_000, 1_000, 7),
    )
    .expect("primary subscription");
    let packet_hash = "1".repeat(96);
    pftl_uniswap_export_debit(
        &mut ledger,
        export_request(&config, &packet_hash, &"2".repeat(64), 40, 10),
    )
    .expect("export");
    let ledger_before_refund = ledger.clone();
    let non_consumption_proof_hash =
        pftl_uniswap_non_consumption_proof_hash(&config.route_id, &packet_hash, 20)
            .expect("non-consumption proof commitment");

    let (_, refund_receipt) = pftl_uniswap_refund_source_with_receipt(
        &mut ledger,
        PftlUniswapRefundRequest {
            packet_hash: packet_hash.clone(),
            current_height: 20,
            non_consumption_proof_hash: non_consumption_proof_hash.clone(),
        },
    )
    .expect("refund with receipt");
    assert_eq!(refund_receipt.transition, "source_refunded");
    assert_eq!(
        refund_receipt.non_consumption_proof_hash.as_deref(),
        Some(non_consumption_proof_hash.as_str())
    );
    let root =
        pftl_uniswap_receipt_root(std::slice::from_ref(&refund_receipt)).expect("refund root");
    assert_eq!(root.len(), 96);
    let (replayed, replay_report) =
        pftl_uniswap_replay_transition_receipts(&ledger_before_refund, &[refund_receipt])
            .expect("refund replay");
    assert_eq!(replayed, ledger);
    assert_eq!(replay_report.receipt_count, 1);
    assert_eq!(replay_report.receipt_root, root);
    let empty =
        pftl_uniswap_receipt_root_from_hashes(&[]).expect_err("empty receipt roots must fail");
    assert_eq!(empty.code(), "empty_receipt_root");
}

#[test]
fn pftl_uniswap_return_path_rejects_replay_wrong_token_and_low_finality() {
    let config = pftl_uniswap_config();
    let mut ledger = pftl_uniswap_ledger(&config);
    pftl_uniswap_apply_primary_subscription(
        &mut ledger,
        primary_subscription_request(&config, &"0".repeat(64), 100_000, 1_000, 7),
    )
    .expect("primary subscription");
    let packet_hash = "9".repeat(96);
    pftl_uniswap_export_debit(
        &mut ledger,
        export_request(&config, &packet_hash, &"a".repeat(64), 100, 20),
    )
    .expect("export");
    pftl_uniswap_mark_destination_consumed(&mut ledger, &packet_hash).expect("consume");

    let mut wrong_bridge = return_burn_request(&config, &"a".repeat(64), 1, 1_000);
    wrong_bridge.bridge_controller = "0x9999999999999999999999999999999999999999".to_string();
    let error =
        pftl_uniswap_record_return_burn(&mut ledger, wrong_bridge).expect_err("wrong bridge");
    assert_eq!(error.code(), "wrong_return_bridge");

    let mut wrong_token = return_burn_request(&config, &"b".repeat(64), 1, 1_000);
    wrong_token.wrapped_navcoin_token = "0x9999999999999999999999999999999999999999".to_string();
    let error = pftl_uniswap_record_return_burn(&mut ledger, wrong_token).expect_err("wrong token");
    assert_eq!(error.code(), "wrong_return_token");

    let mut malformed_sender = return_burn_request(&config, &"c".repeat(64), 1, 1_000);
    malformed_sender.ethereum_sender = "0x1234".to_string();
    let error = pftl_uniswap_record_return_burn(&mut ledger, malformed_sender)
        .expect_err("malformed sender");
    assert_eq!(error.code(), "bad_ethereum_address");

    let mut malformed_nonce = return_burn_request(&config, &"d".repeat(64), 1, 1_000);
    malformed_nonce.return_nonce = "e".repeat(63);
    let error =
        pftl_uniswap_record_return_burn(&mut ledger, malformed_nonce).expect_err("malformed nonce");
    assert_eq!(error.code(), "bad_genesis_hash");

    let mut low_finality = return_burn_request(&config, &"e".repeat(64), 1, 1_000);
    low_finality.finalized_height = 1_063;
    let error =
        pftl_uniswap_record_return_burn(&mut ledger, low_finality).expect_err("low finality");
    assert_eq!(error.code(), "return_event_below_finality");

    let mut mismatched_id = return_burn_request(&config, &"f".repeat(64), 1, 1_000);
    mismatched_id.burn_event_hash = "0".repeat(64);
    let error = pftl_uniswap_record_return_burn(&mut ledger, mismatched_id)
        .expect_err("mismatched return burn id");
    assert_eq!(error.code(), "return_burn_id_mismatch");

    let return_burn = return_burn_request(&config, &"f".repeat(64), 1, 1_000);
    let burn_hash = return_burn.burn_event_hash.clone();
    pftl_uniswap_record_return_burn(&mut ledger, return_burn.clone()).expect("return burn");
    let replay =
        pftl_uniswap_record_return_burn(&mut ledger, return_burn).expect_err("burn replay");
    assert_eq!(replay.code(), "duplicate_return_burn");

    let wrong_recipient = pftl_uniswap_import_return(&mut ledger, &burn_hash, "pfwrongrecipient")
        .expect_err("wrong recipient");
    assert_eq!(wrong_recipient.code(), "wrong_return_recipient");

    pftl_uniswap_import_return(
        &mut ledger,
        &burn_hash,
        "pf124071fd53a12ca4556b7aa1f5ec98b585e73468",
    )
    .expect("import");
    let import_replay = pftl_uniswap_import_return(
        &mut ledger,
        &burn_hash,
        "pf124071fd53a12ca4556b7aa1f5ec98b585e73468",
    )
    .expect_err("import replay");
    assert_eq!(import_replay.code(), "return_burn_not_importable");
}

#[test]
fn pftl_uniswap_packet_fails_closed_when_route_disabled() {
    let mut config = pftl_uniswap_config();
    config.route_trust_class = ROUTE_TRUST_CLASS_DISABLED.to_string();
    let packet = pftl_uniswap_packet(&config);

    let error = validate_pftl_uniswap_packet_against_config(&packet, &config)
        .expect_err("disabled route must fail");
    assert_eq!(error.code(), "route_disabled");
}

#[test]
fn applies_bridge_transfers_and_rejects_replay() {
    let mut state = BridgeState::empty();
    let domain = upsert_domain(&mut state, "xrpl-test", "XRPL Testnet", 100, 75).expect("domain");
    assert_eq!(domain.inbound_cap, 100);
    assert_eq!(domain.source_chain, "xrpl-test");
    assert_eq!(domain.target_chain, "postfiat-local");
    assert_eq!(domain.bridge_id, "xrpl-test");
    assert_eq!(domain.door_account, "door:xrpl-test");

    let inbound = apply_simulated_transfer(
        &mut state,
        attested_request(
            &domain,
            BRIDGE_DIRECTION_INBOUND,
            "xrpl:rSource",
            "pfrecipient",
            40,
            "witness-1",
            1,
        ),
    )
    .expect("inbound transfer");
    assert_eq!(inbound.sequence, 1);
    assert_eq!(inbound.source_chain, "xrpl-test");
    assert_eq!(inbound.target_chain, "postfiat-local");
    assert_eq!(inbound.bridge_id, "xrpl-test");
    assert_eq!(inbound.door_account, "door:xrpl-test");

    let replay = apply_simulated_transfer(
        &mut state,
        attested_request(
            &domain,
            BRIDGE_DIRECTION_INBOUND,
            "xrpl:rSource",
            "pfrecipient",
            1,
            "witness-1",
            1,
        ),
    )
    .expect_err("replay must fail");
    assert_eq!(replay.code(), "duplicate_witness");

    let next_epoch = apply_simulated_transfer(
        &mut state,
        attested_request(
            &domain,
            BRIDGE_DIRECTION_INBOUND,
            "xrpl:rSource",
            "pfrecipient",
            1,
            "witness-1",
            2,
        ),
    )
    .expect("same witness id in a new epoch is a distinct replay domain");
    assert_eq!(next_epoch.witness_epoch, 2);

    let outbound = apply_simulated_transfer(
        &mut state,
        attested_request(
            &domain,
            BRIDGE_DIRECTION_OUTBOUND,
            "pfholder",
            "xrpl:rDest",
            75,
            "witness-2",
            1,
        ),
    )
    .expect("outbound transfer");
    assert_eq!(outbound.sequence, 3);
    assert_eq!(state.domain("xrpl-test").unwrap().inbound_used, 41);
    assert_eq!(state.domain("xrpl-test").unwrap().outbound_used, 75);
}

#[test]
fn pruned_replay_cache_does_not_reopen_historical_witness_replay() {
    let mut state = BridgeState::empty();
    let domain =
        upsert_domain(&mut state, "xrpl-test", "XRPL Testnet", 1_000, 1_000).expect("domain");

    apply_simulated_transfer(
        &mut state,
        attested_request(
            &domain,
            BRIDGE_DIRECTION_INBOUND,
            "xrpl:rSource",
            "pfrecipient",
            1,
            "witness-old",
            1,
        ),
    )
    .expect("old epoch transfer");
    assert!(state
        .replay_cache
        .iter()
        .any(|entry| entry == "xrpl-test:1:witness-old"));

    apply_simulated_transfer(
        &mut state,
        attested_request(
            &domain,
            BRIDGE_DIRECTION_INBOUND,
            "xrpl:rSource",
            "pfrecipient",
            1,
            "witness-new",
            4,
        ),
    )
    .expect("new epoch transfer");
    assert!(!state
        .replay_cache
        .iter()
        .any(|entry| entry == "xrpl-test:1:witness-old"));

    let replay = apply_simulated_transfer(
        &mut state,
        attested_request(
            &domain,
            BRIDGE_DIRECTION_INBOUND,
            "xrpl:rSource",
            "pfrecipient",
            1,
            "witness-old",
            1,
        ),
    )
    .expect_err("pruned historical witness replay must still fail");
    assert_eq!(replay.code(), "duplicate_witness");
}

#[test]
fn enforces_caps_and_pause() {
    let mut state = BridgeState::empty();
    let domain =
        upsert_domain(&mut state, "eth-sepolia", "Ethereum Sepolia", 10, 10).expect("domain");

    let too_large = apply_simulated_transfer(
        &mut state,
        attested_request(
            &domain,
            BRIDGE_DIRECTION_INBOUND,
            "eth:source",
            "pfrecipient",
            11,
            "cap-witness",
            1,
        ),
    )
    .expect_err("cap must fail");
    assert_eq!(too_large.code(), "inbound_cap_exceeded");

    set_domain_paused(&mut state, "eth-sepolia", true).expect("pause");
    let paused = apply_simulated_transfer(
        &mut state,
        attested_request(
            &domain,
            BRIDGE_DIRECTION_OUTBOUND,
            "pfholder",
            "eth:dest",
            1,
            "paused-witness",
            1,
        ),
    )
    .expect_err("paused must fail");
    assert_eq!(paused.code(), "domain_paused");
}

#[test]
fn rejects_malformed_bridge_text_fields() {
    let mut state = BridgeState::empty();
    assert_eq!(
        upsert_domain(&mut state, " xrpl-test", "XRPL Testnet", 100, 75)
            .expect_err("domain id boundary whitespace must fail")
            .code(),
        "boundary_whitespace"
    );

    let mut spec = BridgeDomainSpec::new("xrpl-test", "XRPL Testnet", 100, 75);
    spec.source_chain = "xrpl\ntest".to_string();
    assert_eq!(
        upsert_domain_with_metadata(&mut state, spec)
            .expect_err("source chain control character must fail")
            .code(),
        "control_character"
    );

    let domain = upsert_domain(&mut state, "xrpl-test", "XRPL Testnet", 100, 75).expect("domain");
    let mut request = BridgeTransferRequest {
        domain_id: domain.domain_id.clone(),
        direction: BRIDGE_DIRECTION_INBOUND.to_string(),
        from: " xrpl:rSource".to_string(),
        to: "pfrecipient".to_string(),
        asset_id: "XRP".to_string(),
        amount: 1,
        witness_id: "witness-1".to_string(),
        witness_epoch: 1,
        witness_attestation: None,
    };
    assert_eq!(
        apply_simulated_transfer(&mut state, request.clone())
            .expect_err("request boundary whitespace must fail")
            .code(),
        "boundary_whitespace"
    );

    request.from = "xrpl:rSource".to_string();
    request.witness_id = "witness\n1".to_string();
    assert_eq!(
        bridge_witness_attestation_message(
            test_chain_domain(),
            &domain,
            &request,
            "witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("witness helper must reject request control characters")
        .code(),
        "control_character"
    );

    request.witness_id = "witness-1".to_string();
    assert_eq!(
        bridge_witness_attestation_id(
            test_chain_domain(),
            &domain,
            &request,
            " witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("witness helper must reject signer boundary whitespace")
        .code(),
        "boundary_whitespace"
    );
}

#[test]
fn rejects_missing_or_tampered_witness_attestation() {
    let mut state = BridgeState::empty();
    let domain = upsert_domain(&mut state, "xrpl-test", "XRPL Testnet", 100, 75).expect("domain");

    let mut missing = attested_request(
        &domain,
        BRIDGE_DIRECTION_INBOUND,
        "xrpl:rSource",
        "pfrecipient",
        1,
        "missing-attestation",
        1,
    );
    missing.witness_attestation = None;
    let missing_error =
        apply_simulated_transfer(&mut state, missing).expect_err("missing attestation fails");
    assert_eq!(missing_error.code(), "missing_witness_attestation");

    let mut tampered = attested_request(
        &domain,
        BRIDGE_DIRECTION_INBOUND,
        "xrpl:rSource",
        "pfrecipient",
        1,
        "tampered-attestation",
        1,
    );
    tampered
        .witness_attestation
        .as_mut()
        .expect("attestation")
        .attestation_id = "tampered".to_string();
    let tampered_error =
        apply_simulated_transfer(&mut state, tampered).expect_err("tampered attestation fails");
    assert_eq!(tampered_error.code(), "bad_witness_attestation");
}

#[test]
fn witness_attestation_commits_to_chain_domain() {
    let mut state = BridgeState::empty();
    let domain = upsert_domain(&mut state, "xrpl-test", "XRPL Testnet", 100, 75).expect("domain");
    let request = attested_request(
        &domain,
        BRIDGE_DIRECTION_INBOUND,
        "xrpl:rSource",
        "pfrecipient",
        1,
        "chain-domain-attestation",
        1,
    );
    let attestation = request
        .witness_attestation
        .as_ref()
        .expect("witness attestation");
    let local_id = bridge_witness_attestation_id(
        test_chain_domain(),
        &domain,
        &request,
        &attestation.signer,
        &attestation.algorithm_id,
        &attestation.public_key_hex,
    )
    .expect("local attestation id");
    let other_chain_id = bridge_witness_attestation_id(
        BridgeWitnessChainDomain {
            chain_id: "postfiat-other",
            genesis_hash: TEST_GENESIS_HASH,
            protocol_version: TEST_PROTOCOL_VERSION,
        },
        &domain,
        &request,
        &attestation.signer,
        &attestation.algorithm_id,
        &attestation.public_key_hex,
    )
    .expect("other chain attestation id");
    assert_eq!(local_id, attestation.attestation_id);
    assert_ne!(local_id, other_chain_id);

    let mut tampered = request;
    tampered
        .witness_attestation
        .as_mut()
        .expect("witness attestation")
        .chain_id = "postfiat-other".to_string();
    let tampered_error =
        apply_simulated_transfer(&mut state, tampered).expect_err("wrong chain fails");
    assert_eq!(tampered_error.code(), "bad_witness_attestation");
}

#[test]
fn rejects_malformed_witness_chain_domain() {
    let mut state = BridgeState::empty();
    let domain = upsert_domain(&mut state, "xrpl-test", "XRPL Testnet", 100, 75).expect("domain");
    let request = BridgeTransferRequest {
        domain_id: domain.domain_id.clone(),
        direction: BRIDGE_DIRECTION_INBOUND.to_string(),
        from: "xrpl:rSource".to_string(),
        to: "pfrecipient".to_string(),
        asset_id: "XRP".to_string(),
        amount: 1,
        witness_id: "witness-1".to_string(),
        witness_epoch: 1,
        witness_attestation: None,
    };

    let mut empty_chain_id = test_chain_domain();
    empty_chain_id.chain_id = " ";
    assert_eq!(
        bridge_witness_attestation_message(
            empty_chain_id,
            &domain,
            &request,
            "witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("empty chain id must fail")
        .code(),
        "empty_field"
    );
    assert_eq!(
        bridge_witness_attestation_id(
            empty_chain_id,
            &domain,
            &request,
            "witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("empty chain id must fail")
        .code(),
        "empty_field"
    );

    let mut empty_genesis_hash = test_chain_domain();
    empty_genesis_hash.genesis_hash = "";
    assert_eq!(
        bridge_witness_attestation_message(
            empty_genesis_hash,
            &domain,
            &request,
            "witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("empty genesis hash must fail")
        .code(),
        "empty_field"
    );
    assert_eq!(
        bridge_witness_attestation_id(
            empty_genesis_hash,
            &domain,
            &request,
            "witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("empty genesis hash must fail")
        .code(),
        "empty_field"
    );

    let mut malformed_genesis_hash = test_chain_domain();
    malformed_genesis_hash.genesis_hash = "not-a-genesis-hash";
    assert_eq!(
        bridge_witness_attestation_message(
            malformed_genesis_hash,
            &domain,
            &request,
            "witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("malformed genesis hash must fail")
        .code(),
        "bad_genesis_hash"
    );
    assert_eq!(
        bridge_witness_attestation_id(
            malformed_genesis_hash,
            &domain,
            &request,
            "witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("malformed genesis hash must fail")
        .code(),
        "bad_genesis_hash"
    );

    let mut zero_protocol_version = test_chain_domain();
    zero_protocol_version.protocol_version = 0;
    assert_eq!(
        bridge_witness_attestation_message(
            zero_protocol_version,
            &domain,
            &request,
            "witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("zero protocol version must fail")
        .code(),
        "empty_field"
    );
    assert_eq!(
        bridge_witness_attestation_id(
            zero_protocol_version,
            &domain,
            &request,
            "witness",
            ML_DSA_65_ALGORITHM,
            "00",
        )
        .expect_err("zero protocol version must fail")
        .code(),
        "empty_field"
    );
}
