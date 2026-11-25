"""Generate shared, synthetic Nitro validation cases without exporting keys."""

from datetime import timedelta
from pathlib import Path
import json
import sys

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, rsa
from cryptography.x509.oid import NameOID, ObjectIdentifier

from pftl_schwab_tee.canonical import sha256_bytes
from test_yolo_nitro_attestation import NOW, BINDING_DIGEST, _certificates, _document, _key_usage


def intermediate_chain(*, root_path=1, intermediate_ca=True, intermediate_can_sign=True,
                       leaf_ca=False, leaf_can_sign=True, future_leaf=False,
                       critical_extension=False, signature_hash=None):
    keys = [ec.generate_private_key(ec.SECP384R1()) for _ in range(3)]
    names = [x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, name)])
             for name in ("synthetic-root", "synthetic-intermediate", "synthetic-leaf")]
    certs = []
    for index in range(3):
        ca = (True, intermediate_ca, leaf_ca)[index]
        usage = _key_usage(ca=index < 2)
        if (index == 1 and not intermediate_can_sign) or (index == 2 and not leaf_can_sign):
            usage = x509.KeyUsage(False, False, False, False, False, False, False, False, False)
        builder = (x509.CertificateBuilder().subject_name(names[index])
                   .issuer_name(names[max(index - 1, 0)]).public_key(keys[index].public_key())
                   .serial_number(index + 1)
                   .not_valid_before(NOW + timedelta(seconds=1) if index == 2 and future_leaf else NOW - timedelta(hours=1))
                   .not_valid_after(NOW + timedelta(hours=1))
                   .add_extension(x509.BasicConstraints(ca=ca, path_length=(root_path if index == 0 else 0) if ca else None), critical=True)
                   .add_extension(usage, critical=True))
        if index == 1 and critical_extension:
            builder = builder.add_extension(x509.UnrecognizedExtension(ObjectIdentifier("1.2.3.4.5"), b"\x05\x00"), critical=True)
        certs.append(builder.sign(keys[max(index - 1, 0)], signature_hash or hashes.SHA384()))
    return (keys[0], certs[0], keys[2], certs[2]), [cert.public_bytes(serialization.Encoding.DER) for cert in certs[:2]]


def cases():
    result = []
    base_certificates = _certificates()
    recipient = rsa.generate_private_key(public_exponent=65537, key_size=2048)

    def add(name, valid, *, certificates=base_certificates, mutate=None, **kwargs):
        document, _ = _document(certificates=certificates, recipient_key=recipient, **kwargs)
        if mutate is not None: document = mutate(document)
        root = certificates[1].public_bytes(serialization.Encoding.DER)
        result.append({
            "name": name, "valid": valid, "documentHex": document.hex(),
            "rootDerHex": root.hex(), "bindingDigest": BINDING_DIGEST,
            "verificationTimeMs": int(NOW.timestamp() * 1000),
            "policy": {
                "schema": "postfiat.yolo.nitro_proof_policy.v1", "rootCertificateSha256": sha256_bytes(root),
                "approvedPcrSets": [[{"index": i, "sha384": bytes([0x11 + i]).hex() * 48} for i in range(3)]],
                "maxAgeMs": 60000, "maxFutureSkewMs": 1000,
                "timeRule": "EPOCH_WINDOW_END", "statementRule": "COLLECTION_AND_TARGET_INPUT_V1", "verifierId": "synthetic-only",
            },
        })

    add("valid_direct_chain", True)
    add("valid_age_boundary", True, timestamp=NOW - timedelta(milliseconds=60000))
    add("valid_future_boundary", True, timestamp=NOW + timedelta(milliseconds=1000))
    add("stale", False, timestamp=NOW - timedelta(milliseconds=60001))
    add("future", False, timestamp=NOW + timedelta(milliseconds=1001))
    add("debug_pcrs", False, all_pcrs_zero=True)
    add("wrong_pcr", False, pcr0=b"\x22" * 48)
    add("wrong_pcr_length", False, pcr0=b"\x11" * 32)
    add("wrong_digest", False, payload_overrides={"digest": "SHA256"})
    add("wrong_nonce", False, nonce=b"wrong")
    add("wrong_binding", False, statement_digest="43" * 32)
    add("legacy_schema", False, user_data_schema="postfiat.yolo.nitro_user_data.v1")
    add("invalid_user_data", False, payload_overrides={"user_data": b"{}"})
    add("noncanonical_user_data", False, payload_overrides={"user_data": b"{ }"})
    add("unknown_payload_field", False, payload_overrides={"unexpected": 1})
    add("invalid_spki", False, payload_overrides={"public_key": b"invalid"})
    wrong_rsa = rsa.generate_private_key(public_exponent=65537, key_size=1024)
    add("wrong_rsa_size", False, payload_overrides={"public_key": wrong_rsa.public_key().public_bytes(serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo)})
    add("wrong_cose_algorithm", False, protected_header={1: -7})
    add("boolean_cose_algorithm_key", False, protected_header={True: -35})
    add("extra_protected_header", False, protected_header={1: -35, 4: b"extra"})
    add("unprotected_header", False, unprotected_header={4: b"extra"})
    add("bad_cose_signature", False, mutate=lambda data: data[:-1] + bytes([data[-1] ^ 1]))
    add("trailing_cbor", False, mutate=lambda data: data + b"\x00")
    leaf = base_certificates[3].public_bytes(serialization.Encoding.DER)
    add("bad_leaf_signature", False, payload_overrides={"certificate": leaf[:-1] + bytes([leaf[-1] ^ 1])})
    add("trailing_certificate_der", False, payload_overrides={"certificate": leaf + b"\x00"})
    root = base_certificates[1].public_bytes(serialization.Encoding.DER)
    add("duplicate_certificates", False, payload_overrides={"cabundle": [root, root]})
    other_root = _certificates()[1].public_bytes(serialization.Encoding.DER)
    add("unapproved_root", False, payload_overrides={"cabundle": [other_root]})
    for name, valid, options in [
        ("valid_intermediate_chain", True, {}),
        ("root_path_length", False, {"root_path": 0}),
        ("intermediate_not_ca", False, {"intermediate_ca": False}),
        ("intermediate_cannot_sign", False, {"intermediate_can_sign": False}),
        ("leaf_is_ca", False, {"leaf_ca": True}),
        ("leaf_cannot_sign", False, {"leaf_can_sign": False}),
        ("future_leaf_certificate", False, {"future_leaf": True}),
        ("unknown_critical_extension", False, {"critical_extension": True}),
        ("wrong_certificate_algorithm", False, {"signature_hash": hashes.SHA256()}),
    ]:
        certificates, bundle = intermediate_chain(**options)
        add(name, valid, certificates=certificates, payload_overrides={"cabundle": bundle})
        if valid:
            bad = bundle[1][:-1] + bytes([bundle[1][-1] ^ 1])
            add("bad_intermediate_signature", False, certificates=certificates, payload_overrides={"cabundle": [bundle[0], bad]})
            add("wrong_certificate_order", False, certificates=certificates, payload_overrides={"cabundle": list(reversed(bundle))})
    return {"schema": "postfiat.yolo.synthetic_nitro_validation.v2", "cases": result}


if __name__ == "__main__":
    destination = Path(sys.argv[1])
    destination.parent.mkdir(parents=True, exist_ok=True)
    value = cases()
    with destination.open("x") as output:
        json.dump(value, output, indent=2)
        output.write("\n")
    print(f"Wrote {len(value['cases'])} synthetic Nitro cases")
