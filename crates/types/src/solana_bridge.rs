pub const SOLANA_USDC_ROUTE: &str = "solana-mainnet-usdc-v1";
pub const SOLANA_DEPOSIT_PROOF: &str = "sp1-solana-alpenglow-finality-v1";
pub const SOLANA_HANDOFF_PROOF: &str = "sp1-solana-alpenglow-handoff-v1";
pub const SOLANA_ACTIVATION_HEIGHT: &str = "solana_bridge_activation_height";

/// The anchor and epoch schedule are authenticated by the route amendment.
/// Agave runtime/src/bank.rs uses epoch_schedule.get_epoch(slot) to select
/// epoch stakes. Handoffs and deposits must use that same schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaRoutePins {
    pub genesis_hash: String,
    pub shred_version: u16,
    pub handoff_program_vkey: String,
    pub slots_per_epoch: u64,
    pub epoch_warmup: bool,
    pub recorder_program: String,
    pub anchor_slot: u64,
    pub anchor: SolanaLightClient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaLightClient {
    pub epoch: u64,
    pub validator_set_hash: String,
    // Canonical decimal supports the guest's full u128 in JSON and CBOR.
    pub total_stake: String,
    pub known_key_table_hash: String,
}

impl SolanaLightClient {
    pub fn validate(&self) -> Result<(), String> {
        solana_hash("validator_set_hash", &self.validator_set_hash)?;
        solana_hash("known_key_table_hash", &self.known_key_table_hash)?;
        let stake = self
            .total_stake
            .parse::<u128>()
            .map_err(|_| "invalid Solana stake")?;
        if stake == 0 || stake.to_string() != self.total_stake {
            return Err("invalid Solana stake".into());
        }
        Ok(())
    }
}

fn solana_hash(field: &str, value: &str) -> Result<(), String> {
    validate_lower_hex_len(field, value, 64)?;
    if value.bytes().all(|b| b == b'0') {
        return Err(format!("Solana {field} must be nonzero"));
    }
    Ok(())
}

impl SolanaRoutePins {
    pub fn slot_epoch(&self, slot: u64) -> Option<u64> {
        use solana_epoch_schedule::{EpochSchedule, MINIMUM_SLOTS_PER_EPOCH};
        if self.slots_per_epoch < MINIMUM_SLOTS_PER_EPOCH
            || self.slots_per_epoch.checked_next_power_of_two().is_none()
        {
            return None;
        }
        Some(
            EpochSchedule::custom(
                self.slots_per_epoch,
                self.slots_per_epoch,
                self.epoch_warmup,
            )
            .get_epoch(slot),
        )
    }

    pub fn validate(&self, profile: &VaultBridgeRouteProfileV1) -> Result<(), String> {
        if profile.route_id != SOLANA_USDC_ROUTE
            || profile.verifier_kind != SOLANA_DEPOSIT_PROOF
            || profile.source_chain_id != 0
            || !profile.vault_runtime_code_hash.is_empty()
            || !profile.token_runtime_code_hash.is_empty()
            || !profile.verifier_policy_hash.is_empty()
            || self.slot_epoch(self.anchor_slot) != Some(self.anchor.epoch)
        {
            return Err("invalid Solana route or epoch schedule".into());
        }
        self.anchor.validate()?;
        solana_hash("recorder_program", &self.recorder_program)?;
        solana_hash("genesis_hash", &self.genesis_hash)?;
        solana_hash("vault program", &profile.vault_address)?;
        solana_hash("USDC mint", &profile.token_address)?;
        for key in [&profile.verifier_program_vkey, &self.handoff_program_vkey] {
            solana_hash(
                "program verifying key",
                key.strip_prefix("0x").ok_or("missing SP1 vkey prefix")?,
            )?;
        }
        if profile.verifier_program_vkey == self.handoff_program_vkey {
            return Err("Solana handoff and deposit require distinct program keys".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaRouteState {
    pub profile: VaultBridgeRouteProfileV1,
    pub client: SolanaLightClient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaDepositEvidence {
    pub program_id: String,
    pub slot: u64,
    pub block_id: String,
    pub receipt_address: String,
    pub recipient: String,
    pub amount_atoms: u64,
    pub nonce: String,
    pub deposit_id: String,
    pub depositor: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolanaDepositPublicValues {
    pub validator_set_hash: String,
    pub total_stake: u128,
    pub shred_version: u16,
    pub route_binding: String,
    pub receipts: Vec<SolanaDepositEvidence>,
}

impl SolanaDepositPublicValues {
    /// programs/pfusdc-sol-ingress/src/lib.rs, DepositPublicValues::encode,
    /// BlockPublicValues::encode and StatePublicValues::encode in order.
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        const HEADER: usize = 235;
        const RECEIPT: usize = 156;
        let (header, body) = bytes
            .split_at_checked(HEADER)
            .ok_or("truncated Solana deposit")?;
        if &header[..8] != b"PFSOLDP1" || header[56] > 1 {
            return Err("invalid Solana deposit domain or finality flag".into());
        }
        let changed = u32::from_le_bytes(header[227..231].try_into().unwrap());
        let count = u32::from_le_bytes(header[231..235].try_into().unwrap());
        if count == 0
            || count > changed
            || body.len() % RECEIPT != 0
            || body.len() / RECEIPT != count as usize
        {
            return Err("invalid Solana deposit receipt count".into());
        }
        let slot = u64::from_le_bytes(header[57..65].try_into().unwrap());
        let block_id = bytes_to_lower_hex(&header[65..97]);
        let program_id = bytes_to_lower_hex(&header[163..195]);
        let receipts = body
            .chunks_exact(RECEIPT)
            .map(|row| {
                let amount_atoms = u64::from_le_bytes(row[84..92].try_into().unwrap());
                if amount_atoms == 0 {
                    return Err("zero Solana deposit".to_string());
                }
                Ok(SolanaDepositEvidence {
                    program_id: program_id.clone(),
                    slot,
                    block_id: block_id.clone(),
                    deposit_id: bytes_to_lower_hex(&row[..32]),
                    receipt_address: bytes_to_lower_hex(&row[32..64]),
                    recipient: format!("pf{}", bytes_to_lower_hex(&row[64..84])),
                    amount_atoms,
                    nonce: bytes_to_lower_hex(&row[92..124]),
                    depositor: bytes_to_lower_hex(&row[124..]),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            validator_set_hash: bytes_to_lower_hex(&header[8..40]),
            total_stake: u128::from_le_bytes(header[40..56].try_into().unwrap()),
            shred_version: u16::from_le_bytes(header[97..99].try_into().unwrap()),
            route_binding: bytes_to_lower_hex(&header[195..227]),
            receipts,
        })
    }
}

impl SolanaDepositEvidence {
    pub fn validate(&self) -> Result<(), String> {
        solana_hash("deposit program", &self.program_id)?;
        validate_postfiat_address("Solana deposit recipient", &self.recipient)?;
        for (field, value) in [
            ("block ID", &self.block_id),
            ("receipt address", &self.receipt_address),
            ("nonce", &self.nonce),
            ("deposit ID", &self.deposit_id),
            ("depositor", &self.depositor),
        ] {
            validate_lower_hex_len(field, value, 64)?;
        }
        if self.amount_atoms == 0 {
            return Err("empty Solana deposit".into());
        }
        Ok(())
    }

    fn finality_ref(&self) -> String {
        format!(
            "solana_slot:{}:{}:{}",
            self.slot, self.block_id, self.receipt_address
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaProofOperation {
    pub submitter: String,
    pub profile_hash: String,
    pub proof_kind: String,
    pub proof: Vec<u8>,
    pub public_values: Vec<u8>,
}

impl SolanaProofOperation {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("solana_proof.submitter", &self.submitter)?;
        validate_lower_hex_len("solana_proof.profile_hash", &self.profile_hash, 96)?;
        if !matches!(
            self.proof_kind.as_str(),
            SOLANA_DEPOSIT_PROOF | SOLANA_HANDOFF_PROOF
        ) || self.proof.is_empty()
            || self.proof.len() > DEFAULT_MAX_NAV_SP1_PROOF_BYTES as usize
            || self.public_values.is_empty()
            || self.public_values.len() > DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES as usize
        {
            return Err("unsupported or out-of-bounds Solana proof".into());
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("SolanaProofOperation contains JSON primitives")
    }
}

// HandoffPublicValues::encode and RecorderContext::encode in the recorder
// guest (programs/pfusdc-sol-ingress/src/handoff.rs at 6c1caee1d).
impl SolanaRouteState {
    pub fn handoff_successor(&self, journal: &[u8]) -> Result<SolanaLightClient, String> {
        if journal.len() != 171 || &journal[..8] != b"PFSOLHP1" {
            return Err("invalid Solana handoff journal domain or length".into());
        }
        let pins = self.profile.solana.as_ref().ok_or("missing Solana pins")?;
        let slot = u64::from_le_bytes(journal[40..48].try_into().unwrap());
        let epoch = u64::from_le_bytes(journal[59..67].try_into().unwrap());
        if bytes_to_lower_hex(&journal[8..40]) != pins.recorder_program
            || u16::from_le_bytes(journal[48..50].try_into().unwrap()) != pins.shred_version
            || u64::from_le_bytes(journal[50..58].try_into().unwrap()) != pins.slots_per_epoch
            || journal[58] != u8::from(pins.epoch_warmup)
            || slot < pins.anchor_slot
            || pins.slot_epoch(slot) != Some(epoch)
            || self.client.epoch.checked_add(1) != Some(epoch)
            || bytes_to_lower_hex(&journal[67..99]) != self.client.validator_set_hash
            || bytes_to_lower_hex(&journal[139..171]) != self.client.known_key_table_hash
        {
            return Err(
                "Solana handoff differs from pinned recorder, epoch, set or known keys".into(),
            );
        }
        // The trusted set hash commits to its u128 denominator. The recorder
        // journal authenticates its table as an input, it does not replace it.
        let next = SolanaLightClient {
            epoch,
            validator_set_hash: bytes_to_lower_hex(&journal[99..131]),
            total_stake: u64::from_le_bytes(journal[131..139].try_into().unwrap()).to_string(),
            known_key_table_hash: self.client.known_key_table_hash.clone(),
        };
        next.validate()?;
        Ok(next)
    }
}
