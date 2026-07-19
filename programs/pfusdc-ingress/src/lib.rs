use alloy_consensus::Header;
use alloy_primitives::{keccak256, Address, Bytes, B256, U256};
use alloy_rlp::Decodable;
use alloy_sol_types::{sol, SolCall, SolValue};
use helios_consensus_core::{
    apply_finality_update, apply_update, verify_finality_update, verify_update,
};
use postfiat_types::{
    vault_bridge_deposit_evidence_root, vault_bridge_route_binding, PfUsdcIngressPublicValuesV3,
    VaultBridgeDepositEvidence, VaultBridgeRouteProfileV1,
    NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1, PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V3,
    VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN,
};
use serde::{Deserialize, Serialize};
use sp1_helios_primitives::{
    types::{ContractStorage, ProofInputs},
    verify_storage_slot_proofs,
};
use tree_hash::TreeHash;

pub const PFUSDC_INGRESS_PROOF_WITNESS_SCHEMA_V2: &str = "postfiat.pfusdc.ingress_proof_witness.v2";
pub const PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V2: &str = "postfiat.pfusdc.ingress_proof_policy.v2";
pub const PFUSDC_INGRESS_PROOF_PROGRAM_VERSION_V3: u32 = 3;
pub const NITRO_REFERENCE_COMMIT: &str = "a618155919315241665356fe60f3cd00d66d5e46";
pub const NITRO_CONTRACTS_REFERENCE_COMMIT: &str =
    "4341b132cfbdcc980ead03765ca5224ff6cb5d97";
pub const ETHEREUM_MAINNET_CHAIN_ID: u64 = 1;
pub const ETHEREUM_SEPOLIA_CHAIN_ID: u64 = 11_155_111;
pub const ARBITRUM_ONE_CHAIN_ID: u64 = 42_161;
pub const ARBITRUM_SEPOLIA_CHAIN_ID: u64 = 421_614;
pub const ARBITRUM_ONE_ROLLUP_ADDRESS: Address = Address::new([
    0x4d, 0xce, 0xb4, 0x40, 0x65, 0x7f, 0x21, 0x08, 0x3d, 0xb8, 0xad, 0xd0, 0x76, 0x65, 0xf8, 0xdd,
    0xbe, 0x1d, 0xcf, 0xc0,
]);
pub const ARBITRUM_SEPOLIA_ROLLUP_ADDRESS: Address = Address::new([
    0x04, 0x2b, 0x2e, 0x6c, 0x5e, 0x99, 0xd4, 0xc5, 0x21, 0xbd, 0x49, 0xbe, 0xed, 0x5e, 0x99, 0x65,
    0x1d, 0x9b, 0x0c, 0xf4,
]);
pub const NITRO_LATEST_CONFIRMED_STORAGE_SLOT: B256 = B256::new([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0x74,
]);
pub const ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT: B256 = B256::new([
    0x4b, 0x36, 0x3d, 0xb9, 0x4e, 0x28, 0x61, 0x20, 0xd7, 0x6e, 0xb9, 0x05, 0x34, 0x0f, 0xdd, 0x4e,
    0x54, 0xbf, 0xe9, 0xf0, 0x6b, 0xf3, 0x3f, 0xf6, 0xcf, 0x5a, 0xd2, 0x7f, 0x51, 0x1b, 0xfe, 0x95,
]);
pub const ETHEREUM_SEPOLIA_GENESIS_VALIDATORS_ROOT: B256 = B256::new([
    0xd8, 0xea, 0x17, 0x1f, 0x3c, 0x94, 0xae, 0xa2, 0x1e, 0xbc, 0x42, 0xa1, 0xed, 0x61, 0x05, 0x2a,
    0xcf, 0x3f, 0x92, 0x09, 0xc0, 0x0e, 0x4e, 0xfb, 0xaa, 0xdd, 0xac, 0x09, 0xed, 0x9b, 0x80, 0x78,
]);
const MAX_HELIOS_UPDATES_V1: usize = 8;
const MAX_MPT_NODES_V1: usize = 64;
const MAX_MPT_NODE_BYTES_V1: usize = 16_384;
const MAX_OUTPUT_PROOF_NODES_V1: usize = 64;
const MAX_OUTPUT_CALLDATA_BYTES_V1: usize = 2_048;
const MAX_L2_HEADER_RLP_BYTES_V1: usize = 4_096;

sol! {
    function recordDepositV1(
        bytes32 deposit_id,
        address depositor,
        bytes32 recipient_hash,
        string recipient,
        uint256 amount,
        bytes32 nonce,
        bytes32 route_binding,
        uint256 chain_id,
        address vault,
        address token
    );

    struct NitroGlobalStateV1 {
        bytes32[2] bytes32Vals;
        uint64[2] u64Vals;
    }

    struct NitroAssertionStateV1 {
        NitroGlobalStateV1 globalState;
        uint8 machineStatus;
        bytes32 endHistoryRoot;
    }

}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PfUsdcIngressProofPolicyV2 {
    pub schema: String,
    pub ethereum_chain_id: u64,
    pub ethereum_genesis_validators_root: B256,
    pub arbitrum_chain_id: u64,
    pub arbitrum_rollup_address: Address,
    pub arbitrum_rollup_runtime_code_hash: B256,
    pub rollup_latest_confirmed_storage_slot: B256,
    pub arbitrum_vault_address: Address,
    pub arbitrum_vault_runtime_code_hash: B256,
    pub arbitrum_token_address: Address,
    pub arbitrum_token_runtime_code_hash: B256,
    pub ethereum_ingress_anchor_address: Address,
    pub ethereum_ingress_anchor_runtime_code_hash: B256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NitroSendWitnessV1 {
    pub output_index: u64,
    pub output_proof: Vec<B256>,
    pub l2_sender: Address,
    pub destination: Address,
    pub l2_block_number: u64,
    pub l1_block_number: u64,
    pub timestamp: u64,
    pub value: U256,
    pub calldata: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NitroAssertionWitnessV1 {
    pub parent_assertion_hash: B256,
    pub block_hash: B256,
    pub send_root: B256,
    pub inbox_position: u64,
    pub position_in_message: u64,
    pub machine_status: u8,
    pub end_history_root: B256,
    pub inbox_accumulator: B256,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PfUsdcIngressProofWitnessV2 {
    pub schema: String,
    pub route_profile: VaultBridgeRouteProfileV1,
    pub policy: PfUsdcIngressProofPolicyV2,
    pub helios: ProofInputs,
    pub rollup_storage: ContractStorage,
    pub ethereum_ingress_anchor_account: ContractStorage,
    pub assertion: NitroAssertionWitnessV1,
    pub asserted_l2_header_rlp: Bytes,
    pub asserted_l2_vault_account: ContractStorage,
    pub asserted_l2_token_account: ContractStorage,
    pub output: NitroSendWitnessV1,
    pub evidence: VaultBridgeDepositEvidence,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
}

pub fn verify_ingress_witness_v2(
    witness: &PfUsdcIngressProofWitnessV2,
) -> Result<PfUsdcIngressPublicValuesV3, String> {
    validate_bounds(witness)?;
    witness.route_profile.validate()?;
    witness.evidence.validate()?;
    verify_route_and_policy(witness)?;

    let (prior_root, prior_slot, final_root, final_slot, ethereum_state_root) =
        verify_ethereum_finality(&witness.helios, &witness.policy)?;
    verify_account_code(
        ethereum_state_root,
        &witness.ethereum_ingress_anchor_account,
        witness.policy.ethereum_ingress_anchor_address,
        witness.policy.ethereum_ingress_anchor_runtime_code_hash,
        "Ethereum ingress anchor",
    )?;
    let assertion_hash = verify_arbitrum_assertion(
        ethereum_state_root,
        &witness.rollup_storage,
        &witness.policy,
        &witness.assertion,
    )?;
    let assertion_l2_state_root = verify_asserted_l2_state(witness)?;
    let output_item_hash = verify_confirmed_deposit_output(witness)?;
    let profile_hash = witness.route_profile.profile_hash()?;
    let evidence_root = vault_bridge_deposit_evidence_root(&witness.evidence)?;
    let mut values = PfUsdcIngressPublicValuesV3 {
        schema: PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V3.to_string(),
        proof_program_version: PFUSDC_INGRESS_PROOF_PROGRAM_VERSION_V3,
        pftl_chain_id: witness.pftl_chain_id.clone(),
        pftl_genesis_hash: witness.pftl_genesis_hash.clone(),
        pftl_protocol_version: witness.pftl_protocol_version,
        route_profile_hash: profile_hash,
        route_epoch: u64::from(witness.route_profile.route_epoch),
        ethereum_chain_id: witness.policy.ethereum_chain_id,
        prior_ethereum_finalized_beacon_root: hex32(prior_root),
        prior_ethereum_finalized_slot: prior_slot,
        ethereum_finalized_beacon_root: hex32(final_root),
        ethereum_finalized_slot: final_slot,
        arbitrum_chain_id: witness.policy.arbitrum_chain_id,
        arbitrum_rollup_address: evm_address_text(witness.policy.arbitrum_rollup_address),
        arbitrum_rollup_runtime_code_hash: hex32(witness.policy.arbitrum_rollup_runtime_code_hash),
        rollup_latest_confirmed_storage_slot: hex32(
            witness.policy.rollup_latest_confirmed_storage_slot,
        ),
        arbitrum_assertion_hash: hex32(assertion_hash),
        assertion_l2_block_hash: hex32(witness.assertion.block_hash),
        assertion_l2_state_root: hex32(assertion_l2_state_root),
        assertion_send_root: hex32(witness.assertion.send_root),
        output_index: witness.output.output_index,
        output_item_hash: hex32(output_item_hash),
        output_l2_block_number: witness.output.l2_block_number,
        output_l1_block_number: witness.output.l1_block_number,
        output_timestamp: witness.output.timestamp,
        output_sender: evm_address_text(witness.output.l2_sender),
        output_destination: evm_address_text(witness.output.destination),
        ingress_anchor_runtime_code_hash: hex32(
            witness.policy.ethereum_ingress_anchor_runtime_code_hash,
        ),
        output_calldata_hash: hex32(keccak256(&witness.output.calldata)),
        vault_address: witness.evidence.vault_address.clone(),
        vault_runtime_code_hash: hex32(witness.policy.arbitrum_vault_runtime_code_hash),
        token_address: witness.evidence.token_address.clone(),
        token_runtime_code_hash: hex32(witness.policy.arbitrum_token_runtime_code_hash),
        depositor: witness.evidence.depositor.clone(),
        pftl_recipient: witness.evidence.pftl_recipient.clone(),
        pftl_recipient_hash: witness.evidence.pftl_recipient_hash.clone(),
        amount_atoms: witness.evidence.amount_atoms,
        nonce: witness.evidence.nonce.clone(),
        route_binding: witness.evidence.route_binding.clone(),
        deposit_id: witness.evidence.deposit_id.clone(),
        evidence_root,
        public_values_commitment: String::new(),
    };
    values.seal()?;
    values.validate()?;
    Ok(values)
}

fn verify_route_and_policy(witness: &PfUsdcIngressProofWitnessV2) -> Result<(), String> {
    let profile = &witness.route_profile;
    let policy = &witness.policy;
    if profile.verifier_kind != NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
        || profile.evidence_tier != VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN
        || profile.source_chain_id != policy.arbitrum_chain_id
        || profile.source_chain_id != witness.evidence.source_chain_id
        || profile.vault_address != witness.evidence.vault_address
        || profile.token_address != witness.evidence.token_address
        || profile.vault_address != evm_address_text(policy.arbitrum_vault_address)
        || strip_0x(&profile.vault_runtime_code_hash)
            != hex32(policy.arbitrum_vault_runtime_code_hash)
        || profile.token_address != evm_address_text(policy.arbitrum_token_address)
        || strip_0x(&profile.token_runtime_code_hash)
            != hex32(policy.arbitrum_token_runtime_code_hash)
        || !supported_network_binding(policy)
    {
        return Err("pfUSDC ingress route/policy/evidence binding mismatch".to_string());
    }
    let route_binding = vault_bridge_route_binding(&profile.profile_hash()?, profile.route_epoch)?;
    if route_binding != witness.evidence.route_binding {
        return Err("deposit event does not bind the governed Tier-4 route".to_string());
    }
    if ingress_policy_hash_v2(policy) != profile.verifier_policy_hash {
        return Err("ingress proof policy hash does not match governed route".to_string());
    }
    Ok(())
}

fn verify_ethereum_finality(
    inputs: &ProofInputs,
    policy: &PfUsdcIngressProofPolicyV2,
) -> Result<(B256, u64, B256, u64, B256), String> {
    if inputs.genesis_root != policy.ethereum_genesis_validators_root
        || !inputs.contract_storage.is_empty()
        || inputs.expected_current_slot != *inputs.finality_update.signature_slot()
    {
        return Err("Helios input does not match pinned Ethereum policy".to_string());
    }
    require_supported_forks(policy.ethereum_chain_id, &inputs.forks)?;
    let mut store = inputs.store.clone();
    store.next_sync_committee = None;
    let prior_root: B256 = store.finalized_header.beacon().tree_hash_root();
    let prior_slot = store.finalized_header.beacon().slot;
    for update in &inputs.updates {
        verify_update(
            update,
            inputs.expected_current_slot,
            &store,
            inputs.genesis_root,
            &inputs.forks,
        )
        .map_err(|error| format!("invalid Helios committee update: {error}"))?;
        apply_update(&mut store, update);
    }
    verify_finality_update(
        &inputs.finality_update,
        inputs.expected_current_slot,
        &store,
        inputs.genesis_root,
        &inputs.forks,
    )
    .map_err(|error| format!("invalid Helios finality update: {error}"))?;
    apply_finality_update(&mut store, &inputs.finality_update);
    let final_slot = store.finalized_header.beacon().slot;
    if final_slot <= prior_slot || !final_slot.is_multiple_of(32) {
        return Err("Ethereum finalized checkpoint did not canonically advance".to_string());
    }
    let final_root: B256 = store.finalized_header.beacon().tree_hash_root();
    let execution = store
        .finalized_header
        .execution()
        .map_err(|_| "finalized Ethereum header has no execution payload".to_string())?;
    Ok((
        prior_root,
        prior_slot,
        final_root,
        final_slot,
        *execution.state_root(),
    ))
}

fn supported_network_binding(policy: &PfUsdcIngressProofPolicyV2) -> bool {
    let common_slot = policy.rollup_latest_confirmed_storage_slot
        == NITRO_LATEST_CONFIRMED_STORAGE_SLOT;
    common_slot
        && ((policy.ethereum_chain_id == ETHEREUM_MAINNET_CHAIN_ID
            && policy.ethereum_genesis_validators_root
                == ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT
            && policy.arbitrum_chain_id == ARBITRUM_ONE_CHAIN_ID
            && policy.arbitrum_rollup_address == ARBITRUM_ONE_ROLLUP_ADDRESS)
            || (policy.ethereum_chain_id == ETHEREUM_SEPOLIA_CHAIN_ID
                && policy.ethereum_genesis_validators_root
                    == ETHEREUM_SEPOLIA_GENESIS_VALIDATORS_ROOT
                && policy.arbitrum_chain_id == ARBITRUM_SEPOLIA_CHAIN_ID
                && policy.arbitrum_rollup_address == ARBITRUM_SEPOLIA_ROLLUP_ADDRESS))
}

fn require_supported_forks(
    ethereum_chain_id: u64,
    forks: &helios_consensus_core::types::Forks,
) -> Result<(), String> {
    let expected = match ethereum_chain_id {
        ETHEREUM_MAINNET_CHAIN_ID => [
            (&forks.genesis, 0, [0, 0, 0, 0]),
            (&forks.altair, 74_240, [1, 0, 0, 0]),
            (&forks.bellatrix, 144_896, [2, 0, 0, 0]),
            (&forks.capella, 194_048, [3, 0, 0, 0]),
            (&forks.deneb, 269_568, [4, 0, 0, 0]),
            (&forks.electra, 364_032, [5, 0, 0, 0]),
            (&forks.fulu, 411_392, [6, 0, 0, 0]),
        ],
        ETHEREUM_SEPOLIA_CHAIN_ID => [
            (&forks.genesis, 0, [0x90, 0, 0, 0x69]),
            (&forks.altair, 50, [0x90, 0, 0, 0x70]),
            (&forks.bellatrix, 100, [0x90, 0, 0, 0x71]),
            (&forks.capella, 56_832, [0x90, 0, 0, 0x72]),
            (&forks.deneb, 132_608, [0x90, 0, 0, 0x73]),
            (&forks.electra, 222_464, [0x90, 0, 0, 0x74]),
            (&forks.fulu, 272_640, [0x90, 0, 0, 0x75]),
        ],
        _ => return Err("Helios source network is not allowlisted".to_string()),
    };
    if expected.iter().any(|(fork, epoch, version)| {
        fork.epoch != *epoch || fork.fork_version.as_slice() != version
    }) {
        return Err("Helios fork schedule does not match its allowlisted network".to_string());
    }
    Ok(())
}

fn verify_arbitrum_assertion(
    ethereum_state_root: B256,
    storage: &ContractStorage,
    policy: &PfUsdcIngressProofPolicyV2,
    assertion: &NitroAssertionWitnessV1,
) -> Result<B256, String> {
    if storage.address != policy.arbitrum_rollup_address
        || storage.value.code_hash != policy.arbitrum_rollup_runtime_code_hash
        || storage.storage_slots.len() != 1
        || storage.storage_slots[0].key != policy.rollup_latest_confirmed_storage_slot
        || assertion.machine_status != 1
    {
        return Err("Arbitrum RollupCore proof does not match pinned policy".to_string());
    }
    let verified = verify_storage_slot_proofs(ethereum_state_root, storage)
        .map_err(|error| format!("invalid RollupCore account/storage proof: {error}"))?;
    if verified.len() != 1 {
        return Err("RollupCore latestConfirmed proof must contain exactly one slot".to_string());
    }
    let assertion_hash = nitro_assertion_hash(assertion);
    let proved_value = B256::from(storage.storage_slots[0].value.to_be_bytes::<32>());
    if proved_value != assertion_hash || verified[0].value != assertion_hash {
        return Err("RollupCore latestConfirmed does not equal Nitro assertion hash".to_string());
    }
    Ok(assertion_hash)
}

fn nitro_assertion_hash(assertion: &NitroAssertionWitnessV1) -> B256 {
    let global = NitroGlobalStateV1 {
        bytes32Vals: [assertion.block_hash, assertion.send_root],
        u64Vals: [assertion.inbox_position, assertion.position_in_message],
    };
    let state = NitroAssertionStateV1 {
        globalState: global,
        machineStatus: assertion.machine_status,
        endHistoryRoot: assertion.end_history_root,
    };
    let state_hash = keccak256(state.abi_encode());
    let mut assertion_preimage = Vec::with_capacity(96);
    assertion_preimage.extend_from_slice(assertion.parent_assertion_hash.as_slice());
    assertion_preimage.extend_from_slice(state_hash.as_slice());
    assertion_preimage.extend_from_slice(assertion.inbox_accumulator.as_slice());
    keccak256(assertion_preimage)
}

fn verify_asserted_l2_state(witness: &PfUsdcIngressProofWitnessV2) -> Result<B256, String> {
    if witness.asserted_l2_header_rlp.is_empty()
        || witness.asserted_l2_header_rlp.len() > MAX_L2_HEADER_RLP_BYTES_V1
        || keccak256(&witness.asserted_l2_header_rlp) != witness.assertion.block_hash
    {
        return Err("asserted Arbitrum L2 header does not match Nitro block hash".to_string());
    }
    let mut encoded = witness.asserted_l2_header_rlp.as_ref();
    let header = Header::decode(&mut encoded)
        .map_err(|error| format!("invalid asserted Arbitrum L2 header RLP: {error}"))?;
    if !encoded.is_empty()
        || header.state_root == B256::ZERO
        || header.number < witness.output.l2_block_number
        || header.timestamp < witness.output.timestamp
    {
        return Err(
            "asserted Arbitrum L2 header fields do not cover the deposit output".to_string(),
        );
    }
    verify_account_code(
        header.state_root,
        &witness.asserted_l2_vault_account,
        witness.policy.arbitrum_vault_address,
        witness.policy.arbitrum_vault_runtime_code_hash,
        "Arbitrum vault",
    )?;
    verify_account_code(
        header.state_root,
        &witness.asserted_l2_token_account,
        witness.policy.arbitrum_token_address,
        witness.policy.arbitrum_token_runtime_code_hash,
        "Arbitrum token",
    )?;
    Ok(header.state_root)
}

fn verify_account_code(
    state_root: B256,
    account: &ContractStorage,
    expected_address: Address,
    expected_code_hash: B256,
    label: &str,
) -> Result<(), String> {
    if account.address != expected_address
        || account.value.code_hash != expected_code_hash
        || expected_code_hash == B256::ZERO
        || !account.storage_slots.is_empty()
    {
        return Err(format!(
            "{label} account proof does not match pinned policy"
        ));
    }
    let verified = verify_storage_slot_proofs(state_root, account)
        .map_err(|error| format!("invalid {label} account proof: {error}"))?;
    if !verified.is_empty() {
        return Err(format!(
            "{label} account proof unexpectedly contains storage slots"
        ));
    }
    Ok(())
}

fn verify_confirmed_deposit_output(witness: &PfUsdcIngressProofWitnessV2) -> Result<B256, String> {
    let output = &witness.output;
    let evidence = &witness.evidence;
    if evm_address_text(output.l2_sender) != evidence.vault_address
        || output.destination != witness.policy.ethereum_ingress_anchor_address
        || output.value != U256::ZERO
    {
        return Err(
            "Arbitrum output sender/destination/value does not match governed ingress".to_string(),
        );
    }

    let item_hash = nitro_output_item_hash(output);
    let root = nitro_output_root(output, item_hash)?;
    if root != witness.assertion.send_root {
        return Err(
            "Arbitrum output Merkle proof does not match confirmed assertion sendRoot".to_string(),
        );
    }

    let call = recordDepositV1Call::abi_decode_validate(&output.calldata)
        .map_err(|error| format!("invalid canonical Tier-4 deposit calldata: {error}"))?;
    if hex32(call.deposit_id) != evidence.deposit_id
        || evm_address_text(call.depositor) != evidence.depositor
        || hex32(call.recipient_hash) != evidence.pftl_recipient_hash
        || call.recipient != evidence.pftl_recipient
        || u256_u64(call.amount, "amount")? != evidence.amount_atoms
        || hex32(call.nonce) != evidence.nonce
        || hex32(call.route_binding) != evidence.route_binding
        || u256_u64(call.chain_id, "source chain ID")? != evidence.source_chain_id
        || evm_address_text(call.vault) != evidence.vault_address
        || evm_address_text(call.token) != evidence.token_address
        || evidence.block_hash != hex32(witness.assertion.block_hash)
        || evidence.tx_hash != hex32(item_hash)
        || evidence.log_index != output.output_index
    {
        return Err(
            "confirmed Tier-4 deposit output does not exactly match proposed evidence".to_string(),
        );
    }
    Ok(item_hash)
}

fn nitro_output_item_hash(output: &NitroSendWitnessV1) -> B256 {
    let mut item_preimage = Vec::with_capacity(20 + 20 + 32 * 4 + output.calldata.len());
    item_preimage.extend_from_slice(output.l2_sender.as_slice());
    item_preimage.extend_from_slice(output.destination.as_slice());
    append_u256(&mut item_preimage, U256::from(output.l2_block_number));
    append_u256(&mut item_preimage, U256::from(output.l1_block_number));
    append_u256(&mut item_preimage, U256::from(output.timestamp));
    append_u256(&mut item_preimage, output.value);
    item_preimage.extend_from_slice(&output.calldata);
    keccak256(item_preimage)
}

fn nitro_output_root(output: &NitroSendWitnessV1, item_hash: B256) -> Result<B256, String> {
    let mut root = keccak256(item_hash.as_slice());
    let mut index = output.output_index;
    for sibling in &output.output_proof {
        let mut pair = [0_u8; 64];
        if index & 1 == 0 {
            pair[..32].copy_from_slice(root.as_slice());
            pair[32..].copy_from_slice(sibling.as_slice());
        } else {
            pair[..32].copy_from_slice(sibling.as_slice());
            pair[32..].copy_from_slice(root.as_slice());
        }
        root = keccak256(pair);
        index /= 2;
    }
    if index != 0 {
        return Err("Nitro output index exceeds the supplied Merkle proof depth".to_string());
    }
    Ok(root)
}

fn append_u256(bytes: &mut Vec<u8>, value: U256) {
    bytes.extend_from_slice(&value.to_be_bytes::<32>());
}

pub fn ingress_policy_hash_v2(policy: &PfUsdcIngressProofPolicyV2) -> String {
    let mut bytes = b"PFTL-PFUSDC-INGRESS-POLICY-V2".to_vec();
    bytes.extend_from_slice(&policy.ethereum_chain_id.to_be_bytes());
    bytes.extend_from_slice(policy.ethereum_genesis_validators_root.as_slice());
    bytes.extend_from_slice(&policy.arbitrum_chain_id.to_be_bytes());
    bytes.extend_from_slice(policy.arbitrum_rollup_address.as_slice());
    bytes.extend_from_slice(policy.arbitrum_rollup_runtime_code_hash.as_slice());
    bytes.extend_from_slice(policy.rollup_latest_confirmed_storage_slot.as_slice());
    bytes.extend_from_slice(policy.arbitrum_vault_address.as_slice());
    bytes.extend_from_slice(policy.arbitrum_vault_runtime_code_hash.as_slice());
    bytes.extend_from_slice(policy.arbitrum_token_address.as_slice());
    bytes.extend_from_slice(policy.arbitrum_token_runtime_code_hash.as_slice());
    bytes.extend_from_slice(policy.ethereum_ingress_anchor_address.as_slice());
    bytes.extend_from_slice(policy.ethereum_ingress_anchor_runtime_code_hash.as_slice());
    hex32(keccak256(bytes))
}

fn u256_u64(value: U256, field: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("vault deposit {field} exceeds u64"))
}

fn strip_0x(value: &str) -> String {
    value.strip_prefix("0x").unwrap_or(value).to_string()
}

fn hex32(value: B256) -> String {
    format!("{value:x}")
}

fn evm_address_text(value: Address) -> String {
    format!("{value:#x}")
}

fn validate_bounds(witness: &PfUsdcIngressProofWitnessV2) -> Result<(), String> {
    if witness.schema != PFUSDC_INGRESS_PROOF_WITNESS_SCHEMA_V2
        || witness.policy.schema != PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V2
    {
        return Err("pfUSDC ingress witness/policy schema mismatch".to_string());
    }
    if witness.helios.updates.len() > MAX_HELIOS_UPDATES_V1
        || witness.output.output_proof.len() > MAX_OUTPUT_PROOF_NODES_V1
        || witness.output.calldata.len() > MAX_OUTPUT_CALLDATA_BYTES_V1
        || witness.asserted_l2_header_rlp.is_empty()
        || witness.asserted_l2_header_rlp.len() > MAX_L2_HEADER_RLP_BYTES_V1
    {
        return Err("pfUSDC ingress witness exceeds a byte/count bound".to_string());
    }
    if witness.rollup_storage.mpt_proof.len() > MAX_MPT_NODES_V1
        || witness.rollup_storage.storage_slots.len() != 1
    {
        return Err("pfUSDC ingress rollup storage proof has invalid bounds".to_string());
    }
    validate_mpt_proof(&witness.rollup_storage.mpt_proof)?;
    validate_mpt_proof(&witness.rollup_storage.storage_slots[0].mpt_proof)?;
    validate_account_proof_bounds(&witness.ethereum_ingress_anchor_account)?;
    validate_account_proof_bounds(&witness.asserted_l2_vault_account)?;
    validate_account_proof_bounds(&witness.asserted_l2_token_account)?;
    Ok(())
}

fn validate_account_proof_bounds(account: &ContractStorage) -> Result<(), String> {
    if !account.storage_slots.is_empty() {
        return Err("pfUSDC ingress code account proof must not contain storage slots".to_string());
    }
    validate_mpt_proof(&account.mpt_proof)
}

fn validate_mpt_proof(nodes: &[Bytes]) -> Result<(), String> {
    if nodes.is_empty()
        || nodes.len() > MAX_MPT_NODES_V1
        || nodes.iter().any(|node| node.len() > MAX_MPT_NODE_BYTES_V1)
    {
        return Err("pfUSDC ingress MPT proof has invalid bounds".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_rlp::Encodable;
    use alloy_trie::{proof::ProofRetainer, HashBuilder, Nibbles, TrieAccount, EMPTY_ROOT_HASH};
    use std::collections::BTreeMap;

    fn policy() -> PfUsdcIngressProofPolicyV2 {
        PfUsdcIngressProofPolicyV2 {
            schema: PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V2.to_string(),
            ethereum_chain_id: ETHEREUM_MAINNET_CHAIN_ID,
            ethereum_genesis_validators_root: ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT,
            arbitrum_chain_id: ARBITRUM_ONE_CHAIN_ID,
            arbitrum_rollup_address: ARBITRUM_ONE_ROLLUP_ADDRESS,
            arbitrum_rollup_runtime_code_hash: B256::repeat_byte(0x22),
            rollup_latest_confirmed_storage_slot: NITRO_LATEST_CONFIRMED_STORAGE_SLOT,
            arbitrum_vault_address: Address::repeat_byte(0x44),
            arbitrum_vault_runtime_code_hash: B256::repeat_byte(0x45),
            arbitrum_token_address: Address::repeat_byte(0x46),
            arbitrum_token_runtime_code_hash: B256::repeat_byte(0x47),
            ethereum_ingress_anchor_address: Address::repeat_byte(0x48),
            ethereum_ingress_anchor_runtime_code_hash: B256::repeat_byte(0x49),
        }
    }

    #[test]
    fn ingress_policy_hash_binds_every_policy_field_class() {
        let base = policy();
        let expected = ingress_policy_hash_v2(&base);
        assert_eq!(expected.len(), 64);
        let mut mutations = Vec::new();
        let mut value = base.clone();
        value.ethereum_chain_id += 1;
        mutations.push(value);
        let mut value = base.clone();
        value.ethereum_genesis_validators_root = B256::repeat_byte(0x44);
        mutations.push(value);
        let mut value = base.clone();
        value.arbitrum_chain_id += 1;
        mutations.push(value);
        let mut value = base.clone();
        value.arbitrum_rollup_address = Address::repeat_byte(0x55);
        mutations.push(value);
        let mut value = base.clone();
        value.arbitrum_rollup_runtime_code_hash = B256::repeat_byte(0x66);
        mutations.push(value);
        let mut value = base.clone();
        value.rollup_latest_confirmed_storage_slot = B256::repeat_byte(0x77);
        mutations.push(value);
        let mut value = base.clone();
        value.arbitrum_vault_address = Address::repeat_byte(0x88);
        mutations.push(value);
        let mut value = base.clone();
        value.arbitrum_vault_runtime_code_hash = B256::repeat_byte(0x89);
        mutations.push(value);
        let mut value = base.clone();
        value.arbitrum_token_address = Address::repeat_byte(0x8a);
        mutations.push(value);
        let mut value = base.clone();
        value.arbitrum_token_runtime_code_hash = B256::repeat_byte(0x8b);
        mutations.push(value);
        let mut value = base.clone();
        value.ethereum_ingress_anchor_address = Address::repeat_byte(0x8c);
        mutations.push(value);
        let mut value = base;
        value.ethereum_ingress_anchor_runtime_code_hash = B256::repeat_byte(0x8d);
        mutations.push(value);
        assert!(mutations
            .iter()
            .all(|mutation| ingress_policy_hash_v2(mutation) != expected));
    }

    #[test]
    fn ingress_policy_hash_matches_keccak_conformance_vector() {
        let vector = PfUsdcIngressProofPolicyV2 {
            schema: PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V2.to_string(),
            ethereum_chain_id: 1,
            ethereum_genesis_validators_root: B256::repeat_byte(0x11),
            arbitrum_chain_id: 42_161,
            arbitrum_rollup_address: Address::repeat_byte(0x22),
            arbitrum_rollup_runtime_code_hash: B256::repeat_byte(0x33),
            rollup_latest_confirmed_storage_slot: B256::repeat_byte(0x44),
            arbitrum_vault_address: Address::repeat_byte(0x55),
            arbitrum_vault_runtime_code_hash: B256::repeat_byte(0x66),
            arbitrum_token_address: Address::repeat_byte(0x77),
            arbitrum_token_runtime_code_hash: B256::repeat_byte(0x88),
            ethereum_ingress_anchor_address: Address::repeat_byte(0x99),
            ethereum_ingress_anchor_runtime_code_hash: B256::repeat_byte(0xaa),
        };
        assert_eq!(
            ingress_policy_hash_v2(&vector),
            "733caef1634282b0d659ea837e52e93ec33bb62dede10899d5a482142ca56103"
        );
    }

    #[test]
    fn evm_address_text_is_canonical_lowercase_for_live_style_addresses() {
        let address: Address = "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d"
            .parse()
            .expect("address");
        assert_eq!(
            evm_address_text(address),
            "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d"
        );
    }

    #[test]
    fn canonical_record_deposit_call_decodes_top_level_dynamic_arguments() {
        let expected = recordDepositV1Call {
            deposit_id: B256::repeat_byte(0x11),
            depositor: Address::repeat_byte(0x22),
            recipient_hash: B256::repeat_byte(0x33),
            recipient: "pftl1tier4regression".to_string(),
            amount: U256::from(1_000_000_u64),
            nonce: B256::repeat_byte(0x44),
            route_binding: B256::repeat_byte(0x55),
            chain_id: U256::from(ARBITRUM_SEPOLIA_CHAIN_ID),
            vault: Address::repeat_byte(0x66),
            token: Address::repeat_byte(0x77),
        };
        let calldata = expected.abi_encode();
        let decoded = recordDepositV1Call::abi_decode_validate(&calldata)
            .expect("canonical Solidity call must decode without a tuple offset");
        assert_eq!(decoded.abi_encode(), calldata);
        assert_eq!(decoded.recipient, "pftl1tier4regression");
        assert_eq!(decoded.amount, U256::from(1_000_000_u64));
    }

    #[test]
    fn supported_network_binding_rejects_cross_network_mixtures() {
        let mainnet = policy();
        assert!(supported_network_binding(&mainnet));

        let mut sepolia = mainnet.clone();
        sepolia.ethereum_chain_id = ETHEREUM_SEPOLIA_CHAIN_ID;
        sepolia.ethereum_genesis_validators_root = ETHEREUM_SEPOLIA_GENESIS_VALIDATORS_ROOT;
        sepolia.arbitrum_chain_id = ARBITRUM_SEPOLIA_CHAIN_ID;
        sepolia.arbitrum_rollup_address = ARBITRUM_SEPOLIA_ROLLUP_ADDRESS;
        assert!(supported_network_binding(&sepolia));

        let mut cross_network = sepolia.clone();
        cross_network.arbitrum_rollup_address = ARBITRUM_ONE_ROLLUP_ADDRESS;
        assert!(!supported_network_binding(&cross_network));

        let mut wrong_slot = sepolia;
        wrong_slot.rollup_latest_confirmed_storage_slot = B256::repeat_byte(0x74);
        assert!(!supported_network_binding(&wrong_slot));
    }

    #[test]
    fn bold_assertion_hash_matches_pinned_solidity_vector() {
        let assertion = NitroAssertionWitnessV1 {
            parent_assertion_hash: B256::repeat_byte(0x11),
            block_hash: B256::repeat_byte(0x22),
            send_root: B256::repeat_byte(0x33),
            inbox_position: 1,
            position_in_message: 0,
            machine_status: 1,
            end_history_root: B256::repeat_byte(0x44),
            inbox_accumulator: B256::repeat_byte(0x55),
        };
        let state = NitroAssertionStateV1 {
            globalState: NitroGlobalStateV1 {
                bytes32Vals: [assertion.block_hash, assertion.send_root],
                u64Vals: [assertion.inbox_position, assertion.position_in_message],
            },
            machineStatus: assertion.machine_status,
            endHistoryRoot: assertion.end_history_root,
        };
        assert_eq!(
            keccak256(state.abi_encode()),
            "e5460f927b4570a316bd9d6455ca47aa2dcc52bbda1c530579124d8ba1ad210a"
                .parse::<B256>()
                .expect("pinned Nitro AssertionState hash vector")
        );
        assert_eq!(
            nitro_assertion_hash(&assertion),
            "cd5427de6f33a41b79699cd41cb5d4adad5a88b1d6a253913acd95f27010c434"
                .parse::<B256>()
                .expect("pinned Nitro assertion hash vector")
        );
    }

    #[test]
    fn nitro_output_root_binds_every_item_and_path_field() {
        let output = NitroSendWitnessV1 {
            output_index: 1,
            output_proof: vec![B256::repeat_byte(0x99), B256::repeat_byte(0x98)],
            l2_sender: Address::repeat_byte(0x11),
            destination: Address::repeat_byte(0x22),
            l2_block_number: 7,
            l1_block_number: 8,
            timestamp: 9,
            value: U256::ZERO,
            calldata: Bytes::from_static(b"tier4"),
        };
        let item = nitro_output_item_hash(&output);
        assert_eq!(
            item,
            "f1f98fe000af938f0626c1aa9590fb6344252302d1b4acb388a10b043e756f81"
                .parse::<B256>()
                .expect("pinned Nitro item hash vector")
        );
        let root = nitro_output_root(&output, item).expect("valid Nitro proof");
        assert_eq!(
            root,
            "4f48db66d9a031e08369f9e98df246d2e80b652711b532d3236c81d6ff187d66"
                .parse::<B256>()
                .expect("pinned Nitro sendRoot vector")
        );
        let mut changed = output.clone();
        changed.calldata = Bytes::from_static(b"tier5");
        assert_ne!(
            nitro_output_root(&changed, nitro_output_item_hash(&changed))
                .expect("mutated item remains structurally valid"),
            root
        );
        let mut changed = output.clone();
        changed.output_index = 0;
        assert_ne!(
            nitro_output_root(&changed, nitro_output_item_hash(&changed))
                .expect("mutated index remains structurally valid"),
            root
        );
        let mut changed = output;
        changed.output_proof[0] = B256::repeat_byte(0x97);
        assert_ne!(
            nitro_output_root(&changed, nitro_output_item_hash(&changed))
                .expect("mutated path remains structurally valid"),
            root
        );

        let mut invalid_index = changed;
        invalid_index.output_index = 4;
        assert!(nitro_output_root(&invalid_index, nitro_output_item_hash(&invalid_index)).is_err());

        let mut single_leaf = invalid_index;
        single_leaf.output_index = 0;
        single_leaf.output_proof.clear();
        let single_item = nitro_output_item_hash(&single_leaf);
        assert_eq!(
            nitro_output_root(&single_leaf, single_item).expect("single-leaf Nitro tree"),
            keccak256(single_item.as_slice())
        );
    }

    #[test]
    fn account_code_proof_binds_state_root_address_and_runtime_hash() {
        let accounts = [
            (Address::repeat_byte(0x11), B256::repeat_byte(0x21)),
            (Address::repeat_byte(0x12), B256::repeat_byte(0x22)),
            (Address::repeat_byte(0x13), B256::repeat_byte(0x23)),
        ];
        let mut leaves = BTreeMap::new();
        let mut values = BTreeMap::new();
        for (address, code_hash) in accounts {
            let account = TrieAccount {
                nonce: 1,
                balance: U256::from(2),
                storage_root: EMPTY_ROOT_HASH,
                code_hash,
            };
            let key = keccak256(address.as_slice());
            let mut value = Vec::new();
            account.encode(&mut value);
            leaves.insert(key, value);
            values.insert(address, account);
        }
        let targets = leaves.keys().copied().map(Nibbles::unpack);
        let mut builder =
            HashBuilder::default().with_proof_retainer(ProofRetainer::from_iter(targets));
        for (key, value) in &leaves {
            builder.add_leaf(Nibbles::unpack(*key), value);
        }
        let state_root = builder.root();
        let proof_nodes = builder.take_proof_nodes();
        let address = Address::repeat_byte(0x12);
        let path = Nibbles::unpack(keccak256(address.as_slice()));
        let account = ContractStorage {
            address,
            value: values[&address].clone(),
            mpt_proof: proof_nodes
                .matching_nodes_sorted(&path)
                .into_iter()
                .map(|(_, node)| node)
                .collect(),
            storage_slots: vec![],
        };
        verify_account_code(
            state_root,
            &account,
            address,
            B256::repeat_byte(0x22),
            "test account",
        )
        .expect("valid account/code proof");
        assert!(verify_account_code(
            state_root,
            &account,
            address,
            B256::repeat_byte(0x24),
            "test account",
        )
        .is_err());
        assert!(verify_account_code(
            B256::repeat_byte(0xff),
            &account,
            address,
            B256::repeat_byte(0x22),
            "test account",
        )
        .is_err());
    }
}
