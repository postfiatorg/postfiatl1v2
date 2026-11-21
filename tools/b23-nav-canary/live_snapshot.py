"""Fresh read-only NodeStore export from validator 0; no service mutation."""
import hashlib
from pathlib import Path
import shlex
import subprocess
import uuid

HOST='root@64.176.220.75'
def export_snapshot(binary:Path,output:Path):
    digest=hashlib.sha256(binary.read_bytes()).hexdigest()
    remote='/tmp/b23-tee-nav-canary-'+digest[:16]
    target='/tmp/b23-tee-nav-export-'+uuid.uuid4().hex+'.json'
    subprocess.run(['scp','-q',str(binary),HOST+':'+remote],check=True)
    cmd='chmod 755 '+shlex.quote(remote)+'; sha256sum '+shlex.quote(remote)
    actual=subprocess.check_output(['ssh','-o','BatchMode=yes',HOST,cmd]).decode().split()[0]
    if actual!=digest:raise ValueError('export binary hash mismatch')
    subprocess.run(['ssh','-o','BatchMode=yes',HOST,shlex.quote(remote)+' export --data-dir /var/lib/postfiat/validator-0 --output '+shlex.quote(target)],check=True)
    subprocess.run(['scp','-q',HOST+':'+target,str(output)],check=True)
