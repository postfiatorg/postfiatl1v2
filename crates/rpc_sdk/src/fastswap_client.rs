use crate::{
    aggregate_fastswap_new_round_votes, aggregate_fastswap_votes,
    fastlane_asset_control_apply_request, fastlane_asset_control_catch_up_request,
    fastlane_asset_control_prepare_request, fastlane_asset_control_preview_request,
    fastswap_apply_request, fastswap_cancel_apply_request, fastswap_catch_up_request,
    fastswap_commit_request, fastswap_commit_round_request, fastswap_new_round_vote_request,
    fastswap_precommit_request, fastswap_prepare_request, fastswap_preview_request,
    fastswap_votes_request, verify_fast_asset_control_terminal, verify_fastswap_terminal,
    FastSwapProductStateV1, FastSwapSessionError, FastSwapWalletSessionV1, RpcRequest, RpcResponse,
    VerifiedFastSwapTerminalV1,
};
use postfiat_execution::fastswap_decision::{
    recovery_leader, verify_fastswap_certificate, verify_fastswap_new_round_certificate,
    verify_fastswap_new_round_vote, verify_fastswap_vote,
};
use postfiat_types::{
    FastSwapCertificateV1, FastSwapCommitteeV1, FastSwapDecisionV1, FastSwapEffectsDigestV1,
    FastSwapEffectsV1, FastSwapNewRoundVoteV1, FastSwapPhaseV1, FastSwapPreviewResponseV1,
    FastSwapProposalV1, FastSwapVoteEvidenceResponseV1, FastSwapVoteV1,
    SignedFastAssetControlCommandV1, SignedFastSwapIntentV1,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc;
use std::time::Instant;

/// Transport boundary for a persistent wallet process. Implementations resolve
/// the committee validator ID to an authenticated endpoint and keep any TCP/TLS
/// session pooling outside the protocol state machine.
pub trait FastSwapRpcTransportV1: Clone + Send + Sync + 'static {
    fn call(&self, validator_id: &str, request: &RpcRequest) -> Result<RpcResponse, String>;

    fn encode_fastswap_payload(&self, json: &str) -> Result<String, String> {
        Ok(json.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastSwapClientError {
    Session(FastSwapSessionError),
    Serialization,
    Persistence(String),
    RecoveryInvariant(&'static str),
    QuorumUnavailable {
        stage: &'static str,
        response_errors: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastSwapRecoveryOutcomeV1 {
    Accepted(Box<VerifiedFastSwapTerminalV1>),
    Cancelled {
        cancel_apply_qc: FastSwapCertificateV1,
    },
}

/// Ask every validator to execute admission without reserving inputs and return
/// only after distinct-validator quorum agrees on byte-identical effects. The
/// later prepare wave repeats admission against live state, so preview is not a
/// lock and cannot authorize settlement by itself.
pub fn preview_fastswap<T: FastSwapRpcTransportV1>(
    signed: &SignedFastSwapIntentV1,
    committee: &FastSwapCommitteeV1,
    transport: &T,
) -> Result<FastSwapEffectsV1, FastSwapClientError> {
    let signed_json = transport
        .encode_fastswap_payload(
            &serde_json::to_string(signed).map_err(|_| FastSwapClientError::Serialization)?,
        )
        .map_err(|_| FastSwapClientError::Serialization)?;
    let expected_swap_id = signed
        .swap_id()
        .map_err(|_| FastSwapClientError::Serialization)?;
    let committee_ids = committee
        .validators
        .iter()
        .map(|validator| validator.validator_id.as_str())
        .collect::<BTreeSet<_>>();
    let required = usize::from(committee.domain.quorum);
    let (sender, receiver) = mpsc::channel();
    for validator in &committee.validators {
        let validator_id = validator.validator_id.clone();
        let request = fastswap_preview_request(
            format!("fastswap-preview-{validator_id}"),
            signed_json.clone(),
        );
        let transport = transport.clone();
        let sender = sender.clone();
        std::thread::spawn(move || {
            let result = transport
                .call(&validator_id, &request)
                .and_then(|response| {
                    response
                        .result_as::<FastSwapPreviewResponseV1>()
                        .map_err(|error| format!("{error:?}"))
                });
            let _ = sender.send((validator_id, result));
        });
    }
    drop(sender);
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    let mut groups = BTreeMap::<FastSwapEffectsDigestV1, (FastSwapEffectsV1, usize)>::new();
    while let Ok((endpoint_id, result)) = receiver.recv() {
        match result {
            Ok(preview)
                if preview.schema == "postfiat-fastswap-preview-v1"
                    && preview.validator_id == endpoint_id
                    && committee_ids.contains(preview.validator_id.as_str())
                    && seen.insert(preview.validator_id.clone())
                    && preview.committee == committee.domain
                    && preview.effects.swap_id == expected_swap_id
                    && preview.effects.decision == postfiat_types::FastSwapDecisionV1::Confirm
                    && preview.effects.receipt.accepted
                    && preview.effects.receipt.code == "fastswap_applied" =>
            {
                let digest = preview
                    .effects
                    .digest()
                    .map_err(|_| FastSwapClientError::Serialization)?;
                let group = groups
                    .entry(digest)
                    .or_insert_with(|| (preview.effects.clone(), 0));
                if group.0 != preview.effects {
                    errors.push(format!(
                        "{endpoint_id}: digest collision or non-canonical effects"
                    ));
                    continue;
                }
                group.1 += 1;
                if group.1 >= required {
                    return Ok(group.0.clone());
                }
            }
            Ok(_) => errors.push(format!("{endpoint_id}: invalid preview response")),
            Err(error) => errors.push(format!("{endpoint_id}: {error}")),
        }
    }
    Err(FastSwapClientError::QuorumUnavailable {
        stage: "preview",
        response_errors: errors,
    })
}

pub fn preview_fast_asset_control<T: FastSwapRpcTransportV1>(
    signed: &SignedFastAssetControlCommandV1,
    committee: &FastSwapCommitteeV1,
    transport: &T,
) -> Result<FastSwapEffectsV1, FastSwapClientError> {
    let signed_json =
        serde_json::to_string(signed).map_err(|_| FastSwapClientError::Serialization)?;
    let expected_operation_id = signed
        .operation_id()
        .map_err(|_| FastSwapClientError::Serialization)?;
    let committee_ids = committee
        .validators
        .iter()
        .map(|validator| validator.validator_id.as_str())
        .collect::<BTreeSet<_>>();
    let required = usize::from(committee.domain.quorum);
    let (sender, receiver) = mpsc::channel();
    for validator in &committee.validators {
        let validator_id = validator.validator_id.clone();
        let request = fastlane_asset_control_preview_request(
            format!("fastlane-asset-control-preview-{validator_id}"),
            signed_json.clone(),
        );
        let transport = transport.clone();
        let sender = sender.clone();
        std::thread::spawn(move || {
            let result = transport
                .call(&validator_id, &request)
                .and_then(|response| {
                    response
                        .result_as::<FastSwapPreviewResponseV1>()
                        .map_err(|error| format!("{error:?}"))
                });
            let _ = sender.send((validator_id, result));
        });
    }
    drop(sender);
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    let mut groups = BTreeMap::<FastSwapEffectsDigestV1, (FastSwapEffectsV1, usize)>::new();
    while let Ok((endpoint_id, result)) = receiver.recv() {
        match result {
            Ok(preview)
                if preview.schema == "postfiat-fastlane-asset-control-preview-v1"
                    && preview.validator_id == endpoint_id
                    && committee_ids.contains(preview.validator_id.as_str())
                    && seen.insert(preview.validator_id.clone())
                    && preview.committee == committee.domain
                    && preview.effects.swap_id == expected_operation_id
                    && preview.effects.policy_hash
                        == postfiat_types::FastSwapPolicyHashV1::ZERO
                    && preview.effects.decision == postfiat_types::FastSwapDecisionV1::Confirm
                    && preview.effects.receipt.accepted
                    && preview.effects.receipt.code == "fastlane_asset_control_applied" =>
            {
                let digest = preview
                    .effects
                    .digest()
                    .map_err(|_| FastSwapClientError::Serialization)?;
                let group = groups
                    .entry(digest)
                    .or_insert_with(|| (preview.effects.clone(), 0));
                if group.0 != preview.effects {
                    errors.push(format!(
                        "{endpoint_id}: digest collision or non-canonical effects"
                    ));
                    continue;
                }
                group.1 += 1;
                if group.1 >= required {
                    return Ok(group.0.clone());
                }
            }
            Ok(_) => errors.push(format!("{endpoint_id}: invalid asset control preview")),
            Err(error) => errors.push(format!("{endpoint_id}: {error}")),
        }
    }
    Err(FastSwapClientError::QuorumUnavailable {
        stage: "asset_control_preview",
        response_errors: errors,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapReplicationReportV1 {
    pub applied: Vec<String>,
    pub failed: Vec<(String, String)>,
    pub pending: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastAssetControlWalletProgressV1 {
    pub schema: String,
    pub signed_command: SignedFastAssetControlCommandV1,
    pub expected_effects: FastSwapEffectsV1,
    pub lock_qc: Option<postfiat_types::FastSwapCertificateV1>,
    pub decision_qc: Option<postfiat_types::FastSwapCertificateV1>,
    pub effects_qc: Option<postfiat_types::FastSwapCertificateV1>,
    pub replication_pending: BTreeSet<String>,
}

impl FastAssetControlWalletProgressV1 {
    pub fn new(
        signed_command: SignedFastAssetControlCommandV1,
        expected_effects: FastSwapEffectsV1,
    ) -> Result<Self, FastSwapClientError> {
        let operation_id = signed_command
            .operation_id()
            .map_err(|_| FastSwapClientError::Serialization)?;
        if expected_effects.swap_id != operation_id
            || expected_effects.policy_hash != postfiat_types::FastSwapPolicyHashV1::ZERO
            || expected_effects.decision != postfiat_types::FastSwapDecisionV1::Confirm
            || !expected_effects.receipt.accepted
            || expected_effects.receipt.code != "fastlane_asset_control_applied"
        {
            return Err(FastSwapClientError::Serialization);
        }
        Ok(Self {
            schema: "postfiat-fastlane-asset-control-wallet-progress-v1".to_owned(),
            signed_command,
            expected_effects,
            lock_qc: None,
            decision_qc: None,
            effects_qc: None,
            replication_pending: BTreeSet::new(),
        })
    }
}

impl From<FastSwapSessionError> for FastSwapClientError {
    fn from(value: FastSwapSessionError) -> Self {
        Self::Session(value)
    }
}

/// Drive the three FastSwap certificate waves. Every transition is persisted
/// before the next network wave. Therefore a crash after a validator response
/// can only cause an idempotent replay of that wave, never a contradictory vote.
pub fn drive_fastswap_three_wave<T, P>(
    session: &mut FastSwapWalletSessionV1,
    committee: &FastSwapCommitteeV1,
    transport: &T,
    mut persist: P,
) -> Result<VerifiedFastSwapTerminalV1, FastSwapClientError>
where
    T: FastSwapRpcTransportV1,
    P: FnMut(&FastSwapWalletSessionV1) -> Result<(), String>,
{
    let total_started = Instant::now();
    let mut timings = session.last_timings.clone().unwrap_or_default();
    if session.state == FastSwapProductStateV1::Draft {
        session.begin_fastswap()?;
        persist_session(session, &mut persist)?;
    } else if session.state == FastSwapProductStateV1::Unknown {
        session.resume_unknown(committee)?;
        persist_session(session, &mut persist)?;
    }

    loop {
        match session.state {
            FastSwapProductStateV1::Preparing => {
                let stage_started = Instant::now();
                let signed = transport
                    .encode_fastswap_payload(
                        &serde_json::to_string(&session.signed_intent)
                            .map_err(|_| FastSwapClientError::Serialization)?,
                    )
                    .map_err(|_| FastSwapClientError::Serialization)?;
                let (votes, errors) = collect_votes(
                    committee,
                    "prepare",
                    FastSwapPhaseV1::Precommit,
                    transport,
                    |id| fastswap_prepare_request(id, signed.clone()),
                );
                if let Err(error) = session.accept_prepare_votes(committee, votes) {
                    return mark_unknown(session, &mut persist, "prepare", errors, error);
                }
                timings.prepare_qc_ms = elapsed_ms(stage_started);
                session.last_timings = Some(timings.clone());
                persist_session(session, &mut persist)?;
            }
            FastSwapProductStateV1::Locked => {
                let stage_started = Instant::now();
                let lock_qc = transport
                    .encode_fastswap_payload(
                        &serde_json::to_string(
                            session
                                .lock_qc
                                .as_ref()
                                .ok_or(FastSwapSessionError::InvalidState)?,
                        )
                        .map_err(|_| FastSwapClientError::Serialization)?,
                    )
                    .map_err(|_| FastSwapClientError::Serialization)?;
                let (votes, errors) = collect_votes(
                    committee,
                    "commit",
                    FastSwapPhaseV1::Commit,
                    transport,
                    |id| fastswap_commit_request(id, lock_qc.clone()),
                );
                if let Err(error) = session.accept_commit_votes(committee, votes) {
                    return mark_unknown(session, &mut persist, "commit", errors, error);
                }
                timings.decision_qc_ms = elapsed_ms(stage_started);
                session.last_timings = Some(timings.clone());
                persist_session(session, &mut persist)?;
            }
            FastSwapProductStateV1::Applying => {
                let stage_started = Instant::now();
                let decision_qc = transport
                    .encode_fastswap_payload(
                        &serde_json::to_string(
                            session
                                .decision_qc
                                .as_ref()
                                .ok_or(FastSwapSessionError::InvalidState)?,
                        )
                        .map_err(|_| FastSwapClientError::Serialization)?,
                    )
                    .map_err(|_| FastSwapClientError::Serialization)?;
                let signed = transport
                    .encode_fastswap_payload(
                        &serde_json::to_string(&session.signed_intent)
                            .map_err(|_| FastSwapClientError::Serialization)?,
                    )
                    .map_err(|_| FastSwapClientError::Serialization)?;
                let (votes, errors) = collect_votes(
                    committee,
                    "apply",
                    FastSwapPhaseV1::Effects,
                    transport,
                    |id| fastswap_apply_request(id, decision_qc.clone(), signed.clone()),
                );
                timings.effects_qc_ms = elapsed_ms(stage_started);
                let verify_started = Instant::now();
                let terminal = match session.accept_effects_votes(committee, votes) {
                    Ok(terminal) => terminal,
                    Err(error) => {
                        return mark_unknown(session, &mut persist, "apply", errors, error)
                    }
                };
                timings.terminal_verify_ms = elapsed_ms(verify_started);
                timings.total_ms = elapsed_ms(total_started);
                session.last_timings = Some(timings.clone());
                let effects_signers = terminal
                    .effects_qc
                    .votes
                    .iter()
                    .map(|vote| vote.validator_id.as_str())
                    .collect::<std::collections::BTreeSet<_>>();
                session.replication_pending = committee
                    .validators
                    .iter()
                    .filter(|validator| !effects_signers.contains(validator.validator_id.as_str()))
                    .map(|validator| validator.validator_id.clone())
                    .collect();
                persist_session(session, &mut persist)?;
                return Ok(terminal);
            }
            FastSwapProductStateV1::Accepted => {
                return verify_fastswap_terminal(
                    committee,
                    &session.expected_effects,
                    session
                        .lock_qc
                        .as_ref()
                        .ok_or(FastSwapSessionError::InvalidState)?,
                    session
                        .decision_qc
                        .as_ref()
                        .ok_or(FastSwapSessionError::InvalidState)?,
                    session
                        .effects_qc
                        .as_ref()
                        .ok_or(FastSwapSessionError::InvalidState)?,
                )
                .map_err(FastSwapSessionError::from)
                .map_err(FastSwapClientError::from);
            }
            _ => return Err(FastSwapSessionError::InvalidState.into()),
        }
    }
}

pub fn drive_fast_asset_control_three_wave<T, P>(
    progress: &mut FastAssetControlWalletProgressV1,
    committee: &FastSwapCommitteeV1,
    transport: &T,
    mut persist: P,
) -> Result<VerifiedFastSwapTerminalV1, FastSwapClientError>
where
    T: FastSwapRpcTransportV1,
    P: FnMut(&FastAssetControlWalletProgressV1) -> Result<(), String>,
{
    if progress.schema != "postfiat-fastlane-asset-control-wallet-progress-v1" {
        return Err(FastSwapClientError::Serialization);
    }
    let expected_digest = progress
        .expected_effects
        .digest()
        .map_err(|_| FastSwapClientError::Serialization)?;
    if progress.lock_qc.is_none() {
        let signed = serde_json::to_string(&progress.signed_command)
            .map_err(|_| FastSwapClientError::Serialization)?;
        let (votes, errors) = collect_votes(
            committee,
            "asset-control-prepare",
            FastSwapPhaseV1::Precommit,
            transport,
            |id| fastlane_asset_control_prepare_request(id, signed.clone()),
        );
        let lock_qc = aggregate_fastswap_votes(committee, votes, FastSwapPhaseV1::Precommit)
            .map_err(|error| FastSwapClientError::QuorumUnavailable {
                stage: "asset_control_prepare",
                response_errors: errors
                    .into_iter()
                    .chain(std::iter::once(format!("{error:?}")))
                    .collect(),
            })?;
        let verified = verify_fastswap_certificate(committee, &lock_qc)
            .map_err(|_| FastSwapClientError::Serialization)?;
        if verified.effects_digest != expected_digest
            || verified.decision != Some(postfiat_types::FastSwapDecisionV1::Confirm)
        {
            return Err(FastSwapClientError::Serialization);
        }
        progress.lock_qc = Some(lock_qc);
        persist(progress).map_err(FastSwapClientError::Persistence)?;
    }
    if progress.decision_qc.is_none() {
        let lock_qc = progress
            .lock_qc
            .as_ref()
            .ok_or(FastSwapClientError::Serialization)?;
        let lock_json =
            serde_json::to_string(lock_qc).map_err(|_| FastSwapClientError::Serialization)?;
        let lock_digest = lock_qc
            .digest()
            .map_err(|_| FastSwapClientError::Serialization)?;
        let (votes, errors) = collect_votes(
            committee,
            "asset-control-commit",
            FastSwapPhaseV1::Commit,
            transport,
            |id| fastswap_commit_request(id, lock_json.clone()),
        );
        let decision_qc = aggregate_fastswap_votes(committee, votes, FastSwapPhaseV1::Commit)
            .map_err(|error| FastSwapClientError::QuorumUnavailable {
                stage: "asset_control_commit",
                response_errors: errors
                    .into_iter()
                    .chain(std::iter::once(format!("{error:?}")))
                    .collect(),
            })?;
        let verified = verify_fastswap_certificate(committee, &decision_qc)
            .map_err(|_| FastSwapClientError::Serialization)?;
        if verified.effects_digest != expected_digest
            || verified.decision != Some(postfiat_types::FastSwapDecisionV1::Confirm)
            || decision_qc
                .votes
                .iter()
                .any(|vote| vote.justification_digest != Some(lock_digest))
        {
            return Err(FastSwapClientError::Serialization);
        }
        progress.decision_qc = Some(decision_qc);
        persist(progress).map_err(FastSwapClientError::Persistence)?;
    }
    if progress.effects_qc.is_none() {
        let decision_json = serde_json::to_string(
            progress
                .decision_qc
                .as_ref()
                .ok_or(FastSwapClientError::Serialization)?,
        )
        .map_err(|_| FastSwapClientError::Serialization)?;
        let signed = serde_json::to_string(&progress.signed_command)
            .map_err(|_| FastSwapClientError::Serialization)?;
        let (votes, errors) = collect_votes(
            committee,
            "asset-control-apply",
            FastSwapPhaseV1::Effects,
            transport,
            |id| fastlane_asset_control_apply_request(id, decision_json.clone(), signed.clone()),
        );
        let effects_qc = aggregate_fastswap_votes(committee, votes, FastSwapPhaseV1::Effects)
            .map_err(|error| FastSwapClientError::QuorumUnavailable {
                stage: "asset_control_apply",
                response_errors: errors
                    .into_iter()
                    .chain(std::iter::once(format!("{error:?}")))
                    .collect(),
            })?;
        verify_fast_asset_control_terminal(
            committee,
            &progress.expected_effects,
            progress
                .lock_qc
                .as_ref()
                .ok_or(FastSwapClientError::Serialization)?,
            progress
                .decision_qc
                .as_ref()
                .ok_or(FastSwapClientError::Serialization)?,
            &effects_qc,
        )
        .map_err(|error| FastSwapClientError::Session(FastSwapSessionError::Wallet(error)))?;
        let effects_signers = effects_qc
            .votes
            .iter()
            .map(|vote| vote.validator_id.as_str())
            .collect::<BTreeSet<_>>();
        progress.replication_pending = committee
            .validators
            .iter()
            .filter(|validator| !effects_signers.contains(validator.validator_id.as_str()))
            .map(|validator| validator.validator_id.clone())
            .collect();
        progress.effects_qc = Some(effects_qc);
        persist(progress).map_err(FastSwapClientError::Persistence)?;
    }
    verify_fast_asset_control_terminal(
        committee,
        &progress.expected_effects,
        progress
            .lock_qc
            .as_ref()
            .ok_or(FastSwapClientError::Serialization)?,
        progress
            .decision_qc
            .as_ref()
            .ok_or(FastSwapClientError::Serialization)?,
        progress
            .effects_qc
            .as_ref()
            .ok_or(FastSwapClientError::Serialization)?,
    )
    .map_err(|error| FastSwapClientError::Session(FastSwapSessionError::Wallet(error)))
}

pub fn reconcile_fast_asset_control_replication<T, P>(
    progress: &mut FastAssetControlWalletProgressV1,
    committee: &FastSwapCommitteeV1,
    transport: &T,
    mut persist: P,
) -> Result<FastSwapReplicationReportV1, FastSwapClientError>
where
    T: FastSwapRpcTransportV1,
    P: FnMut(&FastAssetControlWalletProgressV1) -> Result<(), String>,
{
    let lock_qc = serde_json::to_string(
        progress
            .lock_qc
            .as_ref()
            .ok_or(FastSwapClientError::Serialization)?,
    )
    .map_err(|_| FastSwapClientError::Serialization)?;
    let decision_qc = serde_json::to_string(
        progress
            .decision_qc
            .as_ref()
            .ok_or(FastSwapClientError::Serialization)?,
    )
    .map_err(|_| FastSwapClientError::Serialization)?;
    let signed = serde_json::to_string(&progress.signed_command)
        .map_err(|_| FastSwapClientError::Serialization)?;
    let expected_digest = progress
        .expected_effects
        .digest()
        .map_err(|_| FastSwapClientError::Serialization)?;
    let expected_receipt = progress
        .expected_effects
        .receipt
        .digest()
        .map_err(|_| FastSwapClientError::Serialization)?;
    let mut applied = Vec::new();
    let mut failed = Vec::new();
    for validator_id in progress.replication_pending.clone() {
        let request = fastlane_asset_control_catch_up_request(
            format!("fastlane-asset-control-catch-up-{validator_id}"),
            lock_qc.clone(),
            decision_qc.clone(),
            signed.clone(),
        );
        let result = call_certified_catch_up(transport, &validator_id, &request)
            .and_then(|response| {
                response
                    .result_as::<FastSwapVoteV1>()
                    .map_err(|error| format!("{error:?}"))
            })
            .and_then(|vote| {
                verify_fastswap_vote(committee, &vote).map_err(|error| format!("{error:?}"))?;
                if vote.validator_id != validator_id
                    || vote.phase != FastSwapPhaseV1::Effects
                    || vote.effects_digest != expected_digest
                    || vote.receipt_digest != Some(expected_receipt)
                {
                    return Err(
                        "asset control catch-up returned mismatched Effects vote".to_owned()
                    );
                }
                Ok(())
            });
        match result {
            Ok(()) => {
                progress.replication_pending.remove(&validator_id);
                applied.push(validator_id);
            }
            Err(error) => failed.push((validator_id, error)),
        }
    }
    persist(progress).map_err(FastSwapClientError::Persistence)?;
    Ok(FastSwapReplicationReportV1 {
        applied,
        failed,
        pending: progress.replication_pending.iter().cloned().collect(),
    })
}

/// Drive or reconcile one permissionless recovery round. The caller persists
/// every returned certificate before the next mutation wave. If validators
/// already report a terminal decision, this function retrieves the original
/// signed vote artifacts and proves that terminal result instead of voting
/// again.
pub fn recover_fastswap_round<T, P>(
    session: &mut FastSwapWalletSessionV1,
    committee: &FastSwapCommitteeV1,
    transport: &T,
    target_round: u64,
    mut persist: P,
) -> Result<FastSwapRecoveryOutcomeV1, FastSwapClientError>
where
    T: FastSwapRpcTransportV1,
    P: FnMut(&FastSwapWalletSessionV1) -> Result<(), String>,
{
    if target_round == 0
        || session.settlement_mode != crate::SwapSettlementModeV1::FastSwapV1
        || matches!(
            session.state,
            FastSwapProductStateV1::Draft
                | FastSwapProductStateV1::Accepted
                | FastSwapProductStateV1::Cancelled
        )
    {
        return Err(FastSwapSessionError::InvalidState.into());
    }
    let swap_id = session.expected_effects.swap_id;
    let effects_digest = session
        .expected_effects
        .digest()
        .map_err(|_| FastSwapClientError::Serialization)?;
    let new_round_votes = collect_new_round_votes(committee, transport, swap_id, target_round);
    let new_round_qc =
        aggregate_fastswap_new_round_votes(committee, new_round_votes).map_err(|error| {
            FastSwapClientError::QuorumUnavailable {
                stage: "recovery_new_round",
                response_errors: vec![format!("{error:?}")],
            }
        })?;
    let verified_new_round = verify_fastswap_new_round_certificate(committee, &new_round_qc)
        .map_err(|_| FastSwapClientError::Serialization)?;
    if verified_new_round.swap_id != swap_id
        || verified_new_round.target_round != target_round
        || verified_new_round.effects_digest != effects_digest
    {
        return Err(FastSwapClientError::RecoveryInvariant(
            "new-round certificate mismatch",
        ));
    }
    session.recovery_new_round_qc = Some(new_round_qc.clone());
    session.state = FastSwapProductStateV1::Cancelling;
    persist_session(session, &mut persist)?;

    let recovered_lock = if let Some((round, decision, digest)) = verified_new_round.highest_lock {
        let certificate = retrieve_vote_certificate(
            committee,
            transport,
            swap_id,
            FastSwapPhaseV1::Precommit,
            round,
            Some(digest),
        )?;
        let verified = verify_fastswap_certificate(committee, &certificate)
            .map_err(|_| FastSwapClientError::Serialization)?;
        if verified.digest != digest {
            return Err(FastSwapClientError::RecoveryInvariant(
                "retrieved lock certificate digest mismatch",
            ));
        }
        if verified.decision != Some(decision) || verified.effects_digest != effects_digest {
            return Err(FastSwapClientError::RecoveryInvariant(
                "retrieved lock certificate semantic mismatch",
            ));
        }
        Some(certificate)
    } else {
        None
    };

    if let Some((terminal_decision, terminal_digest)) = verified_new_round.terminal {
        let lock_qc = recovered_lock.ok_or(FastSwapClientError::RecoveryInvariant(
            "terminal decision omitted its lock certificate",
        ))?;
        let round = lock_qc
            .votes
            .first()
            .map(|vote| vote.round)
            .ok_or(FastSwapClientError::Serialization)?;
        let decision_qc = retrieve_vote_certificate(
            committee,
            transport,
            swap_id,
            FastSwapPhaseV1::Commit,
            round,
            Some(terminal_digest),
        )?;
        let verified_decision = verify_fastswap_certificate(committee, &decision_qc)
            .map_err(|_| FastSwapClientError::Serialization)?;
        if verified_decision.digest != terminal_digest
            || verified_decision.decision != Some(terminal_decision)
            || decision_qc
                .votes
                .iter()
                .any(|vote| vote.justification_digest != lock_qc.digest().ok())
        {
            return Err(FastSwapClientError::RecoveryInvariant(
                "retrieved terminal decision certificate mismatch",
            ));
        }
        session.lock_qc = Some(lock_qc);
        session.decision_qc = Some(decision_qc);
        persist_session(session, &mut persist)?;
        return finish_recovered_terminal(
            session,
            committee,
            transport,
            terminal_decision,
            round,
            true,
            &mut persist,
        );
    }

    let decision = recovered_lock
        .as_ref()
        .and_then(|certificate| certificate.votes.first()?.decision)
        .unwrap_or(FastSwapDecisionV1::Cancel);
    let proposal = FastSwapProposalV1 {
        domain: committee.domain.clone(),
        swap_id,
        round: target_round,
        decision,
        effects_digest,
        leader_id: recovery_leader(committee, swap_id, target_round)
            .map_err(|_| FastSwapClientError::Serialization)?
            .to_owned(),
        new_round_qc: Some(new_round_qc),
        justification: recovered_lock,
    };
    let proposal_json =
        serde_json::to_string(&proposal).map_err(|_| FastSwapClientError::Serialization)?;
    let (precommit_votes, precommit_errors) = collect_votes(
        committee,
        "recovery-precommit",
        FastSwapPhaseV1::Precommit,
        transport,
        |id| fastswap_precommit_request(id, proposal_json.clone()),
    );
    let lock_qc = aggregate_fastswap_votes(committee, precommit_votes, FastSwapPhaseV1::Precommit)
        .map_err(|error| FastSwapClientError::QuorumUnavailable {
            stage: "recovery_precommit",
            response_errors: precommit_errors
                .into_iter()
                .chain(std::iter::once(format!("{error:?}")))
                .collect(),
        })?;
    session.lock_qc = Some(lock_qc.clone());
    persist_session(session, &mut persist)?;

    let lock_json =
        serde_json::to_string(&lock_qc).map_err(|_| FastSwapClientError::Serialization)?;
    let (commit_votes, commit_errors) = collect_votes(
        committee,
        "recovery-commit",
        FastSwapPhaseV1::Commit,
        transport,
        |id| fastswap_commit_round_request(id, lock_json.clone()),
    );
    let decision_qc = aggregate_fastswap_votes(committee, commit_votes, FastSwapPhaseV1::Commit)
        .map_err(|error| FastSwapClientError::QuorumUnavailable {
            stage: "recovery_commit",
            response_errors: commit_errors
                .into_iter()
                .chain(std::iter::once(format!("{error:?}")))
                .collect(),
        })?;
    session.decision_qc = Some(decision_qc);
    persist_session(session, &mut persist)?;
    finish_recovered_terminal(
        session,
        committee,
        transport,
        decision,
        target_round,
        false,
        &mut persist,
    )
}

fn finish_recovered_terminal<T, P>(
    session: &mut FastSwapWalletSessionV1,
    committee: &FastSwapCommitteeV1,
    transport: &T,
    decision: FastSwapDecisionV1,
    round: u64,
    retrieve_only: bool,
    persist: &mut P,
) -> Result<FastSwapRecoveryOutcomeV1, FastSwapClientError>
where
    T: FastSwapRpcTransportV1,
    P: FnMut(&FastSwapWalletSessionV1) -> Result<(), String>,
{
    let decision_qc = session
        .decision_qc
        .as_ref()
        .ok_or(FastSwapClientError::Serialization)?;
    let decision_json = transport
        .encode_fastswap_payload(
            &serde_json::to_string(decision_qc).map_err(|_| FastSwapClientError::Serialization)?,
        )
        .map_err(|_| FastSwapClientError::Serialization)?;
    match decision {
        FastSwapDecisionV1::Confirm => {
            let (votes, errors) = if retrieve_only {
                (
                    retrieve_vote_certificate(
                        committee,
                        transport,
                        session.expected_effects.swap_id,
                        FastSwapPhaseV1::Effects,
                        round,
                        None,
                    )?
                    .votes,
                    Vec::new(),
                )
            } else {
                let signed = transport
                    .encode_fastswap_payload(
                        &serde_json::to_string(&session.signed_intent)
                            .map_err(|_| FastSwapClientError::Serialization)?,
                    )
                    .map_err(|_| FastSwapClientError::Serialization)?;
                collect_votes(
                    committee,
                    "recovery-apply",
                    FastSwapPhaseV1::Effects,
                    transport,
                    |id| fastswap_apply_request(id, decision_json.clone(), signed.clone()),
                )
            };
            session.state = FastSwapProductStateV1::Applying;
            let terminal = session
                .accept_effects_votes(committee, votes)
                .map_err(|error| FastSwapClientError::QuorumUnavailable {
                    stage: "recovery_apply",
                    response_errors: errors
                        .into_iter()
                        .chain(std::iter::once(format!("{error:?}")))
                        .collect(),
                })?;
            persist_session(session, persist)?;
            Ok(FastSwapRecoveryOutcomeV1::Accepted(Box::new(terminal)))
        }
        FastSwapDecisionV1::Cancel => {
            let (votes, errors) = if retrieve_only {
                (
                    retrieve_vote_certificate(
                        committee,
                        transport,
                        session.expected_effects.swap_id,
                        FastSwapPhaseV1::CancelApply,
                        round,
                        None,
                    )?
                    .votes,
                    Vec::new(),
                )
            } else {
                collect_votes(
                    committee,
                    "recovery-cancel-apply",
                    FastSwapPhaseV1::CancelApply,
                    transport,
                    |id| fastswap_cancel_apply_request(id, decision_json.clone()),
                )
            };
            let cancel_apply_qc =
                aggregate_fastswap_votes(committee, votes, FastSwapPhaseV1::CancelApply).map_err(
                    |error| FastSwapClientError::QuorumUnavailable {
                        stage: "recovery_cancel_apply",
                        response_errors: errors
                            .into_iter()
                            .chain(std::iter::once(format!("{error:?}")))
                            .collect(),
                    },
                )?;
            if cancel_apply_qc.votes.first().map(|vote| vote.round) != Some(round) {
                return Err(FastSwapClientError::Serialization);
            }
            session
                .accept_cancel_apply_qc(committee, cancel_apply_qc.clone())
                .map_err(FastSwapClientError::from)?;
            persist_session(session, persist)?;
            Ok(FastSwapRecoveryOutcomeV1::Cancelled { cancel_apply_qc })
        }
    }
}

fn collect_new_round_votes<T: FastSwapRpcTransportV1>(
    committee: &FastSwapCommitteeV1,
    transport: &T,
    swap_id: postfiat_types::FastSwapIdV1,
    target_round: u64,
) -> Vec<FastSwapNewRoundVoteV1> {
    let swap_id_hex = postfiat_crypto_provider::bytes_to_hex(&swap_id.0);
    let (sender, receiver) = mpsc::channel();
    for validator in &committee.validators {
        let validator_id = validator.validator_id.clone();
        let request = fastswap_new_round_vote_request(
            format!("fastswap-new-round-{validator_id}"),
            swap_id_hex.clone(),
            target_round,
        );
        let transport = transport.clone();
        let sender = sender.clone();
        std::thread::spawn(move || {
            let result = transport
                .call(&validator_id, &request)
                .and_then(|response| {
                    response
                        .result_as::<FastSwapNewRoundVoteV1>()
                        .map_err(|error| format!("{error:?}"))
                });
            let _ = sender.send(result);
        });
    }
    drop(sender);
    let mut votes = Vec::new();
    while let Ok(result) = receiver.recv() {
        if let Ok(vote) = result {
            if verify_fastswap_new_round_vote(committee, &vote).is_ok() {
                votes.push(vote);
                if aggregate_fastswap_new_round_votes(committee, votes.clone()).is_ok() {
                    break;
                }
            }
        }
    }
    votes
}

fn retrieve_vote_certificate<T: FastSwapRpcTransportV1>(
    committee: &FastSwapCommitteeV1,
    transport: &T,
    swap_id: postfiat_types::FastSwapIdV1,
    phase: FastSwapPhaseV1,
    round: u64,
    expected_digest: Option<postfiat_types::FastSwapCertificateDigestV1>,
) -> Result<FastSwapCertificateV1, FastSwapClientError> {
    let swap_id_hex = postfiat_crypto_provider::bytes_to_hex(&swap_id.0);
    let (sender, receiver) = mpsc::channel();
    for validator in &committee.validators {
        let validator_id = validator.validator_id.clone();
        let request = fastswap_votes_request(
            format!("fastswap-votes-{validator_id}"),
            swap_id_hex.clone(),
            phase,
            round,
        );
        let transport = transport.clone();
        let sender = sender.clone();
        std::thread::spawn(move || {
            let result = transport
                .call(&validator_id, &request)
                .and_then(|response| {
                    response
                        .result_as::<FastSwapVoteEvidenceResponseV1>()
                        .map_err(|error| format!("{error:?}"))
                });
            let _ = sender.send(result);
        });
    }
    drop(sender);
    let mut votes = Vec::new();
    let mut errors = Vec::new();
    while let Ok(result) = receiver.recv() {
        match result {
            Ok(evidence) => {
                if evidence.swap_id == swap_id && evidence.phase == phase && evidence.round == round
                {
                    if let (Some(expected), Some(certificate)) =
                        (expected_digest, evidence.certificate)
                    {
                        if verify_fastswap_certificate(committee, &certificate)
                            .is_ok_and(|verified| verified.digest == expected)
                        {
                            return Ok(certificate);
                        }
                    }
                    if let Some(vote) = evidence.vote {
                        votes.push(vote);
                        if expected_digest.is_none() {
                            if let Ok(certificate) =
                                aggregate_fastswap_votes(committee, votes.clone(), phase)
                            {
                                return Ok(certificate);
                            }
                        }
                    }
                }
            }
            Err(error) => errors.push(error),
        }
    }
    Err(FastSwapClientError::QuorumUnavailable {
        stage: "recovery_vote_retrieval",
        response_errors: errors,
    })
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

/// Replay the complete certified chain to validators that were outside the
/// quorum-early critical path. The session's durable pending set is the relay
/// outbox; successful catch-up is removed only after a verified Effects vote.
pub fn reconcile_fastswap_replication<T, P>(
    session: &mut FastSwapWalletSessionV1,
    committee: &FastSwapCommitteeV1,
    transport: &T,
    mut persist: P,
) -> Result<FastSwapReplicationReportV1, FastSwapClientError>
where
    T: FastSwapRpcTransportV1,
    P: FnMut(&FastSwapWalletSessionV1) -> Result<(), String>,
{
    if session.state != FastSwapProductStateV1::Accepted {
        return Err(FastSwapSessionError::InvalidState.into());
    }
    let lock_qc = transport
        .encode_fastswap_payload(
            &serde_json::to_string(
                session
                    .lock_qc
                    .as_ref()
                    .ok_or(FastSwapSessionError::InvalidState)?,
            )
            .map_err(|_| FastSwapClientError::Serialization)?,
        )
        .map_err(|_| FastSwapClientError::Serialization)?;
    let decision_qc = transport
        .encode_fastswap_payload(
            &serde_json::to_string(
                session
                    .decision_qc
                    .as_ref()
                    .ok_or(FastSwapSessionError::InvalidState)?,
            )
            .map_err(|_| FastSwapClientError::Serialization)?,
        )
        .map_err(|_| FastSwapClientError::Serialization)?;
    let signed = transport
        .encode_fastswap_payload(
            &serde_json::to_string(&session.signed_intent)
                .map_err(|_| FastSwapClientError::Serialization)?,
        )
        .map_err(|_| FastSwapClientError::Serialization)?;
    let expected_digest = session
        .expected_effects
        .digest()
        .map_err(|_| FastSwapClientError::Serialization)?;
    let expected_receipt = session
        .expected_effects
        .receipt
        .digest()
        .map_err(|_| FastSwapClientError::Serialization)?;
    let mut applied = Vec::new();
    let mut failed = Vec::new();
    for validator_id in session.replication_pending.clone() {
        let request = fastswap_catch_up_request(
            format!("fastswap-catch-up-{validator_id}"),
            lock_qc.clone(),
            decision_qc.clone(),
            signed.clone(),
        );
        let result = call_certified_catch_up(transport, &validator_id, &request)
            .and_then(|response| {
                response
                    .result_as::<FastSwapVoteV1>()
                    .map_err(|error| format!("{error:?}"))
            })
            .and_then(|vote| {
                verify_fastswap_vote(committee, &vote).map_err(|error| format!("{error:?}"))?;
                if vote.validator_id != validator_id
                    || vote.phase != FastSwapPhaseV1::Effects
                    || vote.effects_digest != expected_digest
                    || vote.receipt_digest != Some(expected_receipt)
                {
                    return Err("catch-up returned mismatched Effects vote".to_owned());
                }
                Ok(())
            });
        match result {
            Ok(()) => {
                session.replication_pending.remove(&validator_id);
                applied.push(validator_id);
            }
            Err(error) => failed.push((validator_id, error)),
        }
    }
    persist_session(session, &mut persist)?;
    Ok(FastSwapReplicationReportV1 {
        applied,
        failed,
        pending: session.replication_pending.iter().cloned().collect(),
    })
}

fn collect_votes<T, F>(
    committee: &FastSwapCommitteeV1,
    stage: &'static str,
    phase: FastSwapPhaseV1,
    transport: &T,
    request: F,
) -> (Vec<FastSwapVoteV1>, Vec<String>)
where
    T: FastSwapRpcTransportV1,
    F: Fn(String) -> RpcRequest + Sync,
{
    let required = usize::from(committee.domain.quorum);
    let mut groups = BTreeMap::<Vec<u8>, BTreeMap<String, FastSwapVoteV1>>::new();
    let mut errors = Vec::new();
    let (sender, receiver) = mpsc::channel();
    for validator in &committee.validators {
        let validator_id = validator.validator_id.clone();
        let rpc_id = format!("fastswap-{stage}-{validator_id}");
        let rpc_request = request(rpc_id);
        let transport = transport.clone();
        let sender = sender.clone();
        std::thread::spawn(move || {
            let result = transport
                .call(&validator_id, &rpc_request)
                .and_then(|response| {
                    response
                        .result_as::<FastSwapVoteV1>()
                        .map_err(|error| format!("{error:?}"))
                });
            let _ = sender.send((validator_id, result));
        });
    }
    drop(sender);
    while let Ok((validator_id, result)) = receiver.recv() {
        match result {
            Ok(vote) => {
                if vote.validator_id != validator_id
                    || vote.phase != phase
                    || verify_fastswap_vote(committee, &vote).is_err()
                {
                    errors.push(format!("{validator_id}: invalid or misrouted vote"));
                    continue;
                }
                let mut common = vote.clone();
                common.validator_id.clear();
                common.signature.clear();
                let key = match common.signing_bytes() {
                    Ok(key) => key,
                    Err(_) => {
                        errors.push(format!("{validator_id}: non-canonical vote"));
                        continue;
                    }
                };
                let group = groups.entry(key).or_default();
                group.entry(validator_id).or_insert(vote);
                if group.len() >= required {
                    let votes = group.values().cloned().collect::<Vec<_>>();
                    let certificate = FastSwapCertificateV1 {
                        votes: votes.clone(),
                    };
                    if verify_fastswap_certificate(committee, &certificate).is_ok() {
                        return (votes, errors);
                    }
                }
            }
            Err(error) => errors.push(format!("{validator_id}: {error}")),
        }
    }
    let votes = groups
        .into_values()
        .max_by_key(BTreeMap::len)
        .map(|group| group.into_values().collect())
        .unwrap_or_default();
    (votes, errors)
}

/// Catch-up carries complete already-verified certificates and is idempotent at
/// the validator WAL. A persistent server may close an otherwise healthy lane
/// after its idle timeout; reconnect once for this replay-only method. New
/// prepare/commit/apply mutations deliberately do not use this helper because
/// a lost response there must enter the wallet's unknown-result recovery path.
fn call_certified_catch_up<T: FastSwapRpcTransportV1>(
    transport: &T,
    validator_id: &str,
    request: &RpcRequest,
) -> Result<RpcResponse, String> {
    match transport.call(validator_id, request) {
        Ok(response) => Ok(response),
        Err(first_error) => transport
            .call(validator_id, request)
            .map_err(|second_error| {
                format!("certified catch-up reconnect failed after `{first_error}`: {second_error}")
            }),
    }
}

fn persist_session<P>(
    session: &FastSwapWalletSessionV1,
    persist: &mut P,
) -> Result<(), FastSwapClientError>
where
    P: FnMut(&FastSwapWalletSessionV1) -> Result<(), String>,
{
    persist(session).map_err(FastSwapClientError::Persistence)
}

fn mark_unknown<T, P>(
    session: &mut FastSwapWalletSessionV1,
    persist: &mut P,
    stage: &'static str,
    response_errors: Vec<String>,
    error: FastSwapSessionError,
) -> Result<T, FastSwapClientError>
where
    P: FnMut(&FastSwapWalletSessionV1) -> Result<(), String>,
{
    session.mark_unknown(format!("{stage} quorum unavailable: {error:?}"))?;
    persist_session(session, persist)?;
    Err(FastSwapClientError::QuorumUnavailable {
        stage,
        response_errors,
    })
}
