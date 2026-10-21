"""Disposable 1020 originals with authenticated database-pointer relocation (fastlane-cap-20261014 tools/prepare.py; the 1020 originals are the 2026-09-28 relocated copies, the 2026-09-15 raw copies are deleted)."""
from runner import *
import shutil, hmac
os.umask(0o077)
old = pathlib.Path('/home/postfiatchad/.cache/release-repair-20260928')
prior = pathlib.Path('/home/postfiatchad/.cache/release-fastlane-cap-20261014')
RB = pathlib.Path('/home/postfiatchad/.local/lib/postfiat/releases/fastlane-cap-20261014/postfiat-node')
RB_SHA = 'b1b3dbb150d3596577df3bb3fe145103d90988cf71558c4d34a035fe4bb44405'
assert sha(RB) == RB_SHA, 'deployed fastlane-cap-20261014 binary hash mismatch'
for i in range(6):
    dst = ROOT/'working'/f'validator-{i}'
    assert not dst.exists()
    dst.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(['cp', '-a', str(old/'working'/f'validator-{i}'), str(dst)], check=True)
    ptr = dst/'transactional_generation.json'
    body, marker = ptr.read_bytes().rsplit(b'\npftmac1:', 1)
    key = (dst/'.integrity.key').read_bytes()
    mac = lambda b: hmac.new(key, b'postfiat.storage.state-file.v1:state file\0'+b, hashlib.sha3_384).hexdigest()
    assert len(key) == 48 and hmac.compare_digest(mac(body), marker.strip().decode())
    before = json.loads(body); after = dict(before)
    generation = pathlib.Path(before['database_directory']).name
    assert generation == 'transactional-generation' or generation.startswith('transactional-generation-')
    assert (dst/generation/before['database_file']).is_file()
    after['database_directory'] = str(dst/generation)
    new = json.dumps(after, indent=2).encode()
    ptr.write_bytes(new+b'\npftmac1:'+mac(new).encode()+b'\n')
    write(ROOT/'reports'/f'relocation-working-validator-{i}.json', dict(original_mac_verified=True, changed_field='database_directory', logical_state_changed=False, source=str(old/'working'/f'validator-{i}'), before=before, after=after))
for i in range(6):
    d = ROOT/'isolated-signers'/f'validator-{i}'; d.mkdir(parents=True, exist_ok=True)
    shutil.copy2(prior/'isolated-signers'/f'validator-{i}'/'validator_keys.json', d/'validator_keys.json')
shutil.copy2(prior/'topology.json', ROOT/'topology.json')
shutil.copy2(RB, ROOT/'binaries/rollback')
assert sha(ROOT/'binaries/rollback') == RB_SHA
write(ROOT/'preparation.json', dict(source_commit=SOURCE, completed_at=now(), scope='new disposable clones of the 1020 originals; authenticated local database-pointer relocation only', working_source=str(old/'working'), signers_source=str(prior/'isolated-signers'), old_binary_source=str(RB), old_binary_sha256=RB_SHA, topology_sha256=sha(ROOT/'topology.json'), live_mutations=False))
print(f'{now()} preparation complete', flush=True)
