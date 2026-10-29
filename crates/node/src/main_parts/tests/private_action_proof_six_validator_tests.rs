#[cfg(test)]
mod private_action_proof_six_validator_tests {
    //! S6, proof-bearing cases: a real Asset-Orchard swap proof and real
    //! private-egress proofs on the six-validator network of
    //! `private_action_six_validator_tests`, through `private_action_submit`,
    //! the validator-owned round service and `private_action_receipt`, with the
    //! pricing enforcement activated by signed governance and a governed route.
    //! Each proof is built once; refusals mutate the public bytes.
    //! Run with `cargo test --release -p postfiat-node --bin postfiat-node
    //! private_action_proof_six_validator -- --ignored --nocapture`.

    use super::*;
    use crate::private_action_six_validator_tests::{
        fleet_with, receipt_matches_independent_view, refused_code, Fleet, ATOMIC_ACTIVATION,
        VALIDATORS,
    };
    use postfiat_node::{
        asset_orchard_pricing_claim, create_asset_orchard_private_egress,
        create_asset_orchard_swap_action, private_action_receipt, AssetOrchardPairReadOptions,
        AssetOrchardPrivateEgressCreateOptions, AssetOrchardPrivateEgressFile,
        AssetOrchardSwapCreateOptions, DevKeyFile, PrivateActionState,
    };
    use postfiat_types::{ShieldedAction, ShieldedSwapActionPayload};

    const EGRESS_POLICY: &str = "wallet_private_egress_public_exit_v1";

    fn seed_hex(seed: u8) -> String {
        postfiat_crypto_provider::bytes_to_hex(&[seed; 32])
    }

    fn holder_address(fleet: &Fleet) -> String {
        let holder: DevKeyFile =
            serde_json::from_slice(&std::fs::read(&fleet.holder_key_file).unwrap()).unwrap();
        holder.address
    }

    /// Admits an action on all six validators and runs the round service
    /// until every validator records it accepted.
    fn admit_and_accept(fleet: &Fleet, action_json: &str) -> String {
        let request = fleet.request(action_json);
        for result in fleet.submit_everywhere(&request) {
            assert_eq!(result.expect("reserve").state, PrivateActionState::Queued);
        }
        assert_eq!(fleet.service_pass(), 1);
        fleet.service_pass();
        for index in 0..VALIDATORS {
            assert_eq!(
                fleet.state(index, &request.intent_id),
                Some(PrivateActionState::Accepted),
                "validator-{index}"
            );
            let receipt = private_action_receipt(&fleet.data_dirs[index], &request.intent_id)
                .unwrap()
                .contract_receipt
                .unwrap();
            assert!(receipt_matches_independent_view(
                &receipt,
                &fleet.data_dirs[(index + 1) % VALIDATORS]
            ));
        }
        request.intent_id
    }

    /// A refusal at admission on every validator with no state change.
    fn assert_refused_everywhere(fleet: &Fleet, action_json: &str) -> String {
        let before = fleet.pool_fingerprint();
        let request = fleet.request(action_json);
        let results = fleet.submit_everywhere(&request);
        let code = refused_code(&results[0]);
        for result in &results {
            assert_eq!(refused_code(result), code);
        }
        for index in 0..VALIDATORS {
            assert!(fleet.state(index, &request.intent_id).is_none());
        }
        assert_eq!(fleet.service_pass(), 0);
        assert_eq!(fleet.pool_fingerprint(), before);
        code
    }

    fn flip_hex_field(action_json: &str, inner: bool, field: &str) -> String {
        let mut value: serde_json::Value = serde_json::from_str(action_json).unwrap();
        let flip = |text: &str| {
            let mut chars = text.chars().collect::<Vec<_>>();
            let middle = chars.len() / 2;
            chars[middle] = if chars[middle] == '0' { '1' } else { '0' };
            chars.into_iter().collect::<String>()
        };
        if inner {
            let mut swap: serde_json::Value =
                serde_json::from_str(value["swap_json"].as_str().unwrap()).unwrap();
            let original = swap[field].as_str().expect("hex field").to_string();
            swap[field] = serde_json::json!(flip(&original));
            value["swap_json"] = serde_json::json!(serde_json::to_string(&swap).unwrap());
        } else {
            let original = value[field].as_str().expect("hex field").to_string();
            value[field] = serde_json::json!(flip(&original));
        }
        serde_json::to_string(&value).unwrap()
    }

    fn egress_action(fleet: &Fleet, note_file: &Path, to: &str, label: &str) -> String {
        let egress_file = fleet.root.join(format!("egress-{label}.json"));
        create_asset_orchard_private_egress(AssetOrchardPrivateEgressCreateOptions {
            data_dir: fleet.data_dirs[0].clone(),
            note_file: note_file.to_path_buf(),
            to: to.to_string(),
            asset_id: None,
            amount: None,
            fee: 0,
            policy_id: EGRESS_POLICY.to_string(),
            disclosure_hash: format!("s6-{label}"),
            pending_output_commitments: Vec::new(),
            egress_file: egress_file.clone(),
            overwrite: false,
        })
        .expect("private egress proof");
        let file: AssetOrchardPrivateEgressFile =
            serde_json::from_slice(&std::fs::read(&egress_file).unwrap()).unwrap();
        serde_json::to_string(&ShieldedAction::AssetOrchardPrivateEgressV1(file.payload)).unwrap()
    }

    #[test]
    #[ignore = "full Asset-Orchard swap and private-egress proofs on six validators are release-only benchmark-scale"]
    fn six_validators_admit_user_proven_swap_and_egress() {
        let started = Instant::now();
        let fleet = fleet_with(true);
        assert_eq!(fleet.height(), 4);
        fleet.transparent_round();
        assert_eq!(fleet.height(), ATOMIC_ACTIVATION - 1);
        let holder = holder_address(&fleet);
        let [base_asset, quote_asset] = fleet.asset_ids.clone();

        // One shield per asset creates the spendable notes.
        let ingress_a = fleet.ingress_action(0, 17, 51);
        admit_and_accept(&fleet, &ingress_a);
        let ingress_b = fleet.ingress_action(1, 34, 52);
        admit_and_accept(&fleet, &ingress_b);

        // The claim comes from the pricing read on a validator for the next height.
        let claim_report = asset_orchard_pricing_claim(AssetOrchardPairReadOptions {
            data_dir: fleet.data_dirs[2].clone(),
            base_asset_id: base_asset.clone(),
            quote_asset_id: quote_asset.clone(),
            inclusion_height: fleet.height() + 1,
        })
        .expect("pricing claim read");
        // Below height 608 the read refuses: the NAV USD-e8 denominator is not
        // active yet. The claim is then the pre-608 consensus form for the same
        // finalized NAV (epoch, packet, NAV per unit 2 -> ratio 2/1).
        let claim = match claim_report.pricing_claim {
            Some(claim) => claim,
            None => {
                assert_eq!(
                    claim_report.refusal.as_ref().map(|refusal| refusal.code.as_str()),
                    Some("asset_orchard_pricing_before_nav_usd_e8_activation")
                );
                let base_tag = postfiat_privacy_orchard::AssetTag::derive(&base_asset).unwrap();
                let quote_tag = postfiat_privacy_orchard::AssetTag::derive(&quote_asset).unwrap();
                postfiat_privacy_orchard::AssetOrchardPricingClaim {
                    nav_epoch: 7,
                    reserve_packet_hash: "ab".repeat(48),
                    ratio_numerator: 2,
                    ratio_denominator: 1,
                    mode: "at_nav_with_band".to_string(),
                    band_bps: 0,
                    base_asset_tag_lo: base_tag.lo,
                    base_asset_tag_hi: base_tag.hi,
                    quote_asset_tag_lo: quote_tag.lo,
                    quote_asset_tag_hi: quote_tag.hi,
                }
            }
        };
        let claim_file = fleet.root.join("pricing-claim.json");
        std::fs::write(&claim_file, serde_json::to_vec(&claim).unwrap()).unwrap();

        // ONE real swap proof.
        let proof_started = Instant::now();
        let swap_file = fleet.root.join("swap-action.json");
        let outputs = [fleet.root.join("swap-out-61.json"), fleet.root.join("swap-out-62.json")];
        create_asset_orchard_swap_action(AssetOrchardSwapCreateOptions {
            data_dir: fleet.data_dirs[0].clone(),
            input_note_files: [fleet.root.join("note-51.json"), fleet.root.join("note-52.json")],
            output_note_seed_hexes: [seed_hex(61), seed_hex(62)],
            pricing_claim_file: claim_file,
            action_file: swap_file.clone(),
            output_note_files: outputs.clone(),
            overwrite: false,
        })
        .expect("swap proof");
        eprintln!("s6 swap proof built in {:.1}s", proof_started.elapsed().as_secs_f64());
        let swap = serde_json::to_string(&ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
            swap_json: std::fs::read_to_string(&swap_file).unwrap(),
        }))
        .unwrap();

        // A second real egress proof over input note A, built before the swap
        // spends it: the same nullifier under a new intent.
        let proof_started = Instant::now();
        let replay = egress_action(&fleet, &fleet.root.join("note-51.json"), &holder, "replay");
        eprintln!("s6 replay egress proof built in {:.1}s", proof_started.elapsed().as_secs_f64());

        // Invalid proof: one flipped proof byte.
        let invalid_code = assert_refused_everywhere(&fleet, &flip_hex_field(&swap, true, "proof"));
        eprintln!("s6 invalid proof refused with `{invalid_code}`");
        assert_eq!(invalid_code, "asset_orchard_swap_proof_verification_failed");

        // Swap accepted: two nullifiers and two commitments on every validator.
        let before_swap = fleet.pool_fingerprint();
        admit_and_accept(&fleet, &swap);
        for (before, after) in before_swap.iter().zip(fleet.pool_fingerprint()) {
            assert_eq!(after.0, before.0 + 2);
            assert_eq!(after.1, before.1 + 2);
        }

        // Replay of a spent nullifier under a new intent.
        let replay_code = assert_refused_everywhere(&fleet, &replay);
        eprintln!("s6 replayed nullifier refused with `{replay_code}`");
        assert!(replay_code.contains("nullifier"), "{replay_code}");

        // ONE real egress proof over the swap's first output (asset B).
        let proof_started = Instant::now();
        let egress = egress_action(&fleet, &outputs[0], &holder, "exit");
        eprintln!("s6 egress proof built in {:.1}s", proof_started.elapsed().as_secs_f64());

        // Wrong recipient: the same proof with a different public destination.
        let mut wrong_recipient: serde_json::Value = serde_json::from_str(&egress).unwrap();
        wrong_recipient["to"] = serde_json::json!(format!("pf{}", "b".repeat(40)));
        let wrong_code = assert_refused_everywhere(&fleet, &wrong_recipient.to_string());
        eprintln!("s6 wrong recipient refused with `{wrong_code}`");

        // Egress accepted: one nullifier and the exact public credit.
        let before_egress = fleet.pool_fingerprint();
        admit_and_accept(&fleet, &egress);
        for (before, after) in before_egress.iter().zip(fleet.pool_fingerprint()) {
            assert_eq!(after.1, before.1 + 1);
            assert_eq!(after.2[1].1, before.2[1].1 + 34, "public credit of the exited note");
            assert_eq!(after.2[0].1, before.2[0].1);
        }

        eprintln!(
            "s6 proof-bearing module finished in {:.1}s",
            started.elapsed().as_secs_f64()
        );
        let _ = std::fs::remove_dir_all(&fleet.root);
    }

    #[test]
    #[ignore = "not built: needs a second swap proof after the NAV age bound to show asset_orchard_pricing_nav_stale"]
    fn pricing_nav_stale_not_built_needs_a_second_swap_proof() {
        panic!("not built: asset_orchard_pricing_nav_stale needs a second real swap proof submitted after the governed NAV age bound");
    }

    #[test]
    #[ignore = "not built: needs a swap proof over a claim whose pair has no governed route"]
    fn pricing_pair_not_governed_not_built_needs_a_violating_claim_proof() {
        panic!("not built: asset_orchard_pricing_pair_not_governed needs a real swap proof over a claim whose pair has no governed route; mutating the claim breaks the proof first");
    }

    #[test]
    #[ignore = "not built: needs a swap proof over an at_nav claim"]
    fn pricing_mode_mismatch_not_built_needs_a_violating_claim_proof() {
        panic!("not built: asset_orchard_pricing_mode_mismatch needs a real swap proof over an at_nav claim; mutating the mode breaks the proof first");
    }

    #[test]
    #[ignore = "not built: needs a swap proof against an anchor that was never a retained root"]
    fn stale_anchor_not_built_needs_a_proof_over_an_unretained_root() {
        panic!("not built: a stale anchor needs a real proof against a root outside the retained prefixes; mutating the anchor breaks the proof first");
    }
}
