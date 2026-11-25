#!/usr/bin/env python3
"""Single B23 native-attestation qualification run; dry review is the default.

--execute is reserved for PHASE 2. No transaction or trading path exists here.
Reuse qualification_infra launch/tags/cleanup and qualification_kms PCR policy.
One m6i.xlarge, 40 GiB delete-on-termination disk, one key, <=2 hours, $25 ceiling.
The modeled maximum is $1 prior reserve + $5 ancillary reserve + $0.384 compute.
This is a resource/time bound with automatic teardown, not a real-time AWS bill.
"""

import argparse
import base64
import fcntl
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import signal
import subprocess
import sys
import time

TEE = Path(__file__).resolve().parents[1]
REPO = Path(__file__).resolve().parents[4]
TEE_DIR = "tools/nav-reserve-proof/schwab-tee"
sys.path.insert(0, str(TEE))
import qualification_infra as infra
import qualification_kms as kms_policy
from cryptography import x509
from cryptography.hazmat.primitives import serialization
from pftl_schwab_tee.canonical import canonical_json_bytes, domain_sha256
from pftl_schwab_tee.nav_reading import validate_manifest, verify_reading, exact, nav_source_policy, commitment48
from pftl_schwab_tee.nitro_attestation import NitroNativeAttestationVerifier

AWS_CREDENTIALS = Path("/home/postfiat/aws.txt")
MINT_READER = TEE / "mint_reader" / "observe_deposit.py"
# The MINT reader instantiates BrokerHubService with these unchanged defaults.
SCHWAB_REMOTE = "pfrpc@productionrpc"
SCHWAB_APP = Path("/home/pfrpc/repos/schwab.txt")
SCHWAB_TOKENS = Path("/home/pfrpc/.config/navstrategies/schwab_tokens.json")
PHASE2_BUDGET = Path("/home/postfiat/repos/b23-coord/evidence/tee/phase2/aws-budget.json")


def reviewed_plan(value):
    exact(value, {"runId", "accountId", "ami", "vpcId", "sourceRevision", "nsmLibrary", "rootDer"}, "run plan")
    checks = {"runId": r"b23-tee-[a-z0-9-]{1,40}", "accountId": r"[0-9]{12}",
              "ami": r"ami-[0-9a-f]{8,17}", "vpcId": r"vpc-[0-9a-f]{8,17}",
              "sourceRevision": r"[0-9a-f]{40}"}
    for key, pattern in checks.items():
        if not isinstance(value[key], str) or not re.fullmatch(pattern, value[key]):
            raise ValueError("invalid reviewed plan field: " + key)
    return {**value, "region": "us-east-1", "resourceProfile": "collector-v1",
        "instanceType": "m6i.xlarge", "rootVolumeGiB": 40, "instanceLimit": 1,
        "kmsKeyLimit": 1, "spendCeilingUsd": 25, "lifecycle": "completion-or-budget",
        "priorQualificationSpendReserveUsd": 1, "ancillarySpendReserveUsd": 5,
        "maxRunSeconds": 7200, "cleanupTimer": value["runId"] + "-cleanup.timer"}


def reader_tree(revision):
    """The reader's tracked files at a revision: every path under its folder with its blob id."""
    listing = subprocess.check_output(["git", "-C", str(REPO), "ls-tree", "-r", revision, "--", TEE_DIR])
    if not listing:
        raise ValueError("no reader files at " + revision)
    return hashlib.sha256(listing).hexdigest()


def check_inputs(plan, manifest, *, execution=False):
    validate_manifest(manifest)
    infra.resource_profile(plan)
    pins = json.loads(subprocess.check_output(["git", "-C", str(REPO), "show",
        plan["sourceRevision"] + ":" + TEE_DIR + "/enclave/nav-build-inputs.json"]))
    missing = []
    for field, pin in (("nsmLibrary", "nsmLibrarySha256"), ("rootDer", "nitroRootDerSha256")):
        path = Path(plan[field])
        if not path.is_absolute() or not path.is_file():
            missing.append(field)
        elif hashlib.sha256(path.read_bytes()).hexdigest() != pins[pin]:
            raise ValueError("pinned dependency differs: " + field)
    for name in ("nav_reading.py", "nav_reading_entrypoint.py"):
        subprocess.run(["git", "-C", str(REPO), "cat-file", "-e",
            plan["sourceRevision"] + ":" + TEE_DIR + "/pftl_schwab_tee/" + name], check=True)
    if execution:
        if missing:
            raise ValueError("pinned dependencies missing")
        # The reviewed revision fixes the reader's files, not the whole repository: a commit
        # that leaves these folders alone may be checked out without a new review or profile.
        if reader_tree("HEAD") != reader_tree(plan["sourceRevision"]):
            raise ValueError("the checkout's reader files differ from the reviewed revision")
        if subprocess.check_output(["git", "-C", str(REPO), "status", "--porcelain", "--untracked-files=no", "--", TEE_DIR]).strip():
            raise ValueError("the reader's folder has tracked changes")
        for path in (AWS_CREDENTIALS,):
            if not path.is_file() or path.stat().st_mode & 0o077:
                raise ValueError("credential path is missing or not owner-only")
        credential_material(check_only=True)
        if not MINT_READER.is_file():
            raise ValueError("MINT reader path is unavailable")
        remaining = manifest["cutoff_unix_ms"] / 1000 - time.time()
        if remaining <= 0:
            raise ValueError("cutoff must still be in the future before launch")
    return {"mode": "PHASE_1_REVIEW", "aws_calls": 0, "chain_writes": 0,
        "max_compute_seconds": infra.budget_seconds(plan), "spend_cap_usd": 25,
        "modeled_total_upper_bound_usd": 6 + (infra.budget_seconds(plan) + 120) * .192 / 3600,
        "missing_pinned_dependencies": missing, "native_only": True,
        "sp1_nav_guest_and_proof_still_required_for_chain": True}


def run(command, *, log=None, timeout=600, input=None):
    if log is None:
        return subprocess.run(command, check=True, timeout=timeout, input=input, stdout=subprocess.PIPE, cwd=REPO).stdout
    with Path(log).open("ab") as output:
        subprocess.run(command, check=True, timeout=timeout, input=input, stdout=output, stderr=subprocess.STDOUT, cwd=REPO)


def detached_command(command, *, cwd, log):
    """Close every SSH channel descriptor before starting the long-lived parent."""
    script = (
        "import os,subprocess; "
        f"output=open({str(log)!r},'ab',buffering=0); "
        f"subprocess.Popen({command!r},cwd={str(cwd)!r},"
        "env={**os.environ,'PYTHONPATH':'.'},stdin=subprocess.DEVNULL,"
        "stdout=output,stderr=subprocess.STDOUT,start_new_session=True,close_fds=True)"
    )
    return "python3 -c " + shlex.quote(script)


def atomic_publish_command(target):
    temporary = str(target) + ".tmp"
    return "umask 077; cat > " + shlex.quote(temporary) + " && mv " + shlex.quote(temporary) + " " + shlex.quote(str(target))


def cost_preflight(aws, plan):
    """Refuse Marketplace AMIs and prices above the conservative $1/hour bound."""
    ec2 = infra.client(aws, "ec2", plan)
    images = ec2.describe_images(ImageIds=[plan["ami"]], Owners=["amazon"])["Images"]
    if len(images) != 1 or images[0].get("ProductCodes") or images[0].get("Architecture") != "x86_64":
        raise ValueError("review requires an Amazon x86_64 AMI without product charges")
    filters = [{"Type": "TERM_MATCH", "Field": k, "Value": v} for k, v in {
        "instanceType": "m6i.xlarge", "regionCode": "us-east-1", "operatingSystem": "Linux",
        "tenancy": "Shared", "preInstalledSw": "NA", "capacitystatus": "Used"}.items()]
    from botocore.exceptions import ClientError
    public_source = None
    try:
        rows = infra.client(aws, "pricing", plan).get_products(ServiceCode="AmazonEC2", Filters=filters, MaxResults=100)
    except ClientError as error:
        if error.response["Error"]["Code"] not in ("AccessDenied", "AccessDeniedException"): raise
        # AWS's own public pricing-page feed is readable without changing IAM.
        import urllib.request, gzip
        public_source = "https://b0.p.awsstatic.com/pricing/2.0/meteredUnitMaps/ec2/USD/current/ec2-ondemand-without-sec-sel/US%20East%20(N.%20Virginia)/Linux/index.json"
        raw = urllib.request.urlopen(public_source, timeout=30).read(8*1024*1024+1)
        if len(raw)>8*1024*1024: raise ValueError("public price feed bound")
        if raw[:2] == b"\x1f\x8b": raw = gzip.decompress(raw)
        if len(raw)>16*1024*1024: raise ValueError("public price JSON bound")
        found = []
        def visit(value):
            if isinstance(value, dict):
                if value.get("Instance Type") == "m6i.xlarge": found.append(value)
                else:
                    for child in value.values(): visit(child)
            elif isinstance(value, list):
                for child in value: visit(child)
        visit(json.loads(raw))
        if len(found)!=1 or found[0]["Location"]!="US East (N. Virginia)" or found[0]["Operating System"]!="Linux" or found[0]["Pre Installed S/W"]!="NA":
            raise ValueError("public price identity ambiguous")
        rows = {"PriceList":[json.dumps({"terms":{"OnDemand":{"public":{"priceDimensions":{"rate":{
            "unit":"Hrs","pricePerUnit":{"USD":found[0]["price"]}}}}}}})]}
    from decimal import Decimal
    rates = set()
    for row in rows["PriceList"]:
        for term in json.loads(row)["terms"]["OnDemand"].values():
            for dimension in term["priceDimensions"].values():
                if dimension["unit"] == "Hrs": rates.add(Decimal(dimension["pricePerUnit"]["USD"]))
    if rows.get("NextToken") or len(rates) != 1 or not Decimal(0) < next(iter(rates)) <= Decimal(1):
        raise ValueError("price lookup is ambiguous or above reviewed rate ceiling")
    return {"ec2_hourly_usd": str(next(iter(rates))), "conservative_rate_ceiling_usd": 1,
            "public_source":public_source,
            "resource_time_bound_usd": 6 + (infra.budget_seconds(plan) + 120) / 3600}


def teardown(aws, plan, directory):
    # Prevent the guard and normal completion from racing key/instance deletion.
    with (directory / "cleanup.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        infra.cleanup(aws, plan, directory / "cleanup.json")
        infra.final_inventory(aws, plan, directory)


def install_guard(plan, directory):
    unit = plan["cleanupTimer"].removesuffix(".timer")
    units = Path.home() / ".config/systemd/user"
    units.mkdir(parents=True, exist_ok=True)
    command = [sys.executable, str(Path(__file__).resolve()), "--guard", "--directory", str(directory)]
    # systemd's parser is not a shell. Reject special specifiers in these paths.
    if any(any(c in part for c in ('%', '\n', '"', '\\')) for part in command):
        raise ValueError("guard command path is not representable")
    quoted = " ".join('"' + part + '"' for part in command)
    (units / (unit + ".service")).write_text("[Service]\nType=oneshot\nExecStart=" + quoted + "\n")
    (units / (unit + ".timer")).write_text("[Timer]\nOnBootSec=10s\nOnUnitActiveSec=30s\nAccuracySec=1s\n[Install]\nWantedBy=timers.target\n")
    run(["systemctl", "--user", "daemon-reload"])
    run(["systemctl", "--user", "enable", "--now", plan["cleanupTimer"]])
    run(["systemctl", "--user", "is-active", "--quiet", plan["cleanupTimer"]])


def verify_recipient(recipient, manifest, root_pem, pcrs):
    exact(recipient, {"schema", "manifest_sha256", "binding_digest", "statement_key", "document_base64"}, "recipient")
    mh = validate_manifest(manifest)
    binding = domain_sha256("postfiat.b23.nav_recipient.v1", {"manifest_sha256": mh, "statement_key": recipient["statement_key"]})
    if (recipient["schema"] != "postfiat.b23.nav_recipient.v1" or
            recipient["manifest_sha256"] != mh or recipient["binding_digest"] != binding):
        raise ValueError("recipient binding differs")
    verifier = NitroNativeAttestationVerifier(root_certificate_pem=root_pem,
        max_age_ms=60_000, max_future_skew_ms=1000)
    parsed = verifier.parse_and_verify(base64.b64decode(recipient["document_base64"], validate=True), attestation_binding_digest=binding)
    if parsed.statement_key != recipient["statement_key"]:
        raise ValueError("recipient signing key differs")
    if any(parsed.pcrs.get(int(i)) != bytes.fromhex(value) or value == "00"*48 for i, value in pcrs.items()):
        raise ValueError("recipient measurements differ")
    return {"schema": "postfiat.b23.nav_recipient_accepted.v1", "binding_digest": binding}


def credential_material(*, check_only=False):
    """Existing MINT reader lives on productionrpc; secret stdout is captured only.

    No plaintext credential file is copied to this host or the EC2 parent.
    """
    if SCHWAB_REMOTE is None:  # Offline tests or an explicitly configured local owner.
        return {"app":SCHWAB_APP.read_text(),"tokens":json.loads(SCHWAB_TOKENS.read_bytes())}
    script = "from pathlib import Path\nimport json\n"
    script += f"a=Path({str(SCHWAB_APP)!r});t=Path({str(SCHWAB_TOKENS)!r})\n"
    script += "assert all(p.is_file() and not p.stat().st_mode&0o077 and p.stat().st_size<65536 for p in (a,t))\n"
    script += "print(json.dumps({'ok':True}))\n" if check_only else "print(json.dumps({'app':a.read_text(),'tokens':json.loads(t.read_bytes())}))\n"
    result = subprocess.run(["ssh","-o","BatchMode=yes","-o","ConnectTimeout=15",SCHWAB_REMOTE,
        "python3 -c "+shlex.quote(script)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True,timeout=30)
    if len(result.stdout)>131072: raise ValueError("credential response bound")
    return json.loads(result.stdout)


def seal_schwab(kms, arn, context, manifest):
    # Only this operator-side function reads the existing files. It returns KMS
    # ciphertext, never plaintext; no credential enters argv, disk staging or EIF.
    material = credential_material()
    app = {}
    for line in material["app"].splitlines():
        if "=" in line:
            key, value = line.split("=", 1); app[key.strip()] = value.strip()
    token = material["tokens"]
    credentials = {"schema": "postfiat.yolo.schwab_credentials.v1",
        "clientId": app["client_id"], "clientSecret": app["client_secret"],
        "refreshToken": token["refresh_token"],
        "accountApplicationIdentitySha256": manifest["account_commitment_sha256"]}
    plaintext = canonical_json_bytes({"manifest_sha256": validate_manifest(manifest), "credentials": credentials})
    if len(plaintext) > 4096:
        raise ValueError("KMS credential plaintext exceeds limit")
    return base64.b64encode(kms.encrypt(KeyId=arn, Plaintext=plaintext, EncryptionContext=context)["CiphertextBlob"]).decode()


def bind_measured_profile(manifest, pcrs, root_der, operation, profile_cli, directory):
    policy = nav_source_policy(manifest, pcrs, root_der)
    manifest = dict(manifest)
    manifest["source_manifest_hash"] = commitment48(policy["schema"], policy)
    manifest["valuation_policy_hash"] = domain_sha256(policy["valuation_policy"]["schema"], policy["valuation_policy"])
    operation = dict(operation, source_manifest_hash=manifest["source_manifest_hash"],
                     valuation_policy_hash=manifest["valuation_policy_hash"])
    infra.write_json(directory / "profile-operation.json", operation)
    profile = json.loads(run([str(profile_cli), "profile", "--operation", str(directory / "profile-operation.json")]))
    manifest["proof_profile_id"] = profile["profile_id"]
    validate_manifest(manifest)
    infra.write_json(directory / "source-policy.json", policy)
    infra.write_json(directory / "profile.json", profile)
    infra.write_json(directory / "manifest.json", manifest)
    return manifest, policy


def reserve_phase2_budget(aws, plan, cost):
    """Caller holds the phase-wide lock through teardown. Never release reservations.

    Charging every attempt its full resource/time ceiling is conservative even
    when a run finishes early. No retry can silently reset the cumulative cap.
    """
    ledger = json.loads(PHASE2_BUDGET.read_bytes()) if PHASE2_BUDGET.exists() else {"cap_usd":25,"runs":[]}
    if ledger["cap_usd"] != 25 or any(r["run_id"] == plan["runId"] for r in ledger["runs"]):
        raise ValueError("budget ledger differs or run ID already reserved")
    reservation = cost["resource_time_bound_usd"]
    total = sum(r["reserved_upper_usd"] for r in ledger["runs"]) + reservation
    if total > 25: raise ValueError("cumulative phase-2 cap would be exceeded")
    ec2 = infra.client(aws, "ec2", plan)
    rows = ec2.describe_instances(Filters=[{"Name":"tag:Project","Values":["yolo-nitro-qualification"]},
        {"Name":"instance-state-name","Values":["pending","running","stopping","stopped","shutting-down"]}])
    if any(r["Instances"] for r in rows["Reservations"]):
        raise ValueError("another qualification instance exists; one-instance cap")
    ledger["runs"].append({"run_id":plan["runId"],"reserved_upper_usd":reservation,
        "reserved_utc":datetime.now(timezone.utc).isoformat(),"pricing":cost})
    ledger["cumulative_reserved_upper_usd"] = total
    infra.write_json(PHASE2_BUDGET, ledger)


def execute(plan, manifest, directory, aws, *, profile_operation, profile_cli, on_demand=False):
    """All create paths are inside try/finally; guard survives this process."""
    directory.mkdir(parents=True, exist_ok=False, mode=0o700)
    infra.write_json(directory / "plan.json", plan)
    infra.write_json(directory / "manifest.json", manifest)
    install_guard(plan, directory)
    try:
        run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(directory / "ssh_key")])
        infra.launch(aws, plan, directory)
        launch = json.loads((directory / "launch.json").read_bytes())
        instance_id = launch["Instances"][0]["InstanceId"]
        ec2 = infra.client(aws, "ec2", plan)
        ec2.get_waiter("instance_running").wait(InstanceIds=[instance_id], WaiterConfig={"Delay": 5, "MaxAttempts": 60})
        instance = ec2.describe_instances(InstanceIds=[instance_id])["Reservations"][0]["Instances"][0]
        host = "ec2-user@" + instance["PublicIpAddress"]
        ssh = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", "-o", "StrictHostKeyChecking=accept-new",
               "-o", "UserKnownHostsFile=" + str(directory / "known_hosts"), "-i", str(directory / "ssh_key"), host]
        def remote(command, **kwargs): return run([*ssh, command], **kwargs)
        def upload(local, target):
            # Contents travel on SSH stdin; targets are fixed, no secret arguments.
            remote("umask 077; cat > " + shlex.quote(target), input=Path(local).read_bytes())
        ready = False
        for _ in range(90):
            try:
                remote("test -f /var/lib/yolo-qualification-ready", timeout=15)
                ready = True; break
            except subprocess.SubprocessError:
                time.sleep(5)
        if not ready: raise TimeoutError("parent bootstrap deadline")
        # Include only tracked source from the pinned commit; no working directory archive.
        bundle = run(["git", "-C", str(REPO), "archive", plan["sourceRevision"],
            TEE_DIR + "/pftl_schwab_tee", TEE_DIR + "/enclave", TEE_DIR + "/parent"])
        remote("mkdir -p /home/ec2-user/nav/src; tar -x -C /home/ec2-user/nav/src", input=bundle)
        upload(plan["nsmLibrary"], "/home/ec2-user/nav/libnsm.so")
        upload(plan["rootDer"], "/home/ec2-user/nav/root.der")
        # Build clean context locally from committed source, using only public policy.
        run([sys.executable, "-c", "import sys; sys.path.insert(0, %r); from pftl_schwab_tee.nitro_build import main; raise SystemExit(main(sys.argv[1:]))" % str(TEE), "--repository", str(REPO),
            "--revision", plan["sourceRevision"], "--nsm-library", plan["nsmLibrary"], "--root-der", plan["rootDer"],
            "--output-directory", str(directory / "build"), "--image-tag", "b23-nav:qualified",
            "--nav-manifest", str(directory / "manifest.json"), "--prepare-only"], log=directory / "build-context.log")
        context_tar = run(["tar", "-cf", "-", "-C", str(directory / "build/context"), "."])
        remote("mkdir -p /home/ec2-user/nav/context; tar -x -C /home/ec2-user/nav/context", input=context_tar)
        remote("python3 -m pip install --user 'cryptography==44.0.3'", log=directory / "parent-package.log")
        epoch = json.loads((directory / "build/context/build-inputs.json").read_bytes())["sourceDateEpoch"]
        pcr_sets = []
        for n in (1, 2):
            remote("docker build --no-cache --build-arg SOURCE_DATE_EPOCH=" + str(epoch) +
                " -t b23-nav:qualified /home/ec2-user/nav/context", timeout=1200, log=directory / f"docker-{n}.log")
            measured = json.loads(remote("nitro-cli build-enclave --docker-uri b23-nav:qualified --output-file /home/ec2-user/nav/nav-"+str(n)+".eif", timeout=600))
            infra.write_json(directory / f"measurement-{n}.json", measured)
            pcr_sets.append({str(i): measured["Measurements"]["PCR"+str(i)] for i in (0,1,2)})
        if pcr_sets[0] != pcr_sets[1]: raise ValueError("two EIF builds have different PCRs")
        pcrs = pcr_sets[0]
        infra.write_json(directory / "pcrs.json", pcrs)
        if on_demand:
            # Time for KMS setup and enclave boot, after the variable build time.
            # Cutoff is already excluded from the measured policy.
            manifest = dict(manifest, cutoff_unix_ms=int(time.time() * 1000) + 120_000)
        manifest, source_policy = bind_measured_profile(manifest, pcrs, Path(plan["rootDer"]).read_bytes(),
            profile_operation, profile_cli, directory)
        kms = infra.client(aws, "kms", plan)
        if infra.owned_keys(kms, plan): raise ValueError("run already owns a KMS key")
        expected_policy = kms_policy.key_policy(plan, pcrs)
        # Same qualification policy, with a description matching real read-only use.
        created = kms.create_key(Policy=json.dumps(expected_policy), Description=plan["runId"] + " B23 read-only NAV",
            KeyUsage="ENCRYPT_DECRYPT", KeySpec="SYMMETRIC_DEFAULT", MultiRegion=False,
            Tags=[{"TagKey": item["Key"], "TagValue": item["Value"]} for item in infra.tags(plan)])
        arn = created["KeyMetadata"]["Arn"]
        infra.write_json(directory / "kms.json", created["KeyMetadata"])
        actual = json.loads(kms.get_key_policy(KeyId=arn, PolicyName="default")["Policy"])
        if actual != expected_policy: raise ValueError("KMS policy readback differs")
        infra.write_json(directory / "kms-policy.json", actual)
        context = kms_policy.context(plan)
        encrypted = seal_schwab(kms, arn, context, manifest)
        boot = {"schema": "postfiat.b23.nav_boot.v1", "manifest": manifest, "kmsKeyArn": arn,
                "encryptionContext": context, "encryptedSchwabCredentialsBase64": encrypted}
        # This file is ciphertext only. AWS temporary role credentials are supplied by IMDS in the parent.
        infra.write_json(directory / "boot.json", boot)
        upload(directory / "boot.json", "/home/ec2-user/nav/boot.json")
        # Start close to the fix; no long-lived parent awaiting a future day's epoch.
        while time.time() < manifest["cutoff_unix_ms"] / 1000:
            time.sleep(max(0, min(1, manifest["cutoff_unix_ms"] / 1000 - time.time())))
        if time.time() > manifest["cutoff_unix_ms"] / 1000 + 10:
            raise TimeoutError("build missed cutoff; no backdated read")
        remote(detached_command(["python3", "parent/yolo_capture_parent.py", "--nav",
            "--eif", "/home/ec2-user/nav/nav-2.eif", "--boot", "/home/ec2-user/nav/boot.json",
            "--output", "/home/ec2-user/nav/output"], cwd="/home/ec2-user/nav/src/" + TEE_DIR,
            log="/home/ec2-user/nav/parent.log"), timeout=15)
        def await_file(name, seconds):
            end = time.monotonic() + seconds
            while time.monotonic() < end:
                try: return remote("cat /home/ec2-user/nav/output/" + name, timeout=10)
                except subprocess.SubprocessError: time.sleep(.5)
            raise TimeoutError("expected enclave output did not arrive")
        recipient = json.loads(await_file("recipient.json", 30))
        root_pem = x509.load_der_x509_certificate(Path(plan["rootDer"]).read_bytes()).public_bytes(serialization.Encoding.PEM)
        ack = verify_recipient(recipient, manifest, root_pem, pcrs)
        infra.write_json(directory / "recipient.json", recipient)
        # The parent polls for existence. Publish only after the complete frame
        # is written; creating the final filename first races the polling read.
        remote(atomic_publish_command("/home/ec2-user/nav/output/recipient-accepted.json"),
            input=canonical_json_bytes(ack))
        result = json.loads(await_file("result.json", 90))
        values = verify_reading(result, manifest=manifest, root_pem=root_pem,
            expected_pcrs=pcrs, now_ms=int(time.time()*1000))
        infra.write_json(directory / "result.json", result)
        infra.write_json(directory / "verified-values.json", values)
        (directory / "witness.json").write_bytes(canonical_json_bytes({"policy":source_policy,
            "manifest":manifest,"envelope":result}))
        (directory / "encrypted-archive.tar").write_bytes(remote("tar -cf - -C /home/ec2-user/nav/output archive"))
        infra.write_json(directory / "completion.json", {"runId": plan["runId"], "outputsRetained": True,
            "nativeVerified": True, "sp1ProofGenerated": False, "chainWrites": 0})
    except BaseException as error:
        infra.write_json(directory / "failure.json", {"error_type": type(error).__name__, "chain_writes": 0})
        # OAuth may already have rotated before a later capture error. Preserve
        # its encrypted recovery artifact before deleting the parent.
        if "remote" in locals():
            for target, command in (("failure-encrypted-archive.tar", "tar -cf - -C /home/ec2-user/nav/output archive"),
                    ("failure-parent.log", "tail -c 1048576 /home/ec2-user/nav/parent.log")):
                try: (directory / target).write_bytes(remote(command, timeout=15))
                except Exception: pass  # Teardown must still run on transport failure.
        raise
    finally:
        # Also covers a RunInstances timeout after AWS accepted the request.
        teardown(aws, plan, directory)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path)
    parser.add_argument("--manifest", type=Path)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--profile-operation", type=Path)
    parser.add_argument("--profile-cli", type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--execute", action="store_true")
    mode.add_argument("--guard", action="store_true")
    mode.add_argument("--cleanup", action="store_true")
    args = parser.parse_args()
    os.umask(0o077)
    directory = args.directory.resolve()
    if args.guard or args.cleanup:
        plan = json.loads((directory / "plan.json").read_bytes())
        if args.cleanup or infra.cleanup_reason(plan, directory):
            aws = infra.session(AWS_CREDENTIALS)
            teardown(aws, plan, directory)
        return
    if args.plan is None or args.manifest is None:
        parser.error("--plan and --manifest are required")
    plan = reviewed_plan(json.loads(args.plan.read_bytes()))
    manifest = json.loads(args.manifest.read_bytes())
    review = check_inputs(plan, manifest, execution=args.execute)
    if not args.execute:
        print(json.dumps(review, indent=2)); return
    def stop(signum, frame): raise SystemExit(128 + signum)
    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)
    aws = infra.session(AWS_CREDENTIALS)
    if aws.client("sts").get_caller_identity()["Account"] != plan["accountId"]:
        raise ValueError("wrong AWS account")
    cost = cost_preflight(aws, plan)
    if args.profile_operation is None or args.profile_cli is None:
        parser.error("--execute requires --profile-operation and --profile-cli")
    operation = json.loads(args.profile_operation.read_bytes())
    if "schedule" not in manifest: raise ValueError("phase 2 requires an immutable opening/daily schedule")
    if operation["verifier_kind"] != "sp1-nav-reserve-v1": raise ValueError("wrong reserve verifier")
    PHASE2_BUDGET.parent.mkdir(parents=True, exist_ok=True)
    with PHASE2_BUDGET.with_suffix(".lock").open("a") as budget_lock:
        fcntl.flock(budget_lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        reserve_phase2_budget(aws, plan, cost)
        execute(plan, manifest, directory, aws, profile_operation=operation, profile_cli=args.profile_cli.resolve())
        infra.write_json(directory / "cost-preflight.json", cost)


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        # Never print credential-derived exception values or tracebacks.
        print(json.dumps({"status": "RUN_FAILED", "error_type": type(error).__name__,
                          "details": "inspect non-secret step logs and cleanup.json"}), file=sys.stderr)
        raise SystemExit(1)
