"""Offline review of budget, secret release, build context and cleanup behavior."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import time

import pytest

from test_nav_reading import manifest

REPO = Path(__file__).resolve().parents[1]
AWS = REPO / "operator"
sys.path.insert(0, str(AWS))
spec = importlib.util.spec_from_file_location("nav_reading_run", AWS / "nav_reading_run.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


def test_detached_parent_does_not_hold_launch_pipes(tmp_path):
    done = tmp_path / "finished"
    code = "import pathlib,time; time.sleep(2); pathlib.Path(" + repr(str(done)) + ").write_text('done')"
    command = runner.detached_command([sys.executable, "-c", code], cwd=tmp_path, log=tmp_path / "parent.log")
    subprocess.run(["bash", "-c", command], capture_output=True, check=True, timeout=1)
    assert not done.exists()
    deadline = time.monotonic() + 4
    while not done.exists() and time.monotonic() < deadline:
        time.sleep(.05)
    assert done.read_text() == "done"


def test_ack_is_invisible_until_all_input_has_arrived(tmp_path):
    target = tmp_path / "ack.json"
    with subprocess.Popen(["bash", "-c", runner.atomic_publish_command(target)], stdin=subprocess.PIPE) as child:
        child.stdin.write(b'{"accepted":'); child.stdin.flush()
        time.sleep(.1)
        assert not target.exists()
        child.stdin.write(b'true}'); child.stdin.close()
        assert child.wait(timeout=2) == 0
    assert target.read_bytes() == b'{"accepted":true}'


def plan():
    return runner.reviewed_plan({"runId": "b23-tee-synthetic", "accountId": "000000000000",
        "ami": "ami-00000000", "vpcId": "vpc-00000000", "sourceRevision": "a"*40,
        "nsmLibrary": "/synthetic/libnsm.so", "rootDer": "/synthetic/root.der"})


def test_budget_resource_and_time_bounds():
    p = plan()
    assert runner.infra.resource_profile(p) == ("m6i.xlarge",40,2,4096)
    assert runner.infra.budget_seconds(p) == 7200
    assert 6 + (7200+120)/3600 < p["spendCeilingUsd"] == 25
    for key,value in (("spendCeilingUsd",26),("instanceLimit",2),("instanceType","m6i.32xlarge"),
                       ("kmsKeyLimit",2),("maxRunSeconds",7201),("maxRunSeconds",True)):
        with pytest.raises(ValueError): runner.infra.resource_profile({**p,key:value})


@pytest.mark.parametrize("bad", ["name; curl evil", "b23-tee-X", "b23-tee-$(id)"])
def test_no_shell_payload_in_plan(bad):
    p = {k:v for k,v in plan().items() if k in {"runId","accountId","ami","vpcId","sourceRevision","nsmLibrary","rootDer"}}
    p["runId"] = bad
    with pytest.raises(ValueError): runner.reviewed_plan(p)


def test_partial_launch_failure_always_tears_down(tmp_path, monkeypatch, manifest):
    events=[]
    monkeypatch.setattr(runner,"install_guard",lambda *a:events.append("guard"))
    monkeypatch.setattr(runner,"run",lambda *a,**k:b"")
    def failed(*a):
        events.append("launch"); raise TimeoutError("synthetic AWS timeout after create")
    monkeypatch.setattr(runner.infra,"launch",failed)
    monkeypatch.setattr(runner.infra,"cleanup",lambda *a:events.append("cleanup"))
    monkeypatch.setattr(runner.infra,"final_inventory",lambda *a:events.append("inventory"))
    with pytest.raises(TimeoutError): runner.execute(plan(),manifest,tmp_path/"run",object(),profile_operation={},profile_cli=Path('/synthetic'))
    assert events==["guard","launch","cleanup","inventory"]
    assert json.loads((tmp_path/"run/failure.json").read_bytes())["error_type"]=="TimeoutError"


def test_key_policy_reuses_exact_measurements_and_denies_plaintext():
    p = plan(); pcrs={str(i):("%02x"%(i+1))*48 for i in range(3)}
    policy=runner.kms_policy.key_policy(p,pcrs)
    deny=next(s for s in policy["Statement"] if s["Sid"]=="DenyPlaintextRelease")
    allow=next(s for s in policy["Statement"] if s["Sid"]=="ExactEnclaveRecipientOnly")
    assert deny["Effect"]=="Deny" and deny["Principal"]=="*"
    assert deny["Action"]==["kms:Decrypt","kms:GenerateDataKey"]
    for i,value in pcrs.items():
        assert allow["Condition"]["StringEquals"]["kms:RecipientAttestation:PCR"+i]==value
    with pytest.raises(ValueError): runner.kms_policy.key_policy(p,{"0":"00"*48,"1":"01"*48,"2":"02"*48})


def test_credential_sealing_only_returns_ciphertext(tmp_path,monkeypatch,manifest):
    monkeypatch.setattr(runner,"SCHWAB_REMOTE",None)
    app=tmp_path/"app"; app.write_text("client_id=SYNTHETIC-ID\nclient_secret=SYNTHETIC-SECRET\n")
    token=tmp_path/"token"; token.write_text(json.dumps({"refresh_token":"SYNTHETIC-REFRESH"}))
    monkeypatch.setattr(runner,"SCHWAB_APP",app); monkeypatch.setattr(runner,"SCHWAB_TOKENS",token)
    seen=[]
    class Kms:
        def encrypt(self,**kwargs):
            seen.append(kwargs); return {"CiphertextBlob":b"ciphertext-only"}
    ciphertext=runner.seal_schwab(Kms(),"synthetic-arn",{"run":"synthetic"},manifest)
    assert "SYNTHETIC" not in ciphertext
    plain=json.loads(seen[0]["Plaintext"])
    assert plain["manifest_sha256"]==runner.validate_manifest(manifest)
    assert plain["credentials"]["refreshToken"]=="SYNTHETIC-REFRESH"
    assert token.read_text()==json.dumps({"refresh_token":"SYNTHETIC-REFRESH"})


def test_review_does_not_call_aws_or_read_credentials(tmp_path,monkeypatch,manifest,capsys):
    p={k:v for k,v in plan().items() if k in {"runId","accountId","ami","vpcId","sourceRevision","nsmLibrary","rootDer"}}
    p["sourceRevision"]=subprocess.check_output(["git","rev-parse","HEAD"],cwd=REPO).decode().strip()
    (tmp_path/"plan.json").write_text(json.dumps(p)); (tmp_path/"manifest.json").write_text(json.dumps(manifest))
    monkeypatch.setattr(sys,"argv",[str(AWS/"nav_reading_run.py"),"--plan",str(tmp_path/"plan.json"),"--manifest",str(tmp_path/"manifest.json"),"--directory",str(tmp_path/"not-created")])
    def forbidden(*a): raise AssertionError("dry review attempted AWS credentials")
    monkeypatch.setattr(runner.infra,"session",forbidden)
    runner.main()
    output=json.loads(capsys.readouterr().out)
    assert output["aws_calls"]==0 and output["chain_writes"]==0
    assert output["missing_pinned_dependencies"]==["nsmLibrary","rootDer"]
    assert not (tmp_path/"not-created").exists()


def test_cumulative_budget_does_not_reset_between_attempts(tmp_path,monkeypatch):
    monkeypatch.setattr(runner,'PHASE2_BUDGET',tmp_path/'budget.json')
    class Ec2:
        def describe_instances(self,**kwargs): return {'Reservations':[]}
    monkeypatch.setattr(runner.infra,'client',lambda *a:Ec2())
    for n in range(3):
        runner.reserve_phase2_budget(None,{**plan(),'runId':f'b23-tee-{n}'},{'resource_time_bound_usd':8.034})
    with pytest.raises(ValueError,match='cumulative'):
        runner.reserve_phase2_budget(None,{**plan(),'runId':'b23-tee-4'},{'resource_time_bound_usd':8.034})
    assert len(json.loads((tmp_path/'budget.json').read_text())['runs'])==3


@pytest.mark.parametrize("rate,product", [("1.01",False),("0.192",True)])
def test_cost_preflight_refuses_unbounded_pricing(monkeypatch,rate,product):
    class Client:
        def describe_images(self,**kwargs):
            return {"Images":[{"Architecture":"x86_64","ProductCodes":[{}] if product else []}]}
        def get_products(self,**kwargs):
            return {"PriceList":[json.dumps({"terms":{"OnDemand":{"term":{"priceDimensions":{"dim":{"unit":"Hrs","pricePerUnit":{"USD":rate}}}}}}})]}
    monkeypatch.setattr(runner.infra,"client",lambda *a:Client())
    with pytest.raises(ValueError): runner.cost_preflight(object(),plan())
