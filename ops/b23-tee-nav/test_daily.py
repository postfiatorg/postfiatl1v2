import importlib.util
from datetime import date
from decimal import Decimal
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('daily',Path(__file__).with_name('daily.py'))
daily=importlib.util.module_from_spec(spec);spec.loader.exec_module(daily)

class DailyTests(unittest.TestCase):
    def test_daily_time_uses_next_ledger_epoch(self):
        seed={'schedule':{'opening_epoch':3,'opening_unix_ms':1794601800000,'first_daily_epoch':4,'first_daily_unix_ms':1794680100000}}
        m=daily.daily_manifest(seed,date(2026,11,14),1301,3,1794601800000)
        self.assertEqual((m['observation_epoch'],m['cutoff_unix_ms']),(4,1794680100000))
        self.assertEqual(daily.daily_manifest(seed,date(2026,11,15),1301,7,1794680100000)['observation_epoch'],8)
        self.assertEqual(m['schedule'],seed['schedule'])
        with self.assertRaises(ValueError):daily.daily_manifest(seed,date(2026,11,13),1301,3,1794601800000)
    def test_two_readings_same_day_and_cutoff_replay(self):
        seed={'schedule':{'opening_epoch':1}}
        first=daily.reading_manifest(seed,1794848400000,1370,3,1794766500000)
        second=daily.reading_manifest(seed,1794848520000,1373,4,first['cutoff_unix_ms'])
        self.assertEqual((first['observation_epoch'],second['observation_epoch']),(4,5))
        self.assertNotEqual(first['challenge_sha256'],second['challenge_sha256'])
        for cutoff in (first['cutoff_unix_ms'],first['cutoff_unix_ms']-1):
            with self.assertRaisesRegex(ValueError,'strictly follow'):
                daily.reading_manifest(seed,cutoff,1373,4,first['cutoff_unix_ms'])
    def test_nonrefundable_monthly_and_per_run_caps(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'budget.json'
            for n in range(31):r=daily.reserve_budget(p,'2026-12',f'run-{n}','8.033333333333333')
            self.assertEqual(r['month_reserved_usd'],'249.24')
            with self.assertRaisesRegex(ValueError,'monthly'):daily.reserve_budget(p,'2026-12','extra','8.04')
            with self.assertRaisesRegex(ValueError,'already'):daily.reserve_budget(p,'2027-01','run-0','8.04')
            with self.assertRaisesRegex(ValueError,'per-run'):daily.reserve_budget(p,'2027-01','too-large','9.01')
            self.assertEqual(daily.reserve_budget(p,'2027-01','next-month','8.04')['month_reserved_usd'],'8.04')
    def test_caps_cannot_be_raised_by_environment(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaisesRegex(ValueError,'committed'):daily.reserve_budget(Path(d)/'x','2026-11','x','8',Decimal('10'),Decimal('250'))

if __name__=='__main__':unittest.main()
