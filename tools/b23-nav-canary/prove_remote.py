#!/usr/bin/env python3
"""Prove one public NAV witness on the explicitly authorized existing Vast box."""
import argparse
import hashlib
import json
from pathlib import Path
import shlex
import subprocess

HOST='root@ssh8.vast.ai'
PORT='20628'
REMOTE='/workspace/b23-tee-nav'
SSH=['ssh','-o','BatchMode=yes','-o','ConnectTimeout=20','-o','ServerAliveInterval=30','-p',PORT,HOST]
SCP=['scp','-q','-P',PORT]
def command(argv,*,log=None):
    if log:
        with log.open('wb') as output:subprocess.run(argv,stdout=output,stderr=subprocess.STDOUT,check=True)
    else:subprocess.run(argv,check=True)
def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('capture','elf','native_cli','output'):p.add_argument('--'+name.replace('_','-'),type=Path,required=True)
    p.add_argument('--prover-cli',type=Path,help='Upload this CUDA-enabled CLI to the isolated job; preserve the shared prover CLI')
    args=p.parse_args();args.output.mkdir(parents=True,exist_ok=False,mode=0o700)
    assert json.loads((args.capture/'completion.json').read_bytes())['nativeVerified'] is True
    assert json.loads((args.capture/'final-inventory.json').read_bytes())['noOwnedBillableComputeRemaining'] is True
    witness=args.capture/'witness.json';profile=json.loads((args.capture/'profile.json').read_bytes())
    command([str(args.native_cli),'native','--witness',str(witness),'--output',str(args.output/'native-values.bin')],log=args.output/'native-verification.json')
    command([str(args.native_cli),'execute','--witness',str(witness),'--elf',str(args.elf),'--output',str(args.output/'guest-values.bin')],log=args.output/'guest-execution.log')
    identity=json.loads(subprocess.check_output([str(args.native_cli),'identity','--elf',str(args.elf)],stderr=subprocess.PIPE))
    assert identity['program_vkey']==profile['sp1_program_vkey']
    job='nav-'+hashlib.sha256(witness.read_bytes()).hexdigest()[:24];remote=REMOTE+'/'+job
    command([*SSH,'mkdir -m 700 '+shlex.quote(remote)])
    command([*SCP,str(witness),HOST+':'+remote+'/witness.json'])
    command([*SCP,str(args.elf),HOST+':'+remote+'/guest.elf'])
    prover='/workspace/b23-tee-nav/target/release/postfiat-b23-nav'
    if args.prover_cli:
        prover=remote+'/postfiat-b24-nav'
        command([*SCP,str(args.prover_cli),HOST+':'+prover])
        digest=hashlib.sha256(args.prover_cli.read_bytes()).hexdigest()
        command([*SSH,'chmod 700 '+shlex.quote(prover)+' && test "$(sha256sum '+shlex.quote(prover)+' | cut -d\" \" -f1)" = '+digest])
    # Shared bridge lock and matching SDK server version avoid concurrent GPU
    # work or replacement/stopping of the bridge's shared GPU server binary.
    script='set -e; cd '+shlex.quote(remote)+'; '
    script+='test "$(sha256sum guest.elf | cut -d\" \" -f1)" = '+identity['elf_sha256']+'; '
    script+='test "$(/root/.sp1/bin/sp1-gpu-server --version)" = 6.3.1; '
    script+='exec 9>>/workspace/b23-gpu/prover.lock; flock -w 600 9; '
    script+='RAYON_NUM_THREADS=16 timeout 1200 '+shlex.quote(prover)+' prove '
    script+='--elf guest.elf --witness witness.json --output proof > prove.log 2>&1'
    command([*SSH,script],log=args.output/'remote-command.log')
    for name in ('proof.bin','proof-calldata.bin','public-values.bin','proof-report.json'):
        command([*SCP,HOST+':'+remote+'/proof/'+name,str(args.output/name)])
    command([*SCP,HOST+':'+remote+'/prove.log',str(args.output/'prove.log')])
    report=json.loads((args.output/'proof-report.json').read_bytes())
    assert report['proof_verified'] and report['backend']=='cuda' and report['proof_bytes']==356
    assert report['elf_sha256']==identity['elf_sha256'] and report['program_vkey']==profile['sp1_program_vkey']
    assert (args.output/'native-values.bin').read_bytes()==(args.output/'public-values.bin').read_bytes()
    command([str(args.native_cli),'verify','--witness',str(witness),'--elf',str(args.elf),
        '--proof',str(args.output/'proof.bin')],log=args.output/'independent-verification.log')
    (args.output/'qualification.json').write_text(json.dumps(dict(passed=True,identity=identity,proof=report,remote_job=remote),indent=2)+'\n')
    print(json.dumps({'real_proof_verified':True,'elapsed_ms':report['elapsed_ms']}))
if __name__=='__main__':main()
