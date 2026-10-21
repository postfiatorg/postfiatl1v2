"""fastswap-committee-migrate with the release executable on scratch stores built like the 2026-10-16 tests
(fastswap_committee_migration_tests.rs node()): dry run then real run. Local only."""
import hashlib, json, pathlib, subprocess, sys
Q = pathlib.Path('/home/postfiatchad/.cache/release-store-handle-20261021'); M = Q/'scratch-migration'
B = Q/'binaries/candidate-1'
REAL = pathlib.Path(sys.argv[1])  # this packet's observed/bridge-activation/
stores = json.loads((M/'scratch-stores.json').read_text())
sha = lambda p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def tree(d):
    d = pathlib.Path(d)/'fastswap-v1'
    return {str(p.relative_to(d)): sha(p) for p in sorted(d.rglob('*')) if p.is_file() and p.name != 'fastswap-v1.lock'}
def run(name, args):
    p = subprocess.run([str(B), 'fastswap-committee-migrate', *args], capture_output=True, text=True)
    (M/f'{name}.stdout').write_text(p.stdout); (M/f'{name}.stderr').write_text(p.stderr)
    rec = dict(name=name, command=['postfiat-node', 'fastswap-committee-migrate', *[a.replace(str(Q), '$Q') for a in args]], exit_code=p.returncode)
    try:
        r = json.loads(p.stdout)
        rec.update(outcome=r.get('outcome'), load_path=r.get('load_path'), validator_id=r.get('validator_id'), notes=r.get('notes'),
                   before_epoch=r['before']['committee_epoch'], after_epoch=r['after']['committee_epoch'],
                   after_root=r['after']['committee_root'], after_fences=r['after']['prepare_fences'],
                   after_anchored=r['after']['anchored_checkpoints'], verification=r.get('verification'),
                   backup_entries=len(r.get('backup_manifest') or []))
    except Exception:
        rec['stderr'] = (p.stderr.splitlines() or [''])[0]
    return rec
out = dict(binary_sha256=sha(B), real_files={f.name: sha(f) for f in sorted(REAL.glob('7.*.fastlane-primary.json'))}, runs=[])
for name in ('snapshot-own-files', 'legacy-own-files'):
    s = stores[name]; before = tree(s['data_dir'])
    base = ['--data-dir', s['data_dir'], '--committee-file', s['committee_file'], '--anchor-file', s['anchor_file']]
    dry = run(f'{name}-dry', base + ['--dry-run']); dry['store_unchanged'] = tree(s['data_dir']) == before
    real = run(f'{name}-real', base + ['--backup-dir', s['root'] + '/backup'])
    real['expected_root'] = s['next_committee_root']
    real['pass'] = dry.get('outcome') == 'would-migrate' and dry['store_unchanged'] and real.get('outcome') == 'migrated' \
        and real.get('after_epoch') == 2 and real.get('after_root') == s['next_committee_root'] \
        and f"committee epoch 2, root {s['next_committee_root'][:8]}" in (real.get('verification') or '') and 'enabled true' in real.get('verification', '')
    again = run(f'{name}-second-dry', base + ['--dry-run'])
    out['runs'] += [dry, real, again]
s = stores['snapshot-real-files']; before = tree(s['data_dir'])
base = ['--data-dir', s['data_dir'], '--committee-file', str(REAL/'7.2-activate-committee.fastlane-primary.json'), '--anchor-file', str(REAL/'7.1-anchor-checkpoint.fastlane-primary.json')]
dry = run('real-files-dry', base + ['--dry-run'])
real = run('real-files-real', base + ['--backup-dir', s['root'] + '/backup'])
real['store_unchanged'] = tree(s['data_dir']) == before; real['backup_written'] = (pathlib.Path(s['root'])/'backup').exists()
out['runs'] += [dry, real]
(M/'migration-runs.json').write_text(json.dumps(out, indent=2) + '\n')
for r in out['runs']:
    print(r['name'], r['exit_code'], r.get('outcome'), r.get('verification') or r.get('stderr', '')[:300], r.get('notes') or '')
