"""Collect the local qualification results of index-v3-20261027 into deployments/index-v3-20261027/qualification/
and fill the result placeholders of the sheet and RELEASE-ID.txt. Local only; reads result files, runs nothing."""
import hashlib, json, pathlib, re, shutil, sys
Q = pathlib.Path('/home/postfiatchad/.cache/release-index-v3-20261027')
OUT = pathlib.Path(sys.argv[1]) / 'deployments/index-v3-20261027'
P = OUT / 'qualification'
for d in ('reports', 'logs', 'replays', 'tools', 'index-check'):
    (P / d).mkdir(parents=True, exist_ok=True)


def load(path, default=None):
    try:
        return json.loads(pathlib.Path(path).read_text())
    except Exception:
        return default


def sha(path): return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


lines = []
builds = load(Q / 'node-builds.json', {})
hashes = [b.get('binary_sha256') for b in builds.get('builds', [])]
history = load(Q / 'history-run.json', {'checks': []})
checks = {c['name']: c for c in history.get('checks', [])}
rollback = load(Q / 'rollback-run.json', {'checks': []})
services = load(Q / 'new-build-service-receipt.json', {})
index = load(Q / 'index-check/index-v3-check.json', {})

# Copy the small result files; logs of the history checks are copied as they are (no secrets: stdout/stderr of verify commands).
for f in (Q / 'reports').glob('*.json'):
    if not f.name.startswith('.stale'):
        shutil.copy2(f, P / 'reports' / f.name)
for f in (Q / 'logs').glob('*'):
    if f.is_file() and f.stat().st_size < 2_000_000 and not f.name.startswith('node-build-') or f.name.endswith(('-dynamic.stdout', '-comment.stdout')):
        shutil.copy2(f, P / 'logs' / f.name)
for name in ('history-run.json', 'rollback-run.json', 'new-build-service-receipt.json', 'operational-complete.json'):
    if (Q / name).exists():
        shutil.copy2(Q / name, P / name)
if (Q / 'index-check/index-v3-check.json').exists():
    shutil.copy2(Q / 'index-check/index-v3-check.json', P / 'index-check/index-v3-check.json')
for h in ('924', '1137'):
    src = Q / f'replay-{h}'
    if not src.exists():
        continue
    dst = P / 'replays' / f'height-{h}'
    dst.mkdir(parents=True, exist_ok=True)
    for f in src.glob('*.txt'):
        shutil.copy2(f, dst / f.name)
    if (src / 'out').exists():
        for f in (src / 'out').rglob('*.json'):
            if f.name.endswith('.private.json'):
                continue
            rel = f.relative_to(src / 'out')
            (dst / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(f, dst / rel)
    if (src / 'harness.log').exists():
        tail = (src / 'harness.log').read_text().splitlines()[-40:]
        (dst / 'harness-tail.log').write_text('\n'.join(tail) + '\n')
for f in ('focused-suites.log', 'replays-driver.log', 'history-driver.log', 'build-driver.log', 'stage.log', 'systemd-analyze-verify.log', 'local-preflight-signed.json'):
    if (Q / f).exists():
        shutil.copy2(Q / f, P / 'logs' / f)
for f in (Q / 'tools').glob('*.*'):
    shutil.copy2(f, P / 'tools' / f.name)


def status(name):
    c = checks.get(name)
    return c.get('status', 'NOT RUN') if c else 'NOT RUN'


def obs(name):
    c = checks.get(name) or {}
    o = c.get('observed') or {}
    return f"{o.get('block_height')} / {str(o.get('block_tip_hash'))[:8]}… / {str(o.get('state_root'))[:8]}…" if o else '—'


def replay(h):
    rep = None
    for f in sorted((Q / f'replay-{h}' / 'out').rglob('*.json')) if (Q / f'replay-{h}' / 'out').exists() else []:
        d = load(f, {})
        if isinstance(d, dict) and 'matched' in d:
            rep = d
    return rep


json.dump(dict(builds=hashes, history={k: v.get('status') for k, v in checks.items()}), sys.stdout, indent=1)
print()
r924, r1137 = replay('924'), replay('1137')
print('replay 924', r924 and (r924.get('matched'), r924.get('source_height'), r924.get('tip_hash', '')[:8], r924.get('state_root', '')[:8]))
print('replay 1137', r1137 and (r1137.get('matched'), r1137.get('source_height'), r1137.get('tip_hash', '')[:8], r1137.get('state_root', '')[:8]))
print('index', json.dumps(index.get('checks', {}), indent=1)[:3000])
print('rollback', {c['name']: c.get('status') for c in rollback.get('checks', [])})
print('services', services.get('result'), services.get('error'))
