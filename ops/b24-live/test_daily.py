"""On-demand ordering against the finalized ledger; all fixtures are local."""
import copy
from datetime import datetime,timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0,str(Path(__file__).resolve().parent))
spec=importlib.util.spec_from_file_location('b24_daily',Path(__file__).with_name('daily.py'))
daily=importlib.util.module_from_spec(spec);spec.loader.exec_module(daily)

class OnDemandTests(unittest.TestCase):
    def setUp(self):
        self.state={'tip':{'height':1370},'ledger':{'nav_assets':[dict(asset_id=daily.B24,finalized_epoch=3)]}}
        self.seed={'schedule':{'opening_epoch':1},'nav_asset_id':daily.B24}

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

if __name__=='__main__':unittest.main()
