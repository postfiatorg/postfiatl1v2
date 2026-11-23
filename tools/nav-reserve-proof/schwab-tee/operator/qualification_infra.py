"""Bounded Phase 5 EC2 lifecycle. Credentials stay on the operator machine.

The reviewed JSON plan is explicit; cleanup discovers only its two ownership
tags so it also covers an interrupted create whose response was never saved.
"""

import argparse
from datetime import datetime, timedelta, timezone
from decimal import Decimal, InvalidOperation
import ipaddress
import json
import os
from pathlib import Path
import shlex
import subprocess
import urllib.request


RESOURCE_PROFILES = {
    "collector-v1": ("m6i.xlarge", 40, 2, 4096),
    "private-prover-v1": ("m6i.8xlarge", 40, 16, 110592),
    "private-prover-avx512-v1": ("m6i.8xlarge", 40, 30, 110592),
}
COMPUTE_HOURLY_USD = {"collector-v1": Decimal("0.192"), "private-prover-v1": Decimal("1.536"),
    "private-prover-avx512-v1": Decimal("1.536")}
TERMINATION_RESERVE_SECONDS = 120


def resource_profile(plan, now=None):
    """Validate the reviewed resources and cumulative budget, without date gates."""
    if (plan.get("lifecycle") != "completion-or-budget"
            or plan["region"] != "us-east-1" or plan["spendCeilingUsd"] != 25
            or plan.get("instanceLimit") != 1):
        raise ValueError("outside the approved lifecycle, region, budget or count")
    profile = RESOURCE_PROFILES.get(plan.get("resourceProfile", "collector-v1"))
    if profile is None or (plan["instanceType"], plan["rootVolumeGiB"]) != profile[:2]:
        raise ValueError("resource size differs from a reviewed qualification profile")
    if plan.get("kmsKeyLimit") != 1:
        raise ValueError("qualification permits one owned KMS key")
    budget_seconds(plan)
    return profile


def budget_seconds(plan):
    """Compute a shutdown safeguard from dollars; never accept a caller's date."""
    try:
        prior = Decimal(str(plan["priorQualificationSpendReserveUsd"]))
        ancillary = Decimal(str(plan["ancillarySpendReserveUsd"]))
        ceiling = Decimal(str(plan["spendCeilingUsd"]))
        rate = COMPUTE_HOURLY_USD[plan["resourceProfile"]]
        if not all(v.is_finite() for v in (prior, ancillary, ceiling)) or prior < 1 or ancillary < 1 or ceiling != 25:
            raise ValueError("invalid cumulative spend reserves")
        seconds = int((ceiling - prior - ancillary) * 3600 / rate) - TERMINATION_RESERVE_SECONDS
        if "maxRunSeconds" in plan:
            bound = plan["maxRunSeconds"]
            if type(bound) is not int or not 600 < bound <= 7200:
                raise ValueError("maxRunSeconds must be a bounded integer")
            seconds = min(seconds, bound)
    except (KeyError, InvalidOperation, TypeError) as exc:
        raise ValueError("explicit reviewed cumulative spend reserves required") from exc
    if seconds <= 600:
        raise ValueError("insufficient remaining budget to launch")
    return seconds


def cleanup_reason(plan, directory, now=None):
    """Completion is recorded only after the operator has retained run outputs."""
    # A damaged completion marker must not suppress the independent spend stop.
    stop = plan.get("budgetStopAt")
    if stop and (now or datetime.now(timezone.utc)) >= datetime.fromisoformat(stop):
        return "cumulative-budget-safeguard"
    completed = directory / "completion.json"
    if completed.exists():
        result = json.loads(completed.read_bytes())
        if result.get("runId") != plan["runId"] or result.get("outputsRetained") is not True:
            raise ValueError("completion marker does not confirm this run's retained outputs")
        return "outputs-retained"
    return None

def session(credentials):
    import boto3
    path = Path(credentials)
    if path.stat().st_mode & 0o077:
        raise ValueError("credential file must be private to its owner")
    tokens = []
    for line in path.read_text().splitlines():
        tokens.extend(shlex.split(line.replace("=", " ").replace(":", " ")))
    access = [s for s in tokens if len(s) == 20 and s[:4] == "AKIA"]
    secret = [s for s in tokens if len(s) == 40 and all(c.isalnum() or c in "/+=" for c in s)]
    if len(access) != 1 or len(secret) != 1:
        raise ValueError("expected exactly one long-term AWS credential pair")
    return boto3.Session(aws_access_key_id=access[0], aws_secret_access_key=secret[0])


def client(aws, service, plan):
    from botocore.config import Config
    return aws.client(service, region_name=plan["region"], config=Config(
        connect_timeout=8, read_timeout=20, retries={"max_attempts": 2}))


def tags(plan):
    return [{"Key": "Project", "Value": "yolo-nitro-qualification"},
            {"Key": "QualificationRun", "Value": plan["runId"]},
            {"Key": "BudgetStopAt", "Value": plan.get("budgetStopAt", "not-launched")},
            {"Key": "Name", "Value": plan["runId"]}]


def filters(plan):
    return [{"Name": "tag:" + tag["Key"], "Values": [tag["Value"]]} for tag in tags(plan)[:2]]


def write_json(path, value):
    path = Path(path)
    temp = path.with_suffix(".new")
    temp.write_text(json.dumps(value, indent=2, default=str) + "\n")
    temp.chmod(0o600)
    temp.replace(path)


def owned_keys(kms, plan):
    wanted = {tag["Key"]: tag["Value"] for tag in tags(plan)[:2]}
    found = []
    for page in kms.get_paginator("list_keys").paginate():
        for item in page["Keys"]:
            actual = {}
            for tag_page in kms.get_paginator("list_resource_tags").paginate(KeyId=item["KeyId"]):
                actual.update({tag["TagKey"]: tag["TagValue"] for tag in tag_page["Tags"]})
            if all(actual.get(k) == v for k, v in wanted.items()):
                found.append(item["KeyId"])
    return found


def cleanup(aws, plan, output):
    from botocore.exceptions import ClientError
    ec2, kms = client(aws, "ec2", plan), client(aws, "kms", plan)
    report = {"runId": plan["runId"], "at": datetime.now(timezone.utc).isoformat(), "actions": [], "errors": []}
    def perform(name, fn):
        try:
            result = fn()
            report["actions"].append({"action": name, "result": result})
            write_json(output, report)
            return result
        except ClientError as exc:
            report["errors"].append({"action": name, "code": exc.response["Error"]["Code"]})
            write_json(output, report)
            return None
    instances = [i for page in ec2.get_paginator("describe_instances").paginate(Filters=filters(plan))
                 for r in page["Reservations"] for i in r["Instances"] if i["State"]["Name"] != "terminated"]
    if instances:
        ids = [i["InstanceId"] for i in instances]
        perform("terminate_instances", lambda: ec2.terminate_instances(InstanceIds=ids))
        ec2.get_waiter("instance_terminated").wait(InstanceIds=ids, WaiterConfig={"Delay": 10, "MaxAttempts": 30})
    for key in owned_keys(kms, plan):
        state = kms.describe_key(KeyId=key)["KeyMetadata"]["KeyState"]
        if state != "PendingDeletion":
            perform("schedule_key_deletion:" + key, lambda key=key: kms.schedule_key_deletion(KeyId=key, PendingWindowInDays=7))
    for sg in ec2.describe_security_groups(Filters=filters(plan))["SecurityGroups"]:
        perform("delete_security_group:" + sg["GroupId"], lambda sg=sg: ec2.delete_security_group(GroupId=sg["GroupId"]))
    for key in ec2.describe_key_pairs(Filters=filters(plan))["KeyPairs"]:
        perform("delete_key_pair:" + key["KeyPairId"], lambda key=key: ec2.delete_key_pair(KeyPairId=key["KeyPairId"]))
    for volume in ec2.describe_volumes(Filters=filters(plan))["Volumes"]:
        if volume["State"] == "available":
            perform("delete_volume:" + volume["VolumeId"], lambda volume=volume: ec2.delete_volume(VolumeId=volume["VolumeId"]))
    report["complete"] = not report["errors"]
    write_json(output, report)
    print(json.dumps({"runId": plan["runId"], "cleanupComplete": report["complete"], "errors": report["errors"]}))
    if report["errors"]:
        raise RuntimeError("owned resource cleanup needs retry")


def final_inventory(aws, plan, directory):
    """Confirm disappearance independently of successful delete API responses."""
    ec2, kms = client(aws, "ec2", plan), client(aws, "kms", plan)
    instances = [i for page in ec2.get_paginator("describe_instances").paginate(Filters=filters(plan))
                 for r in page["Reservations"] for i in r["Instances"]]
    volumes = ec2.describe_volumes(Filters=filters(plan))["Volumes"]
    groups = ec2.describe_security_groups(Filters=filters(plan))["SecurityGroups"]
    pairs = ec2.describe_key_pairs(Filters=filters(plan))["KeyPairs"]
    interfaces = ec2.describe_network_interfaces(Filters=filters(plan))["NetworkInterfaces"]
    keys = [kms.describe_key(KeyId=key)["KeyMetadata"] for key in owned_keys(kms, plan)]
    if (any(i["State"]["Name"] != "terminated" for i in instances) or volumes or groups or pairs or interfaces
            or any(k["KeyState"] != "PendingDeletion" for k in keys)):
        raise RuntimeError("fresh inventory still contains owned resources; keep cleanup guard active")
    now = datetime.now(timezone.utc)
    # Observe the first completed inventory promptly; later operator checks must
    # not recalculate terminated-instance cost as if it were still running.
    report = {"schema": "postfiat.yolo.aws_final_inventory.v2", "checkedAt": now.isoformat(),
        "runId": plan["runId"], "instances": [{"id": i["InstanceId"], "state": i["State"]["Name"],
            "launchTime": i["LaunchTime"].isoformat(), "stateTransitionReason": i["StateTransitionReason"]} for i in instances],
        "ownedVolumes": [], "ownedSecurityGroups": [], "ownedKeyPairs": [], "ownedNetworkInterfaces": [],
        "kmsKeys": [{"id": k["KeyId"], "state": k["KeyState"], "deletionDate": str(k["DeletionDate"])} for k in keys],
        "noOwnedBillableComputeRemaining": True}
    elapsed = sum(max(0, (now - i["LaunchTime"]).total_seconds()) / 3600 for i in instances)
    estimate = (float(plan["priorQualificationSpendReserveUsd"]) + float(plan["ancillarySpendReserveUsd"])
                + elapsed * float(COMPUTE_HOURLY_USD[plan["resourceProfile"]]))
    report["costEstimate"] = {"scope": "Conservative elapsed-to-first-observed-termination estimate; not AWS billing",
        "priorSpendReserveUsd": plan["priorQualificationSpendReserveUsd"],
        "ancillarySpendReserveUsd": plan["ancillarySpendReserveUsd"], "computeHoursUpperBound": elapsed,
        "computeRateUsdPerHour": str(COMPUTE_HOURLY_USD[plan["resourceProfile"]]),
        "estimatedTotalUsdUpperBound": estimate, "authorizedCeilingUsd": plan["spendCeilingUsd"]}
    path = directory / "final-inventory.json"
    if not path.exists():
        write_json(path, report)
    disabled = []
    for timer in [plan["cleanupTimer"], plan.get("deliveryTimer")]:
        if timer:
            subprocess.run(["systemctl", "--user", "disable", "--now", timer], check=True)
            disabled.append(timer)
    write_json(directory / "cleanup-timer-disabled.json", {"schema": "postfiat.yolo.cleanup_timer_completion.v1",
        "recordedAt": now.isoformat(), "timers": disabled, "disabledAfterFinalInventory": True})


def validate_launch_preflight(plan):
    if plan['resourceProfile']=='collector-v1':
        return
    from pftl_schwab_tee.demo_preflight import validate_preflight
    from pftl_schwab_tee.canonical import sha256_bytes
    reference=plan.get('targetPreflight')
    if not isinstance(reference,dict) or set(reference)!={'specPath','reportPath','reportSha256'}:
        raise ValueError('test actual options baskets locally before allocating proof compute')
    encoded=Path(reference['reportPath']).read_bytes()
    if sha256_bytes(encoded)!=reference['reportSha256']:
        raise ValueError('offline basket preflight report differs')
    validate_preflight(json.loads(encoded),json.loads(Path(reference['specPath']).read_bytes()))


def launch(aws, plan, directory):
    validate_launch_preflight(plan)
    profile = resource_profile(plan)
    subprocess.run(["systemctl", "--user", "is-active", "--quiet", plan["cleanupTimer"]], check=True)
    ec2 = client(aws, "ec2", plan)
    existing = ec2.describe_instances(Filters=filters(plan))["Reservations"]
    if existing or (directory / "launch-attempt.json").exists() or (directory / "launch.json").exists():
        raise ValueError("run already has a launch attempt; inspect it before creating more compute")
    project = [{"Name": "tag:Project", "Values": ["yolo-nitro-qualification"]},
               {"Name": "instance-state-name", "Values": ["pending", "running", "stopping", "stopped"]}]
    if any(r["Instances"] for r in ec2.describe_instances(Filters=project)["Reservations"]):
        raise ValueError("another qualification instance already consumes the one-instance allowance")
    started = datetime.now(timezone.utc)
    stop = started + timedelta(seconds=budget_seconds(plan))
    plan.update(budgetStartedAt=started.isoformat(), budgetStopAt=stop.isoformat())
    write_json(directory / "plan.json", plan)
    write_json(directory / "launch-attempt.json", {"runId": plan["runId"], "at": started.isoformat()})
    ip = str(ipaddress.IPv4Address(urllib.request.urlopen("https://checkip.amazonaws.com", timeout=10).read().decode().strip()))
    public_key = (directory / "ssh_key.pub").read_bytes()
    ec2.import_key_pair(KeyName=plan["runId"], PublicKeyMaterial=public_key,
        TagSpecifications=[{"ResourceType": "key-pair", "Tags": tags(plan)}])
    sg = ec2.create_security_group(GroupName=plan["runId"], Description="Temporary YOLO Nitro qualification SSH",
        VpcId=plan["vpcId"], TagSpecifications=[{"ResourceType": "security-group", "Tags": tags(plan)}])["GroupId"]
    ec2.authorize_security_group_ingress(GroupId=sg, IpPermissions=[{"IpProtocol": "tcp", "FromPort": 22,
        "ToPort": 22, "IpRanges": [{"CidrIp": ip + "/32", "Description": "Qualification operator"}]}])
    # Only HTTPS leaves the parent during installation and fixed cloud relays.
    ec2.revoke_security_group_egress(GroupId=sg, IpPermissions=[{"IpProtocol": "-1", "IpRanges": [{"CidrIp": "0.0.0.0/0"}]}])
    ec2.authorize_security_group_egress(GroupId=sg, IpPermissions=[{"IpProtocol": "tcp", "FromPort": 443,
        "ToPort": 443, "IpRanges": [{"CidrIp": "0.0.0.0/0"}]}])
    subnets = sorted(ec2.describe_subnets(Filters=[{"Name": "vpc-id", "Values": [plan["vpcId"]]},
        {"Name": "default-for-az", "Values": ["true"]}])["Subnets"], key=lambda s: s["AvailabilityZone"])
    if not subnets:
        raise ValueError("reviewed default VPC has no default subnet")
    user_data = (Path(__file__).parent / "qualification_parent.sh").read_text().replace(
        "@BUDGET_STOP_AT@", stop.strftime("%Y-%m-%d %H:%M:%S UTC")).replace(
        "@ENCLAVE_CPUS@", str(profile[2])).replace("@ENCLAVE_MEMORY_MIB@", str(profile[3]))
    args = dict(ImageId=plan["ami"], InstanceType=plan["instanceType"], MinCount=1, MaxCount=1,
        ClientToken=plan["runId"], KeyName=plan["runId"], EnclaveOptions={"Enabled": True},
        IamInstanceProfile={"Name": "yolo-nitro-parent-v1"}, InstanceInitiatedShutdownBehavior="terminate",
        MetadataOptions={"HttpTokens": "required", "HttpPutResponseHopLimit": 1, "HttpEndpoint": "enabled"},
        NetworkInterfaces=[{"DeviceIndex": 0, "SubnetId": subnets[0]["SubnetId"], "Groups": [sg],
            "AssociatePublicIpAddress": True, "DeleteOnTermination": True}], UserData=user_data,
        BlockDeviceMappings=[{"DeviceName": "/dev/xvda", "Ebs": {"VolumeSize": profile[1], "VolumeType": "gp3", "DeleteOnTermination": True}}],
        TagSpecifications=[{"ResourceType": r, "Tags": tags(plan)} for r in ["instance", "volume", "network-interface"]])
    write_json(directory / "launch-request.json", args)
    result = ec2.run_instances(**args)
    write_json(directory / "launch.json", result)
    print(json.dumps({"runId": plan["runId"], "instanceId": result["Instances"][0]["InstanceId"], "budgetStopAt": plan["budgetStopAt"]}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["launch", "cleanup", "guard"])
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--credentials", required=True, type=Path)
    args = parser.parse_args()
    os.umask(0o077)
    plan = json.loads(args.plan.read_text())
    if args.operation == "guard" and cleanup_reason(plan, args.plan.parent) is None:
        return
    aws = session(args.credentials)
    if client(aws, "sts", plan).get_caller_identity()["Account"] != plan["accountId"]:
        raise ValueError("AWS account differs from approved plan")
    if args.operation == "launch":
        launch(aws, plan, args.plan.parent)
    else:
        cleanup(aws, plan, args.plan.parent / "cleanup.json")
        if args.operation == "guard":
            final_inventory(aws, plan, args.plan.parent)


if __name__ == "__main__":
    main()
