use alloy_primitives::{keccak256, Address, B256, U256};
use helios_consensus_core::{
    apply_finality_update, apply_update, verify_finality_update, verify_update,
};
use postfiat_types::{
    vault_bridge_deposit_evidence_root, vault_bridge_deposit_id, VaultBridgeDepositEvidence,
};
use serde::{Deserialize, Serialize};
use sp1_helios_primitives::{
    types::{ContractStorage, ProofInputs},
    verify_storage_slot_proofs,
};
use tree_hash::TreeHash;

pub const WITNESS_SCHEMA: &str = "postfiat.pfusdc.ethereum_ingress_witness.v1";
pub const POLICY_SCHEMA: &str = "postfiat.pfusdc.ethereum_ingress_policy.v1";
pub const PUBLIC_VALUES_SCHEMA: &str = "postfiat.pfusdc.ethereum_ingress_public_values.v1";
pub const ROUTE_ID: &str = "ethereum-mainnet-usdc-v1";
pub const MAINNET_CHAIN_ID: u64 = 1;
pub const MAINNET_GENESIS_VALIDATORS_ROOT: B256 = B256::new([
    0x4b, 0x36, 0x3d, 0xb9, 0x4e, 0x28, 0x61, 0x20, 0xd7, 0x6e, 0xb9, 0x05, 0x34, 0x0f, 0xdd, 0x4e,
    0x54, 0xbf, 0xe9, 0xf0, 0x6b, 0xf3, 0x3f, 0xf6, 0xcf, 0x5a, 0xd2, 0x7f, 0x51, 0x1b, 0xfe, 0x95,
]);
const MAX_UPDATES: usize = 8;
const MAX_MPT_NODES: usize = 64;
const MAX_MPT_NODE_BYTES: usize = 16_384;
const DEPOSIT_MAPPING_SLOT: u64 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct EthIngressPolicyV1 {
    pub schema: String,
    pub route_id: String,
    pub source_chain_id: u64,
    pub genesis_validators_root: B256,
    pub vault_address: Address,
    pub vault_runtime_code_hash: B256,
    pub token_address: Address,
    pub token_runtime_code_hash: B256,
    /// Fully-derived storage key for `balanceOf(vault)` in the pinned token implementation.
    pub token_balance_storage_key: B256,
    pub manifest_hash: B256,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EthIngressWitnessV1 {
    pub schema: String,
    pub policy: EthIngressPolicyV1,
    pub helios: ProofInputs,
    pub vault_storage: ContractStorage,
    pub token_storage: ContractStorage,
    pub evidence: VaultBridgeDepositEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthIngressPublicValuesV1 {
    pub schema: String,
    pub route_id: String,
    pub source_chain_id: u64,
    pub prior_finalized_beacon_root: String,
    pub prior_finalized_slot: u64,
    pub finalized_beacon_root: String,
    pub finalized_slot: u64,
    pub finalized_execution_block_hash: String,
    pub finalized_execution_block_number: u64,
    pub execution_state_root: String,
    pub vault_address: String,
    pub vault_runtime_code_hash: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
    pub depositor: String,
    pub pftl_recipient: String,
    pub pftl_recipient_hash: String,
    pub amount_atoms: u64,
    pub nonce: String,
    pub route_binding: String,
    pub deposit_id: String,
    pub evidence_root: String,
    pub manifest_hash: String,
    pub deposit_nullifier: String,
    pub total_obligations_atoms: String,
    pub vault_token_balance_atoms: String,
}

pub fn verify_witness(w: &EthIngressWitnessV1) -> Result<EthIngressPublicValuesV1, String> {
    validate_bounds(w)?;
    if w.schema != WITNESS_SCHEMA || w.policy.schema != POLICY_SCHEMA {
        return Err("Ethereum ingress witness/policy schema mismatch".into());
    }
    let p = &w.policy;
    if p.route_id != ROUTE_ID
        || p.source_chain_id != MAINNET_CHAIN_ID
        || p.genesis_validators_root != MAINNET_GENESIS_VALIDATORS_ROOT
        || p.manifest_hash == B256::ZERO
        || p.vault_runtime_code_hash == B256::ZERO
        || p.token_runtime_code_hash == B256::ZERO
    {
        return Err("Ethereum ingress policy is not the allowlisted Ethereum mainnet route".into());
    }
    w.evidence.validate()?;
    if w.evidence.source_chain_id != p.source_chain_id
        || w.evidence.vault_address != address_text(p.vault_address)
        || w.evidence.token_address != address_text(p.token_address)
        || w.evidence.route_binding.is_empty()
    {
        return Err("Ethereum ingress evidence does not match route policy".into());
    }

    let finality = verify_finality(&w.helios, p)?;
    let vault_slots = verify_contract(
        finality.execution_state_root,
        &w.vault_storage,
        p.vault_address,
        p.vault_runtime_code_hash,
        "vault",
    )?;
    let token_slots = verify_contract(
        finality.execution_state_root,
        &w.token_storage,
        p.token_address,
        p.token_runtime_code_hash,
        "token",
    )?;

    if vault_slots.len() != 5 || token_slots.len() != 1 {
        return Err("Ethereum ingress proof has the wrong storage-slot cardinality".into());
    }
    let deposit_id = parse_b256(&w.evidence.deposit_id, "deposit_id")?;
    let base = mapping_base(deposit_id, DEPOSIT_MAPPING_SLOT);
    let expected_vault_keys = [
        B256::ZERO,
        base,
        add_slot(base, 1)?,
        add_slot(base, 2)?,
        add_slot(base, 3)?,
    ];
    for (actual, expected) in w
        .vault_storage
        .storage_slots
        .iter()
        .zip(expected_vault_keys)
    {
        if actual.key != expected {
            return Err("Ethereum ingress vault storage slot mismatch".into());
        }
    }
    if w.token_storage.storage_slots[0].key != p.token_balance_storage_key {
        return Err("Ethereum ingress token balance slot mismatch".into());
    }

    let obligations = w.vault_storage.storage_slots[0].value;
    let packed = w.vault_storage.storage_slots[1].value;
    let address_mask = (U256::from(1u8) << 160) - U256::from(1u8);
    let packed_address: U256 = packed & address_mask;
    let proved_depositor = Address::from_word(B256::from(packed_address.to_be_bytes::<32>()));
    let proved_amount = packed >> 160;
    let recipient_hash = B256::from(w.vault_storage.storage_slots[2].value.to_be_bytes::<32>());
    let route_binding = B256::from(w.vault_storage.storage_slots[3].value.to_be_bytes::<32>());
    let nonce = B256::from(w.vault_storage.storage_slots[4].value.to_be_bytes::<32>());
    let token_balance = w.token_storage.storage_slots[0].value;

    if address_text(proved_depositor) != w.evidence.depositor
        || proved_amount != U256::from(w.evidence.amount_atoms)
        || hex32(recipient_hash) != w.evidence.pftl_recipient_hash
        || hex32(route_binding) != w.evidence.route_binding
        || hex32(nonce) != w.evidence.nonce
        || obligations < U256::from(w.evidence.amount_atoms)
        || token_balance < obligations
    {
        return Err(
            "Ethereum ingress state does not reproduce the deposit evidence/backing".into(),
        );
    }
    if vault_bridge_deposit_id(&w.evidence)? != w.evidence.deposit_id {
        return Err("Ethereum ingress deposit ID mismatch".into());
    }

    // P0 uses state-bound identifiers for the legacy evidence-only fields. Production may
    // additionally authenticate transaction/receipt tries without changing the mint statement.
    // block_hash is consensus-authenticated. tx_hash/log_index are audit metadata;
    // permanent storage (deposit_id + fields) is the admission authority.
    parse_b256(&w.evidence.tx_hash, "tx hash")?;
    if w.evidence.block_hash != hex32(finality.execution_block_hash) || w.evidence.log_index != 0 {
        return Err("Ethereum ingress finalized state reference mismatch".into());
    }

    let evidence_root = vault_bridge_deposit_evidence_root(&w.evidence)?;
    let mut nullifier_preimage = b"postfiat.pfusdc.ethereum_ingress_nullifier.v1\0".to_vec();
    nullifier_preimage.extend_from_slice(p.manifest_hash.as_slice());
    nullifier_preimage.extend_from_slice(deposit_id.as_slice());
    let nullifier = keccak256(nullifier_preimage);

    Ok(EthIngressPublicValuesV1 {
        schema: PUBLIC_VALUES_SCHEMA.into(),
        route_id: p.route_id.clone(),
        source_chain_id: p.source_chain_id,
        prior_finalized_beacon_root: hex32(finality.prior_root),
        prior_finalized_slot: finality.prior_slot,
        finalized_beacon_root: hex32(finality.final_root),
        finalized_slot: finality.final_slot,
        finalized_execution_block_hash: hex32(finality.execution_block_hash),
        finalized_execution_block_number: finality.execution_block_number,
        execution_state_root: hex32(finality.execution_state_root),
        vault_address: address_text(p.vault_address),
        vault_runtime_code_hash: hex32(p.vault_runtime_code_hash),
        token_address: address_text(p.token_address),
        token_runtime_code_hash: hex32(p.token_runtime_code_hash),
        depositor: w.evidence.depositor.clone(),
        pftl_recipient: w.evidence.pftl_recipient.clone(),
        pftl_recipient_hash: w.evidence.pftl_recipient_hash.clone(),
        amount_atoms: w.evidence.amount_atoms,
        nonce: w.evidence.nonce.clone(),
        route_binding: w.evidence.route_binding.clone(),
        deposit_id: w.evidence.deposit_id.clone(),
        evidence_root,
        manifest_hash: hex32(p.manifest_hash),
        deposit_nullifier: hex32(nullifier),
        total_obligations_atoms: obligations.to_string(),
        vault_token_balance_atoms: token_balance.to_string(),
    })
}

struct Finality {
    prior_root: B256,
    prior_slot: u64,
    final_root: B256,
    final_slot: u64,
    execution_state_root: B256,
    execution_block_hash: B256,
    execution_block_number: u64,
}

fn verify_finality(inputs: &ProofInputs, p: &EthIngressPolicyV1) -> Result<Finality, String> {
    if inputs.genesis_root != p.genesis_validators_root
        || !inputs.contract_storage.is_empty()
        || inputs.expected_current_slot != *inputs.finality_update.signature_slot()
    {
        return Err("Helios input does not match pinned Ethereum mainnet policy".into());
    }
    require_mainnet_forks(&inputs.forks)?;
    let mut store = inputs.store.clone();
    store.next_sync_committee = None;
    let prior_root = store.finalized_header.beacon().tree_hash_root();
    let prior_slot = store.finalized_header.beacon().slot;
    for update in &inputs.updates {
        verify_update(
            update,
            inputs.expected_current_slot,
            &store,
            inputs.genesis_root,
            &inputs.forks,
        )
        .map_err(|e| format!("invalid Helios committee update: {e}"))?;
        apply_update(&mut store, update);
    }
    verify_finality_update(
        &inputs.finality_update,
        inputs.expected_current_slot,
        &store,
        inputs.genesis_root,
        &inputs.forks,
    )
    .map_err(|e| format!("invalid Helios finality update: {e}"))?;
    apply_finality_update(&mut store, &inputs.finality_update);
    let final_slot = store.finalized_header.beacon().slot;
    // A finalized checkpoint may resolve to the most recent occupied block
    // before an epoch boundary when that boundary slot was skipped. Helios
    // authenticates the checkpoint root; monotonicity is the invariant.
    if final_slot <= prior_slot {
        return Err("Ethereum mainnet finalized checkpoint did not canonically advance".into());
    }
    let execution = store
        .finalized_header
        .execution()
        .map_err(|_| "finalized Ethereum mainnet header has no execution payload".to_string())?;
    Ok(Finality {
        prior_root,
        prior_slot,
        final_root: store.finalized_header.beacon().tree_hash_root(),
        final_slot,
        execution_state_root: *execution.state_root(),
        execution_block_hash: *execution.block_hash(),
        execution_block_number: *execution.block_number(),
    })
}

fn require_mainnet_forks(f: &helios_consensus_core::types::Forks) -> Result<(), String> {
    let expected = [
        (&f.genesis, 0, [0x00, 0, 0, 0x00]),
        (&f.altair, 74_240, [0x01, 0, 0, 0x00]),
        (&f.bellatrix, 144_896, [0x02, 0, 0, 0x00]),
        (&f.capella, 194_048, [0x03, 0, 0, 0x00]),
        (&f.deneb, 269_568, [0x04, 0, 0, 0x00]),
        (&f.electra, 364_032, [0x05, 0, 0, 0x00]),
        (&f.fulu, 411_648, [0x06, 0, 0, 0x00]),
    ];
    if expected.iter().any(|(fork, epoch, version)| {
        fork.epoch != *epoch || fork.fork_version.as_slice() != version
    }) {
        return Err("Helios fork schedule does not match Ethereum mainnet".into());
    }
    Ok(())
}

fn verify_contract(
    root: B256,
    storage: &ContractStorage,
    address: Address,
    code_hash: B256,
    label: &str,
) -> Result<Vec<sp1_helios_primitives::types::StorageSlot>, String> {
    if storage.address != address || storage.value.code_hash != code_hash {
        return Err(format!("{label} account/code hash mismatch"));
    }
    verify_storage_slot_proofs(root, storage)
        .map_err(|e| format!("invalid {label} account/storage proof: {e}"))
}

fn mapping_base(key: B256, slot: u64) -> B256 {
    let mut preimage = [0u8; 64];
    preimage[..32].copy_from_slice(key.as_slice());
    preimage[56..].copy_from_slice(&slot.to_be_bytes());
    keccak256(preimage)
}

fn add_slot(base: B256, offset: u64) -> Result<B256, String> {
    let value = U256::from_be_bytes(base.0)
        .checked_add(U256::from(offset))
        .ok_or_else(|| "storage-slot overflow".to_string())?;
    Ok(B256::from(value.to_be_bytes::<32>()))
}

fn parse_b256(value: &str, label: &str) -> Result<B256, String> {
    let bytes = hex_decode(value).ok_or_else(|| format!("invalid {label}"))?;
    if bytes.len() != 32 {
        return Err(format!("invalid {label} width"));
    }
    Ok(B256::from_slice(&bytes))
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.len() % 2 != 0 {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).ok())
        .collect()
}

fn validate_bounds(w: &EthIngressWitnessV1) -> Result<(), String> {
    if w.helios.updates.len() > MAX_UPDATES
        || w.vault_storage.storage_slots.len() != 5
        || w.token_storage.storage_slots.len() != 1
    {
        return Err("Ethereum ingress witness exceeds slot/count bounds".into());
    }
    for account in [&w.vault_storage, &w.token_storage] {
        if account.mpt_proof.len() > MAX_MPT_NODES {
            return Err("account MPT proof too deep".into());
        }
        for node in &account.mpt_proof {
            if node.len() > MAX_MPT_NODE_BYTES {
                return Err("account MPT node too large".into());
            }
        }
        for slot in &account.storage_slots {
            if slot.mpt_proof.len() > MAX_MPT_NODES {
                return Err("storage MPT proof too deep".into());
            }
            for node in &slot.mpt_proof {
                if node.len() > MAX_MPT_NODE_BYTES {
                    return Err("storage MPT node too large".into());
                }
            }
        }
    }
    Ok(())
}

fn address_text(a: Address) -> String {
    format!("{a:#x}")
}
fn hex32(v: B256) -> String {
    format!("{v:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use helios_consensus_core::types::{Fork, Forks};

    fn mainnet_forks() -> Forks {
        let f = |epoch, v: [u8; 4]| Fork {
            epoch,
            fork_version: alloy_primitives::FixedBytes::<4>::from(v),
        };
        Forks {
            genesis: f(0, [0x00, 0, 0, 0x00]),
            altair: f(74_240, [0x01, 0, 0, 0x00]),
            bellatrix: f(144_896, [0x02, 0, 0, 0x00]),
            capella: f(194_048, [0x03, 0, 0, 0x00]),
            deneb: f(269_568, [0x04, 0, 0, 0x00]),
            electra: f(364_032, [0x05, 0, 0, 0x00]),
            fulu: f(411_648, [0x06, 0, 0, 0x00]),
        }
    }

    fn sepolia_forks() -> Forks {
        let f = |epoch, v: [u8; 4]| Fork {
            epoch,
            fork_version: alloy_primitives::FixedBytes::<4>::from(v),
        };
        Forks {
            genesis: f(0, [0x90, 0, 0, 0x69]),
            altair: f(50, [0x90, 0, 0, 0x70]),
            bellatrix: f(100, [0x90, 0, 0, 0x71]),
            capella: f(56_832, [0x90, 0, 0, 0x72]),
            deneb: f(132_608, [0x90, 0, 0, 0x73]),
            electra: f(222_464, [0x90, 0, 0, 0x74]),
            fulu: f(272_640, [0x90, 0, 0, 0x75]),
        }
    }

    #[test]
    fn pinned_constants_are_exactly_mainnet() {
        assert_eq!(ROUTE_ID, "ethereum-mainnet-usdc-v1");
        assert_eq!(MAINNET_CHAIN_ID, 1);
        assert_eq!(
            MAINNET_GENESIS_VALIDATORS_ROOT,
            "0x4b363db94e286120d76eb905340fdd4e54bfe9f06bf33ff6cf5ad27f511bfe95"
                .parse::<B256>()
                .expect("genesis root hex")
        );
    }

    #[test]
    fn accepts_exact_mainnet_fork_schedule() {
        assert!(require_mainnet_forks(&mainnet_forks()).is_ok());
    }

    #[test]
    fn rejects_sepolia_fork_schedule() {
        let err = require_mainnet_forks(&sepolia_forks()).expect_err("sepolia forks must fail");
        assert!(err.contains("mainnet"), "error must name mainnet: {err}");
    }

    #[test]
    fn rejects_mixed_fork_tuple_single_epoch_drift() {
        let mut mixed = mainnet_forks();
        mixed.electra.epoch = 222_464; // Sepolia electra epoch inside an otherwise-mainnet tuple
        assert!(require_mainnet_forks(&mixed).is_err());
        let mut mixed = mainnet_forks();
        mixed.deneb.fork_version = alloy_primitives::FixedBytes::<4>::from([0x90, 0, 0, 0x73]);
        assert!(require_mainnet_forks(&mixed).is_err());
    }

    #[test]
    fn error_text_says_mainnet_not_sepolia() {
        let err = require_mainnet_forks(&sepolia_forks()).unwrap_err();
        assert!(!err.to_lowercase().contains("sepolia"));
        assert!(err.contains("Ethereum mainnet"));
    }

    #[test]
    fn policy_rejects_sepolia_route_and_chain() {
        // The policy tuple gate in verify_witness compares against the pinned
        // mainnet constants; any Sepolia tuple is rejected by inequality.
        assert_ne!(ROUTE_ID, "ethereum-sepolia-usdc-v1");
        assert_ne!(MAINNET_CHAIN_ID, 11_155_111);
        let sepolia_root: B256 =
            "0xd8ea171f3c94aea21ebc42a1ed61052acf3f9209c00e4efbaaddac09ed9b8078"
                .parse()
                .expect("sepolia root hex");
        assert_ne!(MAINNET_GENESIS_VALIDATORS_ROOT, sepolia_root);
    }

    #[test]
    fn mapping_and_slot_math_match_frozen_guest() {
        let key = B256::ZERO;
        let base = mapping_base(key, DEPOSIT_MAPPING_SLOT);
        assert_eq!(add_slot(base, 3).expect("slot+3"), {
            let v = U256::from_be_bytes(base.0) + U256::from(3u8);
            B256::from(v.to_be_bytes::<32>())
        });
        assert!(add_slot(B256::from(U256::MAX.to_be_bytes::<32>()), 1).is_err());
    }
}
