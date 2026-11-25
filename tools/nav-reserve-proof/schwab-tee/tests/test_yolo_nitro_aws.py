"""Replay public hardware evidence at its recorded qualification time."""
from datetime import datetime, timezone
import json
from pathlib import Path

from cryptography import x509
from cryptography.hazmat.primitives import serialization
import pytest

from pftl_schwab_tee.nitro_attestation import _decode_cbor, NitroNativeAttestationVerifier


@pytest.mark.parametrize('case', json.loads((Path(__file__).parent / 'fixtures/yolo_nitro_aws_20260905.json').read_text())['cases'], ids=lambda c: c['name'])
def test_real_aws_attestation_and_adjacent_rejections(case):
    root = x509.load_der_x509_certificate(bytes.fromhex(case['rootDerHex']))
    policy = case['policy']
    verifier = NitroNativeAttestationVerifier(root_certificate_pem=root.public_bytes(serialization.Encoding.PEM),
        max_age_ms=policy['maxAgeMs'], max_future_skew_ms=policy['maxFutureSkewMs'],
        clock=lambda: datetime.fromtimestamp(case['verificationTimeMs'] / 1000, timezone.utc))
    def validate():
        parsed = verifier.parse_and_verify(bytes.fromhex(case['documentHex']), attestation_binding_digest=case['bindingDigest'])
        if not any(all(parsed.pcrs.get(p['index']) == bytes.fromhex(p['sha384']) for p in group) for group in policy['approvedPcrSets']):
            raise ValueError('unapproved PCR')
    if case['valid']: validate()
    else:
        with pytest.raises(ValueError): validate()


def test_indefinite_containers_keep_duplicate_size_and_depth_checks():
    assert _decode_cbor(bytes.fromhex('bf61619f0001ffff')) == {'a': [0, 1]}
    for value in ['bf', 'bf00ff', 'ff', 'bf00010002ff', '9fbf00010002ffff',
                  '9f01', '5f4101ff', 'bf f4 00 ff', '9fffff']:
        with pytest.raises(ValueError): _decode_cbor(bytes.fromhex(value))
    with pytest.raises(ValueError, match='item count'):
        _decode_cbor(b'\x9f' + b'\x00' * 4096 + b'\xff')
    with pytest.raises(ValueError, match='nesting'):
        _decode_cbor(b'\x9f' * 34 + b'\xff' * 34)
