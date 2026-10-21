
// TEMPORARY (release qualification 2026-10-21, never committed): build scratch stores the way the
// tests above do and leave them on disk for the release executable's dry and real runs.
#[test]
#[ignore]
fn release_qualification_scratch_stores() {
    let mut roots = BTreeMap::new();
    for (name, shape) in [
        ("snapshot-own-files", Shape::Snapshot),
        ("snapshot-real-files", Shape::Snapshot),
        ("legacy-own-files", Shape::LegacyNoRecords),
    ] {
        let node = node(shape);
        roots.insert(
            name.to_owned(),
            serde_json::json!({
                "root": node.root.display().to_string(),
                "data_dir": node.data_dir.display().to_string(),
                "committee_file": node.committee_file.display().to_string(),
                "anchor_file": node.anchor_file.display().to_string(),
                "next_committee_root": bytes_to_hex(&node.next.domain.committee_root.0),
                "anchor_id": bytes_to_hex(&node.anchor_id.0),
            }),
        );
        std::mem::forget(node);
    }
    println!("SCRATCH_STORES {}", serde_json::to_string(&roots).expect("json"));
}
