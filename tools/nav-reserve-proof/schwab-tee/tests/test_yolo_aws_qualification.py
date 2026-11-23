"""Ownership and key-release regressions for the opt-in cloud qualification."""
from pathlib import Path

import pytest


DIRECTORY = Path(__file__).parents[1] / 'operator'


@pytest.fixture
def infra(monkeypatch):
    monkeypatch.syspath_prepend(str(DIRECTORY))
    import qualification_infra
    return qualification_infra


def test_cleanup_discovery_requires_both_owner_tags(infra):
    plan = {'runId': 'test-only', 'expiresAt': '2026-09-05T22:34:21+00:00'}
    assert infra.filters(plan) == [
        {'Name': 'tag:Project', 'Values': ['yolo-nitro-qualification']},
        {'Name': 'tag:QualificationRun', 'Values': ['test-only']}]


def test_key_policy_has_no_parent_or_operator_plaintext_grant(infra):
    from qualification_kms import key_policy
    policy = key_policy({'accountId': '111122223333', 'runId': 'test-only'},
        {str(i): bytes([i + 1]).hex() * 48 for i in range(3)})
    statements = {s['Sid']: s for s in policy['Statement']}
    release = statements['ExactEnclaveRecipientOnly']
    assert release['Principal'] == {'AWS': 'arn:aws:iam::111122223333:role/yolo-nitro-parent-v1'}
    conditions = release['Condition']['StringEquals']
    assert set(conditions) == {'kms:RecipientAttestation:PCR0', 'kms:RecipientAttestation:PCR1',
        'kms:RecipientAttestation:PCR2', 'kms:EncryptionContext:Project', 'kms:EncryptionContext:QualificationRun'}
    assert statements['DenyPlaintextRelease']['Condition'] == {'Null': {'kms:RecipientAttestation:PCR0': 'true'}}
    assert statements['DenyPlaintextRelease']['Effect'] == 'Deny'
    assert statements['OperatorEncryptSyntheticFixture']['Action'] == 'kms:Encrypt'
    assert not set(statements['OperatorKeyAdministration']['Action']) & {'kms:Decrypt', 'kms:GenerateDataKey', 'kms:CreateGrant'}


def test_revocation_removes_only_recipient_release(infra):
    from qualification_kms import key_policy
    plan = {'accountId': '111122223333', 'runId': 'test-only'}
    pcrs = {str(i): bytes([i + 1]).hex() * 48 for i in range(3)}
    allowed = key_policy(plan, pcrs)
    revoked = key_policy(plan, pcrs, release=False)
    assert revoked['Statement'] == allowed['Statement'][:-1]


@pytest.mark.parametrize('pcrs', [{}, {'0': '00' * 48, '1': '11' * 48, '2': '22' * 48},
    {'0': 'aa' * 48, '1': '11' * 48, '2': 'invalid'}])
def test_key_policy_rejects_unmeasured_or_debug_images(infra, pcrs):
    from qualification_kms import key_policy
    with pytest.raises(ValueError):
        key_policy({'accountId': '111122223333', 'runId': 'test-only'}, pcrs)


def prover_plan():
    return {'resourceProfile': 'private-prover-v1', 'instanceType': 'm6i.8xlarge',
        'rootVolumeGiB': 40, 'instanceLimit': 1, 'kmsKeyLimit': 1,
        'lifecycle': 'completion-or-budget', 'region': 'us-east-1',
        'spendCeilingUsd': 25, 'ancillarySpendReserveUsd': 1,
        'priorQualificationSpendReserveUsd': 9}


@pytest.mark.parametrize('date', ['2026-09-05T22:34:21+00:00', '2026-09-06T02:00:00+00:00',
    '2020-01-01T00:00:00+00:00', '2099-01-01T00:00:00+00:00'])
def test_calendar_fields_neither_expire_nor_extend_current_authorization(infra, date):
    plan = {**prover_plan(), 'expiresAt': date, 'authorizationId': 'historical-label'}
    assert infra.resource_profile(plan) == ('m6i.8xlarge', 40, 16, 110592)
    assert infra.budget_seconds(plan) == 35036


@pytest.mark.parametrize('field,value', [
    ('instanceLimit', 2), ('instanceType', 'm6i.16xlarge'), ('rootVolumeGiB', 80),
    ('region', 'us-west-2'), ('spendCeilingUsd', 26), ('priorQualificationSpendReserveUsd', 0),
    ('priorQualificationSpendReserveUsd', 24), ('priorQualificationSpendReserveUsd', 'NaN'),
    ('ancillarySpendReserveUsd', 0), ('ancillarySpendReserveUsd', 'Infinity'),
    ('kmsKeyLimit', 2), ('resourceProfile', 'unreviewed'), ('lifecycle', 'fixed-date')])
def test_prover_profile_rejects_expanded_resources_or_invalid_budget(infra, field, value):
    with pytest.raises(ValueError):
        infra.resource_profile({**prover_plan(), field: value})


def test_spend_reserve_reduces_compute_allowance(infra):
    plan = prover_plan()
    seconds = infra.budget_seconds(plan)
    assert 9 + 1 + (seconds + infra.TERMINATION_RESERVE_SECONDS) / 3600 * 1.536 <= 25
    assert infra.budget_seconds({**plan, 'priorQualificationSpendReserveUsd': 10}) < seconds


def test_accelerated_profile_uses_existing_instance_capacity_without_more_spend(infra):
    plan = {**prover_plan(), 'resourceProfile': 'private-prover-avx512-v1'}
    assert infra.resource_profile(plan) == ('m6i.8xlarge', 40, 30, 110592)
    assert infra.budget_seconds(plan) == infra.budget_seconds(prover_plan())


def test_guard_waits_for_outputs_or_actual_budget_not_old_expiration(infra, tmp_path):
    from datetime import datetime, timezone
    plan = {**prover_plan(), 'runId': 'test-only', 'expiresAt': '2000-01-01T00:00:00+00:00'}
    now = datetime(2026, 9, 6, 12, tzinfo=timezone.utc)
    assert infra.cleanup_reason(plan, tmp_path, now) is None  # guard can be installed before launch
    plan['budgetStopAt'] = '2026-09-06T13:00:00+00:00'
    assert infra.cleanup_reason(plan, tmp_path, now) is None
    assert infra.cleanup_reason(plan, tmp_path, datetime(2026, 9, 6, 13, tzinfo=timezone.utc)) == 'cumulative-budget-safeguard'
    infra.write_json(tmp_path / 'completion.json', {'runId': 'test-only', 'outputsRetained': True, 'status': 'VERIFIED'})
    assert infra.cleanup_reason(plan, tmp_path, now) == 'outputs-retained'
    infra.write_json(tmp_path / 'completion.json', {'runId': 'test-only', 'outputsRetained': True, 'status': 'FAILED'})
    assert infra.cleanup_reason(plan, tmp_path, now) == 'outputs-retained'
    infra.write_json(tmp_path / 'completion.json', {'runId': 'other-run', 'outputsRetained': True})
    with pytest.raises(ValueError):
        infra.cleanup_reason(plan, tmp_path, now)
    (tmp_path / 'completion.json').write_text('interrupted write')
    assert infra.cleanup_reason(plan, tmp_path, datetime(2026, 9, 6, 13, tzinfo=timezone.utc)) == 'cumulative-budget-safeguard'


def test_parent_budget_safeguard_installed_before_network_setup():
    script = (DIRECTORY / 'qualification_parent.sh').read_text()
    assert 'OnCalendar=@BUDGET_STOP_AT@' in script
    assert '@EXPIRES_AT@' not in script
    assert script.index('systemctl enable --now yolo-qualification-expire.timer') < script.index('dnf install')


@pytest.mark.parametrize('path', ['../aws.txt', '/etc/passwd', 'MU/../../ssh_key',
    'archive/secret.pem', 'MU/target.json', 'MU/private-replay.json', 'event-.json',
    'MU//proof.bin', 'MU/./proof.bin', 'archive/' + 'z' * 64 + '.ciphertext'])
def test_delivery_rejects_private_and_unexpected_paths(infra, path):
    from qualification_delivery import allowed_output
    assert not allowed_output(path)


@pytest.mark.parametrize('path', ['recipient.json', 'event-21.json', 'MU/proof.bin',
    'NVDA/public-values.bin', 'NVDA/target-ready.json', 'archive/' + 'a' * 64 + '.ciphertext'])
def test_delivery_accepts_public_and_encrypted_outputs(infra, path):
    from qualification_delivery import allowed_output
    assert allowed_output(path)


@pytest.mark.parametrize('load,active,expected', [('loaded', 'active', False),
    ('loaded', 'activating', False), ('not-found', 'inactive', False),
    ('loaded', 'inactive', True), ('loaded', 'failed', True)])
def test_delivery_completion_requires_terminal_started_service(infra, load, active, expected):
    from qualification_delivery import terminal
    assert terminal({'service': {'LoadState': load, 'ActiveState': active}}) is expected


@pytest.mark.parametrize('status', ['VERIFIED', 'FAILED'])
def test_collected_transient_service_with_complete_outputs_is_terminal(infra, status):
    from qualification_delivery import terminal
    report = {'service': {'LoadState': 'not-found', 'ActiveState': 'inactive'},
        'files': dict.fromkeys(['result.json', 'timing.json', 'terminate.log']),
        'lastEvent': 'postfiat.yolo.demo_complete.v1', 'lastStatus': status, 'status': status}
    assert terminal(report)
    for name in report['files']:
        assert not terminal({**report, 'files': {k: v for k, v in report['files'].items() if k != name}})
    assert not terminal({**report, 'lastEvent': 'postfiat.yolo.prover_status.v1'})
    assert not terminal({**report, 'status': None})
    assert not terminal({**report, 'service': {'LoadState': 'not-found', 'ActiveState': 'active'}})


@pytest.mark.parametrize('status', ['VERIFIED', 'FAILED'])
@pytest.mark.parametrize('collected', [False, True])
def test_delivery_marks_completion_only_after_hash_checked_retention(infra, tmp_path, monkeypatch, status, collected):
    import hashlib
    import json
    from types import SimpleNamespace
    import qualification_delivery as delivery
    payload = b'public proof or encrypted diagnostic'
    pin = {'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()}
    report = {'service': {'LoadState': 'loaded', 'ActiveState': 'inactive'},
        'files': {'MU/proof.bin': pin}, 'lastEvent': 'complete',
        'lastStatus': status, 'lastSymbol': 'MU', 'status': status}
    if collected:
        report['service']['LoadState'] = 'not-found'
        report['lastEvent'] = 'postfiat.yolo.demo_complete.v1'
        report['files'].update(dict.fromkeys(['result.json', 'timing.json', 'terminate.log'], pin))
    infra.write_json(tmp_path / 'delivery.json', {'remoteRoot': '/public-output', 'service': 'demo.service'})
    infra.write_json(tmp_path / 'plan.json', {'runId': 'test-only'})
    infra.write_json(tmp_path / 'ssh-command.json', ['ssh', 'reviewed-parent'])
    corrupt = True
    def run(command, **kwargs):
        if command[0] == 'ssh':
            return SimpleNamespace(stdout=json.dumps(report).encode())
        assert command[0] == 'scp'
        assert not (tmp_path / 'completion.json').exists()
        Path(command[-1]).write_bytes(b'partial output' if corrupt else payload)
        return SimpleNamespace(returncode=0)
    monkeypatch.setattr(delivery.subprocess, 'run', run)
    with pytest.raises(ValueError, match='changed during transfer'):
        delivery.deliver(tmp_path)
    assert not (tmp_path / 'completion.json').exists()
    corrupt = False
    delivery.deliver(tmp_path)
    assert (tmp_path / 'output/MU/proof.bin').read_bytes() == payload
    marker = json.loads((tmp_path / 'completion.json').read_bytes())
    assert marker['outputsRetained'] and marker['status'] == status


@pytest.mark.parametrize('changed_identity', [False, True])
def test_delivery_remembers_started_service_before_collection(infra, tmp_path, monkeypatch, changed_identity):
    import hashlib
    import json
    from types import SimpleNamespace
    import qualification_delivery as delivery
    payload = b'public launch diagnostic'
    report = {'service': {'LoadState': 'loaded', 'ActiveState': 'active'},
        'files': {'launch.stderr': {'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()}},
        'lastEvent': None, 'lastStatus': None, 'lastSymbol': None, 'status': None}
    config = {'remoteRoot': '/public-output', 'service': 'started.service'}
    infra.write_json(tmp_path / 'delivery.json', config)
    infra.write_json(tmp_path / 'plan.json', {'runId': 'test-only'})
    infra.write_json(tmp_path / 'ssh-command.json', ['ssh', 'reviewed-parent'])
    def run(command, **kwargs):
        if command[0] == 'ssh':
            return SimpleNamespace(stdout=json.dumps(report).encode())
        Path(command[-1]).write_bytes(payload)
        return SimpleNamespace(returncode=0)
    monkeypatch.setattr(delivery.subprocess, 'run', run)
    delivery.deliver(tmp_path)
    assert not (tmp_path / 'completion.json').exists()
    report['service'] = {'LoadState': 'not-found', 'ActiveState': 'inactive'}
    if changed_identity:
        infra.write_json(tmp_path / 'delivery.json', {**config, 'service': 'never-started.service'})
    delivery.deliver(tmp_path)
    assert (tmp_path / 'completion.json').exists() is not changed_identity
    if not changed_identity:
        assert json.loads((tmp_path / 'completion.json').read_bytes())['status'] == 'FAILED'


@pytest.mark.parametrize('remaining', ['instance', 'volume', 'group', 'pair', 'interface', 'kms', None])
def test_cleanup_guard_stays_enabled_until_fresh_inventory_is_empty(infra, tmp_path, monkeypatch, remaining):
    from datetime import datetime, timezone
    from unittest.mock import Mock
    ec2, kms = Mock(), Mock()
    instance = {'InstanceId': 'owned-instance', 'State': {'Name': 'running' if remaining == 'instance' else 'terminated'},
        'LaunchTime': datetime.now(timezone.utc), 'StateTransitionReason': 'User initiated'}
    ec2.get_paginator.return_value.paginate.return_value = [{'Reservations': [{'Instances': [instance]}]}]
    ec2.describe_volumes.return_value = {'Volumes': [{}] if remaining == 'volume' else []}
    ec2.describe_security_groups.return_value = {'SecurityGroups': [{}] if remaining == 'group' else []}
    ec2.describe_key_pairs.return_value = {'KeyPairs': [{}] if remaining == 'pair' else []}
    ec2.describe_network_interfaces.return_value = {'NetworkInterfaces': [{}] if remaining == 'interface' else []}
    kms.describe_key.return_value = {'KeyMetadata': {'KeyId': 'owned-key',
        'KeyState': 'Enabled' if remaining == 'kms' else 'PendingDeletion', 'DeletionDate': 'later'}}
    monkeypatch.setattr(infra, 'client', lambda aws, service, plan: ec2 if service == 'ec2' else kms)
    monkeypatch.setattr(infra, 'owned_keys', lambda kms, plan: ['owned-key'])
    disabled = []
    def run(command, **kwargs):
        assert (tmp_path / 'final-inventory.json').exists()
        disabled.append(command[-1])
    monkeypatch.setattr(infra.subprocess, 'run', run)
    plan = {**prover_plan(), 'runId': 'test-only', 'cleanupTimer': 'test.timer', 'deliveryTimer': 'delivery.timer'}
    if remaining:
        with pytest.raises(RuntimeError, match='keep cleanup guard active'):
            infra.final_inventory(None, plan, tmp_path)
        assert not disabled and not (tmp_path / 'final-inventory.json').exists()
    else:
        infra.final_inventory(None, plan, tmp_path)
        assert disabled == ['test.timer', 'delivery.timer']
