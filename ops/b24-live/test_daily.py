"""On-demand ordering against the finalized ledger; all fixtures are local."""
import copy
import fcntl
import os
import shlex
from datetime import datetime,timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import MagicMock,patch

sys.path.insert(0,str(Path(__file__).resolve().parent))
spec=importlib.util.spec_from_file_location('b24_daily',Path(__file__).with_name('daily.py'))
daily=importlib.util.module_from_spec(spec);spec.loader.exec_module(daily)
import reading_alert

class OnDemandTests(unittest.TestCase):
    def setUp(self):
        self.state={'tip':{'height':1370},'ledger':{'nav_assets':[dict(asset_id=daily.B24,finalized_epoch=3)]}}
        self.seed={'schedule':{'opening_epoch':1},'nav_asset_id':daily.B24}

    def test_release5_gate_rejects_a_mixed_fleet_before_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            peers=[(f'validator-{i}', '', '') for i in range(6)]
            fake=MagicMock()
            fake.pftl.PEERS=peers
            fake.runtime.side_effect=lambda peer,sha: (peer[0], {'matches':sha==daily.DAILY_NODE_SHA and peer!=peers[-1]})
            with patch.dict(sys.modules,submit_live=fake),patch.object(daily,'DAILY',root),patch.object(daily,'require_runtime'),patch.object(daily,'collect') as collect:
                with self.assertRaisesRegex(RuntimeError,'all six validators'):
                    daily.execute(datetime.now(timezone.utc))
                collect.assert_not_called()
            self.assertEqual(len(fake.runtime.call_args_list),6)

    def test_two_same_day_readings_then_daily_use_ledger_epoch(self):
        now=datetime(2026,11,16,17,0,tzinfo=timezone.utc)
        with patch.object(daily,'previous_cutoff',return_value=int(now.timestamp()*1000)-120000):
            first=daily.next_manifest(self.seed,now,self.state,True)
            self.assertEqual(first['observation_epoch'],4)
            self.assertEqual(first['cutoff_unix_ms'],int(now.timestamp()*1000)+120000)
        second_state=copy.deepcopy(self.state);second_state['ledger']['nav_assets'][0]['finalized_epoch']=4
        with patch.object(daily,'previous_cutoff',return_value=first['cutoff_unix_ms']):
            second=daily.next_manifest(self.seed,now.replace(minute=5),second_state,True)
            scheduled=daily.next_manifest(self.seed,now,second_state)
        self.assertEqual(second['observation_epoch'],5)
        self.assertEqual(scheduled['observation_epoch'],5)
        self.assertGreater(second['cutoff_unix_ms'],first['cutoff_unix_ms'])
        self.assertEqual(datetime.fromtimestamp(scheduled['cutoff_unix_ms']/1000,timezone.utc).strftime('%H:%M'),'18:15')

    def test_epoch_reuse_and_skipping_rejected(self):
        for epoch in (3,5):
            with self.assertRaisesRegex(ValueError,'epoch reuse'):
                daily.validate_next_reading(self.state,dict(observation_epoch=epoch,cutoff_unix_ms=100))

    def test_cutoff_replay_and_backdating_rejected(self):
        with patch.object(daily,'previous_cutoff',return_value=100):
            for cutoff in (99,100):
                with self.assertRaisesRegex(ValueError,'cutoff replay'):
                    daily.validate_next_reading(self.state,dict(observation_epoch=4,cutoff_unix_ms=cutoff))
            daily.validate_next_reading(self.state,dict(observation_epoch=4,cutoff_unix_ms=101))

    def test_previous_cutoff_requires_finalized_proof_commitment(self):
        reading={'cutoff_unix_ms':100,'observed_unix_ms':110}
        digest=hashlib.sha3_384(b'postfiat.b23.nitro_nav_reading.v1.source\0'+json.dumps(reading,sort_keys=True,separators=(',',':')).encode()).digest()
        public=bytes(316)+digest+bytes(220)
        state=copy.deepcopy(self.state);state['ledger']['nav_assets'][0]['finalized_reserve_packet_hash']='packet'
        state['ledger']['nav_reserve_packets']=[dict(asset_id=daily.B24,epoch=3,reserve_packet_hash='packet',state='finalized',sp1_public_values=list(public))]
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            daily.save(root/'manifest.json',dict(nav_asset_id=daily.B24,observation_epoch=3,cutoff_unix_ms=100))
            result=dict(body=dict(reading=reading,public_values_hex=public.hex()))
            daily.save(root/'result.json',result)
            self.assertEqual(daily.helpers.finalized_cutoff(state,daily.B24,[root]),100)
            result['body']['reading']['cutoff_unix_ms']=99;daily.save(root/'result.json',result)
            with self.assertRaisesRegex(ValueError,'No reading matching'):
                daily.helpers.finalized_cutoff(state,daily.B24,[root])

class FailureAlertTests(unittest.TestCase):
    """Every failed run leaves a record and calls the hook once; nothing touches the network."""
    def setUp(self):
        self.tmp=Path(tempfile.mkdtemp());self.addCleanup(__import__('shutil').rmtree,self.tmp)
        self.sent=self.tmp/'sent.txt'
        hook=shlex.join([sys.executable,'-I','-c','import sys;open(sys.argv[1],"a").write(sys.argv[2]+"\\n")',str(self.sent)])
        for target,name,value in ((daily,'DAILY',self.tmp/'daily'),(reading_alert,'STATE',self.tmp/'state'),(daily,'LOCK',self.tmp/'chain.lock'),(daily,'STATUS',self.tmp/'now.md'),(daily,'LOCK_POLL',0),(daily,'LANDING_POLL',0),(daily,'LANDING_WAIT',0)):
            patcher=patch.object(target,name,value);patcher.start();self.addCleanup(patcher.stop)
        patcher=patch.dict(os.environ,{reading_alert.HOOK:hook});patcher.start();self.addCleanup(patcher.stop)
        os.environ.pop('INVOCATION_ID',None)
        self.now=datetime(2026,11,17,17,55,tzinfo=timezone.utc)

    def messages(self):return self.sent.read_text().splitlines() if self.sent.exists() else []
    def state_records(self):return [daily.load(p) for p in sorted((self.tmp/'state/failures').glob('*.json'))]

    def test_run_lock_held_records_and_notifies_once(self):
        (self.tmp/'daily').mkdir()
        with (self.tmp/'daily/run.lock').open('a') as held,patch.object(daily,'require_runtime'):
            fcntl.flock(held,fcntl.LOCK_EX)
            with self.assertRaises(BlockingIOError):daily.run(self.now)
        [record]=self.state_records()
        self.assertEqual((record['coin'],record['stage'],record['error_type'],record['mode']),('B24','run-lock','BlockingIOError','daily'))
        self.assertIn('time',record);self.assertEqual(record['notification'],'sent')
        [line]=self.messages();self.assertIn('B24 price reading failed at stage run-lock: BlockingIOError',line)
        self.assertEqual([p.name for p in (self.tmp/'daily').iterdir()],['run.lock'])

    def test_missing_inputs_recorded_without_hook(self):
        os.environ.pop(reading_alert.HOOK)
        with patch.object(daily,'SEED',self.tmp/'absent'),patch.object(daily,'DEPLOYMENT',self.tmp/'absent'):
            with self.assertRaisesRegex(ValueError,'not qualified; missing'):daily.run(self.now,on_demand=True)
        [record]=self.state_records()
        self.assertEqual((record['stage'],record['mode'],record['notification']),('inputs','now','unset'))
        self.assertEqual(self.messages(),[]);self.assertFalse((self.tmp/'daily').exists())

    def fixture_submission(self,finalized_epoch):
        manifest=dict(observation_epoch=7);ops=[dict(operation=dict(reserve_packet_hash='packet'))]
        landed={'ledger':{'nav_assets':[dict(asset_id=daily.B24,finalized_epoch=finalized_epoch,finalized_reserve_packet_hash='packet',nav_per_unit=1_000_000)]}}
        submitted=MagicMock(return_value=(self.tmp/'final.json',[]))
        patches=[patch.object(daily,'require_runtime'),patch.object(daily,'collect',return_value=(manifest,self.tmp/'capture',{})),patch.object(daily,'runtime_check'),
            patch.object(daily,'export',return_value=landed),patch.object(daily,'prepare',return_value=ops),patch.object(daily,'run_operations',submitted)]
        for patcher in patches:patcher.start();self.addCleanup(patcher.stop)
        return submitted

    def test_failure_after_submission_is_recorded_once(self):
        submitted=self.fixture_submission(finalized_epoch=6)
        with self.assertRaisesRegex(TimeoutError,'epoch 7 not finalized on chain'):daily.run(self.now)
        submitted.assert_called_once()
        [run]=[p for p in (self.tmp/'daily').iterdir() if p.is_dir()]
        record=daily.load(run/'failure.json')
        self.assertEqual((record['stage'],record['notification']),('landing-check','sent'))
        self.assertFalse((run/'complete.json').exists());self.assertFalse((self.tmp/'now.md').exists())
        self.assertEqual(self.state_records(),[]);self.assertEqual((self.tmp/'chain.lock').read_text(),'')
        [line]=self.messages();self.assertIn('stage landing-check: TimeoutError: epoch 7 not finalized',line);self.assertIn(str(run/'failure.json'),line)

    def test_landed_reading_completes_without_notification(self):
        self.fixture_submission(finalized_epoch=7);daily.run(self.now)
        [run]=[p for p in (self.tmp/'daily').iterdir() if p.is_dir()]
        self.assertTrue(daily.load(run/'complete.json')['landed_on_chain']);self.assertTrue((run/'landed.json').exists())
        self.assertFalse((run/'failure.json').exists());self.assertEqual(self.messages(),[]);self.assertIn('epoch 7',(self.tmp/'now.md').read_text())

    def test_lock_waits_have_a_deadline(self):
        path=self.tmp/'budget.lock'
        with path.open('a') as held,path.open('a') as waiting:
            fcntl.flock(held,fcntl.LOCK_EX)
            with self.assertRaisesRegex(TimeoutError,'budget lock held for 60 minutes'):daily.wait_lock(waiting,daily.time.monotonic(),'budget lock')

    def test_webhook_called_once_with_one_line(self):
        os.environ[reading_alert.HOOK]='https://hooks.invalid/b24'
        response=MagicMock(status=200);response.__enter__.return_value=response
        with patch.object(reading_alert.urllib.request,'urlopen',return_value=response) as opened:
            record=reading_alert.record_failure('proof',RuntimeError('prover\ndown'),mode='daily')
        opened.assert_called_once();body=opened.call_args.args[0].data.decode()
        self.assertEqual(len(body.splitlines()),1);self.assertIn('B24 price reading failed at stage proof: RuntimeError: prover down',body)
        self.assertEqual(record['notification'],'sent (HTTP 200)');self.assertNotIn('hooks.invalid',json.dumps(self.state_records()))

    def test_service_death_hook_skips_runs_the_job_reported(self):
        with patch.dict(os.environ,{'INVOCATION_ID':'abc'}),patch.object(daily,'require_runtime',side_effect=SystemExit(143)):
            with self.assertRaises(SystemExit):daily.run(self.now)
        state=dict(InvocationID='abc',Result='signal',ExecMainStatus='15')
        with patch.object(reading_alert,'unit_state',return_value=state):
            self.assertIsNone(reading_alert.service_failed('b24-tee-nav-daily.service'))
            state['InvocationID']='killed';record=reading_alert.service_failed('b24-tee-nav-daily.service')
        self.assertEqual((record['stage'],record['invocation_id']),('service','killed'))
        self.assertEqual(len(self.messages()),2);self.assertIn('stopped with exit code 143',self.messages()[0]);self.assertIn('result signal',self.messages()[1])

if __name__=='__main__':unittest.main()
