use super::*;
use postfiat_types::ExternalLaneExportProgramInputV1;

const WITNESS_RELEASE_FIXTURE: &str =
    "../pft-token-contracts/test/fixtures/ExternalLaneWitnessReleaseFixture.sol";
const WITNESS_RELEASE_INPUT: &[u8] = include_bytes!("../testdata/witness-release-input.cbor");

/// The Solidity library the Foundry test reads: the public values and the
/// values they carry, and three copies that change only the vault, the token
/// or the chain id.
fn witness_release_fixture(values: &PfUsdcEgressPublicValuesV1) -> String {
    let encode = |values: &PfUsdcEgressPublicValuesV1| {
        hex(&values.canonical_bytes_without_commitment().expect("bytes"))
    };
    let variant = |change: &dyn Fn(&mut PfUsdcEgressPublicValuesV1)| {
        let mut changed = values.clone();
        change(&mut changed);
        encode(&changed)
    };
    let strip = |value: &str| value.trim_start_matches("0x").to_string();
    let mut out = String::from(
        "// SPDX-License-Identifier: MIT OR Apache-2.0\npragma solidity ^0.8.24;\n\n\
         /// Written by the Rust test `committed_witness_gives_the_foundry_release_fixture`\n\
         /// (crates/pft_lane_proofs/src/tests.rs): the public values that\n\
         /// `verify_external_lane_export_witness_v1` returns for one export recorded on a node test\n\
         /// chain whose lane names the lock and token test/ExternalLaneWitnessRelease.t.sol deploys\n\
         /// (testdata/witness-release-input.cbor). Regenerate with EXTERNAL_LANE_WRITE_FIXTURE=1.\n\
         library ExternalLaneWitnessReleaseFixture {\n",
    );
    let raw =
        |name: &str, value: &str| format!("    bytes internal constant {name} = hex\"{value}\";\n");
    let word = |kind: &str, name: &str, value: &str| {
        format!("    {kind} internal constant {name} = {kind}(hex\"{value}\");\n")
    };
    let number = |kind: &str, name: &str, value: u64| {
        format!("    {kind} internal constant {name} = {value};\n")
    };
    out.push_str(&raw("PUBLIC_VALUES", &encode(values)));
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
    out.push_str(&raw("COMMITTEE_ROOT", &values.committee_root));
    out.push_str(&raw("ASSET_ID", &values.asset_id));
    out.push_str(&raw("WITHDRAWAL_ID", &values.withdrawal_id));
    out.push_str(&number("uint64", "AMOUNT", values.amount_atoms));
    out.push_str(&word("bytes20", "RECIPIENT", &strip(&values.recipient)));
    out.push_str(&number(
        "uint64",
        "SOURCE_CHAIN_ID",
        values.arbitrum_chain_id,
    ));
    out.push_str(&word("bytes20", "LOCK", &strip(&values.vault_address)));
    out.push_str(&word(
        "bytes32",
        "LOCK_CODE_HASH",
        &values.vault_runtime_code_hash,
    ));
    out.push_str(&word("bytes20", "TOKEN", &strip(&values.token_address)));
    out.push_str(&word(
        "bytes32",
        "TOKEN_CODE_HASH",
        &values.token_runtime_code_hash,
    ));
    out.push_str(&word("bytes32", "PROOF_NULLIFIER", &values.proof_nullifier));
    out.push_str(&raw(
        "WRONG_VAULT",
        &variant(&|v| v.vault_address = format!("0x{}", "1a".repeat(20))),
    ));
    out.push_str(&raw(
        "WRONG_TOKEN",
        &variant(&|v| v.token_address = format!("0x{}", "2b".repeat(20))),
    ));
    out.push_str(&raw(
        "WRONG_CHAIN_ID",
        &variant(&|v| v.arbitrum_chain_id = 84_532),
    ));
    out.push_str("}\n");
    out
}

/// The recorded node-chain input proves its export, and the Foundry release
/// test pays on exactly the public values the statement returns for it.
#[test]
fn committed_witness_gives_the_foundry_release_fixture() {
    let ExternalLaneExportProgramInputV1::Withdrawal(witness) =
        serde_cbor::from_slice(WITNESS_RELEASE_INPUT).expect("cbor input")
    else {
        panic!("withdrawal input");
    };
    let values = verify_external_lane_export_witness_v1(&witness).expect("statement");
    let fixture = witness_release_fixture(&values);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(WITNESS_RELEASE_FIXTURE);
    if std::env::var_os("EXTERNAL_LANE_WRITE_FIXTURE").is_some() {
        std::fs::write(&path, &fixture).expect("write fixture");
    } else {
        assert_eq!(std::fs::read_to_string(&path).expect("fixture"), fixture);
    }
    let mut changed = witness.clone();
    changed.packet.amount_atoms += 1;
    assert!(verify_external_lane_export_witness_v1(&changed).is_err());
}
