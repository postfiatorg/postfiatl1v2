#!/usr/bin/env python3
"""Inspect a launch outbox; explicitly repair ONE verified U4 job under an existing TEE lease.

Does not acquire chain.lock. --execute is for the authorized launch operator,
not the read-only ready lane. The established repair backs up the full outbox,
checks all six finalized histories and the sibling certificate, repairs only
delivery bookkeeping, resumes delivery, and verifies no ledger change.
"""
import argparse
import json
import sys
import time
from pathlib import Path


def verify_candidate(job, blocks):
    error = job.get('last_error') or ''
    if 'storage_duplicate_record: ordered batch `' not in error or 'already exists' not in error:
        raise ValueError('Not the known ordered-history duplicate; do not clear')
    if len(blocks) != 6 or any(len(b) != 1 for b in blocks):
        raise ValueError('Need one independent finalized block from each of six validators')
    for b in blocks:
        h = b[0]['header']
        if (h['height'], h['block_hash'], h['state_root']) != (
                job['block_height'], job['block_hash'], job['expected_state_root']):
            raise ValueError('Finalized block does not match the delivery job')
    if any(b != blocks[0] for b in blocks):
        raise ValueError('Six finalized block records differ')


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--source', type=int, choices=range(6), required=True)
    ap.add_argument('--job-id', required=True)
    ap.add_argument('--output', type=Path, required=True)
    ap.add_argument('--execute', action='store_true')
    args = ap.parse_args()
    if not __debug__:
        raise RuntimeError('Python optimization disables legacy repair guards; refusing')
    import repair_u4 as repair
    from rpc import call
    from concurrent.futures import ThreadPoolExecutor
    from launch_pool import check_coordination
    check_coordination(required_holder='TEE' if args.execute else None)
    if not args.job_id.isalnum() or len(args.job_id) != 96:
        raise ValueError('Expected a full 48-byte hex job id')
    int(args.job_id, 16)
    repair.START = int(time.time()) - 86400
    rows = [r for r in repair.scan(args.source) if r['job']['job_id'] == args.job_id]
    if len(rows) != 1:
        raise ValueError('Expected exactly one current or recent watchdog copy of the job')
    row = rows[0]
    height = row['job']['block_height']
    with ThreadPoolExecutor(max_workers=6) as pool:
        blocks = list(pool.map(lambda i: call('blocks', {'from_height': height, 'limit': 1}, i), range(6)))
    verify_candidate(row['job'], blocks)
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / 'preflight.json').write_text(json.dumps(dict(row=row, blocks=blocks), indent=2) + '\n')
    if args.execute:
        # Recheck lease/daily state immediately before the established repair.
        check_coordination(required_holder='TEE')
        repair.OUT = args.output
        repair.repair(args.source, row, 'launch')
    else:
        print('INSPECTED ONLY: six matching blocks; no repair, restart or lock acquisition')


if __name__ == '__main__':
    main()
