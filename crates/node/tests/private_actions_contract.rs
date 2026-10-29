//! Private actions contract v1 (W-1 segment S1). Generates the canonical
//! vectors and pins them byte for byte, and checks the schema examples,
//! manifest and asset profile against the shared types and code constants.
//! Regenerate with `PRIVATE_ACTIONS_VECTORS_REGENERATE=1`.

use std::{fs, path::PathBuf};

use postfiat_execution::genesis_hash;
use postfiat_node::build_atomic_shielded_action_batch;
use postfiat_privacy_orchard::{asset_orchard as ao, asset_orchard_circuit as aoc};
use postfiat_types::{
    parse_private_public_action, private_action_hash, private_intent_id, AssetBurnOperation,
    AssetOrchardEgressActionPayload, AssetOrchardIngressActionPayload, AssetOrchardIngressNote,
    AssetOrchardIngressV2ActionPayload, AssetOrchardPrivateEgressActionPayload,
    AssetTransactionOperation, Genesis, PrivateActionDomain, PrivateActionRequest,
    PrivateReceiptResponse, PrivateSnapshotRequest, PrivateSnapshotResponse, PrivateSubmitResponse,
    ShieldedAction, ShieldedSwapActionPayload, SignedAssetTransaction, UnsignedAssetTransaction,
    ADDRESS_NAMESPACE, ASSET_BURN_TRANSACTION_KIND, PRIVATE_ACTIONS_SCHEMA_V1,
    PRIVATE_INTENT_HASH_DOMAIN,
};
use serde_json::{json, Value};

const DEVNET_GENESIS_HASH: &str = "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9";
const A666_ASSET_ID: &str = "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c";
const A666_ISSUER: &str = "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b";
const PFUSDC_ASSET_ID: &str = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b";
const PFUSDC_ISSUER: &str = "pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8";
const WALLET: &str = "pf0123456789abcdef0123456789abcdef01234567";
const ML_DSA_65_PUBLIC_KEY_BYTES: usize = 1_952;
const ML_DSA_65_SIGNATURE_BYTES: usize = 3_309;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/private-actions-v1")
        .join(name)
}

fn read_json(path: PathBuf) -> Value {
    serde_json::from_str(&fs::read_to_string(&path).expect("read fixture")).expect("fixture json")
}

fn devnet_genesis() -> Genesis {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/pfeth-reserve-replay/genesis.json");
    serde_json::from_str(&fs::read_to_string(path).expect("read genesis")).expect("genesis json")
}

fn hex(byte: u8, len: usize) -> String {
    format!("{byte:02x}").repeat(len)
}

/// The Pallas generator (x = p - 1, y = 2) in its canonical compressed encoding.
const PALLAS_GENERATOR: &str = "00000000ed302d991bf94c09fc98462200000000000000000000000000000040";

/// A canonical field element: a small little-endian value, padded.
fn field(value: u8) -> String {
    format!("{value:02x}{}", "00".repeat(31))
}

fn domain(genesis: &Genesis) -> PrivateActionDomain {
    PrivateActionDomain {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
    }
}

fn burn(genesis: &Genesis) -> SignedAssetTransaction {
    SignedAssetTransaction {
        unsigned: UnsignedAssetTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: ASSET_BURN_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: "ML-DSA-65".to_string(),
            source: WALLET.to_string(),
            fee: 10,
            sequence: 7,
            operation: AssetTransactionOperation::AssetBurn(AssetBurnOperation {
                owner: WALLET.to_string(),
                issuer: PFUSDC_ISSUER.to_string(),
                asset_id: PFUSDC_ASSET_ID.to_string(),
                amount: 1_000_000,
            }),
        },
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: hex(0x11, ML_DSA_65_PUBLIC_KEY_BYTES),
        signature_hex: hex(0x22, ML_DSA_65_SIGNATURE_BYTES),
    }
}

fn ingress_v2(genesis: &Genesis) -> AssetOrchardIngressV2ActionPayload {
    AssetOrchardIngressV2ActionPayload {
        burn_transaction: burn(genesis),
        pool_id: ao::ASSET_ORCHARD_POOL_ID_V1.to_string(),
        asset_id: PFUSDC_ASSET_ID.to_string(),
        amount: 1_000_000,
        output_commitment: field(3),
        encrypted_output: hex(0x44, 580),
    }
}

fn swap_json(extra: Option<(&str, Value)>) -> String {
    let record =
        |n: u8| json!({"output_commitment": field(n), "value_commitment": PALLAS_GENERATOR});
    let mut swap = json!({
        "version": ao::ASSET_ORCHARD_ACTION_VERSION_V1,
        "schema": ao::ASSET_ORCHARD_ACTION_SCHEMA_V1,
        "pool_id": ao::ASSET_ORCHARD_POOL_ID_V1,
        "proof_system_id": ao::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
        "circuit_id": ao::ASSET_ORCHARD_CIRCUIT_ID_V1,
        "pool_domain": field(1),
        "anchor": field(2),
        "nullifiers": [field(5), field(6)],
        "randomized_verification_keys": [PALLAS_GENERATOR, PALLAS_GENERATOR],
        "output_commitments": [field(9), field(10)],
        "encrypted_outputs": [hex(0x45, 580), hex(0x46, 580)],
        "accounting_inputs": [record(11), record(13)],
        "accounting_outputs": [record(9), record(15)],
        "pricing_claim": {
            "nav_epoch": 12,
            "reserve_packet_hash": hex(0x47, 48),
            "ratio_numerator": 1_000_000,
            "ratio_denominator": 1_000_000,
            "mode": "at_nav_with_band",
            "band_bps": 50,
            "base_asset_tag_lo": format!("{:032x}", 1),
            "base_asset_tag_hi": format!("{:032x}", 2),
            "quote_asset_tag_lo": format!("{:032x}", 3),
            "quote_asset_tag_hi": format!("{:032x}", 4),
        },
        "swap_binding_hash": hex(0x48, 64),
        "fee": 0,
        "proof": hex(0x49, 64),
        "spend_authorization_signatures": [hex(0x4a, 64), hex(0x4b, 64)],
    });
    if let Some((key, value)) = extra {
        swap.as_object_mut()
            .expect("swap object")
            .insert(key.to_string(), value);
    }
    serde_json::to_string(&swap).expect("swap json")
}

fn private_egress() -> AssetOrchardPrivateEgressActionPayload {
    AssetOrchardPrivateEgressActionPayload {
        version: ao::ASSET_ORCHARD_ACTION_VERSION_V1,
        schema: ao::ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1.to_string(),
        pool_id: ao::ASSET_ORCHARD_POOL_ID_V1.to_string(),
        to: WALLET.to_string(),
        asset_id: PFUSDC_ASSET_ID.to_string(),
        amount: 1_000_000,
        fee: 10,
        policy_id: hex(0x51, 48),
        disclosure_hash: hex(0x52, 48),
        proof_system_id: ao::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
        circuit_id: ao::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1.to_string(),
        pool_domain: field(1),
        anchor: field(2),
        nullifier: field(5),
        randomized_verification_key: field(7),
        asset_tag_lo: 3,
        asset_tag_hi: 4,
        exit_binding_hash: hex(0x53, 64),
        proof: hex(0x54, 64),
        spend_authorization_signature: hex(0x55, 64),
    }
}

fn legacy_note() -> AssetOrchardIngressNote {
    AssetOrchardIngressNote {
        diversifier: hex(0x61, 11),
        g_d: field(20),
        pk_d: field(21),
        asset_tag_lo: 3,
        asset_tag_hi: 4,
        value: 1_000_000,
        rho: field(22),
        psi: field(23),
        rcm: field(24),
    }
}

fn positive(kind: &str, action: ShieldedAction, genesis: &Genesis) -> Value {
    let action_json = serde_json::to_string(&action).expect("action json");
    let parsed = parse_private_public_action(&action_json).expect("strict parse");
    assert_eq!(parsed, action);
    let domain = domain(genesis);
    let intent_id = private_intent_id(&domain, &action_json).expect("intent id");
    // Cross-check against the crypto provider the wallet's helper calls.
    let preimage = serde_json::to_vec(&(&domain, action_json.as_str())).expect("preimage");
    let provider = postfiat_crypto_provider::bytes_to_hex(&postfiat_crypto_provider::hash_bytes(
        PRIVATE_INTENT_HASH_DOMAIN,
        &preimage,
    ));
    assert_eq!(intent_id, provider);
    assert_eq!(intent_id.len(), 96);
    let batch = build_atomic_shielded_action_batch(genesis, vec![action.clone()]).expect("batch");
    assert!(batch.atomic);
    json!({
        "name": kind,
        "kind": kind,
        "action_json": action_json,
        "intent_preimage_json": String::from_utf8(preimage).expect("utf8"),
        "intent_id": intent_id,
        "action_hash": private_action_hash(&action).expect("action hash"),
        "batch_id": batch.batch_id,
        "batch_json": serde_json::to_string(&batch).expect("batch json"),
    })
}

fn negative(name: &str, action_json: String, refusal: &str) -> Value {
    assert_eq!(
        parse_private_public_action(&action_json).expect_err(name),
        refusal,
        "{name}"
    );
    json!({"name": name, "action_json": action_json, "refusal": refusal})
}

fn vectors() -> String {
    let genesis = devnet_genesis();
    assert_eq!(genesis_hash(&genesis), DEVNET_GENESIS_HASH);
    let ingress = ShieldedAction::AssetOrchardIngressV2(ingress_v2(&genesis));
    let swap = ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
        swap_json: swap_json(None),
    });
    let ShieldedAction::ShieldedSwapV1(payload) = &swap else {
        unreachable!()
    };
    serde_json::from_str::<ao::AssetOrchardSwapAction>(&payload.swap_json)
        .expect("swap_json decodes as the typed Asset-Orchard swap action");
    let egress = ShieldedAction::AssetOrchardPrivateEgressV1(private_egress());
    let positives = vec![
        positive("asset_orchard_ingress_v2", ingress.clone(), &genesis),
        positive("shielded_swap_v1", swap, &genesis),
        positive("asset_orchard_private_egress_v1", egress, &genesis),
    ];

    let v2 = ingress_v2(&genesis);
    let legacy_ingress = ShieldedAction::AssetOrchardIngressV1(AssetOrchardIngressActionPayload {
        burn_transaction: v2.burn_transaction.clone(),
        pool_id: v2.pool_id.clone(),
        asset_id: v2.asset_id.clone(),
        amount: v2.amount,
        output_commitment: v2.output_commitment.clone(),
        encrypted_output: v2.encrypted_output.clone(),
        note: legacy_note(),
    });
    let legacy_egress = ShieldedAction::AssetOrchardEgressV1(AssetOrchardEgressActionPayload {
        pool_id: ao::ASSET_ORCHARD_POOL_ID_V1.to_string(),
        to: WALLET.to_string(),
        asset_id: PFUSDC_ASSET_ID.to_string(),
        amount: 1_000_000,
        output_commitment: field(3),
        nullifier: field(5),
        note: legacy_note(),
        nk: field(25),
        rivk: field(26),
        spend_auth_verification_key: field(27),
        spend_auth_randomizer: field(28),
        randomized_verification_key: field(7),
        spend_authorization_signature: hex(0x55, 64),
    });
    let mut outer_extra = serde_json::to_value(&ingress).expect("ingress value");
    outer_extra
        .as_object_mut()
        .expect("ingress object")
        .insert("note".to_string(), json!({"value": 1_000_000}));
    let mut negatives = vec![
        negative(
            "legacy_asset_orchard_ingress_v1",
            serde_json::to_string(&legacy_ingress).expect("json"),
            "private_action_legacy_refused",
        ),
        negative(
            "legacy_asset_orchard_egress_v1",
            serde_json::to_string(&legacy_egress).expect("json"),
            "private_action_legacy_refused",
        ),
        negative(
            "swap_json_unknown_field",
            serde_json::to_string(&ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
                swap_json: swap_json(Some(("note_opening", json!(field(30))))),
            }))
            .expect("json"),
            "private_swap_json_fields_refused",
        ),
        negative(
            "action_unknown_field",
            serde_json::to_string(&outer_extra).expect("json"),
            "private_action_unknown_fields",
        ),
    ];

    // Changed text under an existing intent id: the recomputed id differs, so
    // the gateway refuses it. A reformatted text has the same action hash, so
    // deduplication also stops a fresh intent from replaying the action.
    let original = positives[0]["action_json"]
        .as_str()
        .expect("text")
        .to_string();
    let original_intent = positives[0]["intent_id"].as_str().expect("id").to_string();
    let mut changed_payload = v2.clone();
    changed_payload.amount += 1;
    let changed = serde_json::to_string(&ShieldedAction::AssetOrchardIngressV2(changed_payload))
        .expect("json");
    let changed_intent = private_intent_id(&domain(&genesis), &changed).expect("id");
    assert_ne!(changed_intent, original_intent);
    let reformatted = serde_json::to_string_pretty(&ingress).expect("json");
    let reformatted_action = parse_private_public_action(&reformatted).expect("parse");
    assert_ne!(reformatted, original);
    assert_eq!(
        private_action_hash(&reformatted_action).expect("hash"),
        positives[0]["action_hash"].as_str().expect("hash")
    );
    negatives.push(json!({
        "name": "changed_text_under_existing_intent",
        "intent_id": original_intent,
        "action_json": changed,
        "recomputed_intent_id": changed_intent,
        "refusal": "private_intent_mismatch",
    }));
    negatives.push(json!({
        "name": "reformatted_text_same_action",
        "action_json": reformatted,
        "recomputed_intent_id": private_intent_id(&domain(&genesis), &reformatted).expect("id"),
        "action_hash": private_action_hash(&reformatted_action).expect("hash"),
        "refusal": "private_action_duplicate",
    }));

    let document = json!({
        "schema": "postfiat-private-actions-v1-vectors",
        "contract_schema": PRIVATE_ACTIONS_SCHEMA_V1,
        "generated_by": "crates/node/tests/private_actions_contract.rs",
        "unverifiable": "Proofs, signatures, keys, points, commitments and ciphertexts are syntactically valid placeholders. Every action decodes strictly and builds a batch, but no vector verifies or would be accepted on chain.",
        "domain": domain(&genesis),
        "rules": {
            "intent_id": "lowercase hex SHA3-384 of 'tasknode.private-intent.v1' || 0x00 || serde_json::to_vec(&(domain, action_json)): 96 hex characters",
            "action_hash": "lowercase hex SHA3-384 of 'postfiat.private-actions.action.v1' || 0x00 || serde_json::to_vec(&ShieldedAction) of the strictly parsed action",
            "batch_id": "build_atomic_shielded_action_batch(devnet genesis, [action]) with atomic: true, hash domain postfiat.shielded_action_batch.atomic.v2",
        },
        "positive": positives,
        "negative": negatives,
    });
    format!(
        "{}\n",
        serde_json::to_string_pretty(&document).expect("vectors")
    )
}

#[test]
fn vectors_are_pinned() {
    let generated = vectors();
    let path = fixture("vectors.json");
    if std::env::var_os("PRIVATE_ACTIONS_VECTORS_REGENERATE").is_some() {
        fs::write(&path, &generated).expect("write vectors");
    }
    let pinned = fs::read_to_string(&path).expect("read pinned vectors");
    assert!(
        pinned == generated,
        "vectors.json differs from the generator; regenerate with PRIVATE_ACTIONS_VECTORS_REGENERATE=1 and review"
    );
}

fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(value: &Value) {
    let typed: T = serde_json::from_value(value.clone()).expect("example decodes");
    assert_eq!(&serde_json::to_value(&typed).expect("encode"), value);
}

#[test]
fn schema_examples_match_the_shared_types() {
    let schema = read_json(fixture("schema.json"));
    assert_eq!(schema["schema"], PRIVATE_ACTIONS_SCHEMA_V1);
    let examples = &schema["examples"];
    round_trip::<PrivateSnapshotRequest>(&examples["snapshot"]["request"]);
    round_trip::<PrivateSnapshotResponse>(&examples["snapshot"]["response"]);
    round_trip::<PrivateActionRequest>(&examples["submit"]["request"]);
    round_trip::<PrivateSubmitResponse>(&examples["submit"]["response"]);
    round_trip::<PrivateActionRequest>(&examples["receipt"]["request"]);
    round_trip::<PrivateReceiptResponse>(&examples["receipt"]["response"]);
    round_trip::<PrivateReceiptResponse>(&examples["receipt"]["pending_response"]);
    round_trip::<PrivateReceiptResponse>(&examples["receipt"]["abandoned_response"]);
    let abandoned: PrivateReceiptResponse =
        serde_json::from_value(examples["receipt"]["abandoned_response"].clone())
            .expect("abandoned");
    assert_eq!(
        abandoned.state,
        postfiat_types::PrivateAdmissionState::Abandoned
    );
    assert!(abandoned.abandon_reason.is_some());
    // Every other receipt keeps its bytes: the new field is omitted when absent.
    assert!(!serde_json::to_string(
        &serde_json::from_value::<PrivateReceiptResponse>(examples["receipt"]["response"].clone())
            .expect("accepted")
    )
    .expect("encode")
    .contains("abandon_reason"));
    let mut extra = examples["submit"]["request"].clone();
    extra["note"] = json!("refused");
    assert!(serde_json::from_value::<PrivateActionRequest>(extra).is_err());
}

#[test]
fn manifest_matches_the_pinned_code_constants() {
    let manifest = read_json(fixture("manifest.json"));
    let identity = &manifest["identity"];
    assert_eq!(identity["genesis_hash"], DEVNET_GENESIS_HASH);
    assert_eq!(identity["chain_id"], devnet_genesis().chain_id);
    let swap = &manifest["verifiers"]["shielded_swap_v1"];
    assert_eq!(
        swap["proof_system_id"],
        ao::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1
    );
    assert_eq!(swap["circuit_id"], ao::ASSET_ORCHARD_CIRCUIT_ID_V1);
    assert_eq!(swap["k"], aoc::ASSET_ORCHARD_SWAP_V1_K);
    assert_eq!(swap["params_hash"], aoc::ASSET_ORCHARD_SWAP_V1_PARAMS_HASH);
    assert_eq!(swap["vk_hash"], aoc::ASSET_ORCHARD_SWAP_V1_VK_HASH);
    assert_eq!(
        swap["runtime_pinned_vk_fingerprint"],
        aoc::ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );
    assert_eq!(
        swap["public_instance_layout_hash"],
        aoc::ASSET_ORCHARD_SWAP_V1_PUBLIC_INSTANCE_LAYOUT_HASH
    );
    let egress = &manifest["verifiers"]["asset_orchard_private_egress_v1"];
    assert_eq!(
        egress["proof_system_id"],
        ao::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1
    );
    assert_eq!(
        egress["circuit_id"],
        ao::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1
    );
    assert_eq!(egress["k"], aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_K);
    assert_eq!(
        egress["params_hash"],
        aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_PARAMS_HASH
    );
    assert_eq!(
        egress["vk_hash"],
        aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH
    );
    assert_eq!(
        egress["runtime_pinned_vk_fingerprint"],
        aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT
    );
    assert_eq!(
        egress["public_instance_layout_hash"],
        aoc::ASSET_ORCHARD_PRIVATE_EGRESS_V1_PUBLIC_INSTANCE_LAYOUT_HASH
    );
    let assets = read_json(fixture("assets.json"));
    assert_eq!(assets["assets"]["A666"]["asset_id"]["value"], A666_ASSET_ID);
    assert_eq!(assets["assets"]["A666"]["issuer"]["value"], A666_ISSUER);
    assert_eq!(
        assets["assets"]["pfUSDC"]["asset_id"]["value"],
        PFUSDC_ASSET_ID
    );
    assert_eq!(assets["assets"]["pfUSDC"]["issuer"]["value"], PFUSDC_ISSUER);
}
