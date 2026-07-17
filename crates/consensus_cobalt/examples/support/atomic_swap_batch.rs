use postfiat_consensus_cobalt::CobaltDomain;
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed,
    ml_dsa_65_sign_with_context_seed, ml_dsa_65_verify, ML_DSA_65_ALGORITHM, TX_SIGNATURE_CONTEXT,
};
use postfiat_mempool_dag::{
    build_mixed_transaction_batch_with_atomic_swaps, reference_for_batch, verify_batch_payload,
    BatchReference, MempoolBatchDomain,
};
use postfiat_types::{
    AtomicSwapAuthorization, AtomicSwapLeg, SignedAtomicSwapTransaction, TransactionBatch,
    UnsignedAtomicSwapTransaction, ADDRESS_NAMESPACE,
};

#[derive(Debug)]
pub struct AtomicSwapBatchFixture {
    pub batch: TransactionBatch,
    pub reference: BatchReference,
    pub serialized_batch: Vec<u8>,
}

pub fn build_atomic_swap_batch(
    cobalt_domain: &CobaltDomain,
) -> Result<AtomicSwapBatchFixture, String> {
    // Example-only deterministic keys and signing randomness keep the serialized
    // payload hash reproducible. They are never reachable from shipping code.
    let owner_0_key = ml_dsa_65_keygen_from_seed(&[0x31; 32]);
    let owner_1_key = ml_dsa_65_keygen_from_seed(&[0x32; 32]);
    let owner_0 = address_from_public_key(&owner_0_key.public_key);
    let owner_1 = address_from_public_key(&owner_1_key.public_key);
    let unsigned = UnsignedAtomicSwapTransaction {
        chain_id: cobalt_domain.chain_id.clone(),
        genesis_hash: cobalt_domain.genesis_hash.clone(),
        protocol_version: cobalt_domain.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        rfq_hash: "11".repeat(48),
        market_envelope_hash: "00".repeat(48),
        nav_epoch: 0,
        expires_at_height: 1_024,
        swap_nonce: "22".repeat(48),
        leg_0: AtomicSwapLeg {
            owner: owner_0.clone(),
            recipient: owner_1.clone(),
            issuer: format!("pf{}", "03".repeat(20)),
            asset_id: "10".repeat(48),
            amount: 20_000,
            sequence: 3,
            fee: 1_000,
        },
        leg_1: AtomicSwapLeg {
            owner: owner_1.clone(),
            recipient: owner_0.clone(),
            issuer: format!("pf{}", "04".repeat(20)),
            asset_id: "20".repeat(48),
            amount: 164_020,
            sequence: 5,
            fee: 1_000,
        },
    };
    let signing_bytes = unsigned.signing_bytes();
    let signature_0 = ml_dsa_65_sign_with_context_seed(
        &owner_0_key.private_key,
        &signing_bytes,
        TX_SIGNATURE_CONTEXT,
        &[0x41; 32],
    )
    .map_err(|error| error.to_string())?;
    let signature_1 = ml_dsa_65_sign_with_context_seed(
        &owner_1_key.private_key,
        &signing_bytes,
        TX_SIGNATURE_CONTEXT,
        &[0x42; 32],
    )
    .map_err(|error| error.to_string())?;
    if !ml_dsa_65_verify(&owner_0_key.public_key, &signing_bytes, &signature_0)
        || !ml_dsa_65_verify(&owner_1_key.public_key, &signing_bytes, &signature_1)
    {
        return Err("deterministic atomic-swap fixture signature verification failed".to_string());
    }
    let transaction = SignedAtomicSwapTransaction {
        unsigned,
        authorization_0: AtomicSwapAuthorization {
            owner: owner_0,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_0_key.public_key),
            signature_hex: bytes_to_hex(&signature_0),
        },
        authorization_1: AtomicSwapAuthorization {
            owner: owner_1,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&owner_1_key.public_key),
            signature_hex: bytes_to_hex(&signature_1),
        },
    };
    transaction.validate()?;

    let domain = MempoolBatchDomain {
        chain_id: cobalt_domain.chain_id.clone(),
        genesis_hash: cobalt_domain.genesis_hash.clone(),
        protocol_version: cobalt_domain.protocol_version,
    };
    let available = build_mixed_transaction_batch_with_atomic_swaps(
        &domain,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![transaction],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| error.to_string())?;
    let serialized_batch =
        serde_json::to_vec(&available.batch).map_err(|error| error.to_string())?;
    let restored_batch: TransactionBatch =
        serde_json::from_slice(&serialized_batch).map_err(|error| error.to_string())?;
    if restored_batch != available.batch {
        return Err("serialized atomic-swap batch did not round-trip exactly".to_string());
    }
    let restored_reference =
        reference_for_batch(&domain, &restored_batch).map_err(|error| error.to_string())?;
    if restored_reference != available.reference {
        return Err("serialized atomic-swap batch reference changed after round-trip".to_string());
    }
    verify_batch_payload(&domain, &restored_batch, &available.reference)
        .map_err(|error| error.to_string())?;

    Ok(AtomicSwapBatchFixture {
        batch: restored_batch,
        reference: available.reference,
        serialized_batch,
    })
}
