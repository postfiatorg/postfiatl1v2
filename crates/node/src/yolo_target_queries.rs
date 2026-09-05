use super::*;

/// Public committed target metadata plus the existing chain finality evidence.
/// A missing receipt is distinct from an unfinalized transaction.
pub fn yolo_target_receipt_query(
    options: NodeOptions,
    registration_id: &str,
) -> io::Result<serde_json::Value> {
    if registration_id.len() != 96
        || !registration_id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "registration id must be lowercase SHA3-384",
        ));
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let registration = ledger
        .yolo_target_registrations
        .iter()
        .find(|r| r.registration_id == registration_id);
    let receipt = ledger
        .yolo_target_receipts
        .iter()
        .find(|r| r.registration_id == registration_id);
    let finality = receipt
        .map(|record| {
            tx_finality(TxFinalityQueryOptions {
                data_dir: options.data_dir.clone(),
                tx_id: record.transaction_hash.clone(),
                audit_block_log: false,
            })
        })
        .transpose()?;
    Ok(serde_json::json!({
        "schema": "postfiat.yolo.target_receipt_query.v1", "chainId": genesis.chain_id,
        "genesisHash": genesis_hash(&genesis), "registrationId": registration_id,
        "registration": registration, "receipt": receipt, "finality": finality,
    }))
}
