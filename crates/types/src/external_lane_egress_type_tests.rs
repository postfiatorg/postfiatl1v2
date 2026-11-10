const EXTERNAL_LANE_ROUTE_PROFILE_VECTORS: &str =
    include_str!("../testdata/external-lane-route-profile-vectors.json");
const EXTERNAL_LANE_EXPORT_FOUNDRY_FIXTURE_PATH: &str =
    "../ethereum-contracts/test/fixtures/ExternalLaneExportFixture.sol";
const EXTERNAL_LANE_EXPORT_FOUNDRY_FIXTURE: &str =
    include_str!("../../ethereum-contracts/test/fixtures/ExternalLaneExportFixture.sol");

fn keccak_commitment_hex(hex48: &str) -> String {
    format!(
        "0x{}",
        bytes_to_hex(&Keccak256::digest(pfusdc_decode_hex(hex48).expect("hex")))
    )
}

#[test]
fn external_lane_route_profile_and_asset_id_match_the_shared_vectors() {
    let vectors: serde_json::Value =
        serde_json::from_str(EXTERNAL_LANE_ROUTE_PROFILE_VECTORS).expect("vector file");
    for (field, domain) in [
        ("asset_id", EXTERNAL_LANE_ASSET_ID_DOMAIN_V1),
        (
            "route_profile_hash",
            EXTERNAL_LANE_ROUTE_PROFILE_HASH_DOMAIN_V1,
        ),
        (
            "route_profile_schema",
            EXTERNAL_LANE_ROUTE_PROFILE_SCHEMA_V1,
        ),
    ] {
        assert_eq!(vectors["domains"][field], domain);
    }
    let vectors = vectors["vectors"].as_array().expect("vectors");
    assert_eq!(vectors.len(), 2);
    for vector in vectors {
        let lane: GenesisExternalLane =
            serde_json::from_value(vector["lane"].clone()).expect("lane");
        let chain_id = vector["pftl_chain_id"].as_str().expect("chain id");
        let asset_id = lane.pft_asset_id(chain_id).expect("asset id");
        let profile = lane.route_profile(chain_id).expect("profile");
        assert_eq!(asset_id, vector["asset_id"]);
        assert_eq!(profile.asset_id, asset_id);
        assert_eq!(
            profile.canonical_preimage(),
            vector["route_profile_preimage"]
        );
        assert_eq!(profile.profile_hash(), vector["route_profile_hash"]);
        assert_eq!(u64::from(profile.route_epoch), vector["route_epoch"]);
        assert_eq!(
            keccak_commitment_hex(&asset_id),
            vector["verifier_asset_id_commitment"]
        );
        assert_eq!(
            keccak_commitment_hex(&profile.profile_hash()),
            vector["verifier_route_profile_hash_commitment"]
        );
    }
    // The rehearsal lane of 2026-11-06, as its genesis file pins it.
    let rehearsal: Genesis = serde_json::from_str(include_str!(
        "../../../docs/review/base-sepolia-lock-rehearsal-20261106/genesis.json"
    ))
    .expect("rehearsal genesis");
    let lane = &rehearsal.external_lanes.as_ref().expect("lanes")[0];
    assert_eq!(lane.policy.chain_id, EXTERNAL_LANE_BASE_SEPOLIA_CHAIN_ID);
    assert_eq!(
        lane.pft_asset_id(&rehearsal.chain_id).expect("asset id"),
        vectors[0]["asset_id"]
    );
}

#[test]
fn external_lane_route_profile_and_asset_id_bind_each_genesis_field() {
    let lane = sample_external_lane("base-erc20", 1_000);
    let asset_id = lane.pft_asset_id("postfiat-local").expect("asset id");
    let hash = lane
        .route_profile("postfiat-local")
        .expect("profile")
        .profile_hash();
    let changes: [fn(&mut GenesisExternalLane); 6] = [
        |lane| lane.id = "base-erc21".to_string(),
        |lane| lane.policy.chain_id = EXTERNAL_LANE_BASE_SEPOLIA_CHAIN_ID,
        |lane| lane.policy.lock_contract_address = format!("0x{}", "1b".repeat(20)),
        |lane| lane.policy.token_address = format!("0x{}", "2c".repeat(20)),
        |lane| lane.policy.lock_runtime_code_hash = format!("0x{}", "4e".repeat(32)),
        |lane| lane.policy.token_runtime_code_hash = format!("0x{}", "3d".repeat(32)),
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut changed = lane.clone();
        change(&mut changed);
        let profile = changed.route_profile("postfiat-local").expect("profile");
        assert_ne!(profile.profile_hash(), hash, "change {index}");
        // The code hashes are route fields, not asset fields.
        assert_eq!(profile.asset_id == asset_id, index >= 4, "change {index}");
    }
    assert_ne!(
        lane.pft_asset_id("postfiat-other").expect("asset id"),
        asset_id
    );
    // Release-side fields are not part of either value.
    let mut caps = lane.clone();
    caps.release_cap_bps = 100;
    caps.policy.exports_enabled = true;
    assert_eq!(
        caps.route_profile("postfiat-local")
            .expect("profile")
            .profile_hash(),
        hash
    );
    assert!(lane.pft_asset_id("bad\nchain").is_err());
}

fn sample_egress_inputs() -> (
    GenesisExternalLane,
    ExternalLaneExportPacket,
    ExternalLaneExportCheckpointV1,
) {
    let vectors: serde_json::Value =
        serde_json::from_str(EXTERNAL_LANE_ROUTE_PROFILE_VECTORS).expect("vector file");
    let mut lane: GenesisExternalLane =
        serde_json::from_value(vectors["vectors"][0]["lane"].clone()).expect("lane");
    lane.policy.exports_enabled = true;
    let packet = ExternalLaneExportPacket {
        lane_id: lane.id.clone(),
        export_nonce: 3,
        sender: "pftl-sender".to_string(),
        base_recipient: format!("0x{}", "2c".repeat(20)),
        amount_atoms: 1_000_000,
        exported_at_height: 42,
        tx_id: "ab".repeat(48),
    };
    let checkpoint = ExternalLaneExportCheckpointV1 {
        pftl_chain_id: vectors["vectors"][0]["pftl_chain_id"]
            .as_str()
            .expect("chain id")
            .to_string(),
        pftl_genesis_hash: "11".repeat(48),
        pftl_protocol_version: 1,
        prior_checkpoint_block_id: "10".repeat(48),
        committee_epoch: 1,
        committee_root: "41".repeat(48),
        committee_transition_commitment: String::new(),
        finalized_block_height: 50,
        finalized_block_view: 3,
        finalized_block_id: "50".repeat(48),
        finalized_parent_block_id: "4f".repeat(48),
        finalized_state_root: "5a".repeat(48),
        export_history_root: external_lane_export_history_root(None, &packet).expect("root"),
    };
    (lane, packet, checkpoint)
}

#[test]
fn external_lane_export_maps_to_every_egress_public_value() {
    let (lane, packet, checkpoint) = sample_egress_inputs();
    let chain_id = checkpoint.pftl_chain_id.as_str();
    let values =
        external_lane_export_egress_public_values(&lane, &packet, &checkpoint).expect("values");
    let route = lane.route_profile(chain_id).expect("route");
    let strip = |value: &str| value.trim_start_matches("0x").to_string();
    assert_eq!(values.schema, PFUSDC_EGRESS_PUBLIC_VALUES_SCHEMA_V1);
    assert_eq!(values.proof_program_version, 1);
    assert_eq!(values.pftl_chain_id, chain_id);
    assert_eq!(values.route_profile_hash, route.profile_hash());
    assert_eq!(values.route_epoch, 1);
    assert_eq!(
        values.asset_id,
        lane.pft_asset_id(chain_id).expect("asset id")
    );
    assert_eq!(
        values.resulting_checkpoint_block_id,
        checkpoint.finalized_block_id
    );
    assert_eq!(
        values.bridge_exit_root,
        strip(&checkpoint.export_history_root)
    );
    assert_eq!(values.exit_leaf_index, packet.export_nonce);
    let digest = bytes_to_hex(&packet.digest().expect("digest"));
    assert_eq!(values.exit_leaf_commitment, digest);
    assert_eq!(values.withdrawal_packet_hash, digest);
    assert_eq!(values.accepted_receipt_id, packet.tx_id);
    assert_eq!(values.accepted_receipt_code, "accepted");
    assert_eq!(
        values.burn_tx_id,
        bytes_to_hex(&packet.burn_tx_commitment(chain_id))
    );
    assert_eq!(
        values.withdrawal_id,
        bytes_to_hex(
            &packet
                .withdrawal_id_commitment(chain_id)
                .expect("withdrawal id")
        )
    );
    assert_eq!(
        values.source_bucket_id,
        bytes_to_hex(&lane.route_config_digest().expect("route config"))
    );
    assert_eq!(values.destination_hash, "00".repeat(48));
    assert_eq!(values.evidence_root, "00".repeat(48));
    assert_eq!(values.amount_atoms, packet.amount_atoms);
    assert_eq!(values.recipient, packet.base_recipient);
    assert_eq!(
        values.withdrawal_finalized_height,
        packet.exported_at_height
    );
    assert_eq!(
        values.arbitrum_chain_id,
        EXTERNAL_LANE_BASE_SEPOLIA_CHAIN_ID
    );
    assert_eq!(values.vault_address, lane.policy.lock_contract_address);
    assert_eq!(
        values.vault_runtime_code_hash,
        strip(&lane.policy.lock_runtime_code_hash)
    );
    assert_eq!(values.token_address, lane.policy.token_address);
    assert_eq!(
        values.token_runtime_code_hash,
        strip(&lane.policy.token_runtime_code_hash)
    );
    assert_eq!(
        values.withdrawal_packet_digest,
        external_lane_export_evm_digest(&packet).expect("evm digest")
    );
    assert_eq!(
        values.proof_nullifier,
        pfusdc_egress_proof_nullifier_v1(
            1,
            &values.burn_tx_id,
            &values.withdrawal_id,
            &values.finalized_block_id
        )
        .expect("nullifier")
    );
    let bytes = external_lane_export_egress_public_values_bytes(&lane, &packet, &checkpoint)
        .expect("bytes");
    assert_eq!(
        bytes,
        values.canonical_bytes_without_commitment().expect("bytes")
    );
    assert!(bytes.starts_with(b"PFTL-PFUSDC-TIER4"));
}

#[test]
fn external_lane_export_public_values_refuse_what_the_lane_did_not_export() {
    let (lane, packet, checkpoint) = sample_egress_inputs();
    let mut disabled = lane.clone();
    disabled.policy.exports_enabled = false;
    assert!(external_lane_export_egress_public_values(&disabled, &packet, &checkpoint).is_err());
    let mut other = packet.clone();
    other.lane_id = "base-other".to_string();
    assert!(external_lane_export_egress_public_values(&lane, &other, &checkpoint).is_err());
    let mut early = checkpoint.clone();
    early.finalized_block_height = packet.exported_at_height - 1;
    assert!(external_lane_export_egress_public_values(&lane, &packet, &early).is_err());
    let mut text_id = packet.clone();
    text_id.tx_id = "tx-0".to_string();
    assert!(external_lane_export_egress_public_values(&lane, &text_id, &checkpoint).is_err());
    let mut root = checkpoint.clone();
    root.export_history_root = strip_lane_prefix(&root.export_history_root);
    assert!(external_lane_export_egress_public_values(&lane, &packet, &root).is_err());
}

fn strip_lane_prefix(value: &str) -> String {
    value.trim_start_matches("0x").to_string()
}

fn external_lane_export_foundry_fixture() -> String {
    let (lane, packet, checkpoint) = sample_egress_inputs();
    let values =
        external_lane_export_egress_public_values(&lane, &packet, &checkpoint).expect("values");
    solidity_lane_fixture(
        "/// Written by the Rust test `external_lane_export_public_values_match_the_foundry_fixture`\n\
         /// (crates/types/src/external_lane_egress_type_tests.rs): the egress public values of one\n\
         /// export packet on the Base Sepolia rehearsal lane, and the values they carry.\n\
         /// Regenerate with EXTERNAL_LANE_WRITE_FIXTURE=1.\nlibrary ExternalLaneExportFixture {\n",
        &values,
        &[],
    )
}

/// A Solidity library of the public values, the values they carry and any
/// extra named byte strings.
fn solidity_lane_fixture(
    header: &str,
    values: &PfUsdcEgressPublicValuesV1,
    extra: &[(&str, Vec<u8>)],
) -> String {
    let bytes = values.canonical_bytes_without_commitment().expect("bytes");
    let raw =
        |name: &str, value: &str| format!("    bytes internal constant {name} = hex\"{value}\";\n");
    let word = |kind: &str, name: &str, value: &str| {
        format!("    {kind} internal constant {name} = {kind}(hex\"{value}\");\n")
    };
    let number = |kind: &str, name: &str, value: u64| {
        format!("    {kind} internal constant {name} = {value};\n")
    };
    let mut out = format!(
        "// SPDX-License-Identifier: MIT OR Apache-2.0\npragma solidity ^0.8.24;\n\n{header}"
    );
    out.push_str(&raw("PUBLIC_VALUES", &bytes_to_hex(&bytes)));
    out.push_str(&format!(
        "    string internal constant PFTL_CHAIN_ID = \"{}\";\n",
        values.pftl_chain_id
    ));
    out.push_str(&raw("PFTL_GENESIS_HASH", &values.pftl_genesis_hash));
    out.push_str(&number(
        "uint32",
        "PFTL_PROTOCOL_VERSION",
        u64::from(values.pftl_protocol_version),
    ));
    out.push_str(&raw("ROUTE_PROFILE_HASH", &values.route_profile_hash));
    out.push_str(&number("uint64", "ROUTE_EPOCH", values.route_epoch));
    out.push_str(&raw("PRIOR_CHECKPOINT", &values.prior_checkpoint_block_id));
    out.push_str(&raw(
        "RESULTING_CHECKPOINT",
        &values.resulting_checkpoint_block_id,
    ));
    out.push_str(&raw("COMMITTEE_ROOT", &values.committee_root));
    out.push_str(&number(
        "uint64",
        "FINALIZED_HEIGHT",
        values.finalized_block_height,
    ));
    out.push_str(&raw("EXPORT_HISTORY_ROOT", &values.bridge_exit_root));
    out.push_str(&raw("TX_ID", &values.accepted_receipt_id));
    out.push_str(&raw("ASSET_ID", &values.asset_id));
    out.push_str(&raw("BURN_TX_ID", &values.burn_tx_id));
    out.push_str(&raw("WITHDRAWAL_ID", &values.withdrawal_id));
    out.push_str(&number("uint64", "AMOUNT", values.amount_atoms));
    out.push_str(&word(
        "bytes20",
        "RECIPIENT",
        &strip_lane_prefix(&values.recipient),
    ));
    out.push_str(&number(
        "uint64",
        "EXPORTED_AT_HEIGHT",
        values.withdrawal_finalized_height,
    ));
    out.push_str(&number(
        "uint64",
        "SOURCE_CHAIN_ID",
        values.arbitrum_chain_id,
    ));
    out.push_str(&word(
        "bytes20",
        "LOCK",
        &strip_lane_prefix(&values.vault_address),
    ));
    out.push_str(&word(
        "bytes32",
        "LOCK_CODE_HASH",
        &values.vault_runtime_code_hash,
    ));
    out.push_str(&word(
        "bytes20",
        "TOKEN",
        &strip_lane_prefix(&values.token_address),
    ));
    out.push_str(&word(
        "bytes32",
        "TOKEN_CODE_HASH",
        &values.token_runtime_code_hash,
    ));
    out.push_str(&word(
        "bytes32",
        "PACKET_EVM_DIGEST",
        &values.withdrawal_packet_digest,
    ));
    out.push_str(&raw("PACKET_DIGEST", &values.withdrawal_packet_hash));
    out.push_str(&word("bytes32", "PROOF_NULLIFIER", &values.proof_nullifier));
    for (name, value) in extra {
        out.push_str(&raw(name, &bytes_to_hex(value)));
    }
    out.push_str("}\n");
    out
}

#[test]
fn external_lane_export_public_values_match_the_foundry_fixture() {
    let fixture = external_lane_export_foundry_fixture();
    if std::env::var_os("EXTERNAL_LANE_WRITE_FIXTURE").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(EXTERNAL_LANE_EXPORT_FOUNDRY_FIXTURE_PATH);
        std::fs::write(path, &fixture).expect("write fixture");
        return;
    }
    assert_eq!(EXTERNAL_LANE_EXPORT_FOUNDRY_FIXTURE, fixture);
}

const EXTERNAL_LANE_LOCK_RELEASE_FIXTURE_PATH: &str =
    "../pft-token-contracts/test/fixtures/ExternalLaneLockReleaseFixture.sol";
const EXTERNAL_LANE_LOCK_RELEASE_FIXTURE: &str =
    include_str!("../../pft-token-contracts/test/fixtures/ExternalLaneLockReleaseFixture.sol");
/// What `crates/pft-token-contracts/test/ExternalLaneLockRelease.t.sol`
/// deploys on its local chain: the lock, its runtime code hash, the token and
/// its runtime code hash. That test checks them and prints the new values when
/// a contract's bytecode changes.
const EXTERNAL_LANE_LOCAL_LOCK: &str = "0xf62849f9a0b5bf2913b396098f7c7019b51a820a";
const EXTERNAL_LANE_LOCAL_LOCK_CODE_HASH: &str =
    "0x708d08249e4a466cd5d099f502af2e0d9052fecb2420587e482491d5f02c78a5";
const EXTERNAL_LANE_LOCAL_TOKEN: &str = "0x5615deb798bb3e4dfa0139dfa1b3d433cc23b72f";
const EXTERNAL_LANE_LOCAL_TOKEN_CODE_HASH: &str =
    "0xd54e0a6052fbc19c3551706426ab0f553d0ab66f5d6044ec9aeebcd4be83c5b5";

fn external_lane_lock_release_fixture() -> String {
    let (mut lane, mut packet, mut checkpoint) = sample_egress_inputs();
    lane.policy.lock_contract_address = EXTERNAL_LANE_LOCAL_LOCK.to_string();
    lane.policy.lock_runtime_code_hash = EXTERNAL_LANE_LOCAL_LOCK_CODE_HASH.to_string();
    lane.policy.token_address = EXTERNAL_LANE_LOCAL_TOKEN.to_string();
    lane.policy.token_runtime_code_hash = EXTERNAL_LANE_LOCAL_TOKEN_CODE_HASH.to_string();
    packet.lane_id = lane.id.clone();
    checkpoint.export_history_root =
        external_lane_export_history_root(None, &packet).expect("root");
    let values =
        external_lane_export_egress_public_values(&lane, &packet, &checkpoint).expect("values");
    let variant = |change: fn(&mut PfUsdcEgressPublicValuesV1)| {
        let mut changed = values.clone();
        change(&mut changed);
        changed
            .canonical_bytes_without_commitment()
            .expect("variant")
    };
    solidity_lane_fixture(
        "/// Written by the Rust test `external_lane_lock_release_matches_the_foundry_fixture`\n\
         /// (crates/types/src/external_lane_egress_type_tests.rs): the egress public values of one\n\
         /// export packet on a lane whose lock and token are the ones\n\
         /// test/ExternalLaneLockRelease.t.sol deploys, and three copies that change only the\n\
         /// vault, the token or the chain id. Regenerate with EXTERNAL_LANE_WRITE_FIXTURE=1.\n\
         library ExternalLaneLockReleaseFixture {\n",
        &values,
        &[
            (
                "WRONG_VAULT",
                variant(|values| values.vault_address = format!("0x{}", "1a".repeat(20))),
            ),
            (
                "WRONG_TOKEN",
                variant(|values| values.token_address = format!("0x{}", "2b".repeat(20))),
            ),
            (
                "WRONG_CHAIN_ID",
                variant(|values| values.arbitrum_chain_id = EXTERNAL_LANE_BASE_MAINNET_CHAIN_ID),
            ),
        ],
    )
}

#[test]
fn external_lane_lock_release_matches_the_foundry_fixture() {
    let fixture = external_lane_lock_release_fixture();
    if std::env::var_os("EXTERNAL_LANE_WRITE_FIXTURE").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(EXTERNAL_LANE_LOCK_RELEASE_FIXTURE_PATH);
        std::fs::write(path, &fixture).expect("write fixture");
        return;
    }
    assert_eq!(EXTERNAL_LANE_LOCK_RELEASE_FIXTURE, fixture);
}

/// Execution names an export by `asset_transaction_tx_id` (SHA3-384, 96
/// lower-case hex digits), the form the accepted-receipt field needs; the
/// shared packet vectors use it, so each maps to public values.
#[test]
fn external_lane_export_vectors_carry_execution_transaction_ids() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../testdata/external-lane-export-packet-vectors.json"
    ))
    .expect("vector file");
    let (lane, _, checkpoint) = sample_egress_inputs();
    for vector in vectors["vectors"].as_array().expect("vectors") {
        let mut packet: ExternalLaneExportPacket =
            serde_json::from_value(vector["packet"].clone()).expect("packet");
        validate_lower_hex_len("export tx_id", &packet.tx_id, 96).expect("tx id");
        packet.exported_at_height = checkpoint.finalized_block_height;
        let mut checkpoint = checkpoint.clone();
        checkpoint.export_history_root =
            external_lane_export_history_root(None, &packet).expect("root");
        let values =
            external_lane_export_egress_public_values(&lane, &packet, &checkpoint).expect("values");
        assert_eq!(values.accepted_receipt_id, packet.tx_id);
    }
}
