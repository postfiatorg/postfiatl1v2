# Validator Emergency Key Rotation Runbook

Status: controlled-testnet operator runbook
Last updated: 2026-05-13

## Purpose

Use this when a validator hot key is suspected compromised, lost, or exposed.
The controlled-testnet invariant is:

- the validator registry rotates to the replacement public key,
- stale key staging is rejected after activation,
- stale-key block votes are rejected against the live registry root,
- replacement-key votes remain valid,
- public P0/readiness reports do not contain private key material.

This runbook is evidence-backed by:

```text
reports/testnet-p0-network-gate-local-fault-summary/testnet-p0-network-gate-20260512T171824Z.json
reports/testnet-p0-network-gate-local-fault-summary/logs/local-readiness/logs/validator-registry-update-smoke.json
reports/testnet-p0-network-gate-local-fault-summary/logs/local-readiness/logs/validator-registry-update-smoke-logs/emergency-key-rotation/emergency-key-rotation.json
reports/testnet-p0-network-gate-remote-registry-fault-tolerance-5/testnet-p0-network-gate-20260512T132218Z.json
reports/testnet-p0-network-gate-remote-registry-fault-tolerance-5/logs/remote/validator-registry/testnet-remote-validator-registry-drill-20260512T134336Z.json
reports/testnet-remote-emergency-key-rotation-rehearsal/replay-20260512T171824Z/testnet-remote-emergency-key-rotation-rehearsal-20260512T175102Z.json
reports/testnet-remote-emergency-key-rotation-rehearsal/p0-derivation-20260512T171824Z/testnet-remote-emergency-key-rotation-rehearsal-20260512T175658Z.json
reports/testnet-p0-network-gate-remote-emergency-rehearsal/testnet-p0-network-gate-20260512T180400Z.json
reports/testnet-p0-network-gate-remote-emergency-rehearsal/logs/remote/emergency-key-rotation/testnet-remote-emergency-key-rotation-rehearsal.json
reports/testnet-remote-emergency-key-rotation-rehearsal/head-e977d87-waiver-20260513T214519Z/testnet-remote-emergency-key-rotation-rehearsal-20260513T214519Z.json
```

## Operator Procedure

1. Stop the affected validator's signing service or remove its hot key from
   service before announcing the rotation.

2. Generate or retrieve the replacement validator key on the validator host.
   Keep the private key local to the validator machine. Public reports and
   provision bundles must not include private key fields.

3. Compute the old and new registry roots, then create a rotate-key update:

```bash
postfiat-node validator-registry-root --data-dir <node-dir> --validators <active-validator-csv>
postfiat-node validator-registry-update \
  --data-dir <node-dir> \
  --validators <active-validator-csv> \
  --support <support-validator-csv> \
  --activation-height <height> \
  --previous-registry-root <old-root> \
  --new-registry-root <new-root> \
  --previous-validators <active-validator-csv> \
  --new-validators <active-validator-csv> \
  --operation rotate_key \
  --subject-node-id <validator-id> \
  --previous-record-file <old-validator-record.json> \
  --new-record-file <new-validator-record.json> \
  --update-file <rotate-key-update.json>
```

4. Verify and order the update through governance:

```bash
postfiat-node validator-registry-update-verify \
  --data-dir <node-dir> \
  --update-file <rotate-key-update.json> \
  --previous-registry-file <previous-registry.json> \
  --new-registry-file <new-registry.json>

postfiat-node governance-batch \
  --data-dir <node-dir> \
  --registry-update-file <rotate-key-update.json> \
  --batch-file <governance-batch.json>

postfiat-node apply-governance-batch \
  --data-dir <node-dir> \
  --batch-file <governance-batch.json>
```

5. After activation, stage the replacement key on the affected validator:

```bash
postfiat-node validator-key-stage \
  --data-dir <node-dir> \
  --source-key-file <replacement-validator-keys.json> \
  --validator-id <validator-id> \
  --replace

postfiat-node validate-local-keys --data-dir <node-dir> --validators <validator-count>
```

6. Treat any attempt to stage the stale key after activation as a hard failure.
   The expected error includes:

```text
does not match current validator registry
```

7. Treat any stale-key block vote after activation as invalid. The expected
   vote error includes:

```text
public key does not match registry
```

8. Confirm the replacement key can sign a registry-valid vote and that compact
   certificate votes do not carry repeated public keys:

```bash
postfiat-node block-vote \
  --data-dir <node-dir> \
  --key-file <replacement-validator-keys.json> \
  --validator <validator-id> \
  --proposal-file <proposal.json> \
  --batch-file <batch.json> \
  --height <height> \
  --vote-file <replacement.block-vote.json>

jq -e '.schema == "postfiat.block_vote.v1"
  and (.vote.public_key_hex == null or .vote.public_key_hex == "")
  and (.vote.signature_hex | length > 0)' <replacement.block-vote.json>
```

## Evidence Replay

From a P0 report, verify the emergency rotation summary:

```bash
jq -e '.local_validator_registry_fault_drills_ok == true
  and .local_validator_registry_faults.emergency_key_rotation_ok == true
  and .local_validator_registry_faults.stale_key_stage_rejected == true
  and .local_validator_registry_faults.stale_key_vote_rejected == true
  and .local_validator_registry_faults.replacement_key_vote_ok == true' \
  reports/testnet-p0-network-gate-local-fault-summary/testnet-p0-network-gate-20260512T171824Z.json
```

Replay the underlying emergency report:

```bash
jq -e '.schema == "postfiat-testnet-emergency-key-rotation-v1"
  and .emergency_key_rotation_ok == true
  and .replacement_key_stage_ok == true
  and .rotated_public_key_changed == true
  and .stale_key_stage_rejected == true
  and .stale_key_vote_rejected == true
  and .replacement_key_vote_ok == true' \
  reports/testnet-p0-network-gate-local-fault-summary/logs/local-readiness/logs/validator-registry-update-smoke-logs/emergency-key-rotation/emergency-key-rotation.json
```

Check that public report surfaces did not leak private key fields:

```bash
grep -R -E 'private_key_hex|"private_key"|secret_key|BEGIN [A-Z ]*PRIVATE KEY' \
  reports/testnet-p0-network-gate-local-fault-summary/testnet-p0-network-gate-20260512T171824Z.json \
  reports/testnet-p0-network-gate-local-fault-summary/logs/local-readiness/readiness.json \
  reports/testnet-p0-network-gate-local-fault-summary/logs/local-readiness/logs
```

That grep should produce no output.

Replay the latest remote registry drill summary:

```bash
jq -e '.status == "passed"
  and .mode == "remote"
  and .p0_network_ok == true
  and .remote_validator_registry_update_ok == true
  and .remote_validator_registry_fault_tolerance_ok == true' \
  reports/testnet-p0-network-gate-remote-registry-fault-tolerance-5/testnet-p0-network-gate-20260512T132218Z.json
```

Replay the underlying remote validator-registry drill:

```bash
jq -e '. as $report
  | .schema == "postfiat-testnet-remote-validator-registry-drill-v1"
  and .ok == true
  and .governance_ordering.update_verified == true
  and .governance_ordering.round.certificate_validators == .previous_validators
  and .post_suspend_ordering.round.certificate_validators == .new_validators
  and (.post_suspend_ordering.round.certificate_validators | index($report.subject_node)) == null
  and .post_suspend_ordering.round.local_apply_verified == true
  and .post_suspend_ordering.round.subject_failure_recorded == true
  and .post_suspend_convergence.active_validators_converged == true
  and .active_registry_roots.active_registry_roots_verified == true
  and .post_suspend_fault_tolerance_required == true
  and .post_suspend_fault_tolerance.verified == true
  and .post_suspend_fault_tolerance.outage_convergence.active_quorum_preserved == true
  and .post_suspend_fault_tolerance.recovery_replay.verified == true
  and .post_suspend_fault_tolerance.final_convergence.active_validators_converged == true' \
  reports/testnet-p0-network-gate-remote-registry-fault-tolerance-5/logs/remote/validator-registry/testnet-remote-validator-registry-drill-20260512T134336Z.json
```

For a four-validator controlled cohort, suspending one validator leaves only
three active validators. The post-suspend fault-tolerance sub-drill is then
not meaningful and is expected to report
`post_suspend_fault_tolerance_required == false` with the skip reason
`active validator count after suspend is below 4`. The emergency rehearsal
accepts that specific skip because P0 separately exercises remote partial
outage and RPC catch-up before the emergency-key rehearsal.

Replay the manifest-bound remote emergency key-rotation rehearsal:

```bash
jq -e '.schema == "postfiat-testnet-remote-emergency-key-rotation-rehearsal-v1"
  and .status == "passed"
  and .remote_emergency_key_rotation_rehearsal_ok == true
  and .checks.operator_plan_ok == true
  and .checks.operator_private_key_policy_ok == true
  and .checks.operator_command_surface_ok == true
  and .checks.operator_manifest_bound_ok == true
  and .checks.local_emergency_key_rotation_evidence_ok == true
  and .checks.remote_registry_suspension_evidence_ok == true
  and .checks.remote_fault_tolerance_evidence_ok == true
  and .checks.runbook_procedure_ok == true' \
  reports/testnet-remote-emergency-key-rotation-rehearsal/p0-derivation-20260512T171824Z/testnet-remote-emergency-key-rotation-rehearsal-20260512T175658Z.json
```

Replay the live remote P0 gate summary for this runbook:

```bash
jq -e '.status == "passed"
  and .mode == "remote"
  and .p0_network_ok == true
  and .remote_validator_registry_fault_tolerance_ok == true
  and .remote_emergency_key_rotation_rehearsal_ok == true' \
  reports/testnet-p0-network-gate-remote-emergency-rehearsal/testnet-p0-network-gate-20260512T180400Z.json
```

Generate a fresh rehearsal report from archived P0 reports:

```bash
P0_REPORT=reports/testnet-p0-network-gate-remote-registry-fault-tolerance-5/testnet-p0-network-gate-20260512T132218Z.json \
LOCAL_P0_REPORT=reports/testnet-p0-network-gate-local-fault-summary/testnet-p0-network-gate-20260512T171824Z.json \
scripts/testnet-remote-emergency-key-rotation-rehearsal
```

## Current Gap

This runbook is local/P0-evidence backed, references the latest remote
validator-registry drill, and now has a manifest-bound rehearsal checker wired
into the remote P0 gate with live `remote_emergency_key_rotation_rehearsal_ok`
evidence. The next operator-hardening step is capture-threshold and DDoS
drills.
