import subprocess
import sys
import unittest
from pathlib import Path
from retirement import prepare
from common import EVIDENCE,FOUNDER,ALICE,PFUSDC

class RetirementPlanTests(unittest.TestCase):
    def test_all_existing_claims_and_reserve_are_accounted_for(self):
        p=prepare(EVIDENCE/'before-capture-snapshot.json')
        self.assertEqual(sum(x['old_B23_atoms'] for x in p['holder_terms']),p['old_supply_before_atoms'])
        self.assertEqual(sum(x['gross_PFUSDC_atoms'] for x in p['holder_terms'])+p['remaining_reserve_dust_atoms'],p['original_reserve_atoms'])
        for holder in p['holder_terms']:
            self.assertEqual(holder['net_PFUSDC_atoms']+holder['management_fee_atoms'],holder['gross_PFUSDC_atoms'])
        founder=next(x for x in p['holder_terms'] if x['owner']==FOUNDER)
        self.assertEqual((founder['cash_in_B23_atoms'],founder['net_PFUSDC_atoms']),(1_000_000,1_000_482))
        self.assertTrue(any(x['source']==FOUNDER and x['operation']['operation']=='trust_set' and x['operation']['asset_id']==PFUSDC for x in p['operations']))
        self.assertGreater(p['old_historical_nav_and_route_counter_after_atoms'],0)
        self.assertFalse(p['production_authorized'])
    def test_script_refuses_without_explicit_dry_run_flag(self):
        r=subprocess.run([sys.executable,str(Path(__file__).with_name('retirement.py'))],capture_output=True)
        self.assertNotEqual(r.returncode,0)
        self.assertIn(b'--dry-run',r.stderr)

if __name__=='__main__':unittest.main()
