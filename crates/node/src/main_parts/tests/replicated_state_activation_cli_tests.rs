use super::*;

#[test]
fn replicated_state_v2_activation_cli_builds_the_governed_amendment() {
    let root = std::env::temp_dir().join(format!(
        "postfiat-state-v2-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    let data_dir = root.join("node");
    let amendment_file = root.join("state-v2-amendment.json");
    let authorization_file = root.join("state-v2-authorization.json");
    let signed_amendment_file = root.join("state-v2-signed.json");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-state-v2-cli".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize state-v2 CLI fixture");

    run_cli(vec![
        "ratify-replicated-state-v2-activation-height".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--validators".to_string(),
        "validator-0".to_string(),
        "--height".to_string(),
        "7".to_string(),
        "--amendment-file".to_string(),
        amendment_file.display().to_string(),
    ])
    .expect("build state-v2 activation amendment through CLI");

    let amendment: postfiat_types::GovernanceAmendment = serde_json::from_slice(
        &std::fs::read(&amendment_file).expect("read state-v2 activation amendment"),
    )
    .expect("parse state-v2 activation amendment");
    assert_eq!(
        amendment.kind,
        postfiat_types::GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT
    );
    assert_eq!(amendment.value, 7);
    assert_eq!(amendment.validators, ["validator-0"]);
    assert_eq!(amendment.support, ["validator-0"]);
    assert!(amendment.votes.iter().all(|vote| vote.accept));

    run_cli(vec![
        "governance-authorization-sign".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--amendment-file".to_string(),
        amendment_file.display().to_string(),
        "--validator".to_string(),
        "validator-0".to_string(),
        "--validator-key-file".to_string(),
        data_dir.join(VALIDATOR_KEYS_FILE).display().to_string(),
        "--proposal-slot".to_string(),
        "1".to_string(),
        "--expires-at-height".to_string(),
        "9".to_string(),
        "--authorization-file".to_string(),
        authorization_file.display().to_string(),
    ])
    .expect("sign state-v2 activation amendment through CLI");
    run_cli(vec![
        "governance-amendment-assemble".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--amendment-file".to_string(),
        amendment_file.display().to_string(),
        "--authorization-files".to_string(),
        authorization_file.display().to_string(),
        "--proposal-slot".to_string(),
        "1".to_string(),
        "--output".to_string(),
        signed_amendment_file.display().to_string(),
    ])
    .expect("assemble state-v2 activation amendment through CLI");
    let signed: postfiat_types::GovernanceAmendment = serde_json::from_slice(
        &std::fs::read(signed_amendment_file).expect("read signed state-v2 amendment"),
    )
    .expect("parse signed state-v2 amendment");
    assert_eq!(signed.signed_authorizations.len(), 1);
    assert_eq!(signed.signed_authorizations[0].validator, "validator-0");

    std::fs::remove_dir_all(root).expect("remove state-v2 CLI fixture");
}
