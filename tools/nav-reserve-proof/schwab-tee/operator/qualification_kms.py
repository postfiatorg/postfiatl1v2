"""Dedicated synthetic qualification keys; no parent plaintext grant."""
import json

from qualification_infra import client, owned_keys, tags, write_json


SYNTHETIC_SECRET = b"YOLO AWS Nitro qualification synthetic secret v1"


def context(plan):
    return {"Project": "yolo-nitro-qualification", "QualificationRun": plan["runId"]}


def key_policy(plan, pcrs, *, release=True):
    if set(pcrs) != {"0", "1", "2"} or any(len(v) != 96 or v == "0" * 96 or
            any(c not in "0123456789abcdef" for c in v) for v in pcrs.values()):
        raise ValueError("three explicit non-debug PCRs are required")
    account = plan["accountId"]
    operator = f"arn:aws:iam::{account}:user/boxpft"
    role = f"arn:aws:iam::{account}:role/yolo-nitro-parent-v1"
    ctx = context(plan)
    conditions = {"StringEquals": {"kms:EncryptionContext:" + k: v for k, v in ctx.items()},
        "ForAllValues:StringEquals": {"kms:EncryptionContextKeys": sorted(ctx)}}
    policy = {"Version": "2012-10-17", "Statement": [
        {"Sid": "OperatorKeyAdministration", "Effect": "Allow", "Principal": {"AWS": operator},
         "Action": ["kms:DescribeKey", "kms:GetKeyPolicy", "kms:PutKeyPolicy", "kms:ListResourceTags",
            "kms:TagResource", "kms:UntagResource", "kms:EnableKey", "kms:DisableKey",
            "kms:ScheduleKeyDeletion", "kms:CancelKeyDeletion", "kms:UpdateKeyDescription"], "Resource": "*"},
        {"Sid": "OperatorEncryptSyntheticFixture", "Effect": "Allow", "Principal": {"AWS": operator},
         "Action": "kms:Encrypt", "Resource": "*", "Condition": conditions},
        {"Sid": "DenyPlaintextRelease", "Effect": "Deny", "Principal": "*",
         "Action": ["kms:Decrypt", "kms:GenerateDataKey"], "Resource": "*",
         "Condition": {"Null": {"kms:RecipientAttestation:PCR0": "true"}}}
    ]}
    if release:
        recipient_conditions = json.loads(json.dumps(conditions))
        recipient_conditions["StringEquals"].update({"kms:RecipientAttestation:PCR" + i: value for i, value in pcrs.items()})
        policy["Statement"].append({"Sid": "ExactEnclaveRecipientOnly", "Effect": "Allow",
            "Principal": {"AWS": role}, "Action": ["kms:Decrypt", "kms:GenerateDataKey"],
            "Resource": "*", "Condition": recipient_conditions})
    return policy


def create_key(aws, plan, pcrs, directory, name):
    kms = client(aws, "kms", plan)
    if len(owned_keys(kms, plan)) >= plan["kmsKeyLimit"]:
        raise ValueError("approved key count already reached")
    policy = key_policy(plan, pcrs)
    write_json(directory / (name + "-policy.json"), policy)
    result = kms.create_key(Policy=json.dumps(policy), Description=plan["runId"] + " " + name + " SYNTHETIC ONLY",
        KeyUsage="ENCRYPT_DECRYPT", KeySpec="SYMMETRIC_DEFAULT", MultiRegion=False,
        Tags=[{"TagKey": item["Key"], "TagValue": item["Value"]} for item in tags(plan)])
    metadata = result["KeyMetadata"]
    write_json(directory / (name + ".json"), metadata)
    return metadata["Arn"]
