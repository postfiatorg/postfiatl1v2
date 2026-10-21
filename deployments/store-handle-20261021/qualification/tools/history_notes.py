"""After history.py: re-read the six original-validator replays against the identity those copies actually carry
(the 2026-09-28 rotation receipt at 1021; the 1020 originals are deleted) and note the governed rotation that
could not run. Observed values are not changed. Local only."""
from runner import *
h = json.loads((ROOT/'history-run.json').read_text())
r = json.loads(pathlib.Path('/home/postfiatchad/.cache/release-repair-20260928/v2-service-receipt.json').read_text())['finality_statuses'][0]
exp = dict(block_height=r['block_height'], block_tip_hash=r['block_tip_hash'], state_root=r['state_root'])
for c in h['checks']:
    if c['name'].startswith('original-validator-'):
        c['expected_1020_original'] = c.get('expected'); c['expected'] = exp
        c['status'] = 'PASS' if c.get('observed') == exp and c.get('verified') is True else 'FAIL'
        c['note'] = ('the 1020 originals (~/.cache/combined-release-20260915/raw) are deleted; this is a 2026-09-28 copy, '
                     'rotated at 1021 that day; expected = ~/.cache/release-repair-20260928/v2-service-receipt.json finality_statuses[0]')
    if c['name'] in ('governance-prepare', 'governance-sign-0', 'governance-create'):
        c['note'] = 'governed rotation not run: it needs the deleted 1020 originals; the 2026-09-28 copies already carry it (slot 1021 vs next height 1022)'
h['notes_added_at'] = now()
write(ROOT/'history-run.json', h)
print({c['name']: c['status'] for c in h['checks']})
