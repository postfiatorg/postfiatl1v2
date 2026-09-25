import json, pathlib
C = pathlib.Path(__file__).parent
def rec(n): return json.loads((C / 'receipts' / f'{n}.json').read_text())
def out(n): return json.loads((C / 'logs' / f'{n}.stdout').read_text())
vs = out('merged-verify-state-remote'); bl = vs['block_log']
vss = out('merged-verify-state-signed')['block_log'] if (C / 'receipts/merged-verify-state-signed.json').exists() else None
ck = out('merged-verify-checkpoint-remote'); cks = out('merged-verify-checkpoint-signed'); ckr4 = out('r4-verify-checkpoint-signed')
st = out('merged-status-remote')
names = ['merged-import-signed', 'merged-verify-checkpoint-signed', 'merged-status-signed', 'merged-import-remote-unsigned',
         'merged-verify-checkpoint-remote', 'merged-status-remote', 'merged-verify-state-remote', 'merged-import-wrong-key',
         'merged-import-tampered-signature', 'r4-import-signed', 'r4-verify-checkpoint-signed', 'merged-verify-state-signed']
pre = rec('preflight')
res = dict(
    schema='postfiat.release-repair.canary-backup-1036.v1',
    status='PASS' if all(rec(n)['status'] == 'PASS' for n in names) else 'FAIL',
    source='validator-1 (95.179.184.122) /var/lib/postfiat/pre-rollout-snapshots/fastpay-committee-20260925-r4-validator-1-finalized-checkpoint',
    source_kind='unsigned finalized-checkpoint export written by the r4 safe-rollout backup step before r4 was applied',
    remote_size='219M', remote_bytes=229276689, remote_files=21,
    copy='rsync -a over ssh, pull only, no --delete; local copy made read-only (chmod -R a-w); kept under ~/.cache/release-repair-20260925/canary-backup-1036/, not in git',
    copy_matches_r4_evidence_unsigned='IDENTICAL: sha256 of all 21 files equal ~/.postfiat/deployments/fastpay-committee-20260925-r4/evidence/pre-rollout-backup/backup-unsigned',
    signed_copy='r4 evidence backup-signed (the rollout tool signs locally). Its 21 data files equal the validator-1 copy; only snapshot_manifest.json differs because the signed export rewrites it.',
    signed_manifest_sha256=pre['signed_manifest_sha256'],
    r4_state_signed_manifest_sha256='ee2ca6c68b45778ebaedf3081681401d20226304b639740ac50627933d3feb1a',
    trusted_publisher_key=dict(path=pre['trusted_public_key'], sha256=pre['trusted_public_key_sha256'],
        publisher='pf4ebb800c5a985cfca61e3833c40ea60b6767644a', schema='postfiat.snapshot_publisher_key.v1',
        note='Public key only. It matches the publisher in the signed manifest. The r4 release files carry deployment.public.json (deployment publisher pfc531e0...), which the node rejects as a snapshot key; validator-1 has no /etc/postfiat/snapshot-publisher.public.json.'),
    binaries=pre['binaries'],
    signature_verification=dict(
        merged_import_signed=rec('merged-import-signed')['status'],
        tampered_signature_rejected=rec('merged-import-tampered-signature')['status'],
        deployment_key_rejected=rec('merged-import-wrong-key')['status']),
    merged_checkpoint_verification=dict(signed_import=cks['verified'], validator_1_copy=ck['verified'],
        height=ck['checkpoint_height'], tip=ck['checkpoint_block_hash'], state_root=ck['checkpoint_state_root'],
        certificate_id=ck['certificate_id'], committee_epoch=ck['committee_epoch']),
    merged_full_history_verify_state=dict(data='validator-1 copy imported with snapshot-import-finalized-checkpoint', verified=vs['verified'],
        block_log_verified=bl['verified'], block_count=bl['block_count'], tip=bl['tip_hash'], state_root=bl['state_root'],
        elapsed_seconds=rec('merged-verify-state-remote')['elapsed_seconds'],
        matches_r4_recorded_backup=(bl['block_count'] == 1036 and bl['tip_hash'].startswith('4d04d290') and bl['state_root'].startswith('ba7cc012'))),
    merged_full_history_verify_state_signed_import=(dict(verified=out('merged-verify-state-signed')['verified'], block_count=vss['block_count'], tip=vss['tip_hash'], state_root=vss['state_root']) if vss else 'NOT_RUN'),
    block_1034=dict(replayed=bl['block_count'] >= 1034 and vs['verified'],
        contents='transparent batch fd775fa6..., proposer validator-2, one accepted transparent_transfer receipt (835a03d7...), block state root 509b0a0d...',
        fastpay_state_at_1036='fastpay_speculative_effects_v1 holds one consensusless transfer effect (committee epoch 1) retained at tip 1036'),
    r4_contrast=dict(executable='44b6794f2f8eab577713dffb6e59880f8ee8c811863e99ed96ed1b0f4ec66bab', mode='checkpoint verification only, separate import',
        import_signed=rec('r4-import-signed')['status'], verified=ckr4['verified'], height=ckr4['checkpoint_height'],
        tip=ckr4['checkpoint_block_hash'], state_root=ckr4['checkpoint_state_root']),
    storage_status=dict(integrity_status=st['storage']['integrity_status'], ordered_batch_count=st['storage']['ordered_batch_count'],
        last_full_verification_height=st['storage']['last_full_verification_height']),
    not_covered='Blocks after 1036 (1037 to the current tip, including 1042 and 1043) are not in this backup and were not replayed.',
    fleet_actions='read-only ssh to validator-1: du, find, df, sha256sum of a missing key path, ls of the r4 release dir, rsync sender. Nothing written, deleted or restarted.',
    receipts=[f'receipts/{n}.json' for n in ['preflight'] + names],
)
print(json.dumps(res, indent=2))
