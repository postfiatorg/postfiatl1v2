use postfiat_privacy_binius_wallet::keys::*;
#[test]
fn python_hkdf_vector_and_receive_domain() {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../vectors/derivation.json")).unwrap();
    let mut n = Network {
        chain_id: v["chain_id"].as_str().unwrap().into(),
        genesis_hash: v["genesis_hash"].as_str().unwrap().into(),
        protocol_version: 1,
    };
    let seed = hex::decode(v["seed"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let (a, i) = (
        v["account"].as_u64().unwrap() as u32,
        v["index"].as_u64().unwrap() as u32,
    );
    let w = derive(&seed, &n, a, i).unwrap();
    assert!(hex::encode(w.spend_secret) == v["spend"]);
    assert!(hex::encode(w.kem_seed) == v["kem"]);
    assert!(hex::encode(w.addr) == v["owner"]);
    let addr = address(&n, &w.stealth_address()).unwrap();
    assert_eq!(decode(&n, &addr).unwrap().ek_bytes, w.ek_bytes);
    assert!(derive(&seed, &n, a + 1, i).unwrap().addr != w.addr);
    assert!(derive(&seed, &n, a, i + 1).unwrap().addr != w.addr);
    n.chain_id.push('x');
    assert!(derive(&seed, &n, a, i).unwrap().addr != w.addr);
    assert!(decode(&n, &addr).is_err());
}
