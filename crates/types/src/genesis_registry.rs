// Canonical genesis-registry and template-trust-graph types.
//
// Implements the schema sketch in
// `docs/architecture/genesis-registry-proposal-path.md` §4 with the encoding
// rules of the locked testnet genesis specification §2.1: deterministic CBOR
// (RFC 8949 §4.2) with integer map labels, bytewise-lexicographic entry
// ordering, duplicate rejection, and fail-closed unknowns. Every digest is
// domain separated as `SHA-256(uint16_be(len(label)) || label || cbor)`.
//
// Integer map labels (all fields required, ascending order):
//
// ```text
// ProposedGenesisRegistryV1      GenesisRoundRefV1
//   1 version (=1, critical)       1 fork_network        (tstr)
//   2 chain_id          (tstr)     2 round_number        (uint)
//   3 genesis_round     (map)      3 bundle_cid          (tstr)
//   4 receipt_deadline  (map)      4 bundle_digest       (bstr32)
//   5 entries           (array)    5 manifest_digest     (bstr32)
//   6 template_trust_graph (map)   6 final_scores_digest (bstr32)
//                                  7 selected_unl_digest (bstr32)
// GenesisReceiptDeadlineRefV1      8 convergence_report_digest (bstr32)
//   1 fork_ledger_hash (bstr32)    9 anchor_tx_hash      (bstr32)
//   2 fork_ledger_seq  (uint)
//
// ProposedGenesisEntryV1         TemplateTrustGraphV1
//   1 fork_master_key  (bstr33)    1 n_s (uint)
//   2 final_score      (uint)      2 q_s (uint)
//   3 cutoff           (uint)      3 t_s (uint)
//   4 selection_index  (uint)
//   5 identity_evidence_digest (bstr32)
//   6 identity_receipt_digest  (bstr32)
//   7 mldsa_public_key (bstr1952)
//
// GenesisIdentityReceiptBodyV1   GenesisEvidenceRecordV1
//   1 version (=1, critical)       1 version (=1, critical)
//   2 fork_master_key  (bstr33)    2 fork_master_key (bstr33)
//   3 mldsa_public_key (bstr1952)  3 domain          (tstr, may be empty)
//   4 chain_id         (tstr)      4 domain_verified (uint 0/1)
//   5 genesis_round_id (tstr)      5 provider        (tstr, may be empty)
//   6 deadline_ledger_hash (bstr32)  6 country       (tstr, may be empty)
//   7 deadline_ledger_seq  (uint)
//   8 expiry_close_time    (uint)
// ```
//
// These objects are `SHADOW_ONLY` schema work: nothing here grants authority,
// mutates a registry, or contacts any network. Fork master keys are carried
// as the 33-byte key material decoded from the base58 `n…` node-public-key
// form: compressed SEC1 (`0x02`/`0x03`) for secp256k1 keys or the rippled
// `0xED`-prefixed encoding for ed25519 master keys, which is what the
// archived rehearsal rounds contain.

pub const PROPOSED_GENESIS_REGISTRY_VERSION_V1: u64 = 1;
pub const GENESIS_IDENTITY_RECEIPT_VERSION_V1: u64 = 1;
pub const GENESIS_EVIDENCE_RECORD_VERSION_V1: u64 = 1;

pub const PROPOSED_GENESIS_REGISTRY_DOMAIN_V1: &str = "L1V2_PROPOSED_GENESIS_REGISTRY_V1";
pub const GENESIS_IDENTITY_RECEIPT_DOMAIN_V1: &str = "L1V2_IDENTITY_RECEIPT_V1";
pub const GENESIS_EVIDENCE_RECORD_DOMAIN_V1: &str = "L1V2_GENESIS_EVIDENCE_RECORD_V1";

pub const GENESIS_FORK_MASTER_KEY_LEN: usize = 33;
pub const GENESIS_MLDSA65_PUBLIC_KEY_LEN: usize = 1952;
pub const GENESIS_DIGEST_LEN: usize = 32;
pub const GENESIS_REGISTRY_MAX_ENTRIES: usize = 4096;
pub const GENESIS_REGISTRY_MAX_TEXT_BYTES: usize = 256;
pub const GENESIS_REGISTRY_MAX_SELECTION_INDEX: u64 = 65_535;
pub const GENESIS_REGISTRY_MAX_ROUND_NUMBER: u64 = 1_000_000_000;
pub const GENESIS_REGISTRY_MAX_LEDGER_SEQ: u64 = 1 << 62;
pub const GENESIS_REGISTRY_MAX_CLOSE_TIME: u64 = 1 << 62;

/// Closed error set. `code()` values are the named-error contract shared with
/// the Python reference implementation and the mutation fixtures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenesisRegistryError {
    Truncated,
    TrailingBytes,
    WrongType,
    NonCanonicalEncoding,
    DuplicateField,
    UnknownField,
    MissingField,
    InvalidTextEncoding,
    UnknownVersion,
    InvalidChainId,
    InvalidForkNetwork,
    InvalidRoundNumber,
    InvalidBundleCid,
    InvalidDigestLength,
    InvalidLedgerSeq,
    InvalidMasterKey,
    InvalidMldsaKeyLength,
    ScoreOutOfRange,
    CutoffOutOfRange,
    ScoreBelowCutoff,
    CutoffMismatch,
    InvalidSelectionIndex,
    DuplicateSelectionIndex,
    DuplicateMasterKey,
    UnsortedEntries,
    EmptyEntries,
    TooManyEntries,
    TrustGraphMismatch,
    TrustGraphUnsafe,
    InvalidGenesisRoundId,
    InvalidExpiry,
    InvalidDomainFlag,
}

impl GenesisRegistryError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Truncated => "truncated",
            Self::TrailingBytes => "trailing_bytes",
            Self::WrongType => "wrong_type",
            Self::NonCanonicalEncoding => "non_canonical_encoding",
            Self::DuplicateField => "duplicate_field",
            Self::UnknownField => "unknown_field",
            Self::MissingField => "missing_field",
            Self::InvalidTextEncoding => "invalid_text_encoding",
            Self::UnknownVersion => "unknown_version",
            Self::InvalidChainId => "invalid_chain_id",
            Self::InvalidForkNetwork => "invalid_fork_network",
            Self::InvalidRoundNumber => "invalid_round_number",
            Self::InvalidBundleCid => "invalid_bundle_cid",
            Self::InvalidDigestLength => "invalid_digest_length",
            Self::InvalidLedgerSeq => "invalid_ledger_seq",
            Self::InvalidMasterKey => "invalid_master_key",
            Self::InvalidMldsaKeyLength => "invalid_mldsa_key_length",
            Self::ScoreOutOfRange => "score_out_of_range",
            Self::CutoffOutOfRange => "cutoff_out_of_range",
            Self::ScoreBelowCutoff => "score_below_cutoff",
            Self::CutoffMismatch => "cutoff_mismatch",
            Self::InvalidSelectionIndex => "invalid_selection_index",
            Self::DuplicateSelectionIndex => "duplicate_selection_index",
            Self::DuplicateMasterKey => "duplicate_master_key",
            Self::UnsortedEntries => "unsorted_entries",
            Self::EmptyEntries => "empty_entries",
            Self::TooManyEntries => "too_many_entries",
            Self::TrustGraphMismatch => "trust_graph_mismatch",
            Self::TrustGraphUnsafe => "trust_graph_unsafe",
            Self::InvalidGenesisRoundId => "invalid_genesis_round_id",
            Self::InvalidExpiry => "invalid_expiry",
            Self::InvalidDomainFlag => "invalid_domain_flag",
        }
    }
}

impl std::fmt::Display for GenesisRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for GenesisRegistryError {}

fn genesis_sha256(data: &[u8]) -> [u8; 32] {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// `SHA-256(uint16_be(len(label)) || ASCII(label) || canonical_cbor)`.
pub fn genesis_domain_digest(label: &str, canonical_cbor: &[u8]) -> [u8; 32] {
    let label_bytes = label.as_bytes();
    debug_assert!(label_bytes.len() <= u16::MAX as usize);
    let mut preimage = Vec::with_capacity(2 + label_bytes.len() + canonical_cbor.len());
    preimage.extend_from_slice(&(label_bytes.len() as u16).to_be_bytes());
    preimage.extend_from_slice(label_bytes);
    preimage.extend_from_slice(canonical_cbor);
    genesis_sha256(&preimage)
}

// ---------------------------------------------------------------------------
// Deterministic CBOR subset (RFC 8949 §4.2, definite lengths only)
// ---------------------------------------------------------------------------

const GENESIS_CBOR_MAJOR_UINT: u8 = 0;
const GENESIS_CBOR_MAJOR_BYTES: u8 = 2;
const GENESIS_CBOR_MAJOR_TEXT: u8 = 3;
const GENESIS_CBOR_MAJOR_ARRAY: u8 = 4;
const GENESIS_CBOR_MAJOR_MAP: u8 = 5;

#[derive(Debug, Default)]
pub struct GenesisCborWriter {
    buf: Vec<u8>,
}

impl GenesisCborWriter {
    pub fn new() -> Self {
        Self::default()
    }

    fn head(&mut self, major: u8, arg: u64) {
        let m = major << 5;
        if arg < 24 {
            self.buf.push(m | arg as u8);
        } else if arg <= u8::MAX as u64 {
            self.buf.push(m | 24);
            self.buf.push(arg as u8);
        } else if arg <= u16::MAX as u64 {
            self.buf.push(m | 25);
            self.buf.extend_from_slice(&(arg as u16).to_be_bytes());
        } else if arg <= u32::MAX as u64 {
            self.buf.push(m | 26);
            self.buf.extend_from_slice(&(arg as u32).to_be_bytes());
        } else {
            self.buf.push(m | 27);
            self.buf.extend_from_slice(&arg.to_be_bytes());
        }
    }

    pub fn uint(&mut self, value: u64) {
        self.head(GENESIS_CBOR_MAJOR_UINT, value);
    }

    pub fn bytes(&mut self, value: &[u8]) {
        self.head(GENESIS_CBOR_MAJOR_BYTES, value.len() as u64);
        self.buf.extend_from_slice(value);
    }

    pub fn text(&mut self, value: &str) {
        self.head(GENESIS_CBOR_MAJOR_TEXT, value.len() as u64);
        self.buf.extend_from_slice(value.as_bytes());
    }

    pub fn array(&mut self, len: u64) {
        self.head(GENESIS_CBOR_MAJOR_ARRAY, len);
    }

    pub fn map(&mut self, len: u64) {
        self.head(GENESIS_CBOR_MAJOR_MAP, len);
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

#[derive(Debug)]
pub struct GenesisCborReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> GenesisCborReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn byte(&mut self) -> Result<u8, GenesisRegistryError> {
        let b = *self
            .data
            .get(self.pos)
            .ok_or(GenesisRegistryError::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], GenesisRegistryError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(GenesisRegistryError::Truncated)?;
        if end > self.data.len() {
            return Err(GenesisRegistryError::Truncated);
        }
        let out = &self.data[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn head(&mut self, expected_major: u8) -> Result<u64, GenesisRegistryError> {
        let initial = self.byte()?;
        let major = initial >> 5;
        let info = initial & 0x1F;
        if major != expected_major {
            return Err(GenesisRegistryError::WrongType);
        }
        let arg = match info {
            0..=23 => u64::from(info),
            24 => {
                let v = u64::from(self.byte()?);
                if v < 24 {
                    return Err(GenesisRegistryError::NonCanonicalEncoding);
                }
                v
            }
            25 => {
                let v = u64::from(u16::from_be_bytes(self.take(2)?.try_into().unwrap()));
                if v <= u8::MAX as u64 {
                    return Err(GenesisRegistryError::NonCanonicalEncoding);
                }
                v
            }
            26 => {
                let v = u64::from(u32::from_be_bytes(self.take(4)?.try_into().unwrap()));
                if v <= u16::MAX as u64 {
                    return Err(GenesisRegistryError::NonCanonicalEncoding);
                }
                v
            }
            27 => {
                let v = u64::from_be_bytes(self.take(8)?.try_into().unwrap());
                if v <= u32::MAX as u64 {
                    return Err(GenesisRegistryError::NonCanonicalEncoding);
                }
                v
            }
            _ => return Err(GenesisRegistryError::NonCanonicalEncoding),
        };
        Ok(arg)
    }

    pub fn uint(&mut self) -> Result<u64, GenesisRegistryError> {
        self.head(GENESIS_CBOR_MAJOR_UINT)
    }

    pub fn bytes(&mut self) -> Result<&'a [u8], GenesisRegistryError> {
        let len = self.head(GENESIS_CBOR_MAJOR_BYTES)?;
        self.take(usize::try_from(len).map_err(|_| GenesisRegistryError::Truncated)?)
    }

    pub fn text(&mut self) -> Result<&'a str, GenesisRegistryError> {
        let len = self.head(GENESIS_CBOR_MAJOR_TEXT)?;
        let raw = self.take(usize::try_from(len).map_err(|_| GenesisRegistryError::Truncated)?)?;
        std::str::from_utf8(raw).map_err(|_| GenesisRegistryError::InvalidTextEncoding)
    }

    pub fn array_len(&mut self) -> Result<u64, GenesisRegistryError> {
        self.head(GENESIS_CBOR_MAJOR_ARRAY)
    }

    pub fn map_len(&mut self) -> Result<u64, GenesisRegistryError> {
        self.head(GENESIS_CBOR_MAJOR_MAP)
    }

    pub fn finish(&self) -> Result<(), GenesisRegistryError> {
        if self.pos != self.data.len() {
            return Err(GenesisRegistryError::TrailingBytes);
        }
        Ok(())
    }
}

/// Reads exactly the closed set of ascending integer labels `1..=expected`,
/// dispatching each value to `field`. Fails closed: duplicates, unknown
/// labels, out-of-order labels, and missing labels are all named errors.
fn genesis_read_closed_map<F>(
    reader: &mut GenesisCborReader<'_>,
    expected: u64,
    mut field: F,
) -> Result<(), GenesisRegistryError>
where
    F: FnMut(u64, &mut GenesisCborReader<'_>) -> Result<(), GenesisRegistryError>,
{
    let len = reader.map_len()?;
    let mut previous: Option<u64> = None;
    let mut seen = 0u64;
    for _ in 0..len {
        let key = reader.uint()?;
        if let Some(prev) = previous {
            if key == prev {
                return Err(GenesisRegistryError::DuplicateField);
            }
            if key < prev {
                return Err(GenesisRegistryError::NonCanonicalEncoding);
            }
        }
        previous = Some(key);
        if key == 0 || key > expected {
            return Err(GenesisRegistryError::UnknownField);
        }
        field(key, reader)?;
        seen += 1;
    }
    if seen != expected {
        return Err(GenesisRegistryError::MissingField);
    }
    Ok(())
}

fn genesis_fixed_bytes<const N: usize>(
    raw: &[u8],
    error: GenesisRegistryError,
) -> Result<[u8; N], GenesisRegistryError> {
    <[u8; N]>::try_from(raw).map_err(|_| error)
}

fn genesis_validate_text(
    value: &str,
    allow_empty: bool,
    error: GenesisRegistryError,
) -> Result<(), GenesisRegistryError> {
    if !allow_empty && value.is_empty() {
        return Err(error);
    }
    if value.len() > GENESIS_REGISTRY_MAX_TEXT_BYTES {
        return Err(error);
    }
    Ok(())
}

fn genesis_validate_master_key(key: &[u8; 33]) -> Result<(), GenesisRegistryError> {
    match key[0] {
        0x02 | 0x03 | 0xED => Ok(()),
        _ => Err(GenesisRegistryError::InvalidMasterKey),
    }
}

// ---------------------------------------------------------------------------
// TemplateTrustGraphV1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateTrustGraphV1 {
    pub n_s: u64,
    pub q_s: u64,
    pub t_s: u64,
}

/// Uniform template trust graph for `n` genesis members:
/// `q_S = ceil(4n/5)`, `t_S = min(ceil(n/5), floor((q_S-1)/2), 2q_S - n - 1)`.
pub fn template_trust_graph_for(n: u64) -> Result<TemplateTrustGraphV1, GenesisRegistryError> {
    if n == 0 {
        return Err(GenesisRegistryError::EmptyEntries);
    }
    if n > GENESIS_REGISTRY_MAX_ENTRIES as u64 {
        return Err(GenesisRegistryError::TooManyEntries);
    }
    let q_s = (4 * n).div_ceil(5);
    let bound = (2 * q_s)
        .checked_sub(n + 1)
        .ok_or(GenesisRegistryError::TrustGraphUnsafe)?;
    let t_s = n.div_ceil(5).min((q_s - 1) / 2).min(bound);
    let graph = TemplateTrustGraphV1 { n_s: n, q_s, t_s };
    graph.validate()?;
    Ok(graph)
}

impl TemplateTrustGraphV1 {
    pub fn validate(&self) -> Result<(), GenesisRegistryError> {
        if self.n_s == 0 {
            return Err(GenesisRegistryError::EmptyEntries);
        }
        if self.n_s > GENESIS_REGISTRY_MAX_ENTRIES as u64 {
            return Err(GenesisRegistryError::TooManyEntries);
        }
        let q_s = (4 * self.n_s).div_ceil(5);
        let bound = (2 * q_s)
            .checked_sub(self.n_s + 1)
            .ok_or(GenesisRegistryError::TrustGraphUnsafe)?;
        let t_s = self.n_s.div_ceil(5).min((q_s - 1) / 2).min(bound);
        if self.q_s != q_s || self.t_s != t_s {
            return Err(GenesisRegistryError::TrustGraphMismatch);
        }
        let safe = self.t_s >= 1
            && self.q_s <= self.n_s
            && 2 * self.t_s < self.q_s
            && self.t_s < 2 * self.q_s - self.n_s;
        if !safe {
            return Err(GenesisRegistryError::TrustGraphUnsafe);
        }
        Ok(())
    }

    fn encode_into(&self, writer: &mut GenesisCborWriter) {
        writer.map(3);
        writer.uint(1);
        writer.uint(self.n_s);
        writer.uint(2);
        writer.uint(self.q_s);
        writer.uint(3);
        writer.uint(self.t_s);
    }

    fn decode_from(reader: &mut GenesisCborReader<'_>) -> Result<Self, GenesisRegistryError> {
        let mut n_s = None;
        let mut q_s = None;
        let mut t_s = None;
        genesis_read_closed_map(reader, 3, |key, r| {
            match key {
                1 => n_s = Some(r.uint()?),
                2 => q_s = Some(r.uint()?),
                3 => t_s = Some(r.uint()?),
                _ => unreachable!(),
            }
            Ok(())
        })?;
        Ok(Self {
            n_s: n_s.ok_or(GenesisRegistryError::MissingField)?,
            q_s: q_s.ok_or(GenesisRegistryError::MissingField)?,
            t_s: t_s.ok_or(GenesisRegistryError::MissingField)?,
        })
    }
}

// ---------------------------------------------------------------------------
// GenesisRoundRefV1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisRoundRefV1 {
    pub fork_network: String,
    pub round_number: u64,
    pub bundle_cid: String,
    pub bundle_digest: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub final_scores_digest: [u8; 32],
    pub selected_unl_digest: [u8; 32],
    pub convergence_report_digest: [u8; 32],
    pub anchor_tx_hash: [u8; 32],
}

impl GenesisRoundRefV1 {
    pub fn validate(&self) -> Result<(), GenesisRegistryError> {
        genesis_validate_text(&self.fork_network, false, GenesisRegistryError::InvalidForkNetwork)?;
        if self.round_number == 0 || self.round_number > GENESIS_REGISTRY_MAX_ROUND_NUMBER {
            return Err(GenesisRegistryError::InvalidRoundNumber);
        }
        genesis_validate_text(&self.bundle_cid, false, GenesisRegistryError::InvalidBundleCid)?;
        Ok(())
    }

    fn encode_into(&self, writer: &mut GenesisCborWriter) {
        writer.map(9);
        writer.uint(1);
        writer.text(&self.fork_network);
        writer.uint(2);
        writer.uint(self.round_number);
        writer.uint(3);
        writer.text(&self.bundle_cid);
        writer.uint(4);
        writer.bytes(&self.bundle_digest);
        writer.uint(5);
        writer.bytes(&self.manifest_digest);
        writer.uint(6);
        writer.bytes(&self.final_scores_digest);
        writer.uint(7);
        writer.bytes(&self.selected_unl_digest);
        writer.uint(8);
        writer.bytes(&self.convergence_report_digest);
        writer.uint(9);
        writer.bytes(&self.anchor_tx_hash);
    }

    fn decode_from(reader: &mut GenesisCborReader<'_>) -> Result<Self, GenesisRegistryError> {
        let mut fork_network = None;
        let mut round_number = None;
        let mut bundle_cid = None;
        let mut digests: [Option<[u8; 32]>; 6] = [None; 6];
        genesis_read_closed_map(reader, 9, |key, r| {
            match key {
                1 => fork_network = Some(r.text()?.to_owned()),
                2 => round_number = Some(r.uint()?),
                3 => bundle_cid = Some(r.text()?.to_owned()),
                4..=9 => {
                    digests[key as usize - 4] = Some(genesis_fixed_bytes(
                        r.bytes()?,
                        GenesisRegistryError::InvalidDigestLength,
                    )?)
                }
                _ => unreachable!(),
            }
            Ok(())
        })?;
        let missing = GenesisRegistryError::MissingField;
        Ok(Self {
            fork_network: fork_network.ok_or(missing)?,
            round_number: round_number.ok_or(missing)?,
            bundle_cid: bundle_cid.ok_or(missing)?,
            bundle_digest: digests[0].ok_or(missing)?,
            manifest_digest: digests[1].ok_or(missing)?,
            final_scores_digest: digests[2].ok_or(missing)?,
            selected_unl_digest: digests[3].ok_or(missing)?,
            convergence_report_digest: digests[4].ok_or(missing)?,
            anchor_tx_hash: digests[5].ok_or(missing)?,
        })
    }
}

// ---------------------------------------------------------------------------
// GenesisReceiptDeadlineRefV1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisReceiptDeadlineRefV1 {
    pub fork_ledger_hash: [u8; 32],
    pub fork_ledger_seq: u64,
}

impl GenesisReceiptDeadlineRefV1 {
    pub fn validate(&self) -> Result<(), GenesisRegistryError> {
        if self.fork_ledger_seq == 0 || self.fork_ledger_seq > GENESIS_REGISTRY_MAX_LEDGER_SEQ {
            return Err(GenesisRegistryError::InvalidLedgerSeq);
        }
        Ok(())
    }

    fn encode_into(&self, writer: &mut GenesisCborWriter) {
        writer.map(2);
        writer.uint(1);
        writer.bytes(&self.fork_ledger_hash);
        writer.uint(2);
        writer.uint(self.fork_ledger_seq);
    }

    fn decode_from(reader: &mut GenesisCborReader<'_>) -> Result<Self, GenesisRegistryError> {
        let mut fork_ledger_hash = None;
        let mut fork_ledger_seq = None;
        genesis_read_closed_map(reader, 2, |key, r| {
            match key {
                1 => {
                    fork_ledger_hash = Some(genesis_fixed_bytes(
                        r.bytes()?,
                        GenesisRegistryError::InvalidDigestLength,
                    )?)
                }
                2 => fork_ledger_seq = Some(r.uint()?),
                _ => unreachable!(),
            }
            Ok(())
        })?;
        Ok(Self {
            fork_ledger_hash: fork_ledger_hash.ok_or(GenesisRegistryError::MissingField)?,
            fork_ledger_seq: fork_ledger_seq.ok_or(GenesisRegistryError::MissingField)?,
        })
    }
}

// ---------------------------------------------------------------------------
// ProposedGenesisEntryV1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedGenesisEntryV1 {
    pub fork_master_key: [u8; 33],
    pub final_score: u64,
    pub cutoff: u64,
    pub selection_index: u64,
    pub identity_evidence_digest: [u8; 32],
    pub identity_receipt_digest: [u8; 32],
    pub mldsa_public_key: Vec<u8>,
}

impl ProposedGenesisEntryV1 {
    pub fn validate(&self) -> Result<(), GenesisRegistryError> {
        genesis_validate_master_key(&self.fork_master_key)?;
        if self.final_score > 100 {
            return Err(GenesisRegistryError::ScoreOutOfRange);
        }
        if self.cutoff > 100 {
            return Err(GenesisRegistryError::CutoffOutOfRange);
        }
        if self.final_score < self.cutoff {
            return Err(GenesisRegistryError::ScoreBelowCutoff);
        }
        if self.selection_index > GENESIS_REGISTRY_MAX_SELECTION_INDEX {
            return Err(GenesisRegistryError::InvalidSelectionIndex);
        }
        if self.mldsa_public_key.len() != GENESIS_MLDSA65_PUBLIC_KEY_LEN {
            return Err(GenesisRegistryError::InvalidMldsaKeyLength);
        }
        Ok(())
    }

    fn encode_into(&self, writer: &mut GenesisCborWriter) {
        writer.map(7);
        writer.uint(1);
        writer.bytes(&self.fork_master_key);
        writer.uint(2);
        writer.uint(self.final_score);
        writer.uint(3);
        writer.uint(self.cutoff);
        writer.uint(4);
        writer.uint(self.selection_index);
        writer.uint(5);
        writer.bytes(&self.identity_evidence_digest);
        writer.uint(6);
        writer.bytes(&self.identity_receipt_digest);
        writer.uint(7);
        writer.bytes(&self.mldsa_public_key);
    }

    fn decode_from(reader: &mut GenesisCborReader<'_>) -> Result<Self, GenesisRegistryError> {
        let mut fork_master_key = None;
        let mut final_score = None;
        let mut cutoff = None;
        let mut selection_index = None;
        let mut identity_evidence_digest = None;
        let mut identity_receipt_digest = None;
        let mut mldsa_public_key = None;
        genesis_read_closed_map(reader, 7, |key, r| {
            match key {
                1 => {
                    fork_master_key = Some(genesis_fixed_bytes(
                        r.bytes()?,
                        GenesisRegistryError::InvalidMasterKey,
                    )?)
                }
                2 => final_score = Some(r.uint()?),
                3 => cutoff = Some(r.uint()?),
                4 => selection_index = Some(r.uint()?),
                5 => {
                    identity_evidence_digest = Some(genesis_fixed_bytes(
                        r.bytes()?,
                        GenesisRegistryError::InvalidDigestLength,
                    )?)
                }
                6 => {
                    identity_receipt_digest = Some(genesis_fixed_bytes(
                        r.bytes()?,
                        GenesisRegistryError::InvalidDigestLength,
                    )?)
                }
                7 => {
                    let raw = r.bytes()?;
                    if raw.len() != GENESIS_MLDSA65_PUBLIC_KEY_LEN {
                        return Err(GenesisRegistryError::InvalidMldsaKeyLength);
                    }
                    mldsa_public_key = Some(raw.to_vec());
                }
                _ => unreachable!(),
            }
            Ok(())
        })?;
        let missing = GenesisRegistryError::MissingField;
        Ok(Self {
            fork_master_key: fork_master_key.ok_or(missing)?,
            final_score: final_score.ok_or(missing)?,
            cutoff: cutoff.ok_or(missing)?,
            selection_index: selection_index.ok_or(missing)?,
            identity_evidence_digest: identity_evidence_digest.ok_or(missing)?,
            identity_receipt_digest: identity_receipt_digest.ok_or(missing)?,
            mldsa_public_key: mldsa_public_key.ok_or(missing)?,
        })
    }
}

// ---------------------------------------------------------------------------
// ProposedGenesisRegistryV1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedGenesisRegistryV1 {
    pub version: u64,
    pub chain_id: String,
    pub genesis_round: GenesisRoundRefV1,
    pub receipt_deadline: GenesisReceiptDeadlineRefV1,
    pub entries: Vec<ProposedGenesisEntryV1>,
    pub template_trust_graph: TemplateTrustGraphV1,
}

impl ProposedGenesisRegistryV1 {
    pub fn validate(&self) -> Result<(), GenesisRegistryError> {
        if self.version != PROPOSED_GENESIS_REGISTRY_VERSION_V1 {
            return Err(GenesisRegistryError::UnknownVersion);
        }
        genesis_validate_text(&self.chain_id, false, GenesisRegistryError::InvalidChainId)?;
        self.genesis_round.validate()?;
        self.receipt_deadline.validate()?;
        if self.entries.is_empty() {
            return Err(GenesisRegistryError::EmptyEntries);
        }
        if self.entries.len() > GENESIS_REGISTRY_MAX_ENTRIES {
            return Err(GenesisRegistryError::TooManyEntries);
        }
        let mut selection_indices = std::collections::BTreeSet::new();
        for (position, entry) in self.entries.iter().enumerate() {
            entry.validate()?;
            if entry.cutoff != self.entries[0].cutoff {
                return Err(GenesisRegistryError::CutoffMismatch);
            }
            if !selection_indices.insert(entry.selection_index) {
                return Err(GenesisRegistryError::DuplicateSelectionIndex);
            }
            if position > 0 {
                let previous = &self.entries[position - 1].fork_master_key;
                match entry.fork_master_key.cmp(previous) {
                    std::cmp::Ordering::Equal => {
                        return Err(GenesisRegistryError::DuplicateMasterKey)
                    }
                    std::cmp::Ordering::Less => return Err(GenesisRegistryError::UnsortedEntries),
                    std::cmp::Ordering::Greater => {}
                }
            }
        }
        if self.template_trust_graph.n_s != self.entries.len() as u64 {
            return Err(GenesisRegistryError::TrustGraphMismatch);
        }
        self.template_trust_graph.validate()?;
        Ok(())
    }

    fn encode_into(&self, writer: &mut GenesisCborWriter) {
        writer.map(6);
        writer.uint(1);
        writer.uint(self.version);
        writer.uint(2);
        writer.text(&self.chain_id);
        writer.uint(3);
        self.genesis_round.encode_into(writer);
        writer.uint(4);
        self.receipt_deadline.encode_into(writer);
        writer.uint(5);
        writer.array(self.entries.len() as u64);
        for entry in &self.entries {
            entry.encode_into(writer);
        }
        writer.uint(6);
        self.template_trust_graph.encode_into(writer);
    }

    /// Validated deterministic-CBOR encoding.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, GenesisRegistryError> {
        self.validate()?;
        let mut writer = GenesisCborWriter::new();
        self.encode_into(&mut writer);
        Ok(writer.into_bytes())
    }

    /// Strict decode: canonical encoding only, then full validation, then a
    /// re-encode equality check as a fail-closed backstop.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, GenesisRegistryError> {
        let mut reader = GenesisCborReader::new(bytes);
        let registry = Self::decode_from(&mut reader)?;
        reader.finish()?;
        registry.validate()?;
        let reencoded = registry.canonical_bytes()?;
        if reencoded != bytes {
            return Err(GenesisRegistryError::NonCanonicalEncoding);
        }
        Ok(registry)
    }

    fn decode_from(reader: &mut GenesisCborReader<'_>) -> Result<Self, GenesisRegistryError> {
        let mut version = None;
        let mut chain_id = None;
        let mut genesis_round = None;
        let mut receipt_deadline = None;
        let mut entries: Option<Vec<ProposedGenesisEntryV1>> = None;
        let mut template_trust_graph = None;
        genesis_read_closed_map(reader, 6, |key, r| {
            match key {
                1 => version = Some(r.uint()?),
                2 => chain_id = Some(r.text()?.to_owned()),
                3 => genesis_round = Some(GenesisRoundRefV1::decode_from(r)?),
                4 => receipt_deadline = Some(GenesisReceiptDeadlineRefV1::decode_from(r)?),
                5 => {
                    let len = r.array_len()?;
                    if len > GENESIS_REGISTRY_MAX_ENTRIES as u64 {
                        return Err(GenesisRegistryError::TooManyEntries);
                    }
                    let mut list = Vec::with_capacity(len as usize);
                    for _ in 0..len {
                        list.push(ProposedGenesisEntryV1::decode_from(r)?);
                    }
                    entries = Some(list);
                }
                6 => template_trust_graph = Some(TemplateTrustGraphV1::decode_from(r)?),
                _ => unreachable!(),
            }
            Ok(())
        })?;
        let missing = GenesisRegistryError::MissingField;
        Ok(Self {
            version: version.ok_or(missing)?,
            chain_id: chain_id.ok_or(missing)?,
            genesis_round: genesis_round.ok_or(missing)?,
            receipt_deadline: receipt_deadline.ok_or(missing)?,
            entries: entries.ok_or(missing)?,
            template_trust_graph: template_trust_graph.ok_or(missing)?,
        })
    }

    /// `digest("L1V2_PROPOSED_GENESIS_REGISTRY_V1", registry)`.
    pub fn proposed_registry_hash(&self) -> Result<[u8; 32], GenesisRegistryError> {
        Ok(genesis_domain_digest(
            PROPOSED_GENESIS_REGISTRY_DOMAIN_V1,
            &self.canonical_bytes()?,
        ))
    }
}

// ---------------------------------------------------------------------------
// GenesisIdentityReceiptBodyV1
// ---------------------------------------------------------------------------

/// Unsigned receipt body from the genesis specification §3.2. Signatures are
/// detached and out of scope for the schema layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisIdentityReceiptBodyV1 {
    pub version: u64,
    pub fork_master_key: [u8; 33],
    pub mldsa_public_key: Vec<u8>,
    pub chain_id: String,
    pub genesis_round_id: String,
    pub deadline_ledger_hash: [u8; 32],
    pub deadline_ledger_seq: u64,
    pub expiry_close_time: u64,
}

impl GenesisIdentityReceiptBodyV1 {
    pub fn validate(&self) -> Result<(), GenesisRegistryError> {
        if self.version != GENESIS_IDENTITY_RECEIPT_VERSION_V1 {
            return Err(GenesisRegistryError::UnknownVersion);
        }
        genesis_validate_master_key(&self.fork_master_key)?;
        if self.mldsa_public_key.len() != GENESIS_MLDSA65_PUBLIC_KEY_LEN {
            return Err(GenesisRegistryError::InvalidMldsaKeyLength);
        }
        genesis_validate_text(&self.chain_id, false, GenesisRegistryError::InvalidChainId)?;
        genesis_validate_text(
            &self.genesis_round_id,
            false,
            GenesisRegistryError::InvalidGenesisRoundId,
        )?;
        if self.deadline_ledger_seq == 0 || self.deadline_ledger_seq > GENESIS_REGISTRY_MAX_LEDGER_SEQ
        {
            return Err(GenesisRegistryError::InvalidLedgerSeq);
        }
        if self.expiry_close_time == 0 || self.expiry_close_time > GENESIS_REGISTRY_MAX_CLOSE_TIME {
            return Err(GenesisRegistryError::InvalidExpiry);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, GenesisRegistryError> {
        self.validate()?;
        let mut writer = GenesisCborWriter::new();
        writer.map(8);
        writer.uint(1);
        writer.uint(self.version);
        writer.uint(2);
        writer.bytes(&self.fork_master_key);
        writer.uint(3);
        writer.bytes(&self.mldsa_public_key);
        writer.uint(4);
        writer.text(&self.chain_id);
        writer.uint(5);
        writer.text(&self.genesis_round_id);
        writer.uint(6);
        writer.bytes(&self.deadline_ledger_hash);
        writer.uint(7);
        writer.uint(self.deadline_ledger_seq);
        writer.uint(8);
        writer.uint(self.expiry_close_time);
        Ok(writer.into_bytes())
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, GenesisRegistryError> {
        let mut reader = GenesisCborReader::new(bytes);
        let mut version = None;
        let mut fork_master_key = None;
        let mut mldsa_public_key = None;
        let mut chain_id = None;
        let mut genesis_round_id = None;
        let mut deadline_ledger_hash = None;
        let mut deadline_ledger_seq = None;
        let mut expiry_close_time = None;
        genesis_read_closed_map(&mut reader, 8, |key, r| {
            match key {
                1 => version = Some(r.uint()?),
                2 => {
                    fork_master_key = Some(genesis_fixed_bytes(
                        r.bytes()?,
                        GenesisRegistryError::InvalidMasterKey,
                    )?)
                }
                3 => {
                    let raw = r.bytes()?;
                    if raw.len() != GENESIS_MLDSA65_PUBLIC_KEY_LEN {
                        return Err(GenesisRegistryError::InvalidMldsaKeyLength);
                    }
                    mldsa_public_key = Some(raw.to_vec());
                }
                4 => chain_id = Some(r.text()?.to_owned()),
                5 => genesis_round_id = Some(r.text()?.to_owned()),
                6 => {
                    deadline_ledger_hash = Some(genesis_fixed_bytes(
                        r.bytes()?,
                        GenesisRegistryError::InvalidDigestLength,
                    )?)
                }
                7 => deadline_ledger_seq = Some(r.uint()?),
                8 => expiry_close_time = Some(r.uint()?),
                _ => unreachable!(),
            }
            Ok(())
        })?;
        reader.finish()?;
        let missing = GenesisRegistryError::MissingField;
        let body = Self {
            version: version.ok_or(missing)?,
            fork_master_key: fork_master_key.ok_or(missing)?,
            mldsa_public_key: mldsa_public_key.ok_or(missing)?,
            chain_id: chain_id.ok_or(missing)?,
            genesis_round_id: genesis_round_id.ok_or(missing)?,
            deadline_ledger_hash: deadline_ledger_hash.ok_or(missing)?,
            deadline_ledger_seq: deadline_ledger_seq.ok_or(missing)?,
            expiry_close_time: expiry_close_time.ok_or(missing)?,
        };
        body.validate()?;
        Ok(body)
    }

    /// `digest("L1V2_IDENTITY_RECEIPT_V1", body)`.
    pub fn receipt_hash(&self) -> Result<[u8; 32], GenesisRegistryError> {
        Ok(genesis_domain_digest(
            GENESIS_IDENTITY_RECEIPT_DOMAIN_V1,
            &self.canonical_bytes()?,
        ))
    }
}

// ---------------------------------------------------------------------------
// GenesisEvidenceRecordV1
// ---------------------------------------------------------------------------

/// Canonical identity-evidence record whose digest is carried per entry:
/// domain, verified-domain status, declared provider and country.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenesisEvidenceRecordV1 {
    pub version: u64,
    pub fork_master_key: [u8; 33],
    pub domain: String,
    pub domain_verified: u64,
    pub provider: String,
    pub country: String,
}

impl GenesisEvidenceRecordV1 {
    pub fn validate(&self) -> Result<(), GenesisRegistryError> {
        if self.version != GENESIS_EVIDENCE_RECORD_VERSION_V1 {
            return Err(GenesisRegistryError::UnknownVersion);
        }
        genesis_validate_master_key(&self.fork_master_key)?;
        genesis_validate_text(&self.domain, true, GenesisRegistryError::InvalidTextEncoding)?;
        if self.domain_verified > 1 {
            return Err(GenesisRegistryError::InvalidDomainFlag);
        }
        genesis_validate_text(&self.provider, true, GenesisRegistryError::InvalidTextEncoding)?;
        genesis_validate_text(&self.country, true, GenesisRegistryError::InvalidTextEncoding)?;
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, GenesisRegistryError> {
        self.validate()?;
        let mut writer = GenesisCborWriter::new();
        writer.map(6);
        writer.uint(1);
        writer.uint(self.version);
        writer.uint(2);
        writer.bytes(&self.fork_master_key);
        writer.uint(3);
        writer.text(&self.domain);
        writer.uint(4);
        writer.uint(self.domain_verified);
        writer.uint(5);
        writer.text(&self.provider);
        writer.uint(6);
        writer.text(&self.country);
        Ok(writer.into_bytes())
    }

    /// `digest("L1V2_GENESIS_EVIDENCE_RECORD_V1", record)`.
    pub fn evidence_digest(&self) -> Result<[u8; 32], GenesisRegistryError> {
        Ok(genesis_domain_digest(
            GENESIS_EVIDENCE_RECORD_DOMAIN_V1,
            &self.canonical_bytes()?,
        ))
    }
}
