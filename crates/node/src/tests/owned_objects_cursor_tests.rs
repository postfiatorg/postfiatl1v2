use super::*;

fn owner_key(seed: u8) -> String {
    let keypair = postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[seed; 32]);
    postfiat_crypto_provider::bytes_to_hex(&keypair.public_key)
}

fn object(index: u64, owner: &str, asset: &str) -> postfiat_types::OwnedObject {
    postfiat_types::OwnedObject {
        id: format!("{:064x}", index.wrapping_mul(0x9e37_79b9_7f4a_7c15)),
        version: 1 + index % 3,
        owner_pubkey_hex: owner.to_string(),
        value: 10 + index,
        asset: asset.to_string(),
    }
}

fn write_tip(store: &NodeStore, height: u64, root: &str) {
    let genesis = store.read_genesis().expect("genesis");
    store
        .write_chain_tip(&postfiat_types::ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            height,
            block_hash: "aa".repeat(48),
            state_root: root.repeat(48),
            ordered_batch_count: 0,
            receipt_count: 0,
            history_base_height: 0,
        })
        .expect("write tip");
}

/// `count` objects for `owner` split over two assets, inserted out of
/// order, plus objects of another owner that must never appear.
fn fixture(name: &str, owner: &str, count: u64) -> PathBuf {
    let data_dir = unique_test_dir(&format!("owned-objects-cursor-{name}"));
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    let store = NodeStore::new(&data_dir);
    store
        .write_genesis(&postfiat_types::Genesis::new_with_validator_count(
            "owned-objects-cursor",
            1,
        ))
        .expect("write genesis");
    write_tip(&store, 40, "bb");
    let mut ledger = postfiat_types::LedgerState::empty();
    for index in (0..count).rev() {
        let asset = if index % 2 == 0 { "PFT" } else { "USD" };
        ledger.owned_objects.push(object(index, owner, asset));
    }
    let other = owner_key(9);
    for index in 0..5 {
        ledger
            .owned_objects
            .push(object(1_000 + index, &other, "PFT"));
    }
    store.write_ledger(&ledger).expect("write ledger");
    data_dir
}

fn options(data_dir: &Path, owner: &str) -> OwnedObjectsOptions {
    OwnedObjectsOptions {
        data_dir: data_dir.to_path_buf(),
        owner_public_key_hex: owner.to_string(),
        asset: None,
        limit: None,
        cursor: None,
        omit_owner_keys: false,
        max_page_bytes: None,
    }
}

fn sorted_owned(data_dir: &Path, owner: &str) -> Vec<postfiat_types::OwnedObject> {
    let mut objects = NodeStore::new(data_dir)
        .read_ledger()
        .expect("ledger")
        .owned_objects
        .into_iter()
        .filter(|object| object.owner_pubkey_hex == owner)
        .collect::<Vec<_>>();
    objects.sort_by(|a, b| (&a.asset, &a.id, a.version).cmp(&(&b.asset, &b.id, b.version)));
    objects
}

/// Reads every page from the first; returns the pages.
fn read_all(base: OwnedObjectsOptions) -> Vec<OwnedObjectsReport> {
    let mut pages = Vec::new();
    let mut cursor = None;
    loop {
        let page = owned_objects(OwnedObjectsOptions {
            cursor,
            ..base.clone()
        })
        .expect("page");
        cursor = page.next_cursor.clone();
        pages.push(page);
        if cursor.is_none() {
            return pages;
        }
        assert!(pages.len() < 1_000, "paging did not terminate");
    }
}

fn rows_of(pages: &[OwnedObjectsReport]) -> Vec<postfiat_types::OwnedObject> {
    pages
        .iter()
        .flat_map(|page| page.objects.iter().map(|row| row.as_object(page)))
        .collect()
}

fn compact_len(page: &OwnedObjectsReport) -> usize {
    serde_json::to_vec(page).expect("json").len()
}

#[test]
fn owned_objects_cursor_pages_300_objects_by_128_exactly_once_in_order() {
    let owner = owner_key(1);
    let data_dir = fixture("by-128", &owner, 300);
    let expected = sorted_owned(&data_dir, &owner);
    assert_eq!(expected.len(), 300);
    let pages = read_all(OwnedObjectsOptions {
        limit: Some(128),
        ..options(&data_dir, &owner)
    });
    let sizes = pages.iter().map(|page| page.objects.len()).collect::<Vec<_>>();
    assert_eq!(sizes, vec![128, 128, 44]);
    assert_eq!(rows_of(&pages), expected);
    let total = expected.iter().map(|object| object.value).sum::<u64>();
    for (index, page) in pages.iter().enumerate() {
        assert_eq!(page.total_value, total);
        assert_eq!(page.object_count, page.objects.len() as u64);
        assert_eq!(page.truncated, index < 2);
        assert_eq!(page.next_cursor.is_some(), index < 2);
        assert_eq!((page.height, page.state_root.as_str()), (40, "bb".repeat(48).as_str()));
        assert_eq!(page.cursor, (index > 0).then(|| pages[index - 1].next_cursor.clone()).flatten());
    }
    assert!(pages[2].next_cursor.is_none());
}

#[test]
fn owned_objects_byte_bound_ends_pages_early_and_keeps_every_object() {
    let owner = owner_key(2);
    let data_dir = fixture("bytes", &owner, 300);
    let expected = sorted_owned(&data_dir, &owner);
    // The default bound: 300 keyed objects (~1.2 MB) do not fit one page.
    let first = owned_objects(options(&data_dir, &owner)).expect("first page");
    assert!(first.next_cursor.is_some() && first.truncated);
    assert!(first.objects.len() < 300 && first.objects.len() > 200);
    assert!(compact_len(&first) <= OWNED_OBJECTS_PAGE_MAX_BYTES);
    let pages = read_all(options(&data_dir, &owner));
    assert_eq!(rows_of(&pages), expected);
    // A tighter bound forces short pages well under `limit`.
    let bound = 64 * 1024;
    let pages = read_all(OwnedObjectsOptions {
        limit: Some(128),
        max_page_bytes: Some(bound),
        ..options(&data_dir, &owner)
    });
    assert!(pages.len() > 15);
    assert!(pages.iter().all(|page| compact_len(page) <= bound));
    assert!(pages[..pages.len() - 1]
        .iter()
        .all(|page| page.objects.len() < 128 && page.truncated));
    assert_eq!(rows_of(&pages), expected);
    // Without the per-object key, all 300 fit one small page.
    let page = owned_objects(OwnedObjectsOptions {
        omit_owner_keys: true,
        ..options(&data_dir, &owner)
    })
    .expect("keyless page");
    assert_eq!(page.objects.len(), 300);
    assert!(page.next_cursor.is_none() && page.owner_keys_omitted);
    assert!(page.objects.iter().all(|row| row.owner_pubkey_hex.is_none()));
    assert!(compact_len(&page) < 64 * 1024);
    let json = serde_json::to_value(&page).expect("json");
    assert!(json["objects"][0].get("owner_pubkey_hex").is_none());
    assert_eq!(rows_of(&[page]), expected);
}

#[test]
fn owned_objects_continuation_across_a_new_height_and_view_is_refused() {
    let owner = owner_key(3);
    let data_dir = fixture("heights", &owner, 200);
    let base = OwnedObjectsOptions {
        limit: Some(128),
        ..options(&data_dir, &owner)
    };
    let first = owned_objects(base.clone()).expect("first page");
    let store = NodeStore::new(&data_dir);
    let mut ledger = store.read_ledger().expect("ledger");
    let mut late = object(7, &owner, "USD");
    late.id = "ff".repeat(32);
    ledger.owned_objects.push(late.clone());
    store.write_ledger(&ledger).expect("ledger");
    write_tip(&store, 41, "cc");
    let refused = owned_objects(OwnedObjectsOptions {
        cursor: first.next_cursor.clone(),
        ..base.clone()
    })
    .expect_err("the view changed")
    .to_string();
    assert_eq!(
        owned_objects_error_code(&refused),
        Some(OWNED_OBJECTS_VIEW_CHANGED)
    );
    let pages = read_all(base);
    assert!(pages.iter().all(|page| page.height == 41));
    assert!(pages.iter().all(|page| page.view_digest == pages[0].view_digest));
    assert_ne!(pages[0].view_digest, first.view_digest);
    assert_eq!(pages[0].total_value, first.total_value + late.value);
    assert_eq!(rows_of(&pages).last().expect("row").id, late.id);
}

#[test]
fn owned_objects_refuses_foreign_malformed_and_mismatched_cursors() {
    let owner = owner_key(4);
    let other = owner_key(5);
    let data_dir = fixture("refusals", &owner, 10);
    let other_dir = fixture("refusals-other", &other, 10);
    let page = |dir: &Path, key: &str, asset: Option<&str>, cursor: &str| {
        owned_objects(OwnedObjectsOptions {
            limit: Some(3),
            asset: asset.map(ToString::to_string),
            cursor: Some(cursor.to_string()),
            ..options(dir, key)
        })
    };
    let issued = owned_objects(OwnedObjectsOptions {
        limit: Some(3),
        ..options(&other_dir, &other)
    })
    .expect("other page")
    .next_cursor
    .expect("cursor");
    let refusal = |result: io::Result<OwnedObjectsReport>| {
        let error = result.expect_err("refused").to_string();
        owned_objects_error_code(&error).expect("typed code")
    };
    assert_eq!(
        refusal(page(&data_dir, &owner, None, &issued)),
        OWNED_OBJECTS_CURSOR_OWNER_MISMATCH
    );
    assert!(page(&other_dir, &other, None, &issued).is_ok());
    assert_eq!(
        refusal(page(&other_dir, &other, Some("PFT"), &issued)),
        OWNED_OBJECTS_CURSOR_ASSET_MISMATCH
    );
    let parts = issued.split('.').collect::<Vec<_>>();
    let with = |index: usize, value: &str| {
        let mut parts = parts.clone();
        parts[index] = value;
        parts.join(".")
    };
    for malformed in [
        String::new(),
        "garbage".to_string(),
        with(0, "ooc3"),
        with(3, "zz"),
        with(3, &"A".repeat(64)),
        with(4, "zz"),
        with(6, "01"),
        with(6, "x"),
        format!("{issued}.1"),
    ] {
        assert_eq!(
            refusal(page(&other_dir, &other, None, &malformed)),
            OWNED_OBJECTS_CURSOR_MALFORMED,
            "{malformed}"
        );
    }
    for bad_id in ["abc", &"g".repeat(64), &"a".repeat(66)] {
        assert_eq!(
            refusal(page(&other_dir, &other, None, &with(5, bad_id))),
            OWNED_OBJECTS_CURSOR_INVALID_OBJECT_ID
        );
    }
    // A cursor from before the view digest restarts the read.
    let mut v1 = parts.clone();
    v1.remove(3);
    v1[0] = "ooc1";
    assert_eq!(
        refusal(page(&other_dir, &other, None, &v1.join("."))),
        OWNED_OBJECTS_VIEW_CHANGED
    );
}

#[test]
fn owned_objects_default_call_answers_as_before() {
    let owner = owner_key(6);
    let data_dir = fixture("default", &owner, 2);
    let genesis = NodeStore::new(&data_dir).read_genesis().expect("genesis");
    let objects = sorted_owned(&data_dir, &owner);
    let report = owned_objects(options(&data_dir, &owner)).expect("report");
    let mut json = serde_json::to_value(&report).expect("json");
    // The response before the cursor, field for field.
    let before = serde_json::json!({
        "schema": "postfiat-owned-objects-v1",
        "chain_id": genesis.chain_id,
        "genesis_hash": genesis_hash(&genesis),
        "protocol_version": genesis.protocol_version,
        "owner_public_key_hex": owner,
        "asset": null,
        "limit": 2048,
        "truncated": false,
        "object_count": 2,
        "total_value": 21,
        "objects": objects,
    });
    let added = ["cursor", "next_cursor", "height", "state_root"]
        .map(|key| json.as_object_mut().expect("object").remove(key).expect(key));
    let view = json.as_object_mut().expect("object").remove("view_digest");
    assert_eq!(view.and_then(|v| v.as_str().map(str::len)), Some(64));
    // Revision-era field; `node_id` is absent without node state.
    assert_eq!(
        json.as_object_mut().expect("object").remove("inventory_revision"),
        Some(serde_json::json!({"finalized_height": 40, "fastpay_sequence": 0}))
    );
    assert_eq!(json.get("node_id"), None);
    assert_eq!(json, before);
    assert_eq!(
        added,
        [
            serde_json::Value::Null,
            serde_json::Value::Null,
            40.into(),
            "bb".repeat(48).into()
        ]
    );
}

/// The reviewer's sequence: page 1 returns object 1 of {1, 2, 3} (10 each);
/// at the same tip object 1 is replaced by 4. The continuation must be
/// refused, never answered with {2, 3, 4} under the same height and root.
#[test]
fn owned_objects_continuation_after_a_same_tip_change_is_refused() {
    let owner = owner_key(7);
    let data_dir = fixture("same-tip-change", &owner, 0);
    let store = NodeStore::new(&data_dir);
    let numbered = |n: u8| postfiat_types::OwnedObject {
        id: format!("{n:02x}").repeat(32),
        version: 1,
        owner_pubkey_hex: owner.clone(),
        value: 10,
        asset: "PFT".to_string(),
    };
    let mut ledger = store.read_ledger().expect("ledger");
    ledger.owned_objects.extend([1, 2, 3].map(numbered));
    store.write_ledger(&ledger).expect("ledger");
    let base = OwnedObjectsOptions {
        limit: Some(1),
        ..options(&data_dir, &owner)
    };
    let first = owned_objects(base.clone()).expect("first page");
    assert_eq!(first.objects[0].id, numbered(1).id);
    assert_eq!(first.total_value, 30);
    ledger.owned_objects.retain(|object| object.id != numbered(1).id);
    ledger.owned_objects.push(numbered(4));
    store.write_ledger(&ledger).expect("ledger");
    let refused = owned_objects(OwnedObjectsOptions {
        cursor: first.next_cursor.clone(),
        limit: Some(3),
        ..base.clone()
    })
    .expect_err("a continuation over a changed view is refused")
    .to_string();
    assert_eq!(
        owned_objects_error_code(&refused),
        Some(OWNED_OBJECTS_VIEW_CHANGED)
    );
    let again = owned_objects(base).expect("restarted first page");
    assert_eq!((again.height, &again.state_root), (first.height, &first.state_root));
    assert_eq!(again.total_value, 30);
    assert_eq!(again.objects[0].id, numbered(2).id);
}

/// The same sequence on the transactional store with a real FastPay apply:
/// `owned_apply_v3` spends the owner's object without a new block, so
/// height and state root stay while the view digest changes and a
/// continuation issued before the apply is refused.
#[test]
fn owned_objects_continuation_across_a_real_fastpay_apply_is_refused() {
    let (_root, dirs, owner) = super::fastpay_payment_safety::active_fastpay_fixture();
    let certificate = super::fastpay_payment_safety::active_fastpay_transfer(&dirs, &owner);
    let owner_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
    let base = options(&dirs[0], &owner_hex);
    let before = owned_objects(base.clone()).expect("before");
    assert_eq!((before.object_count, before.total_value), (1, 100));
    // A continuation positioned before the only object, at the read view.
    let cursor = format!(
        "ooc2.{}.-.{}.{}.{}.0",
        crate::lifecycle_queries::owned_objects_owner_tag(&owner_hex),
        before.view_digest,
        postfiat_crypto_provider::bytes_to_hex(b"PFT"),
        "00".repeat(32)
    );
    let continued = OwnedObjectsOptions {
        cursor: Some(cursor),
        ..base.clone()
    };
    assert_eq!(owned_objects(continued.clone()).expect("same view").objects, before.objects);
    owned_apply_v3(
        NodeOptions {
            data_dir: dirs[0].clone(),
        },
        &serde_json::to_string(&certificate).expect("certificate json"),
        "validator-0",
    )
    .expect("FastPay apply");
    let after = owned_objects(base).expect("after");
    assert_eq!((after.height, &after.state_root), (before.height, &before.state_root));
    assert_eq!((after.object_count, after.total_value), (0, 0));
    assert_ne!(after.view_digest, before.view_digest);
    let refused = owned_objects(continued).expect_err("view changed").to_string();
    assert_eq!(
        owned_objects_error_code(&refused),
        Some(OWNED_OBJECTS_VIEW_CHANGED)
    );
}
