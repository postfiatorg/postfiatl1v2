"""Copy the local qualification records into deployments/fastlane-cap-20261014/qualification/ and write qualification.json
(signer-rotation-20261007 tools/packet.py)."""
import datetime, hashlib, json, pathlib, re, shutil, sys
Q = pathlib.Path('/home/postfiatchad/.cache/release-fastlane-cap-20261014')
E = pathlib.Path('/home/postfiatchad/.postfiat/deployments/fastlane-cap-20261014/evidence')
OUT = pathlib.Path(sys.argv[1])/'deployments/fastlane-cap-20261014/qualification'
OUT.mkdir(parents=True, exist_ok=True)
sha = lambda p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
for d in ('logs', 'reports', 'tools', 'gates'):
    if (Q/d).exists(): shutil.copytree(Q/d, OUT/d, dirs_exist_ok=True, ignore=shutil.ignore_patterns('__pycache__', '*.tmp'))
for f in ('history-run.json', 'preparation.json', 'v2-service-receipt.json', 'local-preflight-signed.json', 'anchor-check.json',
          'ci-main.json', 'ci-release.json', 'ci-jobs.txt', 'build-driver.log', 'history-driver.log'):
    if (Q/f).exists(): shutil.copy2(Q/f, OUT/(f if not f.endswith('-driver.log') else 'logs/'+f))
(OUT/'anchor-check').mkdir(exist_ok=True)
for f in sorted((Q/'anchor-check').iterdir()):
    if not f.name.endswith('.request.json'): shutil.copy2(f, OUT/'anchor-check'/f.name)
(OUT/'stage').mkdir(exist_ok=True)
for f in ('manifest-validity.txt', 'stage-command.stdout', 'manifest-create.stdout'):
    if (E/f).exists(): shutil.copy2(E/f, OUT/'stage'/f)
shutil.copy2(Q/'verified-record/rpc.deployment-verified.json', OUT/'stage/rpc.deployment-verified.json')
suites = []
for log in sorted((Q/'logs').glob('focused-*.log')):
    text = log.read_text(errors='replace')
    passed = sum(int(m) for m in re.findall(r'test result: ok\. (\d+) passed', text))
    passed += sum(int(m) for m in re.findall(r'^Ran (\d+) test', text, re.M)) if 'python' in log.name else 0
    code = re.findall(r'^exit=(\d+)', text, re.M)
    suites.append(dict(name=log.stem, log=f'logs/{log.name}', passed=passed, status='PASS' if code and code[-1] == '0' and 'FAILED' not in text else 'FAIL'))
(OUT/'focused-suites.json').write_text(json.dumps(dict(cwd='/home/postfiatchad/repos/postfiatl1v2 (main at 8bed9b90, default test profile)', suites=suites), indent=2)+'\n')
history = json.loads((Q/'history-run.json').read_text())
latest = {c['name']: c for c in history['checks']}
summary = dict(
    release_id='fastlane-cap-20261014', source_commit=history['source_commit'], binary_sha256=history['binary_sha256'],
    rollback_binary_sha256=history['rollback_binary_sha256'], generated_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
    builds=json.loads((Q/'node-builds.json').read_text())['result'],
    history={k: dict(status=v['status'], observed=v.get('observed'), expected=v.get('expected')) for k, v in latest.items()},
    anchor_check=json.loads((Q/'anchor-check.json').read_text())['result'],
    focused_suites={s['name']: s['status'] for s in suites},
    local_preflight=json.loads((Q/'local-preflight-signed.json').read_text())['result'],
    live_mutations=False, host_contact=False,
    not_done_today=['canary backup', 'fleet before-state', 'rollout', 'bridge activation'])
(OUT/'qualification.json').write_text(json.dumps(summary, indent=2)+'\n')
files = sorted(p for p in OUT.rglob('*') if p.is_file() and p.name != 'SHA256SUMS')
(OUT/'SHA256SUMS').write_text(''.join(f'{sha(p)}  {p.relative_to(OUT)}\n' for p in files))
print(len(files), 'files')
