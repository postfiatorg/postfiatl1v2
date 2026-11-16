#!/usr/bin/env python3
"""Reproduce the lost-ack branch and historical clearing on an isolated outbox copy."""
import argparse
import copy
import hashlib
import json
import subprocess
import tarfile
from pathlib import Path
from clear_launch_stall import verify_candidate


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ('archive', 'blocks', 'topology', 'output'):
        ap.add_argument('--' + name, type=Path, required=True)
    args = ap.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    with tarfile.open(args.archive) as archive:
        for member in archive.getmembers():
            p = Path(member.name)
            if p.is_absolute() or '..' in p.parts or member.issym() or member.islnk():
                raise ValueError('Unsafe archive member')
        archive.extractall(args.output, filter='data')
    pending = [p for p in (args.output / 'certified-send-outbox').glob('*/job.json')
               if not json.loads(p.read_text()).get('completed')]
    if len(pending) != 1:
        raise ValueError('Expected exactly one stalled job')
    job_path = pending[0]
    job = json.loads(job_path.read_text())
    blocks = json.loads(args.blocks.read_text())
    verify_candidate(job, blocks)
    if 'already applied' in job['last_error']:
        raise ValueError('Not the observed old-handler miss')
    mismatched = copy.deepcopy(blocks)
    mismatched[5][0]['header']['block_hash'] = '00' * 48
    try:
        verify_candidate(job, mismatched)
    except ValueError:
        pass
    else:
        raise AssertionError('Mismatched history accepted')
    helpers = Path(__file__).resolve().parents[1] / 'devnet-2'
    rows = []
    for name, params in [('rebuild_certified_send_tombstone.py', [job_path.parent, args.topology]),
                         ('complete_certified_send_job.py', [job_path])]:
        cmd = ['python3', str(helpers / name), *map(str, params)]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        rows.append(dict(command=cmd, stdout=result.stdout, stderr=result.stderr, returncode=result.returncode))
    repaired = json.loads(job_path.read_text())
    send = json.loads((job_path.parent / (job['target'] + '.send.json')).read_text())
    if not repaired['completed'] or not send['ack']['already_applied']:
        raise AssertionError('Local clearing failed')
    result = dict(passed=True, original_archive_sha256=hashlib.sha256(args.archive.read_bytes()).hexdigest(),
                  job_id=job['job_id'], height=job['block_height'], old_error_branch_missed=True,
                  six_finalized_blocks_verified=True, mismatch_negative_rejected=True,
                  local_repair_completed=True, live_changes=0, commands=rows)
    (args.output / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({k: v for k, v in result.items() if k != 'commands'}))


if __name__ == '__main__':
    main()
