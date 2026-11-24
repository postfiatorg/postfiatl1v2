// Release-gated cases on the WAN genesis (`RELEASES` height 1841), shared with origin/main
// a720cc80f: the same file is compiled into both trees, and main's outputs are pinned.
mod release_height_cases {
    use super::*;

    /// The WAN genesis: its hash is the one `RELEASES` registers.
    fn wan() -> Genesis {
        let mut genesis = Genesis::new_with_validator_count("postfiat-wan-devnet-2", 6);
        genesis.consensus_v2_activation_height = Some(1);
        genesis
    }

    fn digest(ledger: &LedgerState) -> String {
        postfiat_types::pfusdc_ingress_proof_hash_v1(&serde_json::to_vec(ledger).unwrap())
    }

    fn verdict(result: Result<(), Rejection>) -> String {
        match result {
            Ok(()) => "accepted".into(),
            Err((code, message)) => format!("{code}: {message}"),
        }
    }

    /// A ledger holding the revision-8 series: `holder` owns 5 atoms, `issuer` issues it.
    fn series_ledger(genesis: &Genesis, holder: &str) -> LedgerState {
        use postfiat_types::solana_exit::SERIES;
        let mut asset = AssetDefinition::new(&genesis.chain_id, "issuer", "PFUSDC", 1, 6).unwrap();
        asset.asset_id = SERIES.into();
        let line: postfiat_types::TrustLine = serde_json::from_value(serde_json::json!({
            "trustline_id": "line-1", "account": holder, "issuer": "issuer",
            "asset_id": SERIES, "limit": 1_000_000u64, "balance": 5u64, "authorized": true,
        }))
        .unwrap();
        LedgerState::new_with_assets(
            vec![
                Account::new(holder, 1_000_000_000, None),
                Account::new("issuer", 1_000_000_000, None),
            ],
            vec![asset],
            vec![line],
        )
    }

    fn issuer_payment(height: u64) -> (String, String) {
        use postfiat_types::solana_exit::SERIES;
        let genesis = wan();
        let mut ledger = series_ledger(&genesis, "holder");
        let pay = IssuedPaymentOperation {
            from: "issuer".into(),
            to: "holder".into(),
            issuer: "issuer".into(),
            amount: 1,
            asset_id: SERIES.into(),
        };
        let compat = AssetExecutionCompatibility::strict()
            .with_bridge_verification_activation_height(None)
            .with_governed_solana_exit(&genesis, &postfiat_types::GovernanceState::new(6));
        let v = verdict(crate::apply_issued_payment(
            &genesis,
            &mut ledger,
            &pay,
            height,
            compat,
        ));
        (v, digest(&ledger))
    }

    /// A revision-8 burn to a Solana destination; `governed` opens the exit interval.
    fn exit_burn(height: u64, governed: bool) -> (String, String) {
        let genesis = wan();
        let (_, ledger) = first_release_cases::revision_8();
        let mut ledger = ledger;
        let mut compatibility = AssetExecutionCompatibility::strict()
            .with_pfusdc_source_series_activation_height(Some(0));
        if governed {
            compatibility.solana_exit_heights = Some((0, u64::MAX));
            compatibility.solana_exit_open_height = Some(0);
        }
        let operation = VaultBridgeBurnToRedeemOperation {
            owner: ledger.trustlines[0].account.clone(),
            issuer: ledger.nav_assets[0].issuer.clone(),
            asset_id: ledger.nav_assets[0].asset_id.clone(),
            bucket_id: ledger.vault_bridge_bucket_states[0].bucket_id.clone(),
            amount_atoms: 300_000,
            epoch: 1,
            reserve_packet_hash: "aa".repeat(48),
            destination_ref: format!(
                "solana-spl:45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0:{}",
                "ce".repeat(32)
            ),
        };
        let transaction = SignedAssetTransaction {
            unsigned: postfiat_types::UnsignedAssetTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND.to_string(),
                signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                source: operation.owner.clone(),
                fee: 0,
                sequence: 1,
                operation: AssetTransactionOperation::VaultBridgeBurnToRedeem(operation.clone()),
            },
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: String::new(),
            signature_hex: String::new(),
        };
        let v = verdict(apply_vault_bridge_burn_to_redeem(
            &genesis,
            &mut ledger,
            &transaction,
            &operation,
            height,
            compatibility,
            &[],
        ));
        (v, digest(&ledger))
    }

    /// A signed transparent burn of the revision-8 series through the transaction entrypoint.
    fn transparent_burn(height: u64) -> (String, String) {
        use postfiat_crypto_provider::{ml_dsa_65_keygen_from_seed, ml_dsa_65_sign};
        let genesis = wan();
        let key = ml_dsa_65_keygen_from_seed(&[7u8; 32]);
        let holder = address_from_public_key(&key.public_key);
        let mut ledger = series_ledger(&genesis, &holder);
        let unsigned = postfiat_types::UnsignedAssetTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: postfiat_types::ASSET_BURN_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            source: holder.clone(),
            fee: 1_000,
            sequence: 1,
            operation: AssetTransactionOperation::AssetBurn(postfiat_types::AssetBurnOperation {
                owner: holder.clone(),
                issuer: "issuer".into(),
                asset_id: postfiat_types::solana_exit::SERIES.into(),
                amount: 1,
            }),
        };
        let signature = ml_dsa_65_sign(&key.private_key, &unsigned.signing_bytes()).unwrap();
        let transaction = SignedAssetTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&key.public_key),
            signature_hex: bytes_to_hex(&signature),
        };
        let compat = AssetExecutionCompatibility::strict();
        let receipt = crate::execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &transaction,
            height,
            compat,
        );
        (
            format!(
                "{}: {}: {}",
                receipt.accepted, receipt.code, receipt.message
            ),
            digest(&ledger),
        )
    }

    /// (case, verdict, ledger digest) below, at and above the release height.
    pub(super) fn outcomes() -> Vec<(String, String, String)> {
        assert_eq!(
            postfiat_types::solana_exit::release_height(&wan().chain_id, &genesis_hash(&wan())),
            Some(1841)
        );
        let mut out = Vec::new();
        for height in [1840u64, 1841, 1842] {
            let mut push =
                |name: &str, (v, d): (String, String)| out.push((format!("{name}@{height}"), v, d));
            push("issuer_payment", issuer_payment(height));
            push("ungoverned_exit_burn", exit_burn(height, false));
            push("governed_exit_burn", exit_burn(height, true));
            push("transparent_burn", transparent_burn(height));
        }
        out
    }
}
