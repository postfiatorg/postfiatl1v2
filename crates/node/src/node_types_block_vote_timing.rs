#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlockVoteCreationTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub verify_block_log_ms: f64,
    pub store_init_ms: f64,
    pub read_genesis_ms: f64,
    pub target_ms: f64,
    pub key_read_ms: f64,
    pub key_validation_ms: f64,
    pub validator_membership_ms: f64,
    pub registry_read_ms: f64,
    pub registry_key_check_ms: f64,
    pub vote_lock_reservation_ms: f64,
    #[serde(default)]
    pub vote_lock_files_examined: u64,
    #[serde(default)]
    pub vote_lock_bytes_decoded: u64,
    #[serde(default)]
    pub vote_lock_migration_performed: bool,
    pub message_build_ms: f64,
    pub private_key_decode_ms: f64,
    pub mldsa_signing_ms: f64,
    pub vote_construct_ms: f64,
    pub vote_validation_ms: f64,
    pub json_serde_ms: f64,
    pub vote_file_write_ms: f64,
    pub process_spawn_ms: f64,
    pub target_breakdown: BlockVoteTargetTimingReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_work: Option<ApplyBatchStorageWorkReport>,
}

#[cfg(test)]
mod block_vote_timing_tests {
    use super::*;

    #[test]
    fn legacy_report_without_vote_lock_work_fields_deserializes_with_zero_defaults() {
        let mut value =
            serde_json::to_value(BlockVoteCreationTimingReport::default()).expect("serialize");
        let object = value.as_object_mut().expect("timing report object");
        object.remove("vote_lock_files_examined");
        object.remove("vote_lock_bytes_decoded");
        object.remove("vote_lock_migration_performed");

        let decoded: BlockVoteCreationTimingReport =
            serde_json::from_value(value).expect("deserialize legacy timing report");
        assert_eq!(decoded.vote_lock_files_examined, 0);
        assert_eq!(decoded.vote_lock_bytes_decoded, 0);
        assert!(!decoded.vote_lock_migration_performed);
    }
}
