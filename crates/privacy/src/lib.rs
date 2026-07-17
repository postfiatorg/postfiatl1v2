use postfiat_crypto_provider::hash_hex;
use postfiat_proofs::{
    debug_proofs_enabled_for_chain, DebugProofSystem, ProofError, ProofStatement, ProofSystem,
    PublicInput, DEBUG_SHIELDED_MINT_CIRCUIT_ID, DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
};
use postfiat_types::{
    ShieldedDisclosure, ShieldedNote, ShieldedSpendResult, ShieldedState, TurnstileEvent,
    TurnstileSummary, DEBUG_SHIELDED_POOL_ID, DEFAULT_SHIELDED_ASSET_ID,
    TURNSTILE_KIND_BOOTSTRAP_DEPOSIT, TURNSTILE_KIND_ORCHARD_DEPOSIT,
    TURNSTILE_KIND_POOL_MIGRATION,
};

pub const CRATE_PURPOSE: &str = "debug shielded value semantics and proof adapter boundary";
pub const TRANSPARENT_BOOTSTRAP_POOL_ID: &str = "transparent-bootstrap";
const LOCAL_DEBUG_CHAIN_ID: &str = "postfiat-local";
const LOCAL_DEBUG_GENESIS_HASH: &str =
    "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldedError {
    code: &'static str,
    message: String,
}

impl ShieldedError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.code
    }
}

impl std::fmt::Display for ShieldedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ShieldedError {}

#[derive(Debug)]
struct NoteSeed<'a> {
    owner: &'a str,
    asset_id: &'a str,
    value: u64,
    memo: &'a str,
    position: u64,
    created_by: &'a str,
}

#[derive(Debug)]
struct SpendSeed<'a> {
    note_id: &'a str,
    nullifier: &'a str,
    to: &'a str,
    amount: u64,
    memo: &'a str,
}

#[derive(Debug)]
struct TurnstileSeed<'a> {
    kind: &'a str,
    owner: &'a str,
    asset_id: &'a str,
    amount: u64,
    note_id: &'a str,
    source_pool: &'a str,
    target_pool: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldedChainContext<'a> {
    pub chain_id: &'a str,
    pub genesis_hash: &'a str,
}

pub fn mint_debug_note(
    state: &mut ShieldedState,
    owner: impl Into<String>,
    asset_id: impl Into<String>,
    value: u64,
    memo: impl Into<String>,
) -> Result<ShieldedNote, ShieldedError> {
    mint_debug_note_with_creator(state, owner, asset_id, value, memo, "external-mint")
}

pub fn mint_debug_note_with_creator(
    state: &mut ShieldedState,
    owner: impl Into<String>,
    asset_id: impl Into<String>,
    value: u64,
    memo: impl Into<String>,
    created_by: impl Into<String>,
) -> Result<ShieldedNote, ShieldedError> {
    mint_debug_note_with_creator_for_chain(
        state,
        ShieldedChainContext {
            chain_id: LOCAL_DEBUG_CHAIN_ID,
            genesis_hash: LOCAL_DEBUG_GENESIS_HASH,
        },
        owner,
        asset_id,
        value,
        memo,
        created_by,
    )
}

pub fn mint_debug_note_with_creator_for_chain(
    state: &mut ShieldedState,
    chain: ShieldedChainContext<'_>,
    owner: impl Into<String>,
    asset_id: impl Into<String>,
    value: u64,
    memo: impl Into<String>,
    created_by: impl Into<String>,
) -> Result<ShieldedNote, ShieldedError> {
    let owner = owner.into();
    let asset_id = normalize_asset_id(asset_id.into());
    let memo = memo.into();
    let created_by = created_by.into();
    let ShieldedChainContext {
        chain_id,
        genesis_hash,
    } = chain;
    validate_owner("owner", &owner)?;
    validate_asset_id("asset_id", &asset_id)?;
    validate_positive_value(value)?;
    validate_plain_value("created_by", &created_by)?;

    let position = next_note_position(state)?;
    let mint_id = hash_note_seed(
        "postfiat.shielded.mint.debug.v1",
        &NoteSeed {
            owner: &owner,
            asset_id: &asset_id,
            value,
            memo: &memo,
            position,
            created_by: &created_by,
        },
    )?;
    verify_debug_statement_for_chain(
        chain_id,
        genesis_hash,
        ProofStatement::new(
            DEBUG_SHIELDED_MINT_CIRCUIT_ID,
            vec![
                PublicInput::new("owner", owner.clone()),
                PublicInput::new("asset_id", asset_id.clone()),
                PublicInput::new("value", value.to_string()),
                PublicInput::new("position", position.to_string()),
                PublicInput::new("mint_id", mint_id.clone()),
            ],
        ),
    )?;
    let note = build_note_at_position(position, owner, asset_id, value, memo, mint_id)?;
    advance_note_position(state, position)?;
    state.notes.push(note.clone());
    record_turnstile_event(
        state,
        TurnstileSeed {
            kind: TURNSTILE_KIND_BOOTSTRAP_DEPOSIT,
            owner: &note.owner,
            asset_id: &note.asset_id,
            amount: note.value,
            note_id: &note.note_id,
            source_pool: TRANSPARENT_BOOTSTRAP_POOL_ID,
            target_pool: DEBUG_SHIELDED_POOL_ID,
        },
    )?;
    Ok(note)
}

pub fn spend_debug_note(
    state: &mut ShieldedState,
    note_id: &str,
    to: impl Into<String>,
    amount: u64,
    memo: impl Into<String>,
) -> Result<ShieldedSpendResult, ShieldedError> {
    spend_debug_note_for_chain(
        state,
        LOCAL_DEBUG_CHAIN_ID,
        LOCAL_DEBUG_GENESIS_HASH,
        note_id,
        to,
        amount,
        memo,
    )
}

pub fn spend_debug_note_for_chain(
    state: &mut ShieldedState,
    chain_id: &str,
    genesis_hash: &str,
    note_id: &str,
    to: impl Into<String>,
    amount: u64,
    memo: impl Into<String>,
) -> Result<ShieldedSpendResult, ShieldedError> {
    let to = to.into();
    let memo = memo.into();
    validate_owner("to", &to)?;
    validate_positive_value(amount)?;

    let source = state
        .note(note_id)
        .cloned()
        .ok_or_else(|| ShieldedError::new("missing_note", format!("note `{note_id}` not found")))?;
    let nullifier = debug_nullifier(note_id);
    if state.is_nullified(&nullifier) {
        return Err(ShieldedError::new(
            "duplicate_nullifier",
            format!("nullifier `{nullifier}` already exists"),
        ));
    }
    if amount > source.value {
        return Err(ShieldedError::new(
            "insufficient_note_value",
            format!("note value {} is below spend amount {amount}", source.value),
        ));
    }

    let spend_id = hash_spend_seed(
        "postfiat.shielded.spend.debug.v1",
        &SpendSeed {
            note_id,
            nullifier: &nullifier,
            to: &to,
            amount,
            memo: &memo,
        },
    )?;
    verify_debug_statement_for_chain(
        chain_id,
        genesis_hash,
        ProofStatement::new(
            DEBUG_SHIELDED_SPEND_CIRCUIT_ID,
            vec![
                PublicInput::new("note_id", note_id),
                PublicInput::new("nullifier", nullifier.clone()),
                PublicInput::new("to", to.clone()),
                PublicInput::new("amount", amount.to_string()),
                PublicInput::new("spend_id", spend_id.clone()),
            ],
        ),
    )?;

    let mut outputs = Vec::new();
    let recipient_position = next_note_position(state)?;
    let recipient = build_note_at_position(
        recipient_position,
        to,
        source.asset_id.clone(),
        amount,
        memo,
        spend_id.clone(),
    )?;

    let change = source.value - amount;
    let change_note = if change > 0 {
        let change_position = next_position_after(recipient_position)?;
        Some(build_note_at_position(
            change_position,
            source.owner,
            source.asset_id,
            change,
            format!("change:{spend_id}"),
            spend_id.clone(),
        )?)
    } else {
        None
    };

    state.nullifiers.push(nullifier.clone());
    advance_note_position(state, recipient_position)?;
    state.notes.push(recipient.clone());
    outputs.push(recipient);
    if let Some(change_note) = change_note {
        advance_note_position(state, change_note.position)?;
        state.notes.push(change_note.clone());
        outputs.push(change_note);
    }

    Ok(ShieldedSpendResult {
        spend_id,
        spent_note_id: note_id.to_string(),
        nullifier,
        outputs,
    })
}

pub fn migrate_debug_note(
    state: &mut ShieldedState,
    note_id: &str,
    target_pool: impl Into<String>,
    _memo: impl Into<String>,
) -> Result<TurnstileEvent, ShieldedError> {
    let target_pool = target_pool.into();
    validate_pool("target_pool", &target_pool)?;
    if target_pool == DEBUG_SHIELDED_POOL_ID {
        return Err(ShieldedError::new(
            "same_pool_migration",
            format!("target pool `{target_pool}` already holds debug shielded notes"),
        ));
    }
    let note = state
        .note(note_id)
        .cloned()
        .ok_or_else(|| ShieldedError::new("missing_note", format!("note `{note_id}` not found")))?;
    let nullifier = debug_nullifier(note_id);
    if state.is_nullified(&nullifier) {
        return Err(ShieldedError::new(
            "spent_note",
            format!("note `{note_id}` is already spent"),
        ));
    }
    if state.turnstile_events.iter().any(|event| {
        event.kind == TURNSTILE_KIND_POOL_MIGRATION
            && event.note_id == note_id
            && event.target_pool == target_pool
    }) {
        return Err(ShieldedError::new(
            "duplicate_turnstile_migration",
            format!("note `{note_id}` is already accounted for target pool `{target_pool}`"),
        ));
    }

    let event = record_turnstile_event(
        state,
        TurnstileSeed {
            kind: TURNSTILE_KIND_POOL_MIGRATION,
            owner: &note.owner,
            asset_id: &note.asset_id,
            amount: note.value,
            note_id: &note.note_id,
            source_pool: DEBUG_SHIELDED_POOL_ID,
            target_pool: &target_pool,
        },
    )?;
    state.nullifiers.push(nullifier);
    Ok(event)
}

pub fn scan_owner(state: &ShieldedState, owner: &str) -> Vec<ShieldedNote> {
    state
        .notes
        .iter()
        .filter(|note| note.owner == owner)
        .filter(|note| !state.is_nullified(&debug_nullifier(&note.note_id)))
        .cloned()
        .collect()
}

pub fn turnstile_summary(state: &ShieldedState) -> TurnstileSummary {
    let bootstrap_deposit_total = state
        .turnstile_events
        .iter()
        .filter(|event| event.kind == TURNSTILE_KIND_BOOTSTRAP_DEPOSIT)
        .map(|event| event.amount)
        .sum();
    let migration_total = state
        .turnstile_events
        .iter()
        .filter(|event| event.kind == TURNSTILE_KIND_POOL_MIGRATION)
        .map(|event| event.amount)
        .sum();
    let orchard_deposit_total = state
        .turnstile_events
        .iter()
        .filter(|event| event.kind == TURNSTILE_KIND_ORCHARD_DEPOSIT)
        .map(|event| event.amount)
        .sum();
    TurnstileSummary {
        event_count: state.turnstile_events.len() as u64,
        bootstrap_deposit_total,
        migration_total,
        orchard_deposit_total,
        events: state.turnstile_events.clone(),
    }
}

pub fn disclose_note(
    state: &ShieldedState,
    note_id: &str,
) -> Result<ShieldedDisclosure, ShieldedError> {
    let note = state
        .note(note_id)
        .cloned()
        .ok_or_else(|| ShieldedError::new("missing_note", format!("note `{note_id}` not found")))?;
    let nullifier = debug_nullifier(note_id);
    let spent = state.is_nullified(&nullifier);
    Ok(ShieldedDisclosure {
        note,
        nullifier,
        spent,
    })
}

pub fn debug_nullifier(note_id: &str) -> String {
    hash_hex("postfiat.shielded.nullifier.debug.v1", note_id.as_bytes())
}

pub fn debug_note_commitment(
    owner: &str,
    asset_id: &str,
    value: u64,
    memo: &str,
    position: u64,
    created_by: &str,
) -> Result<String, ShieldedError> {
    validate_owner("owner", owner)?;
    validate_asset_id("asset_id", asset_id)?;
    validate_positive_value(value)?;
    validate_plain_value("created_by", created_by)?;
    hash_note_seed(
        "postfiat.shielded.commitment.debug.v1",
        &NoteSeed {
            owner,
            asset_id,
            value,
            memo,
            position,
            created_by,
        },
    )
}

pub fn debug_note_id(commitment: &str) -> String {
    hash_hex("postfiat.shielded.note.debug.v1", commitment.as_bytes())
}

pub fn debug_note_rho(note_id: &str) -> String {
    hash_hex("postfiat.shielded.rho.debug.v1", note_id.as_bytes())
}

pub fn debug_turnstile_event_id(
    kind: &str,
    owner: &str,
    asset_id: &str,
    amount: u64,
    note_id: &str,
    source_pool: &str,
    target_pool: &str,
) -> Result<String, ShieldedError> {
    validate_plain_value("kind", kind)?;
    validate_owner("owner", owner)?;
    validate_asset_id("asset_id", asset_id)?;
    validate_positive_value(amount)?;
    validate_plain_value("note_id", note_id)?;
    validate_pool("source_pool", source_pool)?;
    validate_pool("target_pool", target_pool)?;
    hash_turnstile_seed(
        "postfiat.turnstile.event.debug.v1",
        &TurnstileSeed {
            kind,
            owner,
            asset_id,
            amount,
            note_id,
            source_pool,
            target_pool,
        },
    )
}

pub fn note_tree_root(state: &ShieldedState) -> Result<String, ShieldedError> {
    hash_notes("postfiat.shielded.note_tree.debug.v1", &state.notes)
}

fn build_note_at_position(
    position: u64,
    owner: String,
    asset_id: String,
    value: u64,
    memo: String,
    created_by: String,
) -> Result<ShieldedNote, ShieldedError> {
    let commitment = debug_note_commitment(&owner, &asset_id, value, &memo, position, &created_by)?;
    let note_id = debug_note_id(&commitment);
    let rho = debug_note_rho(&note_id);
    Ok(ShieldedNote {
        note_id,
        commitment,
        position,
        owner,
        asset_id,
        value,
        rho,
        memo,
        created_by,
    })
}

fn normalize_asset_id(asset_id: String) -> String {
    if asset_id.is_empty() {
        DEFAULT_SHIELDED_ASSET_ID.to_string()
    } else {
        asset_id
    }
}

fn validate_owner(field: &'static str, owner: &str) -> Result<(), ShieldedError> {
    validate_text_seed(field, owner, "empty_owner")
}

fn validate_asset_id(field: &'static str, asset_id: &str) -> Result<(), ShieldedError> {
    validate_text_seed(field, asset_id, "empty_asset_id")
}

fn validate_pool(field: &'static str, pool: &str) -> Result<(), ShieldedError> {
    validate_text_seed(field, pool, "empty_pool")
}

fn validate_plain_value(field: &'static str, value: &str) -> Result<(), ShieldedError> {
    validate_text_seed(field, value, "empty_value")
}

fn validate_text_seed(
    field: &'static str,
    value: &str,
    empty_code: &'static str,
) -> Result<(), ShieldedError> {
    if value.trim().is_empty() {
        return Err(ShieldedError::new(
            empty_code,
            format!("{field} must be nonempty"),
        ));
    }
    if value != value.trim() {
        return Err(ShieldedError::new(
            "boundary_whitespace",
            format!("{field} must not have boundary whitespace"),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(ShieldedError::new(
            "control_character",
            format!("{field} must not contain control characters"),
        ));
    }
    Ok(())
}

fn validate_positive_value(value: u64) -> Result<(), ShieldedError> {
    if value == 0 {
        return Err(ShieldedError::new(
            "zero_value",
            "shielded note value must be nonzero",
        ));
    }
    Ok(())
}

fn record_turnstile_event(
    state: &mut ShieldedState,
    event: TurnstileSeed<'_>,
) -> Result<TurnstileEvent, ShieldedError> {
    let event_id = hash_turnstile_seed("postfiat.turnstile.event.debug.v1", &event)?;
    if state
        .turnstile_events
        .iter()
        .any(|event| event.event_id == event_id)
    {
        return Err(ShieldedError::new(
            "duplicate_turnstile_event",
            format!("turnstile event `{event_id}` already exists"),
        ));
    }
    let event = TurnstileEvent {
        event_id,
        kind: event.kind.to_string(),
        owner: event.owner.to_string(),
        asset_id: event.asset_id.to_string(),
        amount: event.amount,
        note_id: event.note_id.to_string(),
        source_pool: event.source_pool.to_string(),
        target_pool: event.target_pool.to_string(),
    };
    state.turnstile_events.push(event.clone());
    Ok(event)
}

pub fn debug_shielded_pool_enabled_for_chain(chain_id: &str, genesis_hash: &str) -> bool {
    debug_proofs_enabled_for_chain(chain_id, genesis_hash)
}

fn verify_debug_statement_for_chain(
    chain_id: &str,
    genesis_hash: &str,
    statement: ProofStatement,
) -> Result<(), ShieldedError> {
    let proof_system = DebugProofSystem::for_chain(chain_id, genesis_hash).map_err(proof_error)?;
    let artifact = proof_system.prove(&statement).map_err(proof_error)?;
    proof_system
        .verify(&statement, &artifact)
        .map_err(proof_error)
}

fn proof_error(error: ProofError) -> ShieldedError {
    ShieldedError::new("proof_verification_failed", error.to_string())
}

fn next_note_position(state: &ShieldedState) -> Result<u64, ShieldedError> {
    let mut next = state.next_note_position;
    for note in &state.notes {
        let candidate = next_position_after(note.position)?;
        if candidate > next {
            next = candidate;
        }
    }
    Ok(next)
}

fn next_position_after(position: u64) -> Result<u64, ShieldedError> {
    position.checked_add(1).ok_or_else(|| {
        ShieldedError::new(
            "position_overflow",
            "shielded note position space is exhausted",
        )
    })
}

fn advance_note_position(state: &mut ShieldedState, position: u64) -> Result<(), ShieldedError> {
    let next = next_position_after(position)?;
    if state.next_note_position < next {
        state.next_note_position = next;
    }
    Ok(())
}

fn hash_note_seed(domain: &str, seed: &NoteSeed<'_>) -> Result<String, ShieldedError> {
    let mut bytes = Vec::new();
    append_str_field(&mut bytes, "owner", seed.owner);
    append_str_field(&mut bytes, "asset_id", seed.asset_id);
    append_u64_field(&mut bytes, "value", seed.value);
    append_str_field(&mut bytes, "memo", seed.memo);
    append_u64_field(&mut bytes, "position", seed.position);
    append_str_field(&mut bytes, "created_by", seed.created_by);
    Ok(hash_hex(domain, &bytes))
}

fn hash_spend_seed(domain: &str, seed: &SpendSeed<'_>) -> Result<String, ShieldedError> {
    let mut bytes = Vec::new();
    append_str_field(&mut bytes, "note_id", seed.note_id);
    append_str_field(&mut bytes, "nullifier", seed.nullifier);
    append_str_field(&mut bytes, "to", seed.to);
    append_u64_field(&mut bytes, "amount", seed.amount);
    append_str_field(&mut bytes, "memo", seed.memo);
    Ok(hash_hex(domain, &bytes))
}

fn hash_turnstile_seed(domain: &str, seed: &TurnstileSeed<'_>) -> Result<String, ShieldedError> {
    let mut bytes = Vec::new();
    append_str_field(&mut bytes, "kind", seed.kind);
    append_str_field(&mut bytes, "owner", seed.owner);
    append_str_field(&mut bytes, "asset_id", seed.asset_id);
    append_u64_field(&mut bytes, "amount", seed.amount);
    append_str_field(&mut bytes, "note_id", seed.note_id);
    append_str_field(&mut bytes, "source_pool", seed.source_pool);
    append_str_field(&mut bytes, "target_pool", seed.target_pool);
    Ok(hash_hex(domain, &bytes))
}

fn hash_notes(domain: &str, notes: &[ShieldedNote]) -> Result<String, ShieldedError> {
    let mut bytes = Vec::new();
    append_u64_field(&mut bytes, "note_count", notes.len() as u64);
    for note in notes {
        append_str_field(&mut bytes, "note.note_id", &note.note_id);
        append_str_field(&mut bytes, "note.commitment", &note.commitment);
        append_u64_field(&mut bytes, "note.position", note.position);
        append_str_field(&mut bytes, "note.owner", &note.owner);
        append_str_field(&mut bytes, "note.asset_id", &note.asset_id);
        append_u64_field(&mut bytes, "note.value", note.value);
        append_str_field(&mut bytes, "note.rho", &note.rho);
        append_str_field(&mut bytes, "note.memo", &note.memo);
        append_str_field(&mut bytes, "note.created_by", &note.created_by);
    }
    Ok(hash_hex(domain, &bytes))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_creator_changes_note_identity() {
        let mut first = ShieldedState::empty();
        let mut second = ShieldedState::empty();
        let first_note =
            mint_debug_note_with_creator(&mut first, "alice", "POSTFIAT", 10, "memo", "creator-a")
                .expect("first note");
        let second_note =
            mint_debug_note_with_creator(&mut second, "alice", "POSTFIAT", 10, "memo", "creator-b")
                .expect("second note");

        assert_ne!(first_note.note_id, second_note.note_id);
        assert_ne!(
            debug_nullifier(&first_note.note_id),
            debug_nullifier(&second_note.note_id)
        );
        let bad_creator = mint_debug_note_with_creator(
            &mut ShieldedState::empty(),
            "alice",
            "POSTFIAT",
            10,
            "memo",
            " creator-a",
        )
        .expect_err("creator provenance should reject boundary whitespace");
        assert_eq!(bad_creator.code(), "boundary_whitespace");
        let control_creator = mint_debug_note_with_creator(
            &mut ShieldedState::empty(),
            "alice",
            "POSTFIAT",
            10,
            "memo",
            "creator\na",
        )
        .expect_err("creator provenance should reject control characters");
        assert_eq!(control_creator.code(), "control_character");
    }

    #[test]
    fn note_positions_are_monotonic_across_state_compaction() {
        let mut state = ShieldedState::empty();
        let first =
            mint_debug_note(&mut state, "alice", "POSTFIAT", 10, "first").expect("first note");
        state.notes.clear();

        let second =
            mint_debug_note(&mut state, "alice", "POSTFIAT", 10, "second").expect("second note");

        assert_eq!(first.position, 0);
        assert_eq!(second.position, 1);
        assert_ne!(first.note_id, second.note_id);
        assert_eq!(state.next_note_position, 2);
    }

    #[test]
    fn note_commitment_has_canonical_golden_vector() {
        assert_eq!(
            debug_note_commitment("alice", "POSTFIAT", 10, "memo", 0, "creator")
                .expect("commitment"),
            "342fd6c2e61922a27c28dcb33d5691c89dc59f26b6c75f1e483a480e7a7a039b33297228e451b0c68914ff0046dc2f95"
        );
    }

    #[test]
    fn mint_scan_disclose_and_spend() {
        let mut state = ShieldedState::empty();
        let minted =
            mint_debug_note(&mut state, "alice", "POSTFIAT", 250, "initial").expect("mint note");

        let initial_turnstile = turnstile_summary(&state);
        assert_eq!(initial_turnstile.event_count, 1);
        assert_eq!(initial_turnstile.bootstrap_deposit_total, 250);
        assert_eq!(initial_turnstile.migration_total, 0);
        assert_eq!(
            initial_turnstile.events[0].kind,
            TURNSTILE_KIND_BOOTSTRAP_DEPOSIT
        );

        assert_eq!(note_tree_root(&state).expect("note root").len(), 96);
        assert_eq!(scan_owner(&state, "alice"), vec![minted.clone()]);

        let migration = migrate_debug_note(
            &mut state,
            &minted.note_id,
            "debug-shielded-pool-v2",
            "upgrade",
        )
        .expect("record migration");
        assert_eq!(migration.kind, TURNSTILE_KIND_POOL_MIGRATION);
        let migrated_turnstile = turnstile_summary(&state);
        assert_eq!(migrated_turnstile.event_count, 2);
        assert_eq!(migrated_turnstile.migration_total, 250);
        assert_eq!(
            migrate_debug_note(
                &mut state,
                &minted.note_id,
                "debug-shielded-pool-v2",
                "upgrade"
            )
            .expect_err("duplicate migration")
            .code(),
            "spent_note"
        );

        let disclosure = disclose_note(&state, &minted.note_id).expect("disclose");
        assert_eq!(disclosure.note, minted.clone());
        assert!(disclosure.spent);

        let migrated_spend = spend_debug_note(&mut state, &minted.note_id, "bob", 1, "replay")
            .expect_err("migrated note cannot be spent");
        assert_eq!(migrated_spend.code(), "duplicate_nullifier");

        let spend_source = mint_debug_note(&mut state, "alice", "POSTFIAT", 175, "spend-source")
            .expect("mint spend source");
        let spend = spend_debug_note(&mut state, &spend_source.note_id, "bob", 125, "payment")
            .expect("spend");
        assert_eq!(spend.outputs.len(), 2);
        assert!(scan_owner(&state, "alice")
            .iter()
            .all(|note| note.note_id != spend_source.note_id));
        assert_eq!(scan_owner(&state, "bob").len(), 1);

        let spent = disclose_note(&state, &spend_source.note_id).expect("disclose spent");
        assert!(spent.spent);
        assert_eq!(spent.nullifier, spend.nullifier);

        let duplicate = spend_debug_note(&mut state, &spend_source.note_id, "carol", 1, "replay")
            .expect_err("duplicate must fail");
        assert_eq!(duplicate.code(), "duplicate_nullifier");
        assert_eq!(
            migrate_debug_note(
                &mut state,
                &minted.note_id,
                "debug-shielded-pool-v3",
                "late"
            )
            .expect_err("spent note migration")
            .code(),
            "spent_note"
        );
    }

    #[test]
    fn rejects_bad_values() {
        let mut state = ShieldedState::empty();
        assert_eq!(
            mint_debug_note(&mut state, "alice", "POSTFIAT", 0, "")
                .expect_err("zero mint")
                .code(),
            "zero_value"
        );

        let minted = mint_debug_note(&mut state, "alice", "POSTFIAT", 10, "").expect("mint note");
        assert_eq!(
            spend_debug_note(&mut state, &minted.note_id, "bob", 11, "")
                .expect_err("overspend")
                .code(),
            "insufficient_note_value"
        );

        assert_eq!(
            mint_debug_note(&mut ShieldedState::empty(), " alice", "POSTFIAT", 10, "")
                .expect_err("owner boundary whitespace")
                .code(),
            "boundary_whitespace"
        );
        assert_eq!(
            mint_debug_note(&mut ShieldedState::empty(), "ali\nce", "POSTFIAT", 10, "")
                .expect_err("owner control character")
                .code(),
            "control_character"
        );
        assert_eq!(
            mint_debug_note(&mut ShieldedState::empty(), "alice", " POSTFIAT", 10, "")
                .expect_err("asset boundary whitespace")
                .code(),
            "boundary_whitespace"
        );
        assert_eq!(
            mint_debug_note(&mut ShieldedState::empty(), "alice", "POST\nFIAT", 10, "")
                .expect_err("asset control character")
                .code(),
            "control_character"
        );
        assert_eq!(
            spend_debug_note(&mut state, &minted.note_id, " bob", 1, "")
                .expect_err("recipient boundary whitespace")
                .code(),
            "boundary_whitespace"
        );
        assert_eq!(
            migrate_debug_note(&mut state, &minted.note_id, " debug-shielded-pool-v2", "")
                .expect_err("pool boundary whitespace")
                .code(),
            "boundary_whitespace"
        );
        assert_eq!(
            debug_note_commitment("alice", "POSTFIAT", 0, "", 0, "creator")
                .expect_err("commitment helper rejects zero value")
                .code(),
            "zero_value"
        );
        assert_eq!(
            debug_turnstile_event_id(
                TURNSTILE_KIND_POOL_MIGRATION,
                "alice",
                "POSTFIAT",
                10,
                "note\nid",
                DEBUG_SHIELDED_POOL_ID,
                "debug-shielded-pool-v2",
            )
            .expect_err("event helper rejects control characters")
            .code(),
            "control_character"
        );
    }
}
