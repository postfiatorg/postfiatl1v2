use super::*;
use postfiat_types::{
    SolanaDepositPublicValues, SolanaProofOperation, SolanaRouteState, SOLANA_DEPOSIT_PROOF,
};

type Rejection = (&'static str, String);

pub(super) fn apply_proof(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &SolanaProofOperation,
    height: u64,
    compatibility: AssetExecutionCompatibility,
    orchard_balances: &[AssetOrchardAssetBalance],
    bridge_hold_events: &mut Vec<postfiat_types::AssetBridgeHoldReceiptEventV1>,
) -> Result<(), Rejection> {
    let secondary = ledger
        .additional_solana_routes
        .contains_key(&operation.profile_hash);
    let route = ledger
        .additional_solana_routes
        .get(&operation.profile_hash)
        .or(ledger.solana_route.as_ref())
        .cloned()
        .ok_or((
            "solana_bridge_disabled",
            "Solana route has no activation".into(),
        ))?;
    let profile = &route.profile;
    if ledger.solana_route_superseded(profile) {
        return Err((
            "solana_route_superseded",
            "Solana route revision has been retired".into(),
        ));
    }
    if !(profile.activation_height..profile.expires_at_height).contains(&height) {
        return Err((
            "solana_bridge_disabled",
            "Solana proof outside active heights".into(),
        ));
    }
    let hash = profile
        .profile_hash()
        .map_err(|e| ("invalid_solana_route", e))?;
    if operation.profile_hash != hash {
        return Err((
            "solana_route_mismatch",
            "Solana proof targets another profile".into(),
        ));
    }
    let pins = profile
        .solana
        .as_ref()
        .ok_or(("invalid_solana_route", "missing Solana pins".into()))?;
    let (verifier_kind, vkey) = match operation.proof_kind.as_str() {
        SOLANA_DEPOSIT_PROOF => (
            profile.verifier_kind.as_str(),
            &profile.verifier_program_vkey,
        ),
        postfiat_types::SOLANA_HANDOFF_PROOF => (
            postfiat_types::SOLANA_HANDOFF_PROOF,
            &pins.handoff_program_vkey,
        ),
        _ => {
            return Err((
                "invalid_solana_proof_kind",
                "Unsupported Solana proof".into(),
            ))
        }
    };
    verify_bounded_sp1_groth16_with_config(
        verifier_kind,
        &operation.proof_kind,
        vkey,
        profile.max_proof_bytes,
        profile.max_public_values_bytes,
        &operation.proof,
        &operation.public_values,
    )
    .map_err(|error| (error.code(), error.message()))?;
    if operation.proof_kind == postfiat_types::SOLANA_HANDOFF_PROOF {
        let client = if secondary {
            ledger
                .additional_solana_routes
                .get_mut(&operation.profile_hash)
        } else {
            ledger.solana_route.as_mut()
        };
        client
            .expect("route loaded above")
            .apply_verified_handoff(&operation.public_values)
            .map_err(|e| ("invalid_solana_handoff", e))?;
        return Ok(());
    }
    credit_verified(
        genesis,
        ledger,
        &route,
        &hash,
        &operation.public_values,
        height,
        compatibility,
        orchard_balances,
        bridge_hold_events,
    )
}

// Entry through apply_asset_operation runs on the transaction's scratch ledger.
// Any failure in a later receipt rolls back the entire batch, including fees.
#[allow(clippy::too_many_arguments)]
fn credit_verified(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    route: &SolanaRouteState,
    hash: &str,
    public_values: &[u8],
    height: u64,
    compatibility: AssetExecutionCompatibility,
    orchard_balances: &[AssetOrchardAssetBalance],
    bridge_hold_events: &mut Vec<postfiat_types::AssetBridgeHoldReceiptEventV1>,
) -> Result<(), Rejection> {
    let profile = &route.profile;
    let pins = profile
        .solana
        .as_ref()
        .ok_or(("invalid_solana_route", "missing Solana pins".into()))?;
    let values = SolanaDepositPublicValues::decode(public_values)
        .map_err(|e| ("invalid_solana_deposit_values", e))?;
    let certificate_epoch = pins.slot_epoch(values.certificate_slot).ok_or((
        "solana_source_mismatch",
        "invalid Solana certificate epoch".into(),
    ))?;
    let signing_client = route.client_at(certificate_epoch).ok_or((
        "solana_source_mismatch",
        "certificate epoch has no authenticated client".into(),
    ))?;
    if values.validator_set_hash != signing_client.validator_set_hash
        || values.total_stake.to_string() != signing_client.total_stake
    {
        return Err((
            "solana_validator_set_mismatch",
            "Solana proof stake or validator set differs from the current client".into(),
        ));
    }
    let binding = postfiat_types::solana_deposit_route_binding(
        &genesis.chain_id,
        &genesis_hash(genesis),
        &profile.route_id,
        &pins.genesis_hash,
        &profile.vault_address,
    )
    .map_err(|e| ("invalid_solana_route", e))?;
    if values.route_binding != pins.route_binding
        || binding != pins.route_binding
        || values.shred_version != pins.shred_version
    {
        return Err((
            "solana_route_mismatch",
            "Solana route binding or shred version mismatch".into(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for evidence in &values.receipts {
        if evidence.program_id != profile.vault_address
            || evidence.slot < pins.anchor_slot
            || pins
                .slot_epoch(evidence.slot)
                .is_none_or(|e| e < certificate_epoch.saturating_sub(1))
        {
            return Err((
                "solana_source_mismatch",
                "Solana vault or slot outside current anchor".into(),
            ));
        }
        if !ids.insert(&evidence.deposit_id) {
            return Err((
                "duplicate_solana_deposit",
                "duplicate Solana deposit ID".into(),
            ));
        }
    }
    if ledger
        .vault_bridge_receipts
        .iter()
        .filter_map(|r| r.solana_deposit_evidence.as_ref())
        .any(|e| ids.contains(&e.deposit_id))
    {
        return Err((
            "duplicate_solana_deposit",
            "Solana deposit already credited".into(),
        ));
    }
    let family = ledger
        .asset_definition(&profile.asset_id)
        .cloned()
        .ok_or(("missing_asset", "Solana asset family missing".into()))?;
    let bucket = vault_bridge_bucket_id(&family.asset_id, &profile.source_domain(), hash)
        .map_err(|e| ("invalid_solana_route", e))?;
    let series_id =
        postfiat_types::solana_source_series_id(&genesis.chain_id, &family.asset_id, &bucket);
    for evidence in values.receipts {
        let amount = evidence.amount_atoms;
        let recipient = evidence.recipient.clone();
        let receipt = VaultBridgeReceipt::from_solana(&genesis.chain_id, profile, evidence, height)
            .map_err(|e| ("bad_vault_bridge_receipt", e))?;
        let evidence_root = receipt.receipt_id.clone();
        let supply =
            bridge_supply_after_claim(ledger, &family, amount, orchard_balances, compatibility)?;
        let nav = ledger.nav_asset(&family.asset_id).ok_or((
            "missing_nav_asset",
            "Solana asset family has no NAV state".into(),
        ))?;
        let cap = nav.circulating_supply;
        let checkpoint = if supply > cap {
            if supply - cap > amount {
                return Err((
                    "proof_bounded_nav_cap_exceeded",
                    "cap growth exceeds proven Solana backing".into(),
                ));
            }
            Some(
                proof_bounded_nav_cap_checkpoint_hash(
                    &family.asset_id,
                    &profile.route_id,
                    &evidence_root,
                    cap,
                    supply,
                )
                .map_err(|e| ("bad_proof_bounded_nav_checkpoint", e))?,
            )
        } else {
            None
        };
        if recipient == nav.issuer {
            return Err((
                "unsupported_issuer_mint",
                "Solana deposit requires a holder trustline account".into(),
            ));
        }
        let (series, create_series) = bridge_source_series_asset(
            ledger,
            &family,
            &series_id,
            &bucket,
            format!("{} · Solana", family.code),
        )?;
        let settlement = if compatibility.asset_gate_v1_active(height) {
            match asset_gate(
                ledger,
                &series,
                None,
                Some(&recipient),
                AssetMovementOpKind::BridgeCredit,
                height,
            ) {
                AssetGateOutcome::Allow => {
                    let index = issued_asset_credit_recipient_line_index_gated(
                        ledger,
                        &series,
                        &recipient,
                        amount,
                        "Solana deposit",
                    )?;
                    let (balance, limit) = prepare_issued_asset_credit_gated(
                        ledger,
                        &series,
                        &recipient,
                        index,
                        amount,
                        "Solana deposit",
                    )?;
                    BridgeCreditSettlement::Trustline {
                        recipient_index: index,
                        recipient_after: balance,
                        recipient_required: limit,
                    }
                }
                AssetGateOutcome::RouteToHold { reason_code } => {
                    BridgeCreditSettlement::Hold(Box::new(BridgeCreditHold {
                        asset: series.clone(),
                        recipient: recipient.clone(),
                        source_reference: receipt.source_tx_or_attestation.clone(),
                        reason_code,
                    }))
                }
                AssetGateOutcome::AllowReturnOnly => {
                    return Err((
                        "asset_gate_recipient_ineligible",
                        "Solana deposit is not a return-only exit".into(),
                    ));
                }
                AssetGateOutcome::Reject { code, message } => return Err((code, message)),
            }
        } else {
            let index = issued_asset_credit_recipient_line_index(
                ledger,
                &series,
                &recipient,
                amount,
                "Solana deposit",
            )?;
            let (balance, limit) = prepare_issued_asset_credit(
                ledger,
                &series,
                &recipient,
                index,
                amount,
                "Solana deposit",
            )?;
            BridgeCreditSettlement::Trustline {
                recipient_index: index,
                recipient_after: balance,
                recipient_required: limit,
            }
        };
        if let Some(event) = commit_bridge_receipt_credit_settled(
            genesis,
            ledger,
            receipt,
            &evidence_root,
            &family.asset_id,
            height,
            true,
            create_series,
            settlement,
        )? {
            bridge_hold_events.push(event);
        }
        update_bridge_cap(ledger, &family.asset_id, supply, height, checkpoint)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_types::{
        Account, SolanaLightClient, SolanaRoutePins, VaultBridgeRouteProfileV1, SOLANA_USDC_ROUTE,
    };

    fn setup() -> (Genesis, LedgerState, SolanaRouteState, Vec<u8>) {
        setup_on("postfiat-local")
    }

    fn setup_on(chain: &str) -> (Genesis, LedgerState, SolanaRouteState, Vec<u8>) {
        let genesis = Genesis::new(chain);
        let family =
            AssetDefinition::new(&genesis.chain_id, "test-issuer", "pfUSDC", 1, 6).unwrap();
        let client = SolanaLightClient {
            epoch: 10,
            validator_set_hash: "04".repeat(32),
            total_stake: "600".into(),
            known_key_table_hash: "05".repeat(32),
            handoff: None,
        };
        let profile = VaultBridgeRouteProfileV1 {
            schema: postfiat_types::VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1.into(),
            route_id: SOLANA_USDC_ROUTE.into(),
            asset_id: family.asset_id.clone(),
            source_chain_id: 0,
            vault_address: "07".repeat(32),
            token_address: "08".repeat(32),
            vault_runtime_code_hash: String::new(),
            token_runtime_code_hash: String::new(),
            route_epoch: 1,
            verifier_kind: SOLANA_DEPOSIT_PROOF.into(),
            verifier_program_vkey: format!("0x{}", "01".repeat(32)),
            verifier_policy_hash: String::new(),
            verifier_proof_encoding: postfiat_types::NAV_SP1_PROOF_ENCODING_GROTH16.into(),
            evidence_tier: postfiat_types::VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN.into(),
            min_attestations: 0,
            minimum_confirmations: 0,
            min_challenge_bond: 0,
            max_proof_bytes: 4096,
            max_public_values_bytes: 16384,
            max_snapshot_age_blocks: 100,
            challenge_window_blocks: 1,
            max_epoch_gap_blocks: 100,
            settle_deadline_blocks: 10,
            activation_height: 10,
            expires_at_height: 1000,
            solana: Some(SolanaRoutePins {
                genesis_hash: "01".repeat(32),
                route_binding: postfiat_types::solana_deposit_route_binding(
                    &genesis.chain_id,
                    &genesis_hash(&genesis),
                    SOLANA_USDC_ROUTE,
                    &"01".repeat(32),
                    &"07".repeat(32),
                )
                .unwrap(),
                shred_version: 50093,
                handoff_program_vkey: format!("0x{}", "02".repeat(32)),
                slots_per_epoch: 100,
                epoch_warmup: false,
                recorder_program: "06".repeat(32),
                anchor_slot: 1010,
                anchor: client.clone(),
            }),
        };
        let mut values = b"PFSOLDP1".to_vec();
        values.extend([4; 32]);
        values.extend(600u128.to_le_bytes());
        values.push(1);
        values.extend(1011u64.to_le_bytes());
        values.extend([3; 32]);
        values.extend(50093u16.to_le_bytes());
        values.extend([6; 64]);
        values.extend([7; 32]);
        values.extend(hex_to_bytes(&profile.solana.as_ref().unwrap().route_binding).unwrap());
        values.extend(1u32.to_le_bytes());
        values.extend(1u32.to_le_bytes());
        values.extend([9; 32]);
        values.extend([10; 32]);
        values.extend([11; 20]);
        values.extend(5_000_000u64.to_le_bytes());
        values.extend([12; 32]);
        values.extend([13; 32]);
        let route = SolanaRouteState {
            profile,
            client,
            epoch_history: Default::default(),
        };
        let mut ledger = LedgerState::new(vec![Account::new(
            format!("pf{}", "0b".repeat(20)),
            1000,
            None,
        )]);
        ledger.nav_assets.push(
            NavTrackedAsset::new(
                &family.asset_id,
                &family.issuer,
                &family.issuer,
                "fixture-profile",
                "USDC_ATOMS",
                &family.issuer,
            )
            .unwrap(),
        );
        ledger.asset_definitions.push(family);
        ledger.solana_route = Some(route.clone());
        (genesis, ledger, route, values)
    }

    fn credit(
        genesis: &Genesis,
        ledger: &mut LedgerState,
        route: &SolanaRouteState,
        values: &[u8],
    ) -> Result<(), Rejection> {
        let mut hold_events = Vec::new();
        credit_verified(
            genesis,
            ledger,
            route,
            &route.profile.profile_hash().unwrap(),
            values,
            10,
            AssetExecutionCompatibility::strict(),
            &[],
            &mut hold_events,
        )
    }

    const EXIT_GENESIS: &str = "45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0";
    const EXIT_VAULT: &str = "e2a6fbfcf1e65a0f3e08d400c97d913e3e4eca81f84c814efa0bbcada24aac8b";
    /// The default minimum, the smallest exit burn the chain accepts without a governed value.
    const EXIT_AMOUNT: u64 = postfiat_types::solana_exit::DEFAULT_MIN_WITHDRAWAL_ATOMS;
    const EXIT_MINT: &str = "c6fa7af3bedbad3a3d65f36aabc97431b1bbe4c2d2f6e0e47ca60203452f5d61";

    /// A deposit of the exit family into the shared Solana custody, credited under `revision`.
    /// These tests exercise the second release's rules, so they set it on this thread.
    fn exit_custody_deposit(revision: u32) -> (Genesis, LedgerState) {
        postfiat_types::solana_exit::set_second_release_for_tests(Some(0));
        let (genesis, mut ledger, mut route, mut journal) = setup();
        let family = postfiat_types::solana_exit::FAMILY.to_string();
        ledger.asset_definitions[0].asset_id = family.clone();
        ledger.nav_assets[0].asset_id = family.clone();
        route.profile.asset_id = family;
        route.profile.route_epoch = revision;
        route.profile.vault_address = EXIT_VAULT.into();
        route.profile.token_address = EXIT_MINT.into();
        let pins = route.profile.solana.as_mut().unwrap();
        pins.genesis_hash = EXIT_GENESIS.into();
        pins.route_binding = postfiat_types::solana_deposit_route_binding(
            &genesis.chain_id,
            &genesis_hash(&genesis),
            &route.profile.route_id,
            EXIT_GENESIS,
            EXIT_VAULT,
        )
        .unwrap();
        journal[163..195].copy_from_slice(&hex_to_bytes(EXIT_VAULT).unwrap());
        journal[195..227].copy_from_slice(&hex_to_bytes(&pins.route_binding).unwrap());
        ledger.solana_route = Some(route.clone());
        credit(&genesis, &mut ledger, &route, &journal).unwrap();
        let nav = &mut ledger.nav_assets[0];
        nav.finalized_epoch = 1;
        nav.finalized_reserve_packet_hash = "aa".repeat(48);
        nav.nav_per_unit = 1_000_000;
        (genesis, ledger)
    }

    /// The compatibility a governed, open Solana exit gives the burn path.
    fn open_exit() -> AssetExecutionCompatibility {
        let mut compatibility = AssetExecutionCompatibility::strict()
            .with_pfusdc_source_series_activation_height(Some(0));
        compatibility.solana_exit_heights = Some((0, u64::MAX));
        compatibility.solana_exit_open_height = Some(0);
        compatibility
    }

    fn exit_burn(
        genesis: &Genesis,
        ledger: &mut LedgerState,
        bucket_id: &str,
        amount_atoms: u64,
        destination_ref: String,
    ) -> Result<(), Rejection> {
        exit_burn_with(
            genesis,
            ledger,
            bucket_id,
            amount_atoms,
            destination_ref,
            open_exit(),
        )
    }

    fn exit_burn_with(
        genesis: &Genesis,
        ledger: &mut LedgerState,
        bucket_id: &str,
        amount_atoms: u64,
        destination_ref: String,
        compatibility: AssetExecutionCompatibility,
    ) -> Result<(), Rejection> {
        let owner = ledger.trustlines[0].account.clone();
        let operation = VaultBridgeBurnToRedeemOperation {
            owner: owner.clone(),
            issuer: ledger.nav_assets[0].issuer.clone(),
            asset_id: ledger.nav_assets[0].asset_id.clone(),
            bucket_id: bucket_id.into(),
            amount_atoms,
            epoch: 1,
            reserve_packet_hash: "aa".repeat(48),
            destination_ref,
        };
        let transaction = SignedAssetTransaction {
            unsigned: postfiat_types::UnsignedAssetTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND.to_string(),
                signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                source: owner,
                fee: 0,
                sequence: 1,
                operation: AssetTransactionOperation::VaultBridgeBurnToRedeem(operation.clone()),
            },
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: String::new(),
            signature_hex: String::new(),
        };
        apply_vault_bridge_burn_to_redeem(
            genesis,
            ledger,
            &transaction,
            &operation,
            20,
            compatibility,
            &[],
        )
    }

    include!("solana_exit_first_release_cases.rs");
    include!("solana_exit_release_height_cases.rs");

    /// `release_height_cases::outcomes()` run on origin/main a720cc80f: the issuer-payment
    /// refusal, the closed-outside-interval refusal and the transparent supply-delta check at
    /// 1840, 1841 and 1842 on the WAN genesis.
    const RELEASE_HEIGHT_GOLDEN: [(&str, &str, &str); 12] = [
        (
            "issuer_payment@1840",
            "accepted",
            "ab1d71a239b489b9d7103d5caeda8f1c7145b946c82b9553c24b0b1802bd391a0a4840e5c4b13ed69507b3a36752d99f",
        ),
        (
            "ungoverned_exit_burn@1840",
            "missing_vault_bridge_pinned_profile: vault bridge lifecycle references a source/policy profile that is not registered",
            "055e800dc33535397cdc759751e30e99e8b77856ee3c37cf3b63b3c8d0dcabab71589e753273b65e090d6b1c87b84831",
        ),
        (
            "governed_exit_burn@1840",
            "accepted",
            "2e697b100a544d7cee80621f05e18292e3d8c4b2b126c362f11f4f7e26bf0328b086143bfa4457d56b3443641d8493e9",
        ),
        (
            "transparent_burn@1840",
            "false: invalid_asset_state: asset.asset_id does not match chain, issuer, code, and version",
            "88719d7679b5e9490e4e09dfc85da62d5960823b7742c3d30704cfb3ca5ef0e57495e7960dbc2b017bc3e5dd1cca8ef6",
        ),
        (
            "issuer_payment@1841",
            "solana_exit_issuer_payment_disabled: revision-8 issuer payments cannot create redeemable claims",
            "5e2caa8624f9e9fb5e489572500ec53cfe462244d429a33fb1f459caaabda40fce434c3c3985d027d5385e9957ce2eaf",
        ),
        (
            "ungoverned_exit_burn@1841",
            "solana_exit_burns_closed: Solana exit burns are closed outside the governed descriptor interval",
            "055e800dc33535397cdc759751e30e99e8b77856ee3c37cf3b63b3c8d0dcabab71589e753273b65e090d6b1c87b84831",
        ),
        (
            "governed_exit_burn@1841",
            "accepted",
            "5e6147c9cf5c6c5467f1331b5db6ada8118042ee9b33845ad4cab83788a4840e103e3bd3ecdce6982e28eb7c0fab5b98",
        ),
        (
            "transparent_burn@1841",
            "false: solana_exit_unmatched_supply_change: revision-8 supply change -1 requires matching outstanding and counted backing (outstanding change 0, counted change 0)",
            "88719d7679b5e9490e4e09dfc85da62d5960823b7742c3d30704cfb3ca5ef0e57495e7960dbc2b017bc3e5dd1cca8ef6",
        ),
        (
            "issuer_payment@1842",
            "solana_exit_issuer_payment_disabled: revision-8 issuer payments cannot create redeemable claims",
            "5e2caa8624f9e9fb5e489572500ec53cfe462244d429a33fb1f459caaabda40fce434c3c3985d027d5385e9957ce2eaf",
        ),
        (
            "ungoverned_exit_burn@1842",
            "solana_exit_burns_closed: Solana exit burns are closed outside the governed descriptor interval",
            "055e800dc33535397cdc759751e30e99e8b77856ee3c37cf3b63b3c8d0dcabab71589e753273b65e090d6b1c87b84831",
        ),
        (
            "governed_exit_burn@1842",
            "accepted",
            "f648ee88ace441ff9f824301f437950eb28f09bb84d0618086cf78e76353d1db726b7188e79f1f0b4eac2a3bdc01f869",
        ),
        (
            "transparent_burn@1842",
            "false: solana_exit_unmatched_supply_change: revision-8 supply change -1 requires matching outstanding and counted backing (outstanding change 0, counted change 0)",
            "88719d7679b5e9490e4e09dfc85da62d5960823b7742c3d30704cfb3ca5ef0e57495e7960dbc2b017bc3e5dd1cca8ef6",
        ),
    ];

    #[test]
    fn unset_second_release_matches_the_first_release_around_its_height() {
        postfiat_types::solana_exit::set_second_release_for_tests(None);
        let golden: Vec<(String, String, String)> = RELEASE_HEIGHT_GOLDEN
            .iter()
            .map(|(n, v, d)| (n.to_string(), v.to_string(), d.to_string()))
            .collect();
        let outcomes = release_height_cases::outcomes();
        assert_eq!(outcomes, golden);
    }

    /// `first_release_cases::outcomes()` run on origin/main a720cc80f, the live first release.
    const FIRST_RELEASE_GOLDEN: [(&str, &str, &str); 6] = [
        (
            "r8_burn",
            "accepted",
            "97ce70d8d26f9a408615c659545a081a10f76186a9d5659d087c2b67f7553aae5428fbdd886d6b2aee48520ccad80004",
        ),
        (
            "r9_burn",
            "missing_vault_bridge_pinned_profile: vault bridge lifecycle references a source/policy profile that is not registered",
            "fd2f3f30bc0441db7daff96d4e1cbad509bef20f95a2d3e14b713028eec03ef4b40aa4914a15039f83d8d826de014ca9",
        ),
        (
            "r8_off_curve_owner",
            "accepted",
            "02aca8fdd75a949613c3b3efbd4514ad33503e52aa26f5e97168c85165167de454e7436352e1d5e8ece4884d67ca7dfc",
        ),
        (
            "r8_below_minimum",
            "accepted",
            "6c4a59f4bff1034ef2080f6f04dca3e6ce9b1bf20e1238d3036da7a8fa05fa6ec5a9b191e1d0b9c89b118b76cdc3f82b",
        ),
        (
            "r9_below_minimum_off_curve",
            "missing_vault_bridge_pinned_profile: vault bridge lifecycle references a source/policy profile that is not registered",
            "fd2f3f30bc0441db7daff96d4e1cbad509bef20f95a2d3e14b713028eec03ef4b40aa4914a15039f83d8d826de014ca9",
        ),
        (
            "r8_unreconciled",
            "solana_exit_supply_mismatch: revision-8 series supply 5000001 differs from bucket outstanding 5000000",
            "d11cd8620c318be4d8805dd777d00e98940212ae26683aed53c1f60b457eeac4ff051cb540c90109e67b2416dc6e82ef",
        ),
    ];

    fn first_release_outcomes() -> Vec<(&'static str, String, String)> {
        first_release_cases::outcomes()
    }

    fn golden() -> Vec<(&'static str, String, String)> {
        FIRST_RELEASE_GOLDEN
            .iter()
            .map(|(n, v, d)| (*n, v.to_string(), d.to_string()))
            .collect()
    }

    #[test]
    fn unset_second_release_burns_exactly_as_the_first_release() {
        postfiat_types::solana_exit::set_second_release_for_tests(None);
        assert_eq!(first_release_outcomes(), golden());
    }

    #[test]
    fn second_release_burn_rules_start_at_its_height() {
        // The cases burn at block 20: a second release at 21 has not started.
        postfiat_types::solana_exit::set_second_release_for_tests(Some(21));
        assert_eq!(first_release_outcomes(), golden());
        postfiat_types::solana_exit::set_second_release_for_tests(Some(20));
        let second = first_release_outcomes();
        postfiat_types::solana_exit::set_second_release_for_tests(None);
        let verdict = |name: &str| {
            let row = second.iter().find(|(n, _, _)| *n == name).unwrap();
            row.1.split(':').next().unwrap().to_string()
        };
        // Revision 8 burns as before, with the same ledger.
        assert_eq!(second[0], golden()[0]);
        assert_eq!(verdict("r9_burn"), "accepted");
        assert_eq!(
            verdict("r8_off_curve_owner"),
            "solana_exit_destination_not_wallet"
        );
        assert_eq!(verdict("r8_below_minimum"), "solana_exit_below_minimum");
        assert_eq!(
            verdict("r9_below_minimum_off_curve"),
            "solana_exit_destination_not_wallet"
        );
        assert_eq!(verdict("r8_unreconciled"), "solana_exit_supply_mismatch");
        let unreconciled = &second
            .iter()
            .find(|(n, _, _)| *n == "r8_unreconciled")
            .unwrap()
            .1;
        assert!(unreconciled.contains("in bucket"), "{unreconciled}");
    }

    fn exit_destination() -> String {
        format!("solana-spl:{EXIT_GENESIS}:{}", "ce".repeat(32))
    }

    #[test]
    fn every_solana_revision_of_the_custody_redeems_through_the_exit() {
        for revision in [3, 9] {
            let (genesis, mut ledger) = exit_custody_deposit(revision);
            let bucket = ledger.vault_bridge_bucket_states[0].clone();
            assert_ne!(bucket.bucket_id, postfiat_types::solana_exit::BUCKET);
            let line = ledger.trustlines[0].clone();
            assert_eq!(
                line.asset_id,
                postfiat_types::solana_source_series_id(
                    &genesis.chain_id,
                    &bucket.asset_id,
                    &bucket.bucket_id
                )
            );
            exit_burn(
                &genesis,
                &mut ledger,
                &bucket.bucket_id,
                EXIT_AMOUNT,
                exit_destination(),
            )
            .unwrap_or_else(|e| panic!("revision {revision}: {e:?}"));
            let after = ledger.vault_bridge_bucket(&bucket.bucket_id).unwrap();
            assert_eq!(ledger.trustlines[0].balance, line.balance - EXIT_AMOUNT);
            assert_eq!(
                after.outstanding_vault_bridge_atoms,
                bucket.outstanding_vault_bridge_atoms - EXIT_AMOUNT
            );
            assert_eq!(
                after.redemption_queue_atoms,
                bucket.redemption_queue_atoms + EXIT_AMOUNT
            );
            let redemption = ledger.vault_bridge_redemptions.last().unwrap().clone();
            assert_eq!(redemption.bucket_id, bucket.bucket_id);
            assert_eq!(
                redemption.withdrawal_packet.source_bucket_id,
                bucket.bucket_id
            );
            assert_eq!(
                redemption.withdrawal_packet.source_chain_id,
                postfiat_types::SOLANA_EGRESS_PACKET_CHAIN
            );
            // Settlement retires the burn against its own bucket.
            settle_governed_solana_payout(
                &mut ledger,
                &genesis.chain_id,
                &redemption.withdrawal_packet_hash,
                &"5e".repeat(48),
                21,
                true,
            )
            .unwrap_or_else(|e| panic!("revision {revision} settlement: {e}"));
            let settled = ledger.vault_bridge_bucket(&bucket.bucket_id).unwrap();
            assert_eq!(
                settled.redemption_queue_atoms,
                bucket.redemption_queue_atoms
            );
            assert_eq!(
                settled.counted_value_atoms,
                bucket.counted_value_atoms - EXIT_AMOUNT
            );
        }
    }

    #[test]
    fn a_cancelled_burn_is_refunded_once_in_its_own_series_and_bucket() {
        let (genesis, mut ledger) = exit_custody_deposit(9);
        let bucket = ledger.vault_bridge_bucket_states[0].clone();
        let line = ledger.trustlines[0].clone();
        let supply = |l: &LedgerState| crate::issued_asset_supply(l, &line.asset_id).unwrap();
        let supply_before = supply(&ledger);
        exit_burn(
            &genesis,
            &mut ledger,
            &bucket.bucket_id,
            EXIT_AMOUNT,
            exit_destination(),
        )
        .unwrap();
        let packet = ledger
            .vault_bridge_redemptions
            .last()
            .unwrap()
            .withdrawal_packet_hash
            .clone();
        assert_eq!(supply(&ledger), supply_before - EXIT_AMOUNT);
        let refund = "6e".repeat(48);
        assert!(refund_cancelled_solana_burn(
            &mut ledger,
            &genesis.chain_id,
            &"77".repeat(48),
            &refund,
            21
        )
        .is_err());
        refund_cancelled_solana_burn(&mut ledger, &genesis.chain_id, &packet, &refund, 21).unwrap();
        // The burner holds the amount again, in the same series; the bucket's sums are as before.
        let after = ledger.vault_bridge_bucket(&bucket.bucket_id).unwrap();
        assert_eq!(ledger.trustlines[0].balance, line.balance);
        assert_eq!(ledger.trustlines[0].asset_id, line.asset_id);
        assert_eq!(
            after.outstanding_vault_bridge_atoms,
            bucket.outstanding_vault_bridge_atoms
        );
        assert_eq!(after.redemption_queue_atoms, bucket.redemption_queue_atoms);
        assert_eq!(after.counted_value_atoms, bucket.counted_value_atoms);
        assert_eq!(supply(&ledger), supply_before);
        assert_eq!(supply(&ledger), after.outstanding_vault_bridge_atoms);
        let red = ledger.vault_bridge_redemptions.last().unwrap();
        assert_eq!(
            red.state,
            postfiat_types::VAULT_BRIDGE_REDEMPTION_STATE_REFUNDED
        );
        assert_eq!(
            (red.settled_atoms, red.settlement_receipt_hash.as_str()),
            (0, refund.as_str())
        );
        red.validate_for_chain(&genesis.chain_id).unwrap();
        // A second refund, and a payout settlement after the refund, change nothing.
        let before = ledger.clone();
        assert!(
            refund_cancelled_solana_burn(&mut ledger, &genesis.chain_id, &packet, &refund, 22)
                .is_err()
        );
        assert!(settle_governed_solana_payout(
            &mut ledger,
            &genesis.chain_id,
            &packet,
            &"5e".repeat(48),
            22,
            true
        )
        .is_err());
        assert_eq!(ledger, before);
    }

    #[test]
    fn a_paid_burn_cannot_be_refunded() {
        let (genesis, mut ledger) = exit_custody_deposit(9);
        let bucket = ledger.vault_bridge_bucket_states[0].bucket_id.clone();
        exit_burn(
            &genesis,
            &mut ledger,
            &bucket,
            EXIT_AMOUNT,
            exit_destination(),
        )
        .unwrap();
        let packet = ledger
            .vault_bridge_redemptions
            .last()
            .unwrap()
            .withdrawal_packet_hash
            .clone();
        settle_governed_solana_payout(
            &mut ledger,
            &genesis.chain_id,
            &packet,
            &"5e".repeat(48),
            21,
            true,
        )
        .unwrap();
        let before = ledger.clone();
        assert!(refund_cancelled_solana_burn(
            &mut ledger,
            &genesis.chain_id,
            &packet,
            &"6e".repeat(48),
            22
        )
        .is_err());
        assert_eq!(ledger, before);
    }

    #[test]
    fn solana_exit_refuses_owners_that_are_not_wallets() {
        let (genesis, mut ledger) = exit_custody_deposit(9);
        let bucket = ledger.vault_bridge_bucket_states[0].bucket_id.clone();
        let before = ledger.clone();
        for owner in [
            // AhmUaCijypvcbYMDxeXVbUPqjPX1tiHfLH6NWkYnQTCi, the USDC custody token account.
            "902ab403a58407aed4a100bf03f5e6e982f8a433ff7f0c98c8114e346ba96857",
            // FQe34dCQg9m6yEW6NjbjoR8LfWwKGLX2hBvTdY2Lu6s1, the verifier's light PDA.
            "d611da10ce47786f6bc97c0b046215f672a398a70da4525c38ec14808dd41ab8",
        ] {
            let destination = format!("solana-spl:{EXIT_GENESIS}:{owner}");
            assert_eq!(
                exit_burn(&genesis, &mut ledger, &bucket, EXIT_AMOUNT, destination)
                    .unwrap_err()
                    .0,
                "solana_exit_destination_not_wallet"
            );
            assert_eq!(ledger, before);
        }
        // The identity, small-order points and non-canonical encodings are no wallet keys.
        let p_minus_1 = format!("ec{}7f", "ff".repeat(30));
        let p = format!("ed{}7f", "ff".repeat(30));
        let p_plus_1 = format!("ee{}7f", "ff".repeat(30));
        let identity = format!("01{}", "00".repeat(31));
        for owner in [identity, p_minus_1, p, p_plus_1] {
            let destination = format!("solana-spl:{EXIT_GENESIS}:{owner}");
            assert_eq!(
                exit_burn(&genesis, &mut ledger, &bucket, EXIT_AMOUNT, destination)
                    .unwrap_err()
                    .0,
                "solana_exit_destination_not_wallet",
                "{owner}"
            );
            assert_eq!(ledger, before);
        }
        // An Ed25519 wallet key is accepted.
        exit_burn(
            &genesis,
            &mut ledger,
            &bucket,
            EXIT_AMOUNT,
            exit_destination(),
        )
        .unwrap();
    }

    #[test]
    fn solana_exit_refuses_a_burn_below_the_minimum_before_any_debit() {
        let (genesis, mut ledger) = exit_custody_deposit(9);
        let bucket = ledger.vault_bridge_bucket_states[0].bucket_id.clone();
        let before = ledger.clone();
        assert_eq!(
            exit_burn(&genesis, &mut ledger, &bucket, 299_999, exit_destination())
                .unwrap_err()
                .0,
            "solana_exit_below_minimum"
        );
        assert_eq!(ledger, before);
        exit_burn(&genesis, &mut ledger, &bucket, 300_000, exit_destination()).unwrap();
        // A governed lower minimum admits the 0.02 USDC canary amount.
        let mut ledger = before;
        let mut lowered = open_exit();
        lowered.solana_exit_min_atoms = 20_000;
        exit_burn_with(
            &genesis,
            &mut ledger,
            &bucket,
            20_000,
            exit_destination(),
            lowered,
        )
        .unwrap();
    }

    #[test]
    fn solana_exit_supply_checks_cover_a_revision_9_bucket_like_revision_8() {
        let (genesis, mut ledger) = exit_custody_deposit(9);
        let chain = genesis.chain_id.clone();
        let bucket = ledger.vault_bridge_bucket_states[0].clone();
        assert_ne!(bucket.bucket_id, postfiat_types::solana_exit::BUCKET);
        let series = ledger.trustlines[0].asset_id.clone();
        // The revision-9 series is recognised as a covered custody series.
        assert_eq!(
            solana_exit_series_bucket(&chain, &ledger, &series, true),
            Some(bucket.bucket_id.clone())
        );
        assert_eq!(solana_exit_series_bucket(&chain, &ledger, "00", true), None);
        // Its series supply reconciles with its own bucket, and revision 8 stays covered.
        let claims = SolanaExitAccounting::read(&chain, &ledger, &[], true).unwrap();
        let own = claims.bucket(&bucket.bucket_id).unwrap();
        assert_eq!(
            (own.series_id.as_str(), own.supply),
            (series.as_str(), bucket.outstanding_vault_bridge_atoms)
        );
        assert!(claims.bucket(postfiat_types::solana_exit::BUCKET).is_some());
        claims.require_reconciled().unwrap();
        // An Orchard balance of the series counts as supply, as for revision 8.
        let orchard = [AssetOrchardAssetBalance {
            asset_id: series.clone(),
            ingress_total: 1,
            egress_total: 0,
            live_total: 1,
        }];
        let shielded = SolanaExitAccounting::read(&chain, &ledger, &orchard, true).unwrap();
        let error = shielded.require_reconciled().unwrap_err();
        assert_eq!(error.0, "solana_exit_supply_mismatch");
        assert!(error.1.contains(&bucket.bucket_id), "{}", error.1);
        // A supply change without matching outstanding is refused; a paired credit with
        // matching outstanding and counted backing passes.
        let mut grown = ledger.clone();
        grown.trustlines[0].balance += 1;
        let after = SolanaExitAccounting::read(&chain, &grown, &[], true).unwrap();
        assert_eq!(
            claims.verify_change(&after).unwrap_err().0,
            "solana_exit_unmatched_supply_change"
        );
        let mut backed = ledger.clone();
        let b = backed
            .vault_bridge_bucket_states
            .iter_mut()
            .find(|b| b.bucket_id == bucket.bucket_id)
            .unwrap();
        b.outstanding_vault_bridge_atoms += 1;
        b.counted_value_atoms += 1;
        let mut after = SolanaExitAccounting::read(&chain, &backed, &[], true).unwrap();
        after.credit(&series, 1).unwrap();
        claims.verify_change(&after).unwrap();
        // A burn from an unreconciled revision-9 bucket is refused before any debit.
        ledger.trustlines[0].balance -= 1;
        let before = ledger.clone();
        let refused = exit_burn(
            &genesis,
            &mut ledger,
            &bucket.bucket_id,
            EXIT_AMOUNT,
            exit_destination(),
        );
        assert_eq!(refused.unwrap_err().0, "solana_exit_supply_mismatch");
        assert_eq!(ledger, before);
    }

    #[test]
    fn solana_exit_keeps_each_bucket_limit_and_refuses_cross_cases() {
        let (genesis, mut ledger) = exit_custody_deposit(9);
        let bucket = ledger.vault_bridge_bucket_states[0].clone();
        // More than the bucket holds, even with a larger series balance. The series
        // supply then exceeds the bucket outstanding, which the supply check refuses
        // before the bucket limit is reached.
        ledger.trustlines[0].balance += 1;
        let before = ledger.clone();
        let over = bucket.outstanding_vault_bridge_atoms + 1;
        assert_eq!(
            exit_burn(
                &genesis,
                &mut ledger,
                &bucket.bucket_id,
                over,
                exit_destination()
            )
            .unwrap_err()
            .0,
            "solana_exit_supply_mismatch"
        );
        assert_eq!(ledger, before);
        // An EVM destination cannot leave a Solana bucket.
        let evm = format!("evm-erc20:1:0x{}", "22".repeat(20));
        assert!(exit_burn(&genesis, &mut ledger, &bucket.bucket_id, EXIT_AMOUNT, evm).is_err());
        assert_eq!(ledger, before);
        // A Solana bucket of another custody does not exit through this one.
        let (genesis, mut other, _, values) = setup();
        let route = other.solana_route.clone().unwrap();
        credit(&genesis, &mut other, &route, &values).unwrap();
        other.nav_assets[0].finalized_epoch = 1;
        other.nav_assets[0].finalized_reserve_packet_hash = "aa".repeat(48);
        other.nav_assets[0].nav_per_unit = 1_000_000;
        let other_bucket = other.vault_bridge_bucket_states[0].bucket_id.clone();
        let before = other.clone();
        let destination = format!("solana-spl:{}:{}", "01".repeat(32), "ce".repeat(32));
        assert!(exit_burn(
            &genesis,
            &mut other,
            &other_bucket,
            EXIT_AMOUNT,
            destination
        )
        .is_err());
        assert_eq!(other, before);
    }

    #[test]
    fn solana_bridge_route_binding_matches_vault_initialization_bytes() {
        let binding = postfiat_types::solana_deposit_route_binding(
            "postfiat-wan-devnet-2",
            "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9",
            SOLANA_USDC_ROUTE, &"01".repeat(32), &"07".repeat(32),
        ).unwrap();
        assert_eq!(
            binding,
            "69e3b20d329221662ff58b4f8c7060f11fee756a4f9feb02fc337e4a5762d934"
        );
    }

    #[test]
    fn cluster_route_binding_and_backing_agree_with_governance() {
        for route_id in ["solana-devnet-usdc-v1", "solana-local-usdc-v1"] {
            let (genesis, mut ledger, mut route, mut journal) = setup();
            route.profile.route_id = route_id.into();
            let pins = route.profile.solana.as_mut().unwrap();
            pins.route_binding = postfiat_types::solana_deposit_route_binding(
                &genesis.chain_id,
                &genesis_hash(&genesis),
                route_id,
                &pins.genesis_hash,
                &route.profile.vault_address,
            )
            .unwrap();
            journal[195..227].copy_from_slice(&hex_to_bytes(&pins.route_binding).unwrap());
            ledger.solana_route = Some(route.clone());
            credit(&genesis, &mut ledger, &route, &journal).unwrap();
            ledger.validate_asset_state(&genesis.chain_id).unwrap();
            let rows = ledger
                .vault_bridge_route_backing(&route.profile.asset_id)
                .unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].route_id, route_id);
            assert_eq!(rows[0].claims_minted, 5_000_000);
        }
    }

    #[test]
    fn solana_bridge_verified_receipt_credits_the_source_series_and_rejects_replay() {
        let (genesis, mut ledger, route, values) = setup();
        credit(&genesis, &mut ledger, &route, &values).unwrap();
        ledger.validate_asset_state(&genesis.chain_id).unwrap();
        let family_id = &route.profile.asset_id;
        let line = &ledger.trustlines[0];
        assert_ne!(&line.asset_id, family_id);
        let series = ledger.asset_definition(&line.asset_id).unwrap();
        assert_eq!(&series.asset_family_id, family_id);
        assert_eq!(
            series.source_bucket_id,
            ledger.vault_bridge_bucket_states[0].bucket_id
        );
        assert_eq!(line.balance, 5_000_000);
        assert_eq!(ledger.nav_assets[0].circulating_supply, line.balance);
        assert_eq!(
            ledger.vault_bridge_allocations[0].amount_atoms,
            line.balance
        );
        let row = ledger
            .vault_bridge_route_backing(family_id)
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            (
                row.route_id.as_str(),
                row.deposits_verified,
                row.claims_minted
            ),
            (SOLANA_USDC_ROUTE, line.balance, line.balance)
        );
        let before = ledger.clone();
        assert_eq!(
            credit(&genesis, &mut ledger, &route, &values)
                .unwrap_err()
                .0,
            "duplicate_solana_deposit"
        );
        assert_eq!(ledger, before);
    }

    #[test]
    fn solana_bridge_rejects_wrong_set_stake_vault_shred_version_epoch_and_route_binding() {
        let (genesis, mut ledger, route, values) = setup();
        for (offset, code) in [
            (8, "solana_validator_set_mismatch"),
            (40, "solana_validator_set_mismatch"),
            (58, "solana_source_mismatch"),
            (97, "solana_route_mismatch"),
            (163, "solana_source_mismatch"),
            (195, "solana_route_mismatch"),
        ] {
            let mut changed = values.clone();
            changed[offset] ^= 1;
            let before = ledger.clone();
            assert_eq!(
                credit(&genesis, &mut ledger, &route, &changed)
                    .unwrap_err()
                    .0,
                code
            );
            assert_eq!(before, ledger);
        }
    }

    #[test]
    fn solana_bridge_descendant_epoch_selects_the_client_and_preserves_receipt_slot() {
        let (genesis, mut ledger, mut route, mut values) = setup();
        // B=1011 in epoch 10. D=1101 in epoch 11. The authenticated set follows D.
        values[..8].copy_from_slice(b"PFSOLDP2");
        values.extend(1101u64.to_le_bytes());
        assert_eq!(
            credit(&genesis, &mut ledger, &route, &values)
                .unwrap_err()
                .0,
            "solana_source_mismatch"
        );
        route.client.epoch = 11;
        ledger.solana_route = Some(route.clone());
        credit(&genesis, &mut ledger, &route, &values).unwrap();
        let receipt = ledger.vault_bridge_receipts[0]
            .solana_deposit_evidence
            .as_ref()
            .unwrap();
        assert_eq!(receipt.slot, 1011);
        let end = values.len();
        values[end - 8..].copy_from_slice(&1011u64.to_le_bytes());
        assert!(SolanaDepositPublicValues::decode(&values).is_err());
        values[end - 8..].copy_from_slice(&1201u64.to_le_bytes());
        route.client.epoch = 12;
        assert_eq!(
            credit(&genesis, &mut ledger, &route, &values)
                .unwrap_err()
                .0,
            "solana_source_mismatch"
        );
    }

    #[test]
    fn solana_bridge_refuses_partial_extra_and_duplicate_receipts() {
        let (genesis, mut ledger, route, mut values) = setup();
        for invalid in [&values[..234], &values[..values.len() - 1]] {
            assert!(SolanaDepositPublicValues::decode(invalid).is_err());
        }
        values.push(0);
        assert!(SolanaDepositPublicValues::decode(&values).is_err());
        values.pop();
        values.extend_from_within(235..);
        values[227..231].copy_from_slice(&2u32.to_le_bytes());
        values[231..235].copy_from_slice(&2u32.to_le_bytes());
        let before = ledger.clone();
        assert_eq!(
            credit(&genesis, &mut ledger, &route, &values)
                .unwrap_err()
                .0,
            "duplicate_solana_deposit"
        );
        assert_eq!(ledger, before);
    }
    #[test]
    fn solana_bridge_restored_client_must_match_the_anchor_before_its_first_handoff() {
        let (genesis, mut ledger, route, _) = setup();
        let mut substituted = route.clone();
        substituted.client.total_stake = "1".into();
        ledger.solana_route = Some(substituted);
        assert!(ledger.validate_asset_state(&genesis.chain_id).is_err());
        ledger.solana_route = Some(route);
        ledger.validate_asset_state(&genesis.chain_id).unwrap();
    }

    #[test]
    fn solana_bridge_handoff_requires_the_current_set_keys_recorder_and_successor_epoch() {
        let (_, _, route, _) = setup();
        let mut journal = b"PFSOLHP1".to_vec();
        journal.extend([6; 32]);
        journal.extend(1100u64.to_le_bytes());
        journal.extend(50093u16.to_le_bytes());
        journal.extend(100u64.to_le_bytes());
        journal.push(0);
        journal.extend(11u64.to_le_bytes());
        journal.extend([4; 32]);
        journal.extend([7; 32]);
        journal.extend(900u64.to_le_bytes());
        journal.extend([5; 32]);
        let next = route.handoff_successor(&journal).unwrap();
        assert_eq!((next.epoch, next.total_stake.as_str()), (11, "900"));
        assert_eq!(next.known_key_table_hash, route.client.known_key_table_hash);
        assert_eq!(next.validator_set_hash, "07".repeat(32));
        for offset in [0, 8, 41, 48, 50, 58, 59, 67, 139] {
            let mut changed = journal.clone();
            changed[offset] ^= 1;
            assert!(route.handoff_successor(&changed).is_err(), "byte {offset}");
        }
        assert!(route.handoff_successor(&journal[..170]).is_err());
        journal.push(0);
        assert!(route.handoff_successor(&journal).is_err());
    }
    #[test]
    fn solana_release_handoff_proofs_advance_two_epochs() {
        let (genesis, mut ledger, mut route, _) = setup();
        let anchor = SolanaLightClient {
            epoch: 5,
            validator_set_hash: "2aaf446972f5d12e0a090aa6c61b337676e54a90f3632524f43863830a57d7e2"
                .into(),
            total_stake: "100".into(),
            known_key_table_hash:
                "ee71934093715c8b6a08168ab84173cd6d910179471d39dd7e5dc16ba7557595".into(),
            handoff: None,
        };
        let pins = route.profile.solana.as_mut().unwrap();
        pins.anchor = anchor.clone();
        pins.anchor_slot = 320;
        pins.slots_per_epoch = 64;
        pins.recorder_program = "11".repeat(32);
        pins.handoff_program_vkey = postfiat_types::SOLANA_RELEASE_HANDOFF_VKEY.into();
        route.client = anchor;
        let hash = route.profile.profile_hash().unwrap();
        ledger.solana_route = Some(route);
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../node/testdata/solana-bridge/ranked");
        for (epoch, total) in [(6, "100"), (7, "105")] {
            let operation = SolanaProofOperation {
                submitter: format!("pf{}", "0b".repeat(20)),
                profile_hash: hash.clone(),
                proof_kind: postfiat_types::SOLANA_HANDOFF_PROOF.into(),
                proof: std::fs::read(directory.join(format!("epoch-{epoch}.groth16"))).unwrap(),
                public_values: std::fs::read(directory.join(format!("epoch-{epoch}.values")))
                    .unwrap(),
            };
            apply_proof(
                &genesis,
                &mut ledger,
                &operation,
                10,
                AssetExecutionCompatibility::strict(),
                &[],
                &mut Vec::new(),
            )
            .unwrap();
            let client = &ledger.solana_route.as_ref().unwrap().client;
            assert_eq!((client.epoch, client.total_stake.as_str()), (epoch, total));
            assert_eq!(client.handoff.as_ref().unwrap().listed_stake, 100);
            let before = ledger.clone();
            let error = apply_proof(
                &genesis,
                &mut ledger,
                &operation,
                10,
                AssetExecutionCompatibility::strict(),
                &[],
                &mut Vec::new(),
            )
            .unwrap_err();
            assert_eq!(error.0, "invalid_solana_handoff");
            assert_eq!(ledger, before);
        }
        let rotated = ledger.solana_route.as_mut().unwrap();
        let live = rotated.client.clone();
        let anchor = rotated.profile.solana.as_ref().unwrap().anchor.clone();
        rotated.epoch_history.insert(anchor.epoch, anchor);
        let backfill = SolanaProofOperation {
            submitter: format!("pf{}", "0b".repeat(20)),
            profile_hash: hash,
            proof_kind: postfiat_types::SOLANA_HANDOFF_PROOF.into(),
            proof: std::fs::read(directory.join("epoch-6.groth16")).unwrap(),
            public_values: std::fs::read(directory.join("epoch-6.values")).unwrap(),
        };
        apply_proof(
            &genesis,
            &mut ledger,
            &backfill,
            10,
            AssetExecutionCompatibility::strict(),
            &[],
            &mut Vec::new(),
        )
        .unwrap();
        let rotated = ledger.solana_route.as_ref().unwrap();
        assert_eq!(rotated.client, live);
        let six = rotated.client_at(6).unwrap();
        assert_eq!(
            six.validator_set_hash,
            live.handoff.as_ref().unwrap().predecessor_set_hash
        );
        let before = ledger.clone();
        assert!(apply_proof(
            &genesis,
            &mut ledger,
            &backfill,
            10,
            AssetExecutionCompatibility::strict(),
            &[],
            &mut Vec::new(),
        )
        .is_err());
        assert_eq!(ledger, before);
    }

    #[test]
    fn solana_pinned_guest_journals_match_release_admission() {
        use guest::handoff::{HandoffPublicValues, RecorderContext};
        use pfusdc_sol_ingress_program as guest;

        let (_, _, mut route, _) = setup();
        let info: serde_json::Value = serde_json::from_str(include_str!(
            "../../../programs/pfusdc-sol-ingress/program-info.json"
        ))
        .unwrap();
        let handoff_key = info["programs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == "handoff")
            .unwrap()["program_vkey"]
            .as_str()
            .unwrap();
        assert_eq!(handoff_key, postfiat_types::SOLANA_RELEASE_HANDOFF_VKEY);
        route.profile.solana.as_mut().unwrap().handoff_program_vkey = handoff_key.into();
        let mut proof = HandoffPublicValues {
            context: RecorderContext {
                recorder_program: [6; 32],
                slot: 1100,
                shred_version: 50093,
                slots_per_epoch: 100,
                warmup: false,
                epoch: 11,
                trusted_set_hash: [4; 32],
            },
            new_set_hash: [7; 32],
            new_total_stake: 900,
            listed_stake: 720,
            known_key_table_hash: [5; 32],
        };
        let encoded = proof.encode();
        let mut obsolete = encoded.clone();
        obsolete[7] = b'1';
        obsolete.drain(139..147);
        assert!(route.handoff_successor(&obsolete).is_err());
        route.client = route.handoff_successor(&encoded).unwrap();
        assert!(route.handoff_successor(&encoded).is_err());
        proof.listed_stake = 719;
        assert!(route.handoff_successor(&proof.encode()).is_err());
        proof.listed_stake = 850;
        proof.new_total_stake = 901;
        assert!(route.handoff_successor(&proof.encode()).is_err());
        proof.new_total_stake = 900;
        proof.context.trusted_set_hash = [7; 32];
        assert!(route.handoff_successor(&proof.encode()).is_err());
        proof.context.trusted_set_hash = [4; 32];
        proof.new_set_hash = [8; 32];
        route.client = route.handoff_successor(&proof.encode()).unwrap();
        assert_eq!(route.client.handoff.as_ref().unwrap().listed_stake, 850);
        proof.context.trusted_set_hash = [8; 32];
        proof.context.epoch = 12;
        proof.context.slot = 1200;
        proof.new_set_hash = [9; 32];
        route.client = route.handoff_successor(&proof.encode()).unwrap();
        assert_eq!(route.client.epoch, 12);
        assert!(route.handoff_successor(&encoded).is_err());

        let journal = guest::DepositPublicValues {
            validator_set_hash: [9; 32],
            total_stake: 900,
            fast_finality: true,
            block: guest::BlockPublicValues {
                slot: 1201,
                block_id: [1; 32],
                shred_version: 50093,
                state: guest::StatePublicValues {
                    bank_hash: [2; 32],
                    parent_bank_hash: [3; 32],
                    vault_program: [4; 32],
                    route_binding: [5; 32],
                    changed_accounts: 2,
                    receipts: vec![guest::Receipt {
                        address: [6; 32],
                        pftl_recipient: [7; 20],
                        amount: 42,
                        nonce: [8; 32],
                        depositor: [10; 32],
                        deposit_id: [11; 32],
                    }],
                },
            },
        };
        let decoded = SolanaDepositPublicValues::decode(&journal.encode()).unwrap();
        assert_eq!(decoded.validator_set_hash, "09".repeat(32));
        assert_eq!(decoded.total_stake, 900);
        assert_eq!(decoded.route_binding, "05".repeat(32));
        let receipt = &decoded.receipts[0];
        assert_eq!(receipt.slot, 1201);
        assert_eq!(receipt.program_id, "04".repeat(32));
        assert_eq!(receipt.recipient, format!("pf{}", "07".repeat(20)));
        assert_eq!(receipt.amount_atoms, 42);
        assert_eq!(receipt.deposit_id, "0b".repeat(32));
    }
}
