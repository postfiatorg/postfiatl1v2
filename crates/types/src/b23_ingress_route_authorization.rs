use crate::{hash_hex_domain, GovernanceAmendment, VaultBridgeRouteProfileRecordV1};

/// V1 records retain the amendment ID, but no beacon-bootstrap hash. Preserve
/// their serialized representation and authenticate the complete beacon kind
/// using that ID. The ledger anchor cannot supply the bootstrap hash: it advances
/// after each proven deposit. Governance admission already verified the signed
/// amendment; this check binds its kind to the immutable ID in the stored record.
pub(super) fn matches(
    record: &VaultBridgeRouteProfileRecordV1,
    amendment: &GovernanceAmendment,
) -> Result<bool, String> {
    let base = record.amendment_kind()?;
    if amendment.kind == base {
        // Keep historical no-bootstrap and Arc authorization byte-for-byte.
        return Ok(true);
    }
    if record.profile.verifier_kind != "sp1-groth16" || record.arc_bootstrap_hash.is_some() {
        return Ok(false);
    }
    let prefix = format!("{base}:beacon-bootstrap-v2:");
    let Some(suffix) = amendment.kind.strip_prefix(&prefix) else {
        return Ok(false);
    };
    if suffix.len() != 96
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Ok(false);
    }
    // Same canonical preimage/domain as consensus_cobalt::governance_amendment_id.
    // Keep this in types to avoid a dependency cycle; the real archived-batch
    // regression also verifies the ID using the Cobalt implementation.
    let payload = format!(
        "chain_id={}\ngenesis_hash={}\nprotocol_version={}\ninstance_id={}\ncertificate_id={}\nkind={}\nvalue={}\nsupport={}\n",
        amendment.chain_id, amendment.genesis_hash, amendment.protocol_version,
        amendment.instance_id, amendment.certificate_id, amendment.kind,
        amendment.value, amendment.support.join(","),
    );
    let bound_id = hash_hex_domain("postfiat.cobalt.amendment.v1", payload.as_bytes());
    Ok(bound_id == amendment.amendment_id && bound_id == record.governance_amendment_id)
}
