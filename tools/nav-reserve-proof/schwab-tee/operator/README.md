# YOLO Nitro parent IAM identity

This is the administrator bootstrap for the EC2 parent identity used by
[B24 Schwab NAV Reading](../../../../docs/runbooks/b24-schwab-nav-reading.md).
It targets account `146575258003` and the existing `yolosetup` group.
The group already grants `boxpft` EC2 access, KMS power-user access and IAM
inspection. It does not grant IAM role creation. No existing EC2 instance
profile was found during the 2026-09-05 inventory.

## Reviewed resources

- Role `yolo-nitro-parent-v1`, trusted only by `ec2.amazonaws.com`.
- Instance profile of the same name, containing only that role.
- Inline group policy `yolo-nitro-parent-pass-role-v1`, granting `yolosetup`
  only `iam:PassRole` for that exact role, conditional on passing it to EC2.

The role starts with no identity permissions. The later dedicated KMS key policy
must grant its attested-recipient operations with the reviewed measurements and
encryption context. This bootstrap does not qualify or grant KMS data access.
It changes no existing S3 policy and creates no compute, key, storage or network
resource. It grants the group no IAM role/policy creation or editing permission.

## Administrator execution

1. Review [`parent-identity.cloudformation.json`](parent-identity.cloudformation.json)
   and the generated, self-contained
   [`yolo_nitro_iam_bootstrap_v1.sh`](yolo_nitro_iam_bootstrap_v1.sh).
2. Open AWS CloudShell as an administrator in account `146575258003`.
3. Upload the shell script using **Actions → Upload file**.
4. Run:

```bash
bash yolo_nitro_iam_bootstrap_v1.sh
```

The script verifies the account and existing group, validates the template with
CloudFormation, and creates a new named IAM stack `yolo-nitro-iam-v1`. It waits
for completion and prints the role/profile ARNs. If stack creation fails, it
prints the stack events. It does not update a pre-existing stack or import a
pre-existing role/profile. Review the events before retrying a failed stack.
`us-east-1` stores this IAM-only stack; the compute deployment region remains
an explicit owner choice.

CloudFormation manages rollback of resources created by this stack. For an
intentional later teardown, detach/terminate the owned parent first, then delete
this exact stack. Never delete the existing `yolosetup` group or unrelated roles.

## Qualification

`cfn-lint 1.56.0` validates the template. `bash -n` checks the shell script, and
the embedded template equals the standalone source. AWS `SimulateCustomPolicy`
passes five cases: intended role/EC2 allowed; another role, another service,
role creation and role-policy modification all receive no grant from this policy.
Simulation is not a deployed permission or hardware qualification.

References: [IAM instance profiles](https://docs.aws.amazon.com/IAM/latest/UserGuide/id_roles_use_switch-role-ec2_instance-profiles.html),
[PassRole](https://docs.aws.amazon.com/IAM/latest/UserGuide/id_roles_use_passrole.html),
[CloudFormation GroupPolicy](https://docs.aws.amazon.com/AWSCloudFormation/latest/TemplateReference/aws-resource-iam-grouppolicy.html).
