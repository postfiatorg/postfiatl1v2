#!/usr/bin/env bash
# Run this reviewed IAM-only bootstrap in AWS CloudShell as the account administrator.
# Account: 146575258003. Existing group: yolosetup.
# Creates one EC2 role/profile and grants that group PassRole only for this role to EC2.
# us-east-1 stores the IAM-only stack; this does not choose the later compute region.
set -euo pipefail
export AWS_PAGER=""
yolo_bootstrap_account="$(aws sts get-caller-identity --query Account --output text)"
if [[ "$yolo_bootstrap_account" != "146575258003" ]]; then
  printf '%s\n' 'Wrong AWS account: expected 146575258003.' >&2
  exit 1
fi
aws iam get-group --group-name yolosetup --query Group.GroupName --output text >/dev/null
yolo_bootstrap_dir="$(mktemp -d)"
trap 'rm -rf -- "$yolo_bootstrap_dir"' EXIT
cat > "$yolo_bootstrap_dir/template.json" <<'YOLO_IAM_TEMPLATE'
{
  "AWSTemplateFormatVersion": "2010-09-09",
  "Description": "YOLO Nitro parent identity and EC2-only pass-role permission. IAM resources only; no compute, key, bucket or network resources.",
  "Metadata": {
    "Runbook": "postfiatl1v2/tools/nav-reserve-proof/schwab-tee/README.md",
    "ExpectedAccountId": "146575258003",
    "ExistingGroup": "yolosetup"
  },
  "Resources": {
    "ParentRole": {
      "Type": "AWS::IAM::Role",
      "Properties": {
        "RoleName": "yolo-nitro-parent-v1",
        "Path": "/",
        "Description": "YOLO Nitro parent; KMS access is granted later by the dedicated attested-recipient key policy.",
        "AssumeRolePolicyDocument": {
          "Version": "2012-10-17",
          "Statement": [
            {
              "Effect": "Allow",
              "Principal": { "Service": "ec2.amazonaws.com" },
              "Action": "sts:AssumeRole"
            }
          ]
        },
        "Tags": [
          { "Key": "Project", "Value": "yolo-tee" },
          { "Key": "Scope", "Value": "portfolio-targets-only" }
        ]
      }
    },
    "ParentInstanceProfile": {
      "Type": "AWS::IAM::InstanceProfile",
      "Properties": {
        "InstanceProfileName": "yolo-nitro-parent-v1",
        "Path": "/",
        "Roles": [{ "Ref": "ParentRole" }]
      }
    },
    "GroupPassRole": {
      "Type": "AWS::IAM::GroupPolicy",
      "DependsOn": "ParentRole",
      "Properties": {
        "GroupName": "yolosetup",
        "PolicyName": "yolo-nitro-parent-pass-role-v1",
        "PolicyDocument": {
          "Version": "2012-10-17",
          "Statement": [
            {
              "Sid": "PassOnlyYoloNitroParentToEc2",
              "Effect": "Allow",
              "Action": "iam:PassRole",
              "Resource": "arn:aws:iam::146575258003:role/yolo-nitro-parent-v1",
              "Condition": {
                "StringEquals": { "iam:PassedToService": "ec2.amazonaws.com" }
              }
            }
          ]
        }
      }
    }
  },
  "Outputs": {
    "ParentRoleArn": { "Value": { "Fn::GetAtt": ["ParentRole", "Arn"] } },
    "ParentInstanceProfileArn": { "Value": { "Fn::GetAtt": ["ParentInstanceProfile", "Arn"] } }
  }
}
YOLO_IAM_TEMPLATE
aws cloudformation validate-template \
  --region us-east-1 \
  --template-body "file://$yolo_bootstrap_dir/template.json" >/dev/null
aws cloudformation create-stack \
  --region us-east-1 \
  --stack-name yolo-nitro-iam-v1 \
  --template-body "file://$yolo_bootstrap_dir/template.json" \
  --capabilities CAPABILITY_NAMED_IAM \
  --tags Key=Project,Value=yolo-tee
if ! aws cloudformation wait stack-create-complete \
  --region us-east-1 --stack-name yolo-nitro-iam-v1; then
  aws cloudformation describe-stack-events \
    --region us-east-1 --stack-name yolo-nitro-iam-v1 \
    --query 'StackEvents[].[LogicalResourceId,ResourceStatus,ResourceStatusReason]' \
    --output table
  exit 1
fi
aws cloudformation describe-stacks \
  --region us-east-1 --stack-name yolo-nitro-iam-v1 \
  --query 'Stacks[0].Outputs' --output table
