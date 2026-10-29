"""Run inside a read-only, network-disabled test container with synthetic keys."""
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


def main():
    binary, key_path = sys.argv[1:]
    before = hashlib.sha256(Path(key_path).read_bytes()).hexdigest()
    challenge = 'pf-tasknode:' + '1'*32 + ':acct_fixture:' + str(int(datetime.datetime.now().timestamp()) + 900)
    data = Path(binary).read_bytes()
    fd = os.memfd_create('test-l1-signer', os.MFD_ALLOW_SEALING)
    with os.fdopen(os.dup(fd), 'wb') as stream: stream.write(data)
    os.fchmod(fd,0o500)
    fcntl.fcntl(fd,fcntl.F_ADD_SEALS,fcntl.F_SEAL_WRITE|fcntl.F_SEAL_GROW|fcntl.F_SEAL_SHRINK|fcntl.F_SEAL_SEAL)
    result=subprocess.run(['/proc/self/fd/'+str(fd),'--key-file',key_path,'--validator-id','validator-fixture'],input=challenge.encode(),capture_output=True,pass_fds=(fd,),check=True)
    proof=json.loads(result.stdout)
    assert set(proof)=={'validator_id','ml_dsa_public_key','signature'}
    assert len(proof['ml_dsa_public_key'])==3904 and len(proof['signature'])==6618
    assert hashlib.sha256(Path(key_path).read_bytes()).hexdigest()==before
    assert not result.stderr
    print(json.dumps({'message':challenge,'context':'postfiat-l1-v2/tasknode-validator-binding/v1',**proof}))


if __name__ == '__main__':
    main()
