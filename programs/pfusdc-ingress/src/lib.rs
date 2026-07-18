use alloy_primitives::{keccak256, Address, Bytes, B256, U256};
use alloy_sol_types::{sol, SolValue};
use helios_consensus_core::{
    apply_finality_update, apply_update, verify_finality_update, verify_update,
};
use postfiat_types::{
    vault_bridge_deposit_evidence_root, vault_bridge_route_binding, PfUsdcIngressPublicValuesV2,
    VaultBridgeDepositEvidence, VaultBridgeRouteProfileV1,
    NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1, PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V2,
    VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN,
};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_384};
use sp1_helios_primitives::{
    types::{ContractStorage, ProofInputs},
    verify_storage_slot_proofs,
};
use tree_hash::TreeHash;

pub const PFUSDC_INGRESS_PROOF_WITNESS_SCHEMA_V1: &str = "postfiat.pfusdc.ingress_proof_witness.v1";
pub const PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V1: &str = "postfiat.pfusdc.ingress_proof_policy.v1";
pub const PFUSDC_INGRESS_PROOF_PROGRAM_VERSION_V2: u32 = 2;
pub const ETHEREUM_MAINNET_CHAIN_ID: u64 = 1;
pub const ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT: B256 = B256::new([
    0x4b, 0x36, 0x3d, 0xb9, 0x4e, 0x28, 0x61, 0x20, 0xd7, 0x6e, 0xb9, 0x05, 0x34, 0x0f, 0xdd, 0x4e,
    0x54, 0xbf, 0xe9, 0xf0, 0x6b, 0xf3, 0x3f, 0xf6, 0xcf, 0x5a, 0xd2, 0x7f, 0x51, 0x1b, 0xfe, 0x95,
]);
const MAX_HELIOS_UPDATES_V1: usize = 8;
const MAX_MPT_NODES_V1: usize = 64;
const MAX_MPT_NODE_BYTES_V1: usize = 16_384;
const MAX_OUTPUT_PROOF_NODES_V1: usize = 64;
const MAX_OUTPUT_CALLDATA_BYTES_V1: usize = 2_048;

sol! {
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
pub struct PfUsdcIngressProofPolicyV1 {
    pub schema: String,
    pub ethereum_chain_id: u64,
    pub ethereum_genesis_validators_root: B256,
    pub arbitrum_chain_id: u64,
    pub arbitrum_rollup_address: Address,
    pub arbitrum_rollup_runtime_code_hash: B256,
    pub rollup_latest_confirmed_storage_slot: B256,
    pub arbitrum_ingress_anchor_address: Address,
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
pub struct PfUsdcIngressProofWitnessV1 {
    pub schema: String,
    pub route_profile: VaultBridgeRouteProfileV1,
    pub policy: PfUsdcIngressProofPolicyV1,
    pub helios: ProofInputs,
    pub rollup_storage: ContractStorage,
    pub assertion: NitroAssertionWitnessV1,
    pub output: NitroSendWitnessV1,
    pub evidence: VaultBridgeDepositEvidence,
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
}

pub fn verify_ingress_witness_v1(
    witness: &PfUsdcIngressProofWitnessV1,
) -> Result<PfUsdcIngressPublicValuesV2, String> {
    validate_bounds(witness)?;
    witness.route_profile.validate()?;
    witness.evidence.validate()?;
    verify_route_and_policy(witness)?;

    let (prior_root, prior_slot, final_root, final_slot, ethereum_state_root) =
        verify_ethereum_finality(&witness.helios, &witness.policy)?;
    let assertion_hash = verify_arbitrum_assertion(
        ethereum_state_root,
        &witness.rollup_storage,
        &witness.policy,
        &witness.assertion,
    )?;
    let output_item_hash = verify_confirmed_deposit_output(witness)?;
    let profile_hash = witness.route_profile.profile_hash()?;
    let evidence_root = vault_bridge_deposit_evidence_root(&witness.evidence)?;
    let mut values = PfUsdcIngressPublicValuesV2 {
        schema: PFUSDC_INGRESS_PUBLIC_VALUES_SCHEMA_V2.to_string(),
        proof_program_version: PFUSDC_INGRESS_PROOF_PROGRAM_VERSION_V2,
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
        arbitrum_rollup_address: witness.policy.arbitrum_rollup_address.to_string(),
        arbitrum_assertion_hash: hex32(assertion_hash),
        assertion_l2_block_hash: hex32(witness.assertion.block_hash),
        assertion_send_root: hex32(witness.assertion.send_root),
        output_index: witness.output.output_index,
        output_item_hash: hex32(output_item_hash),
        output_l2_block_number: witness.output.l2_block_number,
        output_l1_block_number: witness.output.l1_block_number,
        output_timestamp: witness.output.timestamp,
        output_destination: witness.output.destination.to_string(),
        output_calldata_hash: hex32(keccak256(&witness.output.calldata)),
        vault_address: witness.evidence.vault_address.clone(),
        vault_runtime_code_hash: strip_0x(&witness.route_profile.vault_runtime_code_hash),
        token_address: witness.evidence.token_address.clone(),
        token_runtime_code_hash: strip_0x(&witness.route_profile.token_runtime_code_hash),
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

fn verify_route_and_policy(witness: &PfUsdcIngressProofWitnessV1) -> Result<(), String> {
    let profile = &witness.route_profile;
    let policy = &witness.policy;
    if profile.verifier_kind != NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
        || profile.evidence_tier != VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN
        || profile.source_chain_id != policy.arbitrum_chain_id
        || profile.source_chain_id != witness.evidence.source_chain_id
        || profile.vault_address != witness.evidence.vault_address
        || profile.token_address != witness.evidence.token_address
        || policy.ethereum_chain_id != ETHEREUM_MAINNET_CHAIN_ID
        || policy.ethereum_genesis_validators_root != ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT
    {
        return Err("pfUSDC ingress route/policy/evidence binding mismatch".to_string());
    }
    let route_binding = vault_bridge_route_binding(&profile.profile_hash()?, profile.route_epoch)?;
    if route_binding != witness.evidence.route_binding {
        return Err("deposit event does not bind the governed Tier-4 route".to_string());
    }
    if ingress_policy_hash(policy) != profile.verifier_policy_hash {
        return Err("ingress proof policy hash does not match governed route".to_string());
    }
    Ok(())
}

fn verify_ethereum_finality(
    inputs: &ProofInputs,
    policy: &PfUsdcIngressProofPolicyV1,
) -> Result<(B256, u64, B256, u64, B256), String> {
    if inputs.genesis_root != policy.ethereum_genesis_validators_root
        || !inputs.contract_storage.is_empty()
        || inputs.expected_current_slot != *inputs.finality_update.signature_slot()
    {
        return Err("Helios input does not match pinned Ethereum policy".to_string());
    }
    require_mainnet_forks(&inputs.forks)?;
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

fn require_mainnet_forks(forks: &helios_consensus_core::types::Forks) -> Result<(), String> {
    let expected = [
        (&forks.genesis, 0, [0, 0, 0, 0]),
        (&forks.altair, 74_240, [1, 0, 0, 0]),
        (&forks.bellatrix, 144_896, [2, 0, 0, 0]),
        (&forks.capella, 194_048, [3, 0, 0, 0]),
        (&forks.deneb, 269_568, [4, 0, 0, 0]),
        (&forks.electra, 364_032, [5, 0, 0, 0]),
        (&forks.fulu, 411_392, [6, 0, 0, 0]),
    ];
    if expected.iter().any(|(fork, epoch, version)| {
        fork.epoch != *epoch || fork.fork_version.as_slice() != version
    }) {
        return Err("Helios fork schedule is not pinned Ethereum mainnet".to_string());
    }
    Ok(())
}

fn verify_arbitrum_assertion(
    ethereum_state_root: B256,
    storage: &ContractStorage,
    policy: &PfUsdcIngressProofPolicyV1,
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
    let assertion_hash = keccak256(assertion_preimage);
    let proved_value = B256::from(storage.storage_slots[0].value.to_be_bytes::<32>());
    if proved_value != assertion_hash || verified[0].value != assertion_hash {
        return Err("RollupCore latestConfirmed does not equal Nitro assertion hash".to_string());
    }
    Ok(assertion_hash)
}

fn verify_confirmed_deposit_output(witness: &PfUsdcIngressProofWitnessV1) -> Result<B256, String> {
    let output = &witness.output;
    let evidence = &witness.evidence;
    if output.l2_sender.to_string() != evidence.vault_address
        || output.destination != witness.policy.arbitrum_ingress_anchor_address
        || output.value != U256::ZERO
    {
        return Err(
            "Arbitrum output sender/destination/value does not match governed ingress".to_string(),
        );
    }

    let item_hash = nitro_output_item_hash(output);
    let root = nitro_output_root(output, item_hash);
    if root != witness.assertion.send_root {
        return Err(
            "Arbitrum output Merkle proof does not match confirmed assertion sendRoot".to_string(),
        );
    }

    let selector = keccak256(b"recordDepositV1(bytes32,address,bytes32,string,uint256,bytes32,bytes32,uint256,address,address)");
    if output.calldata.len() < 4 || output.calldata[..4] != selector[..4] {
        return Err("Arbitrum output is not the canonical Tier-4 deposit call".to_string());
    }
    type DepositCall = (
        B256,
        Address,
        B256,
        String,
        U256,
        B256,
        B256,
        U256,
        Address,
        Address,
    );
    let (
        deposit_id,
        depositor,
        recipient_hash,
        recipient,
        amount,
        nonce,
        route_binding,
        chain_id,
        vault,
        token,
    ) = DepositCall::abi_decode(&output.calldata[4..])
        .map_err(|error| format!("invalid canonical Tier-4 deposit calldata: {error}"))?;
    if hex32(deposit_id) != evidence.deposit_id
        || depositor.to_string() != evidence.depositor
        || hex32(recipient_hash) != evidence.pftl_recipient_hash
        || recipient != evidence.pftl_recipient
        || u256_u64(amount, "amount")? != evidence.amount_atoms
        || hex32(nonce) != evidence.nonce
        || hex32(route_binding) != evidence.route_binding
        || u256_u64(chain_id, "source chain ID")? != evidence.source_chain_id
        || vault.to_string() != evidence.vault_address
        || token.to_string() != evidence.token_address
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

fn nitro_output_root(output: &NitroSendWitnessV1, item_hash: B256) -> B256 {
    let mut root = keccak256(item_hash.as_slice());
    for (level, sibling) in output.output_proof.iter().enumerate() {
        let mut pair = [0_u8; 64];
        if output.output_index & (1_u64 << level) == 0 {
            pair[..32].copy_from_slice(root.as_slice());
            pair[32..].copy_from_slice(sibling.as_slice());
        } else {
            pair[..32].copy_from_slice(sibling.as_slice());
            pair[32..].copy_from_slice(root.as_slice());
        }
        root = keccak256(pair);
    }
    root
}

fn append_u256(bytes: &mut Vec<u8>, value: U256) {
    bytes.extend_from_slice(&value.to_be_bytes::<32>());
}

fn ingress_policy_hash(policy: &PfUsdcIngressProofPolicyV1) -> String {
    let mut bytes = b"PFTL-PFUSDC-INGRESS-POLICY-V1".to_vec();
    bytes.extend_from_slice(&policy.ethereum_chain_id.to_be_bytes());
    bytes.extend_from_slice(policy.ethereum_genesis_validators_root.as_slice());
    bytes.extend_from_slice(&policy.arbitrum_chain_id.to_be_bytes());
    bytes.extend_from_slice(policy.arbitrum_rollup_address.as_slice());
    bytes.extend_from_slice(policy.arbitrum_rollup_runtime_code_hash.as_slice());
    bytes.extend_from_slice(policy.rollup_latest_confirmed_storage_slot.as_slice());
    bytes.extend_from_slice(policy.arbitrum_ingress_anchor_address.as_slice());
    let digest = Sha3_384::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
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

fn validate_bounds(witness: &PfUsdcIngressProofWitnessV1) -> Result<(), String> {
    if witness.schema != PFUSDC_INGRESS_PROOF_WITNESS_SCHEMA_V1
        || witness.policy.schema != PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V1
    {
        return Err("pfUSDC ingress witness/policy schema mismatch".to_string());
    }
    if witness.helios.updates.len() > MAX_HELIOS_UPDATES_V1
        || witness.output.output_proof.is_empty()
        || witness.output.output_proof.len() > MAX_OUTPUT_PROOF_NODES_V1
        || witness.output.calldata.len() > MAX_OUTPUT_CALLDATA_BYTES_V1
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
    Ok(())
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

    fn policy() -> PfUsdcIngressProofPolicyV1 {
        PfUsdcIngressProofPolicyV1 {
            schema: PFUSDC_INGRESS_PROOF_POLICY_SCHEMA_V1.to_string(),
            ethereum_chain_id: ETHEREUM_MAINNET_CHAIN_ID,
            ethereum_genesis_validators_root: ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT,
            arbitrum_chain_id: 42_161,
            arbitrum_rollup_address: Address::repeat_byte(0x11),
            arbitrum_rollup_runtime_code_hash: B256::repeat_byte(0x22),
            rollup_latest_confirmed_storage_slot: B256::repeat_byte(0x33),
            arbitrum_ingress_anchor_address: Address::repeat_byte(0x44),
        }
    }

    #[test]
    fn ingress_policy_hash_binds_every_policy_field_class() {
        let base = policy();
        let expected = ingress_policy_hash(&base);
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
        let mut value = base;
        value.arbitrum_ingress_anchor_address = Address::repeat_byte(0x88);
        mutations.push(value);
        assert!(mutations
            .iter()
            .all(|mutation| ingress_policy_hash(mutation) != expected));
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
        let root = nitro_output_root(&output, item);
        let mut changed = output.clone();
        changed.calldata = Bytes::from_static(b"tier5");
        assert_ne!(
            nitro_output_root(&changed, nitro_output_item_hash(&changed)),
            root
        );
        let mut changed = output.clone();
        changed.output_index = 0;
        assert_ne!(
            nitro_output_root(&changed, nitro_output_item_hash(&changed)),
            root
        );
        let mut changed = output;
        changed.output_proof[0] = B256::repeat_byte(0x97);
        assert_ne!(
            nitro_output_root(&changed, nitro_output_item_hash(&changed)),
            root
        );
    }
}
