"""Decode the AWS KMS Nitro recipient CMS envelope inside the enclave.

AWS's SDK specifies CMS v2, RSA-OAEP/SHA-256 key transport and AES-256-CBC:
https://github.com/aws/aws-nitro-enclaves-sdk-c/blob/main/source/cms.c
Only decrypt responses received over enclave-owned authenticated KMS TLS.
CMS encryption itself does not authenticate a sender.
"""

from cryptography.hazmat.primitives import hashes, padding as symmetric_padding
from cryptography.hazmat.primitives.asymmetric import padding, rsa
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes


MAX_RECIPIENT_CMS_BYTES = 64 * 1024


def _require(condition: bool) -> None:
    if not condition:
        raise ValueError("unsupported KMS recipient CMS envelope")


def _sha256_algorithm(value) -> None:
    _require(len(value) == 2 and value["algorithm"].native == "sha256")
    _require(value["parameters"].native is None)


def decrypt_recipient_ciphertext(private_key: rsa.RSAPrivateKey, ciphertext: bytes) -> bytes:
    """Unwrap one bounded CMS recipient result; never treat it as raw RSA.

    Both DER and constructed/indefinite BER from KMS are supported. ASN.1
    parsing has a ten-level indefinite-length recursion limit; this adapter
    additionally bounds the whole input and accepts only the fixed CMS shape.
    """
    from asn1crypto import cms, core

    _require(isinstance(private_key, rsa.RSAPrivateKey) and private_key.key_size == 2048)
    _require(isinstance(ciphertext, bytes) and 0 < len(ciphertext) <= MAX_RECIPIENT_CMS_BYTES)
    try:
        info = cms.ContentInfo.load(ciphertext, strict=True)
        _require(len(info) == 2 and info["content_type"].native == "enveloped_data")
        envelope = info["content"]
        _require(len(envelope) == 5 and envelope["version"].native == "v2")
        _require(isinstance(envelope["originator_info"], core.Void))
        _require(isinstance(envelope["unprotected_attrs"], core.Void))
        recipients = envelope["recipient_infos"]
        _require(len(recipients) == 1 and recipients[0].name == "ktri")
        recipient = recipients[0].chosen
        _require(len(recipient) == 4 and recipient["version"].native == "v2")
        _require(recipient["rid"].name == "subject_key_identifier")
        _require(0 < len(recipient["rid"].native) <= 64)
        transport = recipient["key_encryption_algorithm"]
        _require(len(transport) == 2 and transport["algorithm"].native == "rsaes_oaep")
        parameters = transport["parameters"]
        _require(len(parameters) == 3)
        _sha256_algorithm(parameters["hash_algorithm"])
        mask = parameters["mask_gen_algorithm"]
        _require(len(mask) == 2 and mask["algorithm"].native == "mgf1")
        _sha256_algorithm(mask["parameters"])
        source = parameters["p_source_algorithm"]
        _require(len(source) == 2 and source["algorithm"].native == "p_specified")
        _require(source["parameters"].native == b"")
        encrypted_key = recipient["encrypted_key"].native
        _require(len(encrypted_key) == 256)
        content = envelope["encrypted_content_info"]
        _require(len(content) == 3 and content["content_type"].native == "data")
        algorithm = content["content_encryption_algorithm"]
        _require(len(algorithm) == 2 and algorithm["algorithm"].native == "aes256_cbc")
        iv = algorithm["parameters"].native
        encrypted_content = content["encrypted_content"].native
        _require(isinstance(iv, bytes) and len(iv) == 16)
        _require(isinstance(encrypted_content, bytes) and len(encrypted_content) > 0)
        _require(len(encrypted_content) % 16 == 0)
        key = private_key.decrypt(encrypted_key, padding.OAEP(
            mgf=padding.MGF1(hashes.SHA256()), algorithm=hashes.SHA256(), label=None,
        ))
        _require(len(key) == 32)
        decryptor = Cipher(algorithms.AES(key), modes.CBC(iv)).decryptor()
        padded = decryptor.update(encrypted_content) + decryptor.finalize()
        unpadder = symmetric_padding.PKCS7(128).unpadder()
        return unpadder.update(padded) + unpadder.finalize()
    except (ValueError, TypeError, KeyError, IndexError, RecursionError, OverflowError):
        # Do not expose parser, key-unwrapping or padding details at the boundary.
        raise ValueError("invalid KMS recipient CMS envelope") from None
