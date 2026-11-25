// The FastLane fee refusal on the WAN genesis around `RELEASES` height 1841, shared with
// origin/main a720cc80f: the same file is compiled into both trees, and main's outputs are pinned.
mod release_height_fastlane_cases {
    use super::*;

    /// (height, receipt, ledger effect) for a checkpoint whose fees burn revision-8 reserves.
    /// The fixture's keys are random, so the effect is stated, not hashed.
    pub(super) fn outcomes() -> Vec<(u64, String, String)> {
        let fx = fixture("release-height-fastlane-fee");
        let mut genesis = Genesis::new_with_validator_count("postfiat-wan-devnet-2", 6);
        genesis.consensus_v2_activation_height = Some(1);
        let asset_id = postfiat_types::FastAssetIdV1(
            hex_to_bytes(postfiat_types::solana_exit::SERIES)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let mut ledger = NodeStore::new(&fx.root).read_ledger().unwrap();
        let mut committee = fx.epoch1.clone();
        committee.domain.chain = chain_domain(&genesis).unwrap();
        committee.domain.committee_root = committee.computed_root().unwrap();
        ledger.fastswap_committees = vec![committee.clone()];
        ledger
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
                asset_id,
                amount_atoms: 7,
            });
        let mut state = FastLaneStateV1::empty(committee.domain.clone());
        state.pending_fee_burns.insert(asset_id, 7);
        let checkpoint = build_fastlane_checkpoint(&state, &ledger, None, 1).unwrap();
        let tx = postfiat_types::FastLanePrimaryTransactionV1 {
            operation: postfiat_types::FastLanePrimaryOperationV1::AnchorCheckpoint {
                certificate: sign_checkpoint(&fx, &checkpoint),
            },
        };
        let before = ledger.clone();
        [1840u64, 1841, 1842]
            .into_iter()
            .map(|height| {
                let mut ledger = ledger.clone();
                let receipt = crate::execution_actions::execute_fastlane_primary_for_chain(
                    &genesis,
                    &mut ledger,
                    &tx,
                    height,
                );
                let effect = if ledger == before {
                    "ledger unchanged".to_string()
                } else {
                    format!(
                        "revision-8 reserve {:?}",
                        ledger
                            .fast_lane_reserves
                            .iter()
                            .find(|r| r.asset_id == asset_id)
                            .map(|r| r.amount_atoms)
                    )
                };
                (
                    height,
                    format!(
                        "{}: {}: {}",
                        receipt.accepted, receipt.code, receipt.message
                    ),
                    effect,
                )
            })
            .collect()
    }
}
