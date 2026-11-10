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
            profile.canonical_preimage().expect("preimage"),
            vector["route_profile_preimage"]
        );
        assert_eq!(
            profile.profile_hash().expect("hash"),
            vector["route_profile_hash"]
        );
        assert_eq!(u64::from(profile.route_epoch), vector["route_epoch"]);
        assert_eq!(
            keccak_commitment_hex(&asset_id),
            vector["verifier_asset_id_commitment"]
        );
        assert_eq!(
            keccak_commitment_hex(&profile.profile_hash().expect("hash")),
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
fn external_lane_route_profile_validates_before_hashing() {
    let lane = sample_external_lane("base-erc20", 1_000);
    let profile = lane.route_profile("postfiat-local").expect("profile");
    profile.validate().expect("valid");
    let changes: [fn(&mut ExternalLaneRouteProfileV1); 8] = [
        |profile| profile.schema = "postfiat.vault_bridge.route_profile.v1".to_string(),
        |profile| profile.route_id = "Base".to_string(),
        |profile| profile.asset_id = "ab".repeat(32),
        |profile| profile.source_chain_id = 1,
        |profile| profile.vault_address = "0x1A".repeat(10),
        |profile| profile.token_address = format!("0x{}", "2b".repeat(19)),
        |profile| profile.vault_runtime_code_hash = "4d".repeat(32),
        |profile| profile.route_epoch = 0,
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut changed = profile.clone();
        change(&mut changed);
        assert!(changed.validate().is_err(), "change {index}");
        assert!(changed.profile_hash().is_err(), "change {index}");
        assert!(changed.canonical_preimage().is_err(), "change {index}");
    }
}

#[test]
fn external_lane_route_profile_and_asset_id_bind_each_genesis_field() {
    let lane = sample_external_lane("base-erc20", 1_000);
    let asset_id = lane.pft_asset_id("postfiat-local").expect("asset id");
    let hash = lane
        .route_profile("postfiat-local")
        .expect("profile")
        .profile_hash()
        .expect("hash");
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
        assert_ne!(
            profile.profile_hash().expect("hash"),
            hash,
            "change {index}"
        );
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
            .profile_hash()
            .expect("hash"),
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
        export: ExternalLaneExportInclusionV1::single(&packet, "postfiat-lane-sepolia-example")
            .expect("inclusion"),
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
    assert_eq!(
        values.route_profile_hash,
        route.profile_hash().expect("hash")
    );
    assert_eq!(values.route_epoch, 1);
    assert_eq!(
        values.asset_id,
        lane.pft_asset_id(chain_id).expect("asset id")
    );
    assert_eq!(
        values.resulting_checkpoint_block_id,
        checkpoint.finalized_block_id
    );
    // Tags 18-20 and 31 come from the export's block header root and leaf.
    assert_eq!(values.bridge_exit_root, checkpoint.export.export_root);
    assert_eq!(values.exit_leaf_index, checkpoint.export.proof.leaf_index);
    assert_eq!(
        values.exit_leaf_commitment,
        bytes_to_hex(&packet.export_root_leaf(chain_id).expect("leaf"))
    );
    assert_eq!(
        values.withdrawal_finalized_height,
        checkpoint.export.block_height
    );
    let digest = bytes_to_hex(&packet.digest().expect("digest"));
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
    for recipient in [
        lane.policy.lock_contract_address.clone(),
        lane.policy.token_address.clone(),
        format!("0x{}", "0".repeat(40)),
    ] {
        let mut changed = packet.clone();
        changed.base_recipient = recipient;
        assert!(external_lane_export_egress_public_values(&lane, &changed, &checkpoint).is_err());
    }
    let mut zero_amount = packet.clone();
    zero_amount.amount_atoms = 0;
    assert!(external_lane_export_egress_public_values(&lane, &zero_amount, &checkpoint).is_err());
    let mut zero_height = packet.clone();
    zero_height.exported_at_height = 0;
    assert!(external_lane_export_egress_public_values(&lane, &zero_height, &checkpoint).is_err());
    let mut text_id = packet.clone();
    text_id.tx_id = "tx-0".to_string();
    assert!(external_lane_export_egress_public_values(&lane, &text_id, &checkpoint).is_err());
    // The inclusion must prove this packet's leaf, in its block, under the root.
    let mut root = checkpoint.clone();
    root.export.export_root = bridge_exit_empty_root_v1();
    assert!(external_lane_export_egress_public_values(&lane, &packet, &root).is_err());
    let mut block = checkpoint.clone();
    block.export.block_height += 1;
    assert!(external_lane_export_egress_public_values(&lane, &packet, &block).is_err());
    let mut other = packet.clone();
    other.amount_atoms += 1;
    assert!(external_lane_export_egress_public_values(&lane, &other, &checkpoint).is_err());
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
    out.push_str(&raw("EXPORT_ROOT", &values.bridge_exit_root));
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
    checkpoint.export = ExternalLaneExportInclusionV1::single(&packet, &checkpoint.pftl_chain_id)
        .expect("inclusion");
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
        checkpoint.export =
            ExternalLaneExportInclusionV1::single(&packet, &checkpoint.pftl_chain_id)
                .expect("inclusion");
        let values =
            external_lane_export_egress_public_values(&lane, &packet, &checkpoint).expect("values");
        assert_eq!(values.accepted_receipt_id, packet.tx_id);
    }
}

#[test]
fn external_lane_export_root_matches_the_shared_vectors() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../testdata/external-lane-export-root-vectors.json"
    ))
    .expect("vector file");
    assert_eq!(
        vectors["domains"]["leaf"],
        EXTERNAL_LANE_EXPORT_ROOT_LEAF_DOMAIN_V1
    );
    let chain_id = vectors["chain_id"].as_str().expect("chain id");
    let leaves = vectors["packets"]
        .as_array()
        .expect("packets")
        .iter()
        .map(|packet| {
            serde_json::from_value::<ExternalLaneExportPacket>(packet.clone())
                .expect("packet")
                .export_root_leaf(chain_id)
                .expect("leaf")
        })
        .collect::<Vec<_>>();
    assert_eq!(leaves.len(), 3);
    for (leaf, expected) in leaves
        .iter()
        .zip(vectors["leaves"].as_array().expect("leaves"))
    {
        assert_eq!(bytes_to_hex(leaf), *expected);
    }
    let root = external_lane_export_root_v1(&leaves).expect("root");
    assert_eq!(root, vectors["root"]);
    for (index, expected) in vectors["proofs"]
        .as_array()
        .expect("proofs")
        .iter()
        .enumerate()
    {
        let proof = external_lane_export_root_proof_v1(&leaves, index).expect("proof");
        assert_eq!(proof.leaf_index, expected["leaf_index"]);
        assert_eq!(proof.leaf_count, 3);
        assert_eq!(serde_json::json!(proof.siblings), expected["siblings"]);
        verify_external_lane_export_root_proof_v1(&root, &proof).expect("member");
        let mut moved = proof.clone();
        moved.leaf_index = (proof.leaf_index + 1) % 3;
        assert!(verify_external_lane_export_root_proof_v1(&root, &moved).is_err());
        let mut other = proof.clone();
        other.leaf = bytes_to_hex(&leaves[(index + 1) % 3]);
        assert!(verify_external_lane_export_root_proof_v1(&root, &other).is_err());
    }
    // A block without exports carries the tree's empty value.
    assert_eq!(
        external_lane_export_root_v1(&[]).expect("empty"),
        vectors["empty_root"]
    );
    assert_eq!(vectors["empty_root"], bridge_exit_empty_root_v1());
    // The leaf binds the chain id, the lane, the nonce and the packet.
    let packet: ExternalLaneExportPacket =
        serde_json::from_value(vectors["packets"][0].clone()).expect("packet");
    let mut other_lane = packet.clone();
    other_lane.lane_id = "base-other".to_string();
    let mut other_nonce = packet.clone();
    other_nonce.export_nonce = 9;
    let mut other_amount = packet.clone();
    other_amount.amount_atoms += 1;
    for changed in [
        packet.export_root_leaf("postfiat-other").expect("leaf"),
        other_lane.export_root_leaf(chain_id).expect("leaf"),
        other_nonce.export_root_leaf(chain_id).expect("leaf"),
        other_amount.export_root_leaf(chain_id).expect("leaf"),
    ] {
        assert_ne!(changed, leaves[0]);
    }
}

#[test]
fn external_lane_export_root_activation_reads_genesis_then_governance() {
    let mut genesis = Genesis::new("postfiat-local");
    let mut governance = GovernanceState::new(4);
    assert!(!external_lane_export_root_active_at(
        &genesis,
        &governance,
        u64::MAX
    ));
    assert!(!genesis
        .to_json()
        .expect("json")
        .contains("external_lane_export_root"));
    governance.amendments.push(
        serde_json::from_value(serde_json::json!({
            "amendment_id": "a", "chain_id": "postfiat-local", "genesis_hash": "g",
            "protocol_version": 1, "instance_id": "i", "proposal_id": "p",
            "certificate_id": "c", "proposer": "v", "validators": [], "quorum": 1,
            "kind": GOVERNANCE_KIND_EXTERNAL_LANE_EXPORT_ROOT_ACTIVATION_HEIGHT,
            "value": 20, "support": [], "votes": []
        }))
        .expect("amendment"),
    );
    assert!(!external_lane_export_root_active_at(
        &genesis,
        &governance,
        19
    ));
    assert!(external_lane_export_root_active_at(
        &genesis,
        &governance,
        20
    ));
    governance.amendments[0].paused = true;
    assert!(!external_lane_export_root_active_at(
        &genesis,
        &governance,
        20
    ));
    genesis.external_lane_export_root_activation_height = Some(1);
    assert!(external_lane_export_root_active_at(
        &genesis,
        &governance,
        1
    ));
    assert!(genesis
        .to_json()
        .expect("json")
        .contains("external_lane_export_root_activation_height"));
}

#[test]
fn external_lane_verifier_pins_come_from_one_genesis() {
    let genesis: Genesis = serde_json::from_str(include_str!(
        "../../../docs/review/base-sepolia-lock-rehearsal-20261106/genesis.json"
    ))
    .expect("rehearsal genesis");
    let pins = genesis
        .external_lane_verifier_pins("base-erc20")
        .expect("pins");
    let vectors: serde_json::Value =
        serde_json::from_str(EXTERNAL_LANE_ROUTE_PROFILE_VECTORS).expect("vector file");
    assert_eq!(pins.pftl_chain_id, genesis.chain_id);
    assert_eq!(pins.asset_id, vectors["vectors"][0]["asset_id"]);
    assert_eq!(
        pins.route_profile_hash,
        vectors["vectors"][0]["route_profile_hash"]
    );
    assert_eq!(
        pins.asset_id_commitment,
        vectors["vectors"][0]["verifier_asset_id_commitment"]
    );
    assert_eq!(
        pins.route_profile_hash_commitment,
        vectors["vectors"][0]["verifier_route_profile_hash_commitment"]
    );
    assert!(genesis.external_lane_verifier_pins("base-other").is_err());

    let mut other_chain = genesis.clone();
    other_chain.chain_id = "postfiat-lane-sepolia-other".to_string();
    let mut other_lock = genesis.clone();
    other_lock.external_lanes.as_mut().expect("lanes")[0]
        .policy
        .lock_contract_address = format!("0x{}", "1b".repeat(20));
    for changed in [other_chain, other_lock] {
        let other = changed
            .external_lane_verifier_pins("base-erc20")
            .expect("pins");
        assert_ne!(other.asset_id, pins.asset_id);
        assert_ne!(other.route_profile_hash, pins.route_profile_hash);
        assert_ne!(other.asset_id_commitment, pins.asset_id_commitment);
        assert_ne!(
            other.route_profile_hash_commitment,
            pins.route_profile_hash_commitment
        );
        assert_ne!(
            other.pftl_genesis_hash_commitment,
            pins.pftl_genesis_hash_commitment
        );
    }
}
