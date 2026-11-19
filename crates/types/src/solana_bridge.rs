pub const SOLANA_USDC_ROUTE: &str = "solana-mainnet-usdc-v1";
pub const SOLANA_DEPOSIT_PROOF: &str = "sp1-solana-alpenglow-finality-v1";
pub const SOLANA_HANDOFF_PROOF: &str = "sp1-solana-alpenglow-handoff-v1";
pub const SOLANA_ACTIVATION_HEIGHT: &str = "solana_bridge_activation_height";

pub fn solana_source_series_id(chain_id: &str, asset: &str, bucket: &str) -> String {
    hash_hex_domain("postfiat.pfusdc.solana.source_series.v1",
        format!("{chain_id}\n{asset}\n{bucket}").as_bytes())
}

/// The anchor and epoch schedule are authenticated by the route amendment.
/// Agave runtime/src/bank.rs uses epoch_schedule.get_epoch(slot) to select
/// epoch stakes. Handoffs and deposits must use that same schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaRoutePins {
    pub genesis_hash: String,
    pub route_binding: String,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<SolanaRankedHandoff>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SolanaRankedHandoff {
    pub predecessor_set_hash: String,
    pub predecessor_key_table_hash: String,
    pub listed_stake: u64,
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
        if let Some(handoff) = &self.handoff {
            solana_hash("predecessor_set_hash", &handoff.predecessor_set_hash)?;
            solana_hash(
                "predecessor_key_table_hash",
                &handoff.predecessor_key_table_hash,
            )?;
            if handoff.listed_stake == 0
                || u128::from(handoff.listed_stake) > stake
                || handoff.predecessor_key_table_hash != self.known_key_table_hash
            {
                return Err("invalid Solana ranked handoff".into());
            }
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

/// Binding written by the Solana vault at initialization and into each receipt.
/// Chain and route IDs are UTF-8. Genesis hashes and program ID are raw bytes.
pub fn solana_deposit_route_binding(
    pftl_chain_id: &str,
    pftl_genesis_hash: &str,
    route_id: &str,
    solana_genesis_hash: &str,
    vault_program_id: &str,
) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    hash.update(b"postfiat.pfusdc.solana.route_binding.v1");
    hash.update(pftl_chain_id.as_bytes());
    hash.update(decode_lower_hex_exact(
        "Post Fiat genesis hash",
        pftl_genesis_hash,
        48,
    )?);
    hash.update(route_id.as_bytes());
    hash.update(decode_lower_hex_exact(
        "Solana genesis hash",
        solana_genesis_hash,
        32,
    )?);
    hash.update(decode_lower_hex_exact(
        "Solana vault program",
        vault_program_id,
        32,
    )?);
    Ok(bytes_to_lower_hex(&hash.finalize()))
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
        if self.anchor.handoff.is_some()
            || !matches!(
                profile.route_id.as_str(),
                SOLANA_USDC_ROUTE | "solana-devnet-usdc-v1" | "solana-local-usdc-v1"
            )
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
        solana_hash("route_binding", &self.route_binding)?;
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

// The governed verifying key authenticates the entire journal before this transition.
impl SolanaRouteState {
    pub fn handoff_successor(&self, journal: &[u8]) -> Result<SolanaLightClient, String> {
        let pins = self.profile.solana.as_ref().ok_or("missing Solana pins")?;
        let listed_stake = match journal.len() {
            179 if &journal[..8] == b"PFSOLHP2" => {
                Some(u64::from_le_bytes(journal[139..147].try_into().unwrap()))
            }
            171 if &journal[..8] == b"PFSOLHP1"
                && self.client.handoff.is_none()
                && pins.handoff_program_vkey != SOLANA_RELEASE_HANDOFF_VKEY =>
            {
                None
            }
            _ => return Err("invalid Solana handoff journal domain or length".into()),
        };
        let slot = u64::from_le_bytes(journal[40..48].try_into().unwrap());
        let epoch = u64::from_le_bytes(journal[59..67].try_into().unwrap());
        let total = u64::from_le_bytes(journal[131..139].try_into().unwrap());
        let set_hash = bytes_to_lower_hex(&journal[67..99]);
        let key_hash = bytes_to_lower_hex(&journal[journal.len() - 32..]);
        if bytes_to_lower_hex(&journal[8..40]) != pins.recorder_program
            || u16::from_le_bytes(journal[48..50].try_into().unwrap()) != pins.shred_version
            || u64::from_le_bytes(journal[50..58].try_into().unwrap()) != pins.slots_per_epoch
            || journal[58] != u8::from(pins.epoch_warmup)
            || slot < pins.anchor_slot
            || pins.slot_epoch(slot) != Some(epoch)
        {
            return Err("Solana handoff differs from its recorder or epoch schedule".into());
        }
        if epoch == self.client.epoch {
            let prior = self
                .client
                .handoff
                .as_ref()
                .ok_or("no predecessor for Solana replacement")?;
            if listed_stake.is_none_or(|stake| stake <= prior.listed_stake)
                || self.client.total_stake != total.to_string()
                || set_hash != prior.predecessor_set_hash
                || key_hash != prior.predecessor_key_table_hash
            {
                return Err(
                    "Solana replacement must extend the same predecessor with more stake".into(),
                );
            }
        } else if self.client.epoch.checked_add(1) != Some(epoch)
            || set_hash != self.client.validator_set_hash
            || key_hash != self.client.known_key_table_hash
        {
            return Err("Solana handoff differs from the expected predecessor".into());
        }
        let next = SolanaLightClient {
            epoch,
            validator_set_hash: bytes_to_lower_hex(&journal[99..131]),
            total_stake: total.to_string(),
            known_key_table_hash: key_hash.clone(),
            handoff: listed_stake.map(|listed_stake| SolanaRankedHandoff {
                predecessor_set_hash: set_hash,
                predecessor_key_table_hash: key_hash,
                listed_stake,
            }),
        };
        next.validate()?;
        Ok(next)
    }
}

// programs/pfusdc-sol-ingress/program-info.json, reproduced release handoff guest.
pub const SOLANA_RELEASE_HANDOFF_VKEY: &str =
    "0x0030b8018a18de3eec41e3b5afb5a282b1796055ba680bcc95ab216bd403f584";
