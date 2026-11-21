// Private payment action for the asset pool: two hidden notes in, two hidden notes out, all of
// one hidden asset, with an optional public exit amount. Included into `asset_orchard.rs`.
//
// What differs from the swap action:
// - the circuit proves `in0 + in1 = out0 + out1 + exit_amount` on hidden values, so a payment
//   can make change, merge and split;
// - no asset tag, price or per-note accounting record is published;
// - a note may have value zero, so one shape covers every payment;
// - outputs are addressed to a shielded address, not built from the recipient's seed.

pub const ASSET_ORCHARD_TRANSFER_CIRCUIT_ID_V1: &str = "asset_orchard.transfer.v1";
pub const ASSET_ORCHARD_TRANSFER_ACTION_SCHEMA_V1: &str =
    "postfiat-asset-orchard-transfer-action-v1";
pub const ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN: usize = 23;
pub const ASSET_ORCHARD_TRANSFER_H_ACTION_FIELD_COUNT: usize = 26;
pub const ASSET_ORCHARD_TRANSFER_H_ACTION_POSEIDON_INPUT_COUNT: usize = 28;
pub const ASSET_ORCHARD_SHIELDED_ADDRESS_PREFIX: &str = "pfs1";
const ASSET_ORCHARD_SHIELDED_ADDRESS_BODY_BYTES: usize =
    ASSET_ORCHARD_DIVERSIFIER_BYTES + 2 * ASSET_ORCHARD_POINT_BYTES;
const ASSET_ORCHARD_SHIELDED_ADDRESS_CHECKSUM_BYTES: usize = 4;
const TRANSFER_H_ACTION_HASH_NAME: &str = "postfiat.asset_orchard.transfer.h_action.v1";
const TRANSFER_H_SIG_DOMAIN: &[u8] = b"postfiat.asset_orchard.transfer.sighash.v1";
const TRANSFER_EXIT_BINDING_HASH_NAME: &str = "postfiat.asset_orchard.transfer.exit_binding.v1";
const SHIELDED_ADDRESS_CHECKSUM_DOMAIN: &[u8] = b"postfiat.asset_orchard.shielded_address.v1";

/// Where a note can be sent. Knowing an address lets anyone pay it; spending needs the keys it
/// was derived from.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AssetOrchardShieldedAddress {
    pub diversifier: [u8; ASSET_ORCHARD_DIVERSIFIER_BYTES],
    pub g_d: pallas::Affine,
    pub pk_d: pallas::Affine,
}

impl AssetOrchardShieldedAddress {
    pub fn encode(&self) -> Result<String, AssetOrchardError> {
        reject_identity_point("g_d", self.g_d)?;
        reject_identity_point("pk_d", self.pk_d)?;
        let mut body = Vec::with_capacity(
            ASSET_ORCHARD_SHIELDED_ADDRESS_BODY_BYTES + ASSET_ORCHARD_SHIELDED_ADDRESS_CHECKSUM_BYTES,
        );
        body.extend_from_slice(&self.diversifier);
        body.extend_from_slice(&self.g_d.to_bytes());
        body.extend_from_slice(&self.pk_d.to_bytes());
        let checksum = shielded_address_checksum(&body);
        body.extend_from_slice(&checksum);
        Ok(format!(
            "{ASSET_ORCHARD_SHIELDED_ADDRESS_PREFIX}{}",
            bytes_to_hex(&body)
        ))
    }

    pub fn parse(text: &str) -> Result<Self, AssetOrchardError> {
        let invalid = |message: &str| {
            AssetOrchardError::new("invalid_asset_orchard_shielded_address", message.to_string())
        };
        let hex = text
            .strip_prefix(ASSET_ORCHARD_SHIELDED_ADDRESS_PREFIX)
            .ok_or_else(|| invalid("a shielded address starts with pfs1"))?;
        if hex.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return Err(invalid("a shielded address is lower case"));
        }
        let bytes = hex_to_bytes(hex).map_err(|_| invalid("a shielded address is hexadecimal"))?;
        if bytes.len()
            != ASSET_ORCHARD_SHIELDED_ADDRESS_BODY_BYTES
                + ASSET_ORCHARD_SHIELDED_ADDRESS_CHECKSUM_BYTES
        {
            return Err(invalid("a shielded address has the wrong length"));
        }
        let (body, checksum) = bytes.split_at(ASSET_ORCHARD_SHIELDED_ADDRESS_BODY_BYTES);
        if shielded_address_checksum(body) != checksum {
            return Err(invalid("the shielded address checksum does not match"));
        }
        let mut diversifier = [0u8; ASSET_ORCHARD_DIVERSIFIER_BYTES];
        diversifier.copy_from_slice(&body[..ASSET_ORCHARD_DIVERSIFIER_BYTES]);
        let point = |offset: usize| -> Result<pallas::Affine, AssetOrchardError> {
            let mut encoded = [0u8; ASSET_ORCHARD_POINT_BYTES];
            encoded.copy_from_slice(&body[offset..offset + ASSET_ORCHARD_POINT_BYTES]);
            let point = Option::<pallas::Affine>::from(pallas::Affine::from_bytes(&encoded))
                .ok_or_else(|| invalid("the shielded address holds an invalid curve point"))?;
            reject_identity_point("shielded address point", point)?;
            Ok(point)
        };
        Ok(Self {
            diversifier,
            g_d: point(ASSET_ORCHARD_DIVERSIFIER_BYTES)?,
            pk_d: point(ASSET_ORCHARD_DIVERSIFIER_BYTES + ASSET_ORCHARD_POINT_BYTES)?,
        })
    }
}

fn shielded_address_checksum(body: &[u8]) -> [u8; ASSET_ORCHARD_SHIELDED_ADDRESS_CHECKSUM_BYTES] {
    let mut hasher = Sha3_256::new();
    Digest::update(&mut hasher, SHIELDED_ADDRESS_CHECKSUM_DOMAIN);
    Digest::update(&mut hasher, body);
    let digest = hasher.finalize();
    let mut checksum = [0u8; ASSET_ORCHARD_SHIELDED_ADDRESS_CHECKSUM_BYTES];
    checksum.copy_from_slice(&digest[..ASSET_ORCHARD_SHIELDED_ADDRESS_CHECKSUM_BYTES]);
    checksum
}

/// Everything a wallet derives from one shielded seed. The derivation is the one wallet notes
/// have always used, so a seed that owns existing notes owns this address too.
#[derive(Clone)]
pub struct AssetOrchardShieldedKeys {
    pub address: AssetOrchardShieldedAddress,
    pub(crate) nk: pallas::Base,
    pub(crate) rivk: pallas::Scalar,
    pub(crate) spend_scalar: pallas::Scalar,
    pub(crate) ivk: pallas::Scalar,
}

impl AssetOrchardShieldedKeys {
    pub fn from_seed(seed_hex: &str) -> Result<Self, AssetOrchardError> {
        let seed = fixed_lower_hex_array::<32>("asset_orchard_wallet_note_seed", seed_hex)?;
        let mut diversifier = [0u8; ASSET_ORCHARD_DIVERSIFIER_BYTES];
        diversifier.copy_from_slice(
            &derive_bytes("postfiat.asset_orchard.wallet_note.diversifier.v1", &seed)?
                [..ASSET_ORCHARD_DIVERSIFIER_BYTES],
        );
        let nk = hash_to_pallas_base("postfiat.asset_orchard.wallet_note.nk.v1", &seed)?;
        let rivk =
            hash_to_pallas_scalar_nonzero("postfiat.asset_orchard.wallet_note.rivk.v1", &seed)?;
        let spend_scalar = hash_to_pallas_scalar_nonzero(
            "postfiat.asset_orchard.wallet_note.spend_auth_sk.v1",
            &seed,
        )?;
        let spend_key =
            SigningKey::<SpendAuth>::try_from(scalar_enc(spend_scalar)).map_err(|_| {
                AssetOrchardError::new(
                    "invalid_asset_orchard_spend_key",
                    "derived AssetOrchard spend key is not accepted by RedPallas",
                )
            })?;
        let ak = verification_key_affine(&VerificationKey::from(&spend_key))?;
        let g_d = pallas::Point::hash_to_curve("postfiat.asset_orchard.wallet_note.g_d.v1")(&seed)
            .to_affine();
        let ivk_base = orchard_commit_ivk(ak, nk, rivk)?;
        let ivk = Option::<pallas::Scalar>::from(pallas::Scalar::from_repr(ivk_base.to_repr()))
            .ok_or_else(|| {
                AssetOrchardError::new(
                    "invalid_asset_orchard_ivk_scalar",
                    "derived Orchard ivk cannot be represented as a Pallas scalar",
                )
            })?;
        let pk_d = (pallas::Point::from(g_d) * ivk).to_affine();
        Ok(Self {
            address: AssetOrchardShieldedAddress {
                diversifier,
                g_d,
                pk_d,
            },
            nk,
            rivk,
            spend_scalar,
            ivk,
        })
    }

    pub(crate) fn incoming_viewing_key(&self) -> pallas::Scalar {
        self.ivk
    }

    /// A note this wallet can spend: the opening as received, plus this wallet's keys.
    pub(crate) fn wallet_note(
        &self,
        pool_domain: pallas::Base,
        asset_id: &str,
        note: &AssetNoteOpening,
        rseed: &[u8; ASSET_ORCHARD_RSEED_BYTES],
    ) -> Result<AssetOrchardWalletNote, AssetOrchardError> {
        Ok(AssetOrchardWalletNote {
            schema: "postfiat-asset-orchard-wallet-note-v1".to_string(),
            pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
            pool_domain: AssetOrchardFieldElement::from_field(pool_domain),
            asset_id: asset_id.to_string(),
            value: note.value,
            output_commitment: AssetOrchardFieldElement::from_field(note.cmx(pool_domain)?),
            note: AssetOrchardPublicNoteOpening::from_note(note)?,
            nk: AssetOrchardSecret::new(AssetOrchardFieldElement::from_field(self.nk)),
            rivk: AssetOrchardSecret::new(bytes_to_hex(&scalar_enc(self.rivk))),
            spend_auth_signing_key: AssetOrchardSecret::new(bytes_to_hex(&scalar_enc(
                self.spend_scalar,
            ))),
            rseed: AssetOrchardSecret::new(bytes_to_hex(rseed)),
        })
    }
}

/// A note addressed to someone, as its sender knows it. `value` may be zero.
pub fn asset_orchard_note_for_address(
    address: &AssetOrchardShieldedAddress,
    asset_tag: AssetTag,
    value: u64,
    rho: pallas::Base,
    rseed: &[u8; ASSET_ORCHARD_RSEED_BYTES],
) -> Result<AssetNoteOpening, AssetOrchardError> {
    let note = AssetNoteOpening {
        diversifier: address.diversifier,
        g_d: address.g_d,
        pk_d: address.pk_d,
        asset_tag,
        value,
        rho,
        psi: orchard_psi(rseed, rho)?,
        rcm: orchard_rcm(rseed, rho)?,
    };
    note.validate_allowing_zero_value()?;
    Ok(note)
}

impl AssetNoteOpening {
    /// The transfer circuit accepts zero-value notes; every older action rejects them.
    pub fn validate_allowing_zero_value(&self) -> Result<(), AssetOrchardError> {
        self.asset_tag.validate()?;
        reject_identity_point("g_d", self.g_d)?;
        reject_identity_point("pk_d", self.pk_d)?;
        if bool::from(self.rcm.is_zero()) {
            return Err(AssetOrchardError::new(
                "zero_rcm",
                "asset-orchard note rcm must be nonzero",
            ));
        }
        Ok(())
    }

    pub fn cmx_allowing_zero_value(
        &self,
        pool_domain: pallas::Base,
    ) -> Result<pallas::Base, AssetOrchardError> {
        self.validate_allowing_zero_value()?;
        let mut bits = Vec::with_capacity(ASSET_ORCHARD_NOTE_MESSAGE_BITS);
        bits.extend(i2lebsp_from_field(pool_domain, 255));
        bits.extend(i2lebsp_from_u128(self.asset_tag.lo, 128));
        bits.extend(i2lebsp_from_u128(self.asset_tag.hi, 128));
        bits.extend(point_bits(self.g_d)?);
        bits.extend(point_bits(self.pk_d)?);
        bits.extend(i2lebsp_from_u64(self.value, 64));
        bits.extend(i2lebsp_from_field(self.rho, 255));
        bits.extend(i2lebsp_from_field(self.psi, 255));
        let domain = sinsemilla::CommitDomain::new(ASSET_ORCHARD_NOTE_COMMIT_DOMAIN_V1);
        let point = Option::<pallas::Point>::from(domain.commit(bits.into_iter(), &self.rcm))
            .ok_or_else(|| {
                AssetOrchardError::new(
                    "asset_note_commit_failed",
                    "asset note commitment produced an invalid Sinsemilla point",
                )
            })?;
        extract_p(point)
    }
}

/// The public side of a transfer that leaves the pool. Absent for a payment inside the pool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardTransferExit {
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
}

impl AssetOrchardTransferExit {
    pub fn validate(&self) -> Result<(), AssetOrchardError> {
        validate_canonical_text("asset_orchard_transfer_exit_to", &self.to, 256)?;
        validate_canonical_text(
            "asset_orchard_transfer_exit_asset_id",
            &self.asset_id,
            ASSET_ORCHARD_MAX_ASSET_ID_BYTES,
        )?;
        if self.amount == 0 {
            return Err(AssetOrchardError::new(
                "zero_asset_orchard_transfer_exit_amount",
                "a transfer exit amount must be nonzero; omit the exit for a payment inside the pool",
            ));
        }
        AssetTag::derive(&self.asset_id)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardTransferAction {
    pub version: u16,
    pub schema: String,
    pub pool_id: String,
    pub proof_system_id: String,
    pub circuit_id: String,
    pub pool_domain: AssetOrchardFieldElement,
    pub anchor: AssetOrchardFieldElement,
    pub nullifiers: Vec<AssetOrchardFieldElement>,
    pub randomized_verification_keys: Vec<AssetOrchardPoint>,
    pub output_commitments: Vec<AssetOrchardFieldElement>,
    pub encrypted_outputs: Vec<AssetOrchardBoundedBytes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<AssetOrchardTransferExit>,
    pub exit_binding_hash: AssetOrchardSwapBindingHash,
    pub action_binding_hash: AssetOrchardSwapBindingHash,
    pub proof: AssetOrchardProofBytes,
    pub spend_authorization_signatures: Vec<AssetOrchardSpendAuthSignature>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct AssetOrchardTransferPublicFields {
    pub pool_domain: pallas::Base,
    pub anchor: pallas::Base,
    pub nullifiers: [pallas::Base; 2],
    pub randomized_verification_keys: [RandomizedVerificationKeyFields; 2],
    pub output_commitments: [pallas::Base; 2],
    pub encrypted_output_hashes: [EncryptedOutputHash; 2],
    /// `(0, 0)` when nothing leaves the pool.
    pub exit_asset_tag_lo: u128,
    pub exit_asset_tag_hi: u128,
    pub exit_amount: u64,
    pub exit_binding_hash: [u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES],
}

impl AssetOrchardTransferPublicFields {
    fn rows_without_context(&self) -> Result<[pallas::Base; 21], AssetOrchardError> {
        if self.exit_amount == 0
            && (self.exit_asset_tag_lo != 0
                || self.exit_asset_tag_hi != 0
                || self.exit_binding_hash != [0u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES])
        {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_exit_fields_without_exit",
                "a transfer with no exit amount must leave the exit tag and binding zero",
            ));
        }
        if self.exit_amount != 0 {
            AssetTag {
                lo: self.exit_asset_tag_lo,
                hi: self.exit_asset_tag_hi,
            }
            .validate()?;
        }
        let eo0 = self.encrypted_output_hashes[0].as_fields();
        let eo1 = self.encrypted_output_hashes[1].as_fields();
        let exit_binding = binding_hash_fields(&self.exit_binding_hash)?;
        Ok([
            self.pool_domain,
            self.anchor,
            self.nullifiers[0],
            self.nullifiers[1],
            self.randomized_verification_keys[0].x,
            self.randomized_verification_keys[0].y,
            self.randomized_verification_keys[1].x,
            self.randomized_verification_keys[1].y,
            self.output_commitments[0],
            self.output_commitments[1],
            eo0[0],
            eo0[1],
            eo0[2],
            eo1[0],
            eo1[1],
            eo1[2],
            pallas::Base::from_u128(self.exit_asset_tag_lo),
            pallas::Base::from_u128(self.exit_asset_tag_hi),
            pallas::Base::from(self.exit_amount),
            exit_binding[0],
            exit_binding[1],
        ])
    }

    pub fn public_instance(
        &self,
    ) -> Result<[pallas::Base; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN], AssetOrchardError> {
        let rows = self.rows_without_context()?;
        let action_context = transfer_h_action_from_rows(&rows)?;
        let mut instance = [pallas::Base::ZERO; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN];
        instance[..21].copy_from_slice(&rows);
        instance[21] = action_context[0];
        instance[22] = action_context[1];
        Ok(instance)
    }
}

fn transfer_h_action_fields_from_rows(
    rows: &[pallas::Base; 21],
) -> Result<[pallas::Base; ASSET_ORCHARD_TRANSFER_H_ACTION_FIELD_COUNT], AssetOrchardError> {
    let mut fields = [pallas::Base::ZERO; ASSET_ORCHARD_TRANSFER_H_ACTION_FIELD_COUNT];
    fields[0] = const_field(&format!("proof_system:{ASSET_ORCHARD_PROOF_SYSTEM_ID_V1}"))?;
    fields[1] = const_field(&format!("circuit:{ASSET_ORCHARD_TRANSFER_CIRCUIT_ID_V1}"))?;
    fields[2] = const_field(&format!("schema:{ASSET_ORCHARD_TRANSFER_ACTION_SCHEMA_V1}"))?;
    fields[3] = const_field(&format!("pool:{ASSET_ORCHARD_POOL_ID_V1}"))?;
    fields[4] = const_field(&format!("note_version:{ASSET_ORCHARD_NOTE_VERSION_V1}"))?;
    fields[5..].copy_from_slice(rows);
    Ok(fields)
}

fn transfer_h_action_from_rows(
    rows: &[pallas::Base; 21],
) -> Result<[pallas::Base; 2], AssetOrchardError> {
    poseidon_hash2(
        TRANSFER_H_ACTION_HASH_NAME,
        &transfer_h_action_fields_from_rows(rows)?,
    )
}

/// The sponge inputs the circuit recomputes from its first 21 public rows.
pub fn transfer_h_action_poseidon_inputs_from_public_instance(
    public_instance: &[pallas::Base; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN],
) -> Result<[pallas::Base; ASSET_ORCHARD_TRANSFER_H_ACTION_POSEIDON_INPUT_COUNT], AssetOrchardError>
{
    let mut rows = [pallas::Base::ZERO; 21];
    rows.copy_from_slice(&public_instance[..21]);
    let fields = transfer_h_action_fields_from_rows(&rows)?;
    let mut inputs = [pallas::Base::ZERO; ASSET_ORCHARD_TRANSFER_H_ACTION_POSEIDON_INPUT_COUNT];
    inputs[0] = const_field(TRANSFER_H_ACTION_HASH_NAME)?;
    inputs[1] = pallas::Base::from(ASSET_ORCHARD_TRANSFER_H_ACTION_FIELD_COUNT as u64);
    inputs[2..].copy_from_slice(&fields);
    Ok(inputs)
}

pub fn transfer_action_binding_hash(
    fields: &AssetOrchardTransferPublicFields,
) -> Result<[u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES], AssetOrchardError> {
    let action_context = transfer_h_action_from_rows(&fields.rows_without_context()?)?;
    let mut out = [0u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES];
    out[0..32].copy_from_slice(&field_enc(action_context[0]));
    out[32..64].copy_from_slice(&field_enc(action_context[1]));
    Ok(out)
}

/// Binds the public destination of an exit into the proof, so a relayer cannot redirect it.
pub fn asset_orchard_transfer_exit_binding_hash(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    pool_domain: pallas::Base,
    exit: &AssetOrchardTransferExit,
) -> Result<[u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES], AssetOrchardError> {
    validate_canonical_text("chain_id", chain_id, 256)?;
    if protocol_version == 0 {
        return Err(AssetOrchardError::new(
            "invalid_protocol_version",
            "protocol_version must be nonzero",
        ));
    }
    exit.validate()?;
    let fields = [
        const_field(&format!("circuit:{ASSET_ORCHARD_TRANSFER_CIRCUIT_ID_V1}"))?,
        const_field(&format!("schema:{ASSET_ORCHARD_TRANSFER_ACTION_SCHEMA_V1}"))?,
        text_to_field("transfer_exit.chain_id", chain_id)?,
        bytes_to_field("transfer_exit.genesis_hash", &genesis_hash)?,
        pallas::Base::from(u64::from(protocol_version)),
        pool_domain,
        text_to_field("transfer_exit.to", &exit.to)?,
        text_to_field("transfer_exit.asset_id", &exit.asset_id)?,
        pallas::Base::from(exit.amount),
    ];
    let hash = poseidon_hash2(TRANSFER_EXIT_BINDING_HASH_NAME, &fields)?;
    let mut out = [0u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES];
    out[0..32].copy_from_slice(&field_enc(hash[0]));
    out[32..64].copy_from_slice(&field_enc(hash[1]));
    Ok(out)
}

impl AssetOrchardTransferAction {
    pub fn validate(&self) -> Result<(), AssetOrchardError> {
        if self.version != ASSET_ORCHARD_ACTION_VERSION_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_transfer_version",
                format!("unsupported asset-orchard transfer version {}", self.version),
            ));
        }
        if self.schema != ASSET_ORCHARD_TRANSFER_ACTION_SCHEMA_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_transfer_schema",
                format!("unsupported asset-orchard transfer schema `{}`", self.schema),
            ));
        }
        if self.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_pool",
                format!("unsupported asset-orchard pool `{}`", self.pool_id),
            ));
        }
        if self.proof_system_id != ASSET_ORCHARD_PROOF_SYSTEM_ID_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_proof_system",
                format!(
                    "unsupported asset-orchard proof system `{}`",
                    self.proof_system_id
                ),
            ));
        }
        if self.circuit_id != ASSET_ORCHARD_TRANSFER_CIRCUIT_ID_V1 {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_transfer_circuit",
                format!("unsupported asset-orchard transfer circuit `{}`", self.circuit_id),
            ));
        }
        self.validate_index_counts()?;
        if self.proof.byte_len() > ASSET_ORCHARD_PROOF_MAX_BYTES {
            return Err(AssetOrchardError::new(
                "oversized_asset_orchard_proof",
                format!(
                    "asset-orchard proof has {} bytes, max {ASSET_ORCHARD_PROOF_MAX_BYTES}",
                    self.proof.byte_len()
                ),
            ));
        }
        if has_duplicate_hex(self.nullifiers.iter().map(AssetOrchardFieldElement::as_hex)) {
            return Err(AssetOrchardError::new(
                "duplicate_nullifier",
                "asset-orchard transfer contains duplicate nullifiers",
            ));
        }
        if has_duplicate_hex(
            self.output_commitments
                .iter()
                .map(AssetOrchardFieldElement::as_hex),
        ) {
            return Err(AssetOrchardError::new(
                "duplicate_output_commitment",
                "asset-orchard transfer contains duplicate output commitments",
            ));
        }
        // Every output is the same size, so its length says nothing about asset or amount.
        for output in &self.encrypted_outputs {
            if output.byte_len() != crate::ASSET_ORCHARD_TRANSFER_OUTPUT_CIPHERTEXT_BYTES {
                return Err(AssetOrchardError::new(
                    "asset_orchard_transfer_output_size",
                    format!(
                        "a transfer output has {} bytes, expected {}",
                        output.byte_len(),
                        crate::ASSET_ORCHARD_TRANSFER_OUTPUT_CIPHERTEXT_BYTES
                    ),
                ));
            }
        }
        match &self.exit {
            Some(exit) => exit.validate()?,
            None => {
                if self.exit_binding_hash.to_bytes()?
                    != [0u8; ASSET_ORCHARD_SWAP_BINDING_HASH_BYTES]
                {
                    return Err(AssetOrchardError::new(
                        "asset_orchard_transfer_exit_fields_without_exit",
                        "a transfer with no exit must carry a zero exit binding",
                    ));
                }
            }
        }
        let computed = AssetOrchardSwapBindingHash::from_bytes(&transfer_action_binding_hash(
            &self.public_fields_without_binding_check()?,
        )?);
        if computed != self.action_binding_hash {
            return Err(AssetOrchardError::new(
                "asset_orchard_transfer_binding_mismatch",
                "serialized action_binding_hash does not match the recomputed action hash",
            ));
        }
        Ok(())
    }

    pub fn public_fields(&self) -> Result<AssetOrchardTransferPublicFields, AssetOrchardError> {
        self.validate()?;
        self.public_fields_without_binding_check()
    }

    pub fn public_instance(
        &self,
    ) -> Result<[pallas::Base; ASSET_ORCHARD_TRANSFER_PUBLIC_INSTANCE_LEN], AssetOrchardError> {
        self.public_fields()?.public_instance()
    }

    pub fn validate_domain_binding(
        &self,
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
    ) -> Result<(), AssetOrchardError> {
        let expected =
            AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
        if self.pool_domain.to_field()? != expected {
            return Err(AssetOrchardError::new(
                "asset_orchard_pool_domain_mismatch",
                "serialized pool_domain does not match local chain/genesis/protocol/pool",
            ));
        }
        if let Some(exit) = &self.exit {
            let expected_binding = asset_orchard_transfer_exit_binding_hash(
                chain_id,
                genesis_hash,
                protocol_version,
                expected,
                exit,
            )?;
            if self.exit_binding_hash.to_bytes()? != expected_binding {
                return Err(AssetOrchardError::new(
                    "asset_orchard_transfer_exit_binding_mismatch",
                    "the exit binding does not match the public destination, asset and amount",
                ));
            }
        }
        Ok(())
    }

    pub fn sighash(
        &self,
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
    ) -> Result<[u8; ASSET_ORCHARD_SIGHASH_BYTES], AssetOrchardError> {
        self.validate()?;
        validate_canonical_text("chain_id", chain_id, 256)?;
        if protocol_version == 0 {
            return Err(AssetOrchardError::new(
                "invalid_protocol_version",
                "protocol_version must be nonzero",
            ));
        }
        let mut payload = Vec::new();
        payload.extend_from_slice(TRANSFER_H_SIG_DOMAIN);
        payload.extend_from_slice(&1u16.to_le_bytes());
        append_len_bytes_vec(&mut payload, chain_id.as_bytes())?;
        payload.extend_from_slice(&genesis_hash);
        payload.extend_from_slice(&protocol_version.to_le_bytes());
        append_len_bytes_vec(&mut payload, self.pool_id.as_bytes())?;
        append_len_bytes_vec(&mut payload, self.proof_system_id.as_bytes())?;
        append_len_bytes_vec(&mut payload, self.circuit_id.as_bytes())?;
        append_len_bytes_vec(&mut payload, self.schema.as_bytes())?;
        payload.extend_from_slice(&field_enc(self.pool_domain.to_field()?));
        payload.extend_from_slice(&field_enc(self.anchor.to_field()?));
        for nullifier in &self.nullifiers {
            payload.extend_from_slice(&field_enc(nullifier.to_field()?));
        }
        for key in &self.randomized_verification_keys {
            payload.extend_from_slice(&point_enc(key.to_affine()?)?);
        }
        for commitment in &self.output_commitments {
            payload.extend_from_slice(&field_enc(commitment.to_field()?));
        }
        for output in &self.encrypted_outputs {
            append_len_bytes_vec(&mut payload, &output.to_bytes()?)?;
        }
        match &self.exit {
            Some(exit) => {
                payload.push(1);
                append_len_bytes_vec(&mut payload, exit.to.as_bytes())?;
                append_len_bytes_vec(&mut payload, exit.asset_id.as_bytes())?;
                payload.extend_from_slice(&exit.amount.to_le_bytes());
            }
            None => payload.push(0),
        }
        payload.extend_from_slice(&self.exit_binding_hash.to_bytes()?);
        payload.extend_from_slice(&self.action_binding_hash.to_bytes()?);
        let digest = Sha3_256::digest(&payload);
        digest.as_slice().try_into().map_err(|_| {
            AssetOrchardError::new(
                "digest_slice",
                "invalid asset-orchard transfer sighash digest length",
            )
        })
    }

    pub fn verify_spend_authorizations(
        &self,
        chain_id: &str,
        genesis_hash: [u8; 32],
        protocol_version: u32,
    ) -> Result<(), AssetOrchardError> {
        self.validate_domain_binding(chain_id, genesis_hash, protocol_version)?;
        let sighash = self.sighash(chain_id, genesis_hash, protocol_version)?;
        for index in 0..ASSET_ORCHARD_LEG_COUNT {
            let rk = self.randomized_verification_keys[index].to_spend_auth_verification_key()?;
            let signature = self.spend_authorization_signatures[index].to_orchard()?;
            rk.verify(&sighash, &signature).map_err(|_| {
                AssetOrchardError::new(
                    "asset_orchard_spend_authorization_failed",
                    format!("asset-orchard transfer spend authorization signature {index} failed"),
                )
            })?;
        }
        Ok(())
    }

    fn public_fields_without_binding_check(
        &self,
    ) -> Result<AssetOrchardTransferPublicFields, AssetOrchardError> {
        self.validate_index_counts()?;
        let encrypted_outputs = [
            self.encrypted_outputs[0].to_bytes()?,
            self.encrypted_outputs[1].to_bytes()?,
        ];
        let (exit_asset_tag_lo, exit_asset_tag_hi, exit_amount) = match &self.exit {
            Some(exit) => {
                exit.validate()?;
                let tag = AssetTag::derive(&exit.asset_id)?;
                (tag.lo, tag.hi, exit.amount)
            }
            None => (0, 0, 0),
        };
        Ok(AssetOrchardTransferPublicFields {
            pool_domain: self.pool_domain.to_field()?,
            anchor: self.anchor.to_field()?,
            nullifiers: [
                self.nullifiers[0].to_field()?,
                self.nullifiers[1].to_field()?,
            ],
            randomized_verification_keys: [
                self.randomized_verification_keys[0].fields()?,
                self.randomized_verification_keys[1].fields()?,
            ],
            output_commitments: [
                self.output_commitments[0].to_field()?,
                self.output_commitments[1].to_field()?,
            ],
            encrypted_output_hashes: [
                encrypted_output_hash(0, &encrypted_outputs[0])?,
                encrypted_output_hash(1, &encrypted_outputs[1])?,
            ],
            exit_asset_tag_lo,
            exit_asset_tag_hi,
            exit_amount,
            exit_binding_hash: self.exit_binding_hash.to_bytes()?,
        })
    }

    fn validate_index_counts(&self) -> Result<(), AssetOrchardError> {
        validate_count("nullifiers", self.nullifiers.len(), ASSET_ORCHARD_LEG_COUNT)?;
        validate_count(
            "randomized_verification_keys",
            self.randomized_verification_keys.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )?;
        validate_count(
            "output_commitments",
            self.output_commitments.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )?;
        validate_count(
            "encrypted_outputs",
            self.encrypted_outputs.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )?;
        validate_count(
            "spend_authorization_signatures",
            self.spend_authorization_signatures.len(),
            ASSET_ORCHARD_LEG_COUNT,
        )
    }
}
