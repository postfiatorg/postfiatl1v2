"""On-demand ordering against the finalized ledger; all fixtures are local."""
import copy
import fcntl
import os
import shlex
import subprocess
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
import release_pin
NODE='6dd2ee56d350854dc32f6542895fd51508d12901b068ac840be428d32f0f759b'

class OnDemandTests(unittest.TestCase):
    def setUp(self):
        self.state={'tip':{'height':1370},'ledger':{'nav_assets':[dict(asset_id=daily.B24,finalized_epoch=3)]}}
        self.seed={'schedule':{'opening_epoch':1},'nav_asset_id':daily.B24}

    def test_release_gate_rejects_a_mixed_fleet_before_capture(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            probe = MagicMock()
            probe.side_effect = lambda peer, sha: (peer[0], {'matches': peer != daily.PEERS[-1]})
            chosen=dict(name='sol-exit-r2',node_sha256=NODE,tool=str(root/'tool'),tool_sha256='ab'*32)
            with patch.object(daily, 'runtime', probe),patch.object(daily,'DAILY',root),patch.object(daily,'require_runtime'),patch.object(daily,'collect') as collect,\
                    patch.object(daily,'fleet_statuses'),patch.object(release_pin,'fleet'),patch.object(release_pin,'release',return_value=chosen):
                with self.assertRaisesRegex(release_pin.Refusal,'^python3 ops/b24-live/daily.py --check after restarting both units on validator-5 .*not the binary 6dd2ee56d350854d'):
                    daily.execute(datetime.now(timezone.utc))
                collect.assert_not_called()
            self.assertCountEqual([call.args for call in probe.call_args_list],
                                  [(peer, NODE) for peer in daily.PEERS])
            self.assertEqual(len(daily.PEERS), 6)

    def test_both_running_processes_must_match_the_selected_release(self):
        pin = NODE
        cases = [
            (0, [pin, pin], True),
            (0, [pin, 'other'], False),
            (0, ['other', pin], False),
            (0, [pin], False),
            (1, [pin, pin], False),
        ]
        for code, hashes, expected in cases:
            with self.subTest(code=code, hashes=hashes), patch('runtime_hashes.subprocess.run') as probe:
                probe.return_value.returncode = code
                probe.return_value.stdout = ''.join(value + '  /proc/123/exe\n' for value in hashes)
                name, result = daily.runtime(daily.PEERS[0], pin)
                self.assertEqual(name, 'validator-0')
                self.assertEqual(result, dict(returncode=code, hashes=hashes, matches=expected))

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

class ReleasePinTests(unittest.TestCase):
    """The job derives the release from the six status replies; local fixtures only."""
    REV='04600868'
    def setUp(self):
        self.tmp=Path(tempfile.mkdtemp());self.addCleanup(__import__('shutil').rmtree,self.tmp)
        self.coord=self.tmp/'coord';self.known=self.tmp/'known-releases.txt'
        self.tool=self.tool_at('sol-exit-r2',self.REV)
        self.known.write_text(f'# release commit node tool\nsol-exit-r2 046008683 {NODE} {self.sha(self.tool)}\n')
        self.statuses={f'validator-{i}':dict(build_git_revision=self.REV,deployment_manifest_verified=True,deployment_runtime_artifacts=dict(binary_sha256=NODE)) for i in range(6)}
    def tool_at(self,name,revision):
        """A stand-in tool answering build-info; revision None is a tool built before it recorded one."""
        tool=self.coord/'artifacts'/name/release_pin.TOOL;tool.parent.mkdir(parents=True,exist_ok=True)
        info=json.dumps(dict(build_git_revision=revision,build_profile='release'))
        tool.write_text('#!/bin/sh\n'+(f"test \"$1\" = build-info && echo '{info}' && exit 0\n" if revision else '')+'exit 2\n');tool.chmod(0o755);return tool
    def sha(self,path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
    def pick(self):return release_pin.release(release_pin.fleet(self.statuses),self.coord,self.known)

    def test_fleet_agrees_and_matches_the_tool(self):
        chosen=self.pick()
        self.assertEqual((chosen['name'],chosen['revision'],chosen['node_sha256'],chosen['tool'],chosen['tool_revision']),('sol-exit-r2',self.REV,NODE,str(self.tool),self.REV))
        probe=MagicMock(side_effect=lambda peer,sha:(peer[0],{'matches':sha==NODE}))
        canaries=[patch.object(m,'CANARY',m.CANARY) for m in (daily.common,daily.sign,daily.snapshot,daily.live)]+[patch.dict(daily.__dict__,{'CANARY':daily.CANARY})]
        for patcher in canaries:patcher.start();self.addCleanup(patcher.stop)
        with patch.object(daily,'fleet_statuses',return_value=self.statuses),patch.object(daily,'COORD',self.coord),patch.object(release_pin,'KNOWN',self.known),\
                patch.object(daily,'runtime',probe),patch.object(daily,'remote_copy',return_value='/tmp/copy') as copied:
            result=daily.release_check()
        copied.assert_called_once_with(str(self.tool),self.sha(self.tool))
        self.assertEqual(sorted(result['processes']),release_pin.VALIDATORS)
        for module in (daily.common,daily.sign,daily.snapshot,daily.live):self.assertEqual(module.CANARY,self.tool)
        self.assertEqual(daily.CANARY,self.tool)

    def test_a_tool_that_does_not_record_its_source_is_refused_with_the_rebuild_command(self):
        self.tool=self.tool_at('sol-exit-r2',None);self.known.write_text(f'sol-exit-r2 046008683 {NODE} {self.sha(self.tool)}\n')
        with self.assertRaises(release_pin.Refusal) as refused:self.pick()
        self.assertIn(f'ops/b24-live/rehearsal_tool.sh sol-exit-r2 {self.REV} {NODE}',str(refused.exception))
        self.assertIn('does not record its source',str(refused.exception))

    def test_a_modified_tree_is_refused_and_placed_files_are_named(self):
        for recorded in ('046008683+modified','04600868+modified'):
            with self.subTest(recorded):
                self.tool=self.tool_at('sol-exit-r2',recorded);self.known.write_text(f'sol-exit-r2 046008683 {NODE} {self.sha(self.tool)}\n')
                with self.assertRaisesRegex(release_pin.Refusal,'modified'):self.pick()
        placed='046008683+placed:Cargo.lock=0123456789abcdef,build.rs=fedcba9876543210,src/main.rs=00112233445566ff'
        self.tool=self.tool_at('sol-exit-r2',placed);self.known.write_text(f'sol-exit-r2 046008683 {NODE} {self.sha(self.tool)}\n')
        self.assertEqual(self.pick()['tool_revision'],placed)
        for bad in ('046008683+placed:src/lib.rs=0123456789abcdef','046008683+placed:build.rs=xyz','046008683+other'):
            with self.subTest(bad):
                self.tool=self.tool_at('sol-exit-r2',bad);self.known.write_text(f'sol-exit-r2 046008683 {NODE} {self.sha(self.tool)}\n')
                with self.assertRaises(release_pin.Refusal):self.pick()

    def test_one_validator_differs(self):
        self.statuses['validator-3']['build_git_revision']='71dbc9b3';self.statuses['validator-3']['deployment_runtime_artifacts']['binary_sha256']='d46376ff'*8
        with self.assertRaisesRegex(release_pin.Refusal,r'^python3 ops/b24-live/daily.py --check after finishing the rollout on all six: they run 04600868/6dd2ee56… on 0,1,2,4,5; 71dbc9b3/d46376ff… on 3$'):self.pick()

    def test_manifest_not_verified(self):
        self.statuses['validator-2']['deployment_manifest_verified']=False
        with self.assertRaisesRegex(release_pin.Refusal,r'^python3 ops/b24-live/daily.py --check after reinstalling the signed deployment manifest on validator-2 and restarting it: it reports deployment_manifest_verified=false$'):self.pick()

    def test_status_reply_without_the_fields(self):
        self.statuses['validator-4']=dict(node_id='validator-4',block_height=1900)
        with self.assertRaisesRegex(release_pin.Refusal,'^python3 ops/b24-live/daily.py --check after finishing the release install on validator-4: its status has no build_git_revision, deployment_manifest_verified, deployment_runtime_artifacts.binary_sha256$'):self.pick()
        self.statuses['validator-4']=RuntimeError('validator-4: connection refused')
        with self.assertRaisesRegex(release_pin.Refusal,'^python3 ops/b24-live/daily.py --check after restoring the RPC tunnel to validator-4: it gave no status reply'):self.pick()

    def test_tool_from_another_revision(self):
        other=self.tool_at('sol-exit-r2','71dbc9b3')
        with self.assertRaisesRegex(release_pin.Refusal,f'^ops/b24-live/rehearsal_tool.sh sol-exit-r2 {self.REV} {NODE}: the installed tool has sha256 {self.sha(other)[:16]}…, {self.known}:2 lists'):self.pick()
        self.known.write_text(f'sol-exit-r2 046008683 {NODE} {self.sha(other)}\n')
        with self.assertRaisesRegex(release_pin.Refusal,f'^ops/b24-live/rehearsal_tool.sh sol-exit-r2 {self.REV} {NODE}: the installed tool was built from 71dbc9b3, the six run {self.REV}$'):self.pick()

    def test_tool_missing(self):
        self.tool.unlink()
        with self.assertRaisesRegex(release_pin.Refusal,f'^ops/b24-live/rehearsal_tool.sh sol-exit-r2 {self.REV} {NODE}: no rehearsal tool for sol-exit-r2 at artifacts/sol-exit-r2/postfiat-b23-nav-canary$'):self.pick()

    def test_unlisted_release_names_the_file_and_the_line(self):
        self.known.write_text('# none yet\n')
        with self.assertRaisesRegex(release_pin.Refusal,f'^ops/b24-live/rehearsal_tool.sh <release-name> {self.REV} {NODE}: the six run {self.REV}, which {self.known} does not list; it builds the tool and prints the line to add$'):
            release_pin.release(release_pin.fleet(self.statuses),self.tmp/'empty-coord',self.known)
        line=f'sol-exit-r2 {self.REV} {NODE} {self.sha(self.tool)}'
        with self.assertRaises(release_pin.Refusal) as refused:self.pick()
        self.assertTrue(str(refused.exception).startswith(f"echo {shlex.quote(line)} >> {self.known}: "),refused.exception)

    def test_listed_node_hash_must_match_the_fleet(self):
        self.statuses={k:dict(v,deployment_runtime_artifacts=dict(binary_sha256='ee'*32)) for k,v in self.statuses.items()}
        with self.assertRaisesRegex(release_pin.Refusal,f'^ops/b24-live/rehearsal_tool.sh sol-exit-r2 {self.REV} {"ee"*32}: {self.known}:2 gives sol-exit-r2 the node {NODE[:16]}…, the six run {"ee"*8}…; replace that line'):self.pick()

    def test_check_runs_the_same_comparison_before_anything_else(self):
        self.statuses['validator-1']['deployment_manifest_verified']=None
        with patch.object(sys,'argv',['daily.py','--check']),patch.object(daily,'require_runtime'),patch.object(daily,'fleet_statuses',return_value=self.statuses),patch.object(daily,'load') as loaded:
            with self.assertRaisesRegex(release_pin.Refusal,'validator-1 and restarting it: it reports deployment_manifest_verified=null'):daily.main()
        loaded.assert_not_called()

    def test_validator0_copy_must_match_the_tool(self):
        with patch.object(daily.subprocess,'run') as ssh:
            ssh.return_value.returncode=0;ssh.return_value.stdout='ab'*32+'  /tmp/b23-tee-nav-canary-abababababababab\n'
            self.assertEqual(daily.remote_copy('/x/tool','ab'*32),'/tmp/b23-tee-nav-canary-abababababababab')
            ssh.return_value.returncode=1;ssh.return_value.stdout=''
            with self.assertRaisesRegex(release_pin.Refusal,'^scp -p /x/tool .*:/tmp/b23-tee-nav-canary-abababababababab: validator-0 has no copy of the rehearsal tool$'):daily.remote_copy('/x/tool','ab'*32)

class RefusalFormTests(unittest.TestCase):
    """Every refusal starts with the command that fixes it and fits the alert hook (280 characters)."""
    REV,setUp,tool_at,sha,pick=ReleasePinTests.REV,ReleasePinTests.setUp,ReleasePinTests.tool_at,ReleasePinTests.sha,ReleasePinTests.pick
    FIXES=('python3 ops/b24-live/daily.py --check','ops/b24-live/rehearsal_tool.sh','echo ','scp ')
    def refusal(self,action):
        with self.assertRaises(release_pin.Refusal) as refused:action()
        text=str(refused.exception)
        self.assertLessEqual(len(text),280,text);self.assertTrue(text.startswith(self.FIXES),text)
        return text
    def test_every_refusal_is_short_and_starts_with_its_fix(self):
        long_coord=self.tmp/('c'*60);(long_coord/'artifacts/sol-exit-r2').mkdir(parents=True)
        cases=[]
        def status(**change):
            statuses=json.loads(json.dumps(self.statuses));statuses['validator-3'].update(change);return statuses
        cases.append(lambda:release_pin.fleet(dict(self.statuses,**{'validator-3':RuntimeError('x'*400)})))
        cases.append(lambda:release_pin.fleet(dict(self.statuses,**{'validator-3':dict(node_id='validator-3')})))
        cases.append(lambda:release_pin.fleet(status(deployment_manifest_verified=False)))
        cases.append(lambda:release_pin.fleet(status(build_git_revision=None)))
        mixed={f'validator-{i}':dict(build_git_revision=f'{i}'*8,deployment_manifest_verified=True,deployment_runtime_artifacts=dict(binary_sha256=f'{i}'*64)) for i in range(6)}
        cases.append(lambda:release_pin.fleet(mixed))
        self.known.write_text('a b c\n');cases.append(self.pick)
        for case in cases:self.refusal(case)
        self.known.write_text('# none\n')
        self.refusal(lambda:release_pin.release(release_pin.fleet(self.statuses),long_coord,self.known))
        self.refusal(lambda:release_pin.release(release_pin.fleet(self.statuses),self.coord,self.known))
        self.known.write_text(f'sol-exit-r2 046008683 {"ee"*32} {self.sha(self.tool)}\n');self.refusal(self.pick)
        self.known.write_text(f'sol-exit-r2 046008683 {NODE} {"ab"*32}\n');self.refusal(self.pick)
        for recorded in (None,'046008683+modified','71dbc9b3'):
            self.tool=self.tool_at('sol-exit-r2',recorded);self.known.write_text(f'sol-exit-r2 046008683 {NODE} {self.sha(self.tool)}\n');self.refusal(self.pick)
        self.tool.unlink();self.refusal(self.pick)
        with patch.object(daily.subprocess,'run') as ssh:
            ssh.return_value.returncode=1;ssh.return_value.stdout=''
            self.refusal(lambda:daily.remote_copy('/home/postfiat/repos/b23-coord/artifacts/sol-exit-r2/'+release_pin.TOOL,'ab'*32))

    def test_a_duplicated_release_name_or_commit_is_refused_with_the_file_and_line(self):
        line=f'sol-exit-r2 046008683 {NODE} {self.sha(self.tool)}'
        self.known.write_text(f'{line}\n# again\n{line}\n')
        self.assertIn(f'{self.known}:3',self.refusal(self.pick))
        self.known.write_text(f'{line}\nsol-exit-r3 046008683 {NODE} {self.sha(self.tool)}\n')
        self.assertIn(f'{self.known}:2',self.refusal(self.pick))

    def test_a_malformed_line_is_refused_with_the_file_and_line(self):
        self.known.write_text(f'# c\nsol-exit-r2 046008683 {NODE}\n')
        self.assertIn(f'{self.known}:2',self.refusal(self.pick))
        self.known.write_text(f'sol-exit-r2 0460 {NODE} {self.sha(self.tool)}\n')
        self.assertIn(f'{self.known}:1',self.refusal(self.pick))

    def test_a_null_or_missing_revision_is_a_refusal(self):
        for value in (None,'',7):
            with self.subTest(value):
                self.statuses['validator-5']['build_git_revision']=value
                self.assertIn('validator-5',self.refusal(self.pick))

class ToolSourceTests(unittest.TestCase):
    """The source text rehearsal_tool.sh records, on a scratch git repository."""
    def setUp(self):
        self.tree=Path(tempfile.mkdtemp());self.addCleanup(__import__('shutil').rmtree,self.tree)
        run=lambda *a:subprocess.run(['git','-C',str(self.tree),*a],check=True,capture_output=True)  # noqa: E731
        (self.tree/'tools/b23-nav-canary/src').mkdir(parents=True);(self.tree/'tools/b23-nav-canary/src/main.rs').write_text('old\n')
        (self.tree/'tools/b23-nav-canary/Cargo.lock').write_text('old\n');(self.tree/'crates').mkdir();(self.tree/'crates/x.rs').write_text('x\n')
        run('init','-q');run('add','.');run('-c','user.email=t@t','-c','user.name=t','commit','-qm','c')
        self.commit=subprocess.run(['git','-C',str(self.tree),'rev-parse','--short=9','HEAD'],capture_output=True,text=True).stdout.strip()
        import tool_source;self.source=tool_source
    def test_a_clean_tree_is_its_commit(self):
        self.assertEqual(self.source.text(self.tree),self.commit)
    def test_placed_files_are_named_with_their_hashes_in_a_fixed_form(self):
        for name,body in (('build.rs','fn main(){}\n'),('src/main.rs','new\n'),('Cargo.lock','new\n')):(self.tree/'tools/b23-nav-canary'/name).write_text(body)
        placed=['tools/b23-nav-canary/build.rs','tools/b23-nav-canary/src/main.rs','tools/b23-nav-canary/Cargo.lock']
        h=lambda body:hashlib.sha256(body.encode()).hexdigest()[:16]  # noqa: E731
        self.assertEqual(self.source.text(self.tree,placed),f'{self.commit}+placed:Cargo.lock={h("new\n")},build.rs={h("fn main(){}\n")},src/main.rs={h("new\n")}')
    def test_any_other_difference_is_modified(self):
        (self.tree/'crates/x.rs').write_text('changed\n')
        self.assertEqual(self.source.text(self.tree),self.commit+'+modified')
        (self.tree/'crates/x.rs').write_text('x\n');(self.tree/'stray.txt').write_text('s\n')
        self.assertEqual(self.source.text(self.tree),self.commit+'+modified')
        (self.tree/'stray.txt').unlink();(self.tree/'tools/b23-nav-canary/Cargo.lock').write_text('new\n')
        self.assertEqual(self.source.text(self.tree,['tools/b23-nav-canary/src/main.rs']),self.commit+'+modified')
        self.assertEqual(self.source.text(self.tree,['crates/x.rs']),self.commit+'+modified')

class ReproducibleBuildTests(unittest.TestCase):
    """rehearsal_tool.sh builds where only the commit decides the path and nothing random reaches the binary."""
    def test_the_script_builds_at_a_fixed_path_without_runpath_or_build_paths(self):
        script=(Path(__file__).with_name('rehearsal_tool.sh')).read_text()
        self.assertNotIn('mktemp',script)
        self.assertIn('src=$root/$full',script);self.assertIn('target=$root/$full.target',script)
        for flag in ('-C linker=/usr/bin/cc','--remap-path-prefix=$src=','--remap-path-prefix=$cargo_home=','--remap-path-prefix=$rustup_home='):self.assertIn(flag,script)
        self.assertIn('POSTFIAT_BUILD_SOURCE=$source_text',script)

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

class ResumeTests(unittest.TestCase):
    def setUp(self):
        self.dir=tempfile.TemporaryDirectory();self.addCleanup(self.dir.cleanup);self.tmp=Path(self.dir.name)
        self.d=self.tmp/'run';capture=self.d/'capture';capture.mkdir(parents=True)
        self.cutoff=datetime(2026,11,25,18,0,tzinfo=timezone.utc)
        daily.save(capture/'manifest.json',dict(observation_epoch=10,cutoff_unix_ms=int(self.cutoff.timestamp()*1000)))
        daily.save(capture/'final-inventory.json',dict(noOwnedBillableComputeRemaining=True))
        daily.save(self.d/'budget.json',{});daily.save(self.d/'archive-verification.json',{});(self.d/'negative-reading').mkdir()
        (self.d/'proof').mkdir();(self.d/'proof/public-values.bin').write_bytes(b'v');(self.d/'proof/proof-calldata.bin').write_bytes(b'p')
        for patcher in (patch.object(daily,'DAILY',self.tmp/'daily'),patch.object(daily.subprocess,'check_output',return_value='inactive\n')):
            patcher.start();self.addCleanup(patcher.stop)
    def six(self,rows):
        return [dict(server=dict(ledger=dict(height=1900,state_root='r',hash='h'),mempool=dict(pending=0)),receipts=r) for r in rows]
    def sent_step(self):
        step=self.d/'live/01-daily-submit';step.mkdir(parents=True)
        daily.save(step/'expected-state.json',dict(canary_receipts=[dict(receipt=dict(tx_id='t'*64))]))
    def resume(self,hours=1):
        from datetime import timedelta
        with patch.object(daily,'runtime_check'),patch.object(daily,'submit') as submit,patch.object(daily,'continue_after_submit') as cont:
            daily.resume(self.d,now=self.cutoff+timedelta(hours=hours));return submit,cont

    def test_a_fresh_capture_is_submitted(self):
        submit,cont=self.resume();submit.assert_called_once();cont.assert_not_called()
    def test_an_old_capture_is_refused_and_names_the_command(self):
        with self.assertRaisesRegex(ValueError,r'^python3 ops/b24-live/daily.py --now: this capture is 13\.0 hours old'):self.resume(hours=13)
    def test_the_run_lock_is_taken(self):
        (self.tmp/'daily').mkdir()
        with (self.tmp/'daily/run.lock').open('a') as held:
            fcntl.flock(held,fcntl.LOCK_EX)
            with self.assertRaises(BlockingIOError):self.resume()
    def test_a_scheduled_run_in_progress_refuses(self):
        with patch.object(daily.subprocess,'check_output',return_value='activating\n'),self.assertRaisesRegex(ValueError,'Scheduled B24 reading is active'):self.resume()
    def test_a_submit_that_was_prepared_and_never_sent_is_set_aside_and_sent(self):
        self.sent_step()
        with patch.object(daily,'read_six',return_value=self.six([[]]*6)):submit,cont=self.resume()
        submit.assert_called_once();cont.assert_not_called()
        self.assertFalse((self.d/'live').exists());self.assertEqual(len(list(self.d.glob('live-unsent-*'))),1)
    def test_a_step_folder_without_its_files_was_never_sent(self):
        (self.d/'live/01-daily-submit').mkdir(parents=True);submit,cont=self.resume();submit.assert_called_once()
    def test_a_submit_accepted_on_all_six_continues_after_it_even_when_old(self):
        self.sent_step()
        with patch.object(daily,'read_six',return_value=self.six([[dict(tx_id='t'*64,accepted=True)]]*6)):submit,cont=self.resume(hours=30)
        cont.assert_called_once();submit.assert_not_called();self.assertTrue((self.d/'live').exists())
    def test_a_submit_on_some_validators_only_is_refused(self):
        self.sent_step()
        with patch.object(daily,'read_six',return_value=self.six([[dict(tx_id='t'*64,accepted=True)]]*3+[[]]*3)),self.assertRaisesRegex(RuntimeError,'not accepted on all six'):self.resume()
        self.assertTrue((self.d/'live').exists())
    def test_a_pending_transaction_is_not_treated_as_never_sent(self):
        self.sent_step();rows=self.six([[]]*6);rows[2]['server']['mempool']['pending']=1
        with patch.object(daily,'read_six',return_value=rows),self.assertRaises(RuntimeError):self.resume()

if __name__=='__main__':unittest.main()
