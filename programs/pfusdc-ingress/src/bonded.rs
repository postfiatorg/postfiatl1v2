use alloy_consensus::Header;
use alloy_primitives::{hex, keccak256, Address, Bytes, B256, U256};
use alloy_rlp::{Decodable, Encodable};
use alloy_trie::{proof, Nibbles};
use serde::{Deserialize, Serialize};
use sp1_helios_primitives::types::{ContractStorage, StorageSlot};

use postfiat_types::{
    pfusdc_fast_ingress_deposit_key_v1, vault_bridge_deposit_evidence_root,
    vault_bridge_deposit_id, vault_bridge_route_binding, PfUsdcBondedIngressPublicValuesV1,
    PfUsdcBondedLifecyclePublicValuesV1, VaultBridgeDepositEvidence, VaultBridgeRouteProfileV1,
    NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1, PFUSDC_BONDED_INGRESS_PUBLIC_VALUES_SCHEMA_V1,
    PFUSDC_BONDED_LIFECYCLE_PUBLIC_VALUES_SCHEMA_V1, PFUSDC_BONDED_LIFECYCLE_UPDATE_CONFIRMED,
    PFUSDC_BONDED_LIFECYCLE_UPDATE_REVERTED, VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN,
};

use crate::{
    evm_address_text, hex32, nitro_assertion_hash, strip_0x, verify_account_code,
    verify_ethereum_finality, NitroAssertionWitnessV1, ARBITRUM_ONE_CHAIN_ID,
    ARBITRUM_ONE_ROLLUP_ADDRESS,
    ETHEREUM_MAINNET_CHAIN_ID, ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT,
};

pub const PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1: &str =
    "postfiat.pfusdc.bonded_ingress_witness.v1";
pub const PFUSDC_BONDED_INGRESS_POLICY_SCHEMA_V1: &str = "postfiat.pfusdc.bonded_ingress_policy.v1";
pub const PFUSDC_BONDED_INGRESS_PROGRAM_VERSION_V1: u32 = 1;
pub const PFUSDC_BONDED_ASSERTION_ADAPTER_ID_V1: &str = "arbitrum-one-bold-v1";
pub const PFUSDC_ETHEREUM_FINALITY_ADAPTER_ID_V1: &str = "sp1-helios-mainnet-v1";
pub const PFUSDC_ARBITRUM_HEADER_RULES_ID_V1: &str = "arbitrum-one-nitro-header-v1";

const MAX_ROLLUP_STORAGE_PROOFS: usize = 600;
const MAX_ASSERTION_PATH_LEN: usize = 256;
const MAX_VAULT_STORAGE_PROOFS: usize = 1;
const MAX_L2_HEADER_RLP_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcBondedIngressPolicyV1 {
    pub schema: String,
    pub deployment_manifest_hash: B256,
    pub assertion_protocol_adapter_id: String,
    pub ethereum_finality_adapter_id: String,
    pub l2_header_fork_rules_id: String,
    pub asset_id: String,
    pub cap_atoms: u64,
    pub age_margin_blocks: u64,
    pub ethereum_chain_id: u64,
    pub ethereum_genesis_validators_root: B256,
    pub arbitrum_chain_id: u64,
    pub arbitrum_rollup_address: Address,
    pub arbitrum_rollup_runtime_code_hash: B256,
    pub rollup_admin_implementation_address: Address,
    pub rollup_admin_implementation_runtime_code_hash: B256,
    pub rollup_user_implementation_address: Address,
    pub rollup_user_implementation_runtime_code_hash: B256,
    pub rollup_primary_implementation_slot: B256,
    pub rollup_secondary_implementation_slot: B256,
    pub rollup_paused_storage_slot: B256,
    pub rollup_chain_config_storage_slot: B256,
    pub rollup_assertion_config_storage_slot: B256,
    pub rollup_base_stake_storage_slot: B256,
    pub rollup_wasm_module_root_storage_slot: B256,
    pub rollup_challenge_config_storage_slot: B256,
    pub rollup_stake_token_storage_slot: B256,
    pub rollup_latest_confirmed_storage_slot: B256,
    pub rollup_assertions_mapping_slot: B256,
    pub rollup_staker_list_slot: B256,
    pub rollup_staker_map_slot: B256,
    pub minimum_stake_wei: U256,
    pub expected_confirm_period_blocks: u64,
    pub expected_validator_afk_blocks: u64,
    pub expected_challenge_grace_period_blocks: u64,
    pub expected_wasm_module_root: B256,
    pub challenge_manager_address: Address,
    pub challenge_manager_runtime_code_hash: B256,
    pub challenge_manager_implementation_slot: B256,
    pub challenge_manager_implementation_address: Address,
    pub challenge_manager_implementation_runtime_code_hash: B256,
    pub expected_stake_token: Address,
    pub rollup_validator_policy_storage_slot: B256,
    pub arbitrum_vault_address: Address,
    pub arbitrum_vault_runtime_code_hash: B256,
    pub vault_deposit_seen_mapping_slot: B256,
    pub arbitrum_token_address: Address,
    pub arbitrum_token_runtime_code_hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NitroAssertionConfigWitnessV1 {
    pub wasm_module_root: B256,
    pub required_stake: U256,
    pub challenge_manager: Address,
    pub confirm_period_blocks: u64,
    pub next_inbox_position: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BondedAssertionPathItemV1 {
    /// Assertion preimage authenticated by recomputing the mapping key.
    pub assertion: NitroAssertionWitnessV1,
    /// Configuration committed by this assertion's parent and used to create it.
    pub parent_config: NitroAssertionConfigWitnessV1,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PfUsdcBondedIngressWitnessV1 {
    pub schema: String,
    pub route_profile: VaultBridgeRouteProfileV1,
    pub policy: PfUsdcBondedIngressPolicyV1,
    pub helios: sp1_helios_primitives::types::ProofInputs,
    /// Rollup proxy proof containing the singleton staker list, active-staker
    /// record, and pending assertion record at the Ethereum-finalized state.
    pub rollup_storage: ContractStorage,
    pub rollup_admin_implementation_account: ContractStorage,
    pub rollup_user_implementation_account: ContractStorage,
    pub challenge_manager_account: ContractStorage,
    pub challenge_manager_implementation_account: ContractStorage,
    pub source_staker: Address,
    /// Complete oldest-to-newest path from latestConfirmed's direct child to
    /// the target. The path and singleton staker registry prove branch
    /// completeness without trusting event enumeration.
    pub assertion_path: Vec<BondedAssertionPathItemV1>,
    pub asserted_l2_header_rlp: Bytes,
    /// Vault account and the exact `depositSeen[deposit_id]` slot proof.
    pub asserted_l2_vault_account: ContractStorage,
    pub asserted_l2_token_account: ContractStorage,
    pub evidence: VaultBridgeDepositEvidence,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PfUsdcBondedConfirmationWitnessV1 {
    pub schema: String,
    pub route_profile: VaultBridgeRouteProfileV1,
    pub policy: PfUsdcBondedIngressPolicyV1,
    pub helios: sp1_helios_primitives::types::ProofInputs,
    pub rollup_storage: ContractStorage,
    pub rollup_admin_implementation_account: ContractStorage,
    pub rollup_user_implementation_account: ContractStorage,
    pub challenge_manager_account: ContractStorage,
    pub challenge_manager_implementation_account: ContractStorage,
    pub source_assertion_id: B256,
    /// Oldest-to-newest assertion preimages beginning with
    /// `source_assertion_id` itself and ending at latestConfirmed.
    pub confirmation_path: Vec<NitroAssertionWitnessV1>,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PfUsdcBondedReversionWitnessV1 {
    pub schema: String,
    pub route_profile: VaultBridgeRouteProfileV1,
    pub policy: PfUsdcBondedIngressPolicyV1,
    pub helios: sp1_helios_primitives::types::ProofInputs,
    pub rollup_storage: ContractStorage,
    pub rollup_admin_implementation_account: ContractStorage,
    pub rollup_user_implementation_account: ContractStorage,
    pub challenge_manager_account: ContractStorage,
    pub challenge_manager_implementation_account: ContractStorage,
    pub source_assertion_id: B256,
    /// Direct child of the prior latestConfirmed through the losing source.
    pub source_path: Vec<NitroAssertionWitnessV1>,
    /// A different direct child of the same prior latestConfirmed through the
    /// newer authenticated latestConfirmed winner.
    pub winning_path: Vec<NitroAssertionWitnessV1>,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PfUsdcBondedGuestInputV1 {
    Ingress(PfUsdcBondedIngressWitnessV1),
    Confirmation(PfUsdcBondedConfirmationWitnessV1),
    Reversion(PfUsdcBondedReversionWitnessV1),
}

pub fn verify_bonded_ingress_witness_v1(
    witness: &PfUsdcBondedIngressWitnessV1,
) -> Result<PfUsdcBondedIngressPublicValuesV1, String> {
    validate_bounds(witness)?;
    verify_route_and_policy(witness)?;
    witness.evidence.validate()?;

    let (
        prior_root,
        prior_slot,
        final_root,
        final_slot,
        ethereum_state_root,
        finalized_execution_block,
        finalized_execution_block_hash,
    ) = verify_ethereum_finality(&witness.helios, &confirmed_policy_view(&witness.policy))?;
    let assertion = witness
        .assertion_path
        .last()
        .ok_or_else(|| "bonded assertion path is empty".to_string())?;
    let branch = verify_bonded_assertion_path(witness, ethereum_state_root)?;
    let source_assertion_id = branch.source_assertion_id;
    let created_at_l1_block = branch.source_created_at_l1_block;
    let (l2_state_root, l2_block_number) = verify_asserted_l2_state(witness)?;
    let deposit_seen_slot = verify_deposit_inclusion(witness, l2_state_root)?;

    let profile_hash = witness.route_profile.profile_hash()?;
    let evidence_root = vault_bridge_deposit_evidence_root(&witness.evidence)?;
    let mut values = PfUsdcBondedIngressPublicValuesV1 {
        schema: PFUSDC_BONDED_INGRESS_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        proof_program_version: PFUSDC_BONDED_INGRESS_PROGRAM_VERSION_V1,
        pftl_chain_id: witness.pftl_chain_id.clone(),
        pftl_genesis_hash: witness.pftl_genesis_hash.clone(),
        pftl_protocol_version: witness.pftl_protocol_version,
        route_id: witness.route_profile.route_id.clone(),
        route_profile_hash: profile_hash,
        route_epoch: u64::from(witness.route_profile.route_epoch),
        manifest_hash: hex32(witness.policy.deployment_manifest_hash),
        verifier_policy_hash: bonded_ingress_policy_hash_v1(&witness.policy),
        cap_atoms: witness.policy.cap_atoms,
        age_margin_blocks: witness.policy.age_margin_blocks,
        ethereum_chain_id: witness.policy.ethereum_chain_id,
        prior_ethereum_finalized_beacon_root: hex32(prior_root),
        prior_ethereum_finalized_slot: prior_slot,
        ethereum_finalized_beacon_root: hex32(final_root),
        ethereum_finalized_slot: final_slot,
        ethereum_finalized_execution_block_number: finalized_execution_block,
        ethereum_finalized_execution_block_hash: hex32(finalized_execution_block_hash),
        arbitrum_chain_id: witness.policy.arbitrum_chain_id,
        arbitrum_rollup_address: evm_address_text(witness.policy.arbitrum_rollup_address),
        arbitrum_rollup_runtime_code_hash: hex32(witness.policy.arbitrum_rollup_runtime_code_hash),
        assertion_protocol_adapter_id: witness.policy.assertion_protocol_adapter_id.clone(),
        latest_confirmed_assertion_id: hex32(branch.latest_confirmed_assertion_id),
        source_assertion_id: hex32(source_assertion_id),
        source_assertion_parent_id: hex32(assertion.assertion.parent_assertion_hash),
        source_assertion_created_at_l1_block: created_at_l1_block,
        source_assertion_confirmation_period_blocks: assertion.parent_config.confirm_period_blocks,
        source_assertion_l2_block_number: l2_block_number,
        source_assertion_l2_block_hash: hex32(assertion.assertion.block_hash),
        source_assertion_l2_state_root: hex32(l2_state_root),
        source_assertion_send_root: hex32(assertion.assertion.send_root),
        source_staker: evm_address_text(witness.source_staker),
        vault_address: witness.evidence.vault_address.clone(),
        vault_runtime_code_hash: hex32(witness.policy.arbitrum_vault_runtime_code_hash),
        token_address: witness.evidence.token_address.clone(),
        token_runtime_code_hash: hex32(witness.policy.arbitrum_token_runtime_code_hash),
        asset_id: witness.policy.asset_id.clone(),
        depositor: witness.evidence.depositor.clone(),
        pftl_recipient: witness.evidence.pftl_recipient.clone(),
        pftl_recipient_hash: witness.evidence.pftl_recipient_hash.clone(),
        amount_atoms: witness.evidence.amount_atoms,
        deposit_nonce: witness.evidence.nonce.clone(),
        route_binding: witness.evidence.route_binding.clone(),
        deposit_id: witness.evidence.deposit_id.clone(),
        deposit_key: pfusdc_fast_ingress_deposit_key_v1(
            witness.policy.arbitrum_chain_id,
            &witness.evidence.vault_address,
            &witness.evidence.deposit_id,
        )?,
        deposit_seen_storage_slot: hex32(deposit_seen_slot),
        evidence_root,
        public_values_commitment: String::new(),
    };
    values.seal()?;
    values.validate()?;
    Ok(values)
}

pub fn verify_bonded_confirmation_witness_v1(
    witness: &PfUsdcBondedConfirmationWitnessV1,
) -> Result<PfUsdcBondedLifecyclePublicValuesV1, String> {
    validate_confirmation_bounds(witness)?;
    verify_confirmation_route_and_policy(witness)?;
    let (
        prior_root,
        prior_slot,
        final_root,
        final_slot,
        ethereum_state_root,
        finalized_execution_block,
        finalized_execution_block_hash,
    ) = verify_ethereum_finality(&witness.helios, &confirmed_policy_view(&witness.policy))?;
    let latest_confirmed = verify_confirmed_ancestry(witness, ethereum_state_root)?;
    let latest = witness
        .confirmation_path
        .last()
        .ok_or_else(|| "bonded confirmation path is empty".to_string())?;
    let mut values = PfUsdcBondedLifecyclePublicValuesV1 {
        schema: PFUSDC_BONDED_LIFECYCLE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        proof_program_version: PFUSDC_BONDED_INGRESS_PROGRAM_VERSION_V1,
        pftl_chain_id: witness.pftl_chain_id.clone(),
        pftl_genesis_hash: witness.pftl_genesis_hash.clone(),
        pftl_protocol_version: witness.pftl_protocol_version,
        route_profile_hash: witness.route_profile.profile_hash()?,
        route_epoch: u64::from(witness.route_profile.route_epoch),
        manifest_hash: hex32(witness.policy.deployment_manifest_hash),
        verifier_policy_hash: bonded_ingress_policy_hash_v1(&witness.policy),
        ethereum_chain_id: witness.policy.ethereum_chain_id,
        prior_ethereum_finalized_beacon_root: hex32(prior_root),
        prior_ethereum_finalized_slot: prior_slot,
        ethereum_finalized_beacon_root: hex32(final_root),
        ethereum_finalized_slot: final_slot,
        ethereum_finalized_execution_block_number: finalized_execution_block,
        ethereum_finalized_execution_block_hash: hex32(finalized_execution_block_hash),
        arbitrum_chain_id: witness.policy.arbitrum_chain_id,
        arbitrum_rollup_address: evm_address_text(witness.policy.arbitrum_rollup_address),
        arbitrum_rollup_runtime_code_hash: hex32(witness.policy.arbitrum_rollup_runtime_code_hash),
        source_assertion_id: hex32(witness.source_assertion_id),
        latest_confirmed_assertion_id: hex32(latest_confirmed),
        latest_confirmed_l2_block_hash: hex32(latest.block_hash),
        latest_confirmed_send_root: hex32(latest.send_root),
        common_ancestor_assertion_id: hex32(witness.source_assertion_id),
        update_kind: PFUSDC_BONDED_LIFECYCLE_UPDATE_CONFIRMED.to_string(),
        public_values_commitment: String::new(),
    };
    values.seal()?;
    values.validate()?;
    Ok(values)
}

pub fn verify_bonded_reversion_witness_v1(
    witness: &PfUsdcBondedReversionWitnessV1,
) -> Result<PfUsdcBondedLifecyclePublicValuesV1, String> {
    validate_reversion_bounds(witness)?;
    verify_lifecycle_profile_policy(&witness.schema, &witness.route_profile, &witness.policy)?;
    let (
        prior_root,
        prior_slot,
        final_root,
        final_slot,
        ethereum_state_root,
        finalized_execution_block,
        finalized_execution_block_hash,
    ) = verify_ethereum_finality(&witness.helios, &confirmed_policy_view(&witness.policy))?;
    let latest_confirmed = verify_lifecycle_rollup_snapshot(
        &witness.policy,
        &witness.rollup_storage,
        &witness.rollup_admin_implementation_account,
        &witness.rollup_user_implementation_account,
        &witness.challenge_manager_account,
        &witness.challenge_manager_implementation_account,
        ethereum_state_root,
    )?;
    if witness.source_path.is_empty() || witness.winning_path.is_empty() {
        return Err("bonded reversion paths must be nonempty".to_string());
    }
    verify_hash_chain(&witness.source_path)?;
    verify_hash_chain(&witness.winning_path)?;
    if nitro_assertion_hash(witness.source_path.last().unwrap()) != witness.source_assertion_id
        || nitro_assertion_hash(witness.winning_path.last().unwrap()) != latest_confirmed
    {
        return Err("bonded reversion paths do not terminate at source and winner".to_string());
    }
    let common_ancestor = witness.source_path[0].parent_assertion_hash;
    if common_ancestor == B256::ZERO
        || witness.winning_path[0].parent_assertion_hash != common_ancestor
        || nitro_assertion_hash(&witness.source_path[0])
            == nitro_assertion_hash(&witness.winning_path[0])
    {
        return Err("bonded reversion does not prove divergent sibling branches".to_string());
    }
    let latest = witness.winning_path.last().unwrap();
    let mut values = PfUsdcBondedLifecyclePublicValuesV1 {
        schema: PFUSDC_BONDED_LIFECYCLE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        proof_program_version: PFUSDC_BONDED_INGRESS_PROGRAM_VERSION_V1,
        pftl_chain_id: witness.pftl_chain_id.clone(),
        pftl_genesis_hash: witness.pftl_genesis_hash.clone(),
        pftl_protocol_version: witness.pftl_protocol_version,
        route_profile_hash: witness.route_profile.profile_hash()?,
        route_epoch: u64::from(witness.route_profile.route_epoch),
        manifest_hash: hex32(witness.policy.deployment_manifest_hash),
        verifier_policy_hash: bonded_ingress_policy_hash_v1(&witness.policy),
        ethereum_chain_id: witness.policy.ethereum_chain_id,
        prior_ethereum_finalized_beacon_root: hex32(prior_root),
        prior_ethereum_finalized_slot: prior_slot,
        ethereum_finalized_beacon_root: hex32(final_root),
        ethereum_finalized_slot: final_slot,
        ethereum_finalized_execution_block_number: finalized_execution_block,
        ethereum_finalized_execution_block_hash: hex32(finalized_execution_block_hash),
        arbitrum_chain_id: witness.policy.arbitrum_chain_id,
        arbitrum_rollup_address: evm_address_text(witness.policy.arbitrum_rollup_address),
        arbitrum_rollup_runtime_code_hash: hex32(witness.policy.arbitrum_rollup_runtime_code_hash),
        source_assertion_id: hex32(witness.source_assertion_id),
        latest_confirmed_assertion_id: hex32(latest_confirmed),
        latest_confirmed_l2_block_hash: hex32(latest.block_hash),
        latest_confirmed_send_root: hex32(latest.send_root),
        common_ancestor_assertion_id: hex32(common_ancestor),
        update_kind: PFUSDC_BONDED_LIFECYCLE_UPDATE_REVERTED.to_string(),
        public_values_commitment: String::new(),
    };
    values.seal()?;
    values.validate()?;
    Ok(values)
}

fn verify_hash_chain(path: &[NitroAssertionWitnessV1]) -> Result<(), String> {
    for assertion in path {
        if assertion.machine_status != 1 {
            return Err("bonded lifecycle path contains unfinished execution".to_string());
        }
    }
    for pair in path.windows(2) {
        if pair[1].parent_assertion_hash != nitro_assertion_hash(&pair[0]) {
            return Err("bonded lifecycle path has a broken parent link".to_string());
        }
    }
    Ok(())
}

fn verify_confirmation_route_and_policy(
    witness: &PfUsdcBondedConfirmationWitnessV1,
) -> Result<(), String> {
    verify_lifecycle_profile_policy(&witness.schema, &witness.route_profile, &witness.policy)
}

fn verify_lifecycle_profile_policy(
    witness_schema: &str,
    profile: &VaultBridgeRouteProfileV1,
    policy: &PfUsdcBondedIngressPolicyV1,
) -> Result<(), String> {
    profile.validate()?;
    if witness_schema != PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1
        || policy.schema != PFUSDC_BONDED_INGRESS_POLICY_SCHEMA_V1
        || profile.verifier_kind != NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
        || profile.source_chain_id != policy.arbitrum_chain_id
        || profile.vault_address != evm_address_text(policy.arbitrum_vault_address)
        || strip_0x(&profile.vault_runtime_code_hash)
            != hex32(policy.arbitrum_vault_runtime_code_hash)
        || profile.token_address != evm_address_text(policy.arbitrum_token_address)
        || strip_0x(&profile.token_runtime_code_hash)
            != hex32(policy.arbitrum_token_runtime_code_hash)
        || profile.asset_id != policy.asset_id
        || policy.ethereum_chain_id != ETHEREUM_MAINNET_CHAIN_ID
        || policy.ethereum_genesis_validators_root != ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT
        || policy.arbitrum_chain_id != ARBITRUM_ONE_CHAIN_ID
        || policy.arbitrum_rollup_address != ARBITRUM_ONE_ROLLUP_ADDRESS
        || policy.deployment_manifest_hash == B256::ZERO
        || policy.assertion_protocol_adapter_id != PFUSDC_BONDED_ASSERTION_ADAPTER_ID_V1
        || policy.ethereum_finality_adapter_id != PFUSDC_ETHEREUM_FINALITY_ADAPTER_ID_V1
        || policy.l2_header_fork_rules_id != PFUSDC_ARBITRUM_HEADER_RULES_ID_V1
    {
        return Err("bonded lifecycle route/policy binding mismatch".to_string());
    }
    Ok(())
}

fn verify_confirmed_ancestry(
    witness: &PfUsdcBondedConfirmationWitnessV1,
    ethereum_state_root: B256,
) -> Result<B256, String> {
    let policy = &witness.policy;
    if witness.source_assertion_id == B256::ZERO
        || witness.confirmation_path.is_empty()
        || witness.confirmation_path.len() > MAX_ASSERTION_PATH_LEN
    {
        return Err("bonded confirmation ancestry is out of bounds".to_string());
    }
    for (storage, address, code_hash, label) in [
        (
            &witness.rollup_admin_implementation_account,
            policy.rollup_admin_implementation_address,
            policy.rollup_admin_implementation_runtime_code_hash,
            "Rollup admin implementation",
        ),
        (
            &witness.rollup_user_implementation_account,
            policy.rollup_user_implementation_address,
            policy.rollup_user_implementation_runtime_code_hash,
            "Rollup user implementation",
        ),
    ] {
        verify_account_code(ethereum_state_root, storage, address, code_hash, label)?;
    }
    verify_challenge_manager_implementation(
        ethereum_state_root,
        policy,
        &witness.challenge_manager_account,
        &witness.challenge_manager_implementation_account,
    )?;
    let latest_confirmed = B256::from(
        slot_value(
            &witness.rollup_storage,
            policy.rollup_latest_confirmed_storage_slot,
        )
        .ok_or_else(|| "latestConfirmed proof is absent".to_string())?
        .to_be_bytes::<32>(),
    );
    if latest_confirmed == B256::ZERO {
        return Err("latestConfirmed is zero".to_string());
    }
    let confirmed_base = mapping_slot(latest_confirmed, policy.rollup_assertions_mapping_slot);
    let expected = vec![
        policy.rollup_primary_implementation_slot,
        policy.rollup_secondary_implementation_slot,
        policy.rollup_paused_storage_slot,
        policy.rollup_chain_config_storage_slot,
        policy.rollup_assertion_config_storage_slot,
        policy.rollup_base_stake_storage_slot,
        policy.rollup_wasm_module_root_storage_slot,
        policy.rollup_challenge_config_storage_slot,
        policy.rollup_stake_token_storage_slot,
        policy.rollup_latest_confirmed_storage_slot,
        policy.rollup_validator_policy_storage_slot,
        confirmed_base,
        add_slot(confirmed_base, 1)?,
    ];
    if witness.rollup_storage.address != policy.arbitrum_rollup_address
        || witness.rollup_storage.value.code_hash != policy.arbitrum_rollup_runtime_code_hash
        || witness.rollup_storage.storage_slots.len() != expected.len()
    {
        return Err("bonded lifecycle RollupCore account proof mismatch".to_string());
    }
    let verified = verify_bonded_storage_slot_proofs(ethereum_state_root, &witness.rollup_storage)
        .map_err(|error| format!("invalid bonded lifecycle RollupCore proof: {error}"))?;
    if verified.len() != expected.len()
        || expected
            .iter()
            .any(|slot| slot_value(&witness.rollup_storage, *slot).is_none())
    {
        return Err("bonded lifecycle RollupCore proof omitted a required slot".to_string());
    }
    verify_pinned_rollup_configuration(&witness.rollup_storage, policy)?;
    let confirmed =
        assertion_record_from_storage(&witness.rollup_storage, policy, latest_confirmed)?;
    if confirmed.status != 2 || confirmed.created_at_block == 0 {
        return Err("latestConfirmed does not authenticate a confirmed assertion".to_string());
    }
    let first = witness.confirmation_path.first().unwrap();
    if nitro_assertion_hash(first) != witness.source_assertion_id || first.machine_status != 1 {
        return Err("confirmation path does not begin with the source assertion".to_string());
    }
    let mut parent = witness.source_assertion_id;
    for assertion in witness.confirmation_path.iter().skip(1) {
        if assertion.parent_assertion_hash != parent || assertion.machine_status != 1 {
            return Err("confirmed assertion ancestry is broken".to_string());
        }
        parent = nitro_assertion_hash(assertion);
    }
    if nitro_assertion_hash(witness.confirmation_path.last().unwrap()) != latest_confirmed {
        return Err("source assertion is not an ancestor of latestConfirmed".to_string());
    }
    Ok(latest_confirmed)
}

fn verify_lifecycle_rollup_snapshot(
    policy: &PfUsdcBondedIngressPolicyV1,
    rollup_storage: &ContractStorage,
    admin_implementation: &ContractStorage,
    user_implementation: &ContractStorage,
    challenge_manager: &ContractStorage,
    challenge_manager_implementation: &ContractStorage,
    ethereum_state_root: B256,
) -> Result<B256, String> {
    for (storage, address, code_hash, label) in [
        (
            admin_implementation,
            policy.rollup_admin_implementation_address,
            policy.rollup_admin_implementation_runtime_code_hash,
            "Rollup admin implementation",
        ),
        (
            user_implementation,
            policy.rollup_user_implementation_address,
            policy.rollup_user_implementation_runtime_code_hash,
            "Rollup user implementation",
        ),
    ] {
        verify_account_code(ethereum_state_root, storage, address, code_hash, label)?;
    }
    verify_challenge_manager_implementation(
        ethereum_state_root,
        policy,
        challenge_manager,
        challenge_manager_implementation,
    )?;
    let latest_confirmed = B256::from(
        slot_value(rollup_storage, policy.rollup_latest_confirmed_storage_slot)
            .ok_or_else(|| "latestConfirmed proof is absent".to_string())?
            .to_be_bytes::<32>(),
    );
    if latest_confirmed == B256::ZERO {
        return Err("latestConfirmed is zero".to_string());
    }
    let confirmed_base = mapping_slot(latest_confirmed, policy.rollup_assertions_mapping_slot);
    let expected = vec![
        policy.rollup_primary_implementation_slot,
        policy.rollup_secondary_implementation_slot,
        policy.rollup_paused_storage_slot,
        policy.rollup_chain_config_storage_slot,
        policy.rollup_assertion_config_storage_slot,
        policy.rollup_base_stake_storage_slot,
        policy.rollup_wasm_module_root_storage_slot,
        policy.rollup_challenge_config_storage_slot,
        policy.rollup_stake_token_storage_slot,
        policy.rollup_latest_confirmed_storage_slot,
        policy.rollup_validator_policy_storage_slot,
        confirmed_base,
        add_slot(confirmed_base, 1)?,
    ];
    if rollup_storage.address != policy.arbitrum_rollup_address
        || rollup_storage.value.code_hash != policy.arbitrum_rollup_runtime_code_hash
        || rollup_storage.storage_slots.len() != expected.len()
    {
        return Err("bonded lifecycle RollupCore account proof mismatch".to_string());
    }
    let verified = verify_bonded_storage_slot_proofs(ethereum_state_root, rollup_storage)
        .map_err(|error| format!("invalid bonded lifecycle RollupCore proof: {error}"))?;
    if verified.len() != expected.len()
        || expected
            .iter()
            .any(|slot| slot_value(rollup_storage, *slot).is_none())
    {
        return Err("bonded lifecycle RollupCore proof omitted a required slot".to_string());
    }
    verify_pinned_rollup_configuration(rollup_storage, policy)?;
    let confirmed = assertion_record_from_storage(rollup_storage, policy, latest_confirmed)?;
    if confirmed.status != 2 || confirmed.created_at_block == 0 {
        return Err("latestConfirmed does not authenticate a confirmed assertion".to_string());
    }
    Ok(latest_confirmed)
}

fn confirmed_policy_view(
    policy: &PfUsdcBondedIngressPolicyV1,
) -> crate::PfUsdcIngressProofPolicyV2 {
    crate::PfUsdcIngressProofPolicyV2 {
        schema: crate::PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V2.to_string(),
        ethereum_chain_id: policy.ethereum_chain_id,
        ethereum_genesis_validators_root: policy.ethereum_genesis_validators_root,
        arbitrum_chain_id: policy.arbitrum_chain_id,
        arbitrum_rollup_address: policy.arbitrum_rollup_address,
        arbitrum_rollup_runtime_code_hash: policy.arbitrum_rollup_runtime_code_hash,
        rollup_latest_confirmed_storage_slot: crate::NITRO_LATEST_CONFIRMED_STORAGE_SLOT,
        arbitrum_vault_address: policy.arbitrum_vault_address,
        arbitrum_vault_runtime_code_hash: policy.arbitrum_vault_runtime_code_hash,
        arbitrum_token_address: policy.arbitrum_token_address,
        arbitrum_token_runtime_code_hash: policy.arbitrum_token_runtime_code_hash,
        ethereum_ingress_anchor_address: Address::ZERO,
        ethereum_ingress_anchor_runtime_code_hash: B256::ZERO,
    }
}

fn verify_route_and_policy(witness: &PfUsdcBondedIngressWitnessV1) -> Result<(), String> {
    let profile = &witness.route_profile;
    let policy = &witness.policy;
    profile.validate()?;
    if witness.schema != PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1
        || policy.schema != PFUSDC_BONDED_INGRESS_POLICY_SCHEMA_V1
        || policy.deployment_manifest_hash == B256::ZERO
        || policy.assertion_protocol_adapter_id != PFUSDC_BONDED_ASSERTION_ADAPTER_ID_V1
        || policy.ethereum_finality_adapter_id != PFUSDC_ETHEREUM_FINALITY_ADAPTER_ID_V1
        || policy.l2_header_fork_rules_id != PFUSDC_ARBITRUM_HEADER_RULES_ID_V1
        || policy.asset_id.is_empty()
        || policy.cap_atoms == 0
        || policy.age_margin_blocks < 64
        || profile.verifier_kind != NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
        || profile.evidence_tier != VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN
        || profile.source_chain_id != policy.arbitrum_chain_id
        || profile.vault_address != evm_address_text(policy.arbitrum_vault_address)
        || strip_0x(&profile.vault_runtime_code_hash)
            != hex32(policy.arbitrum_vault_runtime_code_hash)
        || profile.token_address != evm_address_text(policy.arbitrum_token_address)
        || strip_0x(&profile.token_runtime_code_hash)
            != hex32(policy.arbitrum_token_runtime_code_hash)
        || profile.asset_id != policy.asset_id
    {
        return Err("bonded ingress route/policy binding mismatch".to_string());
    }
    let expected_binding =
        vault_bridge_route_binding(&profile.profile_hash()?, profile.route_epoch)?;
    if witness.evidence.route_binding != expected_binding
        || witness.evidence.source_chain_id != policy.arbitrum_chain_id
        || witness.evidence.vault_address != profile.vault_address
        || witness.evidence.token_address != profile.token_address
    {
        return Err("bonded ingress evidence does not match governed route".to_string());
    }
    if policy.ethereum_chain_id != ETHEREUM_MAINNET_CHAIN_ID
        || policy.ethereum_genesis_validators_root != ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT
        || policy.arbitrum_chain_id != ARBITRUM_ONE_CHAIN_ID
        || policy.arbitrum_rollup_address != ARBITRUM_ONE_ROLLUP_ADDRESS
        || policy.rollup_primary_implementation_slot
            != B256::new(hex!(
                "360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"
            ))
        || policy.rollup_secondary_implementation_slot
            != B256::new(hex!(
                "2b1dbce74324248c222f0ec2d5ed7bd323cfc425b336f0253c5ccfda7265546d"
            ))
        || policy.rollup_paused_storage_slot != slot(51)
        || policy.rollup_chain_config_storage_slot != slot(101)
        || policy.rollup_assertion_config_storage_slot != slot(102)
        || policy.rollup_base_stake_storage_slot != slot(103)
        || policy.rollup_wasm_module_root_storage_slot != slot(104)
        || policy.rollup_challenge_config_storage_slot != slot(105)
        || policy.rollup_stake_token_storage_slot != slot(112)
        || policy.rollup_latest_confirmed_storage_slot != slot(116)
        || policy.rollup_assertions_mapping_slot != slot(117)
        || policy.rollup_staker_list_slot != slot(118)
        || policy.rollup_staker_map_slot != slot(119)
        || policy.rollup_validator_policy_storage_slot != slot(123)
        || policy.vault_deposit_seen_mapping_slot != slot(3)
        || policy.minimum_stake_wei.is_zero()
        || policy.expected_confirm_period_blocks == 0
        || policy.expected_validator_afk_blocks == 0
        || policy.expected_challenge_grace_period_blocks == 0
        || policy.expected_wasm_module_root == B256::ZERO
        || policy.challenge_manager_address == Address::ZERO
        || policy.challenge_manager_implementation_slot
            != B256::new(hex!(
                "360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"
            ))
        || policy.challenge_manager_implementation_address == Address::ZERO
        || policy.challenge_manager_implementation_runtime_code_hash == B256::ZERO
        || policy.expected_stake_token == Address::ZERO
    {
        return Err(
            "bonded ingress policy is not the pinned Ethereum/Arbitrum-One layout".to_string(),
        );
    }
    Ok(())
}

struct VerifiedBondedBranch {
    latest_confirmed_assertion_id: B256,
    source_assertion_id: B256,
    source_created_at_l1_block: u64,
}

#[derive(Clone, Copy)]
struct AssertionRecord {
    first_child_block: u64,
    second_child_block: u64,
    created_at_block: u64,
    is_first_child: bool,
    status: u8,
    config_hash: B256,
}

fn verify_bonded_assertion_path(
    witness: &PfUsdcBondedIngressWitnessV1,
    ethereum_state_root: B256,
) -> Result<VerifiedBondedBranch, String> {
    let policy = &witness.policy;
    if witness.assertion_path.is_empty() || witness.assertion_path.len() > MAX_ASSERTION_PATH_LEN {
        return Err("bonded assertion path length is out of bounds".to_string());
    }
    verify_account_code(
        ethereum_state_root,
        &witness.rollup_admin_implementation_account,
        policy.rollup_admin_implementation_address,
        policy.rollup_admin_implementation_runtime_code_hash,
        "Rollup admin implementation",
    )?;
    verify_account_code(
        ethereum_state_root,
        &witness.rollup_user_implementation_account,
        policy.rollup_user_implementation_address,
        policy.rollup_user_implementation_runtime_code_hash,
        "Rollup user implementation",
    )?;
    verify_challenge_manager_implementation(
        ethereum_state_root,
        policy,
        &witness.challenge_manager_account,
        &witness.challenge_manager_implementation_account,
    )?;

    let staker_base = address_mapping_slot(witness.source_staker, policy.rollup_staker_map_slot);
    let staker_list_item = keccak256(policy.rollup_staker_list_slot.as_slice());
    let mut expected = vec![
        policy.rollup_primary_implementation_slot,
        policy.rollup_secondary_implementation_slot,
        policy.rollup_paused_storage_slot,
        policy.rollup_chain_config_storage_slot,
        policy.rollup_assertion_config_storage_slot,
        policy.rollup_base_stake_storage_slot,
        policy.rollup_wasm_module_root_storage_slot,
        policy.rollup_challenge_config_storage_slot,
        policy.rollup_stake_token_storage_slot,
        policy.rollup_latest_confirmed_storage_slot,
        policy.rollup_staker_list_slot,
        staker_list_item,
        staker_base,
        add_slot(staker_base, 1)?,
        add_slot(staker_base, 2)?,
        policy.rollup_validator_policy_storage_slot,
    ];
    let latest_confirmed = B256::from(
        slot_value(
            &witness.rollup_storage,
            policy.rollup_latest_confirmed_storage_slot,
        )
        .ok_or_else(|| "latestConfirmed proof is absent".to_string())?
        .to_be_bytes::<32>(),
    );
    if latest_confirmed == B256::ZERO {
        return Err("latestConfirmed is zero".to_string());
    }
    let confirmed_base = mapping_slot(latest_confirmed, policy.rollup_assertions_mapping_slot);
    expected.push(confirmed_base);
    expected.push(add_slot(confirmed_base, 1)?);
    for item in &witness.assertion_path {
        let assertion_id = nitro_assertion_hash(&item.assertion);
        let base = mapping_slot(assertion_id, policy.rollup_assertions_mapping_slot);
        expected.push(base);
        expected.push(add_slot(base, 1)?);
    }
    if witness.rollup_storage.address != policy.arbitrum_rollup_address
        || witness.rollup_storage.value.code_hash != policy.arbitrum_rollup_runtime_code_hash
        || witness.rollup_storage.storage_slots.len() != expected.len()
    {
        return Err("bonded RollupCore account proof does not match policy".to_string());
    }
    let verified = verify_bonded_storage_slot_proofs(ethereum_state_root, &witness.rollup_storage)
        .map_err(|error| format!("invalid bonded RollupCore proof: {error}"))?;
    if verified.len() != expected.len() {
        return Err("bonded RollupCore proof omitted a required slot".to_string());
    }
    for key in &expected {
        if slot_value(&witness.rollup_storage, *key).is_none() {
            return Err("bonded RollupCore proof contains wrong or duplicate slots".to_string());
        }
    }
    if address_from_word(
        slot_value(
            &witness.rollup_storage,
            policy.rollup_primary_implementation_slot,
        )
        .unwrap(),
    ) != policy.rollup_admin_implementation_address
        || address_from_word(
            slot_value(
                &witness.rollup_storage,
                policy.rollup_secondary_implementation_slot,
            )
            .unwrap(),
        ) != policy.rollup_user_implementation_address
        || slot_value(&witness.rollup_storage, policy.rollup_paused_storage_slot)
            != Some(U256::ZERO)
        || slot_value(
            &witness.rollup_storage,
            policy.rollup_chain_config_storage_slot,
        ) != Some(U256::from(policy.arbitrum_chain_id))
    {
        return Err("Rollup proxy, chain, or pause configuration mismatch".to_string());
    }
    let assertion_config = slot_value(
        &witness.rollup_storage,
        policy.rollup_assertion_config_storage_slot,
    )
    .unwrap();
    let confirm_period = (assertion_config & U256::from(u64::MAX)).to::<u64>();
    let validator_afk = ((assertion_config >> 64_usize) & U256::from(u64::MAX)).to::<u64>();
    let challenge_config = slot_value(
        &witness.rollup_storage,
        policy.rollup_challenge_config_storage_slot,
    )
    .unwrap();
    let challenge_grace = ((challenge_config >> 160_usize) & U256::from(u64::MAX)).to::<u64>();
    if confirm_period != policy.expected_confirm_period_blocks
        || validator_afk != policy.expected_validator_afk_blocks
        || slot_value(
            &witness.rollup_storage,
            policy.rollup_base_stake_storage_slot,
        ) != Some(policy.minimum_stake_wei)
        || B256::from(
            slot_value(
                &witness.rollup_storage,
                policy.rollup_wasm_module_root_storage_slot,
            )
            .unwrap()
            .to_be_bytes::<32>(),
        ) != policy.expected_wasm_module_root
        || address_from_word(challenge_config) != policy.challenge_manager_address
        || challenge_grace != policy.expected_challenge_grace_period_blocks
        || address_from_word(
            slot_value(
                &witness.rollup_storage,
                policy.rollup_stake_token_storage_slot,
            )
            .unwrap(),
        ) != policy.expected_stake_token
        || slot_value(
            &witness.rollup_storage,
            policy.rollup_validator_policy_storage_slot,
        ) != Some(U256::from(1))
    {
        return Err(format!(
            "RollupCore/BoLD economic or validator configuration mismatch: confirm={confirm_period}/{} afk={validator_afk}/{} base_stake={:?}/{:?} wasm={:?}/{:?} challenge_manager={:?}/{:?} challenge_grace={challenge_grace}/{} stake_token={:?}/{:?} validator_policy={:?}",
            policy.expected_confirm_period_blocks,
            policy.expected_validator_afk_blocks,
            slot_value(
                &witness.rollup_storage,
                policy.rollup_base_stake_storage_slot,
            ),
            policy.minimum_stake_wei,
            slot_value(
                &witness.rollup_storage,
                policy.rollup_wasm_module_root_storage_slot,
            ),
            policy.expected_wasm_module_root,
            address_from_word(challenge_config),
            policy.challenge_manager_address,
            policy.expected_challenge_grace_period_blocks,
            address_from_word(
                slot_value(
                    &witness.rollup_storage,
                    policy.rollup_stake_token_storage_slot,
                )
                .unwrap(),
            ),
            policy.expected_stake_token,
            slot_value(
                &witness.rollup_storage,
                policy.rollup_validator_policy_storage_slot,
            ),
        ));
    }
    if slot_value(&witness.rollup_storage, policy.rollup_staker_list_slot) != Some(U256::from(1))
        || address_from_word(slot_value(&witness.rollup_storage, staker_list_item).unwrap())
            != witness.source_staker
    {
        return Err(
            "bonded ingress currently requires the canonical singleton staker set".to_string(),
        );
    }
    let amount_staked = slot_value(&witness.rollup_storage, staker_base).unwrap();
    let latest_staked = B256::from(
        slot_value(&witness.rollup_storage, add_slot(staker_base, 1)?)
            .unwrap()
            .to_be_bytes::<32>(),
    );
    let staker_packed = slot_value(&witness.rollup_storage, add_slot(staker_base, 2)?).unwrap();
    let staker_index = (staker_packed & U256::from(u64::MAX)).to::<u64>();
    let is_staked = ((staker_packed >> 64_usize) & U256::from(0xff_u8)).to::<u8>();
    let source_assertion_id =
        nitro_assertion_hash(&witness.assertion_path.last().unwrap().assertion);
    if amount_staked < policy.minimum_stake_wei
        || latest_staked != source_assertion_id
        || staker_index != 0
        || is_staked != 1
    {
        return Err(
            "source assertion is not the active bonded staker's newest assertion".to_string(),
        );
    }
    let mut parent_id = latest_confirmed;
    let mut parent = assertion_record(witness, latest_confirmed)?;
    if parent.status != 2 || parent.created_at_block == 0 {
        return Err(
            "latestConfirmed record is not an authenticated confirmed assertion".to_string(),
        );
    }
    for item in &witness.assertion_path {
        let assertion_id = nitro_assertion_hash(&item.assertion);
        if item.assertion.parent_assertion_hash != parent_id || item.assertion.machine_status != 1 {
            return Err(
                "bonded assertion path has a broken parent or execution-state link".to_string(),
            );
        }
        let current = assertion_record(witness, assertion_id)?;
        if current.status != 1
            || !current.is_first_child
            || current.created_at_block == 0
            || current.second_child_block != 0
            || parent.second_child_block != 0
            || parent.first_child_block != current.created_at_block
        {
            return Err(
                "bonded assertion path is not the unique live first-child branch".to_string(),
            );
        }
        if assertion_config_hash(&item.parent_config) != parent.config_hash
            || item.parent_config.wasm_module_root != policy.expected_wasm_module_root
            || item.parent_config.required_stake != policy.minimum_stake_wei
            || item.parent_config.challenge_manager != policy.challenge_manager_address
            || item.parent_config.confirm_period_blocks != policy.expected_confirm_period_blocks
            || item.parent_config.next_inbox_position == 0
        {
            return Err("bonded assertion parent configuration is not authenticated".to_string());
        }
        parent_id = assertion_id;
        parent = current;
    }
    if parent.first_child_block != 0 {
        return Err("source assertion is not the newest live assertion leaf".to_string());
    }
    Ok(VerifiedBondedBranch {
        latest_confirmed_assertion_id: latest_confirmed,
        source_assertion_id,
        source_created_at_l1_block: parent.created_at_block,
    })
}

fn assertion_record(
    witness: &PfUsdcBondedIngressWitnessV1,
    assertion_id: B256,
) -> Result<AssertionRecord, String> {
    assertion_record_from_storage(&witness.rollup_storage, &witness.policy, assertion_id)
}

fn assertion_record_from_storage(
    storage: &ContractStorage,
    policy: &PfUsdcBondedIngressPolicyV1,
    assertion_id: B256,
) -> Result<AssertionRecord, String> {
    let base = mapping_slot(assertion_id, policy.rollup_assertions_mapping_slot);
    let word =
        slot_value(storage, base).ok_or_else(|| "assertion path record is absent".to_string())?;
    Ok(AssertionRecord {
        first_child_block: (word & U256::from(u64::MAX)).to::<u64>(),
        second_child_block: ((word >> 64_usize) & U256::from(u64::MAX)).to::<u64>(),
        created_at_block: ((word >> 128_usize) & U256::from(u64::MAX)).to::<u64>(),
        is_first_child: ((word >> 192_usize) & U256::from(0xff_u8)).to::<u8>() == 1,
        status: ((word >> 200_usize) & U256::from(0xff_u8)).to::<u8>(),
        config_hash: B256::from(
            slot_value(storage, add_slot(base, 1)?)
                .ok_or_else(|| "assertion config hash is absent".to_string())?
                .to_be_bytes::<32>(),
        ),
    })
}

fn verify_pinned_rollup_configuration(
    storage: &ContractStorage,
    policy: &PfUsdcBondedIngressPolicyV1,
) -> Result<(), String> {
    if policy.rollup_primary_implementation_slot
        != B256::new(hex!(
            "360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc"
        ))
        || policy.rollup_secondary_implementation_slot
            != B256::new(hex!(
                "2b1dbce74324248c222f0ec2d5ed7bd323cfc425b336f0253c5ccfda7265546d"
            ))
        || policy.rollup_paused_storage_slot != slot(51)
        || policy.rollup_chain_config_storage_slot != slot(101)
        || policy.rollup_assertion_config_storage_slot != slot(102)
        || policy.rollup_base_stake_storage_slot != slot(103)
        || policy.rollup_wasm_module_root_storage_slot != slot(104)
        || policy.rollup_challenge_config_storage_slot != slot(105)
        || policy.rollup_stake_token_storage_slot != slot(112)
        || policy.rollup_latest_confirmed_storage_slot != slot(116)
        || policy.rollup_assertions_mapping_slot != slot(117)
        || policy.rollup_validator_policy_storage_slot != slot(123)
    {
        return Err("unsupported RollupCore/BoLD storage layout".to_string());
    }
    let assertion_config = slot_value(storage, policy.rollup_assertion_config_storage_slot)
        .ok_or_else(|| "Rollup assertion config proof is absent".to_string())?;
    let challenge_config = slot_value(storage, policy.rollup_challenge_config_storage_slot)
        .ok_or_else(|| "Rollup challenge config proof is absent".to_string())?;
    let confirm_period = (assertion_config & U256::from(u64::MAX)).to::<u64>();
    let validator_afk = ((assertion_config >> 64_usize) & U256::from(u64::MAX)).to::<u64>();
    let challenge_grace = ((challenge_config >> 160_usize) & U256::from(u64::MAX)).to::<u64>();
    if address_from_word(
        slot_value(storage, policy.rollup_primary_implementation_slot)
            .ok_or_else(|| "Rollup primary implementation proof is absent".to_string())?,
    ) != policy.rollup_admin_implementation_address
        || address_from_word(
            slot_value(storage, policy.rollup_secondary_implementation_slot)
                .ok_or_else(|| "Rollup secondary implementation proof is absent".to_string())?,
        ) != policy.rollup_user_implementation_address
        || slot_value(storage, policy.rollup_paused_storage_slot) != Some(U256::ZERO)
        || slot_value(storage, policy.rollup_chain_config_storage_slot)
            != Some(U256::from(policy.arbitrum_chain_id))
        || confirm_period != policy.expected_confirm_period_blocks
        || validator_afk != policy.expected_validator_afk_blocks
        || slot_value(storage, policy.rollup_base_stake_storage_slot)
            != Some(policy.minimum_stake_wei)
        || B256::from(
            slot_value(storage, policy.rollup_wasm_module_root_storage_slot)
                .ok_or_else(|| "Rollup wasm root proof is absent".to_string())?
                .to_be_bytes::<32>(),
        ) != policy.expected_wasm_module_root
        || address_from_word(challenge_config) != policy.challenge_manager_address
        || challenge_grace != policy.expected_challenge_grace_period_blocks
        || address_from_word(
            slot_value(storage, policy.rollup_stake_token_storage_slot)
                .ok_or_else(|| "Rollup stake token proof is absent".to_string())?,
        ) != policy.expected_stake_token
        || slot_value(storage, policy.rollup_validator_policy_storage_slot) != Some(U256::from(1))
    {
        return Err("RollupCore/BoLD pinned configuration mismatch".to_string());
    }
    Ok(())
}

fn verify_challenge_manager_implementation(
    ethereum_state_root: B256,
    policy: &PfUsdcBondedIngressPolicyV1,
    proxy: &ContractStorage,
    implementation: &ContractStorage,
) -> Result<(), String> {
    if proxy.address != policy.challenge_manager_address
        || proxy.value.code_hash != policy.challenge_manager_runtime_code_hash
        || policy.challenge_manager_runtime_code_hash == B256::ZERO
        || proxy.storage_slots.len() != 1
        || proxy.storage_slots[0].key != policy.challenge_manager_implementation_slot
    {
        return Err(
            "BoLD challenge-manager proxy proof has the wrong implementation slot".to_string(),
        );
    }
    let verified = verify_bonded_storage_slot_proofs(ethereum_state_root, proxy)
        .map_err(|error| format!("invalid challenge-manager implementation proof: {error}"))?;
    if verified.len() != 1
        || address_from_word(proxy.storage_slots[0].value)
            != policy.challenge_manager_implementation_address
    {
        return Err("BoLD challenge-manager active implementation mismatch".to_string());
    }
    verify_account_code(
        ethereum_state_root,
        implementation,
        policy.challenge_manager_implementation_address,
        policy.challenge_manager_implementation_runtime_code_hash,
        "BoLD challenge manager implementation",
    )
}

fn assertion_config_hash(config: &NitroAssertionConfigWitnessV1) -> B256 {
    let mut bytes = Vec::with_capacity(100);
    bytes.extend_from_slice(config.wasm_module_root.as_slice());
    bytes.extend_from_slice(&config.required_stake.to_be_bytes::<32>());
    bytes.extend_from_slice(config.challenge_manager.as_slice());
    bytes.extend_from_slice(&config.confirm_period_blocks.to_be_bytes());
    bytes.extend_from_slice(&config.next_inbox_position.to_be_bytes());
    keccak256(bytes)
}

fn verify_asserted_l2_state(witness: &PfUsdcBondedIngressWitnessV1) -> Result<(B256, u64), String> {
    let assertion = &witness
        .assertion_path
        .last()
        .ok_or_else(|| "bonded assertion path is empty".to_string())?
        .assertion;
    if assertion.machine_status != 1
        || witness.asserted_l2_header_rlp.is_empty()
        || witness.asserted_l2_header_rlp.len() > MAX_L2_HEADER_RLP_BYTES
        || keccak256(&witness.asserted_l2_header_rlp) != assertion.block_hash
    {
        return Err("bonded assertion L2 header does not match the finished assertion".to_string());
    }
    let mut encoded = witness.asserted_l2_header_rlp.as_ref();
    let header = Header::decode(&mut encoded)
        .map_err(|error| format!("invalid bonded assertion L2 header: {error}"))?;
    if !encoded.is_empty() || header.state_root == B256::ZERO {
        return Err("bonded assertion L2 header is noncanonical".to_string());
    }
    Ok((header.state_root, header.number))
}

fn verify_deposit_inclusion(
    witness: &PfUsdcBondedIngressWitnessV1,
    l2_state_root: B256,
) -> Result<B256, String> {
    let expected_deposit_id = vault_bridge_deposit_id(&witness.evidence)?;
    if witness.evidence.deposit_id != expected_deposit_id {
        return Err("bonded ingress evidence/deposit-id binding mismatch".to_string());
    }
    let deposit_id = parse_hex32(&witness.evidence.deposit_id)?;
    let deposit_seen_slot =
        mapping_slot(deposit_id, witness.policy.vault_deposit_seen_mapping_slot);
    let vault = &witness.asserted_l2_vault_account;
    if vault.address != witness.policy.arbitrum_vault_address
        || vault.value.code_hash != witness.policy.arbitrum_vault_runtime_code_hash
        || vault.storage_slots.len() != 1
        || vault.storage_slots[0].key != deposit_seen_slot
    {
        return Err("vault depositSeen proof does not match pinned vault/deposit".to_string());
    }
    let verified = verify_bonded_storage_slot_proofs(l2_state_root, vault)
        .map_err(|error| format!("invalid vault depositSeen proof: {error}"))?;
    if verified.len() != 1
        || verified[0].value != B256::from(U256::from(1).to_be_bytes::<32>())
        || vault.storage_slots[0].value != U256::from(1)
    {
        return Err(
            "asserted state does not contain the exact successful vault deposit".to_string(),
        );
    }
    verify_account_code(
        l2_state_root,
        &witness.asserted_l2_token_account,
        witness.policy.arbitrum_token_address,
        witness.policy.arbitrum_token_runtime_code_hash,
        "Arbitrum token",
    )?;
    Ok(deposit_seen_slot)
}

pub fn bonded_ingress_policy_hash_v1(policy: &PfUsdcBondedIngressPolicyV1) -> String {
    let mut bytes = b"PFTL-PFUSDC-BONDED-INGRESS-POLICY-V1".to_vec();
    bytes.extend_from_slice(policy.deployment_manifest_hash.as_slice());
    append_policy_text(&mut bytes, &policy.assertion_protocol_adapter_id);
    append_policy_text(&mut bytes, &policy.ethereum_finality_adapter_id);
    append_policy_text(&mut bytes, &policy.l2_header_fork_rules_id);
    append_policy_text(&mut bytes, &policy.asset_id);
    bytes.extend_from_slice(&policy.cap_atoms.to_be_bytes());
    bytes.extend_from_slice(&policy.age_margin_blocks.to_be_bytes());
    bytes.extend_from_slice(&policy.ethereum_chain_id.to_be_bytes());
    bytes.extend_from_slice(policy.ethereum_genesis_validators_root.as_slice());
    bytes.extend_from_slice(&policy.arbitrum_chain_id.to_be_bytes());
    bytes.extend_from_slice(policy.arbitrum_rollup_address.as_slice());
    bytes.extend_from_slice(policy.arbitrum_rollup_runtime_code_hash.as_slice());
    bytes.extend_from_slice(policy.rollup_admin_implementation_address.as_slice());
    bytes.extend_from_slice(
        policy
            .rollup_admin_implementation_runtime_code_hash
            .as_slice(),
    );
    bytes.extend_from_slice(policy.rollup_user_implementation_address.as_slice());
    bytes.extend_from_slice(
        policy
            .rollup_user_implementation_runtime_code_hash
            .as_slice(),
    );
    bytes.extend_from_slice(policy.rollup_primary_implementation_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_secondary_implementation_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_paused_storage_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_chain_config_storage_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_assertion_config_storage_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_base_stake_storage_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_wasm_module_root_storage_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_challenge_config_storage_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_stake_token_storage_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_latest_confirmed_storage_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_assertions_mapping_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_staker_list_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_staker_map_slot.as_slice());
    bytes.extend_from_slice(policy.rollup_validator_policy_storage_slot.as_slice());
    bytes.extend_from_slice(&policy.minimum_stake_wei.to_be_bytes::<32>());
    bytes.extend_from_slice(&policy.expected_confirm_period_blocks.to_be_bytes());
    bytes.extend_from_slice(&policy.expected_validator_afk_blocks.to_be_bytes());
    bytes.extend_from_slice(&policy.expected_challenge_grace_period_blocks.to_be_bytes());
    bytes.extend_from_slice(policy.expected_wasm_module_root.as_slice());
    bytes.extend_from_slice(policy.challenge_manager_address.as_slice());
    bytes.extend_from_slice(policy.challenge_manager_runtime_code_hash.as_slice());
    bytes.extend_from_slice(policy.challenge_manager_implementation_slot.as_slice());
    bytes.extend_from_slice(policy.challenge_manager_implementation_address.as_slice());
    bytes.extend_from_slice(
        policy
            .challenge_manager_implementation_runtime_code_hash
            .as_slice(),
    );
    bytes.extend_from_slice(policy.expected_stake_token.as_slice());
    bytes.extend_from_slice(policy.arbitrum_vault_address.as_slice());
    bytes.extend_from_slice(policy.arbitrum_vault_runtime_code_hash.as_slice());
    bytes.extend_from_slice(policy.vault_deposit_seen_mapping_slot.as_slice());
    bytes.extend_from_slice(policy.arbitrum_token_address.as_slice());
    bytes.extend_from_slice(policy.arbitrum_token_runtime_code_hash.as_slice());
    hex32(keccak256(bytes))
}

fn append_policy_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u32).to_be_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn validate_bounds(witness: &PfUsdcBondedIngressWitnessV1) -> Result<(), String> {
    if witness.schema != PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1
        || witness.policy.schema != PFUSDC_BONDED_INGRESS_POLICY_SCHEMA_V1
        || witness.rollup_storage.storage_slots.len() > MAX_ROLLUP_STORAGE_PROOFS
        || witness.asserted_l2_vault_account.storage_slots.len() > MAX_VAULT_STORAGE_PROOFS
        || witness.asserted_l2_token_account.storage_slots.len() > MAX_VAULT_STORAGE_PROOFS
    {
        return Err("bonded ingress storage-proof count exceeds bound".to_string());
    }
    crate::validate_mpt_proof(&witness.rollup_storage.mpt_proof)?;
    for proof in &witness.rollup_storage.storage_slots {
        crate::validate_mpt_proof(&proof.mpt_proof)?;
    }
    crate::validate_mpt_proof(&witness.asserted_l2_vault_account.mpt_proof)?;
    for proof in &witness.asserted_l2_vault_account.storage_slots {
        crate::validate_mpt_proof(&proof.mpt_proof)?;
    }
    crate::validate_account_proof_bounds(&witness.asserted_l2_token_account)?;
    crate::validate_account_proof_bounds(&witness.rollup_admin_implementation_account)?;
    crate::validate_account_proof_bounds(&witness.rollup_user_implementation_account)?;
    validate_storage_account_bounds(&witness.challenge_manager_account, 1)?;
    crate::validate_account_proof_bounds(&witness.challenge_manager_implementation_account)?;
    Ok(())
}

fn validate_confirmation_bounds(witness: &PfUsdcBondedConfirmationWitnessV1) -> Result<(), String> {
    if witness.schema != PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1
        || witness.policy.schema != PFUSDC_BONDED_INGRESS_POLICY_SCHEMA_V1
        || witness.rollup_storage.storage_slots.len() > MAX_ROLLUP_STORAGE_PROOFS
        || witness.confirmation_path.len() > MAX_ASSERTION_PATH_LEN
    {
        return Err("bonded lifecycle witness exceeds a guest bound".to_string());
    }
    validate_storage_account_bounds(&witness.rollup_storage, MAX_ROLLUP_STORAGE_PROOFS)?;
    crate::validate_account_proof_bounds(&witness.rollup_admin_implementation_account)?;
    crate::validate_account_proof_bounds(&witness.rollup_user_implementation_account)?;
    validate_storage_account_bounds(&witness.challenge_manager_account, 1)?;
    crate::validate_account_proof_bounds(&witness.challenge_manager_implementation_account)?;
    Ok(())
}

fn validate_reversion_bounds(witness: &PfUsdcBondedReversionWitnessV1) -> Result<(), String> {
    if witness.schema != PFUSDC_BONDED_INGRESS_WITNESS_SCHEMA_V1
        || witness.policy.schema != PFUSDC_BONDED_INGRESS_POLICY_SCHEMA_V1
        || witness.rollup_storage.storage_slots.len() > MAX_ROLLUP_STORAGE_PROOFS
        || witness.source_path.is_empty()
        || witness.winning_path.is_empty()
        || witness.source_path.len() > MAX_ASSERTION_PATH_LEN
        || witness.winning_path.len() > MAX_ASSERTION_PATH_LEN
    {
        return Err("bonded reversion witness exceeds a guest bound".to_string());
    }
    validate_storage_account_bounds(&witness.rollup_storage, MAX_ROLLUP_STORAGE_PROOFS)?;
    crate::validate_account_proof_bounds(&witness.rollup_admin_implementation_account)?;
    crate::validate_account_proof_bounds(&witness.rollup_user_implementation_account)?;
    validate_storage_account_bounds(&witness.challenge_manager_account, 1)?;
    crate::validate_account_proof_bounds(&witness.challenge_manager_implementation_account)?;
    Ok(())
}

fn validate_storage_account_bounds(
    account: &ContractStorage,
    max_storage_proofs: usize,
) -> Result<(), String> {
    if account.storage_slots.is_empty() || account.storage_slots.len() > max_storage_proofs {
        return Err("bonded ingress storage-account proof count exceeds bound".to_string());
    }
    crate::validate_mpt_proof(&account.mpt_proof)?;
    for proof in &account.storage_slots {
        crate::validate_mpt_proof(&proof.mpt_proof)?;
    }
    Ok(())
}

/// SP1 Helios' stock storage verifier treats integer zero as an included RLP
/// value. Ethereum storage canonicalization instead omits zero-valued leaves,
/// so a genuine `eth_getProof` for a zero slot is a non-inclusion proof. Keep
/// the vetted alloy trie verifier, but select `None` for exactly-zero values.
fn verify_bonded_storage_slot_proofs(
    execution_state_root: B256,
    account: &ContractStorage,
) -> Result<Vec<StorageSlot>, String> {
    let address_hash = keccak256(account.address.as_slice());
    let address_nibbles = Nibbles::unpack(Bytes::copy_from_slice(address_hash.as_ref()));
    let mut encoded_account = Vec::new();
    account.value.encode(&mut encoded_account);
    proof::verify_proof(
        execution_state_root,
        address_nibbles,
        Some(encoded_account),
        &account.mpt_proof,
    )
    .map_err(|error| format!("invalid bonded account proof: {error}"))?;

    let mut verified = Vec::with_capacity(account.storage_slots.len());
    for slot in &account.storage_slots {
        let key_hash = keccak256(slot.key.as_slice());
        let key_nibbles = Nibbles::unpack(Bytes::copy_from_slice(key_hash.as_ref()));
        let expected = if slot.value == U256::ZERO {
            None
        } else {
            let mut encoded = Vec::new();
            slot.value.encode(&mut encoded);
            Some(encoded)
        };
        proof::verify_proof(
            account.value.storage_root,
            key_nibbles,
            expected,
            &slot.mpt_proof,
        )
        .map_err(|error| {
            format!(
                "invalid bonded storage proof for slot {}: {error}",
                hex::encode(slot.key)
            )
        })?;
        verified.push(StorageSlot {
            key: slot.key,
            value: B256::from(slot.value.to_be_bytes::<32>()),
            contractAddress: account.address,
        });
    }
    Ok(verified)
}

fn slot(value: u64) -> B256 {
    B256::from(U256::from(value).to_be_bytes::<32>())
}

fn mapping_slot(key: B256, base: B256) -> B256 {
    let mut preimage = [0_u8; 64];
    preimage[..32].copy_from_slice(key.as_slice());
    preimage[32..].copy_from_slice(base.as_slice());
    keccak256(preimage)
}

fn address_mapping_slot(key: Address, base: B256) -> B256 {
    let mut padded = [0_u8; 32];
    padded[12..].copy_from_slice(key.as_slice());
    mapping_slot(B256::from(padded), base)
}

fn add_slot(value: B256, offset: u64) -> Result<B256, String> {
    let value = U256::from_be_bytes(value.0);
    value
        .checked_add(U256::from(offset))
        .map(|sum| B256::from(sum.to_be_bytes::<32>()))
        .ok_or_else(|| "storage slot overflow".to_string())
}

fn slot_value(storage: &ContractStorage, key: B256) -> Option<U256> {
    let mut found = storage.storage_slots.iter().filter(|slot| slot.key == key);
    let value = found.next().map(|slot| slot.value);
    if found.next().is_some() {
        None
    } else {
        value
    }
}

fn address_from_word(value: U256) -> Address {
    let bytes = value.to_be_bytes::<32>();
    Address::from_slice(&bytes[12..])
}

fn parse_hex32(value: &str) -> Result<B256, String> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.len() != 64 {
        return Err("invalid hex32 length".to_string());
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]).ok_or_else(|| "invalid hex32".to_string())?;
        let low = hex_nibble(pair[1]).ok_or_else(|| "invalid hex32".to_string())?;
        bytes[index] = (high << 4) | low;
    }
    Ok(B256::from(bytes))
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod bonded_tests {
    use super::*;

    fn assertion(parent: B256, byte: u8) -> NitroAssertionWitnessV1 {
        NitroAssertionWitnessV1 {
            parent_assertion_hash: parent,
            block_hash: B256::repeat_byte(byte),
            send_root: B256::repeat_byte(byte.wrapping_add(1)),
            inbox_position: u64::from(byte) + 1,
            position_in_message: 0,
            machine_status: 1,
            end_history_root: B256::repeat_byte(byte.wrapping_add(2)),
            inbox_accumulator: B256::repeat_byte(byte.wrapping_add(3)),
        }
    }

    #[test]
    fn lifecycle_hash_chain_rejects_omitted_or_reordered_parent() {
        let first = assertion(B256::repeat_byte(0x10), 0x20);
        let second = assertion(nitro_assertion_hash(&first), 0x30);
        let third = assertion(nitro_assertion_hash(&second), 0x40);
        assert!(verify_hash_chain(&[first.clone(), second.clone(), third.clone()]).is_ok());
        assert!(verify_hash_chain(&[first.clone(), third]).is_err());
        let mut wrong = second;
        wrong.parent_assertion_hash = B256::repeat_byte(0xff);
        assert!(verify_hash_chain(&[first, wrong]).is_err());
    }

    #[test]
    fn deposit_record_slot_is_unique_and_not_an_assertion_root() {
        let base = slot(3);
        let first = mapping_slot(B256::repeat_byte(0x11), base);
        let second = mapping_slot(B256::repeat_byte(0x12), base);
        assert_ne!(first, second);
        assert_ne!(first, B256::repeat_byte(0x11));
        assert_ne!(first, base);
    }

    #[test]
    fn assertion_hash_binds_global_state_field_order() {
        let original = assertion(B256::repeat_byte(0x10), 0x20);
        let mut swapped = original.clone();
        std::mem::swap(&mut swapped.block_hash, &mut swapped.send_root);
        assert_ne!(
            nitro_assertion_hash(&original),
            nitro_assertion_hash(&swapped)
        );
    }
}
