#!/usr/bin/env python3
"""Prove only the actual Phase-6 finalized export on the existing shared GPU."""
import os,shlex
from archive import OUT,ELF,PROVER
from sign import *
HOST='root@ssh8.vast.ai';PORT='20628'
SSH=['ssh','-o','BatchMode=yes','-o','ConnectTimeout=20','-o','ServerAliveInterval=30','-p',PORT,HOST]
SCP=['scp','-q','-P',PORT]
def main():
    os.umask(0o077);a=OUT/'real-export/archive';assert load(OUT/'real-export/result.json')['passed'] and load(a/'result.json')['real_live_finality']
    d=OUT/'real-receipt-proof';d.mkdir(mode=0o700,exist_ok=False);witness=a/'receipt-witness-v2.json';digest=hashlib.sha256(witness.read_bytes()).hexdigest();remote='/workspace/b24-real-receipt-'+digest[:24]
    def run(cmd,log=None):
        if log:
            with (d/log).open('w') as out:subprocess.run(cmd,stdout=out,stderr=subprocess.STDOUT,check=True)
        else:subprocess.run(cmd,check=True)
    run([*SSH,'mkdir -m 700 '+shlex.quote(remote)])
    for path,name in [(witness,'witness.json'),(ELF,'receipt-v2.elf')]:run([*SCP,str(path),HOST+':'+remote+'/'+name])
    binary='/workspace/b23-pool-phase2/pftl-uniswap-prover'
    # Lock includes all identity checks, setup, proving, and internal verification.
    script='set -eu\nexec 9>>/workspace/b23-gpu/prover.lock\nflock -w 600 9\ncd '+shlex.quote(remote)+'\n'
    script+='test "$(/root/.sp1/bin/sp1-gpu-server --version)" = 6.3.1\n'
    for path,sha in [('witness.json',digest),('receipt-v2.elf',hashlib.sha256(ELF.read_bytes()).hexdigest()),(binary,hashlib.sha256(PROVER.read_bytes()).hexdigest())]:
        script+='test "$(sha256sum '+shlex.quote(path)+' | cut -d\" \" -f1)" = '+sha+'\n'
    script+=shlex.join([binary,'program-info','--elf','receipt-v2.elf','--output','program-info.json'])+'\n'
    script+='SP1_PROVER=cuda RAYON_NUM_THREADS=4 timeout 1800 '+shlex.join([binary,'receipt','--witness','witness.json','--elf','receipt-v2.elf','--proof-program-version','2','--require-prover','cuda','--prove','--output-dir','proof'])+' > prove.log 2>&1\n'
    (d/'remote-command.sh').write_text(script);save(d/'inputs.json',dict(remote=remote,witness_sha256=digest,elf_sha256=hashlib.sha256(ELF.read_bytes()).hexdigest(),prover_sha256=hashlib.sha256(PROVER.read_bytes()).hexdigest(),real_live_export=True,new_GPU_rental=False))
    run([*SSH,script],'remote.log')
    for name in ('proof.bin','proof-calldata.bin','public-values.bin','proof-report.json','execute-report.json'):run([*SCP,HOST+':'+remote+'/proof/'+name,str(d/name)])
    for name in ('program-info.json','prove.log'):run([*SCP,HOST+':'+remote+'/'+name,str(d/name)])
    report=load(d/'proof-report.json');info=load(d/'program-info.json');values=(d/'public-values.bin').read_bytes();proof=(d/'proof-calldata.bin').read_bytes()
    assert report['program_vkey']=='0x0060d83567657466eaac782e0be174b57277df6ac349f42288852a93d72f0fe1' and report['prover_backend']=='cuda' and report['proof_mode']=='groth16' and report['proof_program_version']==2
    assert values==(a/'receipt-execute/public-values.bin').read_bytes() and len(values)==1120 and len(proof)==356 and proof[:4].hex()=='4388a21c'
    save(d/'qualification.json',dict(passed=True,real_live_export=True,real_archived_finality=True,native_and_frozen_guest_public_values_match=True,internal_proof_verification=True,independent_mainnet_fork_pending=True,report=report,program_info=info))
    print(json.dumps(load(d/'qualification.json')),flush=True)
if __name__=='__main__':main()
