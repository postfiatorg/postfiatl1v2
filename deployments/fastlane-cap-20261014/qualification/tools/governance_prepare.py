from runner import *
import shutil
out=ROOT/'v2-governance';out.mkdir(mode=0o700)
shutil.copy2('/home/postfiatchad/.cache/release-repair-20260916/v2-governance/payload.json',out/'payload.json')
binary=ROOT/'binaries/candidate-1';data=ROOT/'working/validator-0'
committee=json.loads((out/'payload.json').read_text())['committee']
assert committee['schema']=='postfiat-fastpay-recovery-committee-v2'
ids=','.join(v['validator_id'] for v in committee['validators'])
def cli(name,args):
    rec=run(name,[binary,*args],timeout=180)
    if rec['status']!='PASS': raise RuntimeError(f'{name}: {rec["status"]}')
cli('governance-create',['fastpay-recovery-governance-bootstrap','--data-dir',data,'--validators',ids,'--support',ids,'--payload-file',out/'payload.json','--amendment-file',out/'amendment.json','--batch-file',out/'unsigned-batch.json'])
for i in range(6):
    cli(f'governance-sign-{i}',['governance-authorization-sign','--data-dir',data,'--amendment-file',out/'amendment.json','--validator',f'validator-{i}','--validator-key-file',ROOT/'isolated-signers'/f'validator-{i}'/'validator_keys.json','--proposal-slot','1021','--expires-at-height','1026','--authorization-file',out/f'authorization-{i}.json'])
cli('governance-assemble',['governance-amendment-assemble','--data-dir',data,'--amendment-file',out/'amendment.json','--authorization-files',','.join(str(out/f'authorization-{i}.json') for i in range(6)),'--proposal-slot','1021','--output',out/'signed-amendment.json'])
cli('governance-batch',['fastpay-recovery-governance-bootstrap-assemble','--data-dir',data,'--payload-file',out/'payload.json','--signed-amendment-file',out/'signed-amendment.json','--proposal-slot','1021','--batch-file',out/'signed-batch.json'])
print('Prepared six-authorized V2 rotation for disposable height 1021.',flush=True)
