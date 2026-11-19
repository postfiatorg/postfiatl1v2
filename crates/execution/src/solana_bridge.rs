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
) -> Result<(), Rejection> {
    let secondary = ledger.additional_solana_routes.contains_key(&operation.profile_hash);
    let route = ledger.additional_solana_routes.get(&operation.profile_hash)
        .or(ledger.solana_route.as_ref()).cloned().ok_or((
        "solana_bridge_disabled",
        "Solana route has no activation".into(),
    ))?;
    let profile = &route.profile;
    if ledger.solana_clients().any(|other| other.profile.route_id == profile.route_id
        && other.profile.route_epoch > profile.route_epoch)
    {
        return Err(("solana_route_superseded", "Solana route revision has been retired".into()));
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
        let next = route
            .handoff_successor(&operation.public_values)
            .map_err(|e| ("invalid_solana_handoff", e))?;
        let client = if secondary {
            ledger.additional_solana_routes.get_mut(&operation.profile_hash)
        } else {
            ledger.solana_route.as_mut()
        };
        client.expect("route loaded above").client = next;
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
) -> Result<(), Rejection> {
    let profile = &route.profile;
    let pins = profile
        .solana
        .as_ref()
        .ok_or(("invalid_solana_route", "missing Solana pins".into()))?;
    let values = SolanaDepositPublicValues::decode(public_values)
        .map_err(|e| ("invalid_solana_deposit_values", e))?;
    if values.validator_set_hash != route.client.validator_set_hash
        || values.total_stake.to_string() != route.client.total_stake
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
            || pins.slot_epoch(evidence.slot) != Some(route.client.epoch)
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
    let series_id = postfiat_types::solana_source_series_id(&genesis.chain_id, &family.asset_id, &bucket);
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
        commit_bridge_receipt_credit(
            genesis,
            ledger,
            receipt,
            &evidence_root,
            &family.asset_id,
            height,
            true,
            create_series,
            index,
            balance,
            limit,
        )?;
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
        let genesis = Genesis::new("postfiat-local");
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
        let route = SolanaRouteState { profile, client };
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
        credit_verified(
            genesis,
            ledger,
            route,
            &route.profile.profile_hash().unwrap(),
            values,
            10,
            AssetExecutionCompatibility::strict(),
            &[],
        )
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
            )
            .unwrap_err();
            assert_eq!(error.0, "invalid_solana_handoff");
            assert_eq!(ledger, before);
        }
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
