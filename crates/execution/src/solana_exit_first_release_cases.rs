// Burn cases shared with the first release's code (origin/main a720cc80f): the same file is
// compiled into both trees, and its outputs on main are pinned in `FIRST_RELEASE_GOLDEN`.
mod first_release_cases {
    use super::*;

    const GENESIS: &str = "45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0";
    const VAULT: &str = "e2a6fbfcf1e65a0f3e08d400c97d913e3e4eca81f84c814efa0bbcada24aac8b";
    const MINT: &str = "c6fa7af3bedbad3a3d65f36aabc97431b1bbe4c2d2f6e0e47ca60203452f5d61";
    const CHAIN: &str = "postfiat-wan-devnet-2";

    /// A custody deposit credited under `revision` on a chain named like the WAN.
    fn deposit(revision: u32) -> (Genesis, LedgerState) {
        let (genesis, mut ledger, mut route, mut journal) = setup_on(CHAIN);
        let family = postfiat_types::solana_exit::FAMILY.to_string();
        ledger.asset_definitions[0].asset_id = family.clone();
        ledger.nav_assets[0].asset_id = family.clone();
        route.profile.asset_id = family;
        route.profile.route_epoch = revision;
        route.profile.vault_address = VAULT.into();
        route.profile.token_address = MINT.into();
        let pins = route.profile.solana.as_mut().unwrap();
        pins.genesis_hash = GENESIS.into();
        pins.route_binding = postfiat_types::solana_deposit_route_binding(
            &genesis.chain_id,
            &genesis_hash(&genesis),
            &route.profile.route_id,
            GENESIS,
            VAULT,
        )
        .unwrap();
        journal[163..195].copy_from_slice(&hex_to_bytes(VAULT).unwrap());
        journal[195..227].copy_from_slice(&hex_to_bytes(&pins.route_binding).unwrap());
        ledger.solana_route = Some(route.clone());
        credit(&genesis, &mut ledger, &route, &journal).unwrap();
        let nav = &mut ledger.nav_assets[0];
        nav.finalized_epoch = 1;
        nav.finalized_reserve_packet_hash = "aa".repeat(48);
        nav.nav_per_unit = 1_000_000;
        (genesis, ledger)
    }

    /// The same deposit renamed to the revision-8 bucket, policy and series.
    pub(super) fn revision_8() -> (Genesis, LedgerState) {
        use postfiat_types::solana_exit::{BUCKET, POLICY, SERIES};
        let (genesis, ledger) = deposit(8);
        let bucket = &ledger.vault_bridge_bucket_states[0];
        let series = ledger.trustlines[0].asset_id.clone();
        let text = serde_json::to_string(&ledger)
            .unwrap()
            .replace(&bucket.bucket_id, BUCKET)
            .replace(&bucket.policy_hash, POLICY)
            .replace(&series, SERIES);
        (genesis, serde_json::from_str(&text).unwrap())
    }

    fn burn(
        genesis: &Genesis,
        ledger: &mut LedgerState,
        amount_atoms: u64,
        owner_hex: &str,
    ) -> Result<(), Rejection> {
        let mut compatibility = AssetExecutionCompatibility::strict()
            .with_pfusdc_source_series_activation_height(Some(0));
        compatibility.solana_exit_heights = Some((0, u64::MAX));
        compatibility.solana_exit_open_height = Some(0);
        let operation = VaultBridgeBurnToRedeemOperation {
            owner: ledger.trustlines[0].account.clone(),
            issuer: ledger.nav_assets[0].issuer.clone(),
            asset_id: ledger.nav_assets[0].asset_id.clone(),
            bucket_id: ledger.vault_bridge_bucket_states[0].bucket_id.clone(),
            amount_atoms,
            epoch: 1,
            reserve_packet_hash: "aa".repeat(48),
            destination_ref: format!("solana-spl:{GENESIS}:{owner_hex}"),
        };
        let transaction = SignedAssetTransaction {
            unsigned: postfiat_types::UnsignedAssetTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(genesis),
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

    /// Ed25519 wallet key (a full-order point).
    const WALLET: &str = "cececececececececececececececececececececececececececececececece";
    /// The USDC custody token account, a program-derived address off the curve.
    const OFF_CURVE: &str = "902ab403a58407aed4a100bf03f5e6e982f8a433ff7f0c98c8114e346ba96857";

    /// (case, verdict, ledger digest) at block 20 for each case.
    pub(super) fn outcomes() -> Vec<(&'static str, String, String)> {
        let mut out = Vec::new();
        let mut run = |name: &'static str,
                       (genesis, mut ledger): (Genesis, LedgerState),
                       amount: u64,
                       owner: &str| {
            let verdict = match burn(&genesis, &mut ledger, amount, owner) {
                Ok(()) => "accepted".to_string(),
                Err((code, message)) => format!("{code}: {message}"),
            };
            let digest = postfiat_types::pfusdc_ingress_proof_hash_v1(
                &serde_json::to_vec(&ledger).unwrap(),
            );
            out.push((name, verdict, digest));
        };
        run("r8_burn", revision_8(), 300_000, WALLET);
        run("r9_burn", deposit(9), 300_000, WALLET);
        run("r8_off_curve_owner", revision_8(), 300_000, OFF_CURVE);
        run("r8_below_minimum", revision_8(), 20_000, WALLET);
        run("r9_below_minimum_off_curve", deposit(9), 20_000, OFF_CURVE);
        let (genesis, mut unreconciled) = revision_8();
        unreconciled.trustlines[0].balance += 1;
        run("r8_unreconciled", (genesis, unreconciled), 300_000, WALLET);
        out
    }
}
