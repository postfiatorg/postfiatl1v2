"""Low-credit alert against a stubbed Vast CLI; no provider call, no hook call."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec=importlib.util.spec_from_file_location('prover_credit',Path(__file__).with_name('prover_credit.py'))
prover_credit=importlib.util.module_from_spec(spec);spec.loader.exec_module(prover_credit)

KEY='0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'

def api(credit,*rates,stopped=()):
    """Stub of vastai(): `show user` and `show instances` replies."""
    instances=[dict(id=n,actual_status='running',dph_total=r) for n,r in enumerate(rates)]
    instances+=[dict(id=100+n,actual_status='exited',intended_status='stopped',dph_total=r) for n,r in enumerate(stopped)]
    def run(*args):return dict(id=1,credit=credit,api_key=KEY) if args==('show','user') else instances
    return run

def failing(*args):raise RuntimeError('vastai user exited 1: 401 for /users/current/?api_key='+KEY)

class CreditTests(unittest.TestCase):
    def setUp(self):
        self.directory=tempfile.TemporaryDirectory();self.addCleanup(self.directory.cleanup)
        self.state=Path(self.directory.name)/'state.json';self.sent=[]
        patcher=patch.object(prover_credit,'secrets',return_value=[KEY]);patcher.start();self.addCleanup(patcher.stop)

    def send(self,line):self.sent.append(line);return 'sent'
    def run_check(self,run,send=None):
        with patch('sys.stdout'),patch('sys.stderr'):
            return prover_credit.check(24,6,self.state,run,send or self.send)

    def test_enough_credit_sends_nothing(self):
        result=self.run_check(api(500.0,3.0,2.0,1.0,stopped=(9.0,)))
        self.assertEqual((result['level'],round(result['hours'],1),self.sent),('ok',83.3,[]))

    def test_under_24_hours_reports_once(self):
        for _ in range(3):self.run_check(api(100.0,3.0,2.0,1.0))
        self.assertEqual(len(self.sent),1)
        self.assertTrue(self.sent[0].startswith('Proving hosts stop in about 16.7 h: Vast credit 100.00 USD at 6.00 USD/h for 3 running'))
        self.assertIn('under the 24 h limit',self.sent[0])

    def test_under_6_hours_sends_a_second_louder_report(self):
        self.run_check(api(100.0,6.0))
        self.run_check(api(30.0,6.0));self.run_check(api(24.0,6.0))
        self.assertEqual(len(self.sent),2);self.assertTrue(self.sent[1].startswith('URGENT: proving hosts stop in about 5.0 h'))

    def test_straight_to_under_6_hours_is_one_urgent_report(self):
        self.run_check(api(0.0,6.0));self.run_check(api(-1.0))
        self.assertEqual(len(self.sent),1);self.assertTrue(self.sent[0].startswith('URGENT'))

    def test_a_top_up_rearms_both_reports(self):
        for credit in (100.0,30.0,1000.0,100.0,30.0):self.run_check(api(credit,6.0))
        self.assertEqual([line.split(' ')[0] for line in self.sent],['Proving','URGENT:','Proving','URGENT:'])

    def test_api_failure_is_reported_once_then_cleared(self):
        for _ in range(3):self.assertFalse(self.run_check(failing)['ok'])
        self.assertEqual(len(self.sent),1);self.assertIn('credit could not be read',self.sent[0])
        self.run_check(api(500.0,1.0));self.run_check(failing)
        self.assertEqual(len(self.sent),2)

    def test_failure_does_not_reset_the_threshold_report(self):
        self.run_check(api(100.0,6.0));self.run_check(failing);self.run_check(api(90.0,6.0))
        self.assertEqual(len(self.sent),2)

    def test_a_failed_hook_is_retried_next_run(self):
        self.run_check(api(100.0,6.0),send=lambda line:'hook exited 1');self.run_check(api(100.0,6.0))
        self.run_check(failing,send=lambda line:'hook failed: URLError');self.run_check(failing)
        self.assertEqual(len(self.sent),2)

    def test_the_key_never_reaches_the_report_state_or_log(self):
        with patch('sys.stdout') as out,patch('sys.stderr') as err:
            prover_credit.check(24,6,self.state,failing,self.send)
            prover_credit.check(24,6,self.state,api(1.0,6.0),self.send)
        logged=''.join(str(c) for c in out.mock_calls+err.mock_calls)
        for text in self.sent+[self.state.read_text(),logged]:self.assertNotIn(KEY,text)
        self.assertIn('api_key=[redacted]',self.sent[0])

    def test_cli_errors_are_redacted_and_carry_no_stdout(self):
        bad=subprocess.CompletedProcess([],1,stdout=json.dumps(dict(api_key=KEY)),stderr='error: Bearer '+KEY+' rejected')
        with patch('subprocess.run',return_value=bad),self.assertRaises(RuntimeError) as caught:prover_credit.vastai('show','user')
        self.assertNotIn(KEY,str(caught.exception));self.assertIn('[redacted]',str(caught.exception))
        garbled=subprocess.CompletedProcess([],0,stdout='key '+KEY,stderr='')
        with patch('subprocess.run',return_value=garbled),self.assertRaises(RuntimeError) as caught:prover_credit.vastai('show','user')
        self.assertEqual(str(caught.exception),'vastai user: output is not JSON')

    def test_the_cli_gets_no_key_argument(self):
        ok=subprocess.CompletedProcess([],0,stdout='{"credit": 5}',stderr='')
        with patch('subprocess.run',return_value=ok) as run:prover_credit.vastai('show','user')
        self.assertEqual(run.call_args.args[0],[str(prover_credit.VASTAI),'show','user','--raw'])

    def test_missing_credit_field_counts_as_unreadable(self):
        self.run_check(lambda *args:{} if args==('show','user') else [])
        self.assertEqual(len(self.sent),1);self.assertIn('no credit field',self.sent[0])

if __name__=='__main__':unittest.main()
