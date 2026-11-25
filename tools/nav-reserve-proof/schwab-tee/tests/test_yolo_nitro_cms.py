"""KMS recipient wire-format regression vectors; every key/input is synthetic."""

from datetime import datetime, timedelta, timezone
import os
import shutil
import subprocess

import pytest
from asn1crypto import cms
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding, rsa
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives import padding as symmetric_padding
from cryptography.x509.oid import NameOID

from pftl_schwab_tee.nitro_runtime import decrypt_recipient_ciphertext


def recipient_envelope(public_key, plaintext):
    key, iv = os.urandom(32), os.urandom(16)
    encrypted_key = public_key.encrypt(key, padding.OAEP(
        mgf=padding.MGF1(hashes.SHA256()), algorithm=hashes.SHA256(), label=None,
    ))
    padder = symmetric_padding.PKCS7(128).padder()
    padded = padder.update(plaintext) + padder.finalize()
    encryptor = Cipher(algorithms.AES(key), modes.CBC(iv)).encryptor()
    encrypted = encryptor.update(padded) + encryptor.finalize()
    return cms.ContentInfo({
        "content_type": "enveloped_data",
        "content": {
            "version": "v2",
            "recipient_infos": [cms.RecipientInfo({"ktri": {
                "version": "v2",
                "rid": {"subject_key_identifier": b"synthetic-recipient"},
                "key_encryption_algorithm": {
                    "algorithm": "rsaes_oaep",
                    "parameters": {
                        "hash_algorithm": {"algorithm": "sha256"},
                        "mask_gen_algorithm": {
                            "algorithm": "mgf1", "parameters": {"algorithm": "sha256"},
                        },
                        "p_source_algorithm": {"algorithm": "p_specified", "parameters": b""},
                    },
                },
                "encrypted_key": encrypted_key,
            }})],
            "encrypted_content_info": {
                "content_type": "data",
                "content_encryption_algorithm": {"algorithm": "aes256_cbc", "parameters": iv},
                "encrypted_content": encrypted,
            },
        },
    }).dump()


@pytest.fixture(scope="module")
def recipient_key():
    return rsa.generate_private_key(public_exponent=65537, key_size=2048)


def test_real_cms_container_is_not_a_raw_rsa_ciphertext(recipient_key):
    plaintext = b"synthetic Schwab credential envelope" * 20
    envelope = recipient_envelope(recipient_key.public_key(), plaintext)
    assert len(envelope) > 256
    assert decrypt_recipient_ciphertext(recipient_key, envelope) == plaintext


@pytest.mark.parametrize("size", [1, 15, 16, 17, 32, 1024, 4096])
def test_cms_content_and_padding_boundaries(recipient_key, size):
    plaintext = b"x" * size
    assert decrypt_recipient_ciphertext(
        recipient_key, recipient_envelope(recipient_key.public_key(), plaintext),
    ) == plaintext


@pytest.mark.parametrize("streaming", [False, True])
def test_independent_openssl_cms_der_and_streaming_ber(recipient_key, tmp_path, streaming):
    executable = shutil.which("openssl")
    if executable is None:
        pytest.skip("independent CMS encoder requires OpenSSL")
    subject = x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, "Synthetic CMS recipient")])
    now = datetime.now(timezone.utc)
    certificate = (x509.CertificateBuilder().subject_name(subject).issuer_name(subject)
        .public_key(recipient_key.public_key()).serial_number(1)
        .not_valid_before(now - timedelta(days=1)).not_valid_after(now + timedelta(days=1))
        .add_extension(x509.SubjectKeyIdentifier.from_public_key(recipient_key.public_key()), False)
        .sign(recipient_key, hashes.SHA256()))
    cert_path = tmp_path / "public-recipient.pem"
    cert_path.write_bytes(certificate.public_bytes(serialization.Encoding.PEM))
    plaintext = b"independent synthetic CMS content\x00\xff" * 40
    command = [executable, "cms", "-encrypt", "-binary", "-outform", "DER",
               "-aes-256-cbc", "-keyid", "-recip", str(cert_path),
               "-keyopt", "rsa_padding_mode:oaep", "-keyopt", "rsa_oaep_md:sha256",
               "-keyopt", "rsa_mgf1_md:sha256"]
    if streaming:
        command.append("-stream")
    result = subprocess.run(command, input=plaintext, capture_output=True, check=False)
    assert result.returncode == 0, result.stderr.decode()
    assert decrypt_recipient_ciphertext(recipient_key, result.stdout) == plaintext


@pytest.mark.parametrize("mutation", [
    "envelope_version", "recipient_version", "multiple_recipients", "oaep_hash",
    "mgf_hash", "label", "cipher", "iv_size", "key_size", "content_size",
])
def test_cms_rejects_protocol_substitution(recipient_key, mutation):
    info = cms.ContentInfo.load(recipient_envelope(recipient_key.public_key(), b"secret"))
    envelope = info["content"]
    recipient = envelope["recipient_infos"][0].chosen
    oaep = recipient["key_encryption_algorithm"]["parameters"]
    content = envelope["encrypted_content_info"]
    if mutation == "envelope_version":
        envelope["version"] = "v0"
    elif mutation == "recipient_version":
        recipient["version"] = "v0"
    elif mutation == "multiple_recipients":
        envelope["recipient_infos"].append(envelope["recipient_infos"][0].copy())
    elif mutation == "oaep_hash":
        oaep["hash_algorithm"] = {"algorithm": "sha1"}
    elif mutation == "mgf_hash":
        oaep["mask_gen_algorithm"]["parameters"] = {"algorithm": "sha1"}
    elif mutation == "label":
        oaep["p_source_algorithm"]["parameters"] = b"unapproved"
    elif mutation == "cipher":
        content["content_encryption_algorithm"]["algorithm"] = "aes128_cbc"
    elif mutation == "iv_size":
        content["content_encryption_algorithm"]["parameters"] = b"x" * 15
    elif mutation == "key_size":
        recipient["encrypted_key"] = b"x" * 255
    elif mutation == "content_size":
        content["encrypted_content"] = b"x" * 15
    # ASN.1 Choice objects retain their original encoding unless forced.
    encoded = info.dump(force=True)
    with pytest.raises(ValueError, match="invalid KMS recipient CMS envelope"):
        decrypt_recipient_ciphertext(recipient_key, encoded)


def test_cms_rejects_wrong_key_and_bounded_malformed_inputs(recipient_key):
    envelope = recipient_envelope(recipient_key.public_key(), b"synthetic secret")
    other_key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    with pytest.raises(ValueError):
        decrypt_recipient_ciphertext(other_key, envelope)
    for invalid in [b"", envelope[:-1], envelope + b"\x00", b"x" * (64 * 1024 + 1),
                    b"x" * 256, b"\x30\x80" * 64 + b"\x00\x00" * 64]:
        with pytest.raises(ValueError):
            decrypt_recipient_ciphertext(recipient_key, invalid)
