use postfiat_types::{
    FastAssetControlStateV1, FastAssetObjectV1, FastAssetRuleHashV1, FastAssetRuleV1,
    FastHolderPermitIdV1, FastHolderPermitV1, FastLaneCheckpointIdV1, FastLaneDepositReceiptV1,
    FastLaneExitClaimV1, FastLaneExitEffectsV1, FastLaneExitIdV1, FastLanePrepareFenceV1,
    FastLaneReserveBalanceV1, FastLaneStateV1, FastObjectKeyV1, FastObjectOriginV1,
    FastSwapCertificateDigestV1, FastSwapCertificateV1, FastSwapCommitteeDomainV1,
    FastSwapDecisionV1, FastSwapEffectsDigestV1, FastSwapEffectsV1, FastSwapExitClaimIdV1,
    FastSwapIdV1, FastSwapIntentIdV1, FastSwapLocalStatusV1, FastSwapNewRoundVoteV1,
    FastSwapPhaseV1, FastSwapPolicyHashV1, FastSwapPolicySnapshotV1, FastSwapRecordV1,
    FastSwapReservationV1, FastSwapTerminalTombstoneV1, FastSwapVoteV1,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

const FASTSWAP_WAL_FILE: &str = "fastswap-v1.wal";
const FASTSWAP_SNAPSHOT_FILE: &str = "fastswap-v1.snapshot.json";
const FASTSWAP_LOCK_FILE: &str = "fastswap-v1.lock";
const FASTSWAP_VOTE_ARTIFACT_DIRECTORY: &str = "vote-artifacts";
const FASTSWAP_WAL_MAX_RECORD_BYTES: usize = 1024 * 1024;
const FASTSWAP_SNAPSHOT_MAX_BYTES: usize = 64 * 1024 * 1024;
const FASTSWAP_WAL_CHECKSUM_BYTES: usize = 48;
const FASTSWAP_SNAPSHOT_SCHEMA: &str = "postfiat-fastswap-snapshot-v1";
const FASTLANE_STATE_FILE_SCHEMA: &str = "postfiat-fastlane-state-file-v1";
pub const FASTLANE_STATE_FILE_MAX_BYTES: usize = FASTSWAP_SNAPSHOT_MAX_BYTES;

#[derive(Debug)]
#[non_exhaustive]
pub enum FastSwapStoreError {
    Io(io::Error),
    Serialization(String),
    RecordTooLarge(usize),
    CorruptWal { offset: u64, reason: &'static str },
    CorruptSnapshot(&'static str),
    SequenceMismatch { expected: u64, actual: u64 },
    Conflict(&'static str),
    StateInvariant(&'static str),
}

impl std::fmt::Display for FastSwapStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for FastSwapStoreError {}

impl From<io::Error> for FastSwapStoreError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FastSwapWalRecordV1 {
    Reserve {
        sequence: u64,
        swap_id: FastSwapIdV1,
        intent_id: FastSwapIntentIdV1,
        effects_digest: FastSwapEffectsDigestV1,
        expires_at_height: u64,
        inputs: Vec<FastObjectKeyV1>,
    },
    DecisionLock {
        sequence: u64,
        swap_id: FastSwapIdV1,
        round: u64,
        decision: FastSwapDecisionV1,
        certificate_digest: FastSwapCertificateDigestV1,
    },
    AdvanceRound {
        sequence: u64,
        swap_id: FastSwapIdV1,
        target_round: u64,
    },
    PrecommitVote {
        sequence: u64,
        swap_id: FastSwapIdV1,
        round: u64,
        decision: FastSwapDecisionV1,
    },
    SupersedePartial {
        sequence: u64,
        losing_swap_id: FastSwapIdV1,
        winning_swap_id: FastSwapIdV1,
        winning_decision_certificate_digest: FastSwapCertificateDigestV1,
    },
    ApplyConfirm {
        sequence: u64,
        effects: FastSwapEffectsV1,
        decision_certificate_digest: FastSwapCertificateDigestV1,
    },
    ApplyCancel {
        sequence: u64,
        swap_id: FastSwapIdV1,
        decision_certificate_digest: FastSwapCertificateDigestV1,
    },
    ApplyExit {
        sequence: u64,
        effects: FastLaneExitEffectsV1,
    },
    ImportDeposit {
        sequence: u64,
        receipt: FastLaneDepositReceiptV1,
    },
    Fence {
        sequence: u64,
        committee_epoch: u64,
        policy_epoch: u64,
        finalized_primary_height: u64,
    },
    AnchorCheckpoint {
        sequence: u64,
        checkpoint_id: FastLaneCheckpointIdV1,
        pending_fee_burns: Vec<FastSwapWalFeeBurnV1>,
    },
}

/// JSON WALs cannot losslessly deserialize `u128` through serde_json's default
/// number model. Store checkpoint burn amounts as fixed-width big-endian bytes
/// so replay is exact across every supported amount.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastSwapWalFeeBurnV1 {
    asset_id: postfiat_types::FastAssetIdV1,
    amount_atoms_be: [u8; 16],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FastSwapSnapshotV1 {
    schema: String,
    next_sequence: u64,
    state: FastSwapSnapshotStateV1,
    checksum: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FastLaneStateFileV1 {
    schema: String,
    state: FastSwapSnapshotStateV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FastSwapSnapshotStateV1 {
    schema_version: u32,
    committee: FastSwapCommitteeDomainV1,
    objects: Vec<(FastObjectKeyV1, FastAssetObjectV1)>,
    reservations: Vec<(FastObjectKeyV1, FastSwapReservationV1)>,
    swaps: Vec<(FastSwapIdV1, FastSwapRecordV1)>,
    imported_deposits: Vec<postfiat_types::FastSwapDepositIdV1>,
    exit_claims: Vec<(FastSwapExitClaimIdV1, FastLaneExitClaimV1)>,
    terminal_tombstones: Vec<(FastSwapIdV1, FastSwapTerminalTombstoneV1)>,
    asset_rules: Vec<(FastAssetRuleHashV1, FastAssetRuleV1)>,
    holder_permits: Vec<(FastHolderPermitIdV1, FastHolderPermitV1)>,
    policy_snapshots: Vec<(FastSwapPolicyHashV1, FastSwapPolicySnapshotV1)>,
    prepare_fences: Vec<(u64, FastLanePrepareFenceV1)>,
    pending_fee_burns: Vec<FastSwapWalFeeBurnV1>,
    anchored_checkpoints: Vec<FastLaneCheckpointIdV1>,
}

impl FastSwapSnapshotStateV1 {
    fn from_state(state: &FastLaneStateV1) -> Self {
        Self {
            schema_version: state.schema_version,
            committee: state.committee.clone(),
            objects: state
                .objects
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            reservations: state
                .reservations
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            swaps: state
                .swaps
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            imported_deposits: state.imported_deposits.iter().copied().collect(),
            exit_claims: state
                .exit_claims
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            terminal_tombstones: state
                .terminal_tombstones
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            asset_rules: state
                .asset_rules
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            holder_permits: state
                .holder_permits
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            policy_snapshots: state
                .policy_snapshots
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            prepare_fences: state
                .prepare_fences
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .collect(),
            pending_fee_burns: state
                .pending_fee_burns
                .iter()
                .map(|(asset_id, amount_atoms)| FastSwapWalFeeBurnV1 {
                    asset_id: *asset_id,
                    amount_atoms_be: amount_atoms.to_be_bytes(),
                })
                .collect(),
            anchored_checkpoints: state.anchored_checkpoints.iter().copied().collect(),
        }
    }

    fn into_state(self) -> Result<FastLaneStateV1, FastSwapStoreError> {
        macro_rules! require_sorted {
            ($rows:expr, $key:expr, $name:literal) => {
                if !$rows.windows(2).all(|pair| $key(&pair[0]) < $key(&pair[1])) {
                    return Err(FastSwapStoreError::CorruptSnapshot(concat!(
                        $name,
                        " are not sorted and unique"
                    )));
                }
            };
        }
        require_sorted!(
            self.objects,
            |row: &(FastObjectKeyV1, FastAssetObjectV1)| row.0,
            "objects"
        );
        require_sorted!(
            self.reservations,
            |row: &(FastObjectKeyV1, FastSwapReservationV1)| row.0,
            "reservations"
        );
        require_sorted!(
            self.swaps,
            |row: &(FastSwapIdV1, FastSwapRecordV1)| row.0,
            "swaps"
        );
        require_sorted!(
            self.imported_deposits,
            |row: &postfiat_types::FastSwapDepositIdV1| *row,
            "deposits"
        );
        require_sorted!(
            self.exit_claims,
            |row: &(FastSwapExitClaimIdV1, FastLaneExitClaimV1)| row.0,
            "exit claims"
        );
        require_sorted!(
            self.terminal_tombstones,
            |row: &(FastSwapIdV1, FastSwapTerminalTombstoneV1)| row.0,
            "tombstones"
        );
        require_sorted!(
            self.asset_rules,
            |row: &(FastAssetRuleHashV1, FastAssetRuleV1)| row.0,
            "asset rules"
        );
        require_sorted!(
            self.holder_permits,
            |row: &(FastHolderPermitIdV1, FastHolderPermitV1)| row.0,
            "holder permits"
        );
        require_sorted!(
            self.policy_snapshots,
            |row: &(FastSwapPolicyHashV1, FastSwapPolicySnapshotV1)| row.0,
            "policies"
        );
        require_sorted!(
            self.prepare_fences,
            |row: &(u64, FastLanePrepareFenceV1)| row.0,
            "fences"
        );
        require_sorted!(
            self.pending_fee_burns,
            |row: &FastSwapWalFeeBurnV1| row.asset_id,
            "fee burns"
        );
        require_sorted!(
            self.anchored_checkpoints,
            |row: &FastLaneCheckpointIdV1| *row,
            "checkpoints"
        );
        if self.objects.iter().any(|(key, value)| key != &value.key)
            || self.swaps.iter().any(|(key, value)| key != &value.swap_id)
            || self
                .exit_claims
                .iter()
                .any(|(key, value)| key != &value.exit_claim_id)
            || self
                .terminal_tombstones
                .iter()
                .any(|(key, value)| key != &value.swap_id)
        {
            return Err(FastSwapStoreError::CorruptSnapshot(
                "snapshot map key/value mismatch",
            ));
        }
        let pending_fee_burns = self
            .pending_fee_burns
            .into_iter()
            .map(|row| (row.asset_id, u128::from_be_bytes(row.amount_atoms_be)))
            .collect();
        Ok(FastLaneStateV1 {
            schema_version: self.schema_version,
            committee: self.committee,
            objects: self.objects.into_iter().collect(),
            reservations: self.reservations.into_iter().collect(),
            swaps: self.swaps.into_iter().collect(),
            imported_deposits: self.imported_deposits.into_iter().collect(),
            exit_claims: self.exit_claims.into_iter().collect(),
            terminal_tombstones: self.terminal_tombstones.into_iter().collect(),
            asset_rules: self.asset_rules.into_iter().collect(),
            holder_permits: self.holder_permits.into_iter().collect(),
            policy_snapshots: self.policy_snapshots.into_iter().collect(),
            prepare_fences: self.prepare_fences.into_iter().collect(),
            pending_fee_burns,
            anchored_checkpoints: self.anchored_checkpoints.into_iter().collect(),
        })
    }
}

pub fn encode_fastlane_state_file(state: &FastLaneStateV1) -> Result<Vec<u8>, FastSwapStoreError> {
    let file = FastLaneStateFileV1 {
        schema: FASTLANE_STATE_FILE_SCHEMA.to_owned(),
        state: FastSwapSnapshotStateV1::from_state(state),
    };
    let bytes = serde_json::to_vec_pretty(&file)
        .map_err(|error| FastSwapStoreError::Serialization(error.to_string()))?;
    if bytes.len() > FASTLANE_STATE_FILE_MAX_BYTES {
        return Err(FastSwapStoreError::RecordTooLarge(bytes.len()));
    }
    Ok(bytes)
}

pub fn decode_fastlane_state_file(bytes: &[u8]) -> Result<FastLaneStateV1, FastSwapStoreError> {
    if bytes.len() > FASTLANE_STATE_FILE_MAX_BYTES {
        return Err(FastSwapStoreError::RecordTooLarge(bytes.len()));
    }
    match serde_json::from_slice::<FastLaneStateFileV1>(bytes) {
        Ok(file) => {
            if file.schema != FASTLANE_STATE_FILE_SCHEMA {
                return Err(FastSwapStoreError::CorruptSnapshot(
                    "FastLane state file schema mismatch",
                ));
            }
            file.state.into_state()
        }
        Err(wrapper_error) => serde_json::from_slice::<FastLaneStateV1>(bytes).map_err(|_| {
            FastSwapStoreError::Serialization(format!(
                "FastLane state file decode failed: {wrapper_error}"
            ))
        }),
    }
}

impl FastSwapWalRecordV1 {
    fn sequence(&self) -> u64 {
        match self {
            Self::Reserve { sequence, .. }
            | Self::DecisionLock { sequence, .. }
            | Self::AdvanceRound { sequence, .. }
            | Self::PrecommitVote { sequence, .. }
            | Self::SupersedePartial { sequence, .. }
            | Self::ApplyConfirm { sequence, .. }
            | Self::ApplyCancel { sequence, .. }
            | Self::ApplyExit { sequence, .. }
            | Self::ImportDeposit { sequence, .. }
            | Self::Fence { sequence, .. }
            | Self::AnchorCheckpoint { sequence, .. } => *sequence,
        }
    }
}

#[derive(Debug)]
struct ProcessLock {
    _file: File,
}

impl Drop for ProcessLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // SAFETY: `_file` remains a valid descriptor for the whole call.
            // Explicit unlock makes the logical store owner authoritative even
            // when a concurrently forked child briefly inherited the open file
            // description before exec closed its descriptor.
            let _ = unsafe { libc::flock(self._file.as_raw_fd(), libc::LOCK_UN) };
        }
    }
}

fn acquire_process_lock(file: &File) -> io::Result<()> {
    #[cfg(unix)]
    {
        // SAFETY: flock only observes the valid descriptor borrowed from
        // `file`; ProcessLock retains that File for the entire store lifetime.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "FastSwap durable process locking requires Unix flock",
        ))
    }
}

#[derive(Debug)]
pub struct FastSwapStore {
    wal_path: PathBuf,
    snapshot_path: PathBuf,
    vote_artifact_directory: PathBuf,
    next_sequence: u64,
    _process_lock: ProcessLock,
    #[cfg(test)]
    fail_before_append_once: bool,
    #[cfg(test)]
    fail_after_sync_once: bool,
}

impl FastSwapStore {
    pub fn highest_wal_sequence(&self) -> u64 {
        self.next_sequence.saturating_sub(1)
    }

    pub fn open(directory: impl AsRef<Path>) -> Result<Self, FastSwapStoreError> {
        let directory = directory.as_ref();
        fs::create_dir_all(directory)?;
        let lock_path = directory.join(FASTSWAP_LOCK_FILE);
        let mut lock_file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(FastSwapStoreError::Io)?;
        acquire_process_lock(&lock_file).map_err(|error| {
            if matches!(error.raw_os_error(), Some(libc::EACCES | libc::EAGAIN)) {
                FastSwapStoreError::Conflict("FastSwap store is already locked")
            } else {
                FastSwapStoreError::Io(error)
            }
        })?;
        lock_file.set_len(0)?;
        writeln!(lock_file, "pid={}", std::process::id())?;
        lock_file.sync_all()?;
        let wal_path = directory.join(FASTSWAP_WAL_FILE);
        let snapshot_path = directory.join(FASTSWAP_SNAPSHOT_FILE);
        let vote_artifact_directory = directory.join(FASTSWAP_VOTE_ARTIFACT_DIRECTORY);
        fs::create_dir_all(&vote_artifact_directory)?;
        let snapshot = read_snapshot(&snapshot_path)?;
        let records = read_records(&wal_path)?;
        let snapshot_next = snapshot.as_ref().map_or(0, |value| value.next_sequence);
        let next_sequence = validate_record_sequences(&records, snapshot_next)?;
        Ok(Self {
            wal_path,
            snapshot_path,
            vote_artifact_directory,
            next_sequence,
            _process_lock: ProcessLock { _file: lock_file },
            #[cfg(test)]
            fail_before_append_once: false,
            #[cfg(test)]
            fail_after_sync_once: false,
        })
    }

    #[cfg(test)]
    fn inject_fail_before_append_once(&mut self) {
        self.fail_before_append_once = true;
    }

    #[cfg(test)]
    fn inject_fail_after_sync_once(&mut self) {
        self.fail_after_sync_once = true;
    }

    pub fn replay(&self, base: &FastLaneStateV1) -> Result<FastLaneStateV1, FastSwapStoreError> {
        let records = read_records(&self.wal_path)?;
        let snapshot = read_snapshot(&self.snapshot_path)?;
        let (mut state, mut expected) = match snapshot {
            Some(snapshot) => (snapshot.state.into_state()?, snapshot.next_sequence),
            None => (base.clone(), 0),
        };
        validate_record_sequences(&records, expected)?;
        for record in &records {
            if record.sequence() < expected {
                continue;
            }
            if record.sequence() != expected {
                return Err(FastSwapStoreError::SequenceMismatch {
                    expected,
                    actual: record.sequence(),
                });
            }
            apply_record(&mut state, record)?;
            expected = expected.saturating_add(1);
        }
        Ok(state)
    }

    /// Persist a complete checksummed state image, then discard the WAL prefix
    /// it covers. If the process dies before truncation, replay ignores the
    /// covered prefix; if it dies after truncation, the durable snapshot is the
    /// replay base. Sequence numbers never reset.
    pub fn compact_snapshot(&self, state: &FastLaneStateV1) -> Result<(), FastSwapStoreError> {
        let snapshot_state = FastSwapSnapshotStateV1::from_state(state);
        let snapshot = FastSwapSnapshotV1 {
            schema: FASTSWAP_SNAPSHOT_SCHEMA.to_owned(),
            next_sequence: self.next_sequence,
            checksum: snapshot_checksum(self.next_sequence, &snapshot_state)?.to_vec(),
            state: snapshot_state,
        };
        write_snapshot(&self.snapshot_path, &snapshot)?;
        let wal = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&self.wal_path)?;
        wal.sync_all()?;
        let parent = self
            .wal_path
            .parent()
            .ok_or(FastSwapStoreError::Conflict("WAL path has no parent"))?;
        OpenOptions::new().read(true).open(parent)?.sync_all()?;
        Ok(())
    }

    pub fn reserve_all(
        &mut self,
        state: &mut FastLaneStateV1,
        swap_id: FastSwapIdV1,
        intent_id: FastSwapIntentIdV1,
        effects_digest: FastSwapEffectsDigestV1,
        expires_at_height: u64,
        inputs: &[FastObjectKeyV1],
    ) -> Result<u64, FastSwapStoreError> {
        if inputs.is_empty() || !inputs.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(FastSwapStoreError::Conflict(
                "reservation inputs must be nonempty, sorted, and unique",
            ));
        }
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::Reserve {
            sequence,
            swap_id,
            intent_id,
            effects_digest,
            expires_at_height,
            inputs: inputs.to_vec(),
        };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn persist_decision_lock(
        &mut self,
        state: &mut FastLaneStateV1,
        swap_id: FastSwapIdV1,
        round: u64,
        decision: FastSwapDecisionV1,
        certificate_digest: FastSwapCertificateDigestV1,
    ) -> Result<u64, FastSwapStoreError> {
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::DecisionLock {
            sequence,
            swap_id,
            round,
            decision,
            certificate_digest,
        };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn persist_new_round_vote(
        &mut self,
        state: &mut FastLaneStateV1,
        swap_id: FastSwapIdV1,
        target_round: u64,
    ) -> Result<u64, FastSwapStoreError> {
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::AdvanceRound {
            sequence,
            swap_id,
            target_round,
        };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn persist_precommit_vote(
        &mut self,
        state: &mut FastLaneStateV1,
        swap_id: FastSwapIdV1,
        round: u64,
        decision: FastSwapDecisionV1,
    ) -> Result<u64, FastSwapStoreError> {
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::PrecommitVote {
            sequence,
            swap_id,
            round,
            decision,
        };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn apply_confirm(
        &mut self,
        state: &mut FastLaneStateV1,
        effects: FastSwapEffectsV1,
        decision_certificate_digest: FastSwapCertificateDigestV1,
    ) -> Result<u64, FastSwapStoreError> {
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::ApplyConfirm {
            sequence,
            effects,
            decision_certificate_digest,
        };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn supersede_partial(
        &mut self,
        state: &mut FastLaneStateV1,
        losing_swap_id: FastSwapIdV1,
        winning_swap_id: FastSwapIdV1,
        winning_decision_certificate_digest: FastSwapCertificateDigestV1,
    ) -> Result<u64, FastSwapStoreError> {
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::SupersedePartial {
            sequence,
            losing_swap_id,
            winning_swap_id,
            winning_decision_certificate_digest,
        };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn apply_cancel(
        &mut self,
        state: &mut FastLaneStateV1,
        swap_id: FastSwapIdV1,
        decision_certificate_digest: FastSwapCertificateDigestV1,
    ) -> Result<u64, FastSwapStoreError> {
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::ApplyCancel {
            sequence,
            swap_id,
            decision_certificate_digest,
        };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn apply_exit(
        &mut self,
        state: &mut FastLaneStateV1,
        effects: FastLaneExitEffectsV1,
    ) -> Result<u64, FastSwapStoreError> {
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::ApplyExit { sequence, effects };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn import_deposit(
        &mut self,
        state: &mut FastLaneStateV1,
        receipt: FastLaneDepositReceiptV1,
    ) -> Result<u64, FastSwapStoreError> {
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::ImportDeposit { sequence, receipt };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn persist_prepare_fence(
        &mut self,
        state: &mut FastLaneStateV1,
        committee_epoch: u64,
        policy_epoch: u64,
        finalized_primary_height: u64,
    ) -> Result<u64, FastSwapStoreError> {
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::Fence {
            sequence,
            committee_epoch,
            policy_epoch,
            finalized_primary_height,
        };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn apply_anchored_checkpoint(
        &mut self,
        state: &mut FastLaneStateV1,
        checkpoint_id: FastLaneCheckpointIdV1,
        pending_fee_burns: &[FastLaneReserveBalanceV1],
    ) -> Result<u64, FastSwapStoreError> {
        if state.anchored_checkpoints.contains(&checkpoint_id) {
            return Ok(self.highest_wal_sequence());
        }
        let sequence = self.next_sequence;
        let record = FastSwapWalRecordV1::AnchorCheckpoint {
            sequence,
            checkpoint_id,
            pending_fee_burns: pending_fee_burns
                .iter()
                .map(|burn| FastSwapWalFeeBurnV1 {
                    asset_id: burn.asset_id,
                    amount_atoms_be: burn.amount_atoms.to_be_bytes(),
                })
                .collect(),
        };
        self.persist_then_apply(state, &record)?;
        Ok(sequence)
    }

    pub fn applied_effects(
        &self,
        swap_id: FastSwapIdV1,
    ) -> Result<Option<FastSwapEffectsV1>, FastSwapStoreError> {
        Ok(read_records(&self.wal_path)?
            .into_iter()
            .rev()
            .find_map(|record| match record {
                FastSwapWalRecordV1::ApplyConfirm { effects, .. } if effects.swap_id == swap_id => {
                    Some(effects)
                }
                _ => None,
            }))
    }

    pub fn applied_exit_effects(
        &self,
        exit_id: FastLaneExitIdV1,
    ) -> Result<Option<FastLaneExitEffectsV1>, FastSwapStoreError> {
        Ok(read_records(&self.wal_path)?
            .into_iter()
            .rev()
            .find_map(|record| match record {
                FastSwapWalRecordV1::ApplyExit { effects, .. } if effects.exit_id == exit_id => {
                    Some(effects)
                }
                _ => None,
            }))
    }

    /// Persist retrievable signed evidence before it can leave the validator.
    /// This artifact is not safety state—the preceding WAL record is—but it is
    /// required for permissionless recovery by a replacement relayer.
    pub fn persist_vote_artifact(&self, vote: &FastSwapVoteV1) -> Result<(), FastSwapStoreError> {
        let path = self.vote_artifact_path(
            vote.swap_id,
            phase_name(vote.phase),
            vote.round,
            &vote.validator_id,
        );
        write_synced_json(&path, vote)
    }

    pub fn persist_new_round_vote_artifact(
        &self,
        vote: &FastSwapNewRoundVoteV1,
    ) -> Result<(), FastSwapStoreError> {
        let path = self.vote_artifact_path(
            vote.swap_id,
            "new-round",
            vote.target_round,
            &vote.validator_id,
        );
        write_synced_json(&path, vote)
    }

    pub fn vote_artifact(
        &self,
        swap_id: FastSwapIdV1,
        phase: FastSwapPhaseV1,
        round: u64,
        validator_id: &str,
    ) -> Result<Option<FastSwapVoteV1>, FastSwapStoreError> {
        read_optional_bounded_json(&self.vote_artifact_path(
            swap_id,
            phase_name(phase),
            round,
            validator_id,
        ))
    }

    pub fn new_round_vote_artifact(
        &self,
        swap_id: FastSwapIdV1,
        target_round: u64,
        validator_id: &str,
    ) -> Result<Option<FastSwapNewRoundVoteV1>, FastSwapStoreError> {
        read_optional_bounded_json(&self.vote_artifact_path(
            swap_id,
            "new-round",
            target_round,
            validator_id,
        ))
    }

    pub fn persist_certificate_artifact(
        &self,
        certificate: &FastSwapCertificateV1,
    ) -> Result<(), FastSwapStoreError> {
        let digest = certificate.digest().map_err(|error| {
            FastSwapStoreError::Serialization(format!("invalid certificate: {error:?}"))
        })?;
        write_synced_json(&self.certificate_artifact_path(digest), certificate)
    }

    pub fn certificate_artifact(
        &self,
        digest: FastSwapCertificateDigestV1,
    ) -> Result<Option<FastSwapCertificateV1>, FastSwapStoreError> {
        read_optional_bounded_json(&self.certificate_artifact_path(digest))
    }

    fn certificate_artifact_path(&self, digest: FastSwapCertificateDigestV1) -> PathBuf {
        self.vote_artifact_directory
            .join(format!("certificate-{}.json", lower_hex(&digest.0)))
    }

    fn vote_artifact_path(
        &self,
        swap_id: FastSwapIdV1,
        phase: &str,
        round: u64,
        validator_id: &str,
    ) -> PathBuf {
        let swap = lower_hex(&swap_id.0);
        let validator = checksum(validator_id.as_bytes());
        self.vote_artifact_directory.join(format!(
            "{swap}-{phase}-{round}-{}.json",
            lower_hex(&validator)
        ))
    }

    fn persist_then_apply(
        &mut self,
        state: &mut FastLaneStateV1,
        record: &FastSwapWalRecordV1,
    ) -> Result<(), FastSwapStoreError> {
        let mut candidate = state.clone();
        apply_record(&mut candidate, record)?;
        #[cfg(test)]
        if std::mem::take(&mut self.fail_before_append_once) {
            return Err(FastSwapStoreError::Io(io::Error::new(
                io::ErrorKind::StorageFull,
                "injected WAL pre-append failure",
            )));
        }
        append_synced_record(&self.wal_path, record)?;
        #[cfg(test)]
        if std::mem::take(&mut self.fail_after_sync_once) {
            return Err(FastSwapStoreError::Io(io::Error::new(
                io::ErrorKind::Interrupted,
                "injected crash after WAL sync",
            )));
        }
        *state = candidate;
        self.next_sequence = self.next_sequence.saturating_add(1);
        Ok(())
    }
}

fn phase_name(phase: FastSwapPhaseV1) -> &'static str {
    match phase {
        FastSwapPhaseV1::Precommit => "precommit",
        FastSwapPhaseV1::Commit => "commit",
        FastSwapPhaseV1::Effects => "effects",
        FastSwapPhaseV1::NewRound => "new-round-phase",
        FastSwapPhaseV1::CancelApply => "cancel-apply",
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn write_synced_json<T: Serialize>(path: &Path, value: &T) -> Result<(), FastSwapStoreError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| FastSwapStoreError::Serialization(error.to_string()))?;
    if bytes.len() > FASTSWAP_WAL_MAX_RECORD_BYTES {
        return Err(FastSwapStoreError::RecordTooLarge(bytes.len()));
    }
    let parent = path
        .parent()
        .ok_or(FastSwapStoreError::Conflict("artifact path has no parent"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(FastSwapStoreError::Conflict("artifact filename invalid"))?;
    let temporary = parent.join(format!(".{file_name}.tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn read_optional_bounded_json<T: serde::de::DeserializeOwned>(
    path: &Path,
) -> Result<Option<T>, FastSwapStoreError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if bytes.len() > FASTSWAP_WAL_MAX_RECORD_BYTES {
        return Err(FastSwapStoreError::RecordTooLarge(bytes.len()));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| FastSwapStoreError::Serialization(error.to_string()))
}

fn apply_record(
    state: &mut FastLaneStateV1,
    record: &FastSwapWalRecordV1,
) -> Result<(), FastSwapStoreError> {
    match record {
        FastSwapWalRecordV1::Reserve {
            swap_id,
            intent_id,
            effects_digest,
            expires_at_height,
            inputs,
            ..
        } => {
            if let Some(existing) = state.swaps.get(swap_id) {
                if existing.intent_id == *intent_id && existing.effects_digest == *effects_digest {
                    return Ok(());
                }
                return Err(FastSwapStoreError::Conflict("swap id digest conflict"));
            }
            for key in inputs {
                if !state.objects.contains_key(key) {
                    return Err(FastSwapStoreError::Conflict("reservation input is absent"));
                }
                if state.reservations.contains_key(key) {
                    return Err(FastSwapStoreError::Conflict("input already reserved"));
                }
            }
            let reservation = FastSwapReservationV1 {
                swap_id: *swap_id,
                intent_id: *intent_id,
                effects_digest: *effects_digest,
            };
            for key in inputs {
                state.reservations.insert(*key, reservation.clone());
            }
            state.swaps.insert(
                *swap_id,
                FastSwapRecordV1 {
                    swap_id: *swap_id,
                    intent_id: *intent_id,
                    effects_digest: *effects_digest,
                    expires_at_height: *expires_at_height,
                    status: FastSwapLocalStatusV1::Prepared,
                    highest_precommit_round: 0,
                    highest_new_round_vote: 0,
                    decision_lock_round: None,
                    decision_lock_value: None,
                    lock_certificate_digest: None,
                    decision_certificate_digest: None,
                },
            );
        }
        FastSwapWalRecordV1::DecisionLock {
            swap_id,
            round,
            decision,
            certificate_digest,
            ..
        } => {
            let swap = state
                .swaps
                .get_mut(swap_id)
                .ok_or(FastSwapStoreError::Conflict("decision lock before prepare"))?;
            if *round < swap.highest_precommit_round {
                return Err(FastSwapStoreError::Conflict("stale decision lock round"));
            }
            if let (Some(locked_round), Some(locked_value)) =
                (swap.decision_lock_round, swap.decision_lock_value)
            {
                if *round < locked_round || (*round == locked_round && *decision != locked_value) {
                    return Err(FastSwapStoreError::Conflict("conflicting decision lock"));
                }
            }
            swap.highest_precommit_round = swap.highest_precommit_round.max(*round);
            swap.decision_lock_round = Some(*round);
            swap.decision_lock_value = Some(*decision);
            swap.lock_certificate_digest = Some(*certificate_digest);
            swap.status = FastSwapLocalStatusV1::DecisionLocked;
        }
        FastSwapWalRecordV1::AdvanceRound {
            swap_id,
            target_round,
            ..
        } => {
            let swap = state
                .swaps
                .get_mut(swap_id)
                .ok_or(FastSwapStoreError::Conflict(
                    "new-round vote before prepare",
                ))?;
            if *target_round == 0
                || *target_round != swap.highest_new_round_vote.saturating_add(1)
                || *target_round <= swap.highest_precommit_round
            {
                return Err(FastSwapStoreError::Conflict("invalid new-round advance"));
            }
            swap.highest_new_round_vote = *target_round;
        }
        FastSwapWalRecordV1::PrecommitVote {
            swap_id,
            round,
            decision: _,
            ..
        } => {
            let swap = state
                .swaps
                .get_mut(swap_id)
                .ok_or(FastSwapStoreError::Conflict(
                    "precommit vote before prepare",
                ))?;
            if *round == 0 || *round <= swap.highest_precommit_round {
                return Err(FastSwapStoreError::Conflict(
                    "duplicate or stale precommit vote",
                ));
            }
            swap.highest_precommit_round = *round;
        }
        FastSwapWalRecordV1::SupersedePartial {
            losing_swap_id,
            winning_swap_id,
            winning_decision_certificate_digest: _,
            ..
        } => {
            if losing_swap_id == winning_swap_id
                || state.terminal_tombstones.contains_key(losing_swap_id)
            {
                return Err(FastSwapStoreError::Conflict(
                    "cannot supersede terminal or identical swap",
                ));
            }
            let losing = state
                .swaps
                .get_mut(losing_swap_id)
                .ok_or(FastSwapStoreError::Conflict("superseded swap is unknown"))?;
            if matches!(
                losing.status,
                FastSwapLocalStatusV1::Applied
                    | FastSwapLocalStatusV1::Cancelled
                    | FastSwapLocalStatusV1::Checkpointed
            ) {
                return Err(FastSwapStoreError::Conflict(
                    "cannot supersede terminal swap state",
                ));
            }
            losing.status = FastSwapLocalStatusV1::Superseded;
            state
                .reservations
                .retain(|_, reservation| reservation.swap_id != *losing_swap_id);
        }
        FastSwapWalRecordV1::ApplyConfirm {
            effects,
            decision_certificate_digest,
            ..
        } => {
            if effects.decision != FastSwapDecisionV1::Confirm
                || !effects.receipt.accepted
                || !matches!(
                    effects.receipt.code.as_str(),
                    "fastswap_applied" | "fastlane_asset_control_applied"
                )
            {
                return Err(FastSwapStoreError::Conflict("invalid confirm effects"));
            }
            if state
                .terminal_tombstones
                .get(&effects.swap_id)
                .is_some_and(|tombstone| tombstone.decision == FastSwapDecisionV1::Cancel)
            {
                return Err(FastSwapStoreError::Conflict(
                    "confirm after cancel tombstone",
                ));
            }
            let swap = state
                .swaps
                .get(&effects.swap_id)
                .ok_or(FastSwapStoreError::Conflict("confirm before prepare"))?;
            if swap.decision_lock_value != Some(FastSwapDecisionV1::Confirm) {
                return Err(FastSwapStoreError::Conflict("confirm without confirm lock"));
            }
            if effects
                .digest()
                .map_err(|_| FastSwapStoreError::StateInvariant("effects encoding invalid"))?
                != swap.effects_digest
            {
                return Err(FastSwapStoreError::StateInvariant(
                    "effects digest does not match reserved swap",
                ));
            }
            for key in &effects.consumed {
                let reservation =
                    state
                        .reservations
                        .get(key)
                        .ok_or(FastSwapStoreError::Conflict(
                            "consumed input is not reserved",
                        ))?;
                if reservation.swap_id != effects.swap_id || !state.objects.contains_key(key) {
                    return Err(FastSwapStoreError::Conflict(
                        "consumed reservation mismatch",
                    ));
                }
            }
            for object in &effects.created {
                if state.objects.contains_key(&object.key) {
                    return Err(FastSwapStoreError::StateInvariant("output collision"));
                }
            }
            for key in &effects.consumed {
                state.objects.remove(key);
                state.reservations.remove(key);
            }
            for object in &effects.created {
                state.objects.insert(object.key, object.clone());
            }
            for burn in &effects.fee_burns {
                let amount = state.pending_fee_burns.entry(burn.asset_id).or_default();
                *amount = amount
                    .checked_add(u128::from(burn.amount_atoms))
                    .ok_or(FastSwapStoreError::StateInvariant("fee burn overflow"))?;
            }
            let swap = state
                .swaps
                .get_mut(&effects.swap_id)
                .ok_or(FastSwapStoreError::StateInvariant("swap disappeared"))?;
            swap.status = FastSwapLocalStatusV1::Applied;
            swap.decision_lock_value = Some(FastSwapDecisionV1::Confirm);
            swap.decision_certificate_digest = Some(*decision_certificate_digest);
            state.terminal_tombstones.insert(
                effects.swap_id,
                FastSwapTerminalTombstoneV1 {
                    swap_id: effects.swap_id,
                    decision: FastSwapDecisionV1::Confirm,
                    decision_certificate_digest: *decision_certificate_digest,
                },
            );
        }
        FastSwapWalRecordV1::ApplyCancel {
            swap_id,
            decision_certificate_digest,
            ..
        } => {
            if state
                .terminal_tombstones
                .get(swap_id)
                .is_some_and(|tombstone| tombstone.decision == FastSwapDecisionV1::Confirm)
            {
                return Err(FastSwapStoreError::Conflict(
                    "cancel after confirm tombstone",
                ));
            }
            let swap = state
                .swaps
                .get_mut(swap_id)
                .ok_or(FastSwapStoreError::Conflict("cancel before prepare"))?;
            if swap.decision_lock_value != Some(FastSwapDecisionV1::Cancel) {
                return Err(FastSwapStoreError::Conflict("cancel without cancel lock"));
            }
            state
                .reservations
                .retain(|_, reservation| reservation.swap_id != *swap_id);
            swap.status = FastSwapLocalStatusV1::Cancelled;
            swap.decision_certificate_digest = Some(*decision_certificate_digest);
            state.terminal_tombstones.insert(
                *swap_id,
                FastSwapTerminalTombstoneV1 {
                    swap_id: *swap_id,
                    decision: FastSwapDecisionV1::Cancel,
                    decision_certificate_digest: *decision_certificate_digest,
                },
            );
        }
        FastSwapWalRecordV1::ApplyExit { effects, .. } => {
            effects
                .canonical_bytes()
                .map_err(|_| FastSwapStoreError::StateInvariant("exit effects encoding invalid"))?;
            if let Some(existing) = state.exit_claims.get(&effects.claim.exit_claim_id) {
                if existing == &effects.claim
                    && effects
                        .consumed
                        .iter()
                        .all(|key| !state.objects.contains_key(key))
                {
                    return Ok(());
                }
                return Err(FastSwapStoreError::Conflict("exit claim collision"));
            }
            let mut total = 0_u64;
            for key in &effects.consumed {
                if state.reservations.contains_key(key) {
                    return Err(FastSwapStoreError::Conflict("exit input is reserved"));
                }
                let object = state
                    .objects
                    .get(key)
                    .ok_or(FastSwapStoreError::Conflict("exit input is absent"))?;
                if object.owner_pubkey != effects.claim.owner_pubkey
                    || object.asset_id != effects.claim.asset_id
                    || object.asset_rule_hash != effects.claim.asset_rule_hash
                    || !matches!(
                        object.control_state,
                        postfiat_types::FastAssetControlStateV1::Spendable
                    )
                {
                    return Err(FastSwapStoreError::Conflict("exit input mismatch"));
                }
                total = total
                    .checked_add(object.amount_atoms)
                    .ok_or(FastSwapStoreError::StateInvariant("exit amount overflow"))?;
            }
            if total != effects.claim.amount_atoms {
                return Err(FastSwapStoreError::Conflict("exit amount mismatch"));
            }
            for key in &effects.consumed {
                state.objects.remove(key);
            }
            state
                .exit_claims
                .insert(effects.claim.exit_claim_id, effects.claim.clone());
        }
        FastSwapWalRecordV1::ImportDeposit { receipt, .. } => {
            if !receipt.accepted || receipt.code != "fastlane_deposit_applied" {
                return Err(FastSwapStoreError::Conflict(
                    "deposit receipt is not accepted",
                ));
            }
            if state.imported_deposits.contains(&receipt.deposit_id) {
                if let Some(existing) = state.objects.get(&receipt.initial_object_key) {
                    if existing.owner_pubkey != receipt.destination_owner_pubkey
                        || existing.asset_id != receipt.asset_id
                        || existing.asset_rule_hash != receipt.asset_rule_hash
                        || existing.amount_atoms != receipt.amount_atoms
                    {
                        return Err(FastSwapStoreError::Conflict("deposit replay mismatch"));
                    }
                }
                // The initial object may already have been consumed by a
                // later durable record. Never recreate it during replay.
                return Ok(());
            }
            if state.objects.contains_key(&receipt.initial_object_key) {
                return Err(FastSwapStoreError::Conflict("deposit object collision"));
            }
            let rule = state.asset_rules.get(&receipt.asset_rule_hash).ok_or(
                FastSwapStoreError::Conflict("deposit asset rule is unknown"),
            )?;
            if !rule.fast_lane_enabled || rule.asset_id != receipt.asset_id {
                return Err(FastSwapStoreError::Conflict("deposit asset rule mismatch"));
            }
            state.objects.insert(
                receipt.initial_object_key,
                FastAssetObjectV1 {
                    key: receipt.initial_object_key,
                    owner_pubkey: receipt.destination_owner_pubkey.clone(),
                    asset_id: receipt.asset_id,
                    asset_rule_hash: receipt.asset_rule_hash,
                    amount_atoms: receipt.amount_atoms,
                    control_state: FastAssetControlStateV1::Spendable,
                    origin: FastObjectOriginV1::Deposit {
                        deposit_id: receipt.deposit_id,
                    },
                },
            );
            state.imported_deposits.insert(receipt.deposit_id);
        }
        FastSwapWalRecordV1::Fence {
            committee_epoch,
            policy_epoch,
            finalized_primary_height,
            ..
        } => {
            if *committee_epoch != state.committee.committee_epoch {
                return Err(FastSwapStoreError::Conflict("fence committee mismatch"));
            }
            if !state
                .policy_snapshots
                .values()
                .any(|policy| policy.policy_epoch == *policy_epoch)
            {
                return Err(FastSwapStoreError::Conflict(
                    "fence policy epoch is unknown",
                ));
            }
            state.prepare_fences.insert(
                *policy_epoch,
                postfiat_types::FastLanePrepareFenceV1 {
                    committee_epoch: *committee_epoch,
                    policy_epoch: *policy_epoch,
                    finalized_primary_height: *finalized_primary_height,
                },
            );
        }
        FastSwapWalRecordV1::AnchorCheckpoint {
            checkpoint_id,
            pending_fee_burns,
            ..
        } => {
            if state.anchored_checkpoints.contains(checkpoint_id) {
                return Ok(());
            }
            if !pending_fee_burns
                .windows(2)
                .all(|pair| pair[0].asset_id < pair[1].asset_id)
            {
                return Err(FastSwapStoreError::Conflict(
                    "checkpoint fee burns must be sorted and unique",
                ));
            }
            for burn in pending_fee_burns {
                let amount_atoms = u128::from_be_bytes(burn.amount_atoms_be);
                let current = state
                    .pending_fee_burns
                    .get(&burn.asset_id)
                    .copied()
                    .unwrap_or(0);
                if amount_atoms == 0 || current < amount_atoms {
                    return Err(FastSwapStoreError::Conflict(
                        "checkpoint fee burn exceeds pending amount",
                    ));
                }
            }
            for burn in pending_fee_burns {
                let amount_atoms = u128::from_be_bytes(burn.amount_atoms_be);
                let remaining = state.pending_fee_burns[&burn.asset_id] - amount_atoms;
                if remaining == 0 {
                    state.pending_fee_burns.remove(&burn.asset_id);
                } else {
                    state.pending_fee_burns.insert(burn.asset_id, remaining);
                }
            }
            state.anchored_checkpoints.insert(*checkpoint_id);
        }
    }
    Ok(())
}

fn append_synced_record(
    path: &Path,
    record: &FastSwapWalRecordV1,
) -> Result<(), FastSwapStoreError> {
    let payload = serde_json::to_vec(record)
        .map_err(|error| FastSwapStoreError::Serialization(error.to_string()))?;
    if payload.len() > FASTSWAP_WAL_MAX_RECORD_BYTES {
        return Err(FastSwapStoreError::RecordTooLarge(payload.len()));
    }
    let length: u32 = payload
        .len()
        .try_into()
        .map_err(|_| FastSwapStoreError::RecordTooLarge(payload.len()))?;
    let checksum = checksum(&payload);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(&length.to_be_bytes())?;
    file.write_all(&payload)?;
    file.write_all(&checksum)?;
    file.sync_data()?;
    Ok(())
}

fn validate_record_sequences(
    records: &[FastSwapWalRecordV1],
    snapshot_next: u64,
) -> Result<u64, FastSwapStoreError> {
    let Some(first) = records.first() else {
        return Ok(snapshot_next);
    };
    let mut expected = first.sequence();
    if snapshot_next == 0 && expected != 0 || expected > snapshot_next {
        return Err(FastSwapStoreError::SequenceMismatch {
            expected: snapshot_next,
            actual: expected,
        });
    }
    for record in records {
        if record.sequence() != expected {
            return Err(FastSwapStoreError::SequenceMismatch {
                expected,
                actual: record.sequence(),
            });
        }
        expected = expected.saturating_add(1);
    }
    Ok(expected.max(snapshot_next))
}

fn snapshot_checksum(
    next_sequence: u64,
    state: &FastSwapSnapshotStateV1,
) -> Result<[u8; FASTSWAP_WAL_CHECKSUM_BYTES], FastSwapStoreError> {
    let payload = serde_json::to_vec(&(FASTSWAP_SNAPSHOT_SCHEMA, next_sequence, state))
        .map_err(|error| FastSwapStoreError::Serialization(error.to_string()))?;
    let mut hasher = Sha3_384::new();
    hasher.update(b"postfiat.fastswap.snapshot.v1");
    hasher.update([0_u8]);
    hasher.update(payload);
    Ok(hasher.finalize().into())
}

fn read_snapshot(path: &Path) -> Result<Option<FastSwapSnapshotV1>, FastSwapStoreError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if bytes.len() > FASTSWAP_SNAPSHOT_MAX_BYTES {
        return Err(FastSwapStoreError::CorruptSnapshot(
            "snapshot exceeds size bound",
        ));
    }
    let snapshot: FastSwapSnapshotV1 = serde_json::from_slice(&bytes)
        .map_err(|_| FastSwapStoreError::CorruptSnapshot("snapshot decode failure"))?;
    if snapshot.schema != FASTSWAP_SNAPSHOT_SCHEMA {
        return Err(FastSwapStoreError::CorruptSnapshot(
            "snapshot schema mismatch",
        ));
    }
    if snapshot.checksum.len() != FASTSWAP_WAL_CHECKSUM_BYTES
        || snapshot_checksum(snapshot.next_sequence, &snapshot.state)?.as_slice()
            != snapshot.checksum.as_slice()
    {
        return Err(FastSwapStoreError::CorruptSnapshot(
            "snapshot checksum mismatch",
        ));
    }
    Ok(Some(snapshot))
}

fn write_snapshot(path: &Path, snapshot: &FastSwapSnapshotV1) -> Result<(), FastSwapStoreError> {
    let bytes = serde_json::to_vec(snapshot)
        .map_err(|error| FastSwapStoreError::Serialization(error.to_string()))?;
    if bytes.len() > FASTSWAP_SNAPSHOT_MAX_BYTES {
        return Err(FastSwapStoreError::RecordTooLarge(bytes.len()));
    }
    let parent = path
        .parent()
        .ok_or(FastSwapStoreError::Conflict("snapshot path has no parent"))?;
    let temporary = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or(FastSwapStoreError::Conflict("snapshot filename invalid"))?,
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn read_records(path: &Path) -> Result<Vec<FastSwapWalRecordV1>, FastSwapStoreError> {
    let mut bytes = Vec::new();
    match File::open(path) {
        Ok(mut file) => {
            file.read_to_end(&mut bytes)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(FastSwapStoreError::Io(error)),
    }
    let mut offset = 0_usize;
    let mut records = Vec::new();
    while offset < bytes.len() {
        let start = offset;
        let Some(length_bytes) = bytes.get(offset..offset.saturating_add(4)) else {
            break;
        };
        let length = u32::from_be_bytes(length_bytes.try_into().map_err(|_| {
            FastSwapStoreError::CorruptWal {
                offset: start as u64,
                reason: "invalid length prefix",
            }
        })?) as usize;
        if length > FASTSWAP_WAL_MAX_RECORD_BYTES {
            return Err(FastSwapStoreError::CorruptWal {
                offset: start as u64,
                reason: "record length exceeds bound",
            });
        }
        offset += 4;
        let record_end = offset
            .checked_add(length)
            .and_then(|value| value.checked_add(FASTSWAP_WAL_CHECKSUM_BYTES))
            .ok_or(FastSwapStoreError::CorruptWal {
                offset: start as u64,
                reason: "record length overflow",
            })?;
        if record_end > bytes.len() {
            // A torn final append is safe to ignore: sync precedes every vote,
            // so no signature could have escaped for this incomplete record.
            break;
        }
        let payload = &bytes[offset..offset + length];
        offset += length;
        let stored_checksum = &bytes[offset..record_end];
        offset = record_end;
        if checksum(payload) != stored_checksum {
            return Err(FastSwapStoreError::CorruptWal {
                offset: start as u64,
                reason: "checksum mismatch",
            });
        }
        let record =
            serde_json::from_slice(payload).map_err(|_| FastSwapStoreError::CorruptWal {
                offset: start as u64,
                reason: "record decode failure",
            })?;
        records.push(record);
    }
    Ok(records)
}

fn checksum(payload: &[u8]) -> [u8; FASTSWAP_WAL_CHECKSUM_BYTES] {
    let mut hasher = Sha3_384::new();
    hasher.update(b"postfiat.fastswap.wal.record.v1");
    hasher.update([0_u8]);
    hasher.update(payload);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_types::{
        FastAssetControlStateV1, FastAssetIdV1, FastAssetObjectV1, FastAssetRuleHashV1,
        FastObjectIdV1, FastObjectOriginV1, FastSwapChainDomainV1, FastSwapCommitteeDomainV1,
        FastSwapCommitteeRootV1, FastSwapDepositIdV1, FastSwapOpaqueHashV1,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_ID: AtomicU64 = AtomicU64::new(0);

    fn test_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "postfiat-fastswap-store-{label}-{}-{}",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn state() -> FastLaneStateV1 {
        let domain = FastSwapCommitteeDomainV1 {
            chain: FastSwapChainDomainV1 {
                chain_id: "test".to_owned(),
                genesis_hash: FastSwapOpaqueHashV1([1; 48]),
                protocol_version: 1,
            },
            fastswap_schema_version: 1,
            committee_epoch: 1,
            committee_root: FastSwapCommitteeRootV1([2; 48]),
            validator_count: 6,
            quorum: 5,
        };
        let object = FastAssetObjectV1 {
            key: FastObjectKeyV1 {
                object_id: FastObjectIdV1([3; 32]),
                version: 1,
            },
            owner_pubkey: vec![4; 64],
            asset_id: FastAssetIdV1([5; 48]),
            asset_rule_hash: FastAssetRuleHashV1([6; 48]),
            amount_atoms: 10,
            control_state: FastAssetControlStateV1::Spendable,
            origin: FastObjectOriginV1::Deposit {
                deposit_id: FastSwapDepositIdV1([7; 48]),
            },
        };
        FastLaneStateV1 {
            schema_version: 1,
            committee: domain,
            objects: BTreeMap::from([(object.key, object)]),
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
        }
    }

    #[test]
    fn reservation_is_synced_and_replays_exactly() {
        let directory = test_dir("replay");
        let base = state();
        let mut live = base.clone();
        let key = *live.objects.keys().next().expect("object");
        {
            let mut store = FastSwapStore::open(&directory).expect("open");
            store
                .reserve_all(
                    &mut live,
                    FastSwapIdV1([8; 48]),
                    FastSwapIntentIdV1([9; 48]),
                    FastSwapEffectsDigestV1([10; 48]),
                    100,
                    &[key],
                )
                .expect("reserve");
        }
        let store = FastSwapStore::open(&directory).expect("reopen");
        assert_eq!(store.replay(&base).expect("replay"), live);
        drop(store);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn nonempty_fastlane_base_state_uses_canonical_vector_file() {
        let mut value = state();
        value
            .pending_fee_burns
            .insert(FastAssetIdV1([52; 48]), u128::from(u64::MAX) + 1);
        let encoded = encode_fastlane_state_file(&value).expect("encode state file");
        assert_eq!(
            decode_fastlane_state_file(&encoded).expect("decode state file"),
            value
        );
        assert!(
            serde_json::to_vec(&value).is_err(),
            "direct typed-key map JSON must not be used as the deployment format"
        );

        let mut malformed: serde_json::Value =
            serde_json::from_slice(&encoded).expect("state file JSON");
        let objects = malformed
            .get_mut("state")
            .and_then(|state| state.get_mut("objects"))
            .and_then(serde_json::Value::as_array_mut)
            .expect("object rows");
        objects.push(objects[0].clone());
        assert!(matches!(
            decode_fastlane_state_file(
                &serde_json::to_vec(&malformed).expect("malformed state JSON")
            ),
            Err(FastSwapStoreError::CorruptSnapshot(_))
        ));
    }

    #[test]
    fn anchored_checkpoint_burn_is_atomic_idempotent_and_replays() {
        let directory = test_dir("checkpoint-burn");
        let mut base = state();
        let asset = FastAssetIdV1([44; 48]);
        base.pending_fee_burns.insert(asset, 7);
        let checkpoint_id = FastLaneCheckpointIdV1([45; 48]);
        let burns = vec![FastLaneReserveBalanceV1 {
            asset_id: asset,
            amount_atoms: 3,
        }];
        let roundtrip_record = FastSwapWalRecordV1::AnchorCheckpoint {
            sequence: 0,
            checkpoint_id,
            pending_fee_burns: vec![FastSwapWalFeeBurnV1 {
                asset_id: asset,
                amount_atoms_be: 3_u128.to_be_bytes(),
            }],
        };
        let encoded = serde_json::to_vec(&roundtrip_record).expect("encode record");
        assert_eq!(
            serde_json::from_slice::<FastSwapWalRecordV1>(&encoded).expect("decode record"),
            roundtrip_record
        );
        let mut live = base.clone();
        {
            let mut store = FastSwapStore::open(&directory).expect("open");
            store
                .apply_anchored_checkpoint(&mut live, checkpoint_id, &burns)
                .expect("anchor checkpoint");
            assert_eq!(live.pending_fee_burns.get(&asset), Some(&4));
            assert!(live.anchored_checkpoints.contains(&checkpoint_id));

            let sequence = store.highest_wal_sequence();
            store
                .apply_anchored_checkpoint(&mut live, checkpoint_id, &burns)
                .expect("idempotent anchor");
            assert_eq!(live.pending_fee_burns.get(&asset), Some(&4));
            assert_eq!(
                store.highest_wal_sequence(),
                sequence,
                "idempotent observation must not append another WAL record"
            );

            let before = live.clone();
            let overburn = [FastLaneReserveBalanceV1 {
                asset_id: asset,
                amount_atoms: 5,
            }];
            assert!(matches!(
                store.apply_anchored_checkpoint(
                    &mut live,
                    FastLaneCheckpointIdV1([46; 48]),
                    &overburn,
                ),
                Err(FastSwapStoreError::Conflict(_))
            ));
            assert_eq!(live, before, "rejected overburn must not mutate");
        }
        let store = FastSwapStore::open(&directory).expect("reopen");
        assert_eq!(store.replay(&base).expect("replay"), live);
        drop(store);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn checkpoint_snapshot_survives_old_wal_prefix_and_preserves_sequence() {
        let directory = test_dir("snapshot-replay");
        let mut base = state();
        let large_asset = FastAssetIdV1([47; 48]);
        let large_amount = u128::from(u64::MAX) + 9;
        base.pending_fee_burns.insert(large_asset, large_amount);
        let mut live = base.clone();
        let key = *live.objects.keys().next().expect("object");
        let old_wal_prefix;
        {
            let mut store = FastSwapStore::open(&directory).expect("open");
            store
                .reserve_all(
                    &mut live,
                    FastSwapIdV1([48; 48]),
                    FastSwapIntentIdV1([49; 48]),
                    FastSwapEffectsDigestV1([50; 48]),
                    100,
                    &[key],
                )
                .expect("reserve");
            old_wal_prefix = fs::read(directory.join(FASTSWAP_WAL_FILE)).expect("read WAL");
            store.compact_snapshot(&live).expect("compact snapshot");
            assert_eq!(
                fs::metadata(directory.join(FASTSWAP_WAL_FILE))
                    .expect("WAL metadata")
                    .len(),
                0
            );
        }

        // Simulate a crash after the snapshot rename but before WAL-prefix
        // truncation. The covered prefix is valid but must not be re-applied.
        fs::write(directory.join(FASTSWAP_WAL_FILE), old_wal_prefix)
            .expect("restore covered WAL prefix");
        {
            let mut store = FastSwapStore::open(&directory).expect("reopen covered prefix");
            assert_eq!(store.highest_wal_sequence(), 0);
            store
                .apply_anchored_checkpoint(&mut live, FastLaneCheckpointIdV1([51; 48]), &[])
                .expect("append after snapshot");
            assert_eq!(store.highest_wal_sequence(), 1);
        }
        let store = FastSwapStore::open(&directory).expect("final reopen");
        let replayed = store.replay(&state()).expect("snapshot plus tail replay");
        assert_eq!(replayed, live);
        assert_eq!(
            replayed.pending_fee_burns.get(&large_asset),
            Some(&large_amount)
        );
        drop(store);

        let snapshot_path = directory.join(FASTSWAP_SNAPSHOT_FILE);
        let mut corrupted = fs::read(&snapshot_path).expect("read snapshot");
        let index = corrupted.len() / 2;
        corrupted[index] ^= 1;
        fs::write(&snapshot_path, corrupted).expect("corrupt snapshot");
        assert!(matches!(
            FastSwapStore::open(&directory),
            Err(FastSwapStoreError::CorruptSnapshot(_))
        ));
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn signed_vote_artifacts_survive_restart_and_replace_idempotently() {
        let directory = test_dir("vote-artifact");
        let swap_id = FastSwapIdV1([8; 48]);
        let vote = FastSwapVoteV1 {
            domain: state().committee,
            swap_id,
            phase: FastSwapPhaseV1::Precommit,
            round: 0,
            decision: Some(FastSwapDecisionV1::Confirm),
            justification_digest: None,
            effects_digest: FastSwapEffectsDigestV1([9; 48]),
            receipt_digest: None,
            validator_id: "validator-0".to_owned(),
            signature: vec![1, 2, 3],
        };
        {
            let store = FastSwapStore::open(&directory).expect("open");
            store.persist_vote_artifact(&vote).expect("persist vote");
            store
                .persist_vote_artifact(&vote)
                .expect("replace same vote");
        }
        let store = FastSwapStore::open(&directory).expect("reopen");
        assert_eq!(
            store
                .vote_artifact(swap_id, FastSwapPhaseV1::Precommit, 0, "validator-0")
                .expect("read vote"),
            Some(vote)
        );
        assert!(store
            .vote_artifact(swap_id, FastSwapPhaseV1::Commit, 0, "validator-0")
            .expect("missing vote")
            .is_none());
        drop(store);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn injected_storage_failure_before_append_leaves_complete_old_state() {
        let directory = test_dir("fail-before-append");
        let base = state();
        let mut live = base.clone();
        let key = *live.objects.keys().next().expect("object");
        {
            let mut store = FastSwapStore::open(&directory).expect("open");
            store.inject_fail_before_append_once();
            assert!(matches!(
                store.reserve_all(
                    &mut live,
                    FastSwapIdV1([8; 48]),
                    FastSwapIntentIdV1([9; 48]),
                    FastSwapEffectsDigestV1([10; 48]),
                    100,
                    &[key],
                ),
                Err(FastSwapStoreError::Io(_))
            ));
            assert_eq!(live, base);
        }
        let store = FastSwapStore::open(&directory).expect("reopen");
        assert_eq!(store.replay(&base).expect("replay"), base);
        drop(store);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn injected_crash_after_sync_replays_complete_new_state() {
        let directory = test_dir("fail-after-sync");
        let base = state();
        let mut live = base.clone();
        let key = *live.objects.keys().next().expect("object");
        let swap_id = FastSwapIdV1([8; 48]);
        {
            let mut store = FastSwapStore::open(&directory).expect("open");
            store.inject_fail_after_sync_once();
            assert!(matches!(
                store.reserve_all(
                    &mut live,
                    swap_id,
                    FastSwapIntentIdV1([9; 48]),
                    FastSwapEffectsDigestV1([10; 48]),
                    100,
                    &[key],
                ),
                Err(FastSwapStoreError::Io(_))
            ));
            assert_eq!(
                live, base,
                "memory was not published before simulated crash"
            );
        }
        let store = FastSwapStore::open(&directory).expect("reopen");
        let replayed = store.replay(&base).expect("replay");
        assert_eq!(
            replayed.reservations.get(&key).map(|value| value.swap_id),
            Some(swap_id)
        );
        assert_eq!(
            replayed.swaps.get(&swap_id).map(|value| value.status),
            Some(FastSwapLocalStatusV1::Prepared)
        );
        assert_eq!(replayed.objects, base.objects);
        drop(store);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn torn_final_record_is_ignored_but_checksum_corruption_is_fatal() {
        let directory = test_dir("torn");
        let base = state();
        let mut live = base.clone();
        let key = *live.objects.keys().next().expect("object");
        {
            let mut store = FastSwapStore::open(&directory).expect("open");
            store
                .reserve_all(
                    &mut live,
                    FastSwapIdV1([8; 48]),
                    FastSwapIntentIdV1([9; 48]),
                    FastSwapEffectsDigestV1([10; 48]),
                    100,
                    &[key],
                )
                .expect("reserve");
        }
        let wal = directory.join(FASTSWAP_WAL_FILE);
        OpenOptions::new()
            .append(true)
            .open(&wal)
            .expect("wal")
            .write_all(&[0, 0, 0])
            .expect("torn tail");
        let store = FastSwapStore::open(&directory).expect("torn tail opens");
        assert_eq!(store.replay(&base).expect("replay"), live);
        drop(store);

        let mut bytes = fs::read(&wal).expect("read wal");
        bytes[8] ^= 1;
        fs::write(&wal, bytes).expect("corrupt wal");
        assert!(matches!(
            FastSwapStore::open(&directory),
            Err(FastSwapStoreError::CorruptWal { .. })
        ));
        fs::remove_file(directory.join(FASTSWAP_LOCK_FILE)).ok();
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn cross_process_lock_fails_closed() {
        let directory = test_dir("lock");
        let first = FastSwapStore::open(&directory).expect("first open");
        let lock_path = directory.join(FASTSWAP_LOCK_FILE);
        assert!(lock_path.is_file());
        assert!(matches!(
            FastSwapStore::open(&directory),
            Err(FastSwapStoreError::Conflict(_))
        ));
        drop(first);
        assert!(lock_path.is_file());
        let reopened = FastSwapStore::open(&directory).expect("reopen after lock release");
        drop(reopened);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn store_drop_releases_a_descriptor_inherited_by_a_child() {
        let directory = test_dir("inherited-lock");
        let first = FastSwapStore::open(&directory).expect("first open");
        let inherited_descriptor = first
            ._process_lock
            ._file
            .try_clone()
            .expect("model a descriptor inherited across fork");

        drop(first);
        let reopened = FastSwapStore::open(&directory)
            .expect("dropping the logical store owner must release its process lock");

        drop(reopened);
        drop(inherited_descriptor);
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn stale_pid_file_does_not_block_restart() {
        let directory = test_dir("stale-lock");
        fs::create_dir_all(&directory).expect("directory");
        fs::write(directory.join(FASTSWAP_LOCK_FILE), b"pid=999999999\n")
            .expect("stale lock marker");

        let first = FastSwapStore::open(&directory).expect("kernel lock ignores stale marker");
        assert!(matches!(
            FastSwapStore::open(&directory),
            Err(FastSwapStoreError::Conflict(_))
        ));
        drop(first);
        FastSwapStore::open(&directory).expect("kernel releases lock on close");
        fs::remove_dir_all(directory).expect("cleanup");
    }
}
