"""Ciphertext and pinned public artifacts sent to a fixed-purpose Nitro prover.

This parent transport cannot decrypt the replay. An independent verifier must
check the returned public proof before submitting a PFTL receipt.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

from .canonical import canonical_json_bytes
from .nitro_archive_transport import _read_frame, _write_frame, MAX_CIPHERTEXT_BYTES


CIRCUIT_NAMES = ('groth16_circuit.bin', 'groth16_pk.bin', 'groth16_vk.bin')
CHUNK_BYTES = 1024 * 1024
EVENT_BYTES = 64 * 1024


def exchange(control, *, config, circuits, pins, ciphertext, record):
    """Complete one bounded protocol exchange; config credentials stay in memory."""
    if set(pins) != set(CIRCUIT_NAMES):
        raise ValueError('unexpected public circuit set')
    if not 0 < len(ciphertext) <= MAX_CIPHERTEXT_BYTES:
        raise ValueError('replay ciphertext exceeds bound')
    def event():
        value = json.loads(_read_frame(control, EVENT_BYTES))
        if not isinstance(value, dict):
            raise ValueError('prover event must be an object')
        return value
    _write_frame(control, canonical_json_bytes(config), EVENT_BYTES)
    for name in CIRCUIT_NAMES:
        value = event()
        expected = {'schema': 'postfiat.yolo.prover_artifact_request.v1', 'name': name, **pins[name]}
        if value != expected:
            raise ValueError('enclave circuit request differs from measured pins')
        record(value)
        remaining, hashed = pins[name]['bytes'], hashlib.sha256()
        with (Path(circuits) / name).open('rb') as source:
            while remaining:
                data = source.read(min(CHUNK_BYTES, remaining))
                if not data:
                    raise ValueError('public circuit truncated')
                hashed.update(data)
                _write_frame(control, data, CHUNK_BYTES)
                remaining -= len(data)
            if source.read(1) or hashed.hexdigest() != pins[name]['sha256']:
                raise ValueError('public circuit differs from measured content')
    recipient = event()
    if (set(recipient) != {'schema', 'bindingDigest', 'statementKey', 'documentBase64'}
            or recipient['schema'] != 'postfiat.yolo.prover_recipient.v1'):
        raise ValueError('expected public enclave recipient document')
    record(recipient)
    _write_frame(control, ciphertext, MAX_CIPHERTEXT_BYTES)
    status = event()
    if status != {'schema': 'postfiat.yolo.prover_status.v1', 'status': 'PROVING'}:
        raise ValueError('expected proving acknowledgement')
    record(status)
    result = event()
    common = {'schema', 'status', 'diagnosticReceipt', 'diagnosticAssociatedData', 'resources'}
    allowed = (common | {'programSha256', 'programVkey', 'collectionManifestSha256',
        'publicValuesBase64', 'proofCalldataBase64', 'proofBase64'} if result.get('status') == 'VERIFIED'
        else common | {'stage'})
    if (set(result) != allowed or result['schema'] != 'postfiat.yolo.enclave_proof_result.v1'
            or result['status'] not in ('VERIFIED', 'PROOF_FAILED')):
        raise ValueError('unexpected enclave result')
    record(result)
    return result
