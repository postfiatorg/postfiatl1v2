use super::*;

const STORAGE_VOTE_BLOCKED_AMBIGUOUS_LOCAL_STATE: &str =
    "storage_vote_blocked_ambiguous_local_state";

fn vote_blocked(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{STORAGE_VOTE_BLOCKED_AMBIGUOUS_LOCAL_STATE}: {error}"),
    )
}

/// Fail before any vote lock or signature when the activated transactional
/// store is absent, unreadable, stale, or bound to a different parent.
pub(crate) fn require_unambiguous_storage_for_vote(
    data_dir: &Path,
    vote_height: u64,
    expected_parent_hash: Option<&str>,
) -> io::Result<()> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis().map_err(vote_blocked)?;
    let governance = store.read_governance().map_err(vote_blocked)?;
    let Some(activation_height) =
        effective_storage_commitment_activation_height(&genesis, &governance)
    else {
        return Ok(());
    };
    if vote_height < activation_height {
        return Ok(());
    }

    let expected_finalized_height = vote_height
        .checked_sub(1)
        .ok_or_else(|| vote_blocked("activated storage vote height cannot be zero"))?;
    if !store
        .transactional_storage_configured()
        .map_err(vote_blocked)?
    {
        return Err(vote_blocked(
            "activated transactional store or generation pointer is missing",
        ));
    }
    let transactional = store.transactional_store().map_err(vote_blocked)?;
    let meta = transactional.meta().map_err(vote_blocked)?;
    let expected_genesis_hash = genesis_hash(&genesis);
    if meta.chain_id != genesis.chain_id
        || meta.genesis_hash != expected_genesis_hash
        || meta.protocol_version != genesis.protocol_version
        || meta.scheduled_activation_height != Some(activation_height)
        || meta.last_full_verification_height.is_none()
        || meta.finalized_height != expected_finalized_height
        || expected_parent_hash.is_some_and(|expected| meta.finalized_block_hash != expected)
    {
        return Err(vote_blocked(
            "activated transactional store does not match the exact vote parent",
        ));
    }

    if expected_finalized_height > meta.history_base_height {
        let parent = transactional
            .block(expected_finalized_height)
            .map_err(vote_blocked)?
            .ok_or_else(|| vote_blocked("activated transactional parent block is missing"))?;
        if parent.header.block_hash != meta.finalized_block_hash
            || parent.header.state_root != meta.finalized_state_root
        {
            return Err(vote_blocked(
                "activated transactional parent block conflicts with metadata",
            ));
        }
        let last_ordered = transactional
            .ordered_batch_by_ordinal(meta.ordered_batch_count)
            .map_err(vote_blocked)?
            .ok_or_else(|| vote_blocked("activated transactional ordered tip is missing"))?;
        if last_ordered != parent.header.batch_id {
            return Err(vote_blocked(
                "activated transactional ordered tip conflicts with the parent block",
            ));
        }
    }
    if transactional.ledger().map_err(vote_blocked)?.is_none()
        || transactional.governance().map_err(vote_blocked)?.is_none()
        || transactional.shielded().map_err(vote_blocked)?.is_none()
        || transactional.bridge().map_err(vote_blocked)?.is_none()
        || transactional
            .current_state_raw("validator_registry")
            .map_err(vote_blocked)?
            .is_none()
    {
        return Err(vote_blocked(
            "activated transactional current state is incomplete",
        ));
    }
    Ok(())
}
