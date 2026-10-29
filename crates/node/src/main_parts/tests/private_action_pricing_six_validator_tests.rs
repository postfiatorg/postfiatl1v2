#[cfg(test)]
mod private_action_pricing_six_validator_tests {
    //! S6.2, the refusals that each need their own real swap proof: a claim or
    //! anchor that violates exactly one rule at the inclusion height, with the
    //! pricing enforcement and a small governed maximum NAV age activated by
    //! signed six-validator governance. Every case is refused at admission on
    //! all six validators with no state change and nothing dispatched.
    //! Run with `cargo test --release -p postfiat-node --bin postfiat-node
    //! private_action_pricing_six_validator -- --ignored --nocapture`.

    use super::*;
    use crate::private_action_proof_six_validator_tests::{
        admit_and_accept, assert_refused_everywhere, seed_hex,
    };
    use crate::private_action_six_validator_tests::{
        fleet_with_options, Fleet, FleetOptions, ATOMIC_ACTIVATION,
    };
    use postfiat_node::{create_asset_orchard_swap_action, AssetOrchardSwapCreateOptions};
    use postfiat_privacy_orchard::{AssetOrchardPricingClaim, AssetTag};
    use postfiat_types::{ShieldedAction, ShieldedSwapActionPayload};

    /// `write_pricing_state` finalizes the NAV epoch at height 1, so with this
    /// bound the epoch prices claims through height 9.
    const MAX_NAV_AGE_BLOCKS: u32 = 8;
    const NAV_LIVE_THROUGH: u64 = 1 + MAX_NAV_AGE_BLOCKS as u64;

    /// The pre-608 claim form for the governed NAV state (epoch 7, packet,
    /// NAV per unit 2 -> ratio 2/1); `asset_orchard_pricing_claim` refuses
    /// below height 608 with `asset_orchard_pricing_before_nav_usd_e8_activation`.
    fn claim(base: &str, quote: &str, mode: &str) -> AssetOrchardPricingClaim {
        let base_tag = AssetTag::derive(base).unwrap();
        let quote_tag = AssetTag::derive(quote).unwrap();
        AssetOrchardPricingClaim {
            nav_epoch: 7,
            reserve_packet_hash: "ab".repeat(48),
            ratio_numerator: 2,
            ratio_denominator: 1,
            mode: mode.to_string(),
            band_bps: 0,
            base_asset_tag_lo: base_tag.lo,
            base_asset_tag_hi: base_tag.hi,
            quote_asset_tag_lo: quote_tag.lo,
            quote_asset_tag_hi: quote_tag.hi,
        }
    }

    fn swap_proof(
        fleet: &Fleet,
        data_dir: &Path,
        inputs: [u8; 2],
        claim: &AssetOrchardPricingClaim,
        label: &str,
    ) -> String {
        let started = Instant::now();
        let claim_file = fleet.root.join(format!("claim-{label}.json"));
        std::fs::write(&claim_file, serde_json::to_vec(claim).unwrap()).unwrap();
        let action_file = fleet.root.join(format!("swap-{label}.json"));
        create_asset_orchard_swap_action(AssetOrchardSwapCreateOptions {
            data_dir: data_dir.to_path_buf(),
            input_note_files: inputs.map(|seed| fleet.root.join(format!("note-{seed}.json"))),
            output_note_seed_hexes: [seed_hex(inputs[0] + 100), seed_hex(inputs[1] + 100)],
            pricing_claim_file: claim_file,
            action_file: action_file.clone(),
            output_note_files: [
                fleet.root.join(format!("swap-{label}-out-0.json")),
                fleet.root.join(format!("swap-{label}-out-1.json")),
            ],
            overwrite: true,
        })
        .expect("swap proof");
        eprintln!("s6 {label} swap proof built in {:.1}s", started.elapsed().as_secs_f64());
        serde_json::to_string(&ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
            swap_json: std::fs::read_to_string(&action_file).unwrap(),
        }))
        .unwrap()
    }

    fn copy_dir(source: &Path, target: &Path) {
        std::fs::create_dir_all(target).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let destination = target.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &destination);
            } else {
                std::fs::copy(entry.path(), &destination).unwrap();
            }
        }
    }

    #[test]
    #[ignore = "four full Asset-Orchard swap proofs on six validators are release-only benchmark-scale"]
    fn six_validators_refuse_pricing_rule_and_stale_anchor_swaps() {
        let started = Instant::now();
        let fleet = fleet_with_options(FleetOptions {
            pricing: true,
            max_nav_age_blocks: Some(MAX_NAV_AGE_BLOCKS),
            extra_asset: true,
        });
        assert_eq!(fleet.height(), ATOMIC_ACTIVATION - 1);
        let [base, quote] = fleet.asset_ids.clone();
        let unrouted = fleet.extra_asset_id.clone().expect("third asset");

        // One shield per asset: A666 and USDC (the governed pair) and EURC,
        // which no route names.
        admit_and_accept(&fleet, &fleet.ingress_action_for(&base, 17, 51));
        admit_and_accept(&fleet, &fleet.ingress_action_for(&quote, 34, 52));
        admit_and_accept(&fleet, &fleet.ingress_action_for(&unrouted, 34, 53));
        assert_eq!(fleet.height(), 8);

        // Four real swap proofs, each violating exactly one rule.
        let mode = swap_proof(&fleet, &fleet.data_dirs[0], [51, 52], &claim(&base, &quote, "at_nav"), "mode");
        let pair = swap_proof(
            &fleet,
            &fleet.data_dirs[0],
            [51, 53],
            &claim(&base, &unrouted, "at_nav_with_band"),
            "pair",
        );
        let stale = swap_proof(
            &fleet,
            &fleet.data_dirs[0],
            [51, 52],
            &claim(&base, &quote, "at_nav_with_band"),
            "stale",
        );
        // An anchor that was never retained: the root of a tree with one more
        // commitment than the chain ever held (an ingress that was never sent).
        let fork = fleet.root.join("anchor-fork");
        copy_dir(&fleet.data_dirs[0], &fork);
        let unsent: serde_json::Value =
            serde_json::from_str(&fleet.ingress_action_for(&base, 3, 59)).unwrap();
        let fork_store = NodeStore::new(&fork);
        let mut shielded = fork_store.read_shielded().unwrap();
        shielded
            .orchard
            .as_mut()
            .expect("pool")
            .output_commitments
            .push(unsent["output_commitment"].as_str().unwrap().to_string());
        fork_store.write_shielded(&shielded).unwrap();
        let anchor = swap_proof(&fleet, &fork, [51, 52], &claim(&base, &quote, "at_nav_with_band"), "anchor");

        // Fresh NAV at the inclusion height 9: each case stops at its own rule.
        assert!(fleet.height() < NAV_LIVE_THROUGH);
        for (action, expected) in [
            (&mode, "asset_orchard_pricing_mode_mismatch"),
            (&pair, "asset_orchard_pricing_pair_not_governed"),
            (&anchor, "unretained_orchard_anchor"),
        ] {
            let code = assert_refused_everywhere(&fleet, action);
            eprintln!("s6 refused with `{code}`");
            assert_eq!(code, expected);
        }

        // Advance past the governed NAV age; the otherwise valid swap is stale.
        while fleet.height() < NAV_LIVE_THROUGH {
            fleet.transparent_round();
        }
        let code = assert_refused_everywhere(&fleet, &stale);
        eprintln!("s6 refused with `{code}` at inclusion height {}", fleet.height() + 1);
        assert_eq!(code, "asset_orchard_pricing_nav_stale");

        eprintln!(
            "s6 pricing and anchor module finished in {:.1}s",
            started.elapsed().as_secs_f64()
        );
        let _ = std::fs::remove_dir_all(&fleet.root);
    }
}
