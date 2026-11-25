"""Low-credit alert against a stubbed Vast CLI; no provider call, no hook call."""
import importlib.util
import json
import re
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
    instances+=[dict(id=100+n,actual_status='exited',intended_status='stopped',dph_total=r,storage_total_cost=0.0) for n,r in enumerate(stopped)]
    def run(*args):return dict(id=1,credit=credit,api_key=KEY) if args==('show','user') else instances
    return run

def failing(*args):raise RuntimeError('vastai user exited 1: 401 for /users/current/?api_key='+KEY)

def shaped(user,instances):
    """Stub with arbitrary replies, for replies of an unexpected shape."""
    return lambda *args:user if args==('show','user') else instances

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

class FixRoundTests(unittest.TestCase):
    setUp,send,run_check=CreditTests.setUp,CreditTests.send,CreditTests.run_check

    def test_an_unset_alert_address_is_a_failure_and_the_crossing_stays_pending(self):
        with patch('sys.stdout'),patch('sys.stderr') as err:
            result=prover_credit.check(24,6,self.state,api(10.0,3.0),lambda line:'unset')
        self.assertFalse(result['ok'])
        self.assertIn('the alert address is not set: nothing was sent',''.join(str(c) for c in err.mock_calls))
        self.assertEqual(json.loads(self.state.read_text())['level'],'ok')
        self.run_check(api(10.0,3.0))
        self.assertEqual(len(self.sent),1);self.assertTrue(self.sent[0].startswith('URGENT'))
        with patch('sys.stdout'),patch('sys.stderr'):
            self.assertFalse(prover_credit.check(24,6,self.state,failing,lambda line:'unset')['ok'])
        self.run_check(failing)
        self.assertEqual(len(self.sent),2);self.assertIn('could not be read',self.sent[1])

    def test_the_key_is_removed_before_the_error_is_cut_and_fragments_too(self):
        stderr='x'*150+' api_key='+KEY+' tail'
        bad=subprocess.CompletedProcess([],1,stdout='',stderr=stderr)
        with patch('subprocess.run',return_value=bad),self.assertRaises(RuntimeError) as caught:prover_credit.vastai('show','user')
        for size in range(8,len(KEY)):
            self.assertNotIn(KEY[:size],str(caught.exception).split('api_key=')[-1]+' ')
            self.assertNotIn(KEY[-size:],str(caught.exception))
        self.assertNotIn(KEY[-20:],prover_credit.redact('cut here '+KEY[-20:]+' rest'))
        self.assertNotIn(KEY[:20],prover_credit.redact('ends with '+KEY[:20]))

    def test_a_reply_of_an_unexpected_shape_is_not_usable_and_alerts(self):
        cases={
            'renamed rate':shaped(dict(credit=10.0),[dict(actual_status='running',dph=3.0)]),
            'missing rate':shaped(dict(credit=10.0),[dict(actual_status='running')]),
            'zero rate while running':shaped(dict(credit=10.0),[dict(actual_status='running',dph_total=0)]),
            'instances not a list':shaped(dict(credit=10.0),dict(instances=[])),
            'balance instead of credit':shaped(dict(balance=10.0),[]),
        }
        for name,run in cases.items():
            with self.subTest(name):
                self.state.unlink(missing_ok=True);self.sent.clear()
                result=self.run_check(run)
                self.assertFalse(result['ok'],name);self.assertNotIn('hours',result)
                self.assertEqual(len(self.sent),1,name);self.assertIn('not usable',self.sent[0])

    def test_stopped_machines_billed_for_storage_count_and_can_alert(self):
        def stopped(credit,storage):
            return shaped(dict(credit=credit),[dict(actual_status='exited',intended_status='stopped',dph_total=1.2,
                                                     storage_total_cost=storage)])
        result=self.run_check(stopped(1.0,0.5))
        self.assertEqual((round(result['hours'],1),result['level']),(2.0,'urgent'))
        self.assertEqual(len(self.sent),1);self.assertIn('0 running',self.sent[0])
        self.state.unlink();self.sent.clear()
        self.assertFalse(self.run_check(shaped(dict(credit=1.0),[dict(actual_status='exited',dph_total=1.2)]))['ok'])
        self.assertIn('not usable',self.sent[0])

    def test_a_reading_that_stays_unusable_is_reported_again_after_six_hours(self):
        self.run_check(failing)
        state=json.loads(self.state.read_text());state['reported_at']-=6*3600+1
        self.state.write_text(json.dumps(state))
        self.run_check(failing)
        self.assertEqual(len(self.sent),2)

    def test_a_failed_run_is_reported_by_its_own_failure_unit(self):
        unit=Path(__file__).with_name('prover-credit-alert.service').read_text()
        self.assertIn('OnFailure=prover-credit-alert-failed.service',unit)
        failed=Path(__file__).with_name('prover-credit-alert-failed.service').read_text()
        self.assertIn('prover_credit.py --service-failed',failed)
        runbook=(Path(__file__).parents[2]/'docs/runbooks/prover-credit-alert.md').read_text()
        self.assertIn('loginctl enable-linger',runbook)

    def test_the_failure_unit_reports_a_crash_but_not_a_failure_the_run_reported(self):
        with patch.dict('os.environ',INVOCATION_ID='run-1'):self.run_check(failing)
        with patch.dict('os.environ',MONITOR_INVOCATION_ID='run-1'),patch('sys.stdout'),patch('sys.stderr'):
            self.assertTrue(prover_credit.service_failed('prover-credit-alert.service',self.state,self.send))
        self.assertEqual(len(self.sent),1)
        with patch.dict('os.environ',MONITOR_INVOCATION_ID='run-2',MONITOR_SERVICE_RESULT='timeout'),patch('sys.stdout'),patch('sys.stderr'):
            self.assertTrue(prover_credit.service_failed('prover-credit-alert.service',self.state,self.send))
        self.assertEqual(len(self.sent),2)
        self.assertIn('prover-credit-alert.service failed',self.sent[1]);self.assertIn('timeout',self.sent[1])

class ThirdRoundTests(unittest.TestCase):
    setUp,send,run_check=CreditTests.setUp,CreditTests.send,CreditTests.run_check

    def test_the_timeouts_fit_well_inside_the_unit_start_limit(self):
        unit=(Path(__file__).with_name('prover-credit-alert.service')).read_text()
        limit=re.search(r'^TimeoutStartSec=(\d+)min$',unit,re.M);self.assertIsNotNone(limit,'TimeoutStartSec in minutes')
        hook=max(int(v) for v in re.findall(r'timeout=(\d+)',Path(__file__).with_name('reading_alert.py').read_text().split('def notify',1)[1].split('def ',1)[0]))
        worst=2*prover_credit.VASTAI_TIMEOUT_SECONDS+hook
        self.assertLessEqual(worst+120,int(limit.group(1))*60,(worst,limit.group(1)))

    def test_a_delivered_report_is_written_before_anything_else(self):
        order=[]
        real_write=prover_credit.write
        def send(line):order.append('send');return 'sent'
        def write(path,value):order.append('write');real_write(path,value)
        with patch.object(prover_credit,'write',write),patch('builtins.print',lambda *a,**k:order.append('print')):
            prover_credit.check(24,6,self.state,api(10.0,1.0),send)
        self.assertEqual(order[:2],['send','write'],order)

    def test_an_undelivered_report_is_not_marked_handled_and_the_failure_unit_says_so(self):
        with patch.dict('os.environ',INVOCATION_ID='run-7'):
            result=self.run_check(api(10.0,1.0),send=lambda line:'hook exited 1')
        self.assertFalse(result['ok'])
        self.assertNotEqual(json.loads(self.state.read_text()).get('handled_invocation'),'run-7')
        with patch.dict('os.environ',MONITOR_INVOCATION_ID='run-7',MONITOR_SERVICE_RESULT='exit-code'),patch('sys.stdout'),patch('sys.stderr'):
            self.assertTrue(prover_credit.service_failed('prover-credit-alert.service',self.state,self.send))
        self.assertEqual(len(self.sent),1)
        self.assertNotIn('was not checked',self.sent[0]);self.assertIn('not delivered',self.sent[0])

    def test_a_delivered_report_is_marked_handled(self):
        with patch.dict('os.environ',INVOCATION_ID='run-8'):self.run_check(api(10.0,1.0))
        self.assertEqual(json.loads(self.state.read_text()).get('handled_invocation'),'run-8')

    def test_the_module_has_no_nested_same_quote_f_string(self):
        """Python before 3.12 cannot parse an f-string that reuses its own quote inside braces."""
        source=Path(__file__).with_name('prover_credit.py').read_text()
        self.assertNotIn("{f'",source);self.assertNotIn('{f"',source)

    def test_any_piece_of_the_key_of_16_characters_is_removed(self):
        import secrets as random
        key=random.token_hex(32)
        for piece in (key[20:44],key[5:21],key[30:60]):
            self.assertNotIn(piece,prover_credit.redact('e '+piece+' x',[key]))
        wrapped='e '+key[:20]+' '+key[20:44]+' '+key[44:]
        out=prover_credit.redact(wrapped,[key])
        for piece in (key[:20],key[20:44],key[44:]):self.assertNotIn(piece,out)

if __name__=='__main__':unittest.main()
