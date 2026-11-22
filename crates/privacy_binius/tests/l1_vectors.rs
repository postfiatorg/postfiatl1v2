use postfiat_crypto_provider::{address_from_public_key, hex_to_bytes, ml_dsa_65_keygen_from_seed};
use postfiat_privacy_binius::verify_l1;
use postfiat_types::{parse_private_public_action, pq_payment::*, ShieldedAction};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/pft-binius-shielded-pq")
}
fn chain() -> Chain<'static> {
    Chain{id:"postfiat-pfsh02-m1-disposable",genesis:hex_to_bytes("e9c735eecfc4550f152330b52f782de84eea2aa7d59dc7e9e317718ed2ba28473854f0cb7c6c972f7f2825ba944b72e8").unwrap().try_into().unwrap(),protocol:1}
}
fn vector(name: &str) -> PqPayment {
    PqPayment::from_bytes(fs::read(root().join(format!("l1-vectors/{name}.bin"))).unwrap()).unwrap()
}

#[test]
fn all_modes_bind_the_domain_and_round_trip_exact_bytes() {
    for asset in ["pft", "pfusdc"] {
        for (name, mode) in [
            ("shield", Mode::Shield),
            ("topup", Mode::Shield),
            ("pay", Mode::Internal),
            ("exit", Mode::Exit),
        ] {
            let payment = vector(&format!("{asset}-{name}"));
            let (terms, sig, inner) = payment.parts().unwrap();
            assert_eq!(terms.mode, mode);
            assert_eq!(PqPayment::encode(&terms, sig, inner).unwrap(), payment);
            assert_eq!(
                pft_binius_shielded::wire::ShieldedTx::from_l1_inner(inner)
                    .unwrap()
                    .to_bytes(),
                inner
            );
            let statement = verify_l1(&payment, chain()).unwrap();
            assert_eq!(
                statement.v_mint,
                u64::from_le_bytes(inner[56..64].try_into().unwrap())
            );
            for wrong in [
                Chain {
                    id: "other",
                    ..chain()
                },
                Chain {
                    genesis: [1; 48],
                    ..chain()
                },
                Chain {
                    protocol: 2,
                    ..chain()
                },
            ] {
                assert!(verify_l1(&payment, wrong).is_err());
            }
            for offset in [4, 5, 6, 7, 55, 63, 71, payment.bytes().len() - 1] {
                let mut bad = payment.bytes().to_vec();
                bad[offset] ^= 1;
                if let Ok(changed) = PqPayment::from_bytes(bad) {
                    assert!(verify_l1(&changed, chain()).is_err());
                }
            }
            if mode == Mode::Exit {
                let address =
                    address_from_public_key(&ml_dsa_65_keygen_from_seed(&[7; 32]).public_key);
                assert_eq!(&inner[72..104], destination(&address).unwrap());
                assert!(destination(
                    &postfiat_crypto_provider::checksum_address(&address).unwrap()
                )
                .is_err());
            }
        }
    }
    let standalone = fs::read(root().join("payments/tx_hop1.bin")).unwrap();
    let wrapped =
        PqPayment::encode(&vector("pft-pay").parts().unwrap().0, None, &standalone).unwrap();
    assert!(verify_l1(&wrapped, chain()).is_err());
}

#[test]
fn bounded_framing_and_strict_json() {
    let payment = vector("pfusdc-exit");
    let (terms, sig, inner) = payment.parts().unwrap();
    for (start, len) in [(72, 12), (84, 20), (56, 8)] {
        let mut changed = inner.to_vec();
        if start == 84 {
            changed[start..start + len].fill(0)
        } else {
            changed[start] = 1
        }
        assert!(PqPayment::encode(&terms, sig, &changed).is_err());
    }
    assert_eq!(
        checked_archive_size(INNER_LIMIT, ARCHIVE_LIMIT - INNER_LIMIT),
        Ok(ARCHIVE_LIMIT)
    );
    assert!(checked_archive_size(INNER_LIMIT, ARCHIVE_LIMIT - INNER_LIMIT + 1).is_err());
    let base = payment.bytes();
    for cut in [0, 6, 71, 72, 1999, 5390, base.len() - 1] {
        assert!(PqPayment::from_bytes(base[..cut].to_vec()).is_err());
    }
    let mut extra = base.to_vec();
    extra.push(0);
    assert!(PqPayment::from_bytes(extra).is_err());
    let mut surplus = vector("pft-pay").bytes().to_vec();
    surplus.extend([0; SIGNATURE_BYTES]);
    assert!(PqPayment::from_bytes(surplus).is_err());
    let mut at = inner.to_vec();
    at.resize(INNER_LIMIT, 0);
    at[INNER_PREFIX - 4..INNER_PREFIX]
        .copy_from_slice(&((INNER_LIMIT - INNER_PREFIX) as u32).to_le_bytes());
    assert!(PqPayment::encode(&terms, sig, &at).is_ok());
    at.push(0);
    assert!(PqPayment::encode(&terms, sig, &at).is_err());
    let mut count = inner.to_vec();
    count[INNER_PREFIX - 4..INNER_PREFIX].fill(255);
    assert!(PqPayment::encode(&terms, sig, &count).is_err());
    // The fixed proof section ends with nonce then two LE column-count bytes.
    let off = INNER_PREFIX + 59916 - 4;
    let mut count = inner.to_vec();
    count[off..off + 2].fill(255);
    assert!(pft_binius_shielded::wire::ShieldedTx::from_l1_inner(&count).is_err());
    let json = serde_json::to_string(&ShieldedAction::PqPaymentV1(payment.clone())).unwrap();
    let padded = json.clone() + &" ".repeat(REQUEST_LIMIT - json.len());
    assert!(parse_private_public_action(&padded).is_ok());
    assert!(parse_private_public_action(&(padded + " ")).is_err());
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let b = v["body"].as_str().unwrap();
    for bad in [
        format!(r#"{{"kind":"pq_payment_v1","body":"{b}","body":"{b}"}}"#),
        format!(r#"{{"kind":"pq_payment_v1","kind":"pq_payment_v1","body":"{b}"}}"#),
        json.replace(b, &format!("{b}=")),
        json.replace(b, &format!("!{}", &b[1..])),
        json.replace(b, b.trim_end_matches('=')),
        json.replace("\"body\"", "\"extra\":0,\"body\""),
    ] {
        assert!(parse_private_public_action(&bad).is_err());
    }
}

#[test]
fn python_and_javascript_transport_round_trip() {
    let py="import sys,json,base64; a=json.load(sys.stdin); b=base64.b64decode(a['body'],validate=True); print(json.dumps(dict(kind=a['kind'],body=base64.b64encode(b).decode())))";
    let js="const a=JSON.parse(require('fs').readFileSync(0));const b=Buffer.from(a.body,'base64');process.stdout.write(JSON.stringify({kind:a.kind,body:b.toString('base64')}))";
    for name in ["pft-shield", "pft-pay", "pfusdc-exit"] {
        let action = ShieldedAction::PqPaymentV1(vector(name));
        let mut transport = serde_json::to_vec(&action).unwrap();
        for (cmd, args) in [("python3", ["-c", py]), ("node", ["-e", js])] {
            let mut child = Command::new(cmd)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(&transport).unwrap();
            let out = child.wait_with_output().unwrap();
            assert!(out.status.success());
            transport = out.stdout;
            assert_eq!(
                parse_private_public_action(std::str::from_utf8(&transport).unwrap()).unwrap(),
                action
            );
        }
    }
}
