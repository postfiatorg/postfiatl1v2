"""Prepare YOLO target receipt operations and inspect public node receipts.

No portfolio calculation, secret input, signing, submission or order operation.
Use the existing certified asset-operation CLI to sign an explicitly reviewed
operation. Consensus performs Groth16 verification.
"""
from __future__ import annotations
import argparse
import hashlib
import html
import json
from pathlib import Path
import struct
import subprocess

DIGEST_FIELDS = ('program_sha256', 'methodology_sha256', 'parameter_manifest_sha256',
    'collection_manifest_sha256', 'series_id_sha256', 'underlier_id_sha256', 'epoch_sha256',
    'collection_sha256', 'aggregate_attestation_sha256', 'prior_state_sha256',
    'selected_expiration_sha256', 'target_sha256')
REGISTER_FIELDS = ('registrant', 'submitter', 'program_sha256', 'sp1_program_vkey', 'methodology_sha256',
    'parameter_manifest_sha256', 'collection_manifest_sha256', 'series_id_sha256', 'underlier_id_sha256',
    'epoch_sha256', 'prior_state_sha256', 'replay_id_sha256', 'activation_height')
STATUSES = {1:'TARGET_COMPUTED', 2:'NO_RECONSTITUTION', 3:'NO_ELIGIBLE_SUCCESSOR',
    4:'HALTED_INSTRUMENT', 5:'UNRESOLVED_CAPITAL_POLICY'}


def encoded(value):
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode()


def digest(value, length=64):
    if not isinstance(value, str) or len(value) != length or any(c not in '0123456789abcdef' for c in value):
        raise ValueError('invalid lowercase digest')
    return value


def registration(value):
    if not isinstance(value, dict) or set(value) != set(REGISTER_FIELDS):
        raise ValueError('registration requires every explicit field and no unknown fields')
    result = {key:value[key] for key in REGISTER_FIELDS}
    for key in REGISTER_FIELDS:
        if key.endswith('_sha256'): digest(result[key])
    for key in ('registrant', 'submitter'):
        if not isinstance(result[key], str) or not result[key].strip() or len(result[key].encode()) > 256:
            raise ValueError('invalid explicit registration identity')
    if not isinstance(result['sp1_program_vkey'], str) or not result['sp1_program_vkey'].startswith('0x'):
        raise ValueError('program verification key requires 0x prefix')
    digest(result['sp1_program_vkey'][2:])
    if type(result['activation_height']) is not int or not 0 < result['activation_height'] < 2**64:
        raise ValueError('activation height must be an explicit nonzero u64')
    return result


def registration_id(value):
    body = encoded(registration(value))
    return hashlib.sha3_384(b'postfiat.yolo.target_registration.v1' + len(body).to_bytes(8, 'big') + body).hexdigest()


def decode_public_values(data):
    if len(data) != 408 or data[:12] != b'PFTYTGT1\0\0\0\1':
        raise ValueError('expected the locked 408-byte target ABI v1')
    result = {'schema':'postfiat.yolo.target_public_values.v1'}
    for index, key in enumerate(DIGEST_FIELDS): result[key] = data[12 + index*32:44 + index*32].hex()
    status, snapshots, selected = struct.unpack('>III', data[396:])
    if status not in STATUSES or not 1 <= snapshots <= 64 or selected > 64:
        raise ValueError('invalid target status or count')
    result.update(status=STATUSES[status], snapshot_count=snapshots, selected_contract_count=selected)
    return result


def submit_operation(config, public, proof):
    config = registration(config)
    if not 0 < len(proof) <= 4096: raise ValueError('proof outside its 4096-byte bound')
    values = decode_public_values(public)
    for key in DIGEST_FIELDS:
        if key in config and values[key] != config[key]:
            raise ValueError('proof public values differ from explicitly registered expectations: ' + key)
    return {'operation':'yolo_target_submit_v1', 'submitter':config['submitter'],
        'registration_id':registration_id(config), 'replay_id_sha256':config['replay_id_sha256'],
        'values':values, 'sp1_proof_bytes':list(proof), 'sp1_public_values':list(public)}


def read(path, limit):
    with Path(path).open('rb') as stream: data = stream.read(limit + 1)
    if len(data) > limit: raise ValueError('input exceeds its bound')
    return data


def report_html(value):
    registration_value = value.get('registration') or {}
    receipt = value.get('receipt') or {}
    fields = [('Chain', value.get('chainId')), ('Registration', value.get('registrationId')),
        ('Receipt present', bool(receipt)), ('Inclusion height', receipt.get('inclusion_height')),
        ('Transaction', receipt.get('transaction_hash')),
        ('Program', registration_value.get('operation', {}).get('program_sha256')),
        ('Verification key', registration_value.get('operation', {}).get('sp1_program_vkey'))]
    rows = ''.join('<tr><th>' + html.escape(name) + '</th><td>' + html.escape(str(item)) + '</td></tr>' for name,item in fields)
    return ('<!doctype html><html lang="en"><meta charset="utf-8"><title>YOLO target receipt</title>'
        '<style>body{font:16px system-ui;max-width:1000px;margin:3rem auto;padding:1rem;color:#17212b}table{width:100%;border-collapse:collapse}th,td{text-align:left;padding:.8rem;border-bottom:1px solid #ddd;overflow-wrap:anywhere}pre{white-space:pre-wrap;overflow-wrap:anywhere}</style>'
        '<h1>YOLO target receipt</h1><p>Public node-reported evidence. A portfolio target does not execute trades or establish reserves.</p><table>'
        + rows + '</table><h2>Chain finality evidence</h2><pre>' + html.escape(json.dumps(value.get('finality'), indent=2))
        + '</pre><details><summary>Complete public response</summary><pre>' + html.escape(json.dumps(value, indent=2)) + '</pre></details></html>')


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    register = commands.add_parser('prepare-register', help='Write an unsigned operation from explicit run configuration.')
    register.add_argument('--config', required=True); register.add_argument('--output', required=True)
    submit = commands.add_parser('prepare-submit', help='Bind public proof bytes to reviewed registration metadata.')
    submit.add_argument('--config', required=True); submit.add_argument('--public-values', required=True)
    submit.add_argument('--proof-calldata', required=True); submit.add_argument('--output', required=True)
    query = commands.add_parser('query', help='Read a receipt and chain finality from a local node.')
    query.add_argument('--node-exe', required=True); query.add_argument('--data-dir', required=True)
    query.add_argument('--registration-id', required=True); query.add_argument('--expected-program', required=True)
    query.add_argument('--expected-vkey', required=True); query.add_argument('--output', required=True)
    query.add_argument('--expected-chain-id', required=True); query.add_argument('--expected-genesis-hash', required=True)
    page = commands.add_parser('html', help='Create a standalone public receipt viewer.')
    page.add_argument('--receipt', required=True); page.add_argument('--output', required=True)
    args = parser.parse_args(argv)
    if args.command.startswith('prepare-'):
        config = registration(json.loads(read(args.config, 64*1024)))
        if args.command == 'prepare-register': result = {'operation':'yolo_target_register_v1', **config}
        else: result = submit_operation(config, read(args.public_values,408), read(args.proof_calldata,4096))
    elif args.command == 'query':
        digest(args.registration_id,96); digest(args.expected_program); digest(args.expected_vkey.removeprefix('0x'))
        completed = subprocess.run([args.node_exe,'rpc','--data-dir',args.data_dir,'--method','yolo_target_receipt',
            '--registration-id',args.registration_id],check=True,capture_output=True,text=True,timeout=60)
        response = json.loads(completed.stdout)
        if response.get('error'): raise ValueError('node rejected receipt query')
        result = response['result']; found = result.get('registration')
        if (result.get('schema') != 'postfiat.yolo.target_receipt_query.v1'
            or result.get('chainId') != args.expected_chain_id
            or result.get('genesisHash') != digest(args.expected_genesis_hash,96)):
            raise ValueError('node response differs from independently expected chain')
        if result.get('registrationId') != args.registration_id: raise ValueError('query registration differs')
        if found:
            config = registration(found['operation'])
            if (registration_id(config) != args.registration_id or config['program_sha256'] != args.expected_program
                or config['sp1_program_vkey'] != args.expected_vkey): raise ValueError('node registration differs from independent pins')
    else:
        result = json.loads(read(args.receipt, 4*1024*1024))
        if result.get('schema') != 'postfiat.yolo.target_receipt_query.v1': raise ValueError('invalid receipt query schema')
        with Path(args.output).open('x') as stream: stream.write(report_html(result))
        return 0
    with Path(args.output).open('x') as stream: stream.write(json.dumps(result,indent=2)+'\n')
    print(json.dumps({'output':args.output,'registrationId':registration_id(config) if args.command.startswith('prepare-') else args.registration_id}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
