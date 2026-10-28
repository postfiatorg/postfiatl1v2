#!/usr/bin/env python3
"""Read-only L1 registry and conservatively dated precommit participation export."""
import argparse
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
import re
import socket
import tempfile

UTC = dt.timezone.utc
VERSION = 'postfiat-local-rpc-v1'
CHAIN = 'postfiat-wan-devnet-2'
SCHEMA = 'postfiat-tasknode-l1-registry-v1'


def iso(value):
    return value.astimezone(UTC).isoformat().replace('+00:00', 'Z')


def timestamp(value):
    result = dt.datetime.fromisoformat(value.replace('Z', '+00:00'))
    if result.tzinfo is None:
        raise ValueError('UTC timestamp required')
    return result.astimezone(UTC)


def rpc(port, method, params=None):
    if method not in {'status', 'validators', 'blocks'}:
        raise ValueError('read method required')
    request = {'version': VERSION, 'id': 'tasknode-l1-registry', 'method': method, 'params': params or {}}
    with socket.create_connection(('127.0.0.1', port), 25) as stream:
        stream.sendall((json.dumps(request) + '\n').encode())
        raw = stream.makefile('rb').readline(8 * 1024 * 1024 + 1)
    if len(raw) > 8 * 1024 * 1024 or not raw.endswith(b'\n'):
        raise ValueError('RPC frame truncated or oversized')
    value = json.loads(raw)
    if value.get('version') != VERSION or value.get('id') != request['id'] or value.get('ok') is not True:
        raise ValueError('read-only RPC refused or mismatched')
    return value['result']


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def registry_rows(value):
    records = value['validators']
    if not records or len(records) != value['validator_count']:
        raise ValueError('invalid registry count')
    rows, ids, keys = [], set(), set()
    for record in records:
        node, key = record['node_id'], record['public_key_hex']
        if record['algorithm_id'] != 'ML-DSA-65' or not re.fullmatch(r'[A-Za-z0-9_-]{1,128}', node):
            raise ValueError('invalid validator record')
        if not re.fullmatch('[0-9a-f]{3904}', key) or node in ids or key in keys:
            raise ValueError('invalid or duplicate validator key')
        ids.add(node)
        keys.add(key)
        rows.append({'validator_id': node, 'ml_dsa_public_key': key, 'active': True})
    return sorted(rows, key=lambda x: x['validator_id'])


def block_summary(block, chain_id, genesis_hash):
    header = block['header']
    height, block_hash = header['height'], header['block_hash']
    if type(height) is not int or height < 1 or not re.fullmatch('[0-9a-f]{96}', block_hash):
        raise ValueError('invalid block identity')
    qc = header.get('consensus_v2_commit', {}).get('precommit_qc')
    if not qc:
        raise ValueError('block has no precommit certificate')
    if qc['phase'] != 'precommit' or qc['round']['height'] != height or qc['block']['block_id'] != block_hash:
        raise ValueError('precommit certificate identity mismatch')
    domain = qc['domain']
    if domain['chain_id'] != chain_id or domain['genesis_hash'] != genesis_hash:
        raise ValueError('precommit certificate chain mismatch')
    signers = []
    for vote in qc['votes']:
        signer = vote['validator']
        if signer in signers or signer not in qc['validators'] or vote['signature']['signer'] != signer:
            raise ValueError('invalid precommit signer')
        if vote['phase'] != 'precommit' or vote['block'] != qc['block'] or vote['domain'] != domain or vote['round'] != qc['round']:
            raise ValueError('precommit vote does not match certificate')
        signature = vote['signature']
        if signature['algorithm_id'] != 'ML-DSA-65' or not re.fullmatch('[0-9a-f]{6618}', signature['signature_hex']) or not re.fullmatch('[0-9a-f]{3904}', signature['public_key_hex']):
            raise ValueError('invalid precommit signature encoding')
        signers.append(signer)
    if type(qc['quorum']) is not int or qc['quorum'] < 1 or len(signers) < qc['quorum']:
        raise ValueError('precommit quorum not met')
    return {'height': height, 'block_hash': block_hash, 'parent_hash': header['parent_hash'],
            'certificate_id': qc['certificate_id'], 'committee_root': domain['committee_root'],
            'precommit_signers': sorted(signers), 'rpc_block_sha256': digest(block)}


def daily_participation(state, rows, now):
    end = now.replace(hour=0, minute=0, second=0, microsecond=0)
    start = end - dt.timedelta(days=30)
    intervals = state.get('intervals', [])
    results = {}
    for row in rows:
        daily = []
        for offset in range(30):
            day = start + dt.timedelta(days=offset)
            next_day = day + dt.timedelta(days=1)
            cursor, complete, blocks = day, True, []
            for interval in intervals:
                lower, upper = timestamp(interval['lower_bound']), timestamp(interval['upper_bound'])
                if upper <= day or lower >= next_day:
                    continue
                if lower > cursor or upper <= lower or (upper - lower).total_seconds() > 900:
                    complete = False
                cursor = max(cursor, min(upper, next_day))
                # A changed head across UTC midnight cannot be assigned to either
                # date: the chain contains no clock. Both days stay unreported.
                if interval['blocks'] and (lower < day or upper >= next_day):
                    complete = False
                else:
                    blocks.extend(interval['blocks'])
            complete = complete and cursor >= next_day
            signed = sum(row['validator_id'] in b['precommit_signers'] for b in blocks)
            total = len(blocks)
            reported = complete and total > 0
            daily.append({'date': day.date().isoformat(), 'complete': complete, 'reported': reported,
                          'committed_blocks': total if complete else None,
                          'signed_blocks': signed if complete else None,
                          'score': signed / total if reported else None,
                          'from_height': blocks[0]['height'] if reported else None,
                          'to_height': blocks[-1]['height'] if reported else None})
        reported = [d for d in daily if d['reported']]
        total = sum(d['committed_blocks'] for d in reported)
        signed = sum(d['signed_blocks'] for d in reported)
        # Preserve the inclusive 95% boundary on every supported Python 3,
        # including versions whose built-in sum uses ordinary float addition.
        mean = math.fsum(d['score'] for d in reported) / len(reported) if reported else None
        # D63 accepts conservative observer brackets as the time source. D1
        # requires at least 25 reported days and the unweighted daily mean.
        available = len(reported) >= 25
        eligible = available and mean >= 0.95
        results[row['validator_id']] = {
            'window_start': iso(start), 'window_end': iso(end),
            'time_basis': 'observer_brackets', 'historical_clock_available': False,
            'available': available, 'reported_days': len(reported),
            'committed_blocks': total, 'signed_blocks': signed,
            'share': signed / total if total else None, 'mean_daily_score': mean,
            'eligible': eligible, 'eligibility_reason': (
                'insufficient_observed_days' if not available else
                'eligible' if eligible else 'below_participation_threshold'),
            'daily': daily,
        }
    return results


def snapshot(ports, state, now, read=rpc, clock=lambda: dt.datetime.now(UTC)):
    if len(set(ports)) < 2:
        raise ValueError('at least two independent validator RPC sources required')
    sources, primary = [], None
    for port in ports:
        before = read(port, 'status')
        validators = read(port, 'validators')
        after = read(port, 'status')
        identity = ['chain_id', 'genesis_hash', 'block_height', 'block_tip_hash']
        if any(before[k] != after[k] for k in identity):
            raise ValueError('chain moved during snapshot; preserve previous publication')
        if before['chain_id'] != CHAIN or any(validators[k] != before[k] for k in ['chain_id', 'genesis_hash']):
            raise ValueError('unexpected chain identity')
        rows = registry_rows(validators)
        if primary is None:
            primary = (before, validators, rows)
        elif (rows != primary[2] or validators['registry_root'] != primary[1]['registry_root']
              or any(before[k] != primary[0][k] for k in identity)):
            raise ValueError('RPC sources disagree; preserve previous publication')
        sources.append({'validator_id': before['node_id'], 'height': before['block_height'],
                        'block_hash': before['block_tip_hash'], 'registry_root': validators['registry_root']})
    if len({s['validator_id'] for s in sources}) != len(sources):
        raise ValueError('RPC sources are not distinct validators')
    status, validators, rows = primary
    chain, genesis, height = status['chain_id'], status['genesis_hash'], status['block_height']
    if state and (state['chain_id'] != chain or state['genesis_hash'] != genesis):
        raise ValueError('chain reset; explicit observer reset required')
    state = json.loads(json.dumps(state)) if state else {'chain_id': chain, 'genesis_hash': genesis, 'intervals': []}
    previous = state.get('last')
    if previous and (height < previous['height'] or (height == previous['height'] and previous['block_hash'] != status['block_tip_hash'])):
        raise ValueError('chain regression detected')
    blocks = []
    from_height = previous['height'] + 1 if previous else height
    if height - from_height > 10000:
        raise ValueError('observer catch-up exceeds bounded run')
    for begin in range(from_height, height + 1, 10):
        count = min(10, height + 1 - begin)
        batch = read(ports[0], 'blocks', {'from_height': begin, 'limit': count})
        if len(batch) != count:
            raise ValueError('missing committed blocks')
        for offset, block in enumerate(batch):
            summary = block_summary(block, chain, genesis)
            if summary['height'] != begin + offset:
                raise ValueError('block sequence gap')
            parent = blocks[-1]['block_hash'] if blocks else previous['block_hash'] if previous else None
            if parent and summary['parent_hash'] != parent:
                raise ValueError('block parent mismatch')
            blocks.append(summary)
    latest = blocks[-1] if blocks else state['latest_commit']
    if latest['block_hash'] != status['block_tip_hash']:
        raise ValueError('block tip mismatch')
    finished = clock()
    if finished < now or (finished - now).total_seconds() > 300:
        raise ValueError('snapshot clock or read duration invalid')
    if previous:
        if timestamp(previous['observed_at']) >= now:
            raise ValueError('observer clock regression')
        state['intervals'].append({'lower_bound': previous['observed_at'], 'upper_bound': iso(finished), 'blocks': blocks})
    state['intervals'] = [i for i in state['intervals'] if timestamp(i['upper_bound']) >= now - dt.timedelta(days=32)]
    state['last'] = {'observed_at': iso(now), 'height': height, 'block_hash': status['block_tip_hash']}
    state['latest_commit'] = latest
    participation = daily_participation(state, rows, finished)
    for row in rows:
        row['participation'] = participation[row['validator_id']]
    output = {'schema': SCHEMA, 'network': 'l1-devnet', 'chain_id': chain, 'genesis_hash': genesis,
              'generated_at': iso(finished), 'height': height, 'registry_root': validators['registry_root'],
              'source_heights': sources, 'latest_commit': latest, 'validators': rows,
              'participation_note': 'Chain blocks have no UTC timestamps. Participation uses conservative read-only observer brackets from exporter startup, without inferred earlier history. Availability requires at least 25 reported days in the last 30 complete UTC days; eligibility also requires mean daily participation >=0.95.'}
    return output, state


def atomic_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix='.' + path.name, dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as stream:
            json.dump(value, stream, sort_keys=True, indent=2)
            stream.write('\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(temporary, 0o644)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary): os.unlink(temporary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ports', default='49650,49651')
    parser.add_argument('--state', required=True)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    state = json.loads(Path(args.state).read_text()) if Path(args.state).exists() else {}
    # Store the read start and close each new-block bracket after reads finish.
    started = dt.datetime.now(UTC)
    output, updated = snapshot([int(p) for p in args.ports.split(',')], state, started)
    atomic_json(args.state, updated)
    atomic_json(args.output, output)
    print(json.dumps({'generated_at': output['generated_at'], 'height': output['height'],
                      'validators': len(output['validators']), 'registry_root': output['registry_root'],
                      'participation_available': all(r['participation']['available'] for r in output['validators'])}))


if __name__ == '__main__':
    main()
