use postfiat_crypto_provider::hash_hex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const CRATE_PURPOSE: &str = "FRI/STARK proof adapter interfaces";
pub const DEBUG_PROOF_SYSTEM_ID: &str = "postfiat.debug-shielded-proof.v1";
pub const DEBUG_SHIELDED_MINT_CIRCUIT_ID: &str = "shielded_mint";
pub const DEBUG_SHIELDED_SPEND_CIRCUIT_ID: &str = "shielded_spend";

const DEBUG_SHIELDED_MINT_PUBLIC_INPUTS: &[&str] =
    &["owner", "asset_id", "value", "position", "mint_id"];
const DEBUG_SHIELDED_SPEND_PUBLIC_INPUTS: &[&str] =
    &["note_id", "nullifier", "to", "amount", "spend_id"];
const PROTOCOL_HASH_HEX_LEN: usize = 96;
const LOCAL_DEBUG_CHAIN_ID: &str = "postfiat-local";
const LOCAL_DEBUG_GENESIS_HASH: &str =
    "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicInput {
    pub name: String,
    pub value: String,
}

impl PublicInput {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofStatement {
    pub circuit_id: String,
    pub public_inputs: Vec<PublicInput>,
}

impl ProofStatement {
    pub fn new(circuit_id: impl Into<String>, public_inputs: Vec<PublicInput>) -> Self {
        Self {
            circuit_id: circuit_id.into(),
            public_inputs,
        }
    }

    pub fn validate(&self) -> Result<(), ProofError> {
        validate_identifier("circuit id", &self.circuit_id)?;
        if self.public_inputs.is_empty() {
            return Err(ProofError::new("proof statement has no public inputs"));
        }

        let mut names = BTreeSet::new();
        for input in &self.public_inputs {
            validate_identifier("public input name", &input.name)?;
            if input.value.is_empty() {
                return Err(ProofError::new(format!(
                    "public input `{}` has empty value",
                    input.name
                )));
            }
            if !names.insert(input.name.as_str()) {
                return Err(ProofError::new(format!(
                    "duplicate public input `{}`",
                    input.name
                )));
            }
        }
        Ok(())
    }

    pub fn statement_hash(&self) -> Result<String, ProofError> {
        self.validate()?;
        let mut bytes = Vec::new();
        append_str_field(&mut bytes, "circuit_id", &self.circuit_id);
        append_u64_field(
            &mut bytes,
            "public_input_count",
            self.public_inputs.len() as u64,
        );
        for input in &self.public_inputs {
            append_str_field(&mut bytes, "public_input.name", &input.name);
            append_str_field(&mut bytes, "public_input.value", &input.value);
        }
        Ok(hash_hex("postfiat.proof.statement.v1", &bytes))
    }
}

fn append_str_field(bytes: &mut Vec<u8>, label: &str, value: &str) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(value.len().to_string().as_bytes());
    bytes.push(b':');
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(b'\n');
}

fn append_u64_field(bytes: &mut Vec<u8>, label: &str, value: u64) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(value.to_string().as_bytes());
    bytes.push(b'\n');
}

fn validate_identifier(label: &str, value: &str) -> Result<(), ProofError> {
    if value.is_empty() {
        return Err(ProofError::new(format!("{label} is empty")));
    }
    if value.trim() != value {
        return Err(ProofError::new(format!(
            "{label} `{value}` has leading or trailing whitespace"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofArtifact {
    pub proof_system_id: String,
    pub circuit_id: String,
    pub statement_hash: String,
    pub proof_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofError {
    message: String,
}

impl ProofError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ProofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ProofError {}

pub trait ProofSystem {
    fn prove(&self, statement: &ProofStatement) -> Result<ProofArtifact, ProofError>;
    fn verify(
        &self,
        statement: &ProofStatement,
        artifact: &ProofArtifact,
    ) -> Result<(), ProofError>;
}

#[derive(Debug, Clone, Copy)]
pub struct DebugProofSystem {
    _capability: DebugProofCapability,
}

#[derive(Debug, Clone, Copy)]
struct DebugProofCapability;

impl DebugProofSystem {
    pub fn for_controlled_testnet_debug() -> Result<Self, ProofError> {
        Self::for_chain(LOCAL_DEBUG_CHAIN_ID, LOCAL_DEBUG_GENESIS_HASH)
    }

    pub fn for_chain(chain_id: &str, genesis_hash: &str) -> Result<Self, ProofError> {
        Self::from_gate(debug_proofs_enabled_for_chain(chain_id, genesis_hash))
    }

    fn from_gate(enabled: bool) -> Result<Self, ProofError> {
        if enabled {
            Ok(Self {
                _capability: DebugProofCapability,
            })
        } else {
            Err(ProofError::new(
                "debug proof system is disabled for this chain; debug proofs require an explicit debug/test chain id and canonical genesis hash",
            ))
        }
    }
}

pub fn debug_proofs_enabled_for_chain(chain_id: &str, genesis_hash: &str) -> bool {
    debug_proof_chain_id_allowed(chain_id) && is_lower_hex_len(genesis_hash, PROTOCOL_HASH_HEX_LEN)
}

fn debug_proof_chain_id_allowed(chain_id: &str) -> bool {
    matches!(
        chain_id,
        "postfiat-local"
            | "postfiat-test"
            | "postfiat-vector-test"
            | "postfiat-wallet-sign-transfer"
            | "postfiat-wan-devnet"
    )
}

impl ProofSystem for DebugProofSystem {
    fn prove(&self, statement: &ProofStatement) -> Result<ProofArtifact, ProofError> {
        validate_debug_statement_schema(statement)?;
        let statement_hash = statement.statement_hash()?;
        let proof_hash = debug_proof_hash(&statement_hash);
        Ok(ProofArtifact {
            proof_system_id: DEBUG_PROOF_SYSTEM_ID.to_string(),
            circuit_id: statement.circuit_id.clone(),
            statement_hash,
            proof_hash,
        })
    }

    fn verify(
        &self,
        statement: &ProofStatement,
        artifact: &ProofArtifact,
    ) -> Result<(), ProofError> {
        if artifact.proof_system_id != DEBUG_PROOF_SYSTEM_ID {
            return Err(ProofError::new(format!(
                "unsupported proof system `{}`",
                artifact.proof_system_id
            )));
        }
        if artifact.circuit_id != statement.circuit_id {
            return Err(ProofError::new(format!(
                "circuit mismatch expected `{}` got `{}`",
                statement.circuit_id, artifact.circuit_id
            )));
        }
        validate_debug_statement_schema(statement)?;
        let expected_statement_hash = statement.statement_hash()?;
        if artifact.statement_hash != expected_statement_hash {
            return Err(ProofError::new("statement hash mismatch"));
        }
        let expected_proof_hash = debug_proof_hash(&artifact.statement_hash);
        if artifact.proof_hash != expected_proof_hash {
            return Err(ProofError::new("proof hash mismatch"));
        }
        Ok(())
    }
}

fn validate_debug_statement_schema(statement: &ProofStatement) -> Result<(), ProofError> {
    statement.validate()?;
    let expected_inputs = match statement.circuit_id.as_str() {
        DEBUG_SHIELDED_MINT_CIRCUIT_ID => DEBUG_SHIELDED_MINT_PUBLIC_INPUTS,
        DEBUG_SHIELDED_SPEND_CIRCUIT_ID => DEBUG_SHIELDED_SPEND_PUBLIC_INPUTS,
        other => {
            return Err(ProofError::new(format!(
                "unsupported debug proof circuit `{other}`"
            )));
        }
    };
    if statement.public_inputs.len() != expected_inputs.len() {
        return Err(ProofError::new(format!(
            "debug circuit `{}` expects {} public inputs got {}",
            statement.circuit_id,
            expected_inputs.len(),
            statement.public_inputs.len()
        )));
    }
    for (index, (input, expected_name)) in statement
        .public_inputs
        .iter()
        .zip(expected_inputs.iter())
        .enumerate()
    {
        if input.name != *expected_name {
            return Err(ProofError::new(format!(
                "debug circuit `{}` public input {index} expected `{expected_name}` got `{}`",
                statement.circuit_id, input.name
            )));
        }
    }
    match statement.circuit_id.as_str() {
        DEBUG_SHIELDED_MINT_CIRCUIT_ID => {
            validate_plain_public_input_value(&statement.public_inputs[0])?;
            validate_plain_public_input_value(&statement.public_inputs[1])?;
            validate_decimal_public_input(&statement.public_inputs[2], false)?;
            validate_decimal_public_input(&statement.public_inputs[3], true)?;
            validate_protocol_hash_public_input(&statement.public_inputs[4])?;
        }
        DEBUG_SHIELDED_SPEND_CIRCUIT_ID => {
            validate_protocol_hash_public_input(&statement.public_inputs[0])?;
            validate_protocol_hash_public_input(&statement.public_inputs[1])?;
            validate_plain_public_input_value(&statement.public_inputs[2])?;
            validate_decimal_public_input(&statement.public_inputs[3], false)?;
            validate_protocol_hash_public_input(&statement.public_inputs[4])?;
        }
        _ => unreachable!("debug circuit id matched above"),
    }
    Ok(())
}

fn validate_plain_public_input_value(input: &PublicInput) -> Result<(), ProofError> {
    if input.value.trim() != input.value {
        return Err(ProofError::new(format!(
            "public input `{}` has leading or trailing whitespace in value",
            input.name
        )));
    }
    Ok(())
}

fn validate_decimal_public_input(input: &PublicInput, allow_zero: bool) -> Result<(), ProofError> {
    let parsed = input.value.parse::<u64>().map_err(|_| {
        ProofError::new(format!(
            "public input `{}` must be an unsigned decimal integer",
            input.name
        ))
    })?;
    if !allow_zero && parsed == 0 {
        return Err(ProofError::new(format!(
            "public input `{}` must be greater than zero",
            input.name
        )));
    }
    Ok(())
}

fn validate_protocol_hash_public_input(input: &PublicInput) -> Result<(), ProofError> {
    if !is_lower_hex_len(&input.value, PROTOCOL_HASH_HEX_LEN) {
        return Err(ProofError::new(format!(
            "public input `{}` must be {PROTOCOL_HASH_HEX_LEN} lowercase hex characters",
            input.name
        )));
    }
    Ok(())
}

fn is_lower_hex_len(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn debug_proof_hash(statement_hash: &str) -> String {
    hash_hex(
        "postfiat.proof.debug_artifact.v1",
        format!("proof_system={DEBUG_PROOF_SYSTEM_ID}\nstatement_hash={statement_hash}\n")
            .as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex96(byte: u8) -> String {
        std::iter::repeat_n(byte as char, PROTOCOL_HASH_HEX_LEN).collect()
    }

    fn statement() -> ProofStatement {
        ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![
                PublicInput::new("note_id", hex96(b'a')),
                PublicInput::new("nullifier", hex96(b'b')),
                PublicInput::new("to", "bob"),
                PublicInput::new("amount", "10"),
                PublicInput::new("spend_id", hex96(b'c')),
            ],
        )
    }

    fn mint_statement() -> ProofStatement {
        ProofStatement::new(
            DEBUG_SHIELDED_MINT_CIRCUIT_ID,
            vec![
                PublicInput::new("owner", "alice"),
                PublicInput::new("asset_id", "POSTFIAT"),
                PublicInput::new("value", "10"),
                PublicInput::new("position", "0"),
                PublicInput::new("mint_id", hex96(b'd')),
            ],
        )
    }

    fn debug_system() -> DebugProofSystem {
        DebugProofSystem::for_chain("postfiat-local", &hex96(b'0')).expect("debug proofs enabled")
    }

    #[test]
    fn debug_system_proves_and_verifies() {
        let system = debug_system();
        let statement = statement();
        let artifact = system.prove(&statement).expect("prove");

        system.verify(&statement, &artifact).expect("verify");
    }

    #[test]
    fn debug_system_accepts_mint_schema() {
        let system = debug_system();
        let statement = mint_statement();
        let artifact = system.prove(&statement).expect("prove");

        system.verify(&statement, &artifact).expect("verify");
    }

    #[test]
    fn debug_proof_gate_rejects_mainnet_chain_ids() {
        assert!(!debug_proofs_enabled_for_chain(
            "postfiat-mainnet",
            &hex96(b'1')
        ));
        assert!(!debug_proofs_enabled_for_chain("postfiat-local", "bad"));

        let error = DebugProofSystem::for_chain("postfiat-mainnet", &hex96(b'1'))
            .expect_err("mainnet debug proof gate must fail closed");
        assert!(error.to_string().contains("disabled for this chain"));
    }

    #[test]
    fn debug_proof_gate_allows_explicit_debug_chain_ids() {
        assert!(debug_proofs_enabled_for_chain(
            "postfiat-local",
            &hex96(b'1')
        ));
        assert!(DebugProofSystem::for_chain("postfiat-wan-devnet", &hex96(b'2')).is_ok());
    }

    #[test]
    fn statement_hash_has_canonical_golden_vector() {
        assert_eq!(
            mint_statement().statement_hash().expect("statement hash"),
            "a4bb4a59f16d760c1151f39b9cfadcd49028eb9a47151d6194af78361fddfffd341c38ee06336c79e270ebc637cbfb16"
        );
    }

    #[test]
    fn debug_system_rejects_tampered_artifacts() {
        let system = debug_system();
        let statement = statement();
        let mut artifact = system.prove(&statement).expect("prove");
        artifact.proof_hash = "bad".to_string();

        assert!(system.verify(&statement, &artifact).is_err());
    }

    #[test]
    fn debug_system_rejects_unknown_circuit() {
        let system = debug_system();
        let statement =
            ProofStatement::new("unknown_circuit", vec![PublicInput::new("input", "value")]);

        let error = system.prove(&statement).expect_err("unknown circuit");

        assert!(error
            .to_string()
            .contains("unsupported debug proof circuit"));
    }

    #[test]
    fn debug_system_rejects_wrong_public_input_count() {
        let system = debug_system();
        let statement = ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![
                PublicInput::new("note_id", hex96(b'a')),
                PublicInput::new("nullifier", hex96(b'b')),
            ],
        );

        let error = system.prove(&statement).expect_err("input count");

        assert!(error.to_string().contains("expects 5 public inputs"));
    }

    #[test]
    fn debug_system_rejects_wrong_public_input_order() {
        let system = debug_system();
        let statement = ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![
                PublicInput::new("nullifier", hex96(b'b')),
                PublicInput::new("note_id", hex96(b'a')),
                PublicInput::new("to", "bob"),
                PublicInput::new("amount", "10"),
                PublicInput::new("spend_id", hex96(b'c')),
            ],
        );

        let error = system.prove(&statement).expect_err("input order");

        assert!(error.to_string().contains("expected `note_id`"));
    }

    #[test]
    fn debug_system_rejects_malformed_hash_public_input() {
        let system = debug_system();
        let mut statement = statement();
        statement.public_inputs[0].value = "not-hex".to_string();

        let error = system.prove(&statement).expect_err("malformed hash input");

        assert!(error.to_string().contains("96 lowercase hex"));
    }

    #[test]
    fn debug_system_rejects_zero_amount_public_input() {
        let system = debug_system();
        let mut statement = statement();
        statement.public_inputs[3].value = "0".to_string();

        let error = system.prove(&statement).expect_err("zero amount");

        assert!(error.to_string().contains("greater than zero"));
    }

    #[test]
    fn debug_system_rejects_non_decimal_position_public_input() {
        let system = debug_system();
        let mut statement = mint_statement();
        statement.public_inputs[3].value = "first".to_string();

        let error = system.prove(&statement).expect_err("non-decimal position");

        assert!(error.to_string().contains("unsigned decimal integer"));
    }

    #[test]
    fn debug_system_rejects_boundary_whitespace_in_plain_public_input_value() {
        let system = debug_system();
        let mut statement = mint_statement();
        statement.public_inputs[0].value = " alice".to_string();

        let error = system.prove(&statement).expect_err("whitespace owner");

        assert!(error.to_string().contains("leading or trailing whitespace"));
    }

    #[test]
    fn statement_validation_rejects_empty_circuit_id() {
        let statement = ProofStatement::new("", vec![PublicInput::new("nullifier", "abc")]);

        let error = statement.statement_hash().expect_err("empty circuit id");

        assert!(error.to_string().contains("circuit id is empty"));
    }

    #[test]
    fn statement_validation_rejects_empty_public_inputs() {
        let statement = ProofStatement::new("shielded_spend", Vec::new());

        let error = statement.statement_hash().expect_err("empty inputs");

        assert!(error.to_string().contains("no public inputs"));
    }

    #[test]
    fn statement_validation_rejects_empty_public_input_name() {
        let statement = ProofStatement::new("shielded_spend", vec![PublicInput::new("", "abc")]);

        let error = statement.statement_hash().expect_err("empty input name");

        assert!(error.to_string().contains("public input name is empty"));
    }

    #[test]
    fn statement_validation_rejects_empty_public_input_value() {
        let statement =
            ProofStatement::new("shielded_spend", vec![PublicInput::new("nullifier", "")]);

        let error = statement.statement_hash().expect_err("empty input value");

        assert!(error.to_string().contains("empty value"));
    }

    #[test]
    fn statement_validation_rejects_duplicate_public_input_names() {
        let statement = ProofStatement::new(
            "shielded_spend",
            vec![
                PublicInput::new("nullifier", "abc"),
                PublicInput::new("nullifier", "def"),
            ],
        );

        let error = statement.statement_hash().expect_err("duplicate input");

        assert!(error.to_string().contains("duplicate public input"));
    }

    #[test]
    fn statement_validation_rejects_boundary_whitespace_in_schema_ids() {
        let statement = ProofStatement::new(
            " shielded_spend",
            vec![PublicInput::new("nullifier ", "abc")],
        );

        let error = statement.statement_hash().expect_err("whitespace");

        assert!(error.to_string().contains("leading or trailing whitespace"));
    }
}
