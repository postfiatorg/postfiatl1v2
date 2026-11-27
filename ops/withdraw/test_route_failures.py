"""Route-scoped failures retry with a bounded backoff and then stop gating other burns."""
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path
from unittest.mock import patch

import service


class RouteFailureTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.jobs = Path(tmp.name)
        # Just after the real time: rows written now (updated_at) are then already due.
        self.clock = datetime.now(timezone.utc).replace(microsecond=0) + timedelta(minutes=1)
        for name, value in (('JOBS', self.jobs), ('utcnow', lambda: self.clock)):
            patcher = patch.object(service, name, value, create=True)
            patcher.start()
            self.addCleanup(patcher.stop)

    def ready(self):
        bucket = {'policy_hash':'p', 'bucket_id':'b', 'outstanding_vault_bridge_atoms':5}
        status = dict(buckets=[bucket], issuer='i', finalized_epoch=2, finalized_reserve_packet_hash='r')
        route = dict(beacon_finality_anchor={'route_profile_hash':'p'}, current_height=1)
        with patch.object(service, 'bridge_status', return_value=status), \
             patch.object(service, 'call', return_value=route), \
             patch.object(service, 'payout_preflight', return_value=None):
            return service.readiness()

    def test_route_failure_retries_with_backoff_then_stops_blocking_readiness(self):
        service.stage(dict(burn_tx_id='aa'*48), 'queued')
        with patch.object(service, 'process', side_effect=RuntimeError('GPU proof failed: host down')) as process:
            service.run_jobs()
            self.assertEqual(process.call_count, 1)
            job = service.read_job('aa'*48)
            self.assertEqual((job['stage'], job['error_scope']), ('failed', 'route'))
            self.assertEqual(self.ready()['blocked_jobs'], ['aa'*48], 'a pending route retry gates new payouts')
            service.run_jobs()
            self.assertEqual(process.call_count, 1, 'a retry waits for its backoff')
            self.clock += timedelta(seconds=301)
            service.run_jobs()
            self.assertEqual(process.call_count, 2, 'a route failure is retried after its backoff')
            self.assertEqual(service.read_job('aa'*48)['route_attempts'], 2)
            for attempt in range(3, service.ROUTE_RETRY_ATTEMPTS + 1):
                self.clock += timedelta(seconds=service.ROUTE_RETRY_BASE_SECONDS * 2 ** (attempt - 2) + 1)
                service.run_jobs()
                self.assertEqual(process.call_count, attempt)
            job = service.read_job('aa'*48)
            self.assertEqual((job['stage'], job['route_attempts'], job['retry_exhausted']),
                             ('failed', service.ROUTE_RETRY_ATTEMPTS, True))
            self.clock += timedelta(days=30)
            service.run_jobs()
            self.assertEqual(process.call_count, service.ROUTE_RETRY_ATTEMPTS, 'retries are bounded')
        ready = self.ready()
        self.assertEqual(ready['blocked_jobs'], [])
        self.assertTrue(ready['ready'])
        self.assertEqual(ready['exhausted_jobs'], ['aa'*48])

    def test_legacy_failed_rows_are_classified_on_load(self):
        service.stage(dict(burn_tx_id='bb'*48, error='This burn is not finalized on the network.'),
                      'failed', 'This burn is not finalized on the network.')
        service.stage(dict(burn_tx_id='cc'*48, error='GPU proof failed: host down'), 'failed', 'GPU proof failed: host down')
        self.assertNotIn('error_scope', service.load(service.job_path('bb'*48)))
        self.assertEqual(sorted(self.ready()['blocked_jobs']), ['cc'*48])
        with patch.object(service, 'process', side_effect=RuntimeError('GPU proof failed: host down')) as process:
            service.run_jobs()
        self.assertEqual([call.args[0]['burn_tx_id'] for call in process.call_args_list], ['cc'*48])
        self.assertEqual(service.read_job('bb'*48)['error_scope'], 'burn')
        self.assertEqual(service.read_job('cc'*48)['route_attempts'], 2, 'the legacy failure counts as the first attempt')

    def test_route_rows_written_before_retries_are_retried_once_loaded(self):
        # The first scoped release wrote error_scope 'route' without retry fields.
        service.stage(dict(burn_tx_id='ff'*48, error='SSH failed', error_scope='route'), 'failed', 'SSH failed')
        with patch.object(service, 'process', side_effect=RuntimeError('SSH failed')) as process:
            service.run_jobs()
        self.assertEqual(process.call_count, 1)
        self.assertEqual(service.read_job('ff'*48)['route_attempts'], 2)

    def test_burn_failures_and_reconciliation_keep_their_meaning(self):
        service.stage(dict(burn_tx_id='dd'*48), 'queued')
        service.stage(dict(burn_tx_id='ee'*48), 'queued')
        def fail(job):
            if job['burn_tx_id'] == 'dd'*48:
                raise service.IneligibleBurn('This burn is not for the Ethereum mainnet USDC vault.')
            raise service.ReconciliationRequired('Reconcile signed Ethereum withdraw')
        with patch.object(service, 'process', side_effect=fail) as process:
            service.run_jobs()
            self.clock += timedelta(days=30)
            service.run_jobs()
        self.assertEqual(process.call_count, 2)
        self.assertEqual(service.read_job('dd'*48)['stage'], 'failed')
        self.assertNotIn('route_attempts', service.read_job('dd'*48))
        self.assertEqual(self.ready()['blocked_jobs'], ['ee'*48])


if __name__ == '__main__':
    unittest.main()
