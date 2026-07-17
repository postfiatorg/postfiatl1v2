#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotExportOptions {
    pub data_dir: PathBuf,
    pub snapshot_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotImportOptions {
    pub data_dir: PathBuf,
    pub snapshot_dir: PathBuf,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotPublisherPublicKey {
    pub schema: String,
    pub publisher: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedSnapshotManifest {
    pub schema: String,
    pub publisher: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub source_build_git_revision: String,
    pub source_build_profile: String,
    pub last_certificate_id: Option<String>,
    pub mempool_policy: String,
    pub signer_material_included: bool,
    pub manifest: SnapshotManifest,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedSnapshotExportOptions {
    pub data_dir: PathBuf,
    pub snapshot_dir: PathBuf,
    pub publisher_key_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedSnapshotImportOptions {
    pub data_dir: PathBuf,
    pub snapshot_dir: PathBuf,
    pub trusted_publisher_key_file: PathBuf,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotPublisherKeyExportOptions {
    pub publisher_key_file: PathBuf,
    pub public_key_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentPublisherPublicKey {
    pub schema: String,
    pub publisher: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentPublisherPrivateKey {
    pub schema: String,
    pub purpose: String,
    pub publisher: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub private_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentManifest {
    pub schema: String,
    pub deployment_id: String,
    pub created_unix: u64,
    pub valid_from_unix: u64,
    pub valid_until_unix: u64,
    pub chain_id: String,
    pub genesis_hash: String,
    pub git_revision: String,
    pub binary_sha256: String,
    pub build_profile: String,
    pub build_features: Vec<String>,
    pub protocol_version: u32,
    pub rpc_schema: String,
    pub service_unit_sha256: String,
    pub environment_sha256: String,
    pub validator_bindings: Vec<DeploymentValidatorBinding>,
    pub topology_sha256: String,
    pub swap_circuit_metadata_sha256: String,
    pub private_egress_circuit_metadata_sha256: String,
    pub publisher: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentValidatorBinding {
    pub validator_id: String,
    pub services: Vec<DeploymentServiceArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentValidatorBindingsFile {
    pub schema: String,
    pub validators: Vec<DeploymentValidatorBindingFileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentValidatorBindingFileEntry {
    pub validator_id: String,
    pub services: Vec<DeploymentServiceBindingFileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentServiceBindingFileEntry {
    pub service_id: String,
    pub service_unit_file: PathBuf,
    pub environment_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentManifestCreateOptions {
    pub deployment_id: String,
    pub valid_from_unix: u64,
    pub valid_until_unix: u64,
    pub chain_id: String,
    pub genesis_hash: String,
    pub git_revision: String,
    pub binary_file: PathBuf,
    pub build_profile: String,
    pub build_features: Vec<String>,
    pub protocol_version: u32,
    pub rpc_schema: String,
    pub service_unit_file: PathBuf,
    pub environment_file: PathBuf,
    pub validator_bindings_file: PathBuf,
    pub topology_file: PathBuf,
    pub swap_circuit_metadata_file: PathBuf,
    pub private_egress_circuit_metadata_file: PathBuf,
    pub publisher_key_file: PathBuf,
    pub manifest_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentManifestVerifyOptions {
    pub manifest_file: PathBuf,
    pub trusted_publisher_key_file: PathBuf,
    pub now_unix: Option<u64>,
    pub validator_id: Option<String>,
    pub validator_bindings_file: Option<PathBuf>,
    pub runtime_binary_file: Option<PathBuf>,
    pub runtime_topology_file: Option<PathBuf>,
    pub runtime_swap_circuit_metadata_file: Option<PathBuf>,
    pub runtime_private_egress_circuit_metadata_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentPublisherKeyExportOptions {
    pub publisher_key_file: PathBuf,
    pub public_key_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentPublisherKeyCreateOptions {
    pub publisher_key_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentValidatorUnitsStageOptions {
    pub release_id: String,
    pub topology_file: PathBuf,
    pub binary_file: PathBuf,
    pub swap_circuit_metadata_file: PathBuf,
    pub private_egress_circuit_metadata_file: PathBuf,
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentValidatorUnitsStageRow {
    pub validator_id: String,
    pub rpc_unit_file: PathBuf,
    pub rpc_environment_file: PathBuf,
    pub transport_unit_file: PathBuf,
    pub transport_environment_file: PathBuf,
    pub runtime_bindings_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentValidatorUnitsStageReport {
    pub schema: String,
    pub release_id: String,
    pub rootfs_dir: PathBuf,
    pub binary_file: PathBuf,
    pub topology_file: PathBuf,
    pub swap_circuit_metadata_file: PathBuf,
    pub private_egress_circuit_metadata_file: PathBuf,
    pub signing_bindings_file: PathBuf,
    pub validators: Vec<DeploymentValidatorUnitsStageRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevKeyFile {
    pub algorithm_id: String,
    pub address: String,
    pub public_key_hex: String,
    pub private_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletBackupFile {
    pub schema: String,
    pub algorithm_id: String,
    pub kdf: String,
    pub derivation_domain: String,
    pub chain_id: String,
    pub account_index: u32,
    pub key_role: String,
    pub master_seed_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletKeyReport {
    pub schema: String,
    pub operation: String,
    pub algorithm_id: String,
    pub kdf: String,
    pub derivation_domain: String,
    pub chain_id: String,
    pub account_index: u32,
    pub key_role: String,
    pub address: String,
    pub public_key_hex: String,
    pub key_file: String,
    pub backup_file: Option<String>,
    pub private_key_material_redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletTestVectorReport {
    pub schema: String,
    pub fixture_warning: String,
    pub algorithm_id: String,
    pub kdf: String,
    pub derivation_domain: String,
    pub chain_id: String,
    pub validator_count: u32,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub account_index: u32,
    pub key_role: String,
    pub address: String,
    pub public_key_hex: String,
    pub transfer_signing_bytes_hex: String,
    pub transfer_signing_hash: String,
    pub signed_transfer: SignedTransfer,
    pub minimum_fee: u64,
    pub tx_id: String,
    pub signature_verified: bool,
    pub private_key_material_redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorKeyRecord {
    pub node_id: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub private_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorKeyFile {
    pub validators: Vec<ValidatorKeyRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalKeyValidationReport {
    pub schema: String,
    pub node_id: String,
    pub faucet_key_valid: bool,
    pub faucet_key_permissions_valid: bool,
    pub faucet_address: String,
    pub validator_keys_valid: bool,
    pub validator_key_permissions_valid: bool,
    pub validator_key_count: u32,
    pub required_validator_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistryRecord {
    pub node_id: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistry {
    pub validators: Vec<ValidatorRegistryRecord>,
}
