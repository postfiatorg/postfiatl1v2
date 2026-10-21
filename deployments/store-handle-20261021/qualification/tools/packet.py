"""Copy the local qualification records into deployments/store-handle-20261021/qualification/ and write
qualification.json (fastlane-cap-20261014 tools/packet.py)."""
import datetime, hashlib, json, pathlib, re, shutil, sys
Q = pathlib.Path('/home/postfiatchad/.cache/release-store-handle-20261021')
E = pathlib.Path('/home/postfiatchad/.postfiat/deployments/store-handle-20261021/evidence')
OUT = pathlib.Path(sys.argv[1])/'deployments/store-handle-20261021/qualification'
OUT.mkdir(parents=True, exist_ok=True)
sha = lambda p: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
for d in ('logs', 'reports', 'tools', 'gates'):
    if (Q/d).exists(): shutil.copytree(Q/d, OUT/d, dirs_exist_ok=True, ignore=shutil.ignore_patterns('__pycache__', '*.tmp'))
for f in ('history-run.json', 'preparation.json', 'new-build-service-receipt.json', 'rollback-run.json', 'local-preflight-signed.json',
          'ci-main.json', 'ci-release.json', 'ci-jobs.txt', 'build-driver.log', 'history-driver.log', 'focused-driver.log', 'focused2-driver.log', 'prepare-driver.log'):
    if (Q/f).exists(): shutil.copy2(Q/f, OUT/(f if not f.endswith('-driver.log') else 'logs/'+f))
(OUT/'migration').mkdir(exist_ok=True)
for f in sorted((Q/'scratch-migration').glob('*')):
    if f.is_file() and not f.name.endswith('.stderr'): shutil.copy2(f, OUT/'migration'/f.name)
(OUT/'stage').mkdir(exist_ok=True)
for f in ('manifest-validity.txt', 'stage-command.stdout', 'manifest-create.stdout'):
    if (E/f).exists(): shutil.copy2(E/f, OUT/'stage'/f)
if (Q/'verified-record/rpc.deployment-verified.json').exists():
    shutil.copy2(Q/'verified-record/rpc.deployment-verified.json', OUT/'stage/rpc.deployment-verified.json')
codes = dict(re.findall(r'EXIT (\d+) (\S+)', (Q/'focused-driver.log').read_text() + (Q/'focused2-driver.log').read_text()))
codes = {v: k for k, v in re.findall(r'EXIT (\d+) (\S+)', (Q/'focused-driver.log').read_text() + (Q/'focused2-driver.log').read_text())}
suites = []
for name, code in sorted(codes.items()):
    log = Q/'logs'/f'focused-{name}.log'
    text = log.read_text(errors='replace')
    passed = sum(int(m) for m in re.findall(r'test result: ok\. (\d+) passed', text))
    if passed == 0: continue  # the two lib-target filters that matched nothing (the tests live in the bin target)
    suites.append(dict(name=f'focused-{name}', log=f'logs/focused-{name}.log', passed=passed, status='PASS' if code == '0' and 'FAILED' not in text else 'FAIL'))
for stale in ('focused-node-lib-committee-migration.log', 'focused-node-lib-fastswap-service.log'):
    (OUT/'logs'/stale).unlink(missing_ok=True)
(OUT/'focused-suites.json').write_text(json.dumps(dict(cwd='/home/postfiatchad/repos/postfiatl1v2 (main at 86706699, default test profile)', suites=suites), indent=2)+'\n')
history = json.loads((Q/'history-run.json').read_text())
latest = {c['name']: c for c in history['checks']}
rollback = json.loads((Q/'rollback-run.json').read_text()) if (Q/'rollback-run.json').exists() else {'checks': []}
mig = json.loads((Q/'scratch-migration/migration-runs.json').read_text())
summary = dict(
    release_id='store-handle-20261021', source_commit=history['source_commit'], binary_sha256=history['binary_sha256'],
    rollback_binary_sha256=history['rollback_binary_sha256'], generated_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),
    builds=json.loads((Q/'node-builds.json').read_text())['result'],
    history={k: dict(status=v['status'], observed=v.get('observed'), expected=v.get('expected'), note=v.get('note')) for k, v in latest.items()},
    new_build_services=json.loads((Q/'new-build-service-receipt.json').read_text())['result'],
    rollback={c['name']: c['status'] for c in rollback['checks']},
    migration_scratch={r['name']: dict(exit_code=r['exit_code'], outcome=r.get('outcome'), error=r.get('stderr')) for r in mig['runs']},
    focused_suites={s['name']: s['status'] for s in suites},
    local_preflight=json.loads((Q/'local-preflight-signed.json').read_text())['result'],
    live_mutations=False, host_contact=False,
    not_done_today=['canary backup', 'fleet before-state', 'rollout', 'migration'])
(OUT/'qualification.json').write_text(json.dumps(summary, indent=2)+'\n')
files = sorted(p for p in OUT.rglob('*') if p.is_file() and p.name != 'SHA256SUMS')
(OUT/'SHA256SUMS').write_text(''.join(f'{sha(p)}  {p.relative_to(OUT)}\n' for p in files))
print(len(files), 'files')
