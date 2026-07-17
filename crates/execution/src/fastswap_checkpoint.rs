use crate::fastswap_bridge::{verify_fastlane_solvency, FastLaneBridgeError};
use postfiat_crypto_provider::{hash_bytes, ml_dsa_65_verify_with_context};
use postfiat_types::{
    FastAssetIdV1, FastLaneCheckpointCertificateV1, FastLaneCheckpointIdV1, FastLaneCheckpointV1,
    FastLaneReserveBalanceV1, FastLaneStateV1, FastSwapCommitteeV1, FastSwapLocalStatusV1,
    FastSwapOpaqueHashV1, LedgerState, FASTLANE_CHECKPOINT_CONTEXT_V1,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastLaneCheckpointError {
    Bridge(FastLaneBridgeError),
    Codec,
    NotDrained,
    BelowQuorum,
    MixedCheckpoint,
    UnknownValidator,
    InvalidSignature,
    MissingCommittee,
    CheckpointChainMismatch,
    CheckpointReserveMismatch,
    CheckpointPrimaryRootMismatch,
    CheckpointConflict,
}

impl From<FastLaneBridgeError> for FastLaneCheckpointError {
    fn from(value: FastLaneBridgeError) -> Self {
        Self::Bridge(value)
    }
}

pub fn build_fastlane_checkpoint(
    state: &FastLaneStateV1,
    ledger: &LedgerState,
    previous_checkpoint_id: Option<FastLaneCheckpointIdV1>,
    highest_wal_sequence: u64,
) -> Result<FastLaneCheckpointV1, FastLaneCheckpointError> {
    verify_fastlane_solvency(ledger, state)?;
    let mut live_totals = BTreeMap::<FastAssetIdV1, u128>::new();
    let mut object_preimage = Vec::new();
    for object in state.objects.values() {
        add_total(
            &mut live_totals,
            object.asset_id,
            u128::from(object.amount_atoms),
        )?;
        append_record(
            &mut object_preimage,
            &object
                .canonical_bytes()
                .map_err(|_| FastLaneCheckpointError::Codec)?,
        )?;
    }
    let mut exit_totals = BTreeMap::<FastAssetIdV1, u128>::new();
    let mut exit_preimage = Vec::new();
    let redeemed_claims = ledger
        .redeemed_fast_lane_exit_claims
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    for claim in state.exit_claims.values() {
        if redeemed_claims.contains(&claim.exit_claim_id) {
            continue;
        }
        add_total(
            &mut exit_totals,
            claim.asset_id,
            u128::from(claim.amount_atoms),
        )?;
        append_record(
            &mut exit_preimage,
            &claim
                .canonical_bytes()
                .map_err(|_| FastLaneCheckpointError::Codec)?,
        )?;
    }
    let mut terminal_preimage = Vec::new();
    for (swap_id, tombstone) in &state.terminal_tombstones {
        terminal_preimage.extend_from_slice(&swap_id.0);
        terminal_preimage.push(match tombstone.decision {
            postfiat_types::FastSwapDecisionV1::Confirm => 1,
            postfiat_types::FastSwapDecisionV1::Cancel => 2,
        });
        terminal_preimage.extend_from_slice(&tombstone.decision_certificate_digest.0);
    }
    let mut deposit_preimage = Vec::new();
    for deposit_id in &state.imported_deposits {
        deposit_preimage.extend_from_slice(&deposit_id.0);
    }
    let mut redeemed_preimage = Vec::new();
    let mut redeemed = ledger.redeemed_fast_lane_exit_claims.clone();
    redeemed.sort();
    redeemed.dedup();
    for claim_id in redeemed {
        redeemed_preimage.extend_from_slice(&claim_id.0);
    }
    let checkpoint = FastLaneCheckpointV1 {
        previous_checkpoint_id,
        committee: state.committee.clone(),
        live_object_root: root("postfiat.fastlane.live_objects.v1", &object_preimage)?,
        live_object_totals: totals(live_totals),
        exit_claim_root: root("postfiat.fastlane.exit_claims.v1", &exit_preimage)?,
        exit_claim_totals: totals(exit_totals),
        pending_fee_burn_totals: state
            .pending_fee_burns
            .iter()
            .map(|(asset_id, amount_atoms)| FastLaneReserveBalanceV1 {
                asset_id: *asset_id,
                amount_atoms: *amount_atoms,
            })
            .collect(),
        terminal_root: root("postfiat.fastlane.terminals.v1", &terminal_preimage)?,
        highest_wal_sequence,
        active_policy_hashes: state.policy_snapshots.keys().copied().collect(),
        imported_deposit_root: root("postfiat.fastlane.deposits.v1", &deposit_preimage)?,
        redeemed_exit_claim_root: root("postfiat.fastlane.redeemed_exits.v1", &redeemed_preimage)?,
        drain_ready: fastlane_checkpoint_drain_ready(state),
        fenced_policy_epochs: state.prepare_fences.keys().copied().collect(),
    };
    checkpoint
        .canonical_bytes()
        .map_err(|_| FastLaneCheckpointError::Codec)?;
    Ok(checkpoint)
}

pub fn fastlane_checkpoint_drain_ready(state: &FastLaneStateV1) -> bool {
    state.reservations.is_empty()
        && state.swaps.values().all(|swap| {
            matches!(
                swap.status,
                FastSwapLocalStatusV1::Applied
                    | FastSwapLocalStatusV1::Cancelled
                    | FastSwapLocalStatusV1::Superseded
                    | FastSwapLocalStatusV1::Checkpointed
            )
        })
}

pub fn fastlane_checkpoint_rotation_ready(state: &FastLaneStateV1) -> bool {
    fastlane_checkpoint_drain_ready(state)
        && state.policy_snapshots.values().all(|policy| {
            state
                .prepare_fences
                .get(&policy.policy_epoch)
                .is_some_and(|fence| {
                    fence.committee_epoch == state.committee.committee_epoch
                        && fence.policy_epoch == policy.policy_epoch
                })
        })
}

pub fn verify_fastlane_checkpoint_certificate(
    committee: &FastSwapCommitteeV1,
    certificate: &FastLaneCheckpointCertificateV1,
) -> Result<FastLaneCheckpointV1, FastLaneCheckpointError> {
    committee
        .validate()
        .map_err(|_| FastLaneCheckpointError::Codec)?;
    certificate
        .validate_canonical_order()
        .map_err(|_| FastLaneCheckpointError::Codec)?;
    if certificate.votes.len() < usize::from(committee.domain.quorum) {
        return Err(FastLaneCheckpointError::BelowQuorum);
    }
    let first = &certificate.votes[0].checkpoint;
    for vote in &certificate.votes {
        if &vote.checkpoint != first || vote.checkpoint.committee != committee.domain {
            return Err(FastLaneCheckpointError::MixedCheckpoint);
        }
        let validator = committee
            .validators
            .iter()
            .find(|validator| validator.validator_id == vote.validator_id)
            .ok_or(FastLaneCheckpointError::UnknownValidator)?;
        if !ml_dsa_65_verify_with_context(
            &validator.public_key,
            &vote
                .signing_bytes()
                .map_err(|_| FastLaneCheckpointError::Codec)?,
            &vote.signature,
            FASTLANE_CHECKPOINT_CONTEXT_V1,
        ) {
            return Err(FastLaneCheckpointError::InvalidSignature);
        }
    }
    Ok(first.clone())
}

pub fn anchor_fastlane_checkpoint(
    ledger: &mut LedgerState,
    certificate: &FastLaneCheckpointCertificateV1,
) -> Result<FastLaneCheckpointV1, FastLaneCheckpointError> {
    let certificate_domain = &certificate
        .votes
        .first()
        .ok_or(FastLaneCheckpointError::BelowQuorum)?
        .checkpoint
        .committee;
    let committee = ledger
        .fastswap_committees
        .iter()
        .find(|committee| &committee.domain == certificate_domain)
        .ok_or(FastLaneCheckpointError::MissingCommittee)?;
    let checkpoint = verify_fastlane_checkpoint_certificate(committee, certificate)?;
    let checkpoint_id = checkpoint
        .checkpoint_id()
        .map_err(|_| FastLaneCheckpointError::Codec)?;
    if let Some(existing) = ledger.fast_lane_checkpoint_anchors.iter().find(|existing| {
        existing
            .votes
            .first()
            .and_then(|vote| vote.checkpoint.checkpoint_id().ok())
            == Some(checkpoint_id)
    }) {
        return if existing == certificate {
            Ok(checkpoint)
        } else {
            Err(FastLaneCheckpointError::CheckpointConflict)
        };
    }
    let expected_previous = ledger
        .fast_lane_checkpoint_anchors
        .last()
        .and_then(|last| last.votes.first())
        .map(|vote| vote.checkpoint.checkpoint_id())
        .transpose()
        .map_err(|_| FastLaneCheckpointError::Codec)?;
    if checkpoint.previous_checkpoint_id != expected_previous {
        return Err(FastLaneCheckpointError::CheckpointChainMismatch);
    }

    let mut liabilities = BTreeMap::<FastAssetIdV1, u128>::new();
    for row in checkpoint
        .live_object_totals
        .iter()
        .chain(checkpoint.exit_claim_totals.iter())
        .chain(checkpoint.pending_fee_burn_totals.iter())
    {
        add_total(&mut liabilities, row.asset_id, row.amount_atoms)?;
    }
    let reserves = ledger
        .fast_lane_reserves
        .iter()
        .map(|row| (row.asset_id, row.amount_atoms))
        .collect::<BTreeMap<_, _>>();
    if liabilities != reserves {
        return Err(FastLaneCheckpointError::CheckpointReserveMismatch);
    }

    let mut deposits = ledger
        .fast_lane_deposit_receipts
        .iter()
        .map(|receipt| receipt.deposit_id)
        .collect::<Vec<_>>();
    deposits.sort();
    deposits.dedup();
    let mut deposit_preimage = Vec::new();
    for deposit_id in deposits {
        deposit_preimage.extend_from_slice(&deposit_id.0);
    }
    let mut redeemed = ledger.redeemed_fast_lane_exit_claims.clone();
    redeemed.sort();
    redeemed.dedup();
    let mut redeemed_preimage = Vec::new();
    for claim_id in redeemed {
        redeemed_preimage.extend_from_slice(&claim_id.0);
    }
    if checkpoint.imported_deposit_root != root("postfiat.fastlane.deposits.v1", &deposit_preimage)?
        || checkpoint.redeemed_exit_claim_root
            != root("postfiat.fastlane.redeemed_exits.v1", &redeemed_preimage)?
    {
        return Err(FastLaneCheckpointError::CheckpointPrimaryRootMismatch);
    }

    // A checkpoint is the canonical burn boundary for fees already removed
    // from FastLane objects. Validate every debit against a candidate reserve
    // map first, then publish the reserve update and anchor together. This
    // keeps rejection non-mutating and makes the existing certificate-id
    // idempotency guard prevent a repeated burn.
    let mut next_reserves = reserves;
    for burn in &checkpoint.pending_fee_burn_totals {
        if burn.amount_atoms == 0 {
            return Err(FastLaneCheckpointError::CheckpointReserveMismatch);
        }
        let reserve = next_reserves
            .get_mut(&burn.asset_id)
            .ok_or(FastLaneCheckpointError::CheckpointReserveMismatch)?;
        *reserve = reserve
            .checked_sub(burn.amount_atoms)
            .ok_or(FastLaneCheckpointError::CheckpointReserveMismatch)?;
    }
    ledger.fast_lane_reserves = next_reserves
        .into_iter()
        .filter_map(|(asset_id, amount_atoms)| {
            (amount_atoms != 0).then_some(FastLaneReserveBalanceV1 {
                asset_id,
                amount_atoms,
            })
        })
        .collect();
    ledger
        .fast_lane_checkpoint_anchors
        .push(certificate.clone());
    Ok(checkpoint)
}

fn root(domain: &str, bytes: &[u8]) -> Result<FastSwapOpaqueHashV1, FastLaneCheckpointError> {
    Ok(FastSwapOpaqueHashV1(
        hash_bytes(domain, bytes)
            .try_into()
            .map_err(|_| FastLaneCheckpointError::Codec)?,
    ))
}

fn append_record(output: &mut Vec<u8>, record: &[u8]) -> Result<(), FastLaneCheckpointError> {
    let length: u32 = record
        .len()
        .try_into()
        .map_err(|_| FastLaneCheckpointError::Codec)?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(record);
    Ok(())
}

fn add_total(
    totals: &mut BTreeMap<FastAssetIdV1, u128>,
    asset: FastAssetIdV1,
    amount: u128,
) -> Result<(), FastLaneCheckpointError> {
    let value = totals.entry(asset).or_default();
    *value = value
        .checked_add(amount)
        .ok_or(FastLaneCheckpointError::Codec)?;
    Ok(())
}

fn totals(values: BTreeMap<FastAssetIdV1, u128>) -> Vec<FastLaneReserveBalanceV1> {
    values
        .into_iter()
        .map(|(asset_id, amount_atoms)| FastLaneReserveBalanceV1 {
            asset_id,
            amount_atoms,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context};
    use postfiat_types::{
        FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
        FastSwapValidatorV1, FASTSWAP_SCHEMA_VERSION_V1,
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn checkpoint_is_deterministic_and_quorum_verified() {
        let keys = (0..6)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 20; 32]))
            .collect::<Vec<_>>();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: "checkpoint-test".to_owned(),
                    genesis_hash: postfiat_types::FastSwapOpaqueHashV1([1; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 6,
                quorum: 5,
            },
            validators: keys
                .iter()
                .enumerate()
                .map(|(index, key)| FastSwapValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        committee.domain.committee_root = committee.computed_root().expect("root");
        let mut state = FastLaneStateV1 {
            schema_version: 1,
            committee: committee.domain.clone(),
            objects: BTreeMap::new(),
            reservations: BTreeMap::new(),
            swaps: BTreeMap::new(),
            imported_deposits: BTreeSet::new(),
            exit_claims: BTreeMap::new(),
            terminal_tombstones: BTreeMap::new(),
            asset_rules: BTreeMap::new(),
            holder_permits: BTreeMap::new(),
            policy_snapshots: BTreeMap::new(),
            prepare_fences: BTreeMap::new(),
            pending_fee_burns: BTreeMap::new(),
            anchored_checkpoints: BTreeSet::new(),
        };
        let fee_asset = FastAssetIdV1([91; 48]);
        let claim_asset = FastAssetIdV1([92; 48]);
        state.pending_fee_burns.insert(fee_asset, 7);
        let claim_id = postfiat_types::FastSwapExitClaimIdV1([93; 48]);
        state.exit_claims.insert(
            claim_id,
            postfiat_types::FastLaneExitClaimV1 {
                exit_claim_id: claim_id,
                committee: committee.domain.clone(),
                owner_pubkey: vec![94; 32],
                destination_address: "checkpoint-claim-destination".to_owned(),
                asset_id: claim_asset,
                asset_rule_hash: postfiat_types::FastAssetRuleHashV1([95; 48]),
                amount_atoms: 5,
            },
        );
        let mut ledger = LedgerState::empty();
        ledger.fastswap_committees.push(committee.clone());
        ledger.fast_lane_reserves.push(FastLaneReserveBalanceV1 {
            asset_id: fee_asset,
            amount_atoms: 7,
        });
        ledger.fast_lane_reserves.push(FastLaneReserveBalanceV1 {
            asset_id: claim_asset,
            amount_atoms: 5,
        });
        let checkpoint = build_fastlane_checkpoint(&state, &ledger, None, 0).expect("checkpoint");
        assert_eq!(
            checkpoint.checkpoint_id().expect("id"),
            build_fastlane_checkpoint(&state, &ledger, None, 0)
                .expect("repeat")
                .checkpoint_id()
                .expect("repeat id")
        );
        let votes = committee
            .validators
            .iter()
            .zip(keys.iter())
            .take(5)
            .map(|(validator, key)| {
                let mut vote = postfiat_types::FastLaneCheckpointVoteV1 {
                    checkpoint: checkpoint.clone(),
                    validator_id: validator.validator_id.clone(),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    &key.private_key,
                    &vote.signing_bytes().expect("bytes"),
                    FASTLANE_CHECKPOINT_CONTEXT_V1,
                )
                .expect("sign");
                vote
            })
            .collect();
        let certificate = FastLaneCheckpointCertificateV1 { votes };
        let primary = postfiat_types::FastLanePrimaryTransactionV1 {
            operation: postfiat_types::FastLanePrimaryOperationV1::AnchorCheckpoint {
                certificate: certificate.clone(),
            },
        };
        let primary_json = serde_json::to_vec(&primary).expect("primary checkpoint JSON");
        assert_eq!(
            serde_json::from_slice::<postfiat_types::FastLanePrimaryTransactionV1>(&primary_json)
                .expect("decode primary checkpoint JSON"),
            primary,
            "nonzero u128 reserve totals must survive the tagged RPC envelope"
        );
        assert_eq!(
            verify_fastlane_checkpoint_certificate(&committee, &certificate).expect("certificate"),
            checkpoint
        );
        let mut under_quorum = certificate.clone();
        under_quorum.votes.pop();
        assert_eq!(
            verify_fastlane_checkpoint_certificate(&committee, &under_quorum),
            Err(FastLaneCheckpointError::BelowQuorum)
        );
        let mut duplicate_validator = certificate.clone();
        duplicate_validator.votes[4] = duplicate_validator.votes[3].clone();
        assert_eq!(
            verify_fastlane_checkpoint_certificate(&committee, &duplicate_validator),
            Err(FastLaneCheckpointError::Codec)
        );
        let mut underfunded = ledger.clone();
        underfunded.fast_lane_reserves[0].amount_atoms = 6;
        let before_rejection = underfunded.clone();
        assert_eq!(
            anchor_fastlane_checkpoint(&mut underfunded, &certificate),
            Err(FastLaneCheckpointError::CheckpointReserveMismatch)
        );
        assert_eq!(underfunded, before_rejection, "rejection must not mutate");
        assert_eq!(
            anchor_fastlane_checkpoint(&mut ledger, &certificate).expect("anchor"),
            checkpoint
        );
        assert_eq!(ledger.fast_lane_reserves.len(), 1);
        assert_eq!(ledger.fast_lane_reserves[0].asset_id, claim_asset);
        assert_eq!(ledger.fast_lane_reserves[0].amount_atoms, 5);
        assert_eq!(
            anchor_fastlane_checkpoint(&mut ledger, &certificate).expect("idempotent anchor"),
            checkpoint
        );
        assert_eq!(ledger.fast_lane_reserves[0].amount_atoms, 5);
        assert_eq!(ledger.fast_lane_checkpoint_anchors.len(), 1);
        assert!(fastlane_checkpoint_drain_ready(&state));
        let mut next_committee = committee.clone();
        next_committee.domain.committee_epoch = 2;
        next_committee.domain.committee_root = FastSwapCommitteeRootV1::ZERO;
        next_committee.domain.committee_root = next_committee.computed_root().expect("next root");
        assert_eq!(
            crate::fastswap_control::register_fastswap_committee(
                &mut ledger,
                next_committee,
                Some(&certificate),
            ),
            Err(crate::fastswap_control::FastSwapControlError::NotDrained),
            "v1 cannot rotate an exit claim without a checkpoint membership proof"
        );
        let mut tampered = certificate;
        tampered.votes[0].checkpoint.highest_wal_sequence = 1;
        assert_eq!(
            verify_fastlane_checkpoint_certificate(&committee, &tampered),
            Err(FastLaneCheckpointError::InvalidSignature)
        );
    }
}
