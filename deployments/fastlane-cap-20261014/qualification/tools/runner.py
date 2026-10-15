import datetime, hashlib, json, os, pathlib, resource, signal, subprocess, sys, time
ROOT = pathlib.Path('/home/postfiatchad/.cache/release-fastlane-cap-20261014')
REPO = pathlib.Path('/home/postfiatchad/.cache/release-fastlane-cap-20261014/source-1')
SOURCE = '8bed9b9073f97d0c45d0babb9756636516a2e3f7'
STARTED = datetime.datetime(2026, 10, 15, 9, 42, 0, tzinfo=datetime.timezone.utc)
DEADLINE = STARTED + datetime.timedelta(minutes=60)
OPERATIONS_END = DEADLINE - datetime.timedelta(minutes=10)
ENV = dict(CARGO_BUILD_JOBS='2', CARGO_INCREMENTAL='0', CARGO_TERM_COLOR='never', TMPDIR=str(ROOT/'tmp'), RUST_TEST_THREADS='2', RAYON_NUM_THREADS='2', OMP_NUM_THREADS='2', MALLOC_ARENA_MAX='2', PYTHONDONTWRITEBYTECODE='1')
AS_LIMIT = 20*1024**3
def now(): return datetime.datetime.now(datetime.timezone.utc).isoformat()
def write(path, data):
    path = pathlib.Path(path); path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix+'.tmp'); temp.write_text(json.dumps(data, indent=2)+'\n'); temp.replace(path)
def sha(path):
    h = hashlib.sha256()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1048576), b''): h.update(chunk)
    return h.hexdigest()
def available():
    for line in pathlib.Path('/proc/meminfo').read_text().splitlines():
        if line.startswith('MemAvailable:'): return int(line.split()[1])*1024
def limits():
    resource.setrlimit(resource.RLIMIT_AS, (AS_LIMIT, AS_LIMIT))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
BACKGROUND = ROOT/'full-suite.sid'
def cargo_running():
    bg = BACKGROUND.read_text().strip() if BACKGROUND.exists() else None
    for p in pathlib.Path('/proc').glob('[0-9]*/comm'):
        try:
            if p.read_text().strip() == 'cargo':
                if bg and (p.parent/'stat').read_text().rsplit(')', 1)[1].split()[3] == bg: continue
                return True
        except (FileNotFoundError, ProcessLookupError, PermissionError): pass
    return False
def run(name, cmd, cwd=REPO, extra=None, timeout=1800, separate=True, deadline=None):
    if str(cmd[0]) == 'cargo':
        waited = 0
        while cargo_running():
            if waited == 0: print(f'{now()} WAIT {name}: another cargo process is running', flush=True)
            time.sleep(2); waited += 2
    env = os.environ.copy(); env.update(ENV); env.update(extra or {})
    out = ROOT/'logs'/f'{name}.stdout'; err = ROOT/'logs'/f'{name}.stderr'
    rec = dict(name=name, command=[str(x) for x in cmd], cwd=str(cwd), started_at=now(), status='RUNNING', stdout=str(out), stderr=str(err), environment_overrides=ENV | (extra or {}), address_space_limit_bytes=AS_LIMIT, minimum_available_memory_bytes=4*1024**3)
    write(ROOT/'reports'/f'{name}.json', rec)
    remaining = (deadline or OPERATIONS_END).timestamp() - time.time()
    if remaining <= 0:
        rec.update(status='NOT_RUN_TIMEBOX', reason='time box reserved for packet publication', completed_at=now())
        write(ROOT/'reports'/f'{name}.json', rec)
        print(f'{now()} NOT_RUN_TIMEBOX {name}', flush=True)
        return rec
    timeout = min(timeout, remaining)
    start = time.monotonic(); minimum = available()
    print(f'{now()} START {name}', flush=True)
    with out.open('wb') as stdout, err.open('wb') as stderr:
        if minimum < 4*1024**3:
            rec.update(status='BLOCKED_MEMORY', exit_code=None)
        else:
            p = subprocess.Popen(rec['command'], cwd=cwd, env=env, stdout=stdout, stderr=stderr if separate else stdout, start_new_session=True, preexec_fn=limits)
            rec['pid'] = p.pid; write(ROOT/'reports'/f'{name}.json', rec)
            cause = None
            while p.poll() is None:
                minimum = min(minimum, available())
                if minimum < 4*1024**3: cause = 'STOPPED_MEMORY'
                if time.monotonic()-start > timeout: cause = 'TIMEOUT'
                if cause:
                    os.killpg(p.pid, signal.SIGTERM)
                    try: p.wait(timeout=10)
                    except subprocess.TimeoutExpired: os.killpg(p.pid, signal.SIGKILL); p.wait()
                    break
                time.sleep(1)
            rec.update(exit_code=p.returncode, status=cause or ('PASS' if p.returncode == 0 else 'FAIL'))
    rec.update(completed_at=now(), elapsed_seconds=round(time.monotonic()-start, 1), minimum_available_memory_observed_bytes=minimum)
    write(ROOT/'reports'/f'{name}.json', rec); print(f'{now()} {rec["status"]} {name} ({rec["elapsed_seconds"]}s)', flush=True)
    return rec
import contextlib, fcntl
@contextlib.contextmanager
def lock(name, slots=1):
    (ROOT/'locks').mkdir(exist_ok=True)
    while True:
        for i in range(slots):
            f = open(ROOT/'locks'/f'{name}-{i}', 'w')
            try: fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError: f.close(); continue
            try: yield
            finally: fcntl.flock(f, fcntl.LOCK_UN); f.close()
            return
        time.sleep(2)
def wait_memory(need=8*1024**3, limit=900):
    end = time.time()+limit
    while available() < need and time.time() < end: time.sleep(5)
    return available() >= need
