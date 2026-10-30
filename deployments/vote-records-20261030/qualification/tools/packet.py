"""Collect the local qualification results of vote-records-20261030 into deployments/vote-records-20261030/qualification/
(index-v3-20261027 tools/packet.py). Local only; reads result files, runs nothing."""
import hashlib, json, pathlib, shutil, sys
Q = pathlib.Path('/home/postfiatchad/.cache/release-vote-records-20261030')
OUT = pathlib.Path(sys.argv[1]) / 'deployments/vote-records-20261030'
P = OUT / 'qualification'
for d in ('reports', 'logs', 'replays', 'tools', 'devnet', 'private-path'):
    (P / d).mkdir(parents=True, exist_ok=True)
load = lambda p, d=None: json.loads(pathlib.Path(p).read_text()) if pathlib.Path(p).exists() else d
for f in (Q / 'reports').glob('*.json'):
    shutil.copy2(f, P / 'reports' / f.name)
for f in (Q / 'logs').glob('*'):
    if f.is_file() and f.stat().st_size < 2_000_000 and (not f.name.startswith('node-build-') or f.name.endswith(('-dynamic.stdout', '-comment.stdout'))):
        shutil.copy2(f, P / 'logs' / f.name)
for name in ('history-run.json', 'rollback-run.json', 'new-build-service-receipt.json', 'operational-complete.json'):
    if (Q / name).exists():
        shutil.copy2(Q / name, P / name)
for h in ('924', '1153'):
    src = Q / f'replay-{h}'
    if not src.exists():
        continue
    dst = P / 'replays' / f'height-{h}'
    dst.mkdir(parents=True, exist_ok=True)
    for f in src.glob('*.txt'):
        shutil.copy2(f, dst / f.name)
    if (src / 'out').exists():
        for f in (src / 'out').rglob('*.json'):
            if f.name.endswith('.private.json') or f.stat().st_size > 2_000_000:
                continue
            rel = f.relative_to(src / 'out')
            (dst / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(f, dst / rel)
    if (src / 'harness.log').exists():
        (dst / 'harness-tail.log').write_text('\n'.join((src / 'harness.log').read_text().splitlines()[-40:]) + '\n')
for f in ('replays-driver.log', 'history-driver.log', 'build-driver.log', 'stage.log', 'systemd-analyze-verify.log', 'local-preflight-signed.json', 'devnet-driver.log'):
    if (Q / f).exists():
        shutil.copy2(Q / f, P / 'logs' / f)
for f in (Q / 'logs2').glob('*.log'):
    shutil.copy2(f, P / 'logs' / f.name)
for f in (Q / 'tools').glob('*.*'):
    shutil.copy2(f, P / 'tools' / f.name)
# Devnet (5a, step 4, 5c): the report, both vote records, the snapshot points files and the round-2 checks. No key files.
shutil.copy2(Q / 'devnet-report.json', P / 'devnet/devnet-report.json')
shutil.copy2(Q / 'devnet-after-round2-checks.json', P / 'devnet/after-round2-checks.json')
W = Q / 'devnet'
for i, h in ((1, 1), (2, 2)):
    f = W / f'validator-{i}/block-vote-records/height-{h:020d}.json'
    if f.exists(): shutil.copy2(f, P / 'devnet' / f'vote-record-height-{h}-validator-{i}.json')
for i in range(4):
    f = W / f'validator-{i}/private_pool_snapshot_points.json'
    if f.exists(): shutil.copy2(f, P / 'devnet' / f'snapshot-points-validator-{i}.json')
shutil.copy2(Q / 'private-off-report.json', P / 'private-path/private-off-report.json')
checks = {c['name']: c for c in load(Q / 'history-run.json', {'checks': []})['checks']}
print(json.dumps({k: (v.get('status'), (v.get('observed') or {}).get('state_root', '')[:8]) for k, v in checks.items()}, indent=1))
print('rollback', {c['name']: c.get('status') for c in load(Q / 'rollback-run.json', {'checks': []})['checks']})
print('services', load(Q / 'new-build-service-receipt.json', {}).get('result'))
