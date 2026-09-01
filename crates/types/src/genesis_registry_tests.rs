// Genesis-registry canonical-type tests and fixture generation.
//
// Fixtures live under `benchmarks/genesis-registry/fixtures/` and are built
// from the archived fork rounds in
// `benchmarks/ai-governance/dunl-subscorer-shadow-20260901/rounds/` (rounds
// 12-19, never refetched). Regenerate with:
//
//   GENESIS_REGISTRY_FIXTURES_REGEN=1 cargo test -p postfiat-types \
//       regenerate_genesis_registry_fixtures
//
// The derivation rules for archive-absent artifacts (bundle CID, convergence
// report, anchor transaction, receipt deadline, fixture ML-DSA keys) are
// documented in the fixtures README and mirrored by the Python reference
// implementation in `python/postfiat_rpc/genesis_registry.py`.

use std::path::PathBuf;

const GR_FIXTURE_CHAIN_ID: &str = "postfiat-l1v2-testnet";
const GR_FIXTURE_ROUNDS: [u64; 8] = [12, 13, 14, 15, 16, 17, 18, 19];
const GR_FIXTURE_MLDSA_DOMAIN: &[u8] = b"L1V2_GR_FIXTURE_MLDSA_V1";
const GR_ROUND_FILES: [&str; 7] = [
    "inputs/model_request.json",
    "inputs/previous_unl.json",
    "inputs/validator_map.json",
    "outputs/model_response.json",
    "outputs/selected_unl.json",
    "outputs/validator_scores.json",
    "runtime/execution_manifest.json",
];
const GR_MUTATION_COUNT: usize = 25;
const GR_RIPPLE_ALPHABET: &[u8; 58] =
    b"rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz";

fn gr_repo_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn gr_rounds_root() -> PathBuf {
    gr_repo_path("benchmarks/ai-governance/dunl-subscorer-shadow-20260901")
}

fn gr_fixtures_root() -> PathBuf {
    gr_repo_path("benchmarks/genesis-registry/fixtures")
}

fn gr_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn gr_unhex(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "odd hex length");
    (0..hex.len() / 2)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

fn gr_load_json(path: &PathBuf) -> serde_json::Value {
    let raw = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_slice(&raw).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// Decodes a base58 `n…` node public key (ripple alphabet, prefix 0x1C,
/// double-SHA-256 checksum) to its 33-byte key material.
fn gr_decode_fork_master_key(encoded: &str) -> [u8; 33] {
    let mut num: Vec<u8> = Vec::new();
    for ch in encoded.bytes() {
        let idx = GR_RIPPLE_ALPHABET
            .iter()
            .position(|c| *c == ch)
            .unwrap_or_else(|| panic!("bad base58 char in {encoded}")) as u32;
        let mut carry = idx;
        for b in num.iter_mut().rev() {
            let v = u32::from(*b) * 58 + carry;
            *b = (v & 0xFF) as u8;
            carry = v >> 8;
        }
        while carry > 0 {
            num.insert(0, (carry & 0xFF) as u8);
            carry >>= 8;
        }
    }
    let leading = encoded
        .bytes()
        .take_while(|b| *b == GR_RIPPLE_ALPHABET[0])
        .count();
    let mut raw = vec![0u8; leading];
    raw.extend_from_slice(&num);
    assert_eq!(raw.len(), 38, "node key payload length for {encoded}");
    assert_eq!(raw[0], 28, "node key type prefix for {encoded}");
    let checksum = genesis_sha256(&genesis_sha256(&raw[..34]));
    assert_eq!(&raw[34..], &checksum[..4], "checksum for {encoded}");
    raw[1..34].try_into().expect("33-byte key")
}

/// `min((50c + 20r + 10s + 10d + 10i) // 100, c + 25)` per
/// `dynamic-unl-scoring/docs/DeterministicFinalScore.md`.
fn gr_deterministic_final_score(row: &serde_json::Value) -> u64 {
    let sub = |field: &str| -> u64 {
        let v = row[field].as_u64().unwrap_or_else(|| panic!("sub-score {field}"));
        assert!(v <= 100, "sub-score {field} out of range");
        v
    };
    let (c, r, s, d, i) = (
        sub("consensus"),
        sub("reliability"),
        sub("software"),
        sub("diversity"),
        sub("identity"),
    );
    ((50 * c + 20 * r + 10 * s + 10 * d + 10 * i) / 100).min(c + 25)
}

fn gr_fixture_round_id(round: u64) -> String {
    format!("testnet-r{round}")
}

fn gr_fixture_deadline(round: u64) -> ([u8; 32], u64) {
    let hash = genesis_sha256(format!("fixture-receipt-deadline:testnet-r{round}").as_bytes());
    (hash, 90_000_000 + round * 1_000)
}

fn gr_fixture_mldsa_public_key(master_key: &[u8; 33]) -> Vec<u8> {
    let mut out = Vec::with_capacity(GENESIS_MLDSA65_PUBLIC_KEY_LEN);
    let mut block = 0u32;
    while out.len() < GENESIS_MLDSA65_PUBLIC_KEY_LEN {
        let mut preimage = Vec::with_capacity(GR_FIXTURE_MLDSA_DOMAIN.len() + 33 + 4);
        preimage.extend_from_slice(GR_FIXTURE_MLDSA_DOMAIN);
        preimage.extend_from_slice(master_key);
        preimage.extend_from_slice(&block.to_be_bytes());
        out.extend_from_slice(&genesis_sha256(&preimage));
        block += 1;
    }
    out.truncate(GENESIS_MLDSA65_PUBLIC_KEY_LEN);
    out
}

fn gr_selected_unl(round: u64) -> Vec<String> {
    let path = gr_rounds_root().join(format!(
        "rounds/{}/outputs/selected_unl.json",
        gr_fixture_round_id(round)
    ));
    gr_load_json(&path)["unl"]
        .as_array()
        .expect("unl array")
        .iter()
        .map(|v| v.as_str().expect("unl entry").to_owned())
        .collect()
}

/// Fixture receipt set: every selected operator receipts, except round 19
/// which omits the last two selected operators to exercise the
/// `Selected ∩ Receipted` intersection. Sorted by master-key bytes.
fn gr_fixture_receipts(round: u64) -> Vec<GenesisIdentityReceiptBodyV1> {
    let unl = gr_selected_unl(round);
    let omit_from = if round == 19 { unl.len() - 2 } else { unl.len() };
    let (deadline_hash, deadline_seq) = gr_fixture_deadline(round);
    let mut receipts: Vec<GenesisIdentityReceiptBodyV1> = unl[..omit_from]
        .iter()
        .map(|b58| {
            let key = gr_decode_fork_master_key(b58);
            GenesisIdentityReceiptBodyV1 {
                version: GENESIS_IDENTITY_RECEIPT_VERSION_V1,
                fork_master_key: key,
                mldsa_public_key: gr_fixture_mldsa_public_key(&key),
                chain_id: GR_FIXTURE_CHAIN_ID.to_owned(),
                genesis_round_id: gr_fixture_round_id(round),
                deadline_ledger_hash: deadline_hash,
                deadline_ledger_seq: deadline_seq,
                expiry_close_time: deadline_seq + 86_400,
            }
        })
        .collect();
    receipts.sort_by_key(|r| r.fork_master_key);
    receipts
}

/// Builds the proposed registry for one archived round, mirroring
/// `python/postfiat_rpc/genesis_registry.py::build_registry`.
fn gr_build_registry(round: u64) -> ProposedGenesisRegistryV1 {
    let round_id = gr_fixture_round_id(round);
    let round_dir = gr_rounds_root().join(format!("rounds/{round_id}"));

    // Verify the archived files against the rounds manifest, then derive the
    // fixture bundle digest from the per-file digest map.
    let manifest = gr_load_json(&gr_rounds_root().join("rounds-manifest.json"));
    let files = manifest["rounds"][round.to_string()]
        .as_object()
        .expect("round file map");
    assert_eq!(files.len(), GR_ROUND_FILES.len(), "round file inventory");
    let mut digest_map = std::collections::BTreeMap::new();
    for name in GR_ROUND_FILES {
        let expected = files[name]["sha256"].as_str().expect("manifest sha256");
        let bytes = std::fs::read(round_dir.join(name)).expect("round file");
        assert_eq!(gr_hex(&genesis_sha256(&bytes)), expected, "digest of {name}");
        digest_map.insert(name.to_owned(), expected.to_owned());
    }
    let bundle_digest =
        genesis_sha256(serde_json::to_string(&digest_map).expect("bundle map json").as_bytes());

    let execution_manifest = gr_load_json(&round_dir.join("runtime/execution_manifest.json"));
    let cutoff = execution_manifest["code"]["selector"]["parameters"]["score_cutoff"]
        .as_u64()
        .expect("score_cutoff");

    // Deterministic final scores over every scored validator, sorted by
    // master key; the archived rounds omit `outputs/final_scores.json`, so
    // the fixture digest covers the canonical recomputation.
    let scores = gr_load_json(&round_dir.join("outputs/validator_scores.json"));
    let mut score_rows: Vec<(String, u64)> = scores["validator_scores"]
        .as_array()
        .expect("validator_scores")
        .iter()
        .map(|row| {
            (
                row["master_key"].as_str().expect("master_key").to_owned(),
                gr_deterministic_final_score(row),
            )
        })
        .collect();
    score_rows.sort();
    let final_scores_json = serde_json::json!({
        "final_scores": score_rows
            .iter()
            .map(|(key, score)| serde_json::json!({"final_score": score, "master_key": key}))
            .collect::<Vec<_>>(),
    });
    let final_scores_digest =
        genesis_sha256(serde_json::to_string(&final_scores_json).expect("final scores").as_bytes());
    let score_by_key: std::collections::BTreeMap<String, u64> = score_rows.into_iter().collect();

    // Identity evidence from the frozen model request, joined through the
    // validator map.
    let validator_map = gr_load_json(&round_dir.join("inputs/validator_map.json"));
    let model_request = gr_load_json(&round_dir.join("inputs/model_request.json"));
    let user_content = model_request["messages"]
        .as_array()
        .expect("messages")
        .iter()
        .find(|m| m["role"] == "user")
        .expect("user message")["content"]
        .as_str()
        .expect("user content")
        .to_owned();
    let marker = "VALIDATOR DATA:";
    let start = user_content.find(marker).expect("validator data marker") + marker.len();
    let mut stream =
        serde_json::Deserializer::from_str(user_content[start..].trim_start()).into_iter();
    let evidence_rows: serde_json::Value =
        stream.next().expect("validator data array").expect("validator data json");
    let evidence_by_id: std::collections::BTreeMap<String, &serde_json::Value> = evidence_rows
        .as_array()
        .expect("evidence array")
        .iter()
        .map(|row| (row["validator_id"].as_str().expect("validator_id").to_owned(), row))
        .collect();
    let evidence_id_by_master: std::collections::BTreeMap<String, String> = validator_map
        .as_object()
        .expect("validator map")
        .iter()
        .map(|(vid, rec)| {
            (rec["master_key"].as_str().expect("map master_key").to_owned(), vid.clone())
        })
        .collect();

    let (deadline_hash, deadline_seq) = gr_fixture_deadline(round);
    let receipts = gr_fixture_receipts(round);
    let receipt_by_key: std::collections::BTreeMap<[u8; 33], &GenesisIdentityReceiptBodyV1> =
        receipts.iter().map(|r| (r.fork_master_key, r)).collect();

    let selected = gr_selected_unl(round);
    let selected_unl_bytes =
        std::fs::read(round_dir.join("outputs/selected_unl.json")).expect("selected unl");
    let manifest_bytes =
        std::fs::read(round_dir.join("runtime/execution_manifest.json")).expect("manifest");

    let mut entries = Vec::new();
    for (index, b58) in selected.iter().enumerate() {
        let key = gr_decode_fork_master_key(b58);
        let Some(receipt) = receipt_by_key.get(&key) else {
            continue; // Selected ∩ Receipted
        };
        let vid = evidence_id_by_master
            .get(b58)
            .unwrap_or_else(|| panic!("no validator-map entry for {b58}"));
        let row = evidence_by_id
            .get(vid)
            .unwrap_or_else(|| panic!("no evidence row for {vid}"));
        let text = |v: &serde_json::Value| v.as_str().unwrap_or("").to_owned();
        let evidence = GenesisEvidenceRecordV1 {
            version: GENESIS_EVIDENCE_RECORD_VERSION_V1,
            fork_master_key: key,
            domain: text(&row["domain"]),
            domain_verified: u64::from(row["domain_verified"].as_bool().unwrap_or(false)),
            provider: text(&row["asn"]["as_name"]),
            country: text(&row["geolocation"]["country"]),
        };
        entries.push(ProposedGenesisEntryV1 {
            fork_master_key: key,
            final_score: *score_by_key
                .get(b58)
                .unwrap_or_else(|| panic!("no score for {b58}")),
            cutoff,
            selection_index: index as u64,
            identity_evidence_digest: evidence.evidence_digest().expect("evidence digest"),
            identity_receipt_digest: receipt.receipt_hash().expect("receipt hash"),
            mldsa_public_key: receipt.mldsa_public_key.clone(),
        });
    }
    entries.sort_by_key(|e| e.fork_master_key);

    let registry = ProposedGenesisRegistryV1 {
        version: PROPOSED_GENESIS_REGISTRY_VERSION_V1,
        chain_id: GR_FIXTURE_CHAIN_ID.to_owned(),
        genesis_round: GenesisRoundRefV1 {
            fork_network: manifest["network"].as_str().expect("network").to_owned(),
            round_number: round,
            bundle_cid: format!("fixture:dunl-subscorer-shadow-20260901/{round_id}"),
            bundle_digest,
            manifest_digest: genesis_sha256(&manifest_bytes),
            final_scores_digest,
            selected_unl_digest: genesis_sha256(&selected_unl_bytes),
            convergence_report_digest: genesis_sha256(
                format!("fixture-convergence:{round_id}").as_bytes(),
            ),
            anchor_tx_hash: genesis_sha256(format!("fixture-anchor:{round_id}").as_bytes()),
        },
        receipt_deadline: GenesisReceiptDeadlineRefV1 {
            fork_ledger_hash: deadline_hash,
            fork_ledger_seq: deadline_seq,
        },
        entries,
        template_trust_graph: TemplateTrustGraphV1 { n_s: 0, q_s: 0, t_s: 0 },
    };
    let mut registry = registry;
    registry.template_trust_graph =
        template_trust_graph_for(registry.entries.len() as u64).expect("template trust graph");
    registry.validate().expect("built registry valid");
    registry
}

// --- JSON representations shared with the committed fixtures -----------------

fn gr_registry_to_json(registry: &ProposedGenesisRegistryV1) -> serde_json::Value {
    serde_json::json!({
        "version": registry.version,
        "chain_id": registry.chain_id,
        "genesis_round": {
            "fork_network": registry.genesis_round.fork_network,
            "round_number": registry.genesis_round.round_number,
            "bundle_cid": registry.genesis_round.bundle_cid,
            "bundle_digest_hex": gr_hex(&registry.genesis_round.bundle_digest),
            "manifest_digest_hex": gr_hex(&registry.genesis_round.manifest_digest),
            "final_scores_digest_hex": gr_hex(&registry.genesis_round.final_scores_digest),
            "selected_unl_digest_hex": gr_hex(&registry.genesis_round.selected_unl_digest),
            "convergence_report_digest_hex":
                gr_hex(&registry.genesis_round.convergence_report_digest),
            "anchor_tx_hash_hex": gr_hex(&registry.genesis_round.anchor_tx_hash),
        },
        "receipt_deadline": {
            "fork_ledger_hash_hex": gr_hex(&registry.receipt_deadline.fork_ledger_hash),
            "fork_ledger_seq": registry.receipt_deadline.fork_ledger_seq,
        },
        "entries": registry.entries.iter().map(|entry| serde_json::json!({
            "fork_master_key_hex": gr_hex(&entry.fork_master_key),
            "final_score": entry.final_score,
            "cutoff": entry.cutoff,
            "selection_index": entry.selection_index,
            "identity_evidence_digest_hex": gr_hex(&entry.identity_evidence_digest),
            "identity_receipt_digest_hex": gr_hex(&entry.identity_receipt_digest),
            "mldsa_public_key_hex": gr_hex(&entry.mldsa_public_key),
        })).collect::<Vec<_>>(),
        "template_trust_graph": {
            "n_s": registry.template_trust_graph.n_s,
            "q_s": registry.template_trust_graph.q_s,
            "t_s": registry.template_trust_graph.t_s,
        },
    })
}

fn gr_fixed_from_hex<const N: usize>(value: &serde_json::Value) -> [u8; N] {
    gr_unhex(value.as_str().expect("hex field"))
        .try_into()
        .expect("fixed-length hex field")
}

fn gr_registry_from_json(value: &serde_json::Value) -> ProposedGenesisRegistryV1 {
    let round = &value["genesis_round"];
    let deadline = &value["receipt_deadline"];
    let graph = &value["template_trust_graph"];
    ProposedGenesisRegistryV1 {
        version: value["version"].as_u64().expect("version"),
        chain_id: value["chain_id"].as_str().expect("chain_id").to_owned(),
        genesis_round: GenesisRoundRefV1 {
            fork_network: round["fork_network"].as_str().expect("fork_network").to_owned(),
            round_number: round["round_number"].as_u64().expect("round_number"),
            bundle_cid: round["bundle_cid"].as_str().expect("bundle_cid").to_owned(),
            bundle_digest: gr_fixed_from_hex(&round["bundle_digest_hex"]),
            manifest_digest: gr_fixed_from_hex(&round["manifest_digest_hex"]),
            final_scores_digest: gr_fixed_from_hex(&round["final_scores_digest_hex"]),
            selected_unl_digest: gr_fixed_from_hex(&round["selected_unl_digest_hex"]),
            convergence_report_digest: gr_fixed_from_hex(&round["convergence_report_digest_hex"]),
            anchor_tx_hash: gr_fixed_from_hex(&round["anchor_tx_hash_hex"]),
        },
        receipt_deadline: GenesisReceiptDeadlineRefV1 {
            fork_ledger_hash: gr_fixed_from_hex(&deadline["fork_ledger_hash_hex"]),
            fork_ledger_seq: deadline["fork_ledger_seq"].as_u64().expect("fork_ledger_seq"),
        },
        entries: value["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .map(|entry| ProposedGenesisEntryV1 {
                fork_master_key: gr_fixed_from_hex(&entry["fork_master_key_hex"]),
                final_score: entry["final_score"].as_u64().expect("final_score"),
                cutoff: entry["cutoff"].as_u64().expect("cutoff"),
                selection_index: entry["selection_index"].as_u64().expect("selection_index"),
                identity_evidence_digest: gr_fixed_from_hex(&entry["identity_evidence_digest_hex"]),
                identity_receipt_digest: gr_fixed_from_hex(&entry["identity_receipt_digest_hex"]),
                mldsa_public_key: gr_unhex(
                    entry["mldsa_public_key_hex"].as_str().expect("mldsa hex"),
                ),
            })
            .collect(),
        template_trust_graph: TemplateTrustGraphV1 {
            n_s: graph["n_s"].as_u64().expect("n_s"),
            q_s: graph["q_s"].as_u64().expect("q_s"),
            t_s: graph["t_s"].as_u64().expect("t_s"),
        },
    }
}

fn gr_receipts_to_json(round: u64, receipts: &[GenesisIdentityReceiptBodyV1]) -> serde_json::Value {
    serde_json::json!({
        "schema": "postfiat-genesis-registry-receipts-v1",
        "round": gr_fixture_round_id(round),
        "receipts": receipts.iter().map(|receipt| serde_json::json!({
            "version": receipt.version,
            "fork_master_key_hex": gr_hex(&receipt.fork_master_key),
            "mldsa_public_key_hex": gr_hex(&receipt.mldsa_public_key),
            "chain_id": receipt.chain_id,
            "genesis_round_id": receipt.genesis_round_id,
            "deadline_ledger_hash_hex": gr_hex(&receipt.deadline_ledger_hash),
            "deadline_ledger_seq": receipt.deadline_ledger_seq,
            "expiry_close_time": receipt.expiry_close_time,
        })).collect::<Vec<_>>(),
    })
}

fn gr_receipts_from_json(value: &serde_json::Value) -> Vec<GenesisIdentityReceiptBodyV1> {
    value["receipts"]
        .as_array()
        .expect("receipts")
        .iter()
        .map(|receipt| GenesisIdentityReceiptBodyV1 {
            version: receipt["version"].as_u64().expect("version"),
            fork_master_key: gr_fixed_from_hex(&receipt["fork_master_key_hex"]),
            mldsa_public_key: gr_unhex(
                receipt["mldsa_public_key_hex"].as_str().expect("mldsa hex"),
            ),
            chain_id: receipt["chain_id"].as_str().expect("chain_id").to_owned(),
            genesis_round_id: receipt["genesis_round_id"]
                .as_str()
                .expect("genesis_round_id")
                .to_owned(),
            deadline_ledger_hash: gr_fixed_from_hex(&receipt["deadline_ledger_hash_hex"]),
            deadline_ledger_seq: receipt["deadline_ledger_seq"].as_u64().expect("seq"),
            expiry_close_time: receipt["expiry_close_time"].as_u64().expect("expiry"),
        })
        .collect()
}

// --- Mutation fixtures -------------------------------------------------------

/// Structural mutations that cannot be expressed through the typed encoder.
#[derive(Clone, Copy, PartialEq)]
enum GrStructuralMutation {
    ShortBundleDigest,
    ShortEntryMasterKey,
    CutEntryMldsaKey,
}

/// Mirrors the canonical field order while applying one structural mutation.
fn gr_encode_with_structural_mutation(
    registry: &ProposedGenesisRegistryV1,
    mutation: GrStructuralMutation,
) -> Vec<u8> {
    let mut w = GenesisCborWriter::new();
    w.map(6);
    w.uint(1);
    w.uint(registry.version);
    w.uint(2);
    w.text(&registry.chain_id);
    w.uint(3);
    {
        let round = &registry.genesis_round;
        w.map(9);
        w.uint(1);
        w.text(&round.fork_network);
        w.uint(2);
        w.uint(round.round_number);
        w.uint(3);
        w.text(&round.bundle_cid);
        w.uint(4);
        if mutation == GrStructuralMutation::ShortBundleDigest {
            w.bytes(&round.bundle_digest[..31]);
        } else {
            w.bytes(&round.bundle_digest);
        }
        w.uint(5);
        w.bytes(&round.manifest_digest);
        w.uint(6);
        w.bytes(&round.final_scores_digest);
        w.uint(7);
        w.bytes(&round.selected_unl_digest);
        w.uint(8);
        w.bytes(&round.convergence_report_digest);
        w.uint(9);
        w.bytes(&round.anchor_tx_hash);
    }
    w.uint(4);
    w.map(2);
    w.uint(1);
    w.bytes(&registry.receipt_deadline.fork_ledger_hash);
    w.uint(2);
    w.uint(registry.receipt_deadline.fork_ledger_seq);
    w.uint(5);
    w.array(registry.entries.len() as u64);
    for (position, entry) in registry.entries.iter().enumerate() {
        w.map(7);
        w.uint(1);
        if position == 0 && mutation == GrStructuralMutation::ShortEntryMasterKey {
            w.bytes(&entry.fork_master_key[..32]);
        } else {
            w.bytes(&entry.fork_master_key);
        }
        w.uint(2);
        w.uint(entry.final_score);
        w.uint(3);
        w.uint(entry.cutoff);
        w.uint(4);
        w.uint(entry.selection_index);
        w.uint(5);
        w.bytes(&entry.identity_evidence_digest);
        w.uint(6);
        w.bytes(&entry.identity_receipt_digest);
        w.uint(7);
        if position == 0 && mutation == GrStructuralMutation::CutEntryMldsaKey {
            w.bytes(&entry.mldsa_public_key[..entry.mldsa_public_key.len() - 1]);
        } else {
            w.bytes(&entry.mldsa_public_key);
        }
    }
    w.uint(6);
    w.map(3);
    w.uint(1);
    w.uint(registry.template_trust_graph.n_s);
    w.uint(2);
    w.uint(registry.template_trust_graph.q_s);
    w.uint(3);
    w.uint(registry.template_trust_graph.t_s);
    w.into_bytes()
}

/// Unvalidated typed encoding, for value-level mutation fixtures.
fn gr_encode_unchecked(registry: &ProposedGenesisRegistryV1) -> Vec<u8> {
    let mut writer = GenesisCborWriter::new();
    registry.encode_into(&mut writer);
    writer.into_bytes()
}

fn gr_mutation_fixtures(base: &ProposedGenesisRegistryV1) -> Vec<(String, String, Vec<u8>)> {
    let canonical = base.canonical_bytes().expect("canonical base");
    let mut fixtures: Vec<(String, String, Vec<u8>)> = Vec::new();
    let mut value_mutation = |name: &str,
                              expected: GenesisRegistryError,
                              mutate: &dyn Fn(&mut ProposedGenesisRegistryV1)| {
        let mut mutated = base.clone();
        mutate(&mut mutated);
        fixtures.push((name.to_owned(), expected.code().to_owned(), gr_encode_unchecked(&mutated)));
    };

    value_mutation("version_unknown", GenesisRegistryError::UnknownVersion, &|r| r.version = 2);
    value_mutation("chain_id_empty", GenesisRegistryError::InvalidChainId, &|r| {
        r.chain_id.clear()
    });
    value_mutation("fork_network_empty", GenesisRegistryError::InvalidForkNetwork, &|r| {
        r.genesis_round.fork_network.clear()
    });
    value_mutation("round_number_zero", GenesisRegistryError::InvalidRoundNumber, &|r| {
        r.genesis_round.round_number = 0
    });
    value_mutation("bundle_cid_empty", GenesisRegistryError::InvalidBundleCid, &|r| {
        r.genesis_round.bundle_cid.clear()
    });
    value_mutation("ledger_seq_zero", GenesisRegistryError::InvalidLedgerSeq, &|r| {
        r.receipt_deadline.fork_ledger_seq = 0
    });
    value_mutation("entry_score_above_range", GenesisRegistryError::ScoreOutOfRange, &|r| {
        r.entries[0].final_score = 101
    });
    value_mutation("entry_score_below_cutoff", GenesisRegistryError::ScoreBelowCutoff, &|r| {
        r.entries[0].final_score = r.entries[0].cutoff - 1
    });
    value_mutation("entry_cutoff_above_range", GenesisRegistryError::CutoffOutOfRange, &|r| {
        r.entries[0].cutoff = 101
    });
    value_mutation("entry_cutoff_mismatch", GenesisRegistryError::CutoffMismatch, &|r| {
        r.entries[1].cutoff += 1
    });
    value_mutation("entry_master_key_bad_prefix", GenesisRegistryError::InvalidMasterKey, &|r| {
        r.entries[0].fork_master_key[0] = 0x05
    });
    value_mutation("entries_unsorted", GenesisRegistryError::UnsortedEntries, &|r| {
        r.entries.swap(0, 1)
    });
    value_mutation("entry_master_key_duplicate", GenesisRegistryError::DuplicateMasterKey, &|r| {
        r.entries[1].fork_master_key = r.entries[0].fork_master_key
    });
    value_mutation(
        "entry_selection_index_duplicate",
        GenesisRegistryError::DuplicateSelectionIndex,
        &|r| r.entries[1].selection_index = r.entries[0].selection_index,
    );
    value_mutation("trust_graph_quorum_wrong", GenesisRegistryError::TrustGraphMismatch, &|r| {
        r.template_trust_graph.q_s += 1
    });
    value_mutation("trust_graph_size_mismatch", GenesisRegistryError::TrustGraphMismatch, &|r| {
        r.template_trust_graph.n_s += 1
    });
    value_mutation("entries_empty", GenesisRegistryError::EmptyEntries, &|r| r.entries.clear());

    for (name, expected, mutation) in [
        (
            "bundle_digest_truncated",
            GenesisRegistryError::InvalidDigestLength,
            GrStructuralMutation::ShortBundleDigest,
        ),
        (
            "entry_master_key_truncated",
            GenesisRegistryError::InvalidMasterKey,
            GrStructuralMutation::ShortEntryMasterKey,
        ),
        (
            "entry_mldsa_key_truncated",
            GenesisRegistryError::InvalidMldsaKeyLength,
            GrStructuralMutation::CutEntryMldsaKey,
        ),
    ] {
        fixtures.push((
            name.to_owned(),
            expected.code().to_owned(),
            gr_encode_with_structural_mutation(base, mutation),
        ));
    }

    // Byte-surgery mutations on the canonical encoding. The canonical bytes
    // begin with `A6 01 01` (map(6), key 1, version 1) and `02` (key 2)
    // before the chain-id text.
    assert_eq!(&canonical[..4], &[0xA6, 0x01, 0x01, 0x02], "canonical prefix");
    let chain_id_pair_end = 4 + 1 + base.chain_id.len();

    let mut noncanonical = vec![0xA6, 0x01, 0x18, 0x01];
    noncanonical.extend_from_slice(&canonical[3..]);
    fixtures.push((
        "version_noncanonical_int".to_owned(),
        GenesisRegistryError::NonCanonicalEncoding.code().to_owned(),
        noncanonical,
    ));

    let mut trailing = canonical.clone();
    trailing.push(0x00);
    fixtures.push((
        "registry_trailing_byte".to_owned(),
        GenesisRegistryError::TrailingBytes.code().to_owned(),
        trailing,
    ));

    fixtures.push((
        "registry_truncated".to_owned(),
        GenesisRegistryError::Truncated.code().to_owned(),
        canonical[..canonical.len() - 1].to_vec(),
    ));

    let mut unknown_field = vec![0xA7];
    unknown_field.extend_from_slice(&canonical[1..]);
    unknown_field.extend_from_slice(&[0x07, 0x00]);
    fixtures.push((
        "registry_unknown_field".to_owned(),
        GenesisRegistryError::UnknownField.code().to_owned(),
        unknown_field,
    ));

    let mut duplicate_field = vec![0xA7];
    duplicate_field.extend_from_slice(&canonical[1..chain_id_pair_end]);
    duplicate_field.extend_from_slice(&canonical[3..chain_id_pair_end]);
    duplicate_field.extend_from_slice(&canonical[chain_id_pair_end..]);
    fixtures.push((
        "registry_duplicate_field".to_owned(),
        GenesisRegistryError::DuplicateField.code().to_owned(),
        duplicate_field,
    ));

    assert_eq!(fixtures.len(), GR_MUTATION_COUNT, "mutation inventory");
    fixtures
}

// --- Tests -------------------------------------------------------------------

fn gr_golden_path(round: u64) -> PathBuf {
    gr_fixtures_root().join(format!("golden/{}.json", gr_fixture_round_id(round)))
}

fn gr_receipts_path(round: u64) -> PathBuf {
    gr_fixtures_root().join(format!("receipts/{}.json", gr_fixture_round_id(round)))
}

#[test]
fn genesis_registry_canonical_encoding_round_trip() {
    for round in GR_FIXTURE_ROUNDS {
        let golden = gr_load_json(&gr_golden_path(round));
        let registry = gr_registry_from_json(&golden["registry"]);
        let bytes = registry.canonical_bytes().expect("canonical bytes");
        let decoded = ProposedGenesisRegistryV1::decode_canonical(&bytes).expect("decode");
        assert_eq!(decoded, registry, "registry round trip r{round}");

        for receipt in gr_receipts_from_json(&gr_load_json(&gr_receipts_path(round))) {
            let receipt_bytes = receipt.canonical_bytes().expect("receipt bytes");
            let decoded =
                GenesisIdentityReceiptBodyV1::decode_canonical(&receipt_bytes).expect("decode");
            assert_eq!(decoded, receipt, "receipt round trip r{round}");
        }
    }
}

#[test]
fn genesis_registry_golden_vectors_hash_stability() {
    for round in GR_FIXTURE_ROUNDS {
        let rebuilt = gr_build_registry(round);
        let bytes = rebuilt.canonical_bytes().expect("canonical bytes");
        let hash = rebuilt.proposed_registry_hash().expect("hash");

        let golden = gr_load_json(&gr_golden_path(round));
        assert_eq!(
            golden["proposed_registry_hash_hex"].as_str().expect("hash hex"),
            gr_hex(&hash),
            "content hash r{round}"
        );
        assert_eq!(
            golden["canonical_cbor_hex"].as_str().expect("cbor hex"),
            gr_hex(&bytes),
            "canonical cbor r{round}"
        );
        assert_eq!(golden["registry"], gr_registry_to_json(&rebuilt), "registry json r{round}");

        let committed_receipts = gr_receipts_from_json(&gr_load_json(&gr_receipts_path(round)));
        assert_eq!(committed_receipts, gr_fixture_receipts(round), "receipts r{round}");

        let expected_entries = if round == 19 { 18 } else { 20 };
        assert_eq!(rebuilt.entries.len(), expected_entries, "entry count r{round}");
    }
}

#[test]
fn genesis_registry_mutation_fixtures_rejected() {
    let dir = gr_fixtures_root().join("mutations/testnet-r12");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("mutations dir")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), GR_MUTATION_COUNT, "mutation fixture count");
    for path in paths {
        let fixture = gr_load_json(&path);
        let name = fixture["name"].as_str().expect("name");
        let expected = fixture["expected_error"].as_str().expect("expected_error");
        let bytes = gr_unhex(fixture["cbor_hex"].as_str().expect("cbor_hex"));
        let error = ProposedGenesisRegistryV1::decode_canonical(&bytes)
            .expect_err(&format!("mutation {name} must be rejected"));
        assert_eq!(error.code(), expected, "error code for mutation {name}");
    }
}

#[test]
fn genesis_registry_template_trust_graph_examples() {
    let cases = [(12, 10, 3), (18, 15, 4), (20, 16, 4)];
    for (n, q, t) in cases {
        let graph = template_trust_graph_for(n).expect("graph");
        assert_eq!((graph.q_s, graph.t_s), (q, t), "template for n={n}");
    }
    for n in 3..=64 {
        let graph = template_trust_graph_for(n).expect("graph");
        assert!(graph.t_s >= 1 && 2 * graph.t_s < graph.q_s && graph.t_s < 2 * graph.q_s - n);
    }
    for n in [0, 1, 2] {
        assert!(template_trust_graph_for(n).is_err(), "n={n} must be rejected");
    }
}

#[test]
fn regenerate_genesis_registry_fixtures() {
    if std::env::var("GENESIS_REGISTRY_FIXTURES_REGEN").is_err() {
        return;
    }
    let root = gr_fixtures_root();
    for sub in ["golden", "receipts", "mutations/testnet-r12"] {
        std::fs::create_dir_all(root.join(sub)).expect("fixture dirs");
    }
    let write = |path: PathBuf, value: &serde_json::Value| {
        let mut body = serde_json::to_string_pretty(value).expect("fixture json");
        body.push('\n');
        std::fs::write(&path, body).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    };
    for round in GR_FIXTURE_ROUNDS {
        let registry = gr_build_registry(round);
        let bytes = registry.canonical_bytes().expect("canonical bytes");
        write(
            gr_golden_path(round),
            &serde_json::json!({
                "schema": "postfiat-genesis-registry-golden-v1",
                "round": gr_fixture_round_id(round),
                "source_rounds_dir": format!(
                    "benchmarks/ai-governance/dunl-subscorer-shadow-20260901/rounds/{}",
                    gr_fixture_round_id(round)
                ),
                "domain": PROPOSED_GENESIS_REGISTRY_DOMAIN_V1,
                "proposed_registry_hash_hex":
                    gr_hex(&registry.proposed_registry_hash().expect("hash")),
                "canonical_cbor_hex": gr_hex(&bytes),
                "registry": gr_registry_to_json(&registry),
            }),
        );
        write(gr_receipts_path(round), &gr_receipts_to_json(round, &gr_fixture_receipts(round)));
    }
    let base = gr_build_registry(12);
    for (name, expected, bytes) in gr_mutation_fixtures(&base) {
        write(
            root.join(format!("mutations/testnet-r12/{name}.json")),
            &serde_json::json!({
                "schema": "postfiat-genesis-registry-mutation-v1",
                "round": "testnet-r12",
                "name": name,
                "expected_error": expected,
                "cbor_hex": gr_hex(&bytes),
            }),
        );
    }
}
