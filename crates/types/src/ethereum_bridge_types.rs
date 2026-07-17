pub const ETHEREUM_CHECKPOINT_SCHEMA_V1: u32 = 1;
pub const ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1: &[u8] =
    b"postfiat-l1-v2/ethereum-checkpoint/v1";
pub const ETHEREUM_RECEIPT_PROOF_MAX_NODES: usize = 64;
pub const ETHEREUM_RECEIPT_PROOF_MAX_NODE_BYTES: usize = 64 * 1024;
pub const ETHEREUM_RECEIPT_PROOF_MAX_RECEIPT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumRouteVerificationPolicyV1 {
    pub authority_epoch: u64,
    pub committee_root: FastSwapCommitteeRootV1,
    pub minimum_confirmations: u32,
    pub handoff_controller_code_hash: [u8; 32],
    pub wrapped_navcoin_code_hash: [u8; 32],
}

impl EthereumRouteVerificationPolicyV1 {
    pub fn validate(&self) -> Result<(), FastSwapCodecError> {
        if self.authority_epoch == 0
            || self.committee_root == FastSwapCommitteeRootV1::ZERO
            || self.minimum_confirmations == 0
            || self.handoff_controller_code_hash == [0; 32]
            || self.wrapped_navcoin_code_hash == [0; 32]
        {
            return Err(FastSwapCodecError::NonCanonical(
                "ethereum route verification policy",
            ));
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.validate()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFETHROUTEPOLICY")?;
        encoder.u64(self.authority_epoch);
        encoder.fixed(&self.committee_root.0)?;
        encoder.u32(self.minimum_confirmations);
        encoder.fixed(&self.handoff_controller_code_hash)?;
        encoder.fixed(&self.wrapped_navcoin_code_hash)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumFinalizedCheckpointV1 {
    pub schema_version: u32,
    pub pftl_domain: FastSwapChainDomainV1,
    pub route_id: String,
    pub route_config_digest: FastSwapOpaqueHashV1,
    pub ethereum_chain_id: u64,
    pub block_number: u64,
    pub block_hash: [u8; 32],
    pub receipts_root: [u8; 32],
    pub observed_head_number: u64,
    pub minimum_confirmations: u32,
    pub authority_epoch: u64,
    pub committee_root: FastSwapCommitteeRootV1,
    pub handoff_controller: [u8; 20],
    pub wrapped_navcoin_token: [u8; 20],
    pub handoff_controller_code_hash: [u8; 32],
    pub wrapped_navcoin_code_hash: [u8; 32],
}

impl EthereumFinalizedCheckpointV1 {
    pub fn validate(&self) -> Result<(), FastSwapCodecError> {
        if self.schema_version != ETHEREUM_CHECKPOINT_SCHEMA_V1 {
            return Err(FastSwapCodecError::InvalidSchema(self.schema_version));
        }
        if self.pftl_domain.chain_id.is_empty()
            || self.pftl_domain.chain_id.len() > FASTSWAP_MAX_STRING_BYTES
            || self.route_id.is_empty()
            || self.route_id.len() > FASTSWAP_MAX_STRING_BYTES
        {
            return Err(FastSwapCodecError::LengthExceeded(
                "ethereum checkpoint domain or route",
            ));
        }
        if self.pftl_domain.protocol_version == 0
            || self.ethereum_chain_id == 0
            || self.block_number == 0
            || self.observed_head_number == 0
            || self.minimum_confirmations == 0
            || self.authority_epoch == 0
            || self.route_config_digest == FastSwapOpaqueHashV1::ZERO
            || self.committee_root == FastSwapCommitteeRootV1::ZERO
            || self.block_hash == [0; 32]
            || self.receipts_root == [0; 32]
            || self.handoff_controller == [0; 20]
            || self.wrapped_navcoin_token == [0; 20]
            || self.handoff_controller_code_hash == [0; 32]
            || self.wrapped_navcoin_code_hash == [0; 32]
        {
            return Err(FastSwapCodecError::NonCanonical(
                "ethereum checkpoint zero field",
            ));
        }
        let required_head = self
            .block_number
            .checked_add(u64::from(self.minimum_confirmations))
            .ok_or(FastSwapCodecError::NonCanonical(
                "ethereum checkpoint confirmation overflow",
            ))?;
        if self.observed_head_number < required_head {
            return Err(FastSwapCodecError::NonCanonical(
                "ethereum checkpoint below finality depth",
            ));
        }
        Ok(())
    }

    pub fn signing_bytes(&self) -> Result<Vec<u8>, FastSwapCodecError> {
        self.validate()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFETHCHECKPOINT")?;
        encoder.u32(self.schema_version);
        encoder.string(&self.pftl_domain.chain_id)?;
        encoder.fixed(&self.pftl_domain.genesis_hash.0)?;
        encoder.u32(self.pftl_domain.protocol_version);
        encoder.string(&self.route_id)?;
        encoder.fixed(&self.route_config_digest.0)?;
        encoder.u64(self.ethereum_chain_id);
        encoder.u64(self.block_number);
        encoder.fixed(&self.block_hash)?;
        encoder.fixed(&self.receipts_root)?;
        encoder.u64(self.observed_head_number);
        encoder.u32(self.minimum_confirmations);
        encoder.u64(self.authority_epoch);
        encoder.fixed(&self.committee_root.0)?;
        encoder.fixed(&self.handoff_controller)?;
        encoder.fixed(&self.wrapped_navcoin_token)?;
        encoder.fixed(&self.handoff_controller_code_hash)?;
        encoder.fixed(&self.wrapped_navcoin_code_hash)?;
        Ok(encoder.finish())
    }

    pub fn digest(&self) -> Result<FastSwapOpaqueHashV1, FastSwapCodecError> {
        Ok(FastSwapOpaqueHashV1(hash48(
            b"postfiat.ethereum.finalized-checkpoint.v1",
            &self.signing_bytes()?,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumCheckpointVoteV1 {
    pub validator_id: String,
    pub signature: Vec<u8>,
}

impl EthereumCheckpointVoteV1 {
    pub fn signing_bytes(
        &self,
        checkpoint: &EthereumFinalizedCheckpointV1,
    ) -> Result<Vec<u8>, FastSwapCodecError> {
        if self.validator_id.is_empty() || self.validator_id.len() > FASTSWAP_MAX_STRING_BYTES {
            return Err(FastSwapCodecError::LengthExceeded(
                "ethereum checkpoint validator id",
            ));
        }
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFETHCHECKPOINTVOTE")?;
        encoder.bytes(&checkpoint.signing_bytes()?)?;
        encoder.string(&self.validator_id)?;
        Ok(encoder.finish())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumCheckpointCertificateV1 {
    pub checkpoint: EthereumFinalizedCheckpointV1,
    pub votes: Vec<EthereumCheckpointVoteV1>,
}

impl EthereumCheckpointCertificateV1 {
    pub fn validate_canonical_order(&self) -> Result<(), FastSwapCodecError> {
        self.checkpoint.validate()?;
        if self.votes.is_empty() || self.votes.len() > FASTSWAP_MAX_VALIDATORS {
            return Err(FastSwapCodecError::LengthExceeded(
                "ethereum checkpoint votes",
            ));
        }
        if !self
            .votes
            .windows(2)
            .all(|pair| pair[0].validator_id < pair[1].validator_id)
        {
            return Err(FastSwapCodecError::NonCanonical(
                "ethereum checkpoint vote order",
            ));
        }
        for vote in &self.votes {
            vote.signing_bytes(&self.checkpoint)?;
            if vote.signature.is_empty() || vote.signature.len() > 16 * 1024 {
                return Err(FastSwapCodecError::LengthExceeded(
                    "ethereum checkpoint signature",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumReceiptProofV1 {
    pub transaction_index: u64,
    pub receipt_rlp: Vec<u8>,
    pub proof_nodes_rlp: Vec<Vec<u8>>,
}

impl EthereumReceiptProofV1 {
    pub fn validate_bounds(&self) -> Result<(), FastSwapCodecError> {
        if self.receipt_rlp.is_empty()
            || self.receipt_rlp.len() > ETHEREUM_RECEIPT_PROOF_MAX_RECEIPT_BYTES
        {
            return Err(FastSwapCodecError::LengthExceeded(
                "ethereum receipt RLP",
            ));
        }
        if self.proof_nodes_rlp.is_empty()
            || self.proof_nodes_rlp.len() > ETHEREUM_RECEIPT_PROOF_MAX_NODES
        {
            return Err(FastSwapCodecError::LengthExceeded(
                "ethereum receipt proof nodes",
            ));
        }
        if self.proof_nodes_rlp.iter().any(|node| {
            node.is_empty() || node.len() > ETHEREUM_RECEIPT_PROOF_MAX_NODE_BYTES
        }) {
            return Err(FastSwapCodecError::LengthExceeded(
                "ethereum receipt proof node",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EthereumExternalEventProofV1 {
    pub checkpoint_certificate: EthereumCheckpointCertificateV1,
    pub receipt_proof: EthereumReceiptProofV1,
    pub log_index: u32,
}

impl EthereumExternalEventProofV1 {
    pub fn validate_bounds(&self) -> Result<(), FastSwapCodecError> {
        self.checkpoint_certificate.validate_canonical_order()?;
        self.receipt_proof.validate_bounds()
    }

    pub fn commitment(&self) -> Result<FastSwapOpaqueHashV1, FastSwapCodecError> {
        self.validate_bounds()?;
        let mut encoder = Encoder::new();
        encoder.fixed(b"PFETHEVENTPROOF")?;
        encoder.fixed(&self.checkpoint_certificate.checkpoint.digest()?.0)?;
        encoder.u16(len_u16(
            self.checkpoint_certificate.votes.len(),
            "ethereum checkpoint votes",
        )?);
        for vote in &self.checkpoint_certificate.votes {
            encoder.string(&vote.validator_id)?;
            encoder.bytes(&vote.signature)?;
        }
        encoder.u64(self.receipt_proof.transaction_index);
        encoder.bytes(&self.receipt_proof.receipt_rlp)?;
        encoder.u16(len_u16(
            self.receipt_proof.proof_nodes_rlp.len(),
            "ethereum receipt proof nodes",
        )?);
        for node in &self.receipt_proof.proof_nodes_rlp {
            encoder.bytes(node)?;
        }
        encoder.u32(self.log_index);
        Ok(FastSwapOpaqueHashV1(hash48(
            b"postfiat.ethereum.external-event-proof.v1",
            &encoder.finish(),
        )))
    }
}
