use crate::{
    aggregate_fastswap_votes, verify_fastswap_terminal, FastSwapWalletError,
    VerifiedFastSwapTerminalV1,
};
use postfiat_execution::fastswap_decision::verify_fastswap_certificate;
use postfiat_types::{
    FastAssetIdV1, FastAssetObjectV1, FastSwapCertificateV1, FastSwapCommitteeV1,
    FastSwapDecisionV1, FastSwapEffectsV1, FastSwapPhaseV1, FastSwapVoteV1, SignedFastSwapIntentV1,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

pub const FASTSWAP_WALLET_SESSION_SCHEMA_V1: &str = "postfiat-fastswap-wallet-session-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SwapSettlementModeV1 {
    #[default]
    ConsensusW6,
    FastSwapV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwapSettlementModeParseError;

impl SwapSettlementModeV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConsensusW6 => "consensus_w6",
            Self::FastSwapV1 => "fastswap_v1",
        }
    }
}

impl FromStr for SwapSettlementModeV1 {
    type Err = SwapSettlementModeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "consensus_w6" => Ok(Self::ConsensusW6),
            "fastswap_v1" => Ok(Self::FastSwapV1),
            _ => Err(SwapSettlementModeParseError),
        }
    }
}

impl std::fmt::Display for SwapSettlementModeParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("settlement must be `consensus_w6` or `fastswap_v1`")
    }
}

impl std::error::Error for SwapSettlementModeParseError {}

/// Product routing defaults to the existing W6 path. FastSwap is selected
/// only by an exact, explicit value.
pub fn swap_settlement_mode(
    value: Option<&str>,
) -> Result<SwapSettlementModeV1, SwapSettlementModeParseError> {
    value.map_or(Ok(SwapSettlementModeV1::ConsensusW6), str::parse)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastSwapProductStateV1 {
    Draft,
    AwaitingDualSignature,
    Preparing,
    Locked,
    Deciding,
    Applying,
    Accepted,
    Cancelling,
    Cancelled,
    Unknown,
    RejectedPreflight,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapStageTimingsV1 {
    pub prepare_qc_ms: u64,
    pub decision_qc_ms: u64,
    pub effects_qc_ms: u64,
    pub terminal_verify_ms: u64,
    pub total_ms: u64,
}

impl FastSwapProductStateV1 {
    pub fn inputs_must_remain_reserved(self) -> bool {
        matches!(
            self,
            Self::Preparing | Self::Locked | Self::Deciding | Self::Applying | Self::Unknown
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapWalletSessionV1 {
    pub schema: String,
    pub settlement_mode: SwapSettlementModeV1,
    pub state: FastSwapProductStateV1,
    pub signed_intent: SignedFastSwapIntentV1,
    pub expected_effects: FastSwapEffectsV1,
    pub lock_qc: Option<FastSwapCertificateV1>,
    pub decision_qc: Option<FastSwapCertificateV1>,
    pub effects_qc: Option<FastSwapCertificateV1>,
    #[serde(default)]
    pub recovery_new_round_qc: Option<postfiat_types::FastSwapNewRoundCertificateV1>,
    pub cancel_apply_qc: Option<FastSwapCertificateV1>,
    #[serde(default)]
    pub replication_pending: BTreeSet<String>,
    #[serde(default)]
    pub last_timings: Option<FastSwapStageTimingsV1>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastSwapSessionError {
    WrongSettlementMode,
    InvalidState,
    IntentEffectsMismatch,
    Wallet(FastSwapWalletError),
    InvalidCancelCertificate,
    Serialization,
}

impl From<FastSwapWalletError> for FastSwapSessionError {
    fn from(value: FastSwapWalletError) -> Self {
        Self::Wallet(value)
    }
}

impl FastSwapWalletSessionV1 {
    pub fn new(
        settlement_mode: SwapSettlementModeV1,
        signed_intent: SignedFastSwapIntentV1,
        expected_effects: FastSwapEffectsV1,
    ) -> Result<Self, FastSwapSessionError> {
        if signed_intent
            .swap_id()
            .map_err(|_| FastSwapSessionError::IntentEffectsMismatch)?
            != expected_effects.swap_id
            || expected_effects.decision != FastSwapDecisionV1::Confirm
            || !expected_effects.receipt.accepted
            || expected_effects.receipt.code != "fastswap_applied"
        {
            return Err(FastSwapSessionError::IntentEffectsMismatch);
        }
        Ok(Self {
            schema: FASTSWAP_WALLET_SESSION_SCHEMA_V1.to_owned(),
            settlement_mode,
            state: FastSwapProductStateV1::Draft,
            signed_intent,
            expected_effects,
            lock_qc: None,
            decision_qc: None,
            effects_qc: None,
            recovery_new_round_qc: None,
            cancel_apply_qc: None,
            replication_pending: BTreeSet::new(),
            last_timings: None,
            last_error: None,
        })
    }

    pub fn begin_fastswap(&mut self) -> Result<(), FastSwapSessionError> {
        if self.settlement_mode != SwapSettlementModeV1::FastSwapV1 {
            return Err(FastSwapSessionError::WrongSettlementMode);
        }
        if self.state != FastSwapProductStateV1::Draft {
            return Err(FastSwapSessionError::InvalidState);
        }
        self.state = FastSwapProductStateV1::Preparing;
        Ok(())
    }

    pub fn accept_prepare_votes(
        &mut self,
        committee: &FastSwapCommitteeV1,
        votes: impl IntoIterator<Item = FastSwapVoteV1>,
    ) -> Result<&FastSwapCertificateV1, FastSwapSessionError> {
        if self.state != FastSwapProductStateV1::Preparing {
            return Err(FastSwapSessionError::InvalidState);
        }
        let qc = aggregate_fastswap_votes(committee, votes, FastSwapPhaseV1::Precommit)?;
        self.lock_qc = Some(qc);
        self.state = FastSwapProductStateV1::Locked;
        Ok(self.lock_qc.as_ref().expect("set above"))
    }

    pub fn accept_commit_votes(
        &mut self,
        committee: &FastSwapCommitteeV1,
        votes: impl IntoIterator<Item = FastSwapVoteV1>,
    ) -> Result<&FastSwapCertificateV1, FastSwapSessionError> {
        if self.state != FastSwapProductStateV1::Locked {
            return Err(FastSwapSessionError::InvalidState);
        }
        self.state = FastSwapProductStateV1::Deciding;
        let qc = aggregate_fastswap_votes(committee, votes, FastSwapPhaseV1::Commit)?;
        let lock_digest = self
            .lock_qc
            .as_ref()
            .ok_or(FastSwapSessionError::InvalidState)?
            .digest()
            .map_err(|_| FastSwapSessionError::IntentEffectsMismatch)?;
        if qc
            .votes
            .iter()
            .any(|vote| vote.justification_digest != Some(lock_digest))
        {
            return Err(FastSwapSessionError::IntentEffectsMismatch);
        }
        self.decision_qc = Some(qc);
        self.state = FastSwapProductStateV1::Applying;
        Ok(self.decision_qc.as_ref().expect("set above"))
    }

    pub fn accept_effects_votes(
        &mut self,
        committee: &FastSwapCommitteeV1,
        votes: impl IntoIterator<Item = FastSwapVoteV1>,
    ) -> Result<VerifiedFastSwapTerminalV1, FastSwapSessionError> {
        if self.state != FastSwapProductStateV1::Applying {
            return Err(FastSwapSessionError::InvalidState);
        }
        let effects_qc = aggregate_fastswap_votes(committee, votes, FastSwapPhaseV1::Effects)?;
        let terminal = verify_fastswap_terminal(
            committee,
            &self.expected_effects,
            self.lock_qc
                .as_ref()
                .ok_or(FastSwapSessionError::InvalidState)?,
            self.decision_qc
                .as_ref()
                .ok_or(FastSwapSessionError::InvalidState)?,
            &effects_qc,
        )?;
        self.effects_qc = Some(effects_qc);
        self.state = FastSwapProductStateV1::Accepted;
        self.last_error = None;
        Ok(terminal)
    }

    pub fn mark_unknown(&mut self, message: impl Into<String>) -> Result<(), FastSwapSessionError> {
        if matches!(
            self.state,
            FastSwapProductStateV1::Accepted
                | FastSwapProductStateV1::Cancelled
                | FastSwapProductStateV1::RejectedPreflight
        ) {
            return Err(FastSwapSessionError::InvalidState);
        }
        self.state = FastSwapProductStateV1::Unknown;
        self.last_error = Some(message.into());
        Ok(())
    }

    pub fn resume_unknown(
        &mut self,
        committee: &FastSwapCommitteeV1,
    ) -> Result<(), FastSwapSessionError> {
        if self.state != FastSwapProductStateV1::Unknown {
            return Err(FastSwapSessionError::InvalidState);
        }
        if let Some(cancel_qc) = self.cancel_apply_qc.clone() {
            return self.accept_cancel_apply_qc(committee, cancel_qc);
        }
        if let Some(effects_qc) = self.effects_qc.as_ref() {
            verify_fastswap_terminal(
                committee,
                &self.expected_effects,
                self.lock_qc
                    .as_ref()
                    .ok_or(FastSwapSessionError::InvalidState)?,
                self.decision_qc
                    .as_ref()
                    .ok_or(FastSwapSessionError::InvalidState)?,
                effects_qc,
            )?;
            self.state = FastSwapProductStateV1::Accepted;
            self.last_error = None;
            return Ok(());
        }
        let expected_digest = self
            .expected_effects
            .digest()
            .map_err(|_| FastSwapSessionError::IntentEffectsMismatch)?;
        if let Some(decision_qc) = self.decision_qc.as_ref() {
            let lock_qc = self
                .lock_qc
                .as_ref()
                .ok_or(FastSwapSessionError::InvalidState)?;
            let lock = verify_fastswap_certificate(committee, lock_qc)
                .map_err(|_| FastSwapSessionError::InvalidState)?;
            let decision = verify_fastswap_certificate(committee, decision_qc)
                .map_err(|_| FastSwapSessionError::InvalidState)?;
            let lock_digest = lock_qc
                .digest()
                .map_err(|_| FastSwapSessionError::InvalidState)?;
            if lock.phase != FastSwapPhaseV1::Precommit
                || lock.decision != Some(FastSwapDecisionV1::Confirm)
                || lock.effects_digest != expected_digest
                || decision.phase != FastSwapPhaseV1::Commit
                || decision.decision != Some(FastSwapDecisionV1::Confirm)
                || decision.effects_digest != expected_digest
                || decision_qc
                    .votes
                    .iter()
                    .any(|vote| vote.justification_digest != Some(lock_digest))
            {
                return Err(FastSwapSessionError::InvalidState);
            }
            self.state = FastSwapProductStateV1::Applying;
        } else if let Some(lock_qc) = self.lock_qc.as_ref() {
            let lock = verify_fastswap_certificate(committee, lock_qc)
                .map_err(|_| FastSwapSessionError::InvalidState)?;
            if lock.phase != FastSwapPhaseV1::Precommit
                || lock.decision != Some(FastSwapDecisionV1::Confirm)
                || lock.effects_digest != expected_digest
            {
                return Err(FastSwapSessionError::InvalidState);
            }
            self.state = FastSwapProductStateV1::Locked;
        } else {
            self.state = FastSwapProductStateV1::Preparing;
        }
        self.last_error = None;
        Ok(())
    }

    pub fn accept_cancel_apply_qc(
        &mut self,
        committee: &FastSwapCommitteeV1,
        certificate: FastSwapCertificateV1,
    ) -> Result<(), FastSwapSessionError> {
        let verified = verify_fastswap_certificate(committee, &certificate)
            .map_err(|_| FastSwapSessionError::InvalidCancelCertificate)?;
        if verified.phase != FastSwapPhaseV1::CancelApply
            || verified.decision != Some(FastSwapDecisionV1::Cancel)
            || verified.swap_id != self.expected_effects.swap_id
            || verified.effects_digest
                != self
                    .expected_effects
                    .digest()
                    .map_err(|_| FastSwapSessionError::IntentEffectsMismatch)?
        {
            return Err(FastSwapSessionError::InvalidCancelCertificate);
        }
        self.cancel_apply_qc = Some(certificate);
        self.state = FastSwapProductStateV1::Cancelled;
        self.last_error = None;
        Ok(())
    }

    pub fn to_durable_json(&self) -> Result<Vec<u8>, FastSwapSessionError> {
        serde_json::to_vec(self).map_err(|_| FastSwapSessionError::Serialization)
    }

    pub fn from_durable_json(bytes: &[u8]) -> Result<Self, FastSwapSessionError> {
        let session: Self =
            serde_json::from_slice(bytes).map_err(|_| FastSwapSessionError::Serialization)?;
        if session.schema != FASTSWAP_WALLET_SESSION_SCHEMA_V1
            || session.signed_intent.swap_id().ok() != Some(session.expected_effects.swap_id)
        {
            return Err(FastSwapSessionError::Serialization);
        }
        Ok(session)
    }

    pub fn unavailable_inputs(&self) -> BTreeSet<postfiat_types::FastObjectKeyV1> {
        if self.state.inputs_must_remain_reserved() {
            self.expected_effects.consumed.iter().copied().collect()
        } else {
            BTreeSet::new()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastLanePrefundSuggestionV1 {
    pub asset_id: FastAssetIdV1,
    pub available_atoms: u64,
    pub required_atoms: u64,
    pub deficit_atoms: u64,
}

pub fn fastlane_prefund_suggestions(
    objects: &[FastAssetObjectV1],
    required: &BTreeMap<FastAssetIdV1, u64>,
) -> Result<Vec<FastLanePrefundSuggestionV1>, FastSwapSessionError> {
    let mut available = BTreeMap::<FastAssetIdV1, u64>::new();
    for object in objects {
        let total = available.entry(object.asset_id).or_default();
        *total = total
            .checked_add(object.amount_atoms)
            .ok_or(FastSwapSessionError::IntentEffectsMismatch)?;
    }
    Ok(required
        .iter()
        .filter_map(|(asset_id, required_atoms)| {
            let available_atoms = available.get(asset_id).copied().unwrap_or(0);
            let deficit_atoms = required_atoms.saturating_sub(available_atoms);
            (deficit_atoms > 0).then_some(FastLanePrefundSuggestionV1 {
                asset_id: *asset_id,
                available_atoms,
                required_atoms: *required_atoms,
                deficit_atoms,
            })
        })
        .collect())
}

#[cfg(test)]
mod settlement_mode_tests {
    use super::*;

    #[test]
    fn settlement_mode_defaults_to_w6_and_fastswap_requires_exact_opt_in() {
        assert_eq!(
            swap_settlement_mode(None).expect("default mode"),
            SwapSettlementModeV1::ConsensusW6
        );
        assert_eq!(
            swap_settlement_mode(Some("fastswap_v1")).expect("FastSwap opt-in"),
            SwapSettlementModeV1::FastSwapV1
        );
        assert!(swap_settlement_mode(Some("fastswap")).is_err());
        assert!(swap_settlement_mode(Some("FASTSWAP_V1")).is_err());
    }
}
