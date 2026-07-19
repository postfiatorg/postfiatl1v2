use postfiat_crypto_provider::{hex_to_bytes, ml_dsa_65_sign_with_context};
use postfiat_execution::fastswap::{validate_fastswap_admission, PreparedFastSwapV1};
use postfiat_execution::fastswap_asset_control::validate_fast_asset_control;
use postfiat_execution::fastswap_bridge::{
    validate_fastlane_exit, validate_fastlane_exit_authorization,
};
use postfiat_execution::fastswap_checkpoint::{
    build_fastlane_checkpoint, fastlane_checkpoint_drain_ready, fastlane_checkpoint_rotation_ready,
    verify_fastlane_checkpoint_certificate,
};
use postfiat_execution::fastswap_decision::{
    objective_cancel_valid, verify_fastswap_certificate, verify_fastswap_new_round_vote,
    verify_fastswap_vote, FastSwapDecisionError, FastSwapDecisionMachineV1,
};
use postfiat_storage::{fastswap_store::FastSwapStore, NodeStore};
use postfiat_types::{
    ChainTipState, FastLaneCheckpointIdV1, FastLaneCheckpointStatusV1, FastLaneCheckpointVoteV1,
    FastLaneExitVoteV1, FastLaneStateV1, FastObjectKeyV1, FastSwapCapabilitiesV1,
    FastSwapCertificateV1, FastSwapCommitteeV1, FastSwapDecisionV1, FastSwapEffectsResponseV1,
    FastSwapIdV1, FastSwapLocalStatusV1, FastSwapNewRoundVoteV1, FastSwapObjectsResponseV1,
    FastSwapPhaseV1, FastSwapPolicyHashV1, FastSwapPolicyResponseV1, FastSwapProposalV1,
    FastSwapStatusResponseV1, FastSwapVoteEvidenceResponseV1, FastSwapVoteV1, LedgerState,
    SignedFastAssetControlCommandV1, SignedFastLaneExitIntentV1, SignedFastSwapIntentV1,
    FASTLANE_CHECKPOINT_CONTEXT_V1, FASTLANE_EXIT_VOTE_CONTEXT_V1, FASTSWAP_VOTE_CONTEXT_V1,
};
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

const FASTSWAP_DIRECTORY: &str = "fastswap-v1";
const FASTSWAP_BASE_STATE: &str = "base-state.json";
const FASTSWAP_COMMITTEE: &str = "committee.json";

pub(crate) struct FastSwapValidatorServiceV1 {
    store: FastSwapStore,
    state: FastLaneStateV1,
    committee: FastSwapCommitteeV1,
    validator_id: String,
    secret_key: Vec<u8>,
    finalized_primary_height: u64,
    activation_height: u64,
    committee_active: bool,
    canonical_tip: Option<ChainTipState>,
}

impl FastSwapValidatorServiceV1 {
    pub(crate) fn open(data_dir: &Path, validator_id: &str) -> io::Result<Self> {
        let directory = data_dir.join(FASTSWAP_DIRECTORY);
        let node_store = NodeStore::new(data_dir);
        let ledger = node_store.read_ledger()?;
        provision_epoch_one_base_if_missing(&directory, &ledger)?;
        let mut base = read_fastlane_base_state(&directory.join(FASTSWAP_BASE_STATE))?;
        let committee: FastSwapCommitteeV1 =
            read_bounded_json(&directory.join(FASTSWAP_COMMITTEE))?;
        if committee.validate().is_err() || committee.domain != base.committee {
            return Err(invalid_data("FastSwap committee/base-state mismatch"));
        }
        let secret_key = load_validator_secret_key(data_dir, validator_id)?;
        let validator = committee
            .validators
            .iter()
            .find(|validator| validator.validator_id == validator_id)
            .ok_or_else(|| invalid_data("local validator is absent from FastSwap committee"))?;
        let self_check = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &secret_key,
            b"fastswap-key-self-check-v1",
            FASTSWAP_VOTE_CONTEXT_V1,
        )
        .map_err(|error| invalid_data(&format!("FastSwap key self-check sign failed: {error}")))?;
        if !postfiat_crypto_provider::ml_dsa_65_verify_with_context(
            &validator.public_key,
            b"fastswap-key-self-check-v1",
            &self_check,
            FASTSWAP_VOTE_CONTEXT_V1,
        ) {
            return Err(invalid_data("FastSwap local key does not match committee"));
        }
        if !ledger
            .fastswap_committees
            .iter()
            .any(|registered| registered == &committee)
        {
            return Err(invalid_data(
                "FastSwap committee is not registered in canonical ledger state",
            ));
        }
        let canonical_rules = ledger
            .fast_lane_asset_rules
            .iter()
            .map(|rule| {
                rule.rule_hash()
                    .map(|hash| (hash, rule.clone()))
                    .map_err(codec_error)
            })
            .collect::<io::Result<std::collections::BTreeMap<_, _>>>()?;
        let canonical_policies = ledger
            .fastswap_policy_snapshots
            .iter()
            .map(|policy| (policy.policy_hash, policy.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let canonical_holder_permits = ledger
            .fast_lane_holder_permits
            .iter()
            .map(|permit| (permit.permit_id, permit.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        // Canonical consensus state is authoritative for mutable controls. The
        // base file seeds object state only; it cannot override a finalized
        // rule or policy registration.
        base.asset_rules = canonical_rules;
        base.holder_permits = canonical_holder_permits;
        base.policy_snapshots = canonical_policies;
        let mut store = FastSwapStore::open(&directory).map_err(store_error)?;
        let mut state = store.replay(&base).map_err(store_error)?;
        for fence in ledger
            .fast_lane_prepare_fences
            .iter()
            .filter(|fence| fence.committee_epoch == committee.domain.committee_epoch)
        {
            match state.prepare_fences.get(&fence.policy_epoch) {
                Some(existing) if existing == fence => {}
                Some(_) => return Err(invalid_data("FastSwap canonical fence conflict")),
                None => {
                    store
                        .persist_prepare_fence(
                            &mut state,
                            fence.committee_epoch,
                            fence.policy_epoch,
                            fence.finalized_primary_height,
                        )
                        .map_err(store_error)?;
                }
            }
        }
        for receipt in &ledger.fast_lane_deposit_receipts {
            if state.imported_deposits.contains(&receipt.deposit_id) {
                postfiat_execution::fastswap_bridge::import_finalized_fastlane_deposit(
                    &mut state, receipt,
                )
                .map_err(|error| {
                    invalid_data(&format!("FastLane deposit replay mismatch: {error:?}"))
                })?;
            } else {
                store
                    .import_deposit(&mut state, receipt.clone())
                    .map_err(store_error)?;
            }
        }
        let canonical_deposits = ledger
            .fast_lane_deposit_receipts
            .iter()
            .map(|receipt| receipt.deposit_id)
            .collect::<std::collections::BTreeSet<_>>();
        if !state
            .imported_deposits
            .iter()
            .all(|deposit_id| canonical_deposits.contains(deposit_id))
        {
            return Err(invalid_data(
                "FastSwap state contains a deposit absent from canonical ledger state",
            ));
        }
        let finalized_primary_height = node_store.read_chain_tip()?.height;
        let activation_height = ledger.fastswap_activation_height.unwrap_or(u64::MAX);
        let committee_active = ledger.fastswap_committees.last() == Some(&committee);
        if committee_active && committee.domain.committee_epoch > 1 {
            verify_migrated_committee_base(&state, &ledger, &committee)?;
        }
        let mut service = Self {
            store,
            state,
            committee,
            validator_id: validator_id.to_owned(),
            secret_key,
            finalized_primary_height,
            activation_height,
            committee_active,
            canonical_tip: None,
        };
        service.refresh_canonical(data_dir)?;
        Ok(service)
    }

    /// Refresh consensus-owned controls without reopening or replaying the
    /// FastLane WAL. The RPC server holds one service for its process lifetime;
    /// canonical chain state remains authoritative on every request.
    pub(crate) fn refresh_canonical(&mut self, data_dir: &Path) -> io::Result<()> {
        let configured_committee: FastSwapCommitteeV1 =
            read_bounded_json(&data_dir.join(FASTSWAP_DIRECTORY).join(FASTSWAP_COMMITTEE))?;
        if configured_committee != self.committee {
            return Err(invalid_data(
                "FastSwap committee changed while RPC service is live; restart required",
            ));
        }
        let node_store = NodeStore::new(data_dir);
        let tip_before = node_store.read_chain_tip()?;
        if self.canonical_tip.as_ref() == Some(&tip_before) {
            return Ok(());
        }
        let ledger = node_store.read_ledger()?;
        if !ledger
            .fastswap_committees
            .iter()
            .any(|registered| registered == &self.committee)
        {
            return Err(invalid_data(
                "FastSwap committee is not registered in canonical ledger state",
            ));
        }
        self.state.asset_rules = ledger
            .fast_lane_asset_rules
            .iter()
            .map(|rule| {
                rule.rule_hash()
                    .map(|hash| (hash, rule.clone()))
                    .map_err(codec_error)
            })
            .collect::<io::Result<_>>()?;
        self.state.holder_permits = ledger
            .fast_lane_holder_permits
            .iter()
            .map(|permit| (permit.permit_id, permit.clone()))
            .collect();
        self.state.policy_snapshots = ledger
            .fastswap_policy_snapshots
            .iter()
            .map(|policy| (policy.policy_hash, policy.clone()))
            .collect();
        for fence in ledger
            .fast_lane_prepare_fences
            .iter()
            .filter(|fence| fence.committee_epoch == self.committee.domain.committee_epoch)
        {
            match self.state.prepare_fences.get(&fence.policy_epoch) {
                Some(existing) if existing == fence => {}
                Some(_) => return Err(invalid_data("FastSwap canonical fence conflict")),
                None => {
                    self.store
                        .persist_prepare_fence(
                            &mut self.state,
                            fence.committee_epoch,
                            fence.policy_epoch,
                            fence.finalized_primary_height,
                        )
                        .map_err(store_error)?;
                }
            }
        }
        for receipt in &ledger.fast_lane_deposit_receipts {
            if self.state.imported_deposits.contains(&receipt.deposit_id) {
                postfiat_execution::fastswap_bridge::import_finalized_fastlane_deposit(
                    &mut self.state,
                    receipt,
                )
                .map_err(|error| {
                    invalid_data(&format!("FastLane deposit replay mismatch: {error:?}"))
                })?;
            } else {
                self.store
                    .import_deposit(&mut self.state, receipt.clone())
                    .map_err(store_error)?;
            }
        }
        let canonical_deposits = ledger
            .fast_lane_deposit_receipts
            .iter()
            .map(|receipt| receipt.deposit_id)
            .collect::<BTreeSet<_>>();
        if !self
            .state
            .imported_deposits
            .iter()
            .all(|deposit_id| canonical_deposits.contains(deposit_id))
        {
            return Err(invalid_data(
                "FastSwap state contains a deposit absent from canonical ledger state",
            ));
        }
        for certificate in &ledger.fast_lane_checkpoint_anchors {
            let Some(domain) = certificate
                .votes
                .first()
                .map(|vote| &vote.checkpoint.committee)
            else {
                return Err(invalid_data("FastLane checkpoint certificate is empty"));
            };
            if domain != &self.committee.domain {
                continue;
            }
            let checkpoint = verify_fastlane_checkpoint_certificate(&self.committee, certificate)
                .map_err(|error| {
                invalid_data(&format!(
                    "canonical FastLane checkpoint verification failed: {error:?}"
                ))
            })?;
            let checkpoint_id = checkpoint.checkpoint_id().map_err(codec_error)?;
            if !self.state.anchored_checkpoints.contains(&checkpoint_id) {
                self.store
                    .apply_anchored_checkpoint(
                        &mut self.state,
                        checkpoint_id,
                        &checkpoint.pending_fee_burn_totals,
                    )
                    .map_err(store_error)?;
                self.store
                    .compact_snapshot(&self.state)
                    .map_err(store_error)?;
            }
        }
        let tip_after = node_store.read_chain_tip()?;
        if tip_after != tip_before {
            return Err(invalid_data(
                "canonical chain tip changed during FastSwap control refresh",
            ));
        }
        self.finalized_primary_height = tip_after.height;
        self.activation_height = ledger.fastswap_activation_height.unwrap_or(u64::MAX);
        self.committee_active = ledger.fastswap_committees.last() == Some(&self.committee);
        self.canonical_tip = Some(tip_after);
        Ok(())
    }

    #[cfg(test)]
    fn from_parts(
        directory: &Path,
        base: FastLaneStateV1,
        committee: FastSwapCommitteeV1,
        validator_id: String,
        secret_key: Vec<u8>,
        finalized_primary_height: u64,
    ) -> io::Result<Self> {
        let store = FastSwapStore::open(directory).map_err(store_error)?;
        let state = store.replay(&base).map_err(store_error)?;
        Ok(Self {
            store,
            state,
            committee,
            validator_id,
            secret_key,
            finalized_primary_height,
            activation_height: 0,
            committee_active: true,
            canonical_tip: None,
        })
    }

    pub(crate) fn capabilities(&self) -> FastSwapCapabilitiesV1 {
        FastSwapCapabilitiesV1 {
            schema: "postfiat-fastswap-capabilities-v1".to_owned(),
            enabled: self.committee_active
                && self.finalized_primary_height >= self.activation_height,
            committee: self.committee.domain.clone(),
            phases: vec![
                FastSwapPhaseV1::Precommit,
                FastSwapPhaseV1::Commit,
                FastSwapPhaseV1::Effects,
                FastSwapPhaseV1::NewRound,
                FastSwapPhaseV1::CancelApply,
            ],
            terminal_receipt_code: "fastswap_applied".to_owned(),
            wire_codecs: vec![postfiat_rpc_sdk::FASTSWAP_WIRE_GZIP_BASE64_V2.to_owned()],
        }
    }

    pub(crate) fn prepare(
        &mut self,
        signed: &SignedFastSwapIntentV1,
    ) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        let swap_id = signed.swap_id().map_err(codec_error)?;
        if let Some(record) = self.state.swaps.get(&swap_id) {
            if matches!(
                record.status,
                FastSwapLocalStatusV1::Cancelled
                    | FastSwapLocalStatusV1::DecidedCancel
                    | FastSwapLocalStatusV1::Superseded
                    | FastSwapLocalStatusV1::Checkpointed
            ) || record.highest_precommit_round > 0
            {
                return Err(invalid_input(
                    "FastSwap round-zero prepare is forbidden after recovery or terminal state",
                ));
            }
        }
        let prepared = if self
            .state
            .swaps
            .get(&swap_id)
            .is_some_and(|record| record.status == FastSwapLocalStatusV1::Applied)
        {
            let effects = self
                .store
                .applied_effects(swap_id)
                .map_err(store_error)?
                .ok_or_else(|| invalid_data("applied FastSwap is missing durable effects"))?;
            PreparedFastSwapV1 {
                swap_id,
                intent_id: signed.intent.intent_id().map_err(codec_error)?,
                effects,
            }
        } else {
            validate_fastswap_admission(&self.state, signed, self.finalized_primary_height)
                .map_err(|error| invalid_input(&format!("FastSwap admission failed: {error:?}")))?
        };
        let effects_digest = prepared.effects.digest().map_err(codec_error)?;
        if let Some(existing) = self.state.swaps.get(&prepared.swap_id) {
            if existing.intent_id != prepared.intent_id || existing.effects_digest != effects_digest
            {
                return Err(invalid_data("FastSwap idempotency digest mismatch"));
            }
        } else {
            self.store
                .reserve_all(
                    &mut self.state,
                    prepared.swap_id,
                    prepared.intent_id,
                    effects_digest,
                    signed.intent.expires_at_height,
                    &prepared.effects.consumed,
                )
                .map_err(store_error)?;
        }
        self.sign_vote(FastSwapVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id: prepared.swap_id,
            phase: FastSwapPhaseV1::Precommit,
            round: 0,
            decision: Some(FastSwapDecisionV1::Confirm),
            justification_digest: None,
            effects_digest,
            receipt_digest: None,
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn asset_control_prepare(
        &mut self,
        signed: &SignedFastAssetControlCommandV1,
    ) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        let operation_id = signed.operation_id().map_err(codec_error)?;
        if self
            .state
            .terminal_tombstones
            .get(&operation_id)
            .is_some_and(|row| row.decision == FastSwapDecisionV1::Cancel)
        {
            return Err(invalid_input("asset control operation was cancelled"));
        }
        let intent_id = signed.command.intent_id().map_err(codec_error)?;
        let prepared = if self
            .state
            .swaps
            .get(&operation_id)
            .is_some_and(|record| record.status == FastSwapLocalStatusV1::Applied)
        {
            PreparedFastSwapV1 {
                swap_id: operation_id,
                intent_id,
                effects: self
                    .store
                    .applied_effects(operation_id)
                    .map_err(store_error)?
                    .ok_or_else(|| invalid_data("applied asset control is missing effects"))?,
            }
        } else {
            validate_fast_asset_control(&self.state, signed, self.finalized_primary_height)
                .map_err(|error| {
                    invalid_input(&format!(
                        "FastLane asset control admission failed: {error:?}"
                    ))
                })?
        };
        let effects_digest = prepared.effects.digest().map_err(codec_error)?;
        if let Some(existing) = self.state.swaps.get(&operation_id) {
            if existing.intent_id != intent_id || existing.effects_digest != effects_digest {
                return Err(invalid_data("asset control idempotency digest mismatch"));
            }
        } else {
            self.store
                .reserve_all(
                    &mut self.state,
                    operation_id,
                    intent_id,
                    effects_digest,
                    signed.command.expires_at_height,
                    &prepared.effects.consumed,
                )
                .map_err(store_error)?;
        }
        self.sign_vote(FastSwapVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id: operation_id,
            phase: FastSwapPhaseV1::Precommit,
            round: 0,
            decision: Some(FastSwapDecisionV1::Confirm),
            justification_digest: None,
            effects_digest,
            receipt_digest: None,
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn asset_control_preview(
        &self,
        signed: &SignedFastAssetControlCommandV1,
    ) -> io::Result<postfiat_types::FastSwapPreviewResponseV1> {
        let effects =
            validate_fast_asset_control(&self.state, signed, self.finalized_primary_height)
                .map_err(|error| {
                    invalid_input(&format!("FastLane asset control preview failed: {error:?}"))
                })?
                .effects;
        Ok(postfiat_types::FastSwapPreviewResponseV1 {
            schema: "postfiat-fastlane-asset-control-preview-v1".to_owned(),
            validator_id: self.validator_id.clone(),
            committee: self.committee.domain.clone(),
            effects,
        })
    }

    pub(crate) fn preview(
        &self,
        signed: &SignedFastSwapIntentV1,
    ) -> io::Result<postfiat_types::FastSwapPreviewResponseV1> {
        let effects =
            validate_fastswap_admission(&self.state, signed, self.finalized_primary_height)
                .map_err(|error| invalid_input(&format!("FastSwap preview failed: {error:?}")))?
                .effects;
        Ok(postfiat_types::FastSwapPreviewResponseV1 {
            schema: "postfiat-fastswap-preview-v1".to_owned(),
            validator_id: self.validator_id.clone(),
            committee: self.committee.domain.clone(),
            effects,
        })
    }

    pub(crate) fn commit(&mut self, lock_qc: &FastSwapCertificateV1) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        let verified =
            verify_fastswap_certificate(&self.committee, lock_qc).map_err(decision_error)?;
        if verified.phase != FastSwapPhaseV1::Precommit
            || verified.round != 0
            || verified.decision != Some(FastSwapDecisionV1::Confirm)
        {
            return Err(invalid_input(
                "FastSwap commit requires round-zero CONFIRM LockQC",
            ));
        }
        self.commit_round(lock_qc)
    }

    pub(crate) fn new_round_vote(
        &mut self,
        swap_id: FastSwapIdV1,
        target_round: u64,
    ) -> io::Result<FastSwapNewRoundVoteV1> {
        self.ensure_active()?;
        let record = self
            .state
            .swaps
            .get(&swap_id)
            .ok_or_else(|| invalid_input("FastSwap new-round vote before prepare"))?
            .clone();
        if record.status == FastSwapLocalStatusV1::Superseded {
            return Err(invalid_input(
                "FastSwap superseded partial state cannot enter a new round",
            ));
        }
        let terminal = self.state.terminal_tombstones.get(&swap_id).cloned();
        self.store
            .persist_new_round_vote(&mut self.state, swap_id, target_round)
            .map_err(store_error)?;
        self.sign_new_round_vote(FastSwapNewRoundVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id,
            target_round,
            highest_voted_round: record.highest_precommit_round,
            locked_round: record.decision_lock_round,
            locked_value: record.decision_lock_value,
            locked_certificate_digest: record.lock_certificate_digest,
            terminal_decision: terminal.as_ref().map(|row| row.decision),
            terminal_certificate_digest: terminal
                .as_ref()
                .map(|row| row.decision_certificate_digest),
            effects_digest: record.effects_digest,
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn precommit_round(
        &mut self,
        proposal: &FastSwapProposalV1,
    ) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        if proposal.round == 0 {
            return Err(invalid_input(
                "round-zero precommit is emitted only by fastswap_prepare",
            ));
        }
        self.validate_round_proposal(proposal)?;
        self.store
            .persist_precommit_vote(
                &mut self.state,
                proposal.swap_id,
                proposal.round,
                proposal.decision,
            )
            .map_err(store_error)?;
        self.sign_vote(FastSwapVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id: proposal.swap_id,
            phase: FastSwapPhaseV1::Precommit,
            round: proposal.round,
            decision: Some(proposal.decision),
            justification_digest: proposal
                .new_round_qc
                .as_ref()
                .map(|certificate| certificate.digest())
                .transpose()
                .map_err(codec_error)?,
            effects_digest: proposal.effects_digest,
            receipt_digest: None,
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn validate_round_proposal(&self, proposal: &FastSwapProposalV1) -> io::Result<()> {
        let record = self
            .state
            .swaps
            .get(&proposal.swap_id)
            .ok_or_else(|| invalid_input("FastSwap recovery proposal before prepare"))?;
        if record.status == FastSwapLocalStatusV1::Superseded {
            return Err(invalid_input(
                "FastSwap superseded partial state cannot be proposed",
            ));
        }
        let mut machine = FastSwapDecisionMachineV1::new(proposal.swap_id, record.effects_digest);
        machine.highest_voted_round = Some(record.highest_precommit_round);
        machine.lock_round = record.decision_lock_round;
        machine.lock_value = record.decision_lock_value;
        machine.terminal = self
            .state
            .terminal_tombstones
            .get(&proposal.swap_id)
            .map(|tombstone| tombstone.decision);
        machine
            .validate_proposal(
                &self.committee,
                proposal,
                objective_cancel_valid(
                    record.expires_at_height,
                    self.finalized_primary_height,
                    false,
                ),
            )
            .map_err(decision_error)
    }

    pub(crate) fn commit_round(
        &mut self,
        precommit_qc: &FastSwapCertificateV1,
    ) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        let verified =
            verify_fastswap_certificate(&self.committee, precommit_qc).map_err(decision_error)?;
        self.store
            .persist_certificate_artifact(precommit_qc)
            .map_err(store_error)?;
        if verified.phase != FastSwapPhaseV1::Precommit || verified.decision.is_none() {
            return Err(invalid_input("FastSwap commit-round requires PrecommitQC"));
        }
        let record = self.state.swaps.get(&verified.swap_id).ok_or_else(|| {
            invalid_input("FastSwap PrecommitQC has no complete local reservation")
        })?;
        if record.effects_digest != verified.effects_digest
            || verified.round < record.highest_precommit_round
        {
            return Err(invalid_data(
                "FastSwap PrecommitQC is stale or mismatches local reservation",
            ));
        }
        let decision = verified.decision.expect("checked Some above");
        let lock_digest = precommit_qc.digest().map_err(codec_error)?;
        let already_locked = record.decision_lock_round == Some(verified.round)
            && record.decision_lock_value == Some(decision)
            && record.lock_certificate_digest == Some(lock_digest);
        if !already_locked {
            self.store
                .persist_decision_lock(
                    &mut self.state,
                    verified.swap_id,
                    verified.round,
                    decision,
                    lock_digest,
                )
                .map_err(store_error)?;
        }
        self.sign_vote(FastSwapVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id: verified.swap_id,
            phase: FastSwapPhaseV1::Commit,
            round: verified.round,
            decision: Some(decision),
            justification_digest: Some(lock_digest),
            effects_digest: verified.effects_digest,
            receipt_digest: None,
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn apply(
        &mut self,
        decision_qc: &FastSwapCertificateV1,
        signed: &SignedFastSwapIntentV1,
    ) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        let verified =
            verify_fastswap_certificate(&self.committee, decision_qc).map_err(decision_error)?;
        self.store
            .persist_certificate_artifact(decision_qc)
            .map_err(store_error)?;
        if verified.phase != FastSwapPhaseV1::Commit
            || verified.decision != Some(FastSwapDecisionV1::Confirm)
        {
            return Err(invalid_input(
                "FastSwap apply requires a CONFIRM DecisionQC",
            ));
        }
        if signed.swap_id().map_err(codec_error)? != verified.swap_id {
            return Err(invalid_input(
                "FastSwap DecisionQC does not bind supplied intent",
            ));
        }
        let local_lock_digest = self
            .state
            .swaps
            .get(&verified.swap_id)
            .and_then(|record| {
                (record.decision_lock_value == Some(FastSwapDecisionV1::Confirm))
                    .then_some(record.lock_certificate_digest)
                    .flatten()
            })
            .ok_or_else(|| invalid_input("FastSwap apply has no local CONFIRM decision lock"))?;
        if decision_qc
            .votes
            .iter()
            .any(|vote| vote.justification_digest != Some(local_lock_digest))
        {
            return Err(invalid_input(
                "FastSwap DecisionQC does not justify the locally verified LockQC",
            ));
        }
        let already_applied = self
            .state
            .swaps
            .get(&verified.swap_id)
            .is_some_and(|record| record.status == FastSwapLocalStatusV1::Applied);
        let effects = if already_applied {
            self.store
                .applied_effects(verified.swap_id)
                .map_err(store_error)?
                .ok_or_else(|| invalid_data("applied FastSwap is missing durable effects"))?
        } else {
            validate_fastswap_admission(&self.state, signed, self.finalized_primary_height)
                .map_err(|error| {
                    invalid_input(&format!("FastSwap apply admission failed: {error:?}"))
                })?
                .effects
        };
        if effects.digest().map_err(codec_error)? != verified.effects_digest {
            return Err(invalid_input("FastSwap DecisionQC effects digest mismatch"));
        }
        let decision_digest = decision_qc.digest().map_err(codec_error)?;
        if !already_applied {
            self.store
                .apply_confirm(&mut self.state, effects.clone(), decision_digest)
                .map_err(store_error)?;
        }
        let receipt_digest = effects.receipt.digest().map_err(codec_error)?;
        self.sign_vote(FastSwapVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id: verified.swap_id,
            phase: FastSwapPhaseV1::Effects,
            round: verified.round,
            decision: Some(FastSwapDecisionV1::Confirm),
            justification_digest: Some(decision_digest),
            effects_digest: verified.effects_digest,
            receipt_digest: Some(receipt_digest),
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn asset_control_apply(
        &mut self,
        decision_qc: &FastSwapCertificateV1,
        signed: &SignedFastAssetControlCommandV1,
    ) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        let verified =
            verify_fastswap_certificate(&self.committee, decision_qc).map_err(decision_error)?;
        self.store
            .persist_certificate_artifact(decision_qc)
            .map_err(store_error)?;
        let operation_id = signed.operation_id().map_err(codec_error)?;
        if verified.phase != FastSwapPhaseV1::Commit
            || verified.decision != Some(FastSwapDecisionV1::Confirm)
            || verified.swap_id != operation_id
        {
            return Err(invalid_input(
                "asset control apply requires its CONFIRM DecisionQC",
            ));
        }
        let local_lock_digest = self
            .state
            .swaps
            .get(&operation_id)
            .and_then(|record| {
                (record.decision_lock_value == Some(FastSwapDecisionV1::Confirm))
                    .then_some(record.lock_certificate_digest)
                    .flatten()
            })
            .ok_or_else(|| invalid_input("asset control apply has no local CONFIRM lock"))?;
        if decision_qc
            .votes
            .iter()
            .any(|vote| vote.justification_digest != Some(local_lock_digest))
        {
            return Err(invalid_input(
                "asset control DecisionQC does not justify the local LockQC",
            ));
        }
        let already_applied = self
            .state
            .swaps
            .get(&operation_id)
            .is_some_and(|record| record.status == FastSwapLocalStatusV1::Applied);
        let effects = if already_applied {
            self.store
                .applied_effects(operation_id)
                .map_err(store_error)?
                .ok_or_else(|| invalid_data("applied asset control is missing effects"))?
        } else {
            validate_fast_asset_control(&self.state, signed, self.finalized_primary_height)
                .map_err(|error| {
                    invalid_input(&format!("FastLane asset control apply failed: {error:?}"))
                })?
                .effects
        };
        if effects.digest().map_err(codec_error)? != verified.effects_digest
            || effects.receipt.code != "fastlane_asset_control_applied"
        {
            return Err(invalid_input("asset control effects mismatch"));
        }
        let decision_digest = decision_qc.digest().map_err(codec_error)?;
        if !already_applied {
            self.store
                .apply_confirm(&mut self.state, effects.clone(), decision_digest)
                .map_err(store_error)?;
        }
        self.sign_vote(FastSwapVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id: operation_id,
            phase: FastSwapPhaseV1::Effects,
            round: verified.round,
            decision: Some(FastSwapDecisionV1::Confirm),
            justification_digest: Some(decision_digest),
            effects_digest: verified.effects_digest,
            receipt_digest: Some(effects.receipt.digest().map_err(codec_error)?),
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn asset_control_catch_up(
        &mut self,
        lock_qc: &FastSwapCertificateV1,
        decision_qc: &FastSwapCertificateV1,
        signed: &SignedFastAssetControlCommandV1,
    ) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        let lock = verify_fastswap_certificate(&self.committee, lock_qc).map_err(decision_error)?;
        let decision =
            verify_fastswap_certificate(&self.committee, decision_qc).map_err(decision_error)?;
        self.store
            .persist_certificate_artifact(lock_qc)
            .and_then(|_| self.store.persist_certificate_artifact(decision_qc))
            .map_err(store_error)?;
        let lock_digest = lock_qc.digest().map_err(codec_error)?;
        let decision_digest = decision_qc.digest().map_err(codec_error)?;
        let operation_id = signed.operation_id().map_err(codec_error)?;
        if lock.phase != FastSwapPhaseV1::Precommit
            || lock.decision != Some(FastSwapDecisionV1::Confirm)
            || decision.phase != FastSwapPhaseV1::Commit
            || decision.decision != Some(FastSwapDecisionV1::Confirm)
            || lock.swap_id != operation_id
            || decision.swap_id != operation_id
            || lock.round != decision.round
            || lock.effects_digest != decision.effects_digest
            || decision_qc
                .votes
                .iter()
                .any(|vote| vote.justification_digest != Some(lock_digest))
        {
            return Err(invalid_input(
                "asset control catch-up requires one complete CONFIRM certificate chain",
            ));
        }
        if self
            .state
            .swaps
            .get(&operation_id)
            .is_some_and(|record| record.status == FastSwapLocalStatusV1::Applied)
        {
            return self.asset_control_apply(decision_qc, signed);
        }

        if let Some(losing_operation_id) = self
            .state
            .reservations
            .get(&signed.command.input)
            .map(|reservation| reservation.swap_id)
            .filter(|reserved| *reserved != operation_id)
        {
            self.store
                .supersede_partial(
                    &mut self.state,
                    losing_operation_id,
                    operation_id,
                    decision_digest,
                )
                .map_err(store_error)?;
        }
        let historical_height = signed.command.expires_at_height;
        let prepared = validate_fast_asset_control(&self.state, signed, historical_height)
            .map_err(|error| {
                invalid_data(&format!(
                    "certified asset control cannot reconcile local input: {error:?}"
                ))
            })?;
        let effects_digest = prepared.effects.digest().map_err(codec_error)?;
        if prepared.swap_id != operation_id || effects_digest != decision.effects_digest {
            return Err(invalid_data(
                "certified asset control catch-up recomputed different effects",
            ));
        }
        if !self.state.swaps.contains_key(&operation_id) {
            self.store
                .reserve_all(
                    &mut self.state,
                    operation_id,
                    prepared.intent_id,
                    effects_digest,
                    signed.command.expires_at_height,
                    &prepared.effects.consumed,
                )
                .map_err(store_error)?;
        }
        let locally_locked = self.state.swaps.get(&operation_id).is_some_and(|record| {
            record.decision_lock_round == Some(lock.round)
                && record.decision_lock_value == Some(FastSwapDecisionV1::Confirm)
                && record.lock_certificate_digest == Some(lock_digest)
        });
        if !locally_locked {
            self.store
                .persist_decision_lock(
                    &mut self.state,
                    operation_id,
                    lock.round,
                    FastSwapDecisionV1::Confirm,
                    lock_digest,
                )
                .map_err(store_error)?;
        }
        self.store
            .apply_confirm(&mut self.state, prepared.effects.clone(), decision_digest)
            .map_err(store_error)?;
        self.sign_vote(FastSwapVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id: operation_id,
            phase: FastSwapPhaseV1::Effects,
            round: decision.round,
            decision: Some(FastSwapDecisionV1::Confirm),
            justification_digest: Some(decision_digest),
            effects_digest,
            receipt_digest: Some(prepared.effects.receipt.digest().map_err(codec_error)?),
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn catch_up_confirm(
        &mut self,
        lock_qc: &FastSwapCertificateV1,
        decision_qc: &FastSwapCertificateV1,
        signed: &SignedFastSwapIntentV1,
    ) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        let lock = verify_fastswap_certificate(&self.committee, lock_qc).map_err(decision_error)?;
        let decision =
            verify_fastswap_certificate(&self.committee, decision_qc).map_err(decision_error)?;
        self.store
            .persist_certificate_artifact(lock_qc)
            .and_then(|_| self.store.persist_certificate_artifact(decision_qc))
            .map_err(store_error)?;
        let lock_digest = lock_qc.digest().map_err(codec_error)?;
        let decision_digest = decision_qc.digest().map_err(codec_error)?;
        let swap_id = signed.swap_id().map_err(codec_error)?;
        if lock.phase != FastSwapPhaseV1::Precommit
            || lock.decision != Some(FastSwapDecisionV1::Confirm)
            || decision.phase != FastSwapPhaseV1::Commit
            || decision.decision != Some(FastSwapDecisionV1::Confirm)
            || lock.swap_id != swap_id
            || decision.swap_id != swap_id
            || lock.round != decision.round
            || lock.effects_digest != decision.effects_digest
            || decision_qc
                .votes
                .iter()
                .any(|vote| vote.justification_digest != Some(lock_digest))
        {
            return Err(invalid_input(
                "FastSwap catch-up requires one complete CONFIRM certificate chain",
            ));
        }
        if self
            .state
            .swaps
            .get(&swap_id)
            .is_some_and(|record| record.status == FastSwapLocalStatusV1::Applied)
        {
            return self.apply(decision_qc, signed);
        }

        let historical_height = signed.intent.expires_at_height;
        let preliminary = validate_fastswap_admission(&self.state, signed, historical_height);
        if preliminary.is_err() {
            let consumed = signed
                .intent
                .party_0
                .asset_inputs
                .iter()
                .chain(&signed.intent.party_0.fee_inputs)
                .chain(&signed.intent.party_1.asset_inputs)
                .chain(&signed.intent.party_1.fee_inputs)
                .copied()
                .collect::<BTreeSet<_>>();
            let losing = consumed
                .iter()
                .filter_map(|key| self.state.reservations.get(key))
                .map(|reservation| reservation.swap_id)
                .filter(|reserved_swap_id| *reserved_swap_id != swap_id)
                .collect::<BTreeSet<_>>();
            for losing_swap_id in losing {
                self.store
                    .supersede_partial(&mut self.state, losing_swap_id, swap_id, decision_digest)
                    .map_err(store_error)?;
            }
        }
        let prepared = validate_fastswap_admission(&self.state, signed, historical_height)
            .map_err(|error| {
                invalid_data(&format!(
                    "certified FastSwap cannot reconcile local inputs: {error:?}"
                ))
            })?;
        let effects_digest = prepared.effects.digest().map_err(codec_error)?;
        if prepared.swap_id != swap_id || effects_digest != decision.effects_digest {
            return Err(invalid_data(
                "certified FastSwap catch-up recomputed different effects",
            ));
        }
        if !self.state.swaps.contains_key(&swap_id) {
            self.store
                .reserve_all(
                    &mut self.state,
                    prepared.swap_id,
                    prepared.intent_id,
                    effects_digest,
                    signed.intent.expires_at_height,
                    &prepared.effects.consumed,
                )
                .map_err(store_error)?;
        }
        let locally_locked = self.state.swaps.get(&swap_id).is_some_and(|record| {
            record.decision_lock_round == Some(lock.round)
                && record.decision_lock_value == Some(FastSwapDecisionV1::Confirm)
                && record.lock_certificate_digest == Some(lock_digest)
        });
        if !locally_locked {
            self.store
                .persist_decision_lock(
                    &mut self.state,
                    swap_id,
                    lock.round,
                    FastSwapDecisionV1::Confirm,
                    lock_digest,
                )
                .map_err(store_error)?;
        }
        self.store
            .apply_confirm(&mut self.state, prepared.effects.clone(), decision_digest)
            .map_err(store_error)?;
        let receipt_digest = prepared.effects.receipt.digest().map_err(codec_error)?;
        self.sign_vote(FastSwapVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id,
            phase: FastSwapPhaseV1::Effects,
            round: decision.round,
            decision: Some(FastSwapDecisionV1::Confirm),
            justification_digest: Some(decision_digest),
            effects_digest,
            receipt_digest: Some(receipt_digest),
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn cancel_apply(
        &mut self,
        decision_qc: &FastSwapCertificateV1,
    ) -> io::Result<FastSwapVoteV1> {
        self.ensure_active()?;
        let verified =
            verify_fastswap_certificate(&self.committee, decision_qc).map_err(decision_error)?;
        self.store
            .persist_certificate_artifact(decision_qc)
            .map_err(store_error)?;
        if verified.phase != FastSwapPhaseV1::Commit
            || verified.decision != Some(FastSwapDecisionV1::Cancel)
        {
            return Err(invalid_input(
                "FastSwap cancel-apply requires CANCEL DecisionQC",
            ));
        }
        let record = self
            .state
            .swaps
            .get(&verified.swap_id)
            .ok_or_else(|| invalid_input("FastSwap cancel before prepare"))?;
        if record.effects_digest != verified.effects_digest
            || record.decision_lock_round != Some(verified.round)
            || record.decision_lock_value != Some(FastSwapDecisionV1::Cancel)
        {
            return Err(invalid_input(
                "FastSwap cancel DecisionQC mismatches local decision lock",
            ));
        }
        let lock_digest = record
            .lock_certificate_digest
            .ok_or_else(|| invalid_data("FastSwap cancel lock certificate is missing"))?;
        if decision_qc
            .votes
            .iter()
            .any(|vote| vote.justification_digest != Some(lock_digest))
        {
            return Err(invalid_input(
                "FastSwap cancel DecisionQC does not justify local PrecommitQC",
            ));
        }
        let decision_digest = decision_qc.digest().map_err(codec_error)?;
        let already_cancelled = record.status == FastSwapLocalStatusV1::Cancelled;
        if !already_cancelled {
            self.store
                .apply_cancel(&mut self.state, verified.swap_id, decision_digest)
                .map_err(store_error)?;
        }
        self.sign_vote(FastSwapVoteV1 {
            domain: self.committee.domain.clone(),
            swap_id: verified.swap_id,
            phase: FastSwapPhaseV1::CancelApply,
            round: verified.round,
            decision: Some(FastSwapDecisionV1::Cancel),
            justification_digest: Some(decision_digest),
            effects_digest: verified.effects_digest,
            receipt_digest: None,
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn status(&self, swap_id: FastSwapIdV1) -> FastSwapStatusResponseV1 {
        FastSwapStatusResponseV1 {
            schema: "postfiat-fastswap-status-v1".to_owned(),
            swap_id,
            record: self.state.swaps.get(&swap_id).cloned(),
            terminal_tombstone: self.state.terminal_tombstones.get(&swap_id).cloned(),
        }
    }

    pub(crate) fn effects(&self, swap_id: FastSwapIdV1) -> io::Result<FastSwapEffectsResponseV1> {
        Ok(FastSwapEffectsResponseV1 {
            schema: "postfiat-fastswap-effects-v1".to_owned(),
            swap_id,
            effects: self.store.applied_effects(swap_id).map_err(store_error)?,
        })
    }

    pub(crate) fn vote_evidence(
        &self,
        swap_id: FastSwapIdV1,
        phase: FastSwapPhaseV1,
        round: u64,
    ) -> io::Result<FastSwapVoteEvidenceResponseV1> {
        let certificate_digest = self
            .state
            .swaps
            .get(&swap_id)
            .and_then(|record| match phase {
                FastSwapPhaseV1::Precommit if record.decision_lock_round == Some(round) => {
                    record.lock_certificate_digest
                }
                FastSwapPhaseV1::Commit if record.decision_lock_round == Some(round) => {
                    record.decision_certificate_digest
                }
                _ => None,
            });
        let certificate = certificate_digest
            .map(|digest| self.store.certificate_artifact(digest).map_err(store_error))
            .transpose()?
            .flatten();
        let (vote, new_round_vote) = if phase == FastSwapPhaseV1::NewRound {
            let vote = self
                .store
                .new_round_vote_artifact(swap_id, round, &self.validator_id)
                .map_err(store_error)?;
            if let Some(vote) = vote.as_ref() {
                verify_fastswap_new_round_vote(&self.committee, vote).map_err(decision_error)?;
                if vote.swap_id != swap_id
                    || vote.target_round != round
                    || vote.validator_id != self.validator_id
                {
                    return Err(invalid_data("FastSwap new-round artifact mismatch"));
                }
            }
            (None, vote)
        } else {
            let vote = self
                .store
                .vote_artifact(swap_id, phase, round, &self.validator_id)
                .map_err(store_error)?;
            if let Some(vote) = vote.as_ref() {
                verify_fastswap_vote(&self.committee, vote).map_err(decision_error)?;
                if vote.swap_id != swap_id
                    || vote.phase != phase
                    || vote.round != round
                    || vote.validator_id != self.validator_id
                {
                    return Err(invalid_data("FastSwap vote artifact mismatch"));
                }
            }
            (vote, None)
        };
        Ok(FastSwapVoteEvidenceResponseV1 {
            schema: "postfiat-fastswap-vote-evidence-v1".to_owned(),
            validator_id: self.validator_id.clone(),
            swap_id,
            phase,
            round,
            vote,
            new_round_vote,
            certificate,
        })
    }

    pub(crate) fn objects(
        &self,
        owner_pubkey: &[u8],
        asset_id: Option<postfiat_types::FastAssetIdV1>,
        cursor: Option<FastObjectKeyV1>,
        limit: usize,
    ) -> io::Result<FastSwapObjectsResponseV1> {
        if limit == 0 || limit > 100 {
            return Err(invalid_input("FastSwap object limit must be in 1..=100"));
        }
        let mut objects = self
            .state
            .objects
            .iter()
            .filter(|(key, object)| {
                cursor.is_none_or(|cursor| **key > cursor)
                    && object.owner_pubkey == owner_pubkey
                    && asset_id.is_none_or(|asset| object.asset_id == asset)
            })
            .map(|(_, object)| object.clone())
            .take(limit.saturating_add(1))
            .collect::<Vec<_>>();
        let has_more = objects.len() > limit;
        if has_more {
            objects.pop();
        }
        let next_cursor = has_more.then(|| objects.last().expect("nonzero limit").key);
        Ok(FastSwapObjectsResponseV1 {
            schema: "postfiat-fastswap-objects-v1".to_owned(),
            validator_id: self.validator_id.clone(),
            committee: self.committee.domain.clone(),
            objects,
            next_cursor,
        })
    }

    pub(crate) fn policy(
        &self,
        policy_hash: Option<FastSwapPolicyHashV1>,
        pair: Option<(postfiat_types::FastAssetIdV1, postfiat_types::FastAssetIdV1)>,
    ) -> io::Result<FastSwapPolicyResponseV1> {
        if policy_hash.is_some() == pair.is_some() {
            return Err(invalid_input(
                "FastSwap policy query requires exactly one of policy_hash or active pair",
            ));
        }
        let policy = if let Some(hash) = policy_hash {
            self.state.policy_snapshots.get(&hash).cloned()
        } else {
            let (asset_0, asset_1) = pair.expect("checked Some");
            self.state
                .policy_snapshots
                .values()
                .filter(|policy| {
                    !policy.paused
                        && policy.valid_from_height <= self.finalized_primary_height
                        && policy.valid_through_height >= self.finalized_primary_height
                        && ((policy.pair_asset_0 == asset_0 && policy.pair_asset_1 == asset_1)
                            || (policy.pair_asset_0 == asset_1 && policy.pair_asset_1 == asset_0))
                })
                .max_by_key(|policy| policy.policy_epoch)
                .cloned()
        };
        Ok(FastSwapPolicyResponseV1 {
            schema: "postfiat-fastswap-policy-v1".to_owned(),
            validator_id: self.validator_id.clone(),
            policy,
        })
    }

    pub(crate) fn exit(
        &mut self,
        signed: &SignedFastLaneExitIntentV1,
    ) -> io::Result<FastLaneExitVoteV1> {
        self.ensure_active()?;
        let exit_id = signed.intent.exit_id().map_err(codec_error)?;
        let effects = if let Some(existing) = self
            .store
            .applied_exit_effects(exit_id)
            .map_err(store_error)?
        {
            validate_fastlane_exit_authorization(&self.state, signed).map_err(|error| {
                invalid_input(&format!("FastLane exit authorization failed: {error:?}"))
            })?;
            if existing.consumed != signed.intent.inputs
                || existing.claim.committee != signed.intent.committee
                || existing.claim.owner_pubkey != signed.intent.owner_pubkey
                || existing.claim.destination_address != signed.intent.destination_address
                || existing.claim.asset_id != signed.intent.asset_id
                || existing.claim.asset_rule_hash != signed.intent.asset_rule_hash
                || existing.claim.amount_atoms != signed.intent.amount_atoms
            {
                return Err(invalid_data("FastLane exit idempotency digest mismatch"));
            }
            existing
        } else {
            let effects = validate_fastlane_exit(&self.state, signed).map_err(|error| {
                invalid_input(&format!("FastLane exit admission failed: {error:?}"))
            })?;
            self.store
                .apply_exit(&mut self.state, effects.clone())
                .map_err(store_error)?;
            effects
        };
        self.sign_exit_vote(FastLaneExitVoteV1 {
            committee: self.committee.domain.clone(),
            exit_id,
            effects_digest: effects.digest().map_err(codec_error)?,
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        })
    }

    pub(crate) fn checkpoint_status(
        &self,
        ledger: &LedgerState,
        previous_checkpoint_id: Option<FastLaneCheckpointIdV1>,
    ) -> io::Result<FastLaneCheckpointStatusV1> {
        let checkpoint = build_fastlane_checkpoint(
            &self.state,
            ledger,
            previous_checkpoint_id,
            self.store.highest_wal_sequence(),
        )
        .map_err(|error| invalid_data(&format!("FastLane checkpoint failed: {error:?}")))?;
        let mut vote = FastLaneCheckpointVoteV1 {
            checkpoint: checkpoint.clone(),
            validator_id: self.validator_id.clone(),
            signature: Vec::new(),
        };
        vote.signature = ml_dsa_65_sign_with_context(
            &self.secret_key,
            &vote.signing_bytes().map_err(codec_error)?,
            FASTLANE_CHECKPOINT_CONTEXT_V1,
        )
        .map_err(|error| invalid_data(&format!("FastLane checkpoint signing failed: {error}")))?;
        Ok(FastLaneCheckpointStatusV1 {
            schema: "postfiat-fastlane-checkpoint-status-v1".to_owned(),
            checkpoint,
            vote,
            drain_ready: fastlane_checkpoint_drain_ready(&self.state),
            rotation_ready: fastlane_checkpoint_rotation_ready(&self.state),
        })
    }

    fn ensure_active(&self) -> io::Result<()> {
        if !self.committee_active {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "FastSwap committee is not the active canonical committee",
            ));
        }
        if self.finalized_primary_height < self.activation_height {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "FastSwap is not active until canonical height {}",
                    self.activation_height
                ),
            ));
        }
        Ok(())
    }

    fn sign_vote(&self, mut vote: FastSwapVoteV1) -> io::Result<FastSwapVoteV1> {
        vote.signature = ml_dsa_65_sign_with_context(
            &self.secret_key,
            &vote.signing_bytes().map_err(codec_error)?,
            FASTSWAP_VOTE_CONTEXT_V1,
        )
        .map_err(|error| invalid_data(&format!("FastSwap vote signing failed: {error}")))?;
        self.store
            .persist_vote_artifact(&vote)
            .map_err(store_error)?;
        Ok(vote)
    }

    fn sign_new_round_vote(
        &self,
        mut vote: FastSwapNewRoundVoteV1,
    ) -> io::Result<FastSwapNewRoundVoteV1> {
        vote.signature = ml_dsa_65_sign_with_context(
            &self.secret_key,
            &vote.signing_bytes().map_err(codec_error)?,
            FASTSWAP_VOTE_CONTEXT_V1,
        )
        .map_err(|error| invalid_data(&format!("FastSwap new-round signing failed: {error}")))?;
        self.store
            .persist_new_round_vote_artifact(&vote)
            .map_err(store_error)?;
        Ok(vote)
    }

    fn sign_exit_vote(&self, mut vote: FastLaneExitVoteV1) -> io::Result<FastLaneExitVoteV1> {
        vote.signature = ml_dsa_65_sign_with_context(
            &self.secret_key,
            &vote.signing_bytes().map_err(codec_error)?,
            FASTLANE_EXIT_VOTE_CONTEXT_V1,
        )
        .map_err(|error| invalid_data(&format!("FastLane exit vote signing failed: {error}")))?;
        Ok(vote)
    }
}

fn verify_migrated_committee_base(
    state: &FastLaneStateV1,
    ledger: &LedgerState,
    committee: &FastSwapCommitteeV1,
) -> io::Result<()> {
    let previous_epoch = committee.domain.committee_epoch.saturating_sub(1);
    let previous_committee = ledger
        .fastswap_committees
        .iter()
        .find(|candidate| candidate.domain.committee_epoch == previous_epoch)
        .ok_or_else(|| invalid_data("FastSwap migration previous committee is absent"))?;
    let (previous_checkpoint_id, previous_checkpoint) = ledger
        .fast_lane_checkpoint_anchors
        .iter()
        .rev()
        .filter_map(|certificate| {
            let domain = certificate.votes.first()?.checkpoint.committee.clone();
            (domain == previous_committee.domain).then_some(certificate)
        })
        .find_map(|certificate| {
            let checkpoint =
                verify_fastlane_checkpoint_certificate(previous_committee, certificate).ok()?;
            let checkpoint_id = checkpoint.checkpoint_id().ok()?;
            Some((checkpoint_id, checkpoint))
        })
        .ok_or_else(|| invalid_data("FastSwap migration drain checkpoint is absent or invalid"))?;
    let mut expected_fences = ledger
        .fastswap_policy_snapshots
        .iter()
        .map(|policy| policy.policy_epoch)
        .collect::<Vec<_>>();
    expected_fences.sort_unstable();
    expected_fences.dedup();
    if !previous_checkpoint.drain_ready
        || !previous_checkpoint.exit_claim_totals.is_empty()
        || previous_checkpoint.fenced_policy_epochs != expected_fences
        || !expected_fences.iter().all(|policy_epoch| {
            ledger.fast_lane_prepare_fences.iter().any(|fence| {
                fence.committee_epoch == previous_epoch && fence.policy_epoch == *policy_epoch
            })
        })
        || !state.pending_fee_burns.is_empty()
        || !state.reservations.is_empty()
        || state
            .prepare_fences
            .values()
            .any(|fence| fence.committee_epoch != committee.domain.committee_epoch)
    {
        return Err(invalid_data(
            "FastSwap migration base is not a clean drained state",
        ));
    }
    let candidate = build_fastlane_checkpoint(state, ledger, Some(previous_checkpoint_id), 0)
        .map_err(|error| {
            invalid_data(&format!(
                "FastSwap migration base checkpoint failed: {error:?}"
            ))
        })?;
    if candidate.live_object_root != previous_checkpoint.live_object_root
        || candidate.live_object_totals != previous_checkpoint.live_object_totals
        || candidate.exit_claim_root != previous_checkpoint.exit_claim_root
        || candidate.exit_claim_totals != previous_checkpoint.exit_claim_totals
        || candidate.terminal_root != previous_checkpoint.terminal_root
        || candidate.active_policy_hashes != previous_checkpoint.active_policy_hashes
        || candidate.imported_deposit_root != previous_checkpoint.imported_deposit_root
        || candidate.redeemed_exit_claim_root != previous_checkpoint.redeemed_exit_claim_root
    {
        return Err(invalid_data(
            "FastSwap migration base does not match the anchored drain checkpoint",
        ));
    }
    Ok(())
}

fn read_bounded_json<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    let bytes = fs::read(path)?;
    if bytes.len() > postfiat_types::FASTSWAP_MAX_INTENT_BYTES * 8 {
        return Err(invalid_data("FastSwap state file exceeds bound"));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| invalid_data(&format!("FastSwap JSON parse failed: {error}")))
}

fn provision_epoch_one_base_if_missing(directory: &Path, ledger: &LedgerState) -> io::Result<()> {
    let base_path = directory.join(FASTSWAP_BASE_STATE);
    let committee_path = directory.join(FASTSWAP_COMMITTEE);
    let base_exists = base_path.exists();
    let committee_exists = committee_path.exists();
    if base_exists && committee_exists {
        return Ok(());
    }
    let committee = ledger
        .fastswap_committees
        .last()
        .ok_or_else(|| invalid_data("FastSwap has no canonically registered committee"))?;
    if committee.domain.committee_epoch != 1
        || ledger.fastswap_activation_height.is_none()
        || ledger.fastswap_committees.len() != 1
    {
        return Err(invalid_data(
            "FastSwap automatic base provisioning is restricted to canonical committee epoch 1",
        ));
    }
    if directory.exists() {
        for entry in fs::read_dir(directory)? {
            let name = entry?.file_name();
            if name != FASTSWAP_BASE_STATE && name != FASTSWAP_COMMITTEE {
                return Err(invalid_data(
                    "FastSwap base provisioning found pre-existing durable state",
                ));
            }
        }
    } else {
        fs::create_dir_all(directory)?;
    }
    let expected_base = FastLaneStateV1::empty(committee.domain.clone());
    if base_exists && read_fastlane_base_state(&base_path)? != expected_base {
        return Err(invalid_data(
            "FastSwap partial epoch-one base does not match canonical empty state",
        ));
    }
    if committee_exists {
        let configured: FastSwapCommitteeV1 = read_bounded_json(&committee_path)?;
        if configured != *committee {
            return Err(invalid_data(
                "FastSwap partial epoch-one committee does not match canonical ledger",
            ));
        }
    }
    postfiat_storage::atomic_write(
        &base_path,
        postfiat_storage::fastswap_store::encode_fastlane_state_file(&expected_base)
            .map_err(store_error)?,
    )?;
    postfiat_storage::atomic_write(
        &committee_path,
        serde_json::to_vec(committee)
            .map_err(|error| invalid_data(&format!("FastSwap committee encode failed: {error}")))?,
    )?;
    Ok(())
}

fn read_fastlane_base_state(path: &Path) -> io::Result<FastLaneStateV1> {
    let length = fs::metadata(path)?.len();
    if length > postfiat_storage::fastswap_store::FASTLANE_STATE_FILE_MAX_BYTES as u64 {
        return Err(invalid_data("FastLane base state file exceeds bound"));
    }
    let bytes = fs::read(path)?;
    postfiat_storage::fastswap_store::decode_fastlane_state_file(&bytes).map_err(store_error)
}

fn load_validator_secret_key(data_dir: &Path, validator_id: &str) -> io::Result<Vec<u8>> {
    let file: serde_json::Value = read_bounded_json(&data_dir.join("validator_keys.json"))?;
    let private_key_hex = file
        .get("validators")
        .and_then(serde_json::Value::as_array)
        .and_then(|validators| {
            validators.iter().find_map(|entry| {
                (entry.get("node_id")?.as_str()? == validator_id)
                    .then(|| entry.get("private_key_hex")?.as_str().map(str::to_owned))
                    .flatten()
            })
        })
        .ok_or_else(|| invalid_data("local FastSwap validator secret key is missing"))?;
    hex_to_bytes(&private_key_hex)
        .map_err(|error| invalid_data(&format!("FastSwap validator key hex invalid: {error}")))
}

fn codec_error(error: postfiat_types::FastSwapCodecError) -> io::Error {
    invalid_input(&format!("FastSwap canonical encoding failed: {error:?}"))
}

fn decision_error(error: FastSwapDecisionError) -> io::Error {
    invalid_input(&format!(
        "FastSwap certificate verification failed: {error:?}"
    ))
}

fn store_error(error: postfiat_storage::fastswap_store::FastSwapStoreError) -> io::Error {
    invalid_data(&format!("FastSwap durable store failed: {error}"))
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn invalid_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub(crate) fn parse_swap_id_hex(value: &str) -> io::Result<FastSwapIdV1> {
    let bytes = hex_to_bytes(value)
        .map_err(|error| invalid_input(&format!("FastSwap swap_id hex invalid: {error}")))?;
    let value: [u8; 48] = bytes
        .try_into()
        .map_err(|_| invalid_input("FastSwap swap_id must be exactly 48 bytes"))?;
    Ok(FastSwapIdV1(value))
}

pub(crate) fn parse_checkpoint_id_hex(value: &str) -> io::Result<FastLaneCheckpointIdV1> {
    let bytes = hex_to_bytes(value)
        .map_err(|error| invalid_input(&format!("FastLane checkpoint id hex invalid: {error}")))?;
    let value: [u8; 48] = bytes
        .try_into()
        .map_err(|_| invalid_input("FastLane checkpoint id must be exactly 48 bytes"))?;
    Ok(FastLaneCheckpointIdV1(value))
}

pub(crate) fn parse_asset_id_hex(value: &str) -> io::Result<postfiat_types::FastAssetIdV1> {
    Ok(postfiat_types::FastAssetIdV1(parse_hex_48(
        value,
        "FastSwap asset id",
    )?))
}

pub(crate) fn parse_policy_hash_hex(value: &str) -> io::Result<FastSwapPolicyHashV1> {
    Ok(FastSwapPolicyHashV1(parse_hex_48(
        value,
        "FastSwap policy hash",
    )?))
}

pub(crate) fn parse_object_key(value: &str, version: u64) -> io::Result<FastObjectKeyV1> {
    let bytes = hex_to_bytes(value)
        .map_err(|error| invalid_input(&format!("FastSwap object id hex invalid: {error}")))?;
    let object_id: [u8; 32] = bytes
        .try_into()
        .map_err(|_| invalid_input("FastSwap object id must be exactly 32 bytes"))?;
    Ok(FastObjectKeyV1 {
        object_id: postfiat_types::FastObjectIdV1(object_id),
        version,
    })
}

fn parse_hex_48(value: &str, label: &str) -> io::Result<[u8; 48]> {
    let bytes = hex_to_bytes(value)
        .map_err(|error| invalid_input(&format!("{label} hex invalid: {error}")))?;
    bytes
        .try_into()
        .map_err(|_| invalid_input(&format!("{label} must be exactly 48 bytes")))
}

#[cfg(test)]
pub(crate) fn swap_id_hex(value: FastSwapIdV1) -> String {
    postfiat_crypto_provider::bytes_to_hex(&value.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{
        address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed,
        ml_dsa_65_sign_with_context,
    };
    use postfiat_rpc_sdk::{
        aggregate_fastlane_exit_votes, aggregate_fastswap_new_round_votes,
        aggregate_fastswap_votes, drive_fast_asset_control_three_wave, drive_fastswap_three_wave,
        preview_fast_asset_control, preview_fastswap, reconcile_fast_asset_control_replication,
        reconcile_fastswap_replication, recover_fastswap_round, verify_fast_asset_control_terminal,
        verify_fastswap_terminal, FastAssetControlWalletProgressV1, FastSwapClientError,
        FastSwapProductStateV1, FastSwapRecoveryOutcomeV1, FastSwapRpcTransportV1,
        FastSwapStageTimingsV1, FastSwapWalletSessionV1, RpcRequest, RpcResponse,
        SwapSettlementModeV1, METHOD_FASTLANE_ASSET_CONTROL_APPLY,
        METHOD_FASTLANE_ASSET_CONTROL_CATCH_UP, METHOD_FASTLANE_ASSET_CONTROL_PREPARE,
        METHOD_FASTLANE_ASSET_CONTROL_PREVIEW, METHOD_FASTSWAP_APPLY, METHOD_FASTSWAP_CANCEL_APPLY,
        METHOD_FASTSWAP_CATCH_UP, METHOD_FASTSWAP_COMMIT, METHOD_FASTSWAP_COMMIT_ROUND,
        METHOD_FASTSWAP_NEW_ROUND_VOTE, METHOD_FASTSWAP_PRECOMMIT, METHOD_FASTSWAP_PREPARE,
        METHOD_FASTSWAP_PREVIEW, METHOD_FASTSWAP_VOTES, RPC_VERSION,
    };
    use postfiat_types::{
        ChainTipState, FastAssetControlActionV1, FastAssetControlCommandV1,
        FastAssetControlStateV1, FastAssetDefinitionHashV1, FastAssetIdV1, FastAssetObjectV1,
        FastAssetRuleHashV1, FastAssetRuleV1, FastLaneExitIntentV1, FastObjectIdV1,
        FastObjectKeyV1, FastObjectOriginV1, FastSwapAuthorizationV1, FastSwapCertificateV1,
        FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
        FastSwapDepositIdV1, FastSwapIntentV1, FastSwapMarketEnvelopeHashV1, FastSwapOpaqueHashV1,
        FastSwapPartyV1, FastSwapPolicyHashV1, FastSwapPolicySnapshotV1, FastSwapProposalV1,
        FastSwapQuoteRoundingV1, FastSwapRfqHashV1, FastSwapValidatorV1,
        SignedFastAssetControlCommandV1, SignedFastLaneExitIntentV1,
        FASTLANE_ASSET_CONTROL_CONTEXT_V1, FASTLANE_EXIT_CONTEXT_V1, FASTSWAP_INTENT_CONTEXT_V1,
        FASTSWAP_ML_DSA_65, FASTSWAP_SCHEMA_VERSION_V1, FASTSWAP_VOTE_CONTEXT_V1,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Condvar, Mutex};

    static TEST_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        base: FastLaneStateV1,
        committee: FastSwapCommitteeV1,
        validator_keys: Vec<Vec<u8>>,
        owner_0_private_key: Vec<u8>,
        owner_1_private_key: Vec<u8>,
        signed: SignedFastSwapIntentV1,
    }

    #[derive(Clone)]
    struct InMemoryFastSwapTransport {
        validators: Arc<BTreeMap<String, Mutex<FastSwapValidatorServiceV1>>>,
    }

    #[derive(Clone)]
    struct PartitionedFastSwapTransport {
        inner: InMemoryFastSwapTransport,
        reachable: BTreeSet<String>,
    }

    #[derive(Clone)]
    struct LostResponseFastSwapTransport {
        inner: InMemoryFastSwapTransport,
        method: &'static str,
        validators: BTreeSet<String>,
    }

    #[derive(Clone)]
    struct EvidenceOnlyFastSwapTransport {
        inner: InMemoryFastSwapTransport,
    }

    #[derive(Default)]
    struct ResponseGateState {
        released: bool,
        blocked: usize,
    }

    #[derive(Clone)]
    struct GatedResponseFastSwapTransport {
        inner: InMemoryFastSwapTransport,
        validator_id: String,
        gate: Arc<(Mutex<ResponseGateState>, Condvar)>,
    }

    impl FastSwapRpcTransportV1 for GatedResponseFastSwapTransport {
        fn call(&self, validator_id: &str, request: &RpcRequest) -> Result<RpcResponse, String> {
            let response = self.inner.call(validator_id, request)?;
            if validator_id != self.validator_id {
                return Ok(response);
            }
            let (state_lock, state_changed) = &*self.gate;
            let mut state = state_lock
                .lock()
                .map_err(|_| "gated response state lock poisoned".to_owned())?;
            state.blocked += 1;
            state_changed.notify_all();
            while !state.released {
                state = state_changed
                    .wait(state)
                    .map_err(|_| "gated response state lock poisoned".to_owned())?;
            }
            state.blocked -= 1;
            state_changed.notify_all();
            Ok(response)
        }
    }

    impl FastSwapRpcTransportV1 for EvidenceOnlyFastSwapTransport {
        fn call(&self, validator_id: &str, request: &RpcRequest) -> Result<RpcResponse, String> {
            if !matches!(
                request.method.as_str(),
                METHOD_FASTSWAP_NEW_ROUND_VOTE | METHOD_FASTSWAP_VOTES
            ) {
                return Err(format!(
                    "terminal reconciliation attempted mutation {}",
                    request.method
                ));
            }
            self.inner.call(validator_id, request)
        }
    }

    impl FastSwapRpcTransportV1 for LostResponseFastSwapTransport {
        fn call(&self, validator_id: &str, request: &RpcRequest) -> Result<RpcResponse, String> {
            let response = self.inner.call(validator_id, request)?;
            if request.method == self.method && self.validators.contains(validator_id) {
                return Err(format!(
                    "lost {validator_id} response after durable handling"
                ));
            }
            Ok(response)
        }
    }

    impl FastSwapRpcTransportV1 for PartitionedFastSwapTransport {
        fn call(&self, validator_id: &str, request: &RpcRequest) -> Result<RpcResponse, String> {
            if !self.reachable.contains(validator_id) {
                return Err(format!("validator {validator_id} is across the partition"));
            }
            self.inner.call(validator_id, request)
        }
    }

    impl FastSwapRpcTransportV1 for InMemoryFastSwapTransport {
        fn call(&self, validator_id: &str, request: &RpcRequest) -> Result<RpcResponse, String> {
            let mut validator = self
                .validators
                .get(validator_id)
                .ok_or_else(|| format!("unknown validator {validator_id}"))?
                .lock()
                .map_err(|_| format!("validator {validator_id} lock poisoned"))?;
            let string_param = |name: &str| {
                request
                    .params
                    .get(name)
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| format!("missing {name}"))
            };
            if request.method == METHOD_FASTSWAP_PREVIEW {
                let signed = serde_json::from_str::<SignedFastSwapIntentV1>(string_param(
                    "signed_intent_json",
                )?)
                .map_err(|error| error.to_string())?;
                let preview = validator
                    .preview(&signed)
                    .map_err(|error| error.to_string())?;
                return Ok(RpcResponse {
                    version: RPC_VERSION.to_owned(),
                    id: request.id.clone(),
                    ok: true,
                    result: Some(serde_json::to_value(preview).map_err(|error| error.to_string())?),
                    error: None,
                    events: Vec::new(),
                });
            }
            if request.method == METHOD_FASTLANE_ASSET_CONTROL_PREVIEW {
                let signed = serde_json::from_str::<SignedFastAssetControlCommandV1>(string_param(
                    "signed_command_json",
                )?)
                .map_err(|error| error.to_string())?;
                let preview = validator
                    .asset_control_preview(&signed)
                    .map_err(|error| error.to_string())?;
                return Ok(RpcResponse {
                    version: RPC_VERSION.to_owned(),
                    id: request.id.clone(),
                    ok: true,
                    result: Some(serde_json::to_value(preview).map_err(|error| error.to_string())?),
                    error: None,
                    events: Vec::new(),
                });
            }
            if request.method == METHOD_FASTSWAP_NEW_ROUND_VOTE {
                let swap_id = request
                    .params
                    .get("swap_id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "missing swap_id".to_owned())
                    .and_then(|value| {
                        parse_swap_id_hex(value).map_err(|error| error.to_string())
                    })?;
                let target_round = request
                    .params
                    .get("target_round")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| "missing target_round".to_owned())?;
                let vote = validator
                    .new_round_vote(swap_id, target_round)
                    .map_err(|error| error.to_string())?;
                return Ok(RpcResponse {
                    version: RPC_VERSION.to_owned(),
                    id: request.id.clone(),
                    ok: true,
                    result: Some(serde_json::to_value(vote).map_err(|error| error.to_string())?),
                    error: None,
                    events: Vec::new(),
                });
            }
            if request.method == METHOD_FASTSWAP_VOTES {
                let swap_id = request
                    .params
                    .get("swap_id")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| "missing swap_id".to_owned())
                    .and_then(|value| {
                        parse_swap_id_hex(value).map_err(|error| error.to_string())
                    })?;
                let phase = request
                    .params
                    .get("phase")
                    .and_then(serde_json::Value::as_str)
                    .and_then(postfiat_rpc_sdk::parse_fastswap_phase)
                    .ok_or_else(|| "missing phase".to_owned())?;
                let round = request
                    .params
                    .get("round")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or_else(|| "missing round".to_owned())?;
                let evidence = validator
                    .vote_evidence(swap_id, phase, round)
                    .map_err(|error| error.to_string())?;
                return Ok(RpcResponse {
                    version: RPC_VERSION.to_owned(),
                    id: request.id.clone(),
                    ok: true,
                    result: Some(
                        serde_json::to_value(evidence).map_err(|error| error.to_string())?,
                    ),
                    error: None,
                    events: Vec::new(),
                });
            }
            let vote = match request.method.as_str() {
                METHOD_FASTSWAP_PREPARE => {
                    let signed = serde_json::from_str::<SignedFastSwapIntentV1>(string_param(
                        "signed_intent_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    validator
                        .prepare(&signed)
                        .map_err(|error| error.to_string())?
                }
                METHOD_FASTSWAP_COMMIT => {
                    let qc = serde_json::from_str::<FastSwapCertificateV1>(string_param(
                        "lock_qc_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    validator.commit(&qc).map_err(|error| error.to_string())?
                }
                METHOD_FASTSWAP_PRECOMMIT => {
                    let proposal =
                        serde_json::from_str::<FastSwapProposalV1>(string_param("proposal_json")?)
                            .map_err(|error| error.to_string())?;
                    validator
                        .precommit_round(&proposal)
                        .map_err(|error| error.to_string())?
                }
                METHOD_FASTSWAP_COMMIT_ROUND => {
                    let qc = serde_json::from_str::<FastSwapCertificateV1>(string_param(
                        "precommit_qc_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    validator
                        .commit_round(&qc)
                        .map_err(|error| error.to_string())?
                }
                METHOD_FASTSWAP_CANCEL_APPLY => {
                    let qc = serde_json::from_str::<FastSwapCertificateV1>(string_param(
                        "decision_qc_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    validator
                        .cancel_apply(&qc)
                        .map_err(|error| error.to_string())?
                }
                METHOD_FASTSWAP_APPLY => {
                    let qc = serde_json::from_str::<FastSwapCertificateV1>(string_param(
                        "decision_qc_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    let signed = serde_json::from_str::<SignedFastSwapIntentV1>(string_param(
                        "signed_intent_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    validator
                        .apply(&qc, &signed)
                        .map_err(|error| error.to_string())?
                }
                METHOD_FASTSWAP_CATCH_UP => {
                    let lock_qc = serde_json::from_str::<FastSwapCertificateV1>(string_param(
                        "lock_qc_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    let decision_qc = serde_json::from_str::<FastSwapCertificateV1>(string_param(
                        "decision_qc_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    let signed = serde_json::from_str::<SignedFastSwapIntentV1>(string_param(
                        "signed_intent_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    validator
                        .catch_up_confirm(&lock_qc, &decision_qc, &signed)
                        .map_err(|error| error.to_string())?
                }
                METHOD_FASTLANE_ASSET_CONTROL_PREPARE => {
                    let signed = serde_json::from_str::<SignedFastAssetControlCommandV1>(
                        string_param("signed_command_json")?,
                    )
                    .map_err(|error| error.to_string())?;
                    validator
                        .asset_control_prepare(&signed)
                        .map_err(|error| error.to_string())?
                }
                METHOD_FASTLANE_ASSET_CONTROL_APPLY => {
                    let qc = serde_json::from_str::<FastSwapCertificateV1>(string_param(
                        "decision_qc_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    let signed = serde_json::from_str::<SignedFastAssetControlCommandV1>(
                        string_param("signed_command_json")?,
                    )
                    .map_err(|error| error.to_string())?;
                    validator
                        .asset_control_apply(&qc, &signed)
                        .map_err(|error| error.to_string())?
                }
                METHOD_FASTLANE_ASSET_CONTROL_CATCH_UP => {
                    let lock_qc = serde_json::from_str::<FastSwapCertificateV1>(string_param(
                        "lock_qc_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    let decision_qc = serde_json::from_str::<FastSwapCertificateV1>(string_param(
                        "decision_qc_json",
                    )?)
                    .map_err(|error| error.to_string())?;
                    let signed = serde_json::from_str::<SignedFastAssetControlCommandV1>(
                        string_param("signed_command_json")?,
                    )
                    .map_err(|error| error.to_string())?;
                    validator
                        .asset_control_catch_up(&lock_qc, &decision_qc, &signed)
                        .map_err(|error| error.to_string())?
                }
                method => return Err(format!("unexpected method {method}")),
            };
            Ok(RpcResponse {
                version: RPC_VERSION.to_owned(),
                id: request.id.clone(),
                ok: true,
                result: Some(serde_json::to_value(vote).map_err(|error| error.to_string())?),
                error: None,
                events: Vec::new(),
            })
        }
    }

    fn object(
        id: u8,
        owner: &[u8],
        asset_id: FastAssetIdV1,
        rule: FastAssetRuleHashV1,
        amount_atoms: u64,
    ) -> FastAssetObjectV1 {
        FastAssetObjectV1 {
            key: FastObjectKeyV1 {
                object_id: FastObjectIdV1([id; 32]),
                version: 1,
            },
            owner_pubkey: owner.to_vec(),
            asset_id,
            asset_rule_hash: rule,
            amount_atoms,
            control_state: FastAssetControlStateV1::Spendable,
            origin: FastObjectOriginV1::Deposit {
                deposit_id: FastSwapDepositIdV1([id; 48]),
            },
        }
    }

    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "postfiat-fastswap-service-{}-{}",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let validator_pairs = (0..6)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 10; 32]))
            .collect::<Vec<_>>();
        let validators = validator_pairs
            .iter()
            .enumerate()
            .map(|(index, pair)| FastSwapValidatorV1 {
                validator_id: format!("validator-{index}"),
                public_key: pair.public_key.clone(),
            })
            .collect::<Vec<_>>();
        let chain = FastSwapChainDomainV1 {
            chain_id: "postfiat-fastswap-test".to_owned(),
            genesis_hash: FastSwapOpaqueHashV1([9; 48]),
            protocol_version: 1,
        };
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: chain.clone(),
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 6,
                quorum: 5,
            },
            validators,
        };
        committee.domain.committee_root = committee.computed_root().expect("committee root");
        committee.validate().expect("committee");

        let owner_0 = ml_dsa_65_keygen_from_seed(&[1; 32]);
        let owner_1 = ml_dsa_65_keygen_from_seed(&[2; 32]);
        let asset_0 = FastAssetIdV1([1; 48]);
        let asset_1 = FastAssetIdV1([2; 48]);
        let asset_rule_0 = FastAssetRuleV1 {
            asset_id: asset_0,
            asset_definition_hash: FastAssetDefinitionHashV1([3; 48]),
            issuer_address: "issuer-0".to_owned(),
            issuer_control_pubkey: vec![31; 64],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 100,
            valid_through_height: 120,
        };
        let asset_rule_1 = FastAssetRuleV1 {
            asset_id: asset_1,
            asset_definition_hash: FastAssetDefinitionHashV1([4; 48]),
            issuer_address: "issuer-1".to_owned(),
            issuer_control_pubkey: vec![32; 64],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 100,
            valid_through_height: 120,
        };
        let rule_0 = asset_rule_0.rule_hash().expect("rule 0");
        let rule_1 = asset_rule_1.rule_hash().expect("rule 1");
        let native = FastAssetIdV1::native_pft();
        let objects = [
            object(1, &owner_0.public_key, asset_0, rule_0, 10),
            object(
                2,
                &owner_0.public_key,
                native,
                FastAssetRuleHashV1::ZERO,
                10,
            ),
            object(3, &owner_1.public_key, asset_1, rule_1, 3),
            object(
                4,
                &owner_1.public_key,
                native,
                FastAssetRuleHashV1::ZERO,
                10,
            ),
        ];
        let party_0 = FastSwapPartyV1 {
            owner_address: address_from_public_key(&owner_0.public_key),
            owner_pubkey: owner_0.public_key.clone(),
            offered_asset_id: asset_0,
            offered_asset_rule_hash: rule_0,
            offered_amount: 8,
            receives_asset_id: asset_1,
            receives_asset_rule_hash: rule_1,
            receives_holder_permit_id: None,
            receives_amount: 1,
            asset_inputs: vec![objects[0].key],
            fee_inputs: vec![objects[1].key],
            asset_change: 2,
            fee_change: 9,
            fee_burn_pft: 1,
        };
        let party_1 = FastSwapPartyV1 {
            owner_address: address_from_public_key(&owner_1.public_key),
            owner_pubkey: owner_1.public_key.clone(),
            offered_asset_id: asset_1,
            offered_asset_rule_hash: rule_1,
            offered_amount: 1,
            receives_asset_id: asset_0,
            receives_asset_rule_hash: rule_0,
            receives_holder_permit_id: None,
            receives_amount: 8,
            asset_inputs: vec![objects[2].key],
            fee_inputs: vec![objects[3].key],
            asset_change: 2,
            fee_change: 9,
            fee_burn_pft: 1,
        };
        let envelope = FastSwapMarketEnvelopeHashV1([6; 48]);
        let mut policy = FastSwapPolicySnapshotV1 {
            domain: chain.clone(),
            policy_epoch: 1,
            policy_hash: FastSwapPolicyHashV1::ZERO,
            pair_asset_0: asset_0,
            pair_asset_1: asset_1,
            asset_rule_hash_0: rule_0,
            asset_rule_hash_1: rule_1,
            price_numerator: 1,
            price_denominator: 8,
            rounding: FastSwapQuoteRoundingV1::Exact,
            nav_epoch: 59,
            market_envelope_hash: envelope,
            valid_from_height: 100,
            valid_through_height: 120,
            fee_schedule_hash: FastSwapOpaqueHashV1([10; 48]),
            max_inputs_per_party: 16,
            max_outputs: 8,
            paused: false,
        };
        let policy_hash = policy.computed_hash().expect("policy hash");
        policy.policy_hash = policy_hash;
        let intent = FastSwapIntentV1 {
            domain: committee.domain.clone(),
            policy_hash,
            rfq_hash: FastSwapRfqHashV1([7; 48]),
            market_envelope_hash: envelope,
            nav_epoch: 59,
            expires_at_height: 120,
            nonce: [8; 32],
            party_0,
            party_1,
        };
        let intent_bytes = intent.canonical_bytes().expect("intent");
        let signed = SignedFastSwapIntentV1 {
            intent,
            authorization_0: FastSwapAuthorizationV1 {
                role: 0,
                algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
                public_key: owner_0.public_key,
                signature: ml_dsa_65_sign_with_context(
                    &owner_0.private_key,
                    &intent_bytes,
                    FASTSWAP_INTENT_CONTEXT_V1,
                )
                .expect("owner 0 sign"),
            },
            authorization_1: FastSwapAuthorizationV1 {
                role: 1,
                algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
                public_key: owner_1.public_key,
                signature: ml_dsa_65_sign_with_context(
                    &owner_1.private_key,
                    &intent_bytes,
                    FASTSWAP_INTENT_CONTEXT_V1,
                )
                .expect("owner 1 sign"),
            },
        };
        Fixture {
            root,
            base: FastLaneStateV1 {
                schema_version: 1,
                committee: committee.domain.clone(),
                objects: objects
                    .into_iter()
                    .map(|object| (object.key, object))
                    .collect(),
                reservations: BTreeMap::new(),
                swaps: BTreeMap::new(),
                imported_deposits: BTreeSet::new(),
                exit_claims: BTreeMap::new(),
                terminal_tombstones: BTreeMap::new(),
                asset_rules: BTreeMap::from([(rule_0, asset_rule_0), (rule_1, asset_rule_1)]),
                holder_permits: BTreeMap::new(),
                policy_snapshots: BTreeMap::from([(policy_hash, policy)]),
                prepare_fences: BTreeMap::new(),
                pending_fee_burns: BTreeMap::new(),
                anchored_checkpoints: BTreeSet::new(),
            },
            committee,
            validator_keys: validator_pairs
                .into_iter()
                .map(|pair| pair.private_key.to_vec())
                .collect(),
            owner_0_private_key: owner_0.private_key.to_vec(),
            owner_1_private_key: owner_1.private_key.to_vec(),
            signed,
        }
    }

    #[test]
    fn swap_id_parser_is_exact_width() {
        let id = FastSwapIdV1([7; 48]);
        assert_eq!(parse_swap_id_hex(&swap_id_hex(id)).expect("parse"), id);
        assert!(parse_swap_id_hex("00").is_err());
    }

    #[test]
    fn validator_opens_nonempty_canonical_base_state_file() {
        let fixture = fixture();
        let data_dir = fixture.root.join("base-state-open-node");
        let fastswap_dir = data_dir.join(FASTSWAP_DIRECTORY);
        fs::create_dir_all(&fastswap_dir).expect("create FastSwap directory");
        fs::write(
            fastswap_dir.join(FASTSWAP_BASE_STATE),
            postfiat_storage::fastswap_store::encode_fastlane_state_file(&fixture.base)
                .expect("base state file"),
        )
        .expect("write base state");
        fs::write(
            fastswap_dir.join(FASTSWAP_COMMITTEE),
            serde_json::to_vec(&fixture.committee).expect("committee JSON"),
        )
        .expect("write committee");
        fs::write(
            data_dir.join("validator_keys.json"),
            serde_json::to_vec(&serde_json::json!({
                "validators": [{
                    "node_id": "validator-0",
                    "private_key_hex": bytes_to_hex(&fixture.validator_keys[0]),
                }]
            }))
            .expect("validator key JSON"),
        )
        .expect("write validator key");
        let mut ledger = LedgerState::empty();
        ledger.fastswap_committees.push(fixture.committee.clone());
        ledger.fast_lane_asset_rules = fixture.base.asset_rules.values().cloned().collect();
        ledger.fastswap_policy_snapshots =
            fixture.base.policy_snapshots.values().cloned().collect();
        ledger.fastswap_activation_height = Some(1);
        let store = NodeStore::new(&data_dir);
        store.write_ledger(&ledger).expect("write ledger");
        store
            .write_chain_tip(&ChainTipState {
                schema: "postfiat-chain-tip-v1".to_owned(),
                chain_id: "test".to_owned(),
                genesis_hash: "genesis".to_owned(),
                protocol_version: 1,
                height: 110,
                block_hash: "block".to_owned(),
                state_root: "root".to_owned(),
                ordered_batch_count: 0,
                receipt_count: 0,
                history_base_height: 0,
            })
            .expect("write tip");
        let validator = FastSwapValidatorServiceV1::open(&data_dir, "validator-0")
            .expect("open validator from canonical state file");
        assert_eq!(validator.state.objects, fixture.base.objects);
        assert!(validator.capabilities().enabled);
        assert_eq!(
            validator.preview(&fixture.signed).expect("preview").effects,
            validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
                .expect("expected preview")
                .effects
        );
        drop(validator);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn validator_provisions_empty_epoch_one_base_from_canonical_governance() {
        let fixture = fixture();
        let data_dir = fixture.root.join("governance-provisioned-node");
        fs::create_dir_all(&data_dir).expect("create node directory");
        fs::write(
            data_dir.join("validator_keys.json"),
            serde_json::to_vec(&serde_json::json!({
                "validators": [{
                    "node_id": "validator-0",
                    "private_key_hex": bytes_to_hex(&fixture.validator_keys[0]),
                }]
            }))
            .expect("validator key JSON"),
        )
        .expect("write validator key");
        let mut ledger = LedgerState::empty();
        ledger.fastswap_committees.push(fixture.committee.clone());
        ledger.fast_lane_asset_rules = fixture.base.asset_rules.values().cloned().collect();
        ledger.fastswap_policy_snapshots =
            fixture.base.policy_snapshots.values().cloned().collect();
        ledger.fastswap_activation_height = Some(1);
        let store = NodeStore::new(&data_dir);
        store.write_ledger(&ledger).expect("write ledger");
        store
            .write_chain_tip(&ChainTipState {
                schema: "postfiat-chain-tip-v1".to_owned(),
                chain_id: "test".to_owned(),
                genesis_hash: "genesis".to_owned(),
                protocol_version: 1,
                height: 110,
                block_hash: "block".to_owned(),
                state_root: "root".to_owned(),
                ordered_batch_count: 0,
                receipt_count: 0,
                history_base_height: 0,
            })
            .expect("write chain tip");

        let service = FastSwapValidatorServiceV1::open(&data_dir, "validator-0")
            .expect("auto-provision epoch-one base");
        assert!(service.capabilities().enabled);
        assert!(service.state.objects.is_empty());
        assert!(data_dir
            .join(FASTSWAP_DIRECTORY)
            .join(FASTSWAP_BASE_STATE)
            .is_file());
        assert!(data_dir
            .join(FASTSWAP_DIRECTORY)
            .join(FASTSWAP_COMMITTEE)
            .is_file());
        drop(service);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn canonical_activation_height_gates_swap_votes_fail_closed() {
        let fixture = fixture();
        let mut validator = FastSwapValidatorServiceV1::from_parts(
            &fixture.root.join("activation-validator"),
            fixture.base.clone(),
            fixture.committee.clone(),
            "validator-0".to_owned(),
            fixture.validator_keys[0].clone(),
            110,
        )
        .expect("validator");
        validator.activation_height = 111;
        assert!(!validator.capabilities().enabled);
        let before = validator.state.clone();
        assert_eq!(
            validator
                .preview(&fixture.signed)
                .expect("shadow preview")
                .effects,
            validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
                .expect("expected shadow effects")
                .effects
        );
        assert_eq!(validator.state, before);
        let offered = &fixture.signed.intent.party_0;
        let exit_intent = FastLaneExitIntentV1 {
            committee: fixture.committee.domain.clone(),
            owner_address: offered.owner_address.clone(),
            owner_pubkey: offered.owner_pubkey.clone(),
            inputs: offered.asset_inputs.clone(),
            asset_id: offered.offered_asset_id,
            asset_rule_hash: offered.offered_asset_rule_hash,
            amount_atoms: 10,
            destination_address: "preactivation-destination".to_owned(),
            nonce: [91; 32],
        };
        let signed_exit = SignedFastLaneExitIntentV1 {
            signature: ml_dsa_65_sign_with_context(
                &fixture.owner_0_private_key,
                &exit_intent.canonical_bytes().expect("exit bytes"),
                FASTLANE_EXIT_CONTEXT_V1,
            )
            .expect("exit sign"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            intent: exit_intent,
        };
        assert_eq!(
            validator
                .exit(&signed_exit)
                .expect_err("pre-activation exit must reject")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(validator.state, before);
        assert_eq!(
            validator
                .prepare(&fixture.signed)
                .expect_err("pre-activation must reject")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(validator.state.reservations.is_empty());
        validator.finalized_primary_height = 111;
        validator.committee_active = false;
        assert!(!validator.capabilities().enabled);
        assert_eq!(
            validator
                .prepare(&fixture.signed)
                .expect_err("retired committee must reject")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(validator.state.reservations.is_empty());
        validator.committee_active = true;
        assert!(validator.capabilities().enabled);
        validator
            .prepare(&fixture.signed)
            .expect("activation-height prepare");
        drop(validator);
        if fixture.root.exists() {
            fs::remove_dir_all(&fixture.root).expect("cleanup");
        }
    }

    #[test]
    fn canonical_checkpoint_clears_pending_burn_once_and_replays() {
        let fixture = fixture();
        let data_dir = fixture.root.join("checkpoint-refresh-node");
        let fastswap_dir = data_dir.join(FASTSWAP_DIRECTORY);
        fs::create_dir_all(&fastswap_dir).expect("create FastSwap directory");
        fs::write(
            fastswap_dir.join(FASTSWAP_COMMITTEE),
            serde_json::to_vec(&fixture.committee).expect("committee JSON"),
        )
        .expect("write committee");

        let mut base = fixture.base.clone();
        base.objects.clear();
        base.reservations.clear();
        base.swaps.clear();
        base.policy_snapshots.clear();
        let fee_asset = FastAssetIdV1([93; 48]);
        base.pending_fee_burns.insert(fee_asset, 7);
        let mut validator = FastSwapValidatorServiceV1::from_parts(
            &fastswap_dir,
            base.clone(),
            fixture.committee.clone(),
            "validator-0".to_owned(),
            fixture.validator_keys[0].clone(),
            110,
        )
        .expect("validator");

        let mut ledger = LedgerState::empty();
        ledger.fastswap_committees.push(fixture.committee.clone());
        ledger
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
                asset_id: fee_asset,
                amount_atoms: 7,
            });
        let checkpoint = build_fastlane_checkpoint(&base, &ledger, None, 0).expect("checkpoint");
        let votes = fixture
            .committee
            .validators
            .iter()
            .zip(fixture.validator_keys.iter())
            .take(5)
            .map(|(member, key)| {
                let mut vote = FastLaneCheckpointVoteV1 {
                    checkpoint: checkpoint.clone(),
                    validator_id: member.validator_id.clone(),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    key,
                    &vote.signing_bytes().expect("checkpoint bytes"),
                    FASTLANE_CHECKPOINT_CONTEXT_V1,
                )
                .expect("checkpoint sign");
                vote
            })
            .collect();
        let certificate = postfiat_types::FastLaneCheckpointCertificateV1 { votes };
        postfiat_execution::fastswap_checkpoint::anchor_fastlane_checkpoint(
            &mut ledger,
            &certificate,
        )
        .expect("anchor checkpoint");
        assert!(ledger.fast_lane_reserves.is_empty());
        let node_store = NodeStore::new(&data_dir);
        node_store.write_ledger(&ledger).expect("write ledger");
        node_store
            .write_chain_tip(&ChainTipState {
                schema: "postfiat-chain-tip-v1".to_owned(),
                chain_id: "test".to_owned(),
                genesis_hash: "genesis".to_owned(),
                protocol_version: 1,
                height: 111,
                block_hash: "block".to_owned(),
                state_root: "root".to_owned(),
                ordered_batch_count: 0,
                receipt_count: 0,
                history_base_height: 0,
            })
            .expect("write tip");

        validator.refresh_canonical(&data_dir).expect("refresh");
        assert!(validator.state.pending_fee_burns.is_empty());
        let checkpoint_id = checkpoint.checkpoint_id().expect("checkpoint id");
        assert!(validator
            .state
            .anchored_checkpoints
            .contains(&checkpoint_id));
        let sequence = validator.store.highest_wal_sequence();
        validator
            .refresh_canonical(&data_dir)
            .expect("idempotent refresh");
        assert_eq!(validator.store.highest_wal_sequence(), sequence);
        drop(validator);

        let replayed = FastSwapValidatorServiceV1::from_parts(
            &fastswap_dir,
            base,
            fixture.committee.clone(),
            "validator-0".to_owned(),
            fixture.validator_keys[0].clone(),
            111,
        )
        .expect("replay validator");
        assert!(replayed.state.pending_fee_burns.is_empty());
        assert!(replayed.state.anchored_checkpoints.contains(&checkpoint_id));
        drop(replayed);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn migrated_committee_base_must_match_anchored_drain_root() {
        let fixture = fixture();
        let mut old_state = fixture.base.clone();
        let policy_epoch = old_state
            .policy_snapshots
            .values()
            .next()
            .expect("policy")
            .policy_epoch;
        let fence = postfiat_types::FastLanePrepareFenceV1 {
            committee_epoch: fixture.committee.domain.committee_epoch,
            policy_epoch,
            finalized_primary_height: 110,
        };
        old_state.prepare_fences.insert(policy_epoch, fence.clone());
        let mut ledger = LedgerState::empty();
        ledger.fastswap_committees.push(fixture.committee.clone());
        ledger.fastswap_policy_snapshots = old_state.policy_snapshots.values().cloned().collect();
        ledger.fast_lane_prepare_fences.push(fence);
        let mut reserve_totals = BTreeMap::<FastAssetIdV1, u128>::new();
        for object in old_state.objects.values() {
            *reserve_totals.entry(object.asset_id).or_default() += u128::from(object.amount_atoms);
        }
        ledger.fast_lane_reserves = reserve_totals
            .into_iter()
            .map(
                |(asset_id, amount_atoms)| postfiat_types::FastLaneReserveBalanceV1 {
                    asset_id,
                    amount_atoms,
                },
            )
            .collect();
        let checkpoint =
            build_fastlane_checkpoint(&old_state, &ledger, None, 0).expect("drain checkpoint");
        assert!(checkpoint.drain_ready);
        let votes = fixture
            .committee
            .validators
            .iter()
            .zip(fixture.validator_keys.iter())
            .take(5)
            .map(|(member, key)| {
                let mut vote = FastLaneCheckpointVoteV1 {
                    checkpoint: checkpoint.clone(),
                    validator_id: member.validator_id.clone(),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    key,
                    &vote.signing_bytes().expect("checkpoint bytes"),
                    FASTLANE_CHECKPOINT_CONTEXT_V1,
                )
                .expect("checkpoint sign");
                vote
            })
            .collect();
        let certificate = postfiat_types::FastLaneCheckpointCertificateV1 { votes };
        postfiat_execution::fastswap_checkpoint::anchor_fastlane_checkpoint(
            &mut ledger,
            &certificate,
        )
        .expect("anchor drain checkpoint");

        let mut next_committee = fixture.committee.clone();
        next_committee.domain.committee_epoch = 2;
        next_committee.domain.committee_root = postfiat_types::FastSwapCommitteeRootV1::ZERO;
        next_committee.domain.committee_root = next_committee.computed_root().expect("next root");
        ledger.fastswap_committees.push(next_committee.clone());
        let mut migrated = old_state;
        migrated.committee = next_committee.domain.clone();
        migrated.prepare_fences.clear();
        verify_migrated_committee_base(&migrated, &ledger, &next_committee)
            .expect("matching migration base");

        let first_key = *migrated.objects.keys().next().expect("object");
        migrated
            .objects
            .get_mut(&first_key)
            .expect("object")
            .amount_atoms -= 1;
        assert!(verify_migrated_committee_base(&migrated, &ledger, &next_committee).is_err());
        if fixture.root.exists() {
            fs::remove_dir_all(&fixture.root).expect("cleanup");
        }
    }

    #[test]
    fn issuer_freeze_unfreeze_and_clawback_use_the_swap_lock_domain() {
        let mut fixture = fixture();
        let issuer = ml_dsa_65_keygen_from_seed(&[91; 32]);
        let input_key = fixture.signed.intent.party_0.asset_inputs[0];
        let old_rule_hash = fixture
            .base
            .objects
            .get(&input_key)
            .expect("input object")
            .asset_rule_hash;
        let mut rule = fixture
            .base
            .asset_rules
            .remove(&old_rule_hash)
            .expect("asset rule");
        rule.issuer_address = address_from_public_key(&issuer.public_key);
        rule.issuer_control_pubkey = issuer.public_key.clone();
        rule.freeze_enabled = true;
        rule.clawback_enabled = true;
        let rule_hash = rule.rule_hash().expect("privileged rule hash");
        fixture.base.asset_rules.insert(rule_hash, rule);
        fixture
            .base
            .objects
            .get_mut(&input_key)
            .expect("input object")
            .asset_rule_hash = rule_hash;
        let old_policy_hash = fixture.signed.intent.policy_hash;
        let mut policy = fixture
            .base
            .policy_snapshots
            .remove(&old_policy_hash)
            .expect("policy");
        policy.asset_rule_hash_0 = rule_hash;
        policy.policy_hash = FastSwapPolicyHashV1::ZERO;
        policy.policy_hash = policy.computed_hash().expect("updated policy hash");
        fixture
            .base
            .policy_snapshots
            .insert(policy.policy_hash, policy.clone());
        fixture.signed.intent.policy_hash = policy.policy_hash;
        fixture.signed.intent.party_0.offered_asset_rule_hash = rule_hash;
        fixture.signed.intent.party_1.receives_asset_rule_hash = rule_hash;
        let updated_intent = fixture
            .signed
            .intent
            .canonical_bytes()
            .expect("updated intent bytes");
        fixture.signed.authorization_0.signature = ml_dsa_65_sign_with_context(
            &fixture.owner_0_private_key,
            &updated_intent,
            FASTSWAP_INTENT_CONTEXT_V1,
        )
        .expect("owner 0 updated signature");
        fixture.signed.authorization_1.signature = ml_dsa_65_sign_with_context(
            &fixture.owner_1_private_key,
            &updated_intent,
            FASTSWAP_INTENT_CONTEXT_V1,
        )
        .expect("owner 1 updated signature");

        let mut validators = (0..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture
                        .root
                        .join(format!("asset-control-validator-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator")
            })
            .collect::<Vec<_>>();
        let make_signed =
            |action: FastAssetControlActionV1, input: FastObjectKeyV1, nonce_byte: u8| {
                let command = FastAssetControlCommandV1 {
                    domain: fixture.committee.domain.clone(),
                    action,
                    input,
                    issuer_address: address_from_public_key(&issuer.public_key),
                    issuer_control_pubkey: issuer.public_key.clone(),
                    expires_at_height: 120,
                    nonce: [nonce_byte; 32],
                };
                let signature = ml_dsa_65_sign_with_context(
                    &issuer.private_key,
                    &command.canonical_bytes().expect("control bytes"),
                    FASTLANE_ASSET_CONTROL_CONTEXT_V1,
                )
                .expect("issuer signature");
                SignedFastAssetControlCommandV1 {
                    command,
                    algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
                    signature,
                }
            };
        let prelocked_freeze = make_signed(FastAssetControlActionV1::Freeze, input_key, 1);
        let mut owner_first = (0..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("owner-first-validator-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("owner-first validator")
            })
            .collect::<Vec<_>>();
        for validator in &mut owner_first {
            validator.prepare(&fixture.signed).expect("owner prepare");
            let error = validator
                .asset_control_prepare(&prelocked_freeze)
                .expect_err("issuer control must lose to owner reservation");
            assert!(error.to_string().contains("InputReserved"), "{error}");
        }
        drop(owner_first);
        for validator in &mut validators {
            validator
                .asset_control_prepare(&prelocked_freeze)
                .expect("freeze prepare");
            let error = validator
                .prepare(&fixture.signed)
                .expect_err("owner swap must lose to issuer reservation");
            assert!(error.to_string().contains("InputReserved"), "{error}");
        }

        let settle = |validators: &mut Vec<FastSwapValidatorServiceV1>,
                      action: FastAssetControlActionV1,
                      input: FastObjectKeyV1,
                      nonce_byte: u8| {
            let signed = make_signed(action, input, nonce_byte);
            let operation_id = signed.operation_id().expect("operation id");
            let lock_qc = aggregate_fastswap_votes(
                &fixture.committee,
                validators
                    .iter_mut()
                    .take(5)
                    .map(|validator| {
                        validator
                            .asset_control_prepare(&signed)
                            .expect("control prepare")
                    })
                    .collect::<Vec<_>>(),
                FastSwapPhaseV1::Precommit,
            )
            .expect("control LockQC");
            let decision_qc = aggregate_fastswap_votes(
                &fixture.committee,
                validators
                    .iter_mut()
                    .take(5)
                    .map(|validator| validator.commit(&lock_qc).expect("control commit"))
                    .collect::<Vec<_>>(),
                FastSwapPhaseV1::Commit,
            )
            .expect("control DecisionQC");
            let mut effects_votes = validators
                .iter_mut()
                .take(5)
                .map(|validator| {
                    validator
                        .asset_control_apply(&decision_qc, &signed)
                        .expect("control apply")
                })
                .collect::<Vec<_>>();
            effects_votes.push(
                validators[5]
                    .asset_control_catch_up(&lock_qc, &decision_qc, &signed)
                    .expect("control certified catch-up"),
            );
            let effects_qc = aggregate_fastswap_votes(
                &fixture.committee,
                effects_votes,
                FastSwapPhaseV1::Effects,
            )
            .expect("control EffectsQC");
            let effects = validators[0]
                .effects(operation_id)
                .expect("effects response")
                .effects
                .expect("control effects");
            verify_fast_asset_control_terminal(
                &fixture.committee,
                &effects,
                &lock_qc,
                &decision_qc,
                &effects_qc,
            )
            .expect("verified control terminal");
            effects
        };

        let frozen = settle(
            &mut validators,
            FastAssetControlActionV1::Freeze,
            input_key,
            1,
        );
        let frozen_output = frozen.created[0].clone();
        assert!(matches!(
            frozen_output.control_state,
            FastAssetControlStateV1::Frozen { .. }
        ));
        assert_eq!(
            frozen_output.owner_pubkey,
            fixture.base.objects[&input_key].owner_pubkey
        );

        let unfrozen = settle(
            &mut validators,
            FastAssetControlActionV1::Unfreeze,
            frozen_output.key,
            2,
        );
        let unfrozen_output = unfrozen.created[0].clone();
        assert_eq!(
            unfrozen_output.control_state,
            FastAssetControlStateV1::Spendable
        );

        let clawed_back = settle(
            &mut validators,
            FastAssetControlActionV1::Clawback,
            unfrozen_output.key,
            3,
        );
        let issuer_output = &clawed_back.created[0];
        assert_eq!(issuer_output.owner_pubkey, issuer.public_key);
        assert_eq!(issuer_output.amount_atoms, frozen_output.amount_atoms);
        assert_eq!(issuer_output.key.version, input_key.version + 3);
        assert!(validators.iter().all(|validator| {
            validator.state.objects.get(&issuer_output.key) == Some(issuer_output)
                && !validator.state.objects.contains_key(&input_key)
                && validator.state.reservations.is_empty()
        }));
        drop(validators);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn issuer_wallet_driver_previews_persists_and_repairs_exact_six() {
        let mut fixture = fixture();
        let issuer = ml_dsa_65_keygen_from_seed(&[92; 32]);
        let input_key = fixture.signed.intent.party_0.asset_inputs[0];
        let old_rule_hash = fixture.base.objects[&input_key].asset_rule_hash;
        let mut rule = fixture
            .base
            .asset_rules
            .remove(&old_rule_hash)
            .expect("asset rule");
        rule.issuer_address = address_from_public_key(&issuer.public_key);
        rule.issuer_control_pubkey = issuer.public_key.clone();
        rule.freeze_enabled = true;
        let rule_hash = rule.rule_hash().expect("control rule hash");
        fixture.base.asset_rules.insert(rule_hash, rule);
        for object in fixture.base.objects.values_mut() {
            if object.asset_rule_hash == old_rule_hash {
                object.asset_rule_hash = rule_hash;
            }
        }
        let command = FastAssetControlCommandV1 {
            domain: fixture.committee.domain.clone(),
            action: FastAssetControlActionV1::Freeze,
            input: input_key,
            issuer_address: address_from_public_key(&issuer.public_key),
            issuer_control_pubkey: issuer.public_key.clone(),
            expires_at_height: 120,
            nonce: [93; 32],
        };
        let signed = SignedFastAssetControlCommandV1 {
            signature: ml_dsa_65_sign_with_context(
                &issuer.private_key,
                &command.canonical_bytes().expect("command bytes"),
                FASTLANE_ASSET_CONTROL_CONTEXT_V1,
            )
            .expect("issuer signature"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            command,
        };
        let validators = (0..6)
            .map(|index| {
                let id = format!("validator-{index}");
                let validator = FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("issuer-driver-{id}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    id.clone(),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator");
                (id, Mutex::new(validator))
            })
            .collect::<BTreeMap<_, _>>();
        let transport = InMemoryFastSwapTransport {
            validators: Arc::new(validators),
        };
        let expected = preview_fast_asset_control(&signed, &fixture.committee, &transport)
            .expect("quorum control preview");
        assert!(transport.validators.values().all(|validator| {
            let validator = validator.lock().expect("validator");
            validator.state.reservations.is_empty() && validator.state.swaps.is_empty()
        }));
        let mut progress = FastAssetControlWalletProgressV1::new(signed, expected.clone())
            .expect("control progress");
        let mut snapshots = Vec::new();
        let terminal = drive_fast_asset_control_three_wave(
            &mut progress,
            &fixture.committee,
            &transport,
            |current| {
                snapshots.push(serde_json::to_vec(current).map_err(|error| error.to_string())?);
                Ok(())
            },
        )
        .expect("control driver");
        assert_eq!(terminal.effects, expected);
        assert_eq!(snapshots.len(), 3);
        assert_eq!(progress.replication_pending.len(), 1);
        let report = reconcile_fast_asset_control_replication(
            &mut progress,
            &fixture.committee,
            &transport,
            |_| Ok(()),
        )
        .expect("control replication");
        assert_eq!(report.applied.len(), 1);
        assert!(report.pending.is_empty());
        assert!(transport.validators.values().all(|validator| {
            let validator = validator.lock().expect("validator");
            validator
                .state
                .objects
                .get(&expected.created[0].key)
                .is_some_and(|object| object == &expected.created[0])
        }));
        drop(transport);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn six_validator_three_wave_lifecycle_is_terminal_and_restart_idempotent() {
        let fixture = fixture();
        let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
            .expect("expected effects")
            .effects;
        let mut validators = (0..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("validator-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator")
            })
            .collect::<Vec<_>>();
        let prepare_votes = validators
            .iter_mut()
            .map(|validator| validator.prepare(&fixture.signed).expect("prepare"))
            .collect::<Vec<_>>();
        let mut wallet = FastSwapWalletSessionV1::new(
            SwapSettlementModeV1::FastSwapV1,
            fixture.signed.clone(),
            expected.clone(),
        )
        .expect("wallet session");
        wallet.begin_fastswap().expect("begin FastSwap");
        let lock_qc = wallet
            .accept_prepare_votes(&fixture.committee, prepare_votes)
            .expect("LockQC")
            .clone();
        let commit_votes = validators
            .iter_mut()
            .map(|validator| validator.commit(&lock_qc).expect("commit"))
            .collect::<Vec<_>>();
        let decision_qc = wallet
            .accept_commit_votes(&fixture.committee, commit_votes)
            .expect("DecisionQC")
            .clone();
        let effects_votes = validators
            .iter_mut()
            .map(|validator| {
                validator
                    .apply(&decision_qc, &fixture.signed)
                    .expect("apply")
            })
            .collect::<Vec<_>>();
        let effects_qc = aggregate_fastswap_votes(
            &fixture.committee,
            effects_votes.clone(),
            FastSwapPhaseV1::Effects,
        )
        .expect("EffectsQC");
        wallet
            .accept_effects_votes(&fixture.committee, effects_votes.clone())
            .expect("wallet accepted");
        assert_eq!(wallet.state, FastSwapProductStateV1::Accepted);
        assert_eq!(
            FastSwapWalletSessionV1::from_durable_json(
                &wallet.to_durable_json().expect("durable session")
            )
            .expect("restore session"),
            wallet
        );
        verify_fastswap_terminal(
            &fixture.committee,
            &expected,
            &lock_qc,
            &decision_qc,
            &effects_qc,
        )
        .expect("wallet terminal verification");
        for validator in &validators {
            let status = validator.status(expected.swap_id);
            assert_eq!(
                status.record.as_ref().map(|record| record.status),
                Some(FastSwapLocalStatusV1::Applied)
            );
            assert_eq!(
                validator
                    .effects(expected.swap_id)
                    .expect("effects")
                    .effects,
                Some(expected.clone())
            );
            for phase in [
                FastSwapPhaseV1::Precommit,
                FastSwapPhaseV1::Commit,
                FastSwapPhaseV1::Effects,
            ] {
                let evidence = validator
                    .vote_evidence(expected.swap_id, phase, 0)
                    .expect("durable vote evidence");
                let vote = evidence.vote.expect("phase vote");
                assert_eq!(vote.phase, phase);
                postfiat_execution::fastswap_decision::verify_fastswap_vote(
                    &fixture.committee,
                    &vote,
                )
                .expect("retrieved vote verifies");
            }
        }
        let mut corrupted = effects_votes.into_iter().take(5).collect::<Vec<_>>();
        corrupted[0].receipt_digest = None;
        assert!(
            aggregate_fastswap_votes(&fixture.committee, corrupted, FastSwapPhaseV1::Effects)
                .is_err()
        );

        drop(validators);
        let mut restarted = FastSwapValidatorServiceV1::from_parts(
            &fixture.root.join("validator-0"),
            fixture.base.clone(),
            fixture.committee.clone(),
            "validator-0".to_owned(),
            fixture.validator_keys[0].clone(),
            110,
        )
        .expect("restart");
        assert_eq!(
            restarted.status(expected.swap_id).record.unwrap().status,
            FastSwapLocalStatusV1::Applied
        );
        let repeated = restarted
            .apply(&decision_qc, &fixture.signed)
            .expect("idempotent repeat");
        assert_eq!(
            repeated.signing_bytes().expect("repeated bytes"),
            effects_qc.votes[0].signing_bytes().expect("original bytes")
        );
        postfiat_execution::fastswap_decision::verify_fastswap_vote(&fixture.committee, &repeated)
            .expect("repeated signature");
        drop(restarted);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn expired_prepare_recovers_to_quorum_cancel_without_unlock_race() {
        let fixture = fixture();
        let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
            .expect("expected")
            .effects;
        let mut validators = (0..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("cancel-validator-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator")
            })
            .collect::<Vec<_>>();
        let prepare_votes = validators
            .iter_mut()
            .map(|validator| validator.prepare(&fixture.signed).expect("prepare"))
            .collect::<Vec<_>>();
        let delayed_round_zero_lock_qc = aggregate_fastswap_votes(
            &fixture.committee,
            prepare_votes,
            FastSwapPhaseV1::Precommit,
        )
        .expect("delayed round-zero LockQC");
        for validator in &mut validators {
            validator.finalized_primary_height = 121;
        }
        let new_round_votes = validators
            .iter_mut()
            .map(|validator| {
                validator
                    .new_round_vote(expected.swap_id, 1)
                    .expect("new-round vote")
            })
            .collect::<Vec<_>>();
        let new_round_qc = aggregate_fastswap_new_round_votes(&fixture.committee, new_round_votes)
            .expect("NewRoundQC");
        let proposal = FastSwapProposalV1 {
            domain: fixture.committee.domain.clone(),
            swap_id: expected.swap_id,
            round: 1,
            decision: FastSwapDecisionV1::Cancel,
            effects_digest: expected.digest().expect("effects digest"),
            leader_id: postfiat_execution::fastswap_decision::recovery_leader(
                &fixture.committee,
                expected.swap_id,
                1,
            )
            .expect("leader")
            .to_owned(),
            new_round_qc: Some(new_round_qc),
            justification: None,
        };
        let precommit_votes = validators
            .iter_mut()
            .map(|validator| validator.precommit_round(&proposal).expect("precommit"))
            .collect::<Vec<_>>();
        let precommit_qc = aggregate_fastswap_votes(
            &fixture.committee,
            precommit_votes,
            FastSwapPhaseV1::Precommit,
        )
        .expect("cancel PrecommitQC");
        let commit_votes = validators
            .iter_mut()
            .map(|validator| validator.commit_round(&precommit_qc).expect("commit"))
            .collect::<Vec<_>>();
        let decision_qc =
            aggregate_fastswap_votes(&fixture.committee, commit_votes, FastSwapPhaseV1::Commit)
                .expect("cancel DecisionQC");
        let cancel_votes = validators
            .iter_mut()
            .map(|validator| validator.cancel_apply(&decision_qc).expect("cancel apply"))
            .collect::<Vec<_>>();
        aggregate_fastswap_votes(
            &fixture.committee,
            cancel_votes,
            FastSwapPhaseV1::CancelApply,
        )
        .expect("CancelApplyQC");
        for validator in &validators {
            assert!(validator.state.reservations.is_empty());
            assert_eq!(validator.state.objects.len(), fixture.base.objects.len());
            assert_eq!(
                validator.status(expected.swap_id).record.unwrap().status,
                FastSwapLocalStatusV1::Cancelled
            );
            assert_eq!(
                validator
                    .state
                    .terminal_tombstones
                    .get(&expected.swap_id)
                    .map(|tombstone| tombstone.decision),
                Some(FastSwapDecisionV1::Cancel)
            );
        }
        for validator in &mut validators {
            assert!(validator.commit(&delayed_round_zero_lock_qc).is_err());
            assert_eq!(
                validator.status(expected.swap_id).record.unwrap().status,
                FastSwapLocalStatusV1::Cancelled
            );
        }
        drop(validators);
        for index in 0..6 {
            let mut restarted = FastSwapValidatorServiceV1::from_parts(
                &fixture.root.join(format!("cancel-validator-{index}")),
                fixture.base.clone(),
                fixture.committee.clone(),
                format!("validator-{index}"),
                fixture.validator_keys[index].clone(),
                121,
            )
            .expect("restart cancelled validator");
            assert_eq!(
                restarted.status(expected.swap_id).record.unwrap().status,
                FastSwapLocalStatusV1::Cancelled
            );
            assert!(restarted.commit(&delayed_round_zero_lock_qc).is_err());
            assert!(restarted.state.reservations.is_empty());
        }
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn replacement_relayer_recovers_expired_partial_prepare_via_rpc_evidence() {
        let fixture = fixture();
        let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
            .expect("expected")
            .effects;
        let validators = (0..6)
            .map(|index| {
                let id = format!("validator-{index}");
                let mut validator = FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("relayer-recovery-{id}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    id.clone(),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator");
                validator.prepare(&fixture.signed).expect("partial prepare");
                validator.finalized_primary_height = 121;
                (id, Mutex::new(validator))
            })
            .collect::<BTreeMap<_, _>>();
        let transport = InMemoryFastSwapTransport {
            validators: Arc::new(validators),
        };
        let mut session = FastSwapWalletSessionV1::new(
            SwapSettlementModeV1::FastSwapV1,
            fixture.signed.clone(),
            expected.clone(),
        )
        .expect("session");
        session.begin_fastswap().expect("begin");
        session
            .mark_unknown("original relayer disappeared")
            .expect("unknown");
        let mut snapshots = Vec::new();
        let outcome =
            recover_fastswap_round(&mut session, &fixture.committee, &transport, 1, |current| {
                snapshots.push(serde_json::to_vec(current).map_err(|error| error.to_string())?);
                Ok(())
            })
            .expect("replacement relayer recovery");
        assert!(matches!(
            outcome,
            FastSwapRecoveryOutcomeV1::Cancelled { .. }
        ));
        assert_eq!(session.state, FastSwapProductStateV1::Cancelled);
        assert!(session.recovery_new_round_qc.is_some());
        assert!(snapshots.len() >= 4);
        assert!(transport.validators.values().all(|validator| {
            let validator = validator.lock().expect("validator");
            validator.state.reservations.is_empty()
                && validator.state.objects.len() == fixture.base.objects.len()
                && validator
                    .state
                    .terminal_tombstones
                    .get(&expected.swap_id)
                    .is_some_and(|row| row.decision == FastSwapDecisionV1::Cancel)
        }));
        drop(transport);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn replacement_relayer_reconciles_accepted_terminal_without_resubmission() {
        let fixture = fixture();
        let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
            .expect("expected")
            .effects;
        let mut validators = (0..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("terminal-reconcile-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator")
            })
            .collect::<Vec<_>>();
        let prepare = validators
            .iter_mut()
            .map(|validator| validator.prepare(&fixture.signed).expect("prepare"))
            .collect::<Vec<_>>();
        let lock_qc =
            aggregate_fastswap_votes(&fixture.committee, prepare, FastSwapPhaseV1::Precommit)
                .expect("lock QC");
        let commit = validators
            .iter_mut()
            .map(|validator| validator.commit(&lock_qc).expect("commit"))
            .collect::<Vec<_>>();
        let decision_qc =
            aggregate_fastswap_votes(&fixture.committee, commit, FastSwapPhaseV1::Commit)
                .expect("decision QC");
        for validator in &mut validators {
            validator
                .apply(&decision_qc, &fixture.signed)
                .expect("apply");
        }
        let transport = InMemoryFastSwapTransport {
            validators: Arc::new(
                validators
                    .into_iter()
                    .enumerate()
                    .map(|(index, validator)| (format!("validator-{index}"), Mutex::new(validator)))
                    .collect(),
            ),
        };
        let evidence_only = EvidenceOnlyFastSwapTransport {
            inner: transport.clone(),
        };
        let mut session = FastSwapWalletSessionV1::new(
            SwapSettlementModeV1::FastSwapV1,
            fixture.signed.clone(),
            expected,
        )
        .expect("session");
        session.begin_fastswap().expect("begin");
        session
            .mark_unknown("lost terminal response")
            .expect("unknown");
        let outcome =
            recover_fastswap_round(&mut session, &fixture.committee, &evidence_only, 1, |_| {
                Ok(())
            })
            .expect("evidence-only reconciliation");
        assert!(matches!(outcome, FastSwapRecoveryOutcomeV1::Accepted(_)));
        assert_eq!(session.state, FastSwapProductStateV1::Accepted);
        assert!(session.lock_qc.is_some());
        assert!(session.decision_qc.is_some());
        assert!(session.effects_qc.is_some());
        drop(evidence_only);
        drop(transport);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn replacement_relayer_preserves_locked_confirm_and_finishes_swap() {
        let fixture = fixture();
        let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
            .expect("expected")
            .effects;
        let mut validators = (0..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("locked-confirm-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator")
            })
            .collect::<Vec<_>>();
        let prepare = validators
            .iter_mut()
            .map(|validator| validator.prepare(&fixture.signed).expect("prepare"))
            .collect::<Vec<_>>();
        let lock_qc =
            aggregate_fastswap_votes(&fixture.committee, prepare, FastSwapPhaseV1::Precommit)
                .expect("lock QC");
        for validator in &mut validators {
            validator.commit(&lock_qc).expect("abandoned commit vote");
        }
        let transport = InMemoryFastSwapTransport {
            validators: Arc::new(
                validators
                    .into_iter()
                    .enumerate()
                    .map(|(index, validator)| (format!("validator-{index}"), Mutex::new(validator)))
                    .collect(),
            ),
        };
        let mut session = FastSwapWalletSessionV1::new(
            SwapSettlementModeV1::FastSwapV1,
            fixture.signed.clone(),
            expected.clone(),
        )
        .expect("session");
        session.begin_fastswap().expect("begin");
        session
            .mark_unknown("broker lost DecisionQC")
            .expect("unknown");
        let outcome =
            recover_fastswap_round(&mut session, &fixture.committee, &transport, 1, |_| Ok(()))
                .expect("locked confirm recovery");
        assert!(matches!(outcome, FastSwapRecoveryOutcomeV1::Accepted(_)));
        assert_eq!(session.state, FastSwapProductStateV1::Accepted);
        assert_eq!(
            session
                .lock_qc
                .as_ref()
                .and_then(|qc| qc.votes.first())
                .and_then(|vote| vote.decision),
            Some(FastSwapDecisionV1::Confirm)
        );
        assert!(transport.validators.values().all(|validator| {
            let validator = validator.lock().expect("validator");
            validator
                .state
                .terminal_tombstones
                .get(&expected.swap_id)
                .is_some_and(|row| row.decision == FastSwapDecisionV1::Confirm)
        }));
        drop(transport);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn six_validator_exit_is_durable_before_vote_and_restart_idempotent() {
        let fixture = fixture();
        let offered = &fixture.signed.intent.party_0;
        let intent = FastLaneExitIntentV1 {
            committee: fixture.committee.domain.clone(),
            owner_address: offered.owner_address.clone(),
            owner_pubkey: offered.owner_pubkey.clone(),
            inputs: offered.asset_inputs.clone(),
            asset_id: offered.offered_asset_id,
            asset_rule_hash: offered.offered_asset_rule_hash,
            amount_atoms: 10,
            destination_address: "primary-destination".to_owned(),
            nonce: [31; 32],
        };
        let signed = SignedFastLaneExitIntentV1 {
            signature: ml_dsa_65_sign_with_context(
                &fixture.owner_0_private_key,
                &intent.canonical_bytes().expect("exit bytes"),
                FASTLANE_EXIT_CONTEXT_V1,
            )
            .expect("exit sign"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            intent,
        };
        let expected = validate_fastlane_exit(&fixture.base, &signed).expect("exit effects");
        let mut validators = (0..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("exit-validator-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator")
            })
            .collect::<Vec<_>>();
        let votes = validators
            .iter_mut()
            .map(|validator| validator.exit(&signed).expect("exit vote"))
            .collect::<Vec<_>>();
        let certificate =
            aggregate_fastlane_exit_votes(&fixture.committee, &expected, votes.clone())
                .expect("exit QC");
        postfiat_execution::fastswap_bridge::verify_fastlane_exit_certificate(
            &fixture.committee,
            &certificate,
        )
        .expect("certificate");
        for validator in &validators {
            assert!(expected
                .consumed
                .iter()
                .all(|key| !validator.state.objects.contains_key(key)));
            assert_eq!(
                validator
                    .state
                    .exit_claims
                    .get(&expected.claim.exit_claim_id),
                Some(&expected.claim)
            );
        }
        drop(validators);
        let mut restarted = FastSwapValidatorServiceV1::from_parts(
            &fixture.root.join("exit-validator-0"),
            fixture.base.clone(),
            fixture.committee.clone(),
            "validator-0".to_owned(),
            fixture.validator_keys[0].clone(),
            110,
        )
        .expect("restart");
        let repeated = restarted.exit(&signed).expect("idempotent exit");
        assert_eq!(
            repeated.signing_bytes().expect("repeated bytes"),
            votes[0].signing_bytes().expect("original bytes")
        );
        drop(restarted);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn byzantine_vote_is_ignored_and_below_quorum_cannot_advance() {
        let fixture = fixture();
        let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
            .expect("expected effects")
            .effects;
        let mut validators = (0..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("fault-validator-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator")
            })
            .collect::<Vec<_>>();
        let mut votes = validators
            .iter_mut()
            .map(|validator| validator.prepare(&fixture.signed).expect("prepare"))
            .collect::<Vec<_>>();

        assert!(aggregate_fastswap_votes(
            &fixture.committee,
            votes.iter().take(4).cloned(),
            FastSwapPhaseV1::Precommit,
        )
        .is_err());
        for validator in &validators {
            let status = validator.status(expected.swap_id);
            assert_eq!(
                status.record.as_ref().map(|record| record.status),
                Some(FastSwapLocalStatusV1::Prepared)
            );
            assert!(validator
                .effects(expected.swap_id)
                .expect("effects")
                .effects
                .is_none());
            assert_eq!(validator.state.objects, fixture.base.objects);
        }

        votes[0].effects_digest.0[0] ^= 1;
        votes[0].signature = ml_dsa_65_sign_with_context(
            &fixture.validator_keys[0],
            &votes[0].signing_bytes().expect("Byzantine vote bytes"),
            FASTSWAP_VOTE_CONTEXT_V1,
        )
        .expect("Byzantine conflicting vote signature");
        let lock_qc =
            aggregate_fastswap_votes(&fixture.committee, votes, FastSwapPhaseV1::Precommit)
                .expect("five honest votes form quorum despite one Byzantine vote");
        assert_eq!(lock_qc.votes.len(), 5);
        assert!(lock_qc
            .votes
            .iter()
            .all(|vote| vote.validator_id != "validator-0"));
        drop(validators);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn exit_and_swap_reservation_are_mutually_exclusive() {
        let fixture = fixture();
        let offered = &fixture.signed.intent.party_0;
        let exit_intent = FastLaneExitIntentV1 {
            committee: fixture.committee.domain.clone(),
            owner_address: offered.owner_address.clone(),
            owner_pubkey: offered.owner_pubkey.clone(),
            inputs: offered.asset_inputs.clone(),
            asset_id: offered.offered_asset_id,
            asset_rule_hash: offered.offered_asset_rule_hash,
            amount_atoms: 10,
            destination_address: "primary-destination".to_owned(),
            nonce: [41; 32],
        };
        let signed_exit = SignedFastLaneExitIntentV1 {
            signature: ml_dsa_65_sign_with_context(
                &fixture.owner_0_private_key,
                &exit_intent.canonical_bytes().expect("exit bytes"),
                FASTLANE_EXIT_CONTEXT_V1,
            )
            .expect("exit signature"),
            algorithm_id: FASTSWAP_ML_DSA_65.to_owned(),
            intent: exit_intent,
        };

        let mut swap_first = FastSwapValidatorServiceV1::from_parts(
            &fixture.root.join("race-swap-first"),
            fixture.base.clone(),
            fixture.committee.clone(),
            "validator-0".to_owned(),
            fixture.validator_keys[0].clone(),
            110,
        )
        .expect("swap-first validator");
        swap_first.prepare(&fixture.signed).expect("prepare");
        let after_prepare = swap_first.state.clone();
        assert!(swap_first.exit(&signed_exit).is_err());
        assert_eq!(swap_first.state, after_prepare);

        let mut exit_first = FastSwapValidatorServiceV1::from_parts(
            &fixture.root.join("race-exit-first"),
            fixture.base.clone(),
            fixture.committee.clone(),
            "validator-0".to_owned(),
            fixture.validator_keys[0].clone(),
            110,
        )
        .expect("exit-first validator");
        exit_first.exit(&signed_exit).expect("exit");
        let after_exit = exit_first.state.clone();
        assert!(exit_first.prepare(&fixture.signed).is_err());
        assert_eq!(exit_first.state, after_exit);
        drop(swap_first);
        drop(exit_first);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn restart_after_each_signed_wave_replays_before_next_vote() {
        let fixture = fixture();
        let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
            .expect("expected effects")
            .effects;
        let validator_0_dir = fixture.root.join("restart-wave-validator-0");
        let open_validator_0 = || {
            FastSwapValidatorServiceV1::from_parts(
                &validator_0_dir,
                fixture.base.clone(),
                fixture.committee.clone(),
                "validator-0".to_owned(),
                fixture.validator_keys[0].clone(),
                110,
            )
            .expect("validator 0")
        };
        let mut validator_0 = open_validator_0();
        let mut peers = (1..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("restart-wave-validator-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("peer")
            })
            .collect::<Vec<_>>();

        let mut prepare_votes = vec![validator_0.prepare(&fixture.signed).expect("prepare")];
        prepare_votes.extend(
            peers
                .iter_mut()
                .map(|validator| validator.prepare(&fixture.signed).expect("peer prepare")),
        );
        let lock_qc = aggregate_fastswap_votes(
            &fixture.committee,
            prepare_votes,
            FastSwapPhaseV1::Precommit,
        )
        .expect("LockQC");
        drop(validator_0);
        validator_0 = open_validator_0();
        assert_eq!(
            validator_0.status(expected.swap_id).record.unwrap().status,
            FastSwapLocalStatusV1::Prepared
        );

        let mut commit_votes = vec![validator_0.commit(&lock_qc).expect("commit")];
        commit_votes.extend(
            peers
                .iter_mut()
                .map(|validator| validator.commit(&lock_qc).expect("peer commit")),
        );
        let decision_qc =
            aggregate_fastswap_votes(&fixture.committee, commit_votes, FastSwapPhaseV1::Commit)
                .expect("DecisionQC");
        drop(validator_0);
        validator_0 = open_validator_0();
        assert_eq!(
            validator_0.status(expected.swap_id).record.unwrap().status,
            FastSwapLocalStatusV1::DecisionLocked
        );

        let first_effects_vote = validator_0
            .apply(&decision_qc, &fixture.signed)
            .expect("apply");
        drop(validator_0);
        validator_0 = open_validator_0();
        assert_eq!(
            validator_0.status(expected.swap_id).record.unwrap().status,
            FastSwapLocalStatusV1::Applied
        );
        assert_eq!(
            validator_0
                .effects(expected.swap_id)
                .expect("effects")
                .effects,
            Some(expected)
        );
        let repeated_effects_vote = validator_0
            .apply(&decision_qc, &fixture.signed)
            .expect("repeat apply");
        assert_eq!(
            first_effects_vote.signing_bytes().expect("first vote"),
            repeated_effects_vote
                .signing_bytes()
                .expect("repeated vote")
        );
        drop(validator_0);
        drop(peers);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    fn run_persistent_wallet_driver_once() -> FastSwapStageTimingsV1 {
        let fixture = fixture();
        let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
            .expect("expected effects")
            .effects;
        let validators = (0..6)
            .map(|index| {
                let id = format!("validator-{index}");
                let validator = FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("driver-{id}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    id.clone(),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator");
                (id, validator)
            })
            .collect::<BTreeMap<_, _>>();
        let transport = InMemoryFastSwapTransport {
            validators: Arc::new(
                validators
                    .into_iter()
                    .map(|(id, validator)| (id, Mutex::new(validator)))
                    .collect(),
            ),
        };
        let preview = preview_fastswap(&fixture.signed, &fixture.committee, &transport)
            .expect("quorum preview");
        assert_eq!(preview, expected);
        assert!(transport.validators.values().all(|validator| {
            let validator = validator.lock().expect("validator");
            validator.state.objects == fixture.base.objects
                && validator.state.reservations.is_empty()
                && validator.state.swaps.is_empty()
        }));
        let mut session = FastSwapWalletSessionV1::new(
            SwapSettlementModeV1::FastSwapV1,
            fixture.signed.clone(),
            expected.clone(),
        )
        .expect("session");
        let mut durable_snapshots = Vec::new();
        let terminal =
            drive_fastswap_three_wave(&mut session, &fixture.committee, &transport, |current| {
                durable_snapshots.push(current.to_durable_json().map_err(|e| format!("{e:?}"))?);
                Ok(())
            })
            .expect("three-wave driver");
        assert_eq!(terminal.effects, expected);
        assert_eq!(session.state, FastSwapProductStateV1::Accepted);
        assert_eq!(session.replication_pending.len(), 1);
        let timings = session.last_timings.clone().expect("stage timings");
        assert!(timings.prepare_qc_ms > 0);
        assert!(timings.decision_qc_ms > 0);
        assert!(timings.effects_qc_ms > 0);
        assert_eq!(durable_snapshots.len(), 4);
        assert_eq!(
            durable_snapshots
                .iter()
                .map(|snapshot| {
                    FastSwapWalletSessionV1::from_durable_json(snapshot)
                        .expect("durable snapshot")
                        .state
                })
                .collect::<Vec<_>>(),
            vec![
                FastSwapProductStateV1::Preparing,
                FastSwapProductStateV1::Locked,
                FastSwapProductStateV1::Applying,
                FastSwapProductStateV1::Accepted,
            ]
        );
        let replication = reconcile_fastswap_replication(
            &mut session,
            &fixture.committee,
            &transport,
            |_| Ok(()),
        )
        .expect("background exact-six replication");
        assert_eq!(replication.applied.len(), 1);
        assert!(replication.failed.is_empty());
        assert!(replication.pending.is_empty());
        assert!(transport.validators.values().all(|validator| {
            validator
                .lock()
                .expect("validator")
                .status(session.expected_effects.swap_id)
                .record
                .is_some_and(|record| record.status == FastSwapLocalStatusV1::Applied)
        }));
        drop(transport);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
        timings
    }

    #[test]
    fn persistent_wallet_driver_executes_real_three_wave_rpc_contract() {
        let timings = run_persistent_wallet_driver_once();
        eprintln!("FastSwap in-process warm timings: {timings:?}");
    }

    #[test]
    #[ignore = "isolated in-process warm-latency gate; CI executes it after workspace tests"]
    fn persistent_wallet_driver_meets_isolated_warm_latency_gate() {
        let timings = run_persistent_wallet_driver_once();
        assert!(
            timings.total_ms <= 5_000,
            "isolated FastSwap in-process warm latency exceeded 5s: {timings:?}"
        );
    }

    #[test]
    fn quorum_early_path_ignores_a_gated_sixth_response() {
        let fixture = fixture();
        let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
            .expect("expected effects")
            .effects;
        let validators = (0..6)
            .map(|index| {
                let id = format!("validator-{index}");
                let validator = FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("slow-{id}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    id.clone(),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator");
                (id, validator)
            })
            .collect::<BTreeMap<_, _>>();
        let inner = InMemoryFastSwapTransport {
            validators: Arc::new(
                validators
                    .into_iter()
                    .map(|(id, validator)| (id, Mutex::new(validator)))
                    .collect(),
            ),
        };
        let gate = Arc::new((Mutex::new(ResponseGateState::default()), Condvar::new()));
        let transport = GatedResponseFastSwapTransport {
            inner: inner.clone(),
            validator_id: "validator-5".to_owned(),
            gate: Arc::clone(&gate),
        };
        let preview = preview_fastswap(&fixture.signed, &fixture.committee, &inner)
            .expect("quorum-early preview");
        assert_eq!(preview, expected);
        let mut session = FastSwapWalletSessionV1::new(
            SwapSettlementModeV1::FastSwapV1,
            fixture.signed.clone(),
            expected.clone(),
        )
        .expect("session");
        let terminal =
            drive_fastswap_three_wave(&mut session, &fixture.committee, &transport, |_| Ok(()))
                .expect("quorum-early settlement");
        assert_eq!(terminal.effects, expected);
        assert_eq!(
            session.replication_pending,
            BTreeSet::from(["validator-5".to_owned()])
        );
        let (state_lock, state_changed) = &*gate;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut state = state_lock.lock().expect("gated response state");
        while state.blocked < 3 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "all three sixth-validator responses must reach the gate"
            );
            let (next, timeout) = state_changed
                .wait_timeout(state, remaining)
                .expect("wait for gated sixth-validator responses");
            state = next;
            assert!(!timeout.timed_out() || state.blocked == 3);
        }
        assert_eq!(
            state.blocked, 3,
            "prepare, commit, and apply responses stay gated"
        );
        state.released = true;
        state_changed.notify_all();
        while state.blocked != 0 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            assert!(
                !remaining.is_zero(),
                "gated response workers must terminate"
            );
            let (next, timeout) = state_changed
                .wait_timeout(state, remaining)
                .expect("wait for gated response workers");
            state = next;
            assert!(!timeout.timed_out() || state.blocked == 0);
        }
        drop(state);
        assert!(inner.validators.values().all(|validator| {
            validator
                .lock()
                .expect("validator")
                .status(expected.swap_id)
                .record
                .is_some_and(|record| record.status == FastSwapLocalStatusV1::Applied)
        }));
        drop(transport);
        drop(inner);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }

    #[test]
    fn below_quorum_partitions_cannot_settle_and_heal_idempotently() {
        for reachable_count in [3_usize, 4] {
            let fixture = fixture();
            let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
                .expect("expected effects")
                .effects;
            let validators = (0..6)
                .map(|index| {
                    let id = format!("validator-{index}");
                    let validator = FastSwapValidatorServiceV1::from_parts(
                        &fixture
                            .root
                            .join(format!("partition-{reachable_count}-{id}")),
                        fixture.base.clone(),
                        fixture.committee.clone(),
                        id.clone(),
                        fixture.validator_keys[index].clone(),
                        110,
                    )
                    .expect("validator");
                    (id, validator)
                })
                .collect::<BTreeMap<_, _>>();
            let transport = InMemoryFastSwapTransport {
                validators: Arc::new(
                    validators
                        .into_iter()
                        .map(|(id, validator)| (id, Mutex::new(validator)))
                        .collect(),
                ),
            };
            let partitioned = PartitionedFastSwapTransport {
                inner: transport.clone(),
                reachable: fixture
                    .committee
                    .validators
                    .iter()
                    .take(reachable_count)
                    .map(|validator| validator.validator_id.clone())
                    .collect(),
            };
            let mut session = FastSwapWalletSessionV1::new(
                SwapSettlementModeV1::FastSwapV1,
                fixture.signed.clone(),
                expected.clone(),
            )
            .expect("session");
            let result =
                drive_fastswap_three_wave(&mut session, &fixture.committee, &partitioned, |_| {
                    Ok(())
                });
            assert!(matches!(
                result,
                Err(FastSwapClientError::QuorumUnavailable {
                    stage: "prepare",
                    ..
                })
            ));
            assert_eq!(session.state, FastSwapProductStateV1::Unknown);
            assert!(transport.validators.values().all(|validator| {
                let validator = validator.lock().expect("validator");
                validator.state.objects == fixture.base.objects
                    && validator.state.terminal_tombstones.is_empty()
            }));

            let terminal =
                drive_fastswap_three_wave(&mut session, &fixture.committee, &transport, |_| Ok(()))
                    .expect("partition heal");
            assert_eq!(terminal.effects, expected);
            assert_eq!(session.state, FastSwapProductStateV1::Accepted);
            reconcile_fastswap_replication(
                &mut session,
                &fixture.committee,
                &transport,
                |_| Ok(()),
            )
            .expect("exact-six catch-up");
            assert!(transport.validators.values().all(|validator| {
                validator
                    .lock()
                    .expect("validator")
                    .status(expected.swap_id)
                    .record
                    .is_some_and(|record| record.status == FastSwapLocalStatusV1::Applied)
            }));
            drop(partitioned);
            drop(transport);
            fs::remove_dir_all(&fixture.root).expect("cleanup");
        }
    }

    #[test]
    fn lost_responses_after_every_mutating_wave_resume_idempotently() {
        for lost_method in [
            METHOD_FASTSWAP_PREPARE,
            METHOD_FASTSWAP_COMMIT,
            METHOD_FASTSWAP_APPLY,
        ] {
            let fixture = fixture();
            let expected = validate_fastswap_admission(&fixture.base, &fixture.signed, 110)
                .expect("expected effects")
                .effects;
            let validators = (0..6)
                .map(|index| {
                    let id = format!("validator-{index}");
                    let validator = FastSwapValidatorServiceV1::from_parts(
                        &fixture
                            .root
                            .join(format!("lost-response-{lost_method}-{id}")),
                        fixture.base.clone(),
                        fixture.committee.clone(),
                        id.clone(),
                        fixture.validator_keys[index].clone(),
                        110,
                    )
                    .expect("validator");
                    (id, validator)
                })
                .collect::<BTreeMap<_, _>>();
            let transport = InMemoryFastSwapTransport {
                validators: Arc::new(
                    validators
                        .into_iter()
                        .map(|(id, validator)| (id, Mutex::new(validator)))
                        .collect(),
                ),
            };
            let lossy = LostResponseFastSwapTransport {
                inner: transport.clone(),
                method: lost_method,
                validators: BTreeSet::from(["validator-0".to_owned(), "validator-1".to_owned()]),
            };
            let mut session = FastSwapWalletSessionV1::new(
                SwapSettlementModeV1::FastSwapV1,
                fixture.signed.clone(),
                expected.clone(),
            )
            .expect("session");
            assert!(matches!(
                drive_fastswap_three_wave(&mut session, &fixture.committee, &lossy, |_| Ok(())),
                Err(FastSwapClientError::QuorumUnavailable { .. })
            ));
            assert_eq!(session.state, FastSwapProductStateV1::Unknown);
            let terminal =
                drive_fastswap_three_wave(&mut session, &fixture.committee, &transport, |_| Ok(()))
                    .expect("idempotent resume after lost response");
            assert_eq!(terminal.effects, expected);
            assert_eq!(session.state, FastSwapProductStateV1::Accepted);
            drop(lossy);
            drop(transport);
            fs::remove_dir_all(&fixture.root).expect("cleanup");
        }
    }

    #[test]
    #[ignore = "explicit 100-operation performance gate"]
    fn hundred_warm_wallet_operations_meet_local_latency_gate() {
        let mut totals = (0..100)
            .map(|_| run_persistent_wallet_driver_once().total_ms)
            .collect::<Vec<_>>();
        totals.sort_unstable();
        let p50 = totals[49];
        let p95 = totals[94];
        let p99 = totals[98];
        eprintln!("FastSwap 100-op local warm distribution: p50={p50}ms p95={p95}ms p99={p99}ms");
        assert!(p50 <= 2_000, "warm p50 gate failed: {p50}ms");
        assert!(p95 <= 3_000, "warm p95 gate failed: {p95}ms");
        assert!(p99 <= 5_000, "warm p99 gate failed: {p99}ms");
    }

    #[test]
    fn certified_confirm_catches_up_and_supersedes_a_partial_local_lock() {
        let fixture = fixture();
        let winner = fixture.signed.clone();
        let winner_id = winner.swap_id().expect("winner id");
        let mut loser = winner.clone();
        loser.intent.nonce = [99; 32];
        let loser_bytes = loser.intent.canonical_bytes().expect("loser bytes");
        loser.authorization_0.signature = ml_dsa_65_sign_with_context(
            &fixture.owner_0_private_key,
            &loser_bytes,
            FASTSWAP_INTENT_CONTEXT_V1,
        )
        .expect("loser owner 0 signature");
        loser.authorization_1.signature = ml_dsa_65_sign_with_context(
            &fixture.owner_1_private_key,
            &loser_bytes,
            FASTSWAP_INTENT_CONTEXT_V1,
        )
        .expect("loser owner 1 signature");
        let loser_id = loser.swap_id().expect("loser id");

        let mut validators = (0..6)
            .map(|index| {
                FastSwapValidatorServiceV1::from_parts(
                    &fixture.root.join(format!("catch-up-validator-{index}")),
                    fixture.base.clone(),
                    fixture.committee.clone(),
                    format!("validator-{index}"),
                    fixture.validator_keys[index].clone(),
                    110,
                )
                .expect("validator")
            })
            .collect::<Vec<_>>();
        validators[5].prepare(&loser).expect("partial losing lock");
        let lock_qc = aggregate_fastswap_votes(
            &fixture.committee,
            validators[..5]
                .iter_mut()
                .map(|validator| validator.prepare(&winner).expect("winner prepare")),
            FastSwapPhaseV1::Precommit,
        )
        .expect("winner LockQC");
        let decision_qc = aggregate_fastswap_votes(
            &fixture.committee,
            validators[..5]
                .iter_mut()
                .map(|validator| validator.commit(&lock_qc).expect("winner commit")),
            FastSwapPhaseV1::Commit,
        )
        .expect("winner DecisionQC");
        validators[5].finalized_primary_height = 121;
        let effects_vote = validators[5]
            .catch_up_confirm(&lock_qc, &decision_qc, &winner)
            .expect("certified catch-up");
        postfiat_execution::fastswap_decision::verify_fastswap_vote(
            &fixture.committee,
            &effects_vote,
        )
        .expect("catch-up effects vote");
        assert_eq!(effects_vote.phase, FastSwapPhaseV1::Effects);
        assert_eq!(
            validators[5].status(winner_id).record.unwrap().status,
            FastSwapLocalStatusV1::Applied
        );
        assert_eq!(
            validators[5].status(loser_id).record.unwrap().status,
            FastSwapLocalStatusV1::Superseded
        );
        assert!(validators[5]
            .state
            .reservations
            .values()
            .all(|reservation| reservation.swap_id != loser_id));
        drop(validators);
        let restarted = FastSwapValidatorServiceV1::from_parts(
            &fixture.root.join("catch-up-validator-5"),
            fixture.base.clone(),
            fixture.committee.clone(),
            "validator-5".to_owned(),
            fixture.validator_keys[5].clone(),
            121,
        )
        .expect("restart caught-up validator");
        assert_eq!(
            restarted.status(winner_id).record.unwrap().status,
            FastSwapLocalStatusV1::Applied
        );
        assert_eq!(
            restarted.status(loser_id).record.unwrap().status,
            FastSwapLocalStatusV1::Superseded
        );
        drop(restarted);
        fs::remove_dir_all(&fixture.root).expect("cleanup");
    }
}
