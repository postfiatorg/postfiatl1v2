#!/usr/bin/env python3
"""Retain encrypted archive checkpoints during an already-authorized NAV run.

Read-only SSH. No AWS API, no new instance, no plaintext recovery, no chain RPC.
Also protects an older running runner from losing a completed OAuth archive if
an unrelated later step fails before it downloads the archive.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import time

def main():
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--run',type=Path,required=True)
    ap.add_argument('--host',required=True)
    args=ap.parse_args();os.umask(0o077);root=args.run.resolve()
    manifest=json.loads((root/'manifest.json').read_bytes());cutoff=manifest['cutoff_unix_ms']/1000
    host=args.host
    if not all(c in '0123456789.' for c in host):raise ValueError('unexpected host pin')
    ssh=['ssh','-o','BatchMode=yes','-o','ConnectTimeout=3','-o','StrictHostKeyChecking=yes',
         '-o','UserKnownHostsFile='+str(root/'known_hosts'),'-i',str(root/'ssh_key'),'ec2-user@'+host]
    while time.time()<cutoff-2:time.sleep(min(1,cutoff-2-time.time()))
    checkpoints=0
    while time.time()<cutoff+150:
        result=subprocess.run([*ssh,'test -d /home/ec2-user/nav/output/archive && tar -cf - -C /home/ec2-user/nav/output archive'],
            stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=12)
        if result.returncode==0 and result.stdout:
            temporary=root/'watch-archive.tmp';temporary.write_bytes(result.stdout)
            temporary.replace(root/'watch-encrypted-archive.tar');checkpoints+=1
        if (root/'completion.json').exists():break
        if (root/'failure.json').exists() and checkpoints:break
        time.sleep(2)
    (root/'archive-watch.json').write_text(json.dumps({'encrypted_checkpoints':checkpoints,'chain_writes':0,'aws_api_calls':0}))

if __name__=='__main__':main()
