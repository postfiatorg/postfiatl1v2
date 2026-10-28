#!/bin/sh
# Task Node L1 validator connect. Download and inspect before running if preferred.
# Reads only the running node's configured key; never searches key backups.
set +x
set -eu
command -v python3 >/dev/null 2>&1 || { echo 'L1 connect requires python3.' >&2; exit 1; }
exec python3 - "$@" <<'PY'
import fcntl, glob, hashlib, json, os, pathlib, platform, re, resource, subprocess, sys, urllib.request

BASE = 'https://tasknode.postfiat.org'
HELPER_URL = 'https://178-156-143-199.sslip.io:18443/l1-registry/l1-validator-signer-linux-x86_64'
HELPER_SHA256 = '63c5e3d56556447dc06cbcd9fd0b4d8e620dc17068166bb51d3cdb41f50f6eca'

def fail(message):
    print('L1 validator connect: ' + message, file=sys.stderr)
    raise SystemExit(1)

def discover():
    candidates = set()
    for name in glob.glob('/proc/[0-9]*/cmdline'):
        try:
            args = pathlib.Path(name).read_bytes().decode().rstrip('\0').split('\0')
            if pathlib.Path(args[0]).name != 'postfiat-node' or 'transport-validator-serve' not in args:
                continue
            def flag(key):
                if args.count(key) != 1: raise ValueError()
                return args[args.index(key) + 1]
            key, data = flag('--key-file'), flag('--data-dir')
            if not pathlib.Path(key).is_absolute() or not pathlib.Path(data).is_absolute():
                raise ValueError()
            # node_state.json has a public JSON object followed by the node MAC.
            raw = (pathlib.Path(data) / 'node_state.json').read_text()
            node, _ = json.JSONDecoder().raw_decode(raw)
            validator_id = node['node_id']
            if not isinstance(validator_id, str) or not re.fullmatch(r'[A-Za-z0-9_-]{1,128}', validator_id):
                raise ValueError()
            candidates.add((key, validator_id))
        except (OSError, ValueError, IndexError, KeyError):
            continue
    if len(candidates) != 1:
        fail('Expected one running postfiat-node transport-validator-serve with an absolute --key-file and --data-dir. Run on the validator host as its operator.')
    return candidates.pop()

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise ValueError('redirect refused')

def request(url, payload=None, limit=8 * 1024 * 1024):
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request(url, data=data, headers={'Content-Type': 'application/json', 'User-Agent':'TaskNode-L1-connect/1'})
    with urllib.request.build_opener(NoRedirect()).open(req, timeout=45) as response:
        raw = response.read(limit + 1)
    if len(raw) > limit: raise ValueError('response too large')
    return raw

def main():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    if len(sys.argv) == 2 and sys.argv[1] == '--help':
        print('Usage: sh l1-validator.sh <Task Node connect token>\nRun on the L1 validator host as its operator. No ledger transaction is submitted.')
        return
    if len(sys.argv) == 2 and sys.argv[1] == '--discover':
        key, validator = discover()
        print(json.dumps({'validator_id': validator, 'configured_key_path': key}))
        return
    if len(sys.argv) != 2 or not re.fullmatch(r'vct_[0-9a-f]{64}', sys.argv[1]):
        fail('Copy a fresh command from Task Node Settings > Validator.')
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        fail('This helper currently supports Linux x86_64.')
    key, validator = discover()
    # Verify namespace access before fetching a challenge. No fallback to online signing.
    if subprocess.run(['unshare', '--net', '--', 'true'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
        fail('Network isolation requires root (or CAP_SYS_ADMIN). Run this command as root on the validator host.')
    binary = request(HELPER_URL)
    if hashlib.sha256(binary).hexdigest() != HELPER_SHA256:
        fail('Helper SHA-256 mismatch; nothing was signed.')
    challenge = json.loads(request(BASE + '/api/validators/connect/challenge', {'token':sys.argv[1]}, 65536))['challenge']
    if not isinstance(challenge,str) or not re.fullmatch(r'pf-tasknode:[A-Za-z0-9_-]{32,128}:[A-Za-z0-9_-]{1,200}:[0-9]{1,20}', challenge):
        fail('Server returned an invalid Task Node challenge.')
    # Helper lives in a sealed anonymous memory fd, not a disk file. Only it
    # opens the configured key. Secret material stays inside the offline child.
    fd = os.memfd_create('tasknode-l1-signer', os.MFD_ALLOW_SEALING)
    try:
        with os.fdopen(os.dup(fd), 'wb') as output: output.write(binary)
        os.fchmod(fd, 0o500)
        fcntl.fcntl(fd, fcntl.F_ADD_SEALS, fcntl.F_SEAL_WRITE | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_SEAL)
        result = subprocess.run(['unshare','--net','--','/proc/self/fd/' + str(fd),'--key-file',key,'--validator-id',validator],
                                input=challenge.encode(), stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, pass_fds=(fd,), timeout=30)
    finally:
        os.close(fd)
    if result.returncode: fail('Offline signing failed. Check the node key permissions, validator id and challenge expiry.')
    proof = json.loads(result.stdout)
    if set(proof) != {'validator_id','ml_dsa_public_key','signature'} or proof['validator_id'] != validator:
        fail('Invalid proof shape; nothing was sent.')
    for field, size in [('ml_dsa_public_key',3904),('signature',6618)]:
        if not isinstance(proof[field],str) or not re.fullmatch('[0-9a-f]{' + str(size) + '}',proof[field]):
            fail('Invalid proof encoding; nothing was sent.')
    reply = json.loads(request(BASE + '/api/validators/connect/complete', {'token':sys.argv[1], 'network':'l1-devnet', **proof}, 65536))
    if reply.get('ok') is not True: fail('Task Node refused the binding. Copy a fresh command or check validator eligibility.')
    print('Validator connected: ' + validator + ' (l1-devnet)')

try:
    main()
except Exception:
    # Exceptions/HTTP responses can contain the connect token. Never echo them.
    fail('Request or local setup failed. Check network access and copy a fresh command.')
PY
